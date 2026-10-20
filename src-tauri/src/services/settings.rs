use crate::db::Db;
use crate::error::AppResult;
use crate::finance::LaborRate;
use crate::models::SettingsMap;
use rusqlite::OptionalExtension;
use std::collections::HashMap;

pub fn get_all(db: &Db) -> AppResult<SettingsMap> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT key, value FROM application_settings")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut values = HashMap::new();
        for row in rows {
            let (k, v) = row?;
            values.insert(k, v);
        }
        Ok(SettingsMap { values })
    })
}

pub fn get(db: &Db, key: &str) -> AppResult<Option<String>> {
    db.with_conn(|conn| {
        let v = conn
            .query_row(
                "SELECT value FROM application_settings WHERE key = ?1",
                [key],
                |r| r.get(0),
            )
            .optional()?;
        Ok(v)
    })
}

pub fn set(db: &Db, key: &str, value: &str) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO application_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            (key, value, now),
        )?;
        Ok(())
    })
}

pub fn set_many(db: &Db, map: &HashMap<String, String>) -> AppResult<()> {
    for (k, v) in map {
        set(db, k, v)?;
    }
    Ok(())
}

pub fn labor_rate(db: &Db) -> AppResult<LaborRate> {
    let salary: i64 = get(db, "monthly_salary_minor")?
        .unwrap_or_else(|| "0".into())
        .parse()
        .unwrap_or(0);
    let hours: f64 = get(db, "monthly_hours")?
        .unwrap_or_else(|| "160".into())
        .parse()
        .unwrap_or(160.0);
    let currency = get(db, "default_currency")?.unwrap_or_else(|| "USD".into());
    Ok(LaborRate {
        monthly_salary_minor: salary,
        monthly_hours: hours,
        currency,
    })
}

pub fn idle_threshold_minutes(db: &Db) -> AppResult<i64> {
    Ok(get(db, "idle_threshold_minutes")?
        .unwrap_or_else(|| "5".into())
        .parse()
        .unwrap_or(5))
}

pub fn ingest_port(db: &Db) -> AppResult<u16> {
    Ok(get(db, "ingest_port")?
        .unwrap_or_else(|| "17321".into())
        .parse()
        .unwrap_or(17321))
}
