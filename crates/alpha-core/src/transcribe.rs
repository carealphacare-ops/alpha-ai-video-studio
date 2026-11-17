//! Local transcription with whisper.cpp (`whisper-cli`) and caption rendering with FFmpeg.
//! Processes are started with argument lists only (never through a shell).

use crate::captions::{self, CaptionStyle, Transcript};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn command(path: &Path) -> Command {
    let mut c = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

pub struct WhisperConfig {
    pub ffmpeg: PathBuf,
    pub whisper: PathBuf,
    pub model: PathBuf,
    /// "auto" or a code such as "en", "ur", "hi".
    pub language: String,
    pub work_dir: PathBuf,
}

/// Reads "progress =  42%" from whisper.cpp's stderr.
pub fn parse_whisper_progress(line: &str) -> Option<f64> {
    let rest = line.split("progress =").nth(1)?;
    let num: String = rest.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse::<f64>().ok().map(|p| (p / 100.0).clamp(0.0, 1.0))
}

fn valid_language(l: &str) -> bool {
    l == "auto" || (l.len() >= 2 && l.len() <= 3 && l.chars().all(|c| c.is_ascii_lowercase()))
}

/// Extracts mono 16 kHz audio, runs whisper, parses word times, saves `transcript.json` in `work_dir`.
/// `on_progress` gets real values: 0-5 % for audio extraction, the rest from whisper.
pub fn transcribe(
    cfg: &WhisperConfig,
    input: &Path,
    cancel: &dyn Fn() -> bool,
    on_progress: &mut dyn FnMut(f64),
) -> Result<Transcript, String> {
    if !valid_language(&cfg.language) {
        return Err("Invalid language code.".into());
    }
    if !cfg.model.is_file() {
        return Err(format!("Whisper model not found: {}. Reinstall the app or set a model file.", cfg.model.display()));
    }
    std::fs::create_dir_all(&cfg.work_dir).map_err(|e| format!("Cannot create work folder: {e}"))?;
    let wav = cfg.work_dir.join("audio.wav");

    let out = command(&cfg.ffmpeg)
        .args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(input)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"])
        .arg(&wav)
        .output()
        .map_err(|e| format!("Could not start ffmpeg ({e})."))?;
    if !out.status.success() {
        return Err(format!("Could not read audio: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    on_progress(0.05);
    if cancel() {
        return Err("Cancelled".into());
    }

    let prefix = cfg.work_dir.join("whisper_out");
    let json_path = cfg.work_dir.join("whisper_out.json");
    let _ = std::fs::remove_file(&json_path);
    let mut child = command(&cfg.whisper)
        .arg("-m")
        .arg(&cfg.model)
        .arg("-f")
        .arg(&wav)
        .args(["-l", &cfg.language, "-oj", "-ml", "1", "-sow", "-mc", "0", "-pp", "-of"])
        .arg(&prefix)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start whisper-cli ({e}). The transcription engine is missing from this install."))?;

    let mut tail: Vec<String> = Vec::new();
    let stderr = child.stderr.take().ok_or("no whisper output")?;
    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
        if cancel() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cancelled".into());
        }
        if let Some(p) = parse_whisper_progress(&line) {
            on_progress(0.05 + 0.95 * p);
        } else {
            tail.push(line);
            if tail.len() > 15 {
                tail.remove(0);
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("Transcription failed: {}", tail.join(" | ")));
    }
    let json = std::fs::read_to_string(&json_path).map_err(|e| format!("Whisper produced no result file ({e})."))?;
    let t = captions::parse_whisper_json(&json)?;
    let saved = serde_json::to_string_pretty(&t).map_err(|e| e.to_string())?;
    std::fs::write(cfg.work_dir.join("transcript.json"), saved).map_err(|e| format!("Cannot save transcript: {e}"))?;
    let _ = std::fs::remove_file(&wav);
    on_progress(1.0);
    Ok(t)
}

// ---------- model catalogue and download ----------

pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    /// Approximate size, used only to show download progress.
    pub approx_bytes: u64,
    pub bundled: bool,
}

pub const MODELS: &[ModelInfo] = &[
    ModelInfo { id: "base", label: "Base: bundled, fast, weak for Urdu", file: "ggml-base.bin", url: "", approx_bytes: 148_000_000, bundled: true },
    ModelInfo {
        id: "small",
        label: "Small: better accuracy (about 470 MB download)",
        file: "ggml-small.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        approx_bytes: 488_000_000,
        bundled: false,
    },
    ModelInfo {
        id: "turbo",
        label: "Large v3 Turbo, compressed: best accuracy, slower (about 570 MB download)",
        file: "ggml-large-v3-turbo-q5_0.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        approx_bytes: 574_000_000,
        bundled: false,
    },
];

pub fn model_info(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

/// Downloads `url` to `dest` with curl (included in Windows 10/11). Progress = bytes on disk / `expected`.
/// The file is written as `dest.part` and renamed only when curl succeeds and at least `min_bytes` arrived.
pub fn download_file(
    curl: &Path,
    url: &str,
    dest: &Path,
    expected: u64,
    min_bytes: u64,
    cancel: &dyn Fn() -> bool,
    on_progress: &mut dyn FnMut(f64),
) -> Result<(), String> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create folder: {e}"))?;
    }
    let part = PathBuf::from(format!("{}.part", dest.display()));
    let _ = std::fs::remove_file(&part);
    let mut child = command(curl)
        .args(["-L", "--fail", "--silent", "--show-error", "--retry", "3", "-o"])
        .arg(&part)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start curl ({e}). Windows 10/11 include curl.exe; check that it is not removed."))?;
    loop {
        if cancel() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&part);
            return Err("Cancelled".into());
        }
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => {
                if !status.success() {
                    let mut msg = String::new();
                    if let Some(mut e) = child.stderr.take() {
                        use std::io::Read;
                        let _ = e.read_to_string(&mut msg);
                    }
                    let _ = std::fs::remove_file(&part);
                    return Err(format!("Download failed: {}", msg.trim()));
                }
                break;
            }
            None => {
                let got = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
                on_progress((got as f64 / expected.max(1) as f64).clamp(0.0, 0.99));
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        }
    }
    let got = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    if got < min_bytes {
        let _ = std::fs::remove_file(&part);
        return Err(format!("Download looks incomplete ({got} bytes). Try again."));
    }
    std::fs::rename(&part, dest).map_err(|e| format!("Cannot save the model file: {e}"))?;
    on_progress(1.0);
    Ok(())
}

