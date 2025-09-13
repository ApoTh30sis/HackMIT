# Screenshot Analysis & Suno.ai Request Generator (Rust)

A high-performance Rust application that analyzes screenshots using Claude 3.5 Haiku and generates Suno.ai requests for productivity-boosting music.

## Features

- **Fast Screenshot Analysis**: Uses Claude 3.5 Haiku for efficient image understanding
- **Smart Model Fallback**: Tries Haiku → Sonnet → Legacy Haiku for best compatibility
- **Suno.ai API Integration**: Automatically calls Suno.ai API to generate music
- **Real-time Music Generation**: Submits requests and tracks generation status
- **High Performance**: Built in Rust for speed and reliability
- **Command Line Interface**: Easy to use with clear output and error handling

## Installation

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Clone and build**:
   ```bash
   cd screenshot_analysis_demo
   cargo build --release
   ```

## Setup

Set up API keys in .env file:
```bash
# Create .env file with your API keys
echo "ANTHROPIC_API_KEY=your-anthropic-api-key-here" > .env
echo "SUNO_HACKMIT_TOKEN=your-suno-hackmit-token-here" >> .env
```

## Usage

```bash
# Run the demo
cargo run -- tmp/screenshot1.png

# Or run the compiled binary
./target/release/screenshot_analysis_demo tmp/screenshot1.png
```

## Example Output

```
🔍 Analyzing screenshot: tmp/screenshot1.png
==================================================
Trying model: claude-3-5-haiku-latest
✅ Success with claude-3-5-haiku-latest

📋 Analysis Result:
------------------------------
The user is working in VS Code, writing Python code. They have multiple files open 
and appear to be debugging or developing a web application. The interface shows 
a clean, focused coding environment with syntax highlighting visible. This suggests 
they need calm, ambient music that won't distract from complex problem-solving.

🎵 Generating Suno.ai Request...
------------------------------

✅ Suno.ai Request Generated:
==================================================
{
  "prompt": "Create an instrumental background music track that enhances productivity for: The user is working in VS Code, writing Python code...",
  "tags": "instrumental, ambient, productivity, focus, background music, lofi, chill",
  "title": "Productivity Boost Track",
  "make_instrumental": true,
  "wait_audio": true,
  "style": "ambient",
  "mood": "focused",
  "duration": 180
}

📝 Usage Instructions:
------------------------------
1. Copy the JSON above
2. Use it with Suno.ai API to generate the music
3. The generated music will be optimized for your current task!

💾 Request saved to: suno_request_screenshot1.json
```

## How It Works

1. **Image Processing**: Reads and base64-encodes the screenshot
2. **Claude Analysis**: Uses vision capabilities to understand work context
3. **Model Fallback**: Tries multiple models for best compatibility
4. **Context Parsing**: Extracts relevant information about user activity
5. **Music Generation**: Creates tailored Suno.ai request for optimal productivity

## Model Priority

The application tries models in this order for best cost/speed/vision balance:
1. `claude-3-5-haiku-latest` (fastest, cheapest)
2. `claude-3-5-sonnet-latest` (more capable, pricier)
3. `claude-3-haiku-20240307` (legacy fallback)

## Dependencies

- `reqwest`: HTTP client for API calls
- `tokio`: Async runtime
- `serde`: Serialization/deserialization
- `clap`: Command line argument parsing
- `anyhow`: Error handling
- `base64`: Image encoding

## Performance

- **Fast**: Rust's performance ensures quick image processing
- **Memory Efficient**: Minimal memory footprint
- **Reliable**: Strong error handling and type safety
- **Cross Platform**: Works on Windows, macOS, and Linux

## Use Cases

- **Coding**: Generate focus music for programming sessions
- **Writing**: Create ambient tracks for content creation
- **Design**: Get music optimized for creative work
- **Research**: Generate background music for deep work
- **Any Task**: Analyze any screenshot and get productivity-boosting music

## Integration

The generated JSON can be used directly with Suno.ai's API to create custom productivity music tailored to your specific work context.
