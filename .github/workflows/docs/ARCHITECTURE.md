# ALPHA AI VIDEO STUDIO — Architecture

## 1. Big decisions (and why)

| Decision | Why |
|---|---|
| Tauri 2 + React/TypeScript UI | Small installer, native Windows window, no browser needed |
| All real logic in a plain Rust crate (`alpha-core`) | It can be tested without a window; Tauri only forwards calls |
| SQLite (bundled) in the user's project folder | One file, easy backup, works offline |
| FFmpeg as a pinned sidecar, called with argument lists (never a shell string) | Reproducible, and safe from command injection |
| Python sidecar (later phases) for Whisper, scene detection, face tracking | Best ecosystem for these; packaged with PyInstaller so users install nothing |
| Provider adapters behind one trait/interface | Add or replace a provider without touching the UI |
| Estimated cost is always shown *before* a cloud call; actual cost saved separately | The app must never spend money by surprise |
| No YouTube downloading | See `PROVIDERS.md`: platform rules. Link → metadata only, then import a file |

## 2. Layers

```
React UI (src/)
   │  invoke()
Tauri shell (src-tauri/)         thin: commands + app state
   │
alpha-core (crates/alpha-core)   projects, settings, media probe, jobs, usage/cost, DB
   │                    │
SQLite file         ffprobe / ffmpeg sidecars (Phase 2+)
                        │
Python sidecar (Phase 3+): faster-whisper, PySceneDetect, OpenCV
                        │
Provider adapters (Phase 4/7): LLM, image, video, voice — HTTPS, keys in Windows Credential Manager
```

## 3. Folder structure

```
alpha-ai-video-studio/
  docs/                 PROVIDERS.md, ARCHITECTURE.md, STATUS.md, BUILD_WINDOWS.md
  src/                  React UI (pages/, components/, lib/)
  crates/alpha-core/    Rust logic + tests (src/db.rs, projects.rs, settings.rs, media.rs, jobs.rs, usage.rs)
  src-tauri/            Tauri 2 shell (commands, tauri.conf.json)
  sidecars/             (later) python/ and pinned ffmpeg builds
```

## 4. Database schema (v1, implemented)

`projects`, `media_assets`, `clips`, `scenes`, `characters`, `jobs`,
`providers`, `usage_log`, `settings`, `subtitle_presets`, `tags`, `project_tags`.
Migrations use `PRAGMA user_version`. See `crates/alpha-core/src/db.rs`.

## 5. Provider adapter design (Phase 4 / 7)

One interface per capability: `LlmProvider`, `ImageProvider`, `VideoProvider`,
`VoiceProvider`, `TranscriptionProvider`. Each adapter exposes:
`capabilities()`, `estimate_cost(request)`, `submit(request)`, `poll(task_id)`,
`cancel(task_id)`. Video providers are asynchronous (submit → poll), so every
cloud call is a **job** in the queue. Model lists and prices live in the DB, so a
price change needs no new release. Adapters that are not tested against the real
API are shown as **Planned**, never as connected.

## 6. Video pipeline (Phase 2–3)

Import → `ffprobe` → proxy/preview file → (Python) transcribe with word times →
silence + scene detection → LLM scores candidate segments → user picks →
per clip: cut, reframe (face tracking), caption (ASS file), loudness normalise,
render with FFmpeg → export. Each step is a job row with real progress parsed
from FFmpeg's `-progress` output.

## 7. Security

- API keys: Windows Credential Manager (Rust `keyring` crate), never in SQLite, logs or exports.
- Subprocesses: argument arrays only; paths validated; no shell.
- Imports: extension + `ffprobe` check; user confirms they have rights to the content.
- AI assistant (Phase 8+): can only call a fixed list of app actions; text from videos/files is data, never commands.
- Tauri capabilities: only the commands and file scopes the app needs.

## 8. Windows build

Build on a Windows machine or GitHub Actions `windows-latest`:
`npm ci && npm run tauri build` → `src-tauri/target/release/bundle/nsis/*-setup.exe`.
WebView2 bootstrapper is included by Tauri's installer. Code signing is optional
but removes SmartScreen warnings. See `BUILD_WINDOWS.md`.

## 9. Testing

- Rust unit tests for every core module (run on any OS).
- Frontend type-check + build in CI.
- Manual Windows 10/11 checklist before each release.
- Provider adapters: a "test connection" call against the real API before the adapter is marked **Available**.
