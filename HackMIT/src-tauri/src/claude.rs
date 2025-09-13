use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use base64::Engine as _;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Serialize, Deserialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: Vec<Content>,
}

#[derive(Serialize, Deserialize)]
struct Content {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<ImageSource>,
}

#[derive(Serialize, Deserialize)]
struct ImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Serialize, Deserialize)]
struct AnthropicResponse {
    content: Vec<ResponseContent>,
}

#[derive(Serialize, Deserialize)]
struct ResponseContent {
    text: String,
}

// We no longer depend on strict ClaudeResponse; we'll parse flexibly from serde_json::Value

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct HackmitGenerateReq {
    #[serde(skip_serializing_if = "Option::is_none")] pub topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub make_instrumental: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] pub cover_clip_id: Option<String>,
}

#[derive(Deserialize)]
struct UserPreferences {
    make_instrumental: Option<bool>,
}

fn project_root() -> Result<PathBuf> {
    // Start from current dir and walk up to folder containing package.json (HackMIT root)
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("package.json").exists() {
            return Ok(dir);
        }
        if !dir.pop() { break; }
    }
    anyhow::bail!("Could not locate project root with package.json")
}

fn find_latest_screenshot(temp_dir: &Path) -> Result<PathBuf> {
    let mut latest: Option<(PathBuf, SystemTime)> = None;
    if !temp_dir.exists() { anyhow::bail!("temp directory not found: {}", temp_dir.display()); }
    for entry in fs::read_dir(temp_dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg") {
                let meta = entry.metadata()?;
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                match &latest {
                    Some((_, t)) if mtime <= *t => {}
                    _ => latest = Some((path.clone(), mtime)),
                }
            }
        }
    }
    latest.map(|(p, _)| p).ok_or_else(|| anyhow::anyhow!("No screenshots found in {}", temp_dir.display()))
}

fn load_user_preferences(root: &Path) -> Option<UserPreferences> {
    let prefs_path = root.join("sample_preferences.json");
    let txt = fs::read_to_string(prefs_path).ok()?;
    serde_json::from_str(&txt).ok()
}

fn build_prompt(preferences: &Option<UserPreferences>) -> String {
    let preferences_context = match preferences {
        Some(p) => format!("\n\nPRIMARY FACTOR - USER PREFERENCES (equal weight with screenshot context):\nUser prefers instrumental: {}\n", p.make_instrumental.unwrap_or(true)),
        None => String::new(),
    };

    format!(
        "CRITICAL: Analyze this screenshot and user preferences as EQUAL PRIMARY factors, then use cognitive load analysis to fine-tune the music generation.\n\nPRIMARY ANALYSIS (Equal Priority):\nSCREENSHOT CONTEXT:\n1. What application/website is the user actively using?\n2. What specific task are they performing right now?\n3. What is their current work state (focused, overwhelmed, creative, analytical)?\n4. What type of cognitive load are they experiencing?\n\nUSER PREFERENCES:\n5. What are the user's preferred genres, instruments, and artists?\n6. What energy level and mood do they prefer?\n7. What should be avoided based on their preferences?\n\nCOGNITIVE LOAD & CONTEXT REFINEMENT:\n8. Based on the cognitive load analysis, how should the music be adjusted?\n   - High cognitive load (complex tasks) → Simpler, less distracting music\n   - Low cognitive load (routine tasks) → More engaging, dynamic music\n   - Creative tasks → Inspiring, flowing music\n   - Analytical tasks → Structured, minimal music\n   - Overwhelmed state → Calming, grounding music\n   - Focused state → Steady, supportive music\n\nGenerate a complete Suno.ai music request that balances screenshot context with user preferences, then refines based on cognitive load.\n\nPlease provide your response in this exact JSON format:\n{{\n  \"topic\": \"A detailed description of the music track (400-499 characters) that combines the screenshot work context with user preferences. Include key instruments, mood, tempo, and how it supports the user's current task.\",\n  \"tags\": \"Musical style/genre tags that balance the work activity with user preferences (max 100 characters)\",\n  \"negative_tags\": \"Styles or elements to avoid based on user preferences and work context (max 100 characters)\",\n  \"prompt\": null (leave empty for instrumental tracks, or provide lyrics if you think they would be great for this context)\n}}\n\nBALANCE APPROACH:\n- Screenshot context + User preferences = PRIMARY (equal weight)\n- Cognitive load analysis = REFINEMENT (fine-tune the prompt)\n- Create music that feels both contextually appropriate AND personally satisfying\n\nThe prompt should be detailed and comprehensive, utilizing the full 500 character limit in topic to create the perfect musical environment.{}Return ONLY the JSON, no other text.",
        preferences_context
    )
}

