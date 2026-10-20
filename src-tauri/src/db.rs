use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct Db {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path.as_ref())?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;",
        )?;
        let db = Self {
            conn: Mutex::new(conn),
            path: path.as_ref().to_path_buf(),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn with_conn<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&Connection) -> AppResult<T>,
    {
        let conn = self.conn.lock().map_err(|_| AppError::Message("db lock".into()))?;
        f(&conn)
    }

    pub fn with_conn_mut<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&mut Connection) -> AppResult<T>,
    {
        let mut conn = self.conn.lock().map_err(|_| AppError::Message("db lock".into()))?;
        f(&mut conn)
    }

    fn migrate(&self) -> AppResult<()> {
        self.with_conn(|conn| {
            conn.execute_batch(include_str!("../migrations/001_initial.sql"))?;
            let applied: Option<i64> = conn
                .query_row(
                    "SELECT version FROM schema_migrations WHERE version = 1",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            if applied.is_none() {
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (1, ?1)",
                    [chrono::Utc::now().to_rfc3339()],
                )?;
                seed_defaults(conn)?;
            }
            Ok(())
        })
    }
}

fn seed_defaults(conn: &Connection) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let defaults = [
        ("default_currency", "USD"),
        ("monthly_salary_minor", "0"),
        ("monthly_hours", "160"),
        ("idle_threshold_minutes", "5"),
        ("agent_tracking_enabled", "true"),
        ("store_prompt_text", "false"),
        ("require_attribution_review", "true"),
        ("ingest_port", "17321"),
        ("ingest_token", ""),
        ("theme", "dark"),
    ];
    for (k, v) in defaults {
        conn.execute(
            "INSERT OR IGNORE INTO application_settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            (k, v, &now),
        )?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO integration_status (id, integration, status, detail, updated_at)
         VALUES ('cursor_hooks', 'cursor_hooks', 'not_installed', 'Hooks not installed', ?1)",
        [&now],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn migrates_and_opens() {
        let dir = tempdir().unwrap();
        let db = Db::open(dir.path().join("t.db")).unwrap();
        db.with_conn(|c| {
            let n: i64 = c
                .query_row("SELECT COUNT(*) FROM application_settings", [], |r| r.get(0))
                .unwrap();
            assert!(n >= 8);
            Ok(())
        })
        .unwrap();
    }
}
