# Status (13 Nov 2026)

## Verified on a real Windows PC (by you)
- Installer builds on GitHub Actions and installs.
- App opens, Settings and Clipping pages load, local video import, preview and trim-export to MP4 work.

## Tested in the Linux container (`cargo test -p alpha-core`, 23 tests)
- Database, projects, settings, jobs, spending limits.
- Transcript parsing, caption grouping, SRT/VTT/ASS output, ASS colours, FFmpeg filter path escaping.
- Full transcribe pipeline with real FFmpeg and a stand-in Whisper script: audio extraction, progress, result file, burn-in render, and refusal of a range with no speech.

## NOT verified yet
- The **real** whisper-cli output format and flags (`-oj -ml 1 -sow -pp`) were written from documentation, never run against the real program. First real transcription on Windows is the test. If word timing looks wrong, the parser already splits multi-word entries by length as a fallback.
- Tauri shell commands for transcription/captions compiled only after the Windows CI run; they have not been clicked through yet.
- Urdu/Hindi accuracy with the `base` model. Expect mistakes; the transcript editor exists to fix them.

## Fixed after first Windows test (17 Nov)
- Burned-in captions failed: FFmpeg's filter parser split the Windows path at `C:`. Now FFmpeg runs inside the work folder and gets only `captions.ass`.
- Urdu words came out in pieces: whisper pieces without a leading space are now joined to the previous word.
- Repeat loops ("word word word...") are cut to at most 3 in a row; whisper is run with `-mc 0` to reduce them.
- Better models can be downloaded in the app (Small, Large v3 Turbo q5_0) via curl.exe. Not yet tested against the real download or real Urdu audio.

## Not built yet
Highlight detection and scoring, multi-clip suggestions, vertical reframing and face tracking, silence removal, Story Studio, characters, voice, provider calls, timeline editor, social presets, assistant.

## Next steps
1. Run Windows CI, install, transcribe a short video, burn captions, report problems.
2. Highlight detection: LLM adapter (Claude/OpenAI/Gemini key) that scores transcript windows.
3. Vertical 9:16 reframing (centre crop first, face tracking later).
