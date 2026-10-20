use crate::error::AppResult;
use mysql::prelude::*;
use mysql::{Pool, PooledConn};
use std::path::PathBuf;

pub const DEFAULT_DATABASE_URL: &str = "mysql://root@127.0.0.1:3306/flowledger";
pub const TEST_DATABASE_URL: &str = "mysql://root@127.0.0.1:3306/flowledger_test";

pub struct Db {
    pool: Pool,
    label: String,
}

impl Db {
    pub fn open(url: &str) -> AppResult<Self> {
        let pool = Pool::new(url)?;
        let db = Self {
            pool,
            label: url.to_string(),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Display label for UI / get_app_info (MySQL URL).
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Compatibility helper — returns a PathBuf built from the connection label.
    pub fn path(&self) -> PathBuf {
        PathBuf::from(&self.label)
    }

    pub fn with_conn<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&mut PooledConn) -> AppResult<T>,
    {
        let mut conn = self.pool.get_conn()?;
        f(&mut conn)
    }

    pub fn with_conn_mut<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&mut PooledConn) -> AppResult<T>,
    {
        self.with_conn(f)
    }

    fn migrate(&self) -> AppResult<()> {
        self.with_conn_mut(|conn| {
            // MySQL may reject multi-statement in one call depending on flags — split statements.
            for stmt in include_str!("../migrations/001_initial_mysql.sql")
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                conn.query_drop(stmt)?;
            }
            let applied: Option<i64> = conn.exec_first(
                "SELECT version FROM schema_migrations WHERE version = ?",
                (1i64,),
            )?;
            if applied.is_none() {
                conn.exec_drop(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?, ?)",
                    (1i64, chrono::Utc::now().to_rfc3339()),
                )?;
                seed_defaults(conn)?;
            }
            crate::services::currency::seed_default_rates(conn)?;
            Ok(())
        })
    }
}

fn seed_defaults(conn: &mut PooledConn) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let defaults = [
        ("default_currency", "USD"),
        ("monthly_salary_minor", "0"),
        ("monthly_hours", "160"),
        ("idle_threshold_minutes", "5"),
        ("agent_tracking_enabled", "true"),
        ("store_prompt_text", "false"),
        ("require_attribution_review", "false"),
        ("ingest_port", "17321"),
        ("ingest_token", ""),
        ("theme", "dark"),
    ];
    for (k, v) in defaults {
        conn.exec_drop(
            "INSERT IGNORE INTO application_settings (`key`, value, updated_at) VALUES (?, ?, ?)",
            (k, v, &now),
        )?;
    }
    conn.exec_drop(
        "INSERT IGNORE INTO integration_status (id, integration, status, detail, updated_at)
         VALUES ('cursor_hooks', 'cursor_hooks', 'not_installed', 'Hooks not installed', ?)",
        (&now,),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_and_opens() {
        let db = open_test_db().unwrap();
        db.with_conn(|c| {
            let n: i64 = c
                .query_first("SELECT COUNT(*) FROM application_settings")?
                .unwrap_or(0);
            assert!(n >= 8);
            Ok(())
        })
        .unwrap();
    }
}

#[cfg(test)]
static TEST_DB_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Test helper: exclusive access to `flowledger_test` (serializes parallel cargo tests).
#[cfg(test)]
pub struct TestDb {
    db: Db,
    _guard: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl std::ops::Deref for TestDb {
    type Target = Db;
    fn deref(&self) -> &Db {
        &self.db
    }
}

#[cfg(test)]
pub fn open_test_db() -> AppResult<TestDb> {
    let guard = TEST_DB_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    ensure_test_database()?;
    let db = Db::open(TEST_DATABASE_URL)?;
    db.reset_for_tests()?;
    Ok(TestDb {
        db,
        _guard: guard,
    })
}

#[cfg(test)]
fn ensure_test_database() -> AppResult<()> {
    let pool = Pool::new("mysql://root@127.0.0.1:3306")?;
    let mut conn = pool.get_conn()?;
    conn.query_drop(
        "CREATE DATABASE IF NOT EXISTS flowledger_test CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
    )?;
    Ok(())
}

#[cfg(test)]
impl Db {
    fn reset_for_tests(&self) -> AppResult<()> {
        const TABLES: &[&str] = &[
            "unassigned_activity",
            "activity_intervals",
            "activity_sessions",
            "task_attributions",
            "ai_usage_records",
            "usage_imports",
            "cursor_events",
            "cursor_conversations",
            "project_expenses",
            "revenue_records",
            "payments",
            "exchange_rates",
            "labor_rate_history",
            "tasks",
            "project_workspaces",
            "projects",
            "integration_status",
            "application_settings",
            "schema_migrations",
        ];
        self.with_conn_mut(|conn| {
            conn.query_drop("SET FOREIGN_KEY_CHECKS = 0")?;
            for table in TABLES {
                conn.query_drop(format!("TRUNCATE TABLE `{table}`"))?;
            }
            conn.query_drop("SET FOREIGN_KEY_CHECKS = 1")?;
            conn.exec_drop(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?, ?)",
                (1i64, chrono::Utc::now().to_rfc3339()),
            )?;
            seed_defaults(conn)?;
            crate::services::currency::seed_default_rates(conn)?;
            Ok(())
        })
    }
}
