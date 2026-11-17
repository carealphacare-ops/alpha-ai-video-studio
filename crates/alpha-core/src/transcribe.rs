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
        .args(["-l", &cfg.language, "-oj", "-ml", "1", "-sow", "-pp", "-of"])
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
    let vf = format!("ass={}", captions::ffmpeg_filter_path(&ass_path.to_string_lossy()));

    let span = end - start;
    let mut child = command(ffmpeg)
        .args(["-y", "-hide_banner", "-loglevel", "error", "-progress", "pipe:1", "-nostats"])
        .args(["-ss", &format!("{start:.3}"), "-to", &format!("{end:.3}"), "-i"])
        .arg(input)
        .args(["-vf", &vf])
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
        render_clip_with_captions(Path::new("ffmpeg"), &video, &out, 0.0, 2.0, &t, &captions::preset("bold_yellow"), (320, 240), &dir.path().join("render"), &|| false, &mut |_| {}).unwrap();
        assert!(std::fs::metadata(&out).unwrap().len() > 1000);
        // A range with no speech must be refused, not silently exported.
        let err = render_clip_with_captions(Path::new("ffmpeg"), &video, &dir.path().join("o2.mp4"), 2.5, 3.0, &t, &captions::preset("bold_yellow"), (320, 240), &dir.path().join("r2"), &|| false, &mut |_| {}).unwrap_err();
        assert!(err.contains("no speech"));
    }
}
