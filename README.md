# Alienor

A video synchronization tool for animation reference in Roblox Studio. Syncs video playback across multiple instances and supports Moon Animator.

## Status

- **Windows:** Supported
- **macOS:** Planned
- **Linux:** Planned (no port yet)

## Features

- **Video Sync** across instances
- **MPV-based playback** for performance
- **YouTube support** (requires yt-dlp)

## Installation (Windows)

1. Open the latest release on GitHub and download the installer (`.exe` or `.msi`).
2. Run the installer.

Note: Windows Defender may flag it as a false positive. It is safe; a review was submitted.

## Usage

1. Open Moon Animator in Roblox Studio
2. Launch Alienor
3. Enable the Alienor plugin from the Plugins tab
4. Enter port 3000 (or your configured port) in the plugin. Use the port number provided in the app window to connect to the Roblox Plugin. The port number is displayed in the window.
5. Click "Connect to Alienor" in the plugin
6. Load your video file by dragging it into the player window

## Troubleshooting

1. **Video won't play**
   - Try restarting Alienor
   - Verify the video file format is supported

2. **YouTube videos don't work**
   - Install yt-dlp (`winget install yt-dlp`)
   - Check your internet connection

## Technical Details

- Tauri (Rust backend)
- MPV for video playback
- Axum web server for sync

## License

[MIT License](LICENSE)