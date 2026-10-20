use crate::db::Db;
use crate::error::AppResult;
use crate::models::UsageRecordInput;
use rusqlite::params;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsageRecordDto {
    pub id: String,
    pub model: Option<String>,
    pub occurred_at: String,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub total_tokens: Option<i64>,
    pub reported_cost_minor: Option<i64>,
    pub billing_source: String,
    pub verification_status: String,
    pub notes: Option<String>,
}

pub fn add_manual(db: &Db, input: UsageRecordInput) -> AppResult<UsageRecordDto> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let occurred = input.occurred_at.unwrap_or_else(|| now.clone());
    let external = input
        .external_key
        .unwrap_or_else(|| format!("manual:{}", id));
    let total = input.total_tokens.or_else(|| {
        Some(
            input.input_tokens.unwrap_or(0)
                + input.output_tokens.unwrap_or(0)
                + input.cached_tokens.unwrap_or(0),
        )
    });
    // If caller passes no token fields at all, store NULL (Unavailable), not zero.
    let total = if input.input_tokens.is_none()
        && input.output_tokens.is_none()
        && input.cached_tokens.is_none()
        && input.total_tokens.is_none()
    {
        None
    } else {
        total
    };

    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO ai_usage_records (
                id, provider, model, occurred_at, project_id, task_id, input_tokens, output_tokens,
                cached_tokens, total_tokens, reported_cost_minor, currency, billing_source,
                verification_status, attribution_method, external_key, notes
             ) VALUES (?1,'cursor',?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,'manual',?14,?15)
             ON CONFLICT(external_key) DO NOTHING",
            params![
                id,
                input.model,
                occurred,
                input.project_id,
                input.task_id,
                input.input_tokens,
                input.output_tokens,
                input.cached_tokens,
                total,
                input.reported_cost_minor,
                input.currency,
                input.billing_source,
                input.verification_status,
                external,
                input.notes
            ],
        )?;
        Ok(())
    })?;

    Ok(UsageRecordDto {
        id,
        model: input.model,
        occurred_at: occurred,
        project_id: input.project_id,
        task_id: input.task_id,
        total_tokens: total,
        reported_cost_minor: input.reported_cost_minor,
        billing_source: input.billing_source,
        verification_status: input.verification_status,
        notes: input.notes,
    })
}

/// Idempotent CSV import. Expected flexible headers; unknown token fields remain NULL.
pub fn import_csv(db: &Db, filename: &str, csv_text: &str) -> AppResult<ImportResult> {
    let mut hasher = Sha256::new();
    hasher.update(csv_text.as_bytes());
    let content_hash = hex::encode(hasher.finalize());

    let existing: Option<String> = db.with_conn(|conn| {
        Ok(conn
            .query_row(
                "SELECT id FROM usage_imports WHERE content_hash = ?1",
                [&content_hash],
                |r| r.get(0),
            )
            .optional()?)
    })?;
    if let Some(id) = existing {
        return Ok(ImportResult {
            import_id: id,
            inserted: 0,
            skipped_duplicate_import: true,
            message: "identical import already applied".into(),
        });
    }

    let import_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut inserted = 0i64;

    let lines: Vec<&str> = csv_text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return Ok(ImportResult {
            import_id,
            inserted: 0,
            skipped_duplicate_import: false,
            message: "empty csv".into(),
        });
    }
    let headers: Vec<String> = lines[0]
        .split(',')
        .map(|h| h.trim().trim_matches('"').to_ascii_lowercase())
        .collect();

    db.with_conn_mut(|conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO usage_imports (id, source, filename, imported_at, record_count, content_hash)
             VALUES (?1, 'csv', ?2, ?3, 0, ?4)",
            params![import_id, filename, now, content_hash],
        )?;

        for (idx, line) in lines.iter().skip(1).enumerate() {
            let cols: Vec<&str> = parse_csv_line(line);
            let get = |name: &str| {
                headers
                    .iter()
                    .position(|h| h == name)
                    .and_then(|i| cols.get(i).map(|s| s.trim().trim_matches('"')))
                    .filter(|s| !s.is_empty())
            };
            let external_key = get("id")
                .or_else(|| get("request_id"))
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}:{}", content_hash, idx));
            let model = get("model").map(|s| s.to_string());
            let occurred = get("date")
                .or_else(|| get("timestamp"))
                .or_else(|| get("created_at"))
                .unwrap_or(&now)
                .to_string();
            let input_tokens = get("input_tokens")
                .or_else(|| get("input"))
                .and_then(|s| s.replace(',', "").parse().ok());
            let output_tokens = get("output_tokens")
                .or_else(|| get("output"))
                .and_then(|s| s.replace(',', "").parse().ok());
            let cached_tokens = get("cached_tokens")
                .or_else(|| get("cache_read_tokens"))
                .and_then(|s| s.replace(',', "").parse().ok());
            let total_tokens = get("total_tokens")
                .or_else(|| get("tokens"))
                .and_then(|s| s.replace(',', "").parse().ok())
                .or_else(|| {
                    if input_tokens.is_some() || output_tokens.is_some() || cached_tokens.is_some() {
                        Some(
                            input_tokens.unwrap_or(0)
                                + output_tokens.unwrap_or(0)
                                + cached_tokens.unwrap_or(0),
                        )
                    } else {
                        None
                    }
                });
            let cost_raw = get("cost").or_else(|| get("charged"));
            let reported_cost_minor = cost_raw.and_then(|s| {
                let s = s.trim();
                if s.eq_ignore_ascii_case("included") || s == "-" {
                    return None;
                }
                let cleaned = s.replace('$', "").replace(',', "");
                cleaned.parse::<f64>().ok().map(|v| (v * 100.0).round() as i64)
            });

            let changed = tx.execute(
                "INSERT INTO ai_usage_records (
                    id, import_id, provider, model, occurred_at, input_tokens, output_tokens,
                    cached_tokens, total_tokens, reported_cost_minor, currency, billing_source,
                    verification_status, attribution_method, external_key
                 ) VALUES (?1,?2,'cursor',?3,?4,?5,?6,?7,?8,?9,'USD','cursor_csv','imported_csv','import',?10)
                 ON CONFLICT(external_key) DO NOTHING",
                params![
                    Uuid::new_v4().to_string(),
                    import_id,
                    model,
                    occurred,
                    input_tokens,
                    output_tokens,
                    cached_tokens,
                    total_tokens,
                    reported_cost_minor,
                    external_key
                ],
            )?;
            inserted += changed as i64;
        }

        tx.execute(
            "UPDATE usage_imports SET record_count = ?1 WHERE id = ?2",
            params![inserted, import_id],
        )?;
        tx.commit()?;
        Ok(())
    })?;

    Ok(ImportResult {
        import_id,
        inserted,
        skipped_duplicate_import: false,
        message: "ok".into(),
    })
}

