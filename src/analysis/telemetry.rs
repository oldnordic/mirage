//! Opt-in telemetry for mirage CLI usage tracking.
//!
//! Disabled by default. Enable via:
//! - `--record` CLI flag
//! - `MIRAGE_TELEMETRY=1` environment variable
//!
//! Writes to `~/.magellan/mirage-telemetry.db` (local only, no network).

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;

const DB_DIR: &str = ".magellan";
const DB_NAME: &str = "mirage-telemetry.db";

pub struct TelemetryGuard {
    conn: Option<Mutex<Connection>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TelemetrySummary {
    pub total_commands: i64,
    pub by_command: Vec<(String, i64)>,
}

impl TelemetryGuard {
    pub fn new(enabled: bool) -> Result<Self> {
        if !enabled {
            return Ok(Self { conn: None });
        }

        let db_path = telemetry_db_path()?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS telemetry (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command TEXT NOT NULL,
                args_summary TEXT,
                timestamp TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_telemetry_command ON telemetry(command);
            CREATE INDEX IF NOT EXISTS idx_telemetry_ts ON telemetry(timestamp);",
        )?;

        Ok(Self {
            conn: Some(Mutex::new(conn)),
        })
    }

    pub fn record(&self, command: &str, args_summary: Option<&str>) {
        if let Some(conn_mutex) = &self.conn {
            if let Ok(conn) = conn_mutex.lock() {
                let now = chrono::Utc::now().to_rfc3339();
                let summary = args_summary.unwrap_or("");
                let _ = conn.execute(
                    "INSERT INTO telemetry (command, args_summary, timestamp) VALUES (?1, ?2, ?3)",
                    rusqlite::params![command, summary, now],
                );
            }
        }
    }

    pub fn summary(&self) -> Result<TelemetrySummary> {
        let db_path = telemetry_db_path()?;
        if !db_path.exists() {
            return Ok(TelemetrySummary {
                total_commands: 0,
                by_command: vec![],
            });
        }

        let conn = Connection::open(&db_path)?;

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM telemetry", [], |row| row.get(0))?;

        let mut stmt = conn.prepare(
            "SELECT command, COUNT(*) as cnt FROM telemetry GROUP BY command ORDER BY cnt DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let cmd: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            Ok((cmd, cnt))
        })?;

        let mut by_command = Vec::new();
        for row in rows {
            by_command.push(row?);
        }

        Ok(TelemetrySummary {
            total_commands: total,
            by_command,
        })
    }
}

pub fn is_telemetry_enabled(flag: bool) -> bool {
    if flag {
        return true;
    }
    std::env::var("MIRAGE_TELEMETRY").ok().as_deref() == Some("1")
}

fn telemetry_db_path() -> Result<PathBuf> {
    let home = dirs_home()?;
    Ok(home.join(DB_DIR).join(DB_NAME))
}

fn dirs_home() -> Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("USERPROFILE").map(PathBuf::from))
        .map_err(|_| anyhow::anyhow!("Cannot determine home directory"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_disabled() {
        let guard = TelemetryGuard::new(false).unwrap();
        guard.record("test", None);
        assert!(guard.conn.is_none());
    }

    #[test]
    fn test_telemetry_enabled() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test-telemetry.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS telemetry (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command TEXT NOT NULL,
                args_summary TEXT,
                timestamp TEXT NOT NULL
            );",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO telemetry (command, args_summary, timestamp) VALUES ('cfg', NULL, '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM telemetry", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 1);
    }

    #[test]
    fn test_is_telemetry_enabled() {
        assert!(!is_telemetry_enabled(false));
        assert!(is_telemetry_enabled(true));
    }
}
