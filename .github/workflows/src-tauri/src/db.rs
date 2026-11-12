use rusqlite::Connection;
use std::path::Path;

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

const SCHEMA: &str = "
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
CREATE TABLE IF NOT EXISTS projects(
  id TEXT PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'draft', data_json TEXT NOT NULL DEFAULT '{}',
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS providers(
  id TEXT PRIMARY KEY, enabled INTEGER NOT NULL DEFAULT 0, model TEXT);
CREATE TABLE IF NOT EXISTS usage(
  id TEXT PRIMARY KEY, provider TEXT NOT NULL, model TEXT, project_id TEXT,
  estimated_cost REAL NOT NULL DEFAULT 0, actual_cost REAL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS jobs(
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, status TEXT NOT NULL,
  progress REAL NOT NULL DEFAULT 0, error TEXT, payload TEXT NOT NULL DEFAULT '{}',
  output_path TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS media(
  id TEXT PRIMARY KEY, path TEXT NOT NULL, kind TEXT NOT NULL,
  duration REAL, meta_json TEXT NOT NULL DEFAULT '{}', tags TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS exports(
  id TEXT PRIMARY KEY, project_id TEXT, path TEXT NOT NULL, created_at INTEGER NOT NULL);
";

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}
