use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{Duration, Instant};
use tauri::Emitter;
use device_query::DeviceQuery;
use std::sync::Arc;
use tokio::sync::Mutex;
use image::{ImageBuffer, Rgba};

// Capture screenshot using "screenshots" crate
fn capture_active_display(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    use screenshots::Screen; // macOS supported
    // Try to pick screen under current mouse cursor; fall back to (0,0)
    let (mx, my) = {
        let dev = device_query::DeviceState::new();
        let m = dev.get_mouse();
        (m.coords.0, m.coords.1)
    };
    let screen = Screen::from_point(mx, my).or_else(|_| Screen::from_point(0, 0))
        .context("No screen found to capture")?;
    let img = screen.capture().context("Failed to capture screen")?;
    let width = img.width();
    let height = img.height();
    let buffer = img.into_raw();

    // Create an ImageBuffer from the raw buffer
    let img_buf = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, buffer.clone()).ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

    // Calculate the new dimensions for 720p height, maintaining aspect ratio
    let new_height = 720;
    let new_width = (width as f32 * (new_height as f32 / height as f32)).round() as u32;

    // Resize the image
    let resized_img = image::imageops::resize(&img_buf, new_width, new_height, image::imageops::FilterType::Lanczos3);
    
    // Get the raw bytes of the resized image
    let resized_buffer = resized_img.into_raw();

    // Write PNG for debugging/Claude
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, new_width, new_height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().context("PNG write_header failed")?;
        writer.write_image_data(&resized_buffer).context("PNG write_image_data failed")?;
    }
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let _ = std::fs::write(path, &png_bytes);
    Ok((new_width, new_height, resized_buffer))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummary {
    pub tag: String,           // short label, e.g., "vscode", "browser-google-docs"
    pub details: String,       // brief sentence
    pub app: Option<String>,   // frontmost app name
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEvent {
    pub current_context: ContextSummary,
    pub previous_context: Option<ContextSummary>,
    pub is_similar: bool,
    pub action: String, // "continue_and_queue" or "switch_with_fade"
}

async fn summarize_context(image_path: &Path) -> Result<ContextSummary> {
    // Reuse Claude caller but with a smaller prompt and token budget
    let prompt = "You are classifying the user's current activity from a screenshot.\nReturn JSON ONLY as:\n{\n  tag: stable kebab-case tag focusing on app/site and activity (e.g., 'vscode-coding', 'chrome-docs', 'terminal-build', 'figma-design'),\n  details: one short sentence\n}\nKeep the tag stable across very similar screenshots.";
    // Use existing function to call Anthropic with image; then parse JSON
    let _ = dotenvy::dotenv();
    let root = crate::claude::project_root().context("Find project root failed")?;
    let _ = dotenvy::from_filename(root.join(".env"));
    let api_key = std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY missing")?;
    let client = reqwest::Client::new();
    let raw = crate::claude::call_anthropic(&client, &api_key, image_path, prompt)
        .await
        .context("Claude classify call failed")?;
    let maybe = crate::claude::extract_json_block(&raw).unwrap_or(raw);
    #[derive(Deserialize)]
    struct Resp { tag: String, details: String }
    let parsed: Resp = serde_json::from_str(&maybe).context("Parse context summary JSON failed")?;
    Ok(ContextSummary { tag: parsed.tag, details: parsed.details, app: None })
}

fn similar(a: &ContextSummary, b: &ContextSummary) -> bool {
    // Higher confidence similarity: same app name OR same tag prefix
    let app_same = match (&a.app, &b.app) {
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
        _ => false,
    };
    if app_same { return true; }
    let a_prefix = a.tag.split('-').next().unwrap_or("");
    let b_prefix = b.tag.split('-').next().unwrap_or("");
    a_prefix == b_prefix
}

fn frontmost_app_name() -> Option<String> {
    // macOS: use AppleScript via osascript (may require Accessibility permission)
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let script = r#"tell application \"System Events\" to get name of first process whose frontmost is true"#;
        if let Ok(out) = Command::new("osascript").arg("-e").arg(script).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() { return Some(s); }
            }
        }
    }
    None
}

// Lightweight difference check using perceptual hash
#[derive(Clone)]
struct ImageSig {
    hash: img_hash::ImageHash,
}

fn compute_sig(width: u32, height: u32, rgba: &[u8]) -> Result<ImageSig> {
    use img_hash::{HasherConfig, HashAlg};
    use img_hash::image::{ImageBuffer, Rgba, DynamicImage};
    let buf: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_vec(width, height, rgba.to_vec())
        .ok_or_else(|| anyhow::anyhow!("Failed to build image buffer"))?;
    let dynimg = DynamicImage::ImageRgba8(buf);
    let hasher = HasherConfig::new().hash_alg(HashAlg::Mean).hash_size(8, 8).to_hasher();
    let hash = hasher.hash_image(&dynimg);
    Ok(ImageSig { hash })
}

fn sig_distance(a: &ImageSig, b: &ImageSig) -> u32 {
    a.hash.dist(&b.hash)
}

