use crate::{CoreError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeMap;

/// Only these keys may be stored. Secrets (API keys) are NOT allowed here;
/// they belong in the OS credential store.
pub const ALLOWED_KEYS: &[&str] = &[
    "projects_dir",
    "exports_dir",
    "monthly_budget_usd",
    "default_project_budget_usd",
    "max_generation_cost_usd",
    "local_only_mode",
    "ai_disclosure_default",
];

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get(0))
        .optional()?)
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    if !ALLOWED_KEYS.contains(&key) {
        return Err(CoreError::Invalid(format!("unknown setting '{key}'")));
    }
    if key.ends_with("_usd") {
        let v: f64 = value
            .trim()
            .parse()
            .map_err(|_| CoreError::Invalid(format!("'{key}' must be a number")))?;
        if v < 0.0 || !v.is_finite() {
            return Err(CoreError::Invalid(format!("'{key}' cannot be negative")));
        }
    }
    conn.execute(
        "INSERT INTO settings (key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value.trim()],
    )?;
    Ok(())
}

pub fn all(conn: &Connection) -> Result<BTreeMap<String, String>> {
    let mut st = conn.prepare("SELECT key,value FROM settings")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.collect::<std::result::Result<BTreeMap<_, _>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn set_get_and_validate() {
        let c = open_in_memory().unwrap();
        set(&c, "monthly_budget_usd", "25.5").unwrap();
        assert_eq!(get(&c, "monthly_budget_usd").unwrap().as_deref(), Some("25.5"));
        set(&c, "monthly_budget_usd", "30").unwrap();
        assert_eq!(all(&c).unwrap().len(), 1);
        assert!(set(&c, "monthly_budget_usd", "abc").is_err());
        assert!(set(&c, "monthly_budget_usd", "-1").is_err());
        assert!(set(&c, "api_key", "secret").is_err());
    }
}
