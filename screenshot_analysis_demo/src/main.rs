use anyhow::{Context, Result};
use base64::Engine;
use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

#[derive(Parser)]
#[command(name = "screenshot_analysis_demo")]
#[command(about = "Analyze screenshots with Claude API and generate Suno.ai requests")]
struct Args {
    /// Path to the screenshot file
    screenshot_path: String,
    /// Path to user preferences file (optional)
    #[arg(short, long)]
    preferences: Option<String>,
}

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

#[derive(Serialize, Deserialize)]
struct SunoRequest {
    topic: Option<String>,
    tags: Option<String>,
    negative_tags: Option<String>,
    prompt: Option<String>,
    make_instrumental: Option<bool>,
    cover_clip_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SunoApiResponse {
    id: String,
    status: String,
    audio_url: Option<String>,
}


async fn analyze_screenshot_with_claude(
    client: &Client,
    api_key: &str,
    image_path: &str,
    user_preferences: &Option<UserPreferences>,
) -> Result<String> {
    // Read and encode the image
    let image_data = fs::read(image_path)
        .with_context(|| format!("Failed to read image: {}", image_path))?;
    let base64_data = base64::engine::general_purpose::STANDARD.encode(&image_data);

    // Try different Claude models in order of preference
    let models = [
        "claude-sonnet-4-20250514",
        "claude-3-5-haiku-latest",
        "claude-3-haiku-20240307",
    ];

    // Build preferences context
    let preferences_context = match user_preferences {
        Some(prefs) => {
            let mut context = String::new();
            context.push_str("\n\nPRIMARY FACTOR - USER PREFERENCES (equal weight with screenshot context):\n");
            
            // Musical preferences
            if let Some(genres) = &prefs.preferred_genres {
                context.push_str(&format!("User's preferred genres: {}\n", genres.join(", ")));
            }
            if let Some(instruments) = &prefs.preferred_instruments {
                context.push_str(&format!("User's preferred instruments: {}\n", instruments.join(", ")));
            }
            if let Some(artists) = &prefs.preferred_artists {
                context.push_str(&format!("User's preferred artists: {}\n", artists.join(", ")));
            }
            if let Some(energy) = &prefs.energy_level {
                context.push_str(&format!("User's preferred energy level: {}\n", energy));
            }
            if let Some(moods) = &prefs.mood_preferences {
                context.push_str(&format!("User's preferred moods: {}\n", moods.join(", ")));
            }
            
            // Suno.ai specific preferences
            if let Some(style) = &prefs.preferred_style {
                context.push_str(&format!("User's preferred style: {}\n", style));
            }
            if let Some(tempo) = &prefs.preferred_tempo {
                context.push_str(&format!("User's preferred tempo: {}\n", tempo));
            }
            if let Some(key) = &prefs.preferred_key {
                context.push_str(&format!("User's preferred key: {}\n", key));
            }
            if let Some(dynamics) = &prefs.preferred_dynamics {
                context.push_str(&format!("User's preferred dynamics: {}\n", dynamics));
            }
            if let Some(texture) = &prefs.preferred_texture {
                context.push_str(&format!("User's preferred texture: {}\n", texture));
            }
            
            // Advanced preferences
            if let Some(custom_mode) = &prefs.custom_mode {
                context.push_str(&format!("User prefers custom mode: {}\n", custom_mode));
            }
            if let Some(instrumental) = &prefs.make_instrumental {
                context.push_str(&format!("User prefers instrumental: {}\n", instrumental));
            }
            if let Some(cover_id) = &prefs.cover_clip_id {
                context.push_str(&format!("User wants to cover clip ID: {}\n", cover_id));
            }
            
            // Avoid preferences
            if let Some(avoid_genres) = &prefs.avoid_genres {
                context.push_str(&format!("User wants to avoid genres: {}\n", avoid_genres.join(", ")));
            }
            if let Some(avoid_instruments) = &prefs.avoid_instruments {
                context.push_str(&format!("User wants to avoid instruments: {}\n", avoid_instruments.join(", ")));
            }
            if let Some(avoid_style) = &prefs.avoid_style {
                context.push_str(&format!("User wants to avoid styles: {}\n", avoid_style.join(", ")));
            }
            if let Some(avoid_tempo) = &prefs.avoid_tempo {
                context.push_str(&format!("User wants to avoid tempos: {}\n", avoid_tempo.join(", ")));
            }
            if let Some(avoid_dynamics) = &prefs.avoid_dynamics {
                context.push_str(&format!("User wants to avoid dynamics: {}\n", avoid_dynamics.join(", ")));
            }
            
            // Work context preferences
            if let Some(work_prefs) = &prefs.work_context_preferences {
                context.push_str("User's work context preferences:\n");
                for (context_name, preference) in work_prefs {
                    context.push_str(&format!("  {}: {}\n", context_name, preference));
                }
            }
            
            // Context-specific overrides
            if let Some(overrides) = &prefs.context_overrides {
                context.push_str("User's context-specific overrides:\n");
                for (context_name, overrides_map) in overrides {
                    context.push_str(&format!("  {} context overrides:\n", context_name));
                    for (key, value) in overrides_map {
                        context.push_str(&format!("    {}: {}\n", key, value));
                    }
                }
            }
            
            context.push_str("\nRemember: Screenshot context AND user preferences are EQUAL PRIMARY factors. Use cognitive load analysis to refine the final prompt.");
            context
        }
        None => String::new(),
    };

    let prompt = format!("CRITICAL: Analyze this screenshot and user preferences as EQUAL PRIMARY factors, then use cognitive load analysis to fine-tune the music generation.

PRIMARY ANALYSIS (Equal Priority):
SCREENSHOT CONTEXT:
1. What application/website is the user actively using?
2. What specific task are they performing right now?
3. What is their current work state (focused, overwhelmed, creative, analytical)?
4. What type of cognitive load are they experiencing?

USER PREFERENCES:
5. What are the user's preferred genres, instruments, and artists?
6. What energy level and mood do they prefer?
7. What should be avoided based on their preferences?

COGNITIVE LOAD & CONTEXT REFINEMENT:
8. Based on the cognitive load analysis, how should the music be adjusted?
   - High cognitive load (complex tasks) → Simpler, less distracting music
   - Low cognitive load (routine tasks) → More engaging, dynamic music
   - Creative tasks → Inspiring, flowing music
   - Analytical tasks → Structured, minimal music
   - Overwhelmed state → Calming, grounding music
   - Focused state → Steady, supportive music

Generate a complete Suno.ai music request that balances screenshot context with user preferences, then refines based on cognitive load.

Please provide your response in this exact JSON format:
{{
  \"topic\": \"A detailed description of the music track (400-499 characters) that combines the screenshot work context with user preferences. Include key instruments, mood, tempo, and how it supports the user's current task.\",
  \"tags\": \"Musical style/genre tags that balance the work activity with user preferences (max 100 characters)\",
  \"negative_tags\": \"Styles or elements to avoid based on user preferences and work context (max 100 characters)\",
  \"prompt\": null (leave empty for instrumental tracks, or provide lyrics if you think they would be great for this context)
}}

BALANCE APPROACH:
- Screenshot context + User preferences = PRIMARY (equal weight)
- Cognitive load analysis = REFINEMENT (fine-tune the prompt)
- Create music that feels both contextually appropriate AND personally satisfying

The prompt should be detailed and comprehensive, utilizing the full 500 character limit in topic to create the perfect musical environment.{}Return ONLY the JSON, no other text.", preferences_context);

    for model in &models {
        println!("Trying model: {}", model);

        let request = AnthropicRequest {
            model: model.to_string(),
            max_tokens: 1000,
            messages: vec![Message {
                role: "user".to_string(),
                content: vec![
                    Content {
                        content_type: "text".to_string(),
                        text: Some(prompt.to_string()),
                        source: None,
                    },
                    Content {
                        content_type: "image".to_string(),
                        text: None,
                        source: Some(ImageSource {
                            source_type: "base64".to_string(),
                            media_type: "image/png".to_string(),
                            data: base64_data.clone(),
                        }),
                    },
                ],
            }],
        };

        match call_anthropic_api(client, api_key, &request).await {
            Ok(response) => {
                println!("✅ Success with {}", model);
                return Ok(response);
            }
            Err(e) => {
                println!("❌ Failed with {}: {}", model, e);
                continue;
            }
        }
    }

    anyhow::bail!("All model attempts failed")
}

async fn call_anthropic_api(
    client: &Client,
    api_key: &str,
    request: &AnthropicRequest,
) -> Result<String> {
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(request)
        .send()
        .await
        .context("Failed to send request to Anthropic API")?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "API request failed with status {}: {}",
            status,
            error_text
        );
    }