async fn call_anthropic(client: &Client, api_key: &str, image_path: &Path, prompt: &str) -> Result<String> {
    let image_bytes = fs::read(image_path).with_context(|| format!("Failed to read image: {}", image_path.display()))?;
    let base64_data = BASE64_STD.encode(&image_bytes);
    // determine media type
    let media_type = match image_path.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()) {
        Some(ref ext) if ext == "jpg" || ext == "jpeg" => "image/jpeg",
        Some(ref ext) if ext == "png" => "image/png",
        _ => "image/png",
    };

    let req = AnthropicRequest {
        model: "claude-3-5-haiku-latest".to_string(),
        max_tokens: 1000,
        messages: vec![Message {
            role: "user".into(),
            content: vec![
                Content { content_type: "text".into(), text: Some(prompt.to_string()), source: None },
                Content { content_type: "image".into(), text: None, source: Some(ImageSource { source_type: "base64".into(), media_type: media_type.into(), data: base64_data }) },
            ],
        }],
    };

    let res = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&req)
        .send()
        .await
        .context("Failed to call Anthropic API")?;
    let status = res.status();
    let text = res.text().await.unwrap_or_default();
    if !status.is_success() { anyhow::bail!("Anthropic error ({}): {}", status, text); }
    let parsed: AnthropicResponse = serde_json::from_str(&text).context("Parse Anthropic response failed")?;
    let first = parsed.content.first().ok_or_else(|| anyhow::anyhow!("Empty content from Anthropic"))?;
    Ok(first.text.clone())
}

fn extract_json_block(s: &str) -> Option<String> {
    // If Claude returned a fenced block ```json ... ```, strip the fences first
    let trimmed = s.trim();
    let without_fence = if let Some(start) = trimmed.find("```") {
        // try to find the closing fence
        if let Some(end) = trimmed.rfind("```") {
            let inner = &trimmed[start + 3..end];
            // remove optional 'json' language hint
            inner.trim_start_matches(|c: char| c == 'j' || c == 's' || c == 'o' || c == 'n' || c.is_whitespace()).trim()
                .to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    };

    let start = without_fence.find('{')?;
    let end = without_fence.rfind('}')?;
    Some(without_fence[start..=end].to_string())
}

fn as_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Array(arr)) => {
            // join array of strings into a comma-separated string
            let parts: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
            if parts.is_empty() { None } else { Some(parts.join(", ")) }
        }
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::Bool(b)) => Some(b.to_string()),
        _ => None,
    }
}

fn shorten(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    let take = max.saturating_sub(3);
    format!("{}...", s.chars().take(take).collect::<String>())
}

fn build_hackmit_req_from_claude(json_str: &str, prefs: &Option<UserPreferences>) -> Result<HackmitGenerateReq> {
    // Try strict parse first
    let mut v: Value = serde_json::from_str(json_str).context("Failed to parse Claude JSON")?;

    // Support top-level object or nested under a known key
    if let Some(obj) = v.get("request").cloned() { v = obj; }

    let topic = as_string(v.get("topic")).or_else(|| as_string(v.get("title")));
    let tags = as_string(v.get("tags"));
    let prompt = as_string(v.get("prompt"));

    let topic = topic.unwrap_or_else(|| "Generated track".to_string());
    let mut tags = tags.unwrap_or_else(|| "cinematic, ambient".to_string());
    tags = shorten(&tags, 100);
    let prompt = prompt.map(|p| shorten(&p, 500));

    let make_instrumental = prefs.as_ref().and_then(|p| p.make_instrumental).unwrap_or(true);
    Ok(HackmitGenerateReq {
        topic: Some(topic),
        tags: Some(tags),
        prompt,
        make_instrumental: Some(make_instrumental),
        cover_clip_id: None,
    })
}

pub async fn regenerate_suno_request_json() -> Result<HackmitGenerateReq> {
    // Load env (.env at project root)
    let _ = dotenvy::dotenv();
    // Find root and latest screenshot
    let root = project_root()?;
    // Explicitly load root .env
    let _ = dotenvy::from_filename(root.join(".env"));

    let temp_dir = root.join("temp");
    let shot = find_latest_screenshot(&temp_dir)?;
    let prefs = load_user_preferences(&root);
    let prompt = build_prompt(&prefs);

    let api_key = std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY is not set in .env")?;
    let client = Client::new();
    let raw = call_anthropic(&client, &api_key, &shot, &prompt).await?;
    let json_block = match extract_json_block(&raw) {
        Some(s) => s,
        None => {
            // Try raw as-is in case Claude responded with bare JSON
            if serde_json::from_str::<Value>(&raw).is_ok() { raw.clone() } else {
                anyhow::bail!("Claude response did not contain JSON block or parsable JSON")
            }
        }
    };
    let req = build_hackmit_req_from_claude(&json_block, &prefs)?;

    // Save only to suno-config/suno_request.json (canonical)
    let dir = root.join("suno-config");
    let _ = fs::create_dir_all(&dir);
    let underscore = dir.join("suno_request.json");
    let pretty = serde_json::to_string_pretty(&req)?;
    fs::write(&underscore, &pretty).context("Failed to write suno_request.json")?;
    Ok(req)
}
