use crate::Result;
use rusqlite::Connection;
use std::path::Path;

const SCHEMA_V1: &str = r#"
CREATE TABLE projects (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('clipping','story','editor')),
  status TEXT NOT NULL DEFAULT 'active',
  parent_id INTEGER REFERENCES projects(id) ON DELETE SET NULL,
  budget_usd REAL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE media_assets (
  id INTEGER PRIMARY KEY,
  project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  kind TEXT NOT NULL,
  duration_sec REAL,
  width INTEGER,
  height INTEGER,
  codec TEXT,
  size_bytes INTEGER,
  source_permission_confirmed INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);
CREATE TABLE clips (
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  asset_id INTEGER REFERENCES media_assets(id) ON DELETE SET NULL,
  start_sec REAL NOT NULL,
  end_sec REAL NOT NULL,
  title TEXT,
  hook TEXT,
  description TEXT,
  transcript TEXT,
  score_json TEXT,
  selected INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE scenes (
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  spec_json TEXT NOT NULL,
  asset_id INTEGER REFERENCES media_assets(id) ON DELETE SET NULL
);
CREATE TABLE characters (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  voice_json TEXT,
  reference_assets_json TEXT
);
CREATE TABLE jobs (
  id INTEGER PRIMARY KEY,
  project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'waiting'
    CHECK (status IN ('waiting','processing','completed','failed','cancelled','paused')),
  progress REAL NOT NULL DEFAULT 0,
  payload_json TEXT,
  error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE providers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  category TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'planned',
  enabled INTEGER NOT NULL DEFAULT 0,
  notes TEXT
);
CREATE TABLE usage_log (
  id INTEGER PRIMARY KEY,
  project_id INTEGER REFERENCES projects(id) ON DELETE SET NULL,
  provider_id TEXT NOT NULL,
  model TEXT,
  description TEXT,
  estimated_usd REAL NOT NULL,
  actual_usd REAL,
  created_at INTEGER NOT NULL
);
CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE subtitle_presets (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  style_json TEXT NOT NULL,
  builtin INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE tags (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE
);
CREATE TABLE project_tags (
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (project_id, tag_id)
);
CREATE INDEX idx_jobs_status ON jobs(status);
CREATE INDEX idx_usage_created ON usage_log(created_at);
"#;

/// Providers seeded with an honest status. Nothing is "connected" until a real
/// test call succeeds against the provider's API.
const SEED_PROVIDERS: &[(&str, &str, &str, &str)] = &[
    ("claude", "Claude", "llm", "Official API. Not yet implemented."),
    ("openai", "OpenAI", "llm+image", "Official API. Not yet implemented."),
    ("gemini", "Google Gemini", "llm+image", "Official API. Not yet implemented."),
    ("veo", "Google Veo / Gemini video", "video", "Official API. Not yet implemented."),
    ("runway", "Runway", "video", "Official API. Not yet implemented."),
    ("kling", "Kling", "video", "Official API. Not yet implemented."),
    ("seedance", "Seedance", "video", "API access unclear. Not available yet."),
    ("elevenlabs", "ElevenLabs", "voice", "Official API. Free plan has no commercial use."),
    ("google-tts", "Google Cloud TTS", "voice", "Official API. Not yet implemented."),
    ("faster-whisper", "faster-whisper (local)", "transcription", "Local. Planned for Phase 3."),
];

/// Open (or create) the database and apply migrations.
pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    init(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    init(&conn)?;
    Ok(conn)
}

fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch("BEGIN;")?;
        let res = (|| -> Result<()> {
            conn.execute_batch(SCHEMA_V1)?;
            for (id, name, cat, notes) in SEED_PROVIDERS {
                conn.execute(
                    "INSERT INTO providers (id,name,category,status,enabled,notes) VALUES (?1,?2,?3,'planned',0,?4)",
                    rusqlite::params![id, name, cat, notes],
                )?;
            }
            conn.execute_batch("PRAGMA user_version = 1;")?;
            Ok(())
        })();
        match res {
            Ok(()) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_schema_and_seeds_providers() {
        let c = open_in_memory().unwrap();
        let n: i64 = c.query_row("SELECT COUNT(*) FROM providers", [], |r| r.get(0)).unwrap();
        assert_eq!(n as usize, SEED_PROVIDERS.len());
        let connected: i64 = c
            .query_row("SELECT COUNT(*) FROM providers WHERE status='connected'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(connected, 0, "no provider may start as connected");
    }

    #[test]
    fn reopening_does_not_reseed() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.db");
        drop(open(&p).unwrap());
        let c = open(&p).unwrap();
        let n: i64 = c.query_row("SELECT COUNT(*) FROM providers", [], |r| r.get(0)).unwrap();
        assert_eq!(n as usize, SEED_PROVIDERS.len());
    }
}