    let response_data: AnthropicResponse = response
        .json()
        .await
        .context("Failed to parse Anthropic API response")?;

    if let Some(content) = response_data.content.first() {
        Ok(content.text.clone())
    } else {
        anyhow::bail!("No content in Anthropic API response")
    }
}

#[derive(Deserialize)]
struct ClaudeResponse {
    topic: String,
    tags: String,
    negative_tags: String,
    prompt: Option<String>,
}

#[derive(Deserialize)]
struct UserPreferences {
    // Musical preferences
    preferred_genres: Option<Vec<String>>,
    preferred_instruments: Option<Vec<String>>,
    preferred_artists: Option<Vec<String>>,
    energy_level: Option<String>, // "low", "medium", "high"
    mood_preferences: Option<Vec<String>>,
    avoid_genres: Option<Vec<String>>,
    avoid_instruments: Option<Vec<String>>,
    
    // Work context preferences
    work_context_preferences: Option<std::collections::HashMap<String, String>>,
    
    // Suno.ai specific preferences
    preferred_style: Option<String>,
    preferred_tempo: Option<String>, // "slow", "medium", "fast"
    preferred_key: Option<String>, // "major", "minor", "modal"
    preferred_dynamics: Option<String>, // "soft", "medium", "loud"
    preferred_texture: Option<String>, // "sparse", "dense", "layered"
    
