# ContextFM - AI-Powered Context-Aware Music Generator

A desktop application that automatically generates and plays music based on your current screen content and activity. Built with Tauri (Rust + TypeScript), it uses AI to analyze screenshots and create contextual music through Suno's API.

## 🎵 Features

- **Real-time Screen Analysis**: Captures screenshots every 5 seconds and analyzes them using Claude AI
- **Context-Aware Music Generation**: Generates music that matches your current activity (coding, browsing, etc.)
- **Intelligent Switching**: Automatically switches music when screen content changes by more than 10%
- **Customizable Preferences**: Control genres, vocals, instrumental mode, and other music parameters
- **Seamless Playback**: Continuous music streaming with fade transitions between tracks
- **Cross-Platform**: Works on macOS, Windows, and Linux

## 🛠️ Technology Stack

- **Frontend**: TypeScript, Vite, HTML/CSS
- **Backend**: Rust with Tauri framework
- **AI Integration**: 
  - Claude (Anthropic) for screen analysis
  - Suno API for music generation
- **Image Processing**: Screenshot capture, hash-based change detection
- **Audio**: HTML5 Audio with seamless streaming

## 📋 Prerequisites

- Node.js (v16 or higher)
- Rust (latest stable)
- API Keys:
  - Anthropic API key for Claude
  - Suno API key for music generation

## 🚀 Installation

1. **Clone the repository**
   ```bash
   git clone <repository-url>
   cd HackMIT
   ```

2. **Install dependencies**
   ```bash
   npm install
   ```

3. **Set up environment variables**
   
   Create a `.env` file in the project root:
   ```env
   ANTHROPIC_API_KEY=your_anthropic_api_key_here
   SUNO_API_KEY=your_suno_api_key_here
   ```

4. **Build and run**
   ```bash
   # Development mode
   npm run tauri dev
   
   # Build for production
   npm run tauri build
   ```

## 🎛️ Usage

### Basic Operation

1. **Launch the application** - The app will start capturing screenshots automatically
2. **Configure preferences** - Use the UI controls to set:
   - Music genres (checkboxes)
   - Vocal preferences (Male/Female)
   - Instrumental mode (On/Off)
   - Volume levels
3. **Generate music** - Click "Generate" to create your first track
4. **Automatic switching** - The app will automatically generate new music when your screen content changes significantly

### UI Controls

- **Generate Button**: Manually trigger music generation
- **Genre Selection**: Choose from various music genres
- **Vocal Settings**: Toggle between male/female vocals or instrumental
- **Volume Sliders**: Control different audio levels
- **Playback Controls**: Play/pause, skip forward, go back in history

### Advanced Features

- **Context Display**: Shows current activity analysis and music tags
- **History Navigation**: Use back button to replay previous tracks
- **Rate Limiting**: Prevents excessive music switching (3-second cooldown)
- **Prefetching**: Automatically generates next track for seamless playback

## 🔧 Configuration

### Music Generation Settings

Edit `suno-config/suno_request.json` to customize default music parameters:

```json
{
  "topic": "Your custom music description",
  "tags": "genre1, genre2, mood",
  "make_instrumental": false
}
```

### Change Detection Sensitivity

The app uses image hashing to detect screen changes. The current threshold is set to 10% of maximum possible change. This can be adjusted in `src-tauri/src/screenshot.rs`:

```rust
const CHANGE_THRESHOLD_PERCENT: f32 = 0.10; // 10%
```

## 📁 Project Structure

```
HackMIT/
├── src/                    # Frontend TypeScript code
│   ├── main.ts            # Main application logic
│   ├── styles.css         # UI styling
│   └── assets/            # Static assets
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── lib.rs         # Main Tauri setup
│   │   ├── screenshot.rs  # Screen capture & analysis
│   │   ├── claude.rs      # Claude AI integration
│   │   └── suno.rs        # Suno API integration
│   └── Cargo.toml         # Rust dependencies
├── suno-config/           # Music generation config
│   ├── suno_request.json  # Default music parameters
│   └── recent_genres.json # Genre history
├── temp/                  # Temporary files
│   ├── current.png        # Latest screenshot
│   └── prev.png          # Previous screenshot
└── dist/                  # Built frontend
```

## 🔍 How It Works

1. **Screenshot Capture**: Every 5 seconds, captures the active display
2. **Image Hashing**: Computes perceptual hash of the screenshot
3. **Change Detection**: Compares current hash with previous (10% threshold)
4. **AI Analysis**: If significant change detected, sends screenshot to Claude
5. **Music Generation**: Claude analyzes context and generates Suno request
6. **Audio Streaming**: Suno generates music and streams it to the app
7. **Seamless Playback**: Fades between tracks for continuous experience

## 🐛 Troubleshooting

### Common Issues

1. **API Key Errors**
   - Ensure `.env` file exists with correct API keys
   - Check that keys have sufficient credits/permissions

2. **Screenshot Permissions**
   - On macOS: Grant screen recording permissions
   - On Linux: May need additional permissions for screen capture

3. **Audio Issues**
   - Check system audio settings
   - Ensure no other applications are blocking audio

4. **Build Errors**
   - Update Rust: `rustup update`
   - Clear cache: `cargo clean`
   - Reinstall dependencies: `npm install`

### Debug Mode

Enable detailed logging by running in development mode:
```bash
npm run tauri dev
```

Check console output for detailed information about:
- Hash distances and change detection
- API calls and responses
- Music generation status

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test thoroughly
5. Submit a pull request

## 📄 License

This project was created for HackMIT 2024. Please check with the organizers for specific licensing terms.

## 🙏 Acknowledgments

- **Suno**: For providing the music generation API
- **Anthropic**: For Claude AI integration
- **Tauri**: For the excellent desktop app framework
- **HackMIT**: For the hackathon platform and inspiration

## 📞 Support

For issues and questions:
1. Check the troubleshooting section above
2. Review the console logs for error messages
3. Ensure all API keys are valid and have sufficient credits
4. Create an issue in the repository with detailed information

---

**Note**: This application requires internet connectivity for AI analysis and music generation. Ensure stable internet connection for optimal performance.