pub fn start_periodic_task(app_handle: tauri::AppHandle) {
    #[derive(Clone)]
    struct SharedState {
        prev_summary: Option<ContextSummary>,
        prev_sig: Option<ImageSig>,
        last_infer: Instant,
        last_switch_at: Option<Instant>,
        pending_diff_count: u8,
        infer_in_flight: bool,
    }

    let root = crate::claude::project_root().unwrap_or(std::env::current_dir().unwrap());
    let shot_path = root.join("temp").join("current.png");
    let state = Arc::new(Mutex::new(SharedState {
        prev_summary: None,
        prev_sig: None,
        last_infer: Instant::now() - Duration::from_secs(60),
        last_switch_at: None,
        pending_diff_count: 0,
        infer_in_flight: false,
    }));
    let app = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            // Capture (active display) without blocking future ticks
            let (w, h, rgba) = match capture_active_display(&shot_path) {
                Ok(v) => v,
                Err(e) => { let _ = app.emit("screenshot:error", format!("capture failed: {e}")); continue; }
            };
            let sig = match compute_sig(w, h, &rgba) { Ok(s) => s, Err(e) => { let _ = app.emit("screenshot:error", format!("sig failed: {e}")); continue; } };
            let app_name = frontmost_app_name();

            let state_cloned = state.clone();
            let app_for_evt = app.clone();
            let shot_for_task = shot_path.clone();
            let current_sig = sig.clone();

            let mut should_spawn = false;
            let mut prior_sig_for_task: Option<ImageSig> = None;
            {
                let mut st = state_cloned.lock().await;
                // Determine change vs previous
                let prior_sig = st.prev_sig.clone();
                let need_visual_change = prior_sig.as_ref().map(|ps| sig_distance(&current_sig, ps) > 10).unwrap_or(true);
                let prev_app = st.prev_summary.as_ref().and_then(|s| s.app.clone());
                let app_changed = match (&app_name, &prev_app) { (Some(a), Some(b)) => !a.eq_ignore_ascii_case(&b), _ => false };
                let need_infer = need_visual_change || app_changed || st.prev_summary.is_none();
                let allow_infer = st.last_infer.elapsed() >= Duration::from_secs(3);

                // update latest sig
                st.prev_sig = Some(current_sig.clone());
                prior_sig_for_task = prior_sig;

                if need_infer && allow_infer && !st.infer_in_flight {
                    st.infer_in_flight = true;
                    should_spawn = true;
                }
            }

            if should_spawn {
                tokio::spawn(async move {
                    // Run summarization asynchronously
                    let mut summary = ContextSummary { tag: "unknown".into(), details: "".into(), app: app_name.clone() };
                    match summarize_context(&shot_for_task).await {
                        Ok(mut s) => { s.app = app_name.clone(); summary = s; },
                        Err(e) => { let _ = app_for_evt.emit("screenshot:error", format!("summarize failed: {e}")); }
                    }

                    // Decide and emit
                    let mut st = state_cloned.lock().await;
                    st.last_infer = Instant::now();
                    let prev_summary = st.prev_summary.clone();
                    let latest_sig = st.prev_sig.clone();

                    let dist_small = match (&prior_sig_for_task, &latest_sig) {
                        (Some(prev), Some(latest)) => sig_distance(prev, latest) <= 10,
                        _ => false,
                    };
                    let is_similar_raw = match &prev_summary {
                        Some(p) => {
                            let app_same = match (&summary.app, &p.app) { (Some(a), Some(b)) => a.eq_ignore_ascii_case(b), _ => false };
                            let tag_same = summary.tag == p.tag || summary.tag.split('-').next() == p.tag.split('-').next();
                            (app_same && dist_small) || (tag_same && dist_small)
                        },
                        None => false,
                    };
                    let is_similar = if is_similar_raw { st.pending_diff_count = 0; true } else { st.pending_diff_count = st.pending_diff_count.saturating_add(1); st.pending_diff_count < 2 };

                    let mut action = if is_similar { "continue_and_queue" } else { "switch_with_fade" };
                    if action == "switch_with_fade" {
                        let big_change = match (&prior_sig_for_task, &latest_sig) { (Some(a), Some(b)) => sig_distance(a, b) > 20, _ => true };
                        let app_changed = match (&summary.app, &prev_summary.as_ref().and_then(|s| s.app.clone())) { (Some(a), Some(b)) => !a.eq_ignore_ascii_case(&b), _ => false };
                        if let Some(t) = st.last_switch_at {
                            if t.elapsed() < Duration::from_secs(12) && !big_change && !app_changed {
                                action = "continue_and_queue";
                            }
                        }
                    }

                    let evt = DecisionEvent {
                        current_context: summary.clone(),
                        previous_context: prev_summary.clone(),
                        is_similar,
                        action: action.to_string(),
                    };
                    let _ = app_for_evt.emit("context:decision", &evt);
                    if action == "switch_with_fade" { st.last_switch_at = Some(Instant::now()); }
                    st.prev_summary = Some(summary);
                    st.infer_in_flight = false;
                });
            }
        }
    });
}