    // Advanced preferences
    custom_mode: Option<bool>,
    make_instrumental: Option<bool>,
    cover_clip_id: Option<String>,
    
    // Negative preferences (what to avoid)
    avoid_style: Option<Vec<String>>,
    avoid_tempo: Option<Vec<String>>,
    avoid_dynamics: Option<Vec<String>>,
    
    // Context-specific overrides
    context_overrides: Option<std::collections::HashMap<String, std::collections::HashMap<String, String>>>,
}

fn load_user_preferences(preferences_path: Option<&String>) -> Result<Option<UserPreferences>> {
    match preferences_path {
        Some(path) => {
            if Path::new(path).exists() {
                let content = fs::read_to_string(path)
                    .with_context(|| format!("Failed to read preferences file: {}", path))?;
                
                if content.trim().is_empty() {
                    println!("📝 User preferences file is empty, proceeding without preferences");
                    Ok(None)
                } else {
                    let prefs: UserPreferences = serde_json::from_str(&content)
                        .with_context(|| format!("Failed to parse preferences file: {}", path))?;
                    println!("📝 Loaded user preferences from: {}", path);
                    Ok(Some(prefs))
                }
            } else {
                println!("📝 Preferences file not found: {}, proceeding without preferences", path);
                Ok(None)
            }
        }
        None => {
            println!("📝 No preferences file specified, proceeding without preferences");
            Ok(None)
        }
    }
}