/// Width and height of the first video stream from ffprobe JSON.
pub fn video_size_from_probe(json: &str) -> Option<(u32, u32)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v["streams"].as_array()?.iter().find(|s| s["codec_type"] == "video").and_then(|s| Some((s["width"].as_u64()? as u32, s["height"].as_u64()? as u32)))
}

/// Cuts [start, end] from `input` and burns the captions in. Reports real progress from FFmpeg.
#[allow(clippy::too_many_arguments)]
pub fn render_clip_with_captions(
    ffmpeg: &Path,
    input: &Path,
    output: &Path,
    start: f64,
    end: f64,
    transcript: &Transcript,
    style: &CaptionStyle,
    size: (u32, u32),
    work_dir: &Path,
    cancel: &dyn Fn() -> bool,
    on_progress: &mut dyn FnMut(f64),
) -> Result<(), String> {
    if !(end > start) || start < 0.0 {
        return Err("End time must be after start time.".into());
    }
    if input == output {
        return Err("Output must be a different file than the source.".into());
    }
    let words = captions::clip_words(&transcript.words, start, end);
    if words.is_empty() {
        return Err("There is no speech in this time range, so there is nothing to caption.".into());
    }
    let lines = captions::group_lines(&words, style.max_words, 28, 0.8);
    std::fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
    let ass_path = work_dir.join("captions.ass");
    std::fs::write(&ass_path, captions::to_ass(&lines, style, size.0, size.1)).map_err(|e| e.to_string())?;
    // FFmpeg runs inside work_dir and gets only the file name, so no drive letter or colon ever
    // reaches its filter parser (which broke on paths like C:/Users/...).
    let vf = "ass=captions.ass";

    let span = end - start;
    let mut child = command(ffmpeg)
        .current_dir(work_dir)
        .args(["-y", "-hide_banner", "-loglevel", "error", "-progress", "pipe:1", "-nostats"])
        .args(["-ss", &format!("{start:.3}"), "-to", &format!("{end:.3}"), "-i"])
        .arg(input)
        .args(["-vf", vf])
        .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p"])
        .args(["-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"])
        .arg(output)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start ffmpeg ({e})."))?;
    let stdout = child.stdout.take().ok_or("no ffmpeg output")?;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if cancel() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(output);
            return Err("Cancelled".into());
        }
        if let Some(v) = line.strip_prefix("out_time_us=") {
            if let Ok(us) = v.trim().parse::<f64>() {
                on_progress((us / 1_000_000.0 / span).clamp(0.0, 1.0));
            }
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let _ = std::fs::remove_file(output);
        return Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    on_progress(1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_parsing() {
        assert_eq!(parse_whisper_progress("whisper_print_progress_callback: progress =  45%"), Some(0.45));
        assert_eq!(parse_whisper_progress("something else"), None);
    }

    #[test]
    fn language_validation() {
        assert!(valid_language("auto") && valid_language("ur") && !valid_language("../x") && !valid_language("e"));
    }

    #[test]
    fn size_from_probe() {
        let j = r#"{"streams":[{"codec_type":"audio"},{"codec_type":"video","width":854,"height":480}]}"#;
        assert_eq!(video_size_from_probe(j), Some((854, 480)));
    }

    fn have_ffmpeg() -> bool {
        Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
    }

    /// End to end with the real ffmpeg and a stand-in whisper script (Linux/mac only).
    #[cfg(unix)]
    #[test]
    fn pipeline_with_fake_whisper_and_real_ffmpeg() {
        if !have_ffmpeg() {
            eprintln!("ffmpeg not available, skipping");
            return;
        }
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let video = dir.path().join("in.mp4");
        let ok = Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=320x240:rate=25:duration=3", "-f", "lavfi", "-i", "sine=frequency=440:duration=3", "-shortest", "-pix_fmt", "yuv420p"])
            .arg(&video)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let fake = dir.path().join("fake-whisper");
        std::fs::write(
            &fake,
            "#!/bin/sh\nwhile [ $# -gt 0 ]; do if [ \"$1\" = \"-of\" ]; then OF=\"$2\"; fi; shift; done\n\
echo 'whisper_print_progress_callback: progress =  50%' >&2\n\
echo 'whisper_print_progress_callback: progress = 100%' >&2\n\
printf '%s' '{\"result\":{\"language\":\"en\"},\"transcription\":[{\"offsets\":{\"from\":200,\"to\":900},\"text\":\" Hello\"},{\"offsets\":{\"from\":900,\"to\":1800},\"text\":\" there.\"}]}' > \"$OF.json\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let model = dir.path().join("model.bin");
        std::fs::write(&model, b"x").unwrap();

        let cfg = WhisperConfig { ffmpeg: "ffmpeg".into(), whisper: fake, model, language: "auto".into(), work_dir: dir.path().join("work") };
        let mut seen = Vec::new();
        let t = transcribe(&cfg, &video, &|| false, &mut |p| seen.push(p)).unwrap();
        assert_eq!(t.words.len(), 2);
        assert!(seen.contains(&0.05) && (seen.last().copied() == Some(1.0)));
        assert!(cfg.work_dir.join("transcript.json").is_file());

        let out = dir.path().join("out.mp4");
        render_clip_with_captions(Path::new("ffmpeg"), &video, &out, 0.0, 2.0, &t, &captions::preset("bold_yellow"), (320, 240), &dir.path().join("ren der"), &|| false, &mut |_| {}).unwrap();
        assert!(std::fs::metadata(&out).unwrap().len() > 1000);
        // A range with no speech must be refused, not silently exported.
        let err = render_clip_with_captions(Path::new("ffmpeg"), &video, &dir.path().join("o2.mp4"), 2.5, 3.0, &t, &captions::preset("bold_yellow"), (320, 240), &dir.path().join("r2"), &|| false, &mut |_| {}).unwrap_err();
        assert!(err.contains("no speech"));
    }

    #[test]
    fn model_catalogue() {
        assert!(model_info("small").is_some() && model_info("../evil").is_none());
        assert!(MODELS.iter().filter(|m| !m.bundled).all(|m| m.url.starts_with("https://huggingface.co/")));
    }

    #[cfg(unix)]
    #[test]
    fn download_with_curl_file_url() {
        let have_curl = Command::new("curl").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
        if !have_curl {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.bin");
        std::fs::write(&src, vec![7u8; 200_000]).unwrap();
        let dest = dir.path().join("models").join("m.bin");
        let url = format!("file://{}", src.display());
        let mut last = 0.0;
        download_file(Path::new("curl"), &url, &dest, 200_000, 100_000, &|| false, &mut |p| last = p).unwrap();
        assert_eq!(std::fs::metadata(&dest).unwrap().len(), 200_000);
        assert!(!dir.path().join("models").join("m.bin.part").exists());
        assert_eq!(last, 1.0);
        // too small -> refused and nothing left behind
        let dest2 = dir.path().join("models").join("m2.bin");
        let err = download_file(Path::new("curl"), &url, &dest2, 200_000, 900_000, &|| false, &mut |_| {}).unwrap_err();
        assert!(err.contains("incomplete") && !dest2.exists());
        // missing source -> clear error
        let err = download_file(Path::new("curl"), "file:///no/such/file", &dir.path().join("x.bin"), 10, 1, &|| false, &mut |_| {}).unwrap_err();
        assert!(err.starts_with("Download failed"));
    }
}
