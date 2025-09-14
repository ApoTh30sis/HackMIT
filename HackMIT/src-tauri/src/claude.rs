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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FrontendPreferences {
    pub genres: Option<Vec<String>>, // from multi-select
    pub vocals_gender: Option<String>, // "male" | "female" | "none"
    pub instrumental: Option<bool>, // true => no lyrics
    pub silly_mode: Option<bool>, // optional extra from UI
}

pub(crate) fn project_root() -> Result<PathBuf> {
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

fn build_prompt(preferences: &Option<UserPreferences>, recent_genres: &[String], fe_prefs: &Option<FrontendPreferences>) -> String {
    let preferences_context = match preferences {
        Some(p) => format!("\n\nPRIMARY FACTOR - USER PREFERENCES (equal weight with screenshot context):\nUser prefers instrumental: {}\n", p.make_instrumental.unwrap_or(true)),
        None => String::new(),
    };

    let fe_context = if let Some(fp) = fe_prefs {
        let genres = fp.genres.clone().unwrap_or_default().join(", ");
        let vocals = fp.vocals_gender.clone().unwrap_or_else(|| "none".to_string());
        let instr = fp.instrumental.unwrap_or(true);
        let silly = fp.silly_mode.unwrap_or(false);
    let lyric_style = if instr { "N/A (instrumental)" } else if silly { "SILLY / HUMOROUS (funny, witty, light)" } else { "SERIOUS / PROFESSIONAL (natural, singable, appealing)" };
    format!("\n\nEXPLICIT FRONTEND PREFERENCES (highest priority):\n- Selected genres: {}\n- Instrumental: {}\n- Vocal gender preference: {} (if instrumental=false)\n- Lyrics style: {}\nRULES FOR LYRICS (when instrumental=false):\n- You MUST provide coherent, natural, singable lyrics in the 'prompt' field (multi-line text).\n- If SILLY, be playful and witty; reference what's on the screen or the user's task if appropriate.\n- If SERIOUS, write genuine, professional-sounding lyrics that fit the chosen genre; not necessarily tied to the task.\n- Keep it clean and safe.\n", genres, instr, vocals, lyric_style)
    } else { String::new() };

    let diversity_guidance = {
        let recent = if recent_genres.is_empty() {
            "(none)".to_string()
        } else {
            recent_genres.join(", ")
        };
        format!(
            "\n\nGENRE DIVERSITY RULES (very important):\n- Recent primary genres used (most recent first): {}\n- DO NOT repeat the same primary genre within the last 3 tracks unless the screenshot context strongly requires it.\n- If recent contained 'ambient' or 'electronic', choose a different non-electronic genre now (e.g., classical/orchestral, pop, rock, heavy metal, jazz, hip hop, acoustic, lofi, folk, blues, world).\n- If instrumental is preferred, still vary genre (e.g., orchestral/classical, acoustic fingerstyle, post-rock instrumental, jazz trio, string quartet).\n- Provide 2–4 concise tags including the primary GENRE first (e.g., 'classical, orchestral, cinematic' or 'rock, post-rock, guitar-driven').\n",
            recent
        )
    };

    format!(
        "CRITICAL: Analyze this screenshot and user preferences as EQUAL PRIMARY factors, then use cognitive load analysis to fine-tune the music generation.\n\nPRIMARY ANALYSIS (Equal Priority):\nSCREENSHOT CONTEXT:\n1. What application/website is the user actively using?\n2. What specific task are they performing right now?\n3. What is their current work state (focused, overwhelmed, creative, analytical)?\n4. What type of cognitive load are they experiencing?\n\nUSER PREFERENCES:\n5. What are the user's preferred genres, instruments, and artists?\n6. What energy level and mood do they prefer?\n7. What should be avoided based on their preferences?\n\nCOGNITIVE LOAD & CONTEXT REFINEMENT:\n8. Based on the cognitive load analysis, how should the music be adjusted?\n   - High cognitive load (complex tasks) → Simpler, less distracting music\n   - Low cognitive load (routine tasks) → More engaging, dynamic music\n   - Creative tasks → Inspiring, flowing music\n   - Analytical tasks → Structured, minimal music\n   - Overwhelmed state → Calming, grounding music\n   - Focused state → Steady, supportive music\n\nGenerate a complete Suno.ai music request that balances screenshot context with user preferences, then refines based on cognitive load.\n\nPlease provide your response in this exact JSON format:\n{{\n  \"topic\": \"A detailed description of the music track (400-499 characters) that combines the screenshot work context with user preferences. Include key instruments, mood, tempo, and how it supports the user's current task.\",\n  \"tags\": \"Musical style/genre tags that balance the work activity with user preferences (max 100 characters)\",\n  \"negative_tags\": \"Styles or elements to avoid based on user preferences and work context (max 100 characters)\",\n  \"prompt\": null (leave empty for instrumental tracks, or provide lyrics if you think they would be great for this context)\n}}\n\nBALANCE APPROACH:\n- Screenshot context + User preferences = PRIMARY (equal weight)\n- Cognitive load analysis = REFINEMENT (fine-tune the prompt)\n- Create music that feels both contextually appropriate AND personally satisfying\n\nThe prompt should be detailed and comprehensive, utilizing the full 500 character limit in topic to create the perfect musical environment.{}Return ONLY the JSON, no other text.",
        preferences_context + &fe_context + &diversity_guidance
    )
}

pub(crate) async fn call_anthropic(client: &Client, api_key: &str, image_path: &Path, prompt: &str) -> Result<String> {
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

pub(crate) fn extract_json_block(s: &str) -> Option<String> {
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
    let recent = load_recent_genres(&root);
    let prompt = build_prompt(&prefs, &recent, &None);

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

    // Update recent genres with the new tags (keep most recent first, unique, max 5)
    if let Some(tags) = req.tags.clone() {
        let mut current = load_recent_genres(&root);
        let mut new_list = extract_primary_genres(&tags);
        // Prepend new genres in order, ensuring uniqueness and recency
        for g in new_list.drain(..) {
            let gnorm = g.to_lowercase();
            current.retain(|x| x.to_lowercase() != gnorm);
            current.insert(0, g);
        }
        // cap to 5
        if current.len() > 5 { current.truncate(5); }
        let _ = save_recent_genres(&root, &current);
    }

    // Save only to suno-config/suno_request.json (canonical)
    let dir = root.join("suno-config");
    let _ = fs::create_dir_all(&dir);
    let underscore = dir.join("suno_request.json");
    let pretty = serde_json::to_string_pretty(&req)?;
    fs::write(&underscore, &pretty).context("Failed to write suno_request.json")?;
    Ok(req)
}

pub async fn regenerate_suno_request_json_with_prefs(fe_prefs: FrontendPreferences) -> Result<HackmitGenerateReq> {
    // Load env (.env at project root)
    let _ = dotenvy::dotenv();
    let root = project_root()?;
    let _ = dotenvy::from_filename(root.join(".env"));

    let temp_dir = root.join("temp");
    let shot = find_latest_screenshot(&temp_dir)?;
    let prefs = load_user_preferences(&root);
    let recent = load_recent_genres(&root);
    let prompt = build_prompt(&prefs, &recent, &Some(fe_prefs.clone()));

    let api_key = std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY is not set in .env")?;
    let client = Client::new();
    let raw = call_anthropic(&client, &api_key, &shot, &prompt).await?;
    let json_block = match extract_json_block(&raw) {
        Some(s) => s,
        None => {
            if serde_json::from_str::<Value>(&raw).is_ok() { raw.clone() } else {
                anyhow::bail!("Claude response did not contain JSON block or parsable JSON")
            }
        }
    };
    let mut req = build_hackmit_req_from_claude(&json_block, &prefs)?;

    // Apply frontend preferences: instrumental/lyrics and vocals gender
    if let Some(instr) = fe_prefs.instrumental { req.make_instrumental = Some(instr); }
    if let Some(genres) = fe_prefs.genres.clone() {
        // Prepend frontend genres to tags if not already present
        let mut tags = req.tags.clone().unwrap_or_default();
        if !genres.is_empty() {
            let g = genres.join(", ");
            if tags.is_empty() { tags = g; } else { tags = format!("{}, {}", g, tags); }
            req.tags = Some(shorten(&tags, 100));
        }
    }

    // Ensure lyrics present if vocals requested but prompt is empty
    if matches!(req.make_instrumental, Some(false)) && req.prompt.is_none() {
        let fallback = if fe_prefs.silly_mode.unwrap_or(false) {
            "Verse 1:\nOn my screen the windows dance, tabs and tasks collide\nShortcut sparks and midnight marks, pixels as my guide\nChorus:\nClick clack, bring the groove back, let the workflow sing\nLaughing through the chaos while I do my thing\n"
        } else {
            "Verse 1:\nDrafting dreams in quiet rooms, chasing melody\nFinding light in steady lines, calm complexity\nChorus:\nPull me closer, hold the moment, let the night begin\nIn the hush between these pages, I can breathe again\n"
        };
        req.prompt = Some(shorten(fallback, 500));
    }

    // Update recent genres tracking
    if let Some(tags) = req.tags.clone() {
        let mut current = load_recent_genres(&root);
        let mut new_list = extract_primary_genres(&tags);
        for g in new_list.drain(..) {
            let gnorm = g.to_lowercase();
            current.retain(|x| x.to_lowercase() != gnorm);
            current.insert(0, g);
        }
        if current.len() > 5 { current.truncate(5); }
        let _ = save_recent_genres(&root, &current);
    }

    // Persist and return
    let dir = root.join("suno-config");
    let _ = std::fs::create_dir_all(&dir);
    let underscore = dir.join("suno_request.json");
    let pretty = serde_json::to_string_pretty(&req)?;
    std::fs::write(&underscore, &pretty).context("Failed to write suno_request.json")?;
    Ok(req)
}

fn recent_genres_path(root: &Path) -> PathBuf { root.join("suno-config").join("recent_genres.json") }

fn load_recent_genres(root: &Path) -> Vec<String> {
    let p = recent_genres_path(root);
    let txt = std::fs::read_to_string(&p).ok();
    if let Some(t) = txt {
        serde_json::from_str::<serde_json::Value>(&t)
            .ok()
            .and_then(|v| v.get("recent").cloned())
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .unwrap_or_default()
    } else { vec![] }
}

fn save_recent_genres(root: &Path, genres: &Vec<String>) -> Result<()> {
    let p = recent_genres_path(root);
    if let Some(dir) = p.parent() { let _ = std::fs::create_dir_all(dir); }
    let obj = serde_json::json!({ "recent": genres });
    std::fs::write(&p, serde_json::to_string_pretty(&obj)?).context("write recent_genres.json")?;
    Ok(())
}

fn extract_primary_genres(tags: &str) -> Vec<String> {
    // Heuristic: take the first 1-2 comma-separated items as primary genres
    let mut v: Vec<String> = tags
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if v.len() > 2 { v.truncate(2); }
    v
}