fn generate_suno_request(context: &str, user_preferences: &Option<UserPreferences>) -> Result<SunoRequest> {
    // Extract JSON from Claude's response (it might have extra text)
    let json_start = context.find('{').unwrap_or(0);
    let json_end = context.rfind('}').unwrap_or(context.len() - 1) + 1;
    let json_str = &context[json_start..json_end];
    
    // Parse Claude's JSON response
    let claude_data: ClaudeResponse = serde_json::from_str(json_str)
        .context("Failed to parse Claude's JSON response")?;

    // Ensure constraints are met - keep topic between 400-499 characters
    let topic_trimmed = if claude_data.topic.len() < 400 {
        // If too short, add more detail
        claude_data.topic + ". This composition is designed to enhance focus and productivity while maintaining a soothing atmosphere that complements the user's current workflow and cognitive state."
    } else if claude_data.topic.len() > 499 {
        // If too long, trim to fit within 400-499 range
        let truncated = claude_data.topic.chars().take(450).collect::<String>();
        if let Some(last_period) = truncated.rfind('.') {
            truncated.chars().take(last_period + 1).collect()
        } else {
            truncated + "..."
        }
    } else {
        claude_data.topic
    };

    let tags_trimmed = if claude_data.tags.len() > 100 {
        claude_data.tags.chars().take(97).collect::<String>() + "..."
    } else {
        claude_data.tags
    };

    let negative_tags_trimmed = if claude_data.negative_tags.len() > 100 {
        claude_data.negative_tags.chars().take(97).collect::<String>() + "..."
    } else {
        claude_data.negative_tags
    };

    // Use user preferences for Suno.ai request fields
    let make_instrumental = user_preferences
        .as_ref()
        .and_then(|prefs| prefs.make_instrumental)
        .unwrap_or(true); // Default to instrumental for productivity

    Ok(SunoRequest {
        topic: Some(topic_trimmed),
        tags: Some(tags_trimmed),
        negative_tags: Some(negative_tags_trimmed),
        prompt: claude_data.prompt,
        make_instrumental: Some(make_instrumental),
        cover_clip_id: None, // Never use cover_clip_id
    })
}

async fn call_suno_api(client: &Client, suno_token: &str, request: &SunoRequest) -> Result<SunoApiResponse> {
    let response = client
        .post("https://studio-api.prod.suno.com/api/v2/external/hackmit/generate")
        .header("Authorization", format!("Bearer {}", suno_token))
        .header("Content-Type", "application/json")
        .json(request)
        .send()
        .await
        .context("Failed to send request to Suno.ai API")?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "Suno.ai API request failed with status {}: {}",
            status,
            error_text
        );
    }

    let response_data: SunoApiResponse = response
        .json()
        .await
        .context("Failed to parse Suno.ai API response")?;

    Ok(response_data)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file
    dotenv::dotenv().ok();
    
    let args = Args::parse();

    // Check if file exists
    if !Path::new(&args.screenshot_path).exists() {
        anyhow::bail!("Screenshot file '{}' not found", args.screenshot_path);
    }

    // Get API keys from environment (now loaded from .env)
    let api_key = env::var("ANTHROPIC_API_KEY")
        .context("ANTHROPIC_API_KEY not found in environment or .env file. Please set your Anthropic API key in .env file")?;
    
    let suno_token = env::var("SUNO_HACKMIT_TOKEN")
        .context("SUNO_HACKMIT_TOKEN not found in environment or .env file. Please set your Suno.ai HackMIT token in .env file")?;

    let client = Client::new();

    // Load user preferences
    let user_preferences = load_user_preferences(args.preferences.as_ref())?;

    println!("🔍 Analyzing screenshot: {}", args.screenshot_path);
    println!("{}", "=".repeat(50));

    // Analyze the screenshot
    let context = analyze_screenshot_with_claude(&client, &api_key, &args.screenshot_path, &user_preferences).await?;

    println!("\n📋 Analysis Result:");
    println!("{}", "-".repeat(30));
    println!("{}", context);

    println!("\n🎵 Generating Suno.ai Request...");
    println!("{}", "-".repeat(30));

    // Generate Suno request
    let suno_request = generate_suno_request(&context, &user_preferences)?;

    println!("\n✅ Suno.ai Request Generated:");
    println!("{}", "=".repeat(50));
    println!("{}", serde_json::to_string_pretty(&suno_request)?);


    // Save to file
    fs::write("suno_request.json", serde_json::to_string_pretty(&suno_request)?)
        .with_context(|| "Failed to write suno_request.json")?;

    Ok(())
}
