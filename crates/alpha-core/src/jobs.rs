use crate::{now, CoreError, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct Job {
    pub id: i64,
    pub project_id: Option<i64>,
    pub kind: String,
    pub status: String,
    pub progress: f64,
    pub error: Option<String>,
    pub created_at: i64,
}

pub fn enqueue(conn: &Connection, project_id: Option<i64>, kind: &str, payload_json: &str) -> Result<i64> {
    let t = now();
    conn.execute(
        "INSERT INTO jobs (project_id,kind,payload_json,created_at,updated_at) VALUES (?1,?2,?3,?4,?4)",
        params![project_id, kind, payload_json, t],
    )?;
    Ok(conn.last_insert_rowid())
}

fn allowed(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("waiting", "processing")
            | ("waiting", "cancelled")
            | ("processing", "completed")
            | ("processing", "failed")
            | ("processing", "paused")
            | ("processing", "cancelled")
            | ("paused", "processing")
            | ("paused", "cancelled")
            | ("failed", "waiting") // retry
            | ("cancelled", "waiting")
    )
}

pub fn set_status(conn: &Connection, id: i64, to: &str, error: Option<&str>) -> Result<()> {
    let from: String = conn
        .query_row("SELECT status FROM jobs WHERE id=?1", [id], |r| r.get(0))
        .map_err(|_| CoreError::NotFound(format!("job {id}")))?;
    if !allowed(&from, to) {
        return Err(CoreError::Invalid(format!("cannot move job from '{from}' to '{to}'")));
    }
    conn.execute(
        "UPDATE jobs SET status=?1, error=?2, updated_at=?3, progress=CASE WHEN ?1='completed' THEN 1 WHEN ?1='waiting' THEN 0 ELSE progress END WHERE id=?4",
        params![to, error, now(), id],
    )?;
    Ok(())
}

/// Progress must come from real work (for example parsed FFmpeg output).
pub fn set_progress(conn: &Connection, id: i64, progress: f64) -> Result<()> {
    conn.execute(
        "UPDATE jobs SET progress=?1, updated_at=?2 WHERE id=?3",
        params![progress.clamp(0.0, 1.0), now(), id],
    )?;
    Ok(())
}

pub fn list(conn: &Connection) -> Result<Vec<Job>> {
    let mut st = conn.prepare(
        "SELECT id,project_id,kind,status,progress,error,created_at FROM jobs ORDER BY id DESC LIMIT 200",
    )?;
    let rows = st.query_map([], |r| {
        Ok(Job {
            id: r.get(0)?,
            project_id: r.get(1)?,
            kind: r.get(2)?,
            status: r.get(3)?,
            progress: r.get(4)?,
            error: r.get(5)?,
            created_at: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// After a crash or forced close, jobs stuck in 'processing' did not finish.
/// Mark them failed so the user can retry; never pretend they completed.
pub fn recover_interrupted(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE jobs SET status='failed', error='Interrupted: the app closed before this finished. You can retry it.', updated_at=?1 WHERE status='processing'",
        [now()],
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn lifecycle_and_retry() {
        let c = open_in_memory().unwrap();
        let id = enqueue(&c, None, "export", "{}").unwrap();
        assert!(set_status(&c, id, "completed", None).is_err()); // cannot skip processing
        set_status(&c, id, "processing", None).unwrap();
        set_progress(&c, id, 0.4).unwrap();
        set_status(&c, id, "failed", Some("disk full")).unwrap();
        set_status(&c, id, "waiting", None).unwrap();
        assert_eq!(list(&c).unwrap()[0].progress, 0.0);
    }

    #[test]
    fn crash_recovery() {
        let c = open_in_memory().unwrap();
        let id = enqueue(&c, None, "export", "{}").unwrap();
        set_status(&c, id, "processing", None).unwrap();
        assert_eq!(recover_interrupted(&c).unwrap(), 1);
        let j = &list(&c).unwrap()[0];
        assert_eq!(j.status, "failed");
        assert!(j.error.as_deref().unwrap().contains("Interrupted"));
    }
}
