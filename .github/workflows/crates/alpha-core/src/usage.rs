use crate::{now, settings, CoreError, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct UsageSummary {
    /// Sum of estimates for this calendar period (NOT provider-confirmed).
    pub estimated_usd: f64,
    /// Sum of provider-reported charges (only rows where the provider reported one).
    pub actual_usd: f64,
    pub monthly_budget_usd: Option<f64>,
}

pub fn record_estimate(
    conn: &Connection,
    project_id: Option<i64>,
    provider_id: &str,
    model: &str,
    description: &str,
    estimated_usd: f64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO usage_log (project_id,provider_id,model,description,estimated_usd,created_at) VALUES (?1,?2,?3,?4,?5,?6)",
        params![project_id, provider_id, model, description, estimated_usd, now()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn record_actual(conn: &Connection, usage_id: i64, actual_usd: f64) -> Result<()> {
    conn.execute("UPDATE usage_log SET actual_usd=?1 WHERE id=?2", params![actual_usd, usage_id])?;
    Ok(())
}

/// Sum since `since_unix` (start of the month, chosen by the caller).
pub fn summary_since(conn: &Connection, since_unix: i64) -> Result<UsageSummary> {
    let (est, act): (f64, f64) = conn.query_row(
        "SELECT COALESCE(SUM(estimated_usd),0), COALESCE(SUM(actual_usd),0) FROM usage_log WHERE created_at>=?1",
        [since_unix],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let budget = settings::get(conn, "monthly_budget_usd")?.and_then(|v| v.parse::<f64>().ok());
    Ok(UsageSummary { estimated_usd: est, actual_usd: act, monthly_budget_usd: budget })
}

/// Gate to call BEFORE any paid cloud request. Returns Err if it would break a limit.
/// The UI must still show the estimate and ask the user to confirm.
pub fn check_budget(conn: &Connection, since_unix: i64, new_cost_usd: f64) -> Result<()> {
    if let Some(max) = settings::get(conn, "max_generation_cost_usd")?.and_then(|v| v.parse::<f64>().ok()) {
        if new_cost_usd > max {
            return Err(CoreError::Invalid(format!(
                "estimated cost ${new_cost_usd:.2} is above your per-generation limit of ${max:.2}"
            )));
        }
    }
    if let Some(budget) = settings::get(conn, "monthly_budget_usd")?.and_then(|v| v.parse::<f64>().ok()) {
        let used = summary_since(conn, since_unix)?;
        // Use the larger of estimated and actual so we never under-count.
        let spent = used.estimated_usd.max(used.actual_usd);
        if spent + new_cost_usd > budget {
            return Err(CoreError::Invalid(format!(
                "this would pass your monthly budget of ${budget:.2} (about ${spent:.2} already used)"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn budget_checks() {
        let c = open_in_memory().unwrap();
        assert!(check_budget(&c, 0, 100.0).is_ok()); // no limits set
        settings::set(&c, "monthly_budget_usd", "10").unwrap();
        settings::set(&c, "max_generation_cost_usd", "4").unwrap();
        assert!(check_budget(&c, 0, 5.0).is_err()); // above per-generation cap
        let id = record_estimate(&c, None, "veo", "m", "test", 3.0).unwrap();
        record_estimate(&c, None, "veo", "m", "test", 3.0).unwrap();
        assert!(check_budget(&c, 0, 3.0).is_ok()); // 6 + 3 = 9 <= 10
        assert!(check_budget(&c, 0, 4.0).is_ok()); // 6 + 4 = 10: exactly at budget, allowed
        settings::set(&c, "max_generation_cost_usd", "5").unwrap();
        assert!(check_budget(&c, 0, 4.5).is_err()); // 6 + 4.5 = 10.5 passes the monthly budget
        settings::set(&c, "max_generation_cost_usd", "4").unwrap();
        record_actual(&c, id, 3.5).unwrap();
        let s = summary_since(&c, 0).unwrap();
        assert_eq!(s.estimated_usd, 6.0);
        assert_eq!(s.actual_usd, 3.5);
    }
}
