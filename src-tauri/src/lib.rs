use alpha_core::{captions, transcribe};

mod db;
mod media;
mod secrets;

use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    db: Arc<Mutex<Connection>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
}

type Res<T> = Result<T, String>;

fn lock<'a>(s: &'a State<'a, AppState>) -> Res<std::sync::MutexGuard<'a, Connection>> {
    s.db.lock().map_err(|_| "database lock poisoned".to_string())
}

fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get(0)).ok()
}

// ---------- projects ----------

#[derive(Serialize)]
struct Project {
    id: String,
    name: String,
    kind: String,
    status: String,
    created_at: i64,
    updated_at: i64,
}

#[tauri::command]
fn list_projects(state: State<AppState>) -> Res<Vec<Project>> {
    let conn = lock(&state)?;
    let mut st = conn
        .prepare("SELECT id,name,kind,status,created_at,updated_at FROM projects ORDER BY updated_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = st
        .query_map([], |r| {
            Ok(Project { id: r.get(0)?, name: r.get(1)?, kind: r.get(2)?, status: r.get(3)?, created_at: r.get(4)?, updated_at: r.get(5)? })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[tauri::command]
fn create_project(state: State<AppState>, name: String, kind: String) -> Res<String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.len() > 120 {
        return Err("Project name must be 1-120 characters.".into());
    }
    if !["clipping", "story", "editor"].contains(&kind.as_str()) {
        return Err("Unknown project kind.".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let t = db::now();
    lock(&state)?
        .execute("INSERT INTO projects(id,name,kind,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)", params![id, name, kind, t])
        .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
fn rename_project(state: State<AppState>, id: String, name: String) -> Res<()> {
    let name = name.trim();
    if name.is_empty() || name.len() > 120 {
        return Err("Project name must be 1-120 characters.".into());
    }
    lock(&state)?
        .execute("UPDATE projects SET name=?1, updated_at=?2 WHERE id=?3", params![name, db::now(), id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn duplicate_project(state: State<AppState>, id: String) -> Res<String> {
    let conn = lock(&state)?;
    let new_id = uuid::Uuid::new_v4().to_string();
    let t = db::now();
    let n = conn
        .execute(
            "INSERT INTO projects(id,name,kind,status,data_json,created_at,updated_at)
             SELECT ?1, name || ' (copy)', kind, status, data_json, ?2, ?2 FROM projects WHERE id=?3",
            params![new_id, t, id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("Project not found.".into());
    }
    Ok(new_id)
}

/// Deletes the project record only. Source media files are never deleted.
#[tauri::command]
fn delete_project(state: State<AppState>, id: String) -> Res<()> {
    lock(&state)?.execute("DELETE FROM projects WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ---------- settings ----------

#[tauri::command]
fn get_settings(state: State<AppState>) -> Res<std::collections::HashMap<String, String>> {
    let conn = lock(&state)?;
    let mut st = conn.prepare("SELECT key,value FROM settings").map_err(|e| e.to_string())?;
    let m = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(m)
}

const ALLOWED_SETTINGS: &[&str] =
    &["projects_dir", "exports_dir", "ffmpeg_dir", "monthly_budget", "project_budget", "max_generation_cost", "local_only"];

#[tauri::command]
fn set_setting(state: State<AppState>, key: String, value: String) -> Res<()> {
    if !ALLOWED_SETTINGS.contains(&key.as_str()) {
        return Err(format!("Unknown setting: {key}"));
    }
    lock(&state)?
        .execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=?2", params![key, value])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------- providers & keys ----------

#[derive(Serialize)]
struct ProviderState {
    id: String,
    enabled: bool,
    has_key: bool,
    model: Option<String>,
}

const PROVIDER_IDS: &[&str] =
    &["anthropic", "openai", "gemini", "veo", "runway", "kling", "seedance", "recraft", "elevenlabs", "google_tts"];

#[tauri::command]
fn list_provider_states(state: State<AppState>) -> Res<Vec<ProviderState>> {
    let conn = lock(&state)?;
    Ok(PROVIDER_IDS
        .iter()
        .map(|id| {
            let (enabled, model): (bool, Option<String>) = conn
                .query_row("SELECT enabled, model FROM providers WHERE id=?1", [id], |r| Ok((r.get::<_, i64>(0)? != 0, r.get(1)?)))
                .unwrap_or((false, None));
            ProviderState { id: id.to_string(), enabled, has_key: secrets::has(id), model }
        })
        .collect())
}

#[tauri::command]
fn set_provider_enabled(state: State<AppState>, id: String, enabled: bool) -> Res<()> {
    if !PROVIDER_IDS.contains(&id.as_str()) {
        return Err("Unknown provider.".into());
    }
    lock(&state)?
        .execute("INSERT INTO providers(id,enabled) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET enabled=?2", params![id, enabled as i64])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn set_provider_model(state: State<AppState>, id: String, model: String) -> Res<()> {
    if !PROVIDER_IDS.contains(&id.as_str()) {
        return Err("Unknown provider.".into());
    }
    lock(&state)?
        .execute("INSERT INTO providers(id,model) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET model=?2", params![id, model.trim()])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn save_api_key(id: String, key: String) -> Res<()> {
    if !PROVIDER_IDS.contains(&id.as_str()) {
        return Err("Unknown provider.".into());
    }
    if key.trim().is_empty() {
        return Err("Key is empty.".into());
    }
    secrets::set(&id, key.trim())
}

#[tauri::command]
fn remove_api_key(id: String) -> Res<()> {
    if !PROVIDER_IDS.contains(&id.as_str()) {
        return Err("Unknown provider.".into());
    }
    secrets::delete(&id)
}

// ---------- usage / dashboard ----------

#[derive(Serialize)]
struct UsageSummary {
    month_estimated: f64,
    month_actual: f64,
    count: i64,
}

#[tauri::command]
fn usage_summary(state: State<AppState>) -> Res<UsageSummary> {
    let conn = lock(&state)?;
    let since = db::now() - 30 * 86400;
    conn.query_row(
        "SELECT COALESCE(SUM(estimated_cost),0), COALESCE(SUM(actual_cost),0), COUNT(*) FROM usage WHERE created_at>=?1",
        [since],
        |r| Ok(UsageSummary { month_estimated: r.get(0)?, month_actual: r.get(1)?, count: r.get(2)? }),
    )
    .map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct Job {
    id: String,
    kind: String,
    status: String,
    progress: f64,
    error: Option<String>,
    output_path: Option<String>,
    updated_at: i64,
}

#[tauri::command]
fn list_jobs(state: State<AppState>) -> Res<Vec<Job>> {
    let conn = lock(&state)?;
    let mut st = conn
        .prepare("SELECT id,kind,status,progress,error,output_path,updated_at FROM jobs ORDER BY updated_at DESC LIMIT 50")
        .map_err(|e| e.to_string())?;
    let rows = st
        .query_map([], |r| {
            Ok(Job { id: r.get(0)?, kind: r.get(1)?, status: r.get(2)?, progress: r.get(3)?, error: r.get(4)?, output_path: r.get(5)?, updated_at: r.get(6)? })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[derive(Serialize)]
struct ToolStatus {
    ffmpeg: Option<String>,
    ffprobe: Option<String>,
}

fn tools(app: &AppHandle, state: &State<AppState>) -> Res<(PathBuf, PathBuf)> {
    let conn = lock(state)?;
    let dir = get_setting(&conn, "ffmpeg_dir");
    drop(conn);
    let res = app.path().resource_dir().ok();
    Ok((
        media::tool_path("ffmpeg", dir.as_deref(), res.as_deref()),
        media::tool_path("ffprobe", dir.as_deref(), res.as_deref()),
    ))
}

#[tauri::command]
async fn tool_status(app: AppHandle, state: State<'_, AppState>) -> Res<ToolStatus> {
    let (ff, fp) = tools(&app, &state)?;
    tauri::async_runtime::spawn_blocking(move || ToolStatus { ffmpeg: media::version(&ff), ffprobe: media::version(&fp) })
        .await
        .map_err(|e| e.to_string())
}

// ---------- media ----------

#[tauri::command]
async fn probe_media(app: AppHandle, state: State<'_, AppState>, path: String) -> Res<String> {
    let input = media::validate_media_path(&path)?;
    let (_, fp) = tools(&app, &state)?;
    tauri::async_runtime::spawn_blocking(move || media::probe(&fp, &input)).await.map_err(|e| e.to_string())?
}

#[derive(Serialize, Clone)]
struct JobProgress {
    id: String,
    progress: f64,
}

/// Starts a trim export as a tracked job. Returns the job id immediately; progress arrives as `job-progress` events.
#[tauri::command]
async fn start_trim_export(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
    output: String,
    start: f64,
    end: f64,
) -> Res<String> {
    let input_p = media::validate_media_path(&input)?;
    let output_p = PathBuf::from(&output);
    if output_p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) != Some("mp4".into()) {
        return Err("Output file must end in .mp4".into());
    }
    if let Some(dir) = output_p.parent() {
        if !dir.is_dir() {
            return Err("Output folder does not exist.".into());
        }
    }
    let (ff, _) = tools(&app, &state)?;
    let job_id = uuid::Uuid::new_v4().to_string();
    let t = db::now();
    {
        let conn = lock(&state)?;
        conn.execute(
            "INSERT INTO jobs(id,kind,status,progress,payload,output_path,created_at,updated_at) VALUES(?1,'trim_export','processing',0,?2,?3,?4,?4)",
            params![job_id, serde_json::json!({"input": input, "start": start, "end": end}).to_string(), output, t],
        )
        .map_err(|e| e.to_string())?;
    }
    let db = state.db.clone();
    let cancelled = state.cancelled.clone();
    let jid = job_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let cancel_id = jid.clone();
        let cancel_set = cancelled.clone();
        let is_cancelled = move || cancel_set.lock().map(|s| s.contains(&cancel_id)).unwrap_or(false);
        let (app2, jid2, db2) = (app.clone(), jid.clone(), db.clone());
        let mut last = 0.0;
        let mut on_progress = move |p: f64| {
            if p - last >= 0.01 || p >= 1.0 {
                last = p;
                let _ = app2.emit("job-progress", JobProgress { id: jid2.clone(), progress: p });
                if let Ok(c) = db2.lock() {
                    let _ = c.execute("UPDATE jobs SET progress=?1, updated_at=?2 WHERE id=?3", params![p, db::now(), jid2]);
                }
            }
        };
        let result = media::trim_export(&ff, &input_p, &output_p, start, end, &is_cancelled, &mut on_progress);
        let (status, err) = match &result {
            Ok(()) => ("completed", None),
            Err(e) if e == "Cancelled" => ("cancelled", None),
            Err(e) => ("failed", Some(e.clone())),
        };
        if let Ok(c) = db.lock() {
            let _ = c.execute("UPDATE jobs SET status=?1, error=?2, updated_at=?3 WHERE id=?4", params![status, err, db::now(), jid]);
            if status == "completed" {
                let _ = c.execute(
                    "INSERT INTO exports(id,project_id,path,created_at) VALUES(?1,NULL,?2,?3)",
                    params![uuid::Uuid::new_v4().to_string(), output_p.to_string_lossy(), db::now()],
                );
            }
        }
        let _ = app.emit("job-finished", serde_json::json!({"id": jid, "status": status, "error": err}));
    });
    Ok(job_id)
}

// ---------- transcription and captions ----------

#[derive(Serialize)]
struct StartedJob {
    id: String,
    output: String,
}

#[derive(Serialize)]
struct TranscribeStatus {
    ready: bool,
    detail: String,
}

/// Looks for the bundled whisper.cpp engine and model. `ALPHA_WHISPER_CLI` / `ALPHA_WHISPER_MODEL` override for development.
fn whisper_paths(app: &AppHandle) -> (PathBuf, PathBuf) {
    let exe = if cfg!(windows) { "whisper-cli.exe" } else { "whisper-cli" };
    let res = app.path().resource_dir().ok().map(|r| r.join("whisper"));
    let cli = std::env::var_os("ALPHA_WHISPER_CLI")
        .map(PathBuf::from)
        .or_else(|| res.as_ref().map(|d| d.join(exe)).filter(|p| p.is_file()))
        .unwrap_or_else(|| PathBuf::from(exe));
    let model = std::env::var_os("ALPHA_WHISPER_MODEL")
        .map(PathBuf::from)
        .or_else(|| res.as_ref().map(|d| d.join("ggml-base.bin")).filter(|p| p.is_file()))
        .unwrap_or_else(|| PathBuf::from("ggml-base.bin"));
    (cli, model)
}

#[derive(Serialize)]
struct ModelStatus {
    id: String,
    label: String,
    installed: bool,
    bundled: bool,
    approx_mb: u64,
}

fn model_path(app: &AppHandle, m: &transcribe::ModelInfo) -> Res<PathBuf> {
    if m.bundled {
        return Ok(whisper_paths(app).1);
    }
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("models").join(m.file))
}

#[tauri::command]
fn list_models(app: AppHandle) -> Res<Vec<ModelStatus>> {
    transcribe::MODELS
        .iter()
        .map(|m| -> Res<ModelStatus> {
            Ok(ModelStatus {
                id: m.id.to_string(),
                label: m.label.to_string(),
                installed: model_path(&app, m)?.is_file(),
                bundled: m.bundled,
                approx_mb: m.approx_bytes / 1_000_000,
            })
        })
        .collect()
}

/// Downloads an optional Whisper model (only from the fixed list in alpha-core) as a tracked job.
#[tauri::command]
async fn start_model_download(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<StartedJob> {
    let m = transcribe::model_info(&id).filter(|m| !m.bundled).ok_or("Unknown model.")?;
    let dest = model_path(&app, m)?;
    if dest.is_file() {
        return Err("That model is already downloaded.".into());
    }
    let job_id = uuid::Uuid::new_v4().to_string();
    let curl = PathBuf::from(if cfg!(windows) { "curl.exe" } else { "curl" });
    let (url, expected) = (m.url.to_string(), m.approx_bytes);
    let out = dest.to_string_lossy().to_string();
    spawn_job(
        app,
        &state,
        job_id.clone(),
        "model_download",
        serde_json::json!({ "model": id }).to_string(),
        out.clone(),
        None,
        move |cancel, prog| transcribe::download_file(&curl, &url, &dest, expected, expected / 2, cancel, prog),
    )?;
    Ok(StartedJob { id: job_id, output: out })
}

#[tauri::command]
fn transcribe_status(app: AppHandle) -> TranscribeStatus {
    let (cli, model) = whisper_paths(&app);
    let ready = cli.is_file() && model.is_file();
    let detail = if ready {
        "Local Whisper engine and model found.".to_string()
    } else {
        format!(
            "Transcription engine not found (looked for {} and {}). The Windows installer includes them. In development, set ALPHA_WHISPER_CLI and ALPHA_WHISPER_MODEL.",
            cli.display(),
            model.display()
        )
    };
    TranscribeStatus { ready, detail }
}

/// Only files this app wrote (`<app data>/transcripts/<job>/transcript.json`) may be read or changed.
fn transcript_path(app: &AppHandle, p: &str) -> Res<PathBuf> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?.join("transcripts");
    let base = std::fs::canonicalize(&base).map_err(|_| "No transcripts yet.".to_string())?;
    let path = std::fs::canonicalize(p).map_err(|_| "Transcript file not found.".to_string())?;
    if !path.starts_with(&base) || path.file_name().and_then(|n| n.to_str()) != Some("transcript.json") {
        return Err("That file is not one of this app's transcripts.".into());
    }
    Ok(path)
}

fn read_transcript(app: &AppHandle, p: &str) -> Res<captions::Transcript> {
    let path = transcript_path(app, p)?;
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("Transcript file is damaged: {e}"))
}

/// Registers a job row, runs `work` on a background thread, and reports real progress, cancel and result.
#[allow(clippy::too_many_arguments)]
fn spawn_job<F>(
    app: AppHandle,
    state: &State<'_, AppState>,
    job_id: String,
    kind: &str,
    payload: String,
    output_path: String,
    export_path: Option<PathBuf>,
    work: F,
) -> Res<()>
where
    F: FnOnce(&dyn Fn() -> bool, &mut dyn FnMut(f64)) -> Result<(), String> + Send + 'static,
{
    {
        let conn = state.db.lock().map_err(|_| "database lock poisoned".to_string())?;
        conn.execute(
            "INSERT INTO jobs(id,kind,status,progress,payload,output_path,created_at,updated_at) VALUES(?1,?2,'processing',0,?3,?4,?5,?5)",
            params![job_id, kind, payload, output_path, db::now()],
        )
        .map_err(|e| e.to_string())?;
    }
    let db = state.db.clone();
    let cancelled = state.cancelled.clone();
    let jid = job_id;
    tauri::async_runtime::spawn_blocking(move || {
        let cancel_id = jid.clone();
        let cancel_set = cancelled.clone();
        let is_cancelled = move || cancel_set.lock().map(|s| s.contains(&cancel_id)).unwrap_or(false);
        let (app2, jid2, db2) = (app.clone(), jid.clone(), db.clone());
        let mut last = 0.0;
        let mut on_progress = move |p: f64| {
            if p - last >= 0.01 || p >= 1.0 {
                last = p;
                let _ = app2.emit("job-progress", JobProgress { id: jid2.clone(), progress: p });
                if let Ok(c) = db2.lock() {
                    let _ = c.execute("UPDATE jobs SET progress=?1, updated_at=?2 WHERE id=?3", params![p, db::now(), jid2]);
                }
            }
        };
        let result = work(&is_cancelled, &mut on_progress);
        let (status, err) = match &result {
            Ok(()) => ("completed", None),
            Err(e) if e == "Cancelled" => ("cancelled", None),
            Err(e) => ("failed", Some(e.clone())),
        };
        if let Ok(c) = db.lock() {
            let _ = c.execute("UPDATE jobs SET status=?1, error=?2, updated_at=?3 WHERE id=?4", params![status, err, db::now(), jid]);
            if status == "completed" {
                if let Some(path) = &export_path {
                    let _ = c.execute(
                        "INSERT INTO exports(id,project_id,path,created_at) VALUES(?1,NULL,?2,?3)",
                        params![uuid::Uuid::new_v4().to_string(), path.to_string_lossy(), db::now()],
                    );
                }
            }
        }
        let _ = app.emit("job-finished", serde_json::json!({"id": jid, "status": status, "error": err}));
    });
    Ok(())
}

#[tauri::command]
async fn start_transcribe(app: AppHandle, state: State<'_, AppState>, input: String, language: String, model: String) -> Res<StartedJob> {
    let input_p = media::validate_media_path(&input)?;
    let (ff, _) = tools(&app, &state)?;
    let (cli, _) = whisper_paths(&app);
    let info = transcribe::model_info(&model).ok_or("Unknown model.")?;
    let model = model_path(&app, info)?;
    if !cli.is_file() {
        return Err("The transcription engine is not installed. Use the Windows installer build, which includes it.".into());
    }
    if !model.is_file() {
        return Err(format!("The \"{}\" model is not downloaded yet. Download it first.", info.id));
    }
    let job_id = uuid::Uuid::new_v4().to_string();
    let work_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("transcripts").join(&job_id);
    let out = work_dir.join("transcript.json").to_string_lossy().to_string();
    let cfg = transcribe::WhisperConfig { ffmpeg: ff, whisper: cli, model, language, work_dir };
    spawn_job(
        app,
        &state,
        job_id.clone(),
        "transcribe",
        serde_json::json!({ "input": input }).to_string(),
        out.clone(),
        None,
        move |cancel, prog| transcribe::transcribe(&cfg, &input_p, cancel, prog).map(|_| ()),
    )?;
    Ok(StartedJob { id: job_id, output: out })
}

#[tauri::command]
fn load_transcript(app: AppHandle, path: String) -> Res<String> {
    let t = read_transcript(&app, &path)?;
    serde_json::to_string(&t).map_err(|e| e.to_string())
}

/// Saves a corrected transcript (for example after fixing a misheard name).
#[tauri::command]
fn save_transcript(app: AppHandle, path: String, json: String) -> Res<()> {
    let p = transcript_path(&app, &path)?;
    let t: captions::Transcript = serde_json::from_str(&json).map_err(|e| format!("Not a valid transcript: {e}"))?;
    let text = serde_json::to_string_pretty(&t).map_err(|e| e.to_string())?;
    std::fs::write(p, text).map_err(|e| format!("Cannot save transcript: {e}"))
}

/// Writes an SRT, VTT or ASS file. If `end > start` only that range is used (times are moved so it starts at 0).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn export_captions(
    app: AppHandle,
    transcript: String,
    format: String,
    output: String,
    style: String,
    start: f64,
    end: f64,
    width: u32,
    height: u32,
) -> Res<()> {
    let t = read_transcript(&app, &transcript)?;
    let ext = match format.as_str() {
        "srt" => "srt",
        "vtt" => "vtt",
        "ass" => "ass",
        _ => return Err("Unknown caption format.".into()),
    };
    let out = PathBuf::from(&output);
    if out.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() != Some(ext) {
        return Err(format!("Output file must end in .{ext}"));
    }
    if let Some(dir) = out.parent() {
        if !dir.as_os_str().is_empty() && !dir.is_dir() {
            return Err("Output folder does not exist.".into());
        }
    }
    let words = if end > start { captions::clip_words(&t.words, start, end) } else { t.words.clone() };
    if words.is_empty() {
        return Err("There is no speech in that range.".into());
    }
    let st = captions::preset(&style);
    let lines = if ext == "ass" { captions::group_lines(&words, st.max_words, 28, 0.8) } else { captions::group_lines(&words, 7, 42, 0.8) };
    let body = match ext {
        "srt" => captions::to_srt(&lines),
        "vtt" => captions::to_vtt(&lines),
        _ => captions::to_ass(&lines, &st, width.max(16), height.max(16)),
    };
    std::fs::write(&out, body).map_err(|e| format!("Cannot write file: {e}"))
}

/// Cuts [start, end] and burns animated word-highlight captions into the MP4.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn start_caption_clip(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
    transcript: String,
    output: String,
    start: f64,
    end: f64,
    style: String,
) -> Res<StartedJob> {
    let input_p = media::validate_media_path(&input)?;
    let output_p = PathBuf::from(&output);
    if output_p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) != Some("mp4".into()) {
        return Err("Output file must end in .mp4".into());
    }
    if let Some(dir) = output_p.parent() {
        if !dir.is_dir() {
            return Err("Output folder does not exist.".into());
        }
    }
    let t = read_transcript(&app, &transcript)?;
    let (ff, fp) = tools(&app, &state)?;
    let size = transcribe::video_size_from_probe(&media::probe(&fp, &input_p)?)
        .ok_or_else(|| "This file has no video picture to put captions on.".to_string())?;
    let st = captions::preset(&style);
    let job_id = uuid::Uuid::new_v4().to_string();
    let work_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("render").join(&job_id);
    let out2 = output_p.clone();
    spawn_job(
        app,
        &state,
        job_id.clone(),
        "caption_clip",
        serde_json::json!({ "input": input, "start": start, "end": end, "style": style }).to_string(),
        output.clone(),
        Some(output_p),
        move |cancel, prog| transcribe::render_clip_with_captions(&ff, &input_p, &out2, start, end, &t, &st, size, &work_dir, cancel, prog),
    )?;
    Ok(StartedJob { id: job_id, output })
}

#[tauri::command]
fn cancel_job(state: State<AppState>, id: String) -> Res<()> {
    state.cancelled.lock().map_err(|_| "lock".to_string())?.insert(id);
    Ok(())
}

#[tauri::command]
fn list_exports(state: State<AppState>) -> Res<Vec<(String, i64)>> {
    let conn = lock(&state)?;
    let mut st = conn.prepare("SELECT path, created_at FROM exports ORDER BY created_at DESC LIMIT 20").map_err(|e| e.to_string())?;
    let rows = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open(&dir.join("alpha.db")).map_err(|e| e.to_string())?;
            // Jobs left "processing" by a crash cannot still be running.
            let _ = conn.execute(
                "UPDATE jobs SET status='failed', error='Interrupted (app closed during processing)' WHERE status='processing'",
                [],
            );
            app.manage(AppState { db: Arc::new(Mutex::new(conn)), cancelled: Arc::new(Mutex::new(HashSet::new())) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            create_project,
            rename_project,
            duplicate_project,
            delete_project,
            get_settings,
            set_setting,
            list_provider_states,
            set_provider_enabled,
            set_provider_model,
            save_api_key,
            remove_api_key,
            usage_summary,
            list_jobs,
            list_exports,
            tool_status,
            probe_media,
            start_trim_export,
            cancel_job,
            transcribe_status,
            list_models,
            start_model_download,
            start_transcribe,
            load_transcript,
            save_transcript,
            export_captions,
            start_caption_clip,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ALPHA AI VIDEO STUDIO");
}
