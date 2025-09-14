use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
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

// Removed perceptual hash and thresholds for simplicity

pub fn start_periodic_task(app_handle: tauri::AppHandle) {
    #[derive(Clone, Default)]
    struct SharedState {
        prev_summary: Option<ContextSummary>,
        infer_in_flight: bool,
        needs_rerun: bool,
    }

    let root = crate::claude::project_root().unwrap_or(std::env::current_dir().unwrap());
    let shot_path = root.join("temp").join("current.png");
    let state = Arc::new(Mutex::new(SharedState::default()));
    let app = app_handle.clone();

    tauri::async_runtime::spawn(async move {
    // 2s strikes a balance between responsiveness and CPU/network load
    let mut ticker = tokio::time::interval(Duration::from_millis(2000));
        loop {
            ticker.tick().await;

            // 1) Always capture active display to temp/current.png
            if let Err(e) = capture_active_display(&shot_path) {
                let _ = app.emit("screenshot:error", format!("capture failed: {e}"));
                continue;
            }

            // 2) Notify frontend to enqueue generation for this screenshot
            let _ = app.emit("queue:add", serde_json::json!({ "reason": "tick" }));

            // 3) Spawn a lightweight summarization task (one at a time)
            let should_spawn = {
                let mut st = state.lock().await;
                st.needs_rerun = true; // mark that a new screenshot arrived
                if st.infer_in_flight { false } else { st.infer_in_flight = true; true }
            };

            if should_spawn {
                let app_for_evt = app.clone();
                let state_cloned = state.clone();
                let shot_for_task = shot_path.clone();
                let app_name = frontmost_app_name();
                tokio::spawn(async move {
                    loop {
                        // Snapshot whether we actually need a rerun
                        {
                            let mut st = state_cloned.lock().await;
                            if !st.needs_rerun {
                                // Nothing pending; release in-flight and exit
                                st.infer_in_flight = false;
                                break;
                            }
                            // We'll consume this pending request in this iteration
                            st.needs_rerun = false;
                        }

                        let mut summary = ContextSummary { tag: "unknown".into(), details: "".into(), app: app_name.clone() };
                        match summarize_context(&shot_for_task).await {
                            Ok(mut s) => { s.app = app_name.clone(); summary = s; },
                            Err(e) => { let _ = app_for_evt.emit("screenshot:error", format!("summarize failed: {e}")); }
                        }

                        let mut st = state_cloned.lock().await;
                        let prev = st.prev_summary.clone();
                        let action = match &prev {
                            Some(p) if tags_differ(&summary, p) => "switch_with_fade",
                            None => "switch_with_fade",
                            _ => "continue",
                        };
                        let evt = DecisionEvent { current_context: summary.clone(), previous_context: prev.clone(), is_similar: false, action: action.to_string() };
                        let _ = app_for_evt.emit("context:decision", &evt);
                        st.prev_summary = Some(summary);
                        // Loop will check if needs_rerun was set during this run and continue if so
                    }
                });
            }
        }
    });
}