fn parse_csv_line(line: &str) -> Vec<&str> {
    // Minimal CSV split (handles simple quoted fields)
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut in_q = false;
    let bytes = line.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        match *b {
            b'"' => in_q = !in_q,
            b',' if !in_q => {
                out.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&line[start..]);
    out
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportResult {
    pub import_id: String,
    pub inserted: i64,
    pub skipped_duplicate_import: bool,
    pub message: String,
}

pub fn list_usage(db: &Db, project_id: Option<&str>) -> AppResult<Vec<UsageRecordDto>> {
    db.with_conn(|conn| {
        let mut out = Vec::new();
        if let Some(pid) = project_id {
            let mut stmt = conn.prepare(
                "SELECT id, model, occurred_at, project_id, task_id, total_tokens, reported_cost_minor,
                        billing_source, verification_status, notes
                 FROM ai_usage_records
                 WHERE project_id = ?1 OR task_id IN (SELECT id FROM tasks WHERE project_id = ?1)
                 ORDER BY occurred_at DESC LIMIT 500",
            )?;
            let rows = stmt.query_map([pid], map_usage)?;
            for r in rows {
                out.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, model, occurred_at, project_id, task_id, total_tokens, reported_cost_minor,
                        billing_source, verification_status, notes
                 FROM ai_usage_records ORDER BY occurred_at DESC LIMIT 500",
            )?;
            let rows = stmt.query_map([], map_usage)?;
            for r in rows {
                out.push(r?);
            }
        }
        Ok(out)
    })
}

fn map_usage(r: &rusqlite::Row<'_>) -> rusqlite::Result<UsageRecordDto> {
    Ok(UsageRecordDto {
        id: r.get(0)?,
        model: r.get(1)?,
        occurred_at: r.get(2)?,
        project_id: r.get(3)?,
        task_id: r.get(4)?,
        total_tokens: r.get(5)?,
        reported_cost_minor: r.get(6)?,
        billing_source: r.get(7)?,
        verification_status: r.get(8)?,
        notes: r.get(9)?,
    })
}

use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use tempfile::tempdir;

    #[test]
    fn manual_unavailable_tokens_stay_null() {
        let dir = tempdir().unwrap();
        let db = Db::open(dir.path().join("u.db")).unwrap();
        let rec = add_manual(
            &db,
            UsageRecordInput {
                task_id: None,
                project_id: None,
                model: Some("unknown".into()),
                occurred_at: None,
                input_tokens: None,
                output_tokens: None,
                cached_tokens: None,
                total_tokens: None,
                reported_cost_minor: None,
                currency: "USD".into(),
                billing_source: "manual".into(),
                verification_status: "unavailable".into(),
                notes: Some("no token data".into()),
                external_key: None,
            },
        )
        .unwrap();
        assert_eq!(rec.total_tokens, None);
        assert_eq!(rec.verification_status, "unavailable");
    }

    #[test]
    fn csv_import_is_idempotent() {
        let dir = tempdir().unwrap();
        let db = Db::open(dir.path().join("c.db")).unwrap();
        let csv = "id,model,tokens,cost\nr1,gpt,1000,1.50\n";
        let a = import_csv(&db, "a.csv", csv).unwrap();
        assert_eq!(a.inserted, 1);
        let b = import_csv(&db, "a.csv", csv).unwrap();
        assert!(b.skipped_duplicate_import);
        assert_eq!(b.inserted, 0);
    }
}
