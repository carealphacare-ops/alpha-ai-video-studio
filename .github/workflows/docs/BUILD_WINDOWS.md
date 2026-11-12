# Building the Windows installer

## Easiest: GitHub Actions (no setup on your PC)
1. Create a GitHub repo and push this folder.
2. Open the **Actions** tab -> **Windows installer** -> **Run workflow**.
3. When it finishes, download the artifact `alpha-ai-video-studio-windows`. It holds the `-setup.exe` (and `.msi`).
4. Double-click the setup file. Windows may show SmartScreen because the app is not code-signed: choose "More info" -> "Run anyway".

## On your own Windows PC
Install Node 22, Rust (rustup), and Visual Studio Build Tools ("Desktop development with C++").
```
npm ci
pwsh ./scripts/fetch-ffmpeg.ps1
npm run tauri build
```
Installer appears under `target/release/bundle/nsis/`.

## Development mode
`npm run tauri dev` (needs FFmpeg on PATH, or set its folder in Settings).
