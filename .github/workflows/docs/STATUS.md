# Status (12 Nov 2026)

## Tested here (Linux container)
- `cargo test -p alpha-core`: 13 tests pass (database, projects, settings, media probe/trim, jobs, usage/budget).
- `npm run build`: TypeScript check and Vite build pass.

## NOT tested yet
- The Tauri window itself (`src-tauri`) has **not been compiled or run**. This container has no GTK/WebKit and no Windows. The first Windows CI run will be the first real compile. Expect a few fixes.
- The Windows installer has not been built. Run the "Windows installer" GitHub Action (see BUILD_WINDOWS.md).

## Genuinely built (code exists, unit-tested in core)
Projects (create/rename/duplicate/delete), settings, spending limits, SQLite schema, media probe, trim/export via FFmpeg, job rows, usage log with estimate vs actual.

## Not built yet
Everything AI: transcription, highlights, reframing, animated subtitles, story studio, characters, voice, provider calls and connection tests, timeline editor, batch pause/retry, social presets, assistant. Each page in the app says "Planned" or "Partial" honestly.

## Next steps
1. Run Windows CI, fix compile errors, install and click through the app.
2. Phase 3: Python faster-whisper sidecar -> word timestamps -> ASS captions.
3. Runway adapter with a real test-connection call.
