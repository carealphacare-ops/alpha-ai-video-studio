use crate::{now, CoreError, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

pub const SUPPORTED_EXT: &[&str] = &["mp4", "mov", "mkv", "webm", "avi", "mp3", "wav", "m4a"];

#[derive(Debug, Serialize, Clone)]
pub struct MediaInfo {
    pub kind: String, // "video" | "audio"
    pub duration_sec: f64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub codec: Option<String>,
    pub size_bytes: i64,
}

/// Parse `ffprobe -print_format json -show_format -show_streams` output.
pub fn parse_probe(json: &str) -> Result<MediaInfo> {
    let v: Value = serde_json::from_str(json).map_err(|e| CoreError::Media(format!("bad ffprobe output: {e}")))?;
    let streams = v["streams"].as_array().ok_or_else(|| CoreError::Media("no streams found".into()))?;
    let video = streams.iter().find(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] != 1);
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    let main = video.or(audio).ok_or_else(|| CoreError::Media("file has no audio or video stream".into()))?;
    let duration = v["format"]["duration"]
        .as_str()
        .and_then(|d| d.parse::<f64>().ok())
        .ok_or_else(|| CoreError::Media("could not read duration".into()))?;
    let size = v["format"]["size"].as_str().and_then(|d| d.parse::<i64>().ok()).unwrap_or(0);
    Ok(MediaInfo {
        kind: if video.is_some() { "video".into() } else { "audio".into() },
        duration_sec: duration,
        width: video.and_then(|s| s["width"].as_i64()),
        height: video.and_then(|s| s["height"].as_i64()),
        codec: main["codec_name"].as_str().map(String::from),
        size_bytes: size,
    })
}

/// Check the extension before running any tool on a file.
pub fn validate_path(path: &Path) -> Result<()> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !SUPPORTED_EXT.contains(&ext.as_str()) {
        return Err(CoreError::Media(format!(
            "unsupported file type '.{ext}'. Supported: {}",
            SUPPORTED_EXT.join(", ")
        )));
    }
    if !path.is_file() {
        return Err(CoreError::Media(format!("file not found: {}", path.display())));
    }
    Ok(())
}

/// Run ffprobe (argument list, no shell) and return parsed info.
pub fn probe(ffprobe_bin: &str, path: &Path) -> Result<MediaInfo> {
    validate_path(path)?;
    let out = Command::new(ffprobe_bin)
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
        .arg(path)
        .output()
        .map_err(|e| CoreError::Media(format!("could not run ffprobe ('{ffprobe_bin}'): {e}")))?;
    if !out.status.success() {
        return Err(CoreError::Media(format!(
            "ffprobe could not read this file: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    parse_probe(&String::from_utf8_lossy(&out.stdout))
}

/// Probe a file and register it. Requires the user to confirm they have rights to the content.
pub fn import(
    conn: &Connection,
    ffprobe_bin: &str,
    project_id: i64,
    path: &Path,
    permission_confirmed: bool,
) -> Result<i64> {
    if !permission_confirmed {
        return Err(CoreError::Invalid(
            "please confirm you own this content or have permission to use it".into(),
        ));
    }
    let info = probe(ffprobe_bin, path)?;
    conn.execute(
        "INSERT INTO media_assets (project_id,path,kind,duration_sec,width,height,codec,size_bytes,source_permission_confirmed,created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9)",
        params![
            project_id,
            path.to_string_lossy(),
            info.kind,
            info.duration_sec,
            info.width,
            info.height,
            info.codec,
            info.size_bytes,
            now()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_video_probe() {
        let j = r#"{"streams":[{"codec_type":"video","codec_name":"h264","width":1920,"height":1080},
                    {"codec_type":"audio","codec_name":"aac"}],
                    "format":{"duration":"12.5","size":"1000"}}"#;
        let i = parse_probe(j).unwrap();
        assert_eq!(i.kind, "video");
        assert_eq!(i.width, Some(1920));
        assert!((i.duration_sec - 12.5).abs() < 1e-9);
    }

    #[test]
    fn parses_audio_only() {
        let j = r#"{"streams":[{"codec_type":"audio","codec_name":"mp3"}],"format":{"duration":"3","size":"5"}}"#;
        assert_eq!(parse_probe(j).unwrap().kind, "audio");
    }

    #[test]
    fn rejects_garbage_and_bad_ext() {
        assert!(parse_probe("not json").is_err());
        assert!(parse_probe(r#"{"streams":[],"format":{"duration":"1"}}"#).is_err());
        assert!(validate_path(Path::new("/tmp/x.exe")).is_err());
    }

    #[test]
    fn real_ffprobe_on_generated_file() {
        // Uses the system ffmpeg if present; skipped otherwise.
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("t.mp4");
        let ok = Command::new("ffmpeg")
            .args(["-v", "error", "-f", "lavfi", "-i", "testsrc=duration=2:size=320x240:rate=15", "-pix_fmt", "yuv420p", "-y"])
            .arg(&f)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            eprintln!("ffmpeg not available, skipping");
            return;
        }
        let i = probe("ffprobe", &f).unwrap();
        assert_eq!(i.width, Some(320));
        assert!(i.duration_sec > 1.5 && i.duration_sec < 2.5);
    }
}
