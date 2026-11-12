//! FFmpeg / FFprobe wrappers. No Tauri types here so the logic stays testable.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const MEDIA_EXTS: &[&str] = &["mp4", "mov", "mkv", "webm", "avi", "mp3", "wav", "m4a"];

/// Find a tool: user setting, then bundled copy next to the app, then PATH.
pub fn tool_path(name: &str, custom_dir: Option<&str>, resource_dir: Option<&Path>) -> PathBuf {
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    if let Some(dir) = custom_dir.filter(|d| !d.is_empty()) {
        let p = Path::new(dir).join(&exe);
        if p.is_file() {
            return p;
        }
    }
    if let Some(res) = resource_dir {
        let p = res.join("ffmpeg").join(&exe);
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from(exe)
}

fn command(path: &Path) -> Command {
    let mut c = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

/// Reject anything that is not an existing regular file with a supported extension.
pub fn validate_media_path(p: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(p);
    let meta = std::fs::metadata(&path).map_err(|_| format!("File not found: {p}"))?;
    if !meta.is_file() {
        return Err("That path is not a file.".into());
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !MEDIA_EXTS.contains(&ext.as_str()) {
        return Err(format!("Unsupported format .{ext}. Supported: {}", MEDIA_EXTS.join(", ")));
    }
    Ok(path)
}

pub fn version(tool: &Path) -> Option<String> {
    let out = command(tool).arg("-version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).lines().next().map(|s| s.to_string())
}

/// Returns raw ffprobe JSON.
pub fn probe(ffprobe: &Path, input: &Path) -> Result<String, String> {
    let out = command(ffprobe)
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
        .arg(input)
        .output()
        .map_err(|e| format!("Could not start ffprobe ({e}). Install FFmpeg or set its folder in Settings."))?;
    if !out.status.success() {
        return Err(format!("ffprobe failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn duration_from_probe(json: &str) -> Option<f64> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v["format"]["duration"].as_str()?.parse().ok()
}

pub fn parse_progress_line(line: &str) -> Option<f64> {
    // `-progress` prints out_time_us=123456 (microseconds). out_time_ms is also microseconds.
    let v = line.strip_prefix("out_time_us=").or_else(|| line.strip_prefix("out_time_ms="))?;
    v.trim().parse::<f64>().ok().map(|us| us / 1_000_000.0)
}

/// Re-encodes [start, end] of `input` into `output` (H.264/AAC, frame-accurate).
/// `cancel` is polled between progress lines. `on_progress` receives 0.0..=1.0.
pub fn trim_export(
    ffmpeg: &Path,
    input: &Path,
    output: &Path,
    start: f64,
    end: f64,
    cancel: &dyn Fn() -> bool,
    on_progress: &mut dyn FnMut(f64),
) -> Result<(), String> {
    if !(end > start) || start < 0.0 {
        return Err("End time must be after start time.".into());
    }
    if input == output {
        return Err("Output must be a different file than the source.".into());
    }
    let span = end - start;
    let mut child = command(ffmpeg)
        .args(["-y", "-hide_banner", "-loglevel", "error", "-progress", "pipe:1", "-nostats"])
        .args(["-ss", &format!("{start:.3}"), "-to", &format!("{end:.3}"), "-i"])
        .arg(input)
        .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p"])
        .args(["-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"])
        .arg(output)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start ffmpeg ({e}). Install FFmpeg or set its folder in Settings."))?;

    let stdout = child.stdout.take().ok_or("no ffmpeg output")?;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if cancel() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(output);
            return Err("Cancelled".into());
        }
        if let Some(secs) = parse_progress_line(&line) {
            on_progress((secs / span).clamp(0.0, 1.0));
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
    fn parses_progress() {
        assert_eq!(parse_progress_line("out_time_us=2500000"), Some(2.5));
        assert_eq!(parse_progress_line("frame=10"), None);
    }

    #[test]
    fn rejects_bad_paths() {
        assert!(validate_media_path("/nonexistent/file.mp4").is_err());
        assert!(validate_media_path("/etc/passwd").is_err());
    }

    #[test]
    fn duration_parse() {
        assert_eq!(duration_from_probe(r#"{"format":{"duration":"12.5"}}"#), Some(12.5));
    }
}
