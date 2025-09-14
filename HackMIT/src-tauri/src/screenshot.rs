use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{Duration, Instant};
use tauri::Emitter;
use device_query::DeviceQuery;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    // Write PNG for debugging/Claude
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().context("PNG write_header failed")?;
        writer.write_image_data(&buffer).context("PNG write_image_data failed")?;
    }
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let _ = std::fs::write(path, &png_bytes);
    Ok((width, height, buffer))
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
    pub action: String, // "continue" or "switch_with_fade"
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
    // Use a faster, smaller Claude call for low latency classification
    let raw = crate::claude::call_anthropic_quick(&client, &api_key, image_path, prompt)
        .await
        .context("Claude classify call failed")?;
    let maybe = crate::claude::extract_json_block(&raw).unwrap_or(raw);
    #[derive(Deserialize)]
    struct Resp { tag: String, details: String }
    let parsed: Resp = serde_json::from_str(&maybe).context("Parse context summary JSON failed")?;
    Ok(ContextSummary { tag: parsed.tag, details: parsed.details, app: None })
}

// Basic tag comparison used for switch decision (no image similarity thresholds)
fn tags_differ(a: &ContextSummary, b: &ContextSummary) -> bool {
    !a.tag.eq_ignore_ascii_case(&b.tag)
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

// Fast image hash for context change detection
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
        prev_sig: Option<ImageSig>,
        last_switch: Option<Instant>,
    }

    let root = crate::claude::project_root().unwrap_or(std::env::current_dir().unwrap());
    let shot_path = root.join("temp").join("current.png");
    let state = Arc::new(Mutex::new(SharedState {
        prev_sig: None,
        last_switch: None,
    }));
    let app = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        // Screenshot every 5 seconds
        let mut ticker = tokio::time::interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;

            // Capture screenshot
            let (w, h, rgba) = match capture_active_display(&shot_path) {
                Ok(v) => v,
                Err(e) => { 
                    let _ = app.emit("screenshot:error", format!("capture failed: {e}")); 
                    continue; 
                }
            };

            // Compute image hash
            let current_sig = match compute_sig(w, h, &rgba) { 
                Ok(s) => s, 
                Err(e) => { 
                    let _ = app.emit("screenshot:error", format!("hash failed: {e}")); 
                    continue; 
                } 
            };

            // Check for context change
            let mut should_switch;
            {
                let mut st = state.lock().await;
                let distance = match st.prev_sig.as_ref() {
                    Some(prev) => sig_distance(&current_sig, prev),
                    None => 999, // First screenshot = big change
                };

                // Calculate maximum possible distance for 8x8 hash (64 bits)
                // Each bit can differ, so max distance is 64
                const MAX_HASH_DISTANCE: u32 = 64;
                const CHANGE_THRESHOLD_PERCENT: f32 = 0.10; // 10%
                const THRESHOLD_DISTANCE: u32 = (MAX_HASH_DISTANCE as f32 * CHANGE_THRESHOLD_PERCENT) as u32;
                
                should_switch = distance > THRESHOLD_DISTANCE;
                println!("Hash distance: {} (max: {}, threshold: {}), should_switch: {}", 
                    distance, MAX_HASH_DISTANCE, THRESHOLD_DISTANCE, should_switch);
                
                // Rate limiting: don't switch more than once every 3 seconds
                if should_switch {
                    if let Some(last) = st.last_switch {
                        if last.elapsed() < Duration::from_secs(3) {
                            should_switch = false;
                            println!("Rate limited: too soon since last switch");
                        }
                    }
                }

                if should_switch {
                    st.last_switch = Some(Instant::now());
                }
                st.prev_sig = Some(current_sig);
            }

            // Emit context decision immediately
            let app_name = frontmost_app_name();
            let summary = ContextSummary {
                tag: app_name.clone().unwrap_or_else(|| "unknown".to_string()),
                details: format!("App: {:?}", app_name),
                app: app_name.clone(),
            };

            let action = if should_switch { "switch_with_fade" } else { "continue" };
            let evt = DecisionEvent {
                current_context: summary.clone(),
                previous_context: None,
                is_similar: !should_switch,
                action: action.to_string(),
            };
            let _ = app.emit("context:decision", &evt);

            // If significant change detected, trigger music generation
            if should_switch {
                println!("Context change detected - triggering music generation");
                let app_clone = app.clone();
                tokio::spawn(async move {
                    // Call Claude to analyze the screenshot and generate Suno request
                    match crate::claude::regenerate_suno_request_json().await {
                        Ok(_suno_request) => {
                            println!("Claude analysis completed, generated Suno request");
                            
                            // Call Suno to generate music
                            match crate::suno::suno_hackmit_generate_and_wait().await {
                                Ok(audio_url) => {
                                    println!("Suno generation completed, switching to new audio stream");
                                    
                                    // Emit event to frontend to switch to new audio stream
                                    let _ = app_clone.emit("music:switch", audio_url);
                                },
                                Err(e) => {
                                    println!("Suno generation failed: {}", e);
                                    let _ = app_clone.emit("music:error", format!("Suno generation failed: {}", e));
                                }
                            }
                        },
                        Err(e) => {
                            println!("Claude analysis failed: {}", e);
                            let _ = app_clone.emit("music:error", format!("Claude analysis failed: {}", e));
                        }
                    }
                });
            }
        }
    });
}
