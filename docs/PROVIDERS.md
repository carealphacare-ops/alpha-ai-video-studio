# Provider research (snapshot: 11 Nov 2026)

**How reliable is this?** Most numbers below come from third-party pricing
articles that say they read the official pages in Aug–Oct 2026. I could not
open every official page directly. Prices and model names change often.
**Before you spend money, check each official pricing page.** The app will store
prices in an editable table (`provider_models.price_json`) and always label them
"estimate" until the provider reports the real charge.

## Video generation

| Provider | Official API? | Price seen (per second of video) | Commercial use | Main limits / notes |
|---|---|---|---|---|
| Google (Gemini API / Vertex) | Yes (Gemini API, Vertex AI) | Veo 3.1 Fast ~$0.10–0.12 (720p/1080p); Veo 3.1 standard ~$0.40; a newer model, Gemini Omni Flash, is listed at about $0.10 (720p) and is the one Google recommends as default | Paid tier only; no free tier for Veo | 4–8 s clips, concurrency caps, regional availability varies, failed/blocked generations are not billed |
| Runway | Yes (dev.runwayml.com) — **verified on the official pricing page, 12 Nov 2026** | $0.01 per credit. Gen-4 Turbo 5 credits/s ($0.05); Gen-4.5 12/s ($0.12). Through the same API Runway also sells **Veo 3.1** (20/s silent, 40/s audio; Fast 10/15), **Kling 3.0** (9–17/s), **Seedance 2** (14–150/s by resolution) and Seedance 2.5, Wan 3, etc. Images: gpt_image_2, nano_banana, seedream etc. (1–41 credits/image). Separate wallet from app plans | Check Runway terms | Async task + polling. Page does not state max duration per model: read it from the API at runtime. Runway marks up the underlying model, so direct APIs may be cheaper |

**Recommended first video adapter: Runway.** One key and one integration reach
many models, so it is the fastest route to a real, tested video feature. Direct
Google/Kling adapters can follow when you want lower prices.
| Kling | Yes (Kling developer platform) | Roughly $0.08–0.10/s for 720p silent, rising with 1080p/audio/4K. Prepaid "resource packages" from about $9.80, packages expire (30 or 180 days) | Reported yes; verify terms | API credits are separate from web plans |
| Seedance (ByteDance) | Partly | BytePlus lists Seedance 2.5 token pricing, but launch notes said API access was "coming soon". Seedance 2.0 reachable mostly through resellers | Verify | **Do not rely on it yet.** Treat as "planned" until a real test call succeeds. The consumer Dreamina app has no API |

## Voice

| Provider | Official API? | Price / plan | Commercial use | Notes |
|---|---|---|---|---|
| ElevenLabs | Yes | Free plan 10k credits/month. Paid from about $5–6/month | **Free plan: no commercial use, attribution required.** Paid plans allow it | Hindi supported. Urdu support must be tested before promising it |
| Google Cloud TTS | Yes | Verify on Google pricing page | Verify | Has Urdu and Hindi voices; good low-cost default candidate |
| Local TTS (e.g. Piper) | Local software | Free | Check each voice's licence | Quality lower; Urdu coverage must be tested |

## Language models, images, transcription

| Provider | Role | Status |
|---|---|---|
| Claude (Anthropic API) | Script, scenes, highlight scoring | Official API. Verify pricing at the Anthropic docs |
| OpenAI | LLM + image generation | Official API. Verify pricing |
| Google Gemini | LLM + image | Official API. Verify pricing |
| Whisper / faster-whisper | Transcription | Open-source, runs locally, CPU or CUDA GPU. Free. Word timestamps supported |

## YouTube links (important)

- The official YouTube API gives **metadata**, not the video file.
- The YouTube API terms say API clients must not be designed to cache YouTube
  audiovisual content. YouTube's Terms of Service also forbid downloading
  videos outside its own features, unless the owner allows it.
- **Decision:** the app will *not* download from YouTube links. Pasting a link
  will fetch metadata and then ask the user to import the file they own or have
  permission to use (their own channel export, a licensed file, etc.).
  This is a legal and platform-rules decision, not a technical limit.

## Windows installer facts (Tauri 2)

- NSIS `-setup.exe` can be built on Windows, Linux or macOS. MSI needs Windows + WiX.
- A Windows machine (or Windows CI runner) is the reliable way to build the
  installer. Cross-compiling from Linux exists (`cargo-xwin`) but is fragile.
- FFmpeg is bundled as a Tauri "sidecar" (`externalBin`), pinned to one version.
- Known issue: sidecar paths resolve relative to `src-tauri/`, and same-version
  reinstalls can keep an old sidecar. Bump the app version on every release.
