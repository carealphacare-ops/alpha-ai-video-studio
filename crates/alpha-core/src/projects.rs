use crate::{now, CoreError, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub budget_usd: Option<f64>,
    pub created_at: i64,
    pub updated_at: i64,
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: r.get(2)?,
        status: r.get(3)?,
        budget_usd: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
    })
}

const COLS: &str = "id,name,kind,status,budget_usd,created_at,updated_at";

fn check_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(CoreError::Invalid("project name cannot be empty".into()));
    }
    if n.chars().count() > 120 {
        return Err(CoreError::Invalid("project name is too long (max 120)".into()));
    }
    Ok(n.to_string())
}

pub fn create(conn: &Connection, name: &str, kind: &str) -> Result<Project> {
    let name = check_name(name)?;
    if !["clipping", "story", "editor"].contains(&kind) {
        return Err(CoreError::Invalid(format!("unknown project kind '{kind}'")));
    }
    let t = now();
    conn.execute(
        "INSERT INTO projects (name,kind,created_at,updated_at) VALUES (?1,?2,?3,?3)",
        params![name, kind, t],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn get(conn: &Connection, id: i64) -> Result<Project> {
    conn.query_row(&format!("SELECT {COLS} FROM projects WHERE id=?1"), [id], row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound(format!("project {id}")),
            other => other.into(),
        })
}

/// List projects, newest first. Optional search text matches the name.
pub fn list(conn: &Connection, search: Option<&str>) -> Result<Vec<Project>> {
    let like = format!("%{}%", search.unwrap_or("").trim());
    let mut st = conn.prepare(&format!(
        "SELECT {COLS} FROM projects WHERE status='active' AND name LIKE ?1 ORDER BY updated_at DESC, id DESC"
    ))?;
    let rows = st.query_map([like], row)?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn rename(conn: &Connection, id: i64, name: &str) -> Result<Project> {
    let name = check_name(name)?;
    let n = conn.execute(
        "UPDATE projects SET name=?1, updated_at=?2 WHERE id=?3",
        params![name, now(), id],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound(format!("project {id}")));
    }
    get(conn, id)
}

/// Duplicate a project with its clips and scenes. Media files are NOT copied;
/// asset rows keep pointing at the same files on disk.
pub fn duplicate(conn: &Connection, id: i64) -> Result<Project> {
    let src = get(conn, id)?;
    let t = now();
    conn.execute(
        "INSERT INTO projects (name,kind,parent_id,budget_usd,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?5)",
        params![format!("{} (copy)", src.name), src.kind, id, src.budget_usd, t],
    )?;
    let new_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO clips (project_id,asset_id,start_sec,end_sec,title,hook,description,transcript,score_json,selected)
         SELECT ?1,asset_id,start_sec,end_sec,title,hook,description,transcript,score_json,selected FROM clips WHERE project_id=?2",
        params![new_id, id],
    )?;
    conn.execute(
        "INSERT INTO scenes (project_id,position,spec_json,asset_id)
         SELECT ?1,position,spec_json,asset_id FROM scenes WHERE project_id=?2",
        params![new_id, id],
    )?;
    get(conn, new_id)
}

/// Remove a project from the database. Source media files on disk are never deleted.
pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    let n = conn.execute("DELETE FROM projects WHERE id=?1", [id])?;
    if n == 0 {
        return Err(CoreError::NotFound(format!("project {id}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn create_list_rename_delete() {
        let c = open_in_memory().unwrap();
        let p = create(&c, "  My Clips  ", "clipping").unwrap();
        assert_eq!(p.name, "My Clips");
        assert_eq!(list(&c, None).unwrap().len(), 1);
        assert_eq!(list(&c, Some("zzz")).unwrap().len(), 0);
        let r = rename(&c, p.id, "Renamed").unwrap();
        assert_eq!(r.name, "Renamed");
        delete(&c, p.id).unwrap();
        assert!(get(&c, p.id).is_err());
    }

    #[test]
    fn rejects_bad_input() {
        let c = open_in_memory().unwrap();
        assert!(create(&c, "   ", "clipping").is_err());
        assert!(create(&c, "ok", "weird").is_err());
        assert!(rename(&c, 999, "x").is_err());
        assert!(delete(&c, 999).is_err());
    }

    #[test]
    fn duplicate_copies_clips() {
        let c = open_in_memory().unwrap();
        let p = create(&c, "Orig", "clipping").unwrap();
        c.execute(
            "INSERT INTO clips (project_id,start_sec,end_sec,title) VALUES (?1,1.0,20.0,'t')",
            [p.id],
        )
        .unwrap();
        let d = duplicate(&c, p.id).unwrap();
        assert_eq!(d.name, "Orig (copy)");
        let n: i64 = c
            .query_row("SELECT COUNT(*) FROM clips WHERE project_id=?1", [d.id], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
}
