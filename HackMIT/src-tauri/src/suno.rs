use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenerateRequest {
    pub prompt: Option<String>,
    pub style: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "customMode")]
    pub custom_mode: bool,
    pub instrumental: bool,
    pub model: String,
    #[serde(rename = "negativeTags")]
    pub negative_tags: Option<String>,
    #[serde(rename = "vocalGender")]
    pub vocal_gender: Option<String>,
    #[serde(rename = "styleWeight")]
    pub style_weight: Option<f32>,
    #[serde(rename = "weirdnessConstraint")]
    pub weirdness_constraint: Option<f32>,
    #[serde(rename = "audioWeight")]
    pub audio_weight: Option<f32>,
    #[serde(rename = "callBackUrl")]
    pub callback_url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GenerateResponse {
    pub code: i32,
    pub msg: String,
    pub data: Option<GenerateData>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GenerateData {
    #[serde(rename = "taskId")]
    pub task_id: String,
}

const SUNO_API_URL: &str = "https://api.sunoapi.org/api/v1/generate";
const SUNO_STATUS_URL: &str = "https://api.sunoapi.org/api/v1/generate/record-info";
const SUNO_CREDITS_URL: &str = "https://api.sunoapi.org/api/v1/get-credits";
const HACKMIT_GENERATE_URL: &str = "https://studio-api.prod.suno.com/api/v2/external/hackmit/generate";
const HACKMIT_CLIPS_URL: &str = "https://studio-api.prod.suno.com/api/v2/external/hackmit/clips";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TrackInfo {
    pub id: Option<String>,
    pub title: Option<String>,
    pub tags: Option<String>,
    pub duration: Option<f32>,
    #[serde(rename = "audio_url")]
    pub audio_url: Option<String>,
    #[serde(rename = "stream_audio_url")]
    pub stream_audio_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StatusInnerResponse {
    pub data: Option<Vec<TrackInfo>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StatusData {
    #[serde(rename = "taskId")]
    pub task_id: String,
    pub status: Option<String>,
    pub response: Option<StatusInnerResponse>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StatusResponse {
    pub code: i32,
    pub msg: String,
    pub data: Option<StatusData>,
}

#[tauri::command]
pub async fn suno_generate_from_file() -> Result<String, String> {
    // Load .env once (it's ok to call multiple times; it’s idempotent)
    let _ = dotenvy::dotenv();

    // Read request.json from repo root/suno-config
    let base_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    // Also try loading env from suno-config/.env explicitly
    let _ = dotenvy::from_filename(base_dir.join("suno-config").join(".env"));

    let api_key = std::env::var("SUNO_API_KEY").map_err(|_| {
        "SUNO_API_KEY not set. Put it in suno-config/.env as SUNO_API_KEY=...".to_string()
    })?;
    let req_path = base_dir.join("suno-config").join("request.json");
    let req_text = std::fs::read_to_string(&req_path)
        .map_err(|e| format!("Failed reading {}: {}", req_path.display(), e))?;
    let payload: GenerateRequest = serde_json::from_str(&req_text)
        .map_err(|e| format!("Invalid JSON in request.json: {}", e))?;

    let client = reqwest::Client::new();
    let res = client
        .post(SUNO_API_URL)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    let status = res.status();
    let text = res.text().await.map_err(|e| e.to_string())?;

    if !status.is_success() {
        return Err(format!("Suno API error ({}): {}", status, text));
    }

    let parsed: GenerateResponse = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse response: {}. Raw: {}", e, text))?;

    if parsed.code != 200 {
        return Err(format!("Suno API returned code {}: {}", parsed.code, parsed.msg));
    }

    let task_id = parsed
        .data
        .ok_or_else(|| "Missing data in response".to_string())?
        .task_id;

    Ok(task_id)
}

async fn load_api_key() -> Result<String, String> {
    let _ = dotenvy::dotenv();
    // Walk up ancestors to find suno-config/.env
    if std::env::var("SUNO_API_KEY").is_err() {
        if let Some(env_path) = find_suno_config_file(".env") {
            let _ = dotenvy::from_filename(&env_path);
        }
    }
    std::env::var("SUNO_API_KEY").map_err(|_| {
        "SUNO_API_KEY not set. Put it in suno-config/.env as SUNO_API_KEY=...".to_string()
    })
}

async fn load_request() -> Result<GenerateRequest, String> {
    let path = find_suno_config_file("request.json")
        .ok_or_else(|| "Could not find suno-config/request.json".to_string())?;
    let req_text = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed reading {}: {}", path.display(), e))?;
    serde_json::from_str(&req_text).map_err(|e| format!("Invalid JSON in request.json: {}", e))
}

fn find_suno_config_file(name: &str) -> Option<PathBuf> {
    let start = std::env::current_dir().ok()?;
    for dir in start.ancestors() {
        let candidate = dir.join("suno-config").join(name);
        if candidate.exists() {
            return Some(candidate);
        }
        // Stop once we reach filesystem root
        if dir.parent().is_none() {
            break;
        }
    }
    None
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct CreditsData {
    credits: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct CreditsResponse {
    code: i32,
    msg: String,
    data: Option<CreditsData>,
}

#[tauri::command]
pub async fn suno_get_credits() -> Result<i64, String> {
    let api_key = load_api_key().await?;
    let client = reqwest::Client::new();
    let res = client
        .get(SUNO_CREDITS_URL)
        .bearer_auth(&api_key)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;
    let status = res.status();
    let text = res.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Credits API error ({}): {}", status, text));
    }
    let parsed: CreditsResponse = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse credits response: {}. Raw: {}", e, text))?;
    if parsed.code != 200 {
        return Err(format!("Suno API returned code {}: {}", parsed.code, parsed.msg));
    }
    Ok(parsed.data.and_then(|d| d.credits).unwrap_or(0))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct HackmitGenerateReq {
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    make_instrumental: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cover_clip_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct HackmitGenerateResp {
    id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct HackmitClip {
    id: String,
    request_id: Option<String>,
    created_at: Option<String>,
    status: Option<String>,
    title: Option<String>,
    metadata: Option<serde_json::Value>,
    audio_url: Option<String>,
}

async fn load_hackmit_request() -> Result<HackmitGenerateReq, String> {
    let path = find_suno_config_file("hackmit-request.json")
        .ok_or_else(|| "Could not find suno-config/hackmit-request.json".to_string())?;
    let txt = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed reading {}: {}", path.display(), e))?;
    serde_json::from_str(&txt).map_err(|e| format!("Invalid JSON in hackmit-request.json: {}", e))
}

#[tauri::command]
pub async fn suno_hackmit_generate_and_wait() -> Result<String, String> {
    let api_key = load_api_key().await?;
    let payload = load_hackmit_request().await?;
    let client = reqwest::Client::new();

    // 1) generate
    let gen_res = client
        .post(HACKMIT_GENERATE_URL)
        .bearer_auth(&api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("HTTP error (generate): {}", e))?;
    let status = gen_res.status();
    let gen_text = gen_res.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Generate error ({}): {}", status, gen_text));
    }
    let gen: HackmitGenerateResp = serde_json::from_str(&gen_text)
        .map_err(|e| format!("Parse generate response failed: {}. Raw: {}", e, gen_text))?;

    // 2) poll clips until audio_url present
    let max_iters = 36u32; // ~3 minutes @5s
    for _ in 0..max_iters {
        let url = format!("{}?ids={}", HACKMIT_CLIPS_URL, gen.id);
        let clips_res = client
            .get(url)
            .bearer_auth(&api_key)
            .send()
            .await
            .map_err(|e| format!("HTTP error (clips): {}", e))?;
        let st = clips_res.status();
        let clips_text = clips_res.text().await.map_err(|e| e.to_string())?;
        if !st.is_success() {
            return Err(format!("Clips error ({}): {}", st, clips_text));
        }
        // The API can return either a top-level array or an object with { clips: [...] }
    let clips: Vec<HackmitClip> = match serde_json::from_str::<Vec<HackmitClip>>(&clips_text) {
            Ok(v) => v,
            Err(_) => {
                #[derive(Deserialize)]
                struct Wrapper { clips: Vec<HackmitClip> }
                let w: Wrapper = serde_json::from_str(&clips_text)
                    .map_err(|e| format!("Parse clips response failed: {}. Raw: {}", e, clips_text))?;
                w.clips
            }
        };
        // Find any clip with audio_url present
        if let Some(url) = clips.iter().filter_map(|c| c.audio_url.clone()).next() {
            return Ok(url);
        }
        sleep(std::time::Duration::from_secs(5)).await;
    }
    Err("Timed out waiting for audio URL".to_string())
}

async fn get_status(client: &reqwest::Client, api_key: &str, task_id: &str) -> Result<StatusResponse, String> {
    let url = format!("{}?taskId={}", SUNO_STATUS_URL, task_id);
    let res = client
        .get(url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;
    let status = res.status();
    let text = res.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Status API error ({}): {}", status, text));
    }
    serde_json::from_str::<StatusResponse>(&text)
        .map_err(|e| format!("Failed to parse status response: {}. Raw: {}", e, text))
}

fn pick_stream_or_audio(tracks: &[TrackInfo]) -> Option<String> {
    // Prefer stream URL; fall back to audio_url
    tracks
        .iter()
        .filter_map(|t| t.stream_audio_url.clone().or_else(|| t.audio_url.clone()))
        .next()
}

#[tauri::command]
pub async fn suno_generate_and_wait() -> Result<String, String> {
    let api_key = load_api_key().await?;
    let payload = load_request().await?;

    let client = reqwest::Client::new();
    let res = client
        .post(SUNO_API_URL)
        .bearer_auth(&api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;
    let status = res.status();
    let text = res.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Suno API error ({}): {}", status, text));
    }
    let parsed: GenerateResponse = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse response: {}. Raw: {}", e, text))?;
    if parsed.code != 200 {
        return Err(format!("Suno API returned code {}: {}", parsed.code, parsed.msg));
    }
    let task_id = parsed
        .data
        .ok_or_else(|| "Missing data in response".to_string())?
        .task_id;

    // Poll for up to ~3 minutes; check every 5 seconds
    let max_iters = 36u32; // 36 * 5s = 180s
    for _ in 0..max_iters {
        let status = get_status(&client, &api_key, &task_id).await?;
        if status.code != 200 {
            // Keep trying unless explicit failure can be inferred
        }
        if let Some(data) = status.data {
            if let Some(ref s) = data.status {
                if s.eq_ignore_ascii_case("FAILED") { 
                    return Err("Suno generation failed".to_string());
                }
            }
            if let Some(resp) = data.response {
                if let Some(tracks) = resp.data {
                    if let Some(url) = pick_stream_or_audio(&tracks) {
                        return Ok(url);
                    }
                }
            }
        }
    sleep(std::time::Duration::from_secs(5)).await;
    }
    Err("Timed out waiting for stream URL".to_string())
}
