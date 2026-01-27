//! SQLite implementation of the StateStore port.

use crate::ports::{StateStore, ViewedFile};
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SqliteStateStore {
    conn: Mutex<Connection>,
}

impl SqliteStateStore {
    /// Create a new SQLite state store.
    /// Database is stored in the user's config directory.
    pub fn new() -> Result<Self> {
        let db_path = Self::db_path()?;

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create config directory")?;
        }

        let conn = Connection::open(&db_path)
            .context("Failed to open SQLite database")?;

        // Initialize schema
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS viewed_files (
                id INTEGER PRIMARY KEY,
                repo_path TEXT NOT NULL,
                branch TEXT NOT NULL,
                file_path TEXT NOT NULL,
                viewed_at INTEGER NOT NULL,
                UNIQUE(repo_path, branch, file_path)
            );
            CREATE INDEX IF NOT EXISTS idx_viewed_files_repo_branch
                ON viewed_files(repo_path, branch);
            "
        ).context("Failed to initialize database schema")?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Get the database file path.
    fn db_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?;
        Ok(config_dir.join("rev").join("state.db"))
    }

    /// Get current timestamp in milliseconds.
    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}

impl StateStore for SqliteStateStore {
    fn mark_viewed(&self, repo_path: &str, branch: &str, file_path: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO viewed_files (repo_path, branch, file_path, viewed_at)
             VALUES (?1, ?2, ?3, ?4)",
            (repo_path, branch, file_path, Self::now_ms()),
        )?;
        Ok(())
    }

    fn unmark_viewed(&self, repo_path: &str, branch: &str, file_path: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM viewed_files WHERE repo_path = ?1 AND branch = ?2 AND file_path = ?3",
            (repo_path, branch, file_path),
        )?;
        Ok(())
    }

    fn get_viewed_files(&self, repo_path: &str, branch: &str) -> Result<Vec<ViewedFile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT file_path, viewed_at FROM viewed_files
             WHERE repo_path = ?1 AND branch = ?2"
        )?;

        let files = stmt.query_map((repo_path, branch), |row| {
            Ok(ViewedFile {
                file_path: row.get(0)?,
                viewed_at: row.get(1)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(files)
    }

    fn get_viewed_at(&self, repo_path: &str, branch: &str, file_path: &str) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT viewed_at FROM viewed_files
             WHERE repo_path = ?1 AND branch = ?2 AND file_path = ?3"
        )?;

        let result = stmt.query_row((repo_path, branch, file_path), |row| {
            row.get(0)
        });

        match result {
            Ok(ts) => Ok(Some(ts)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn clear_viewed(&self, repo_path: &str, branch: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM viewed_files WHERE repo_path = ?1 AND branch = ?2",
            (repo_path, branch),
        )?;
        Ok(())
    }
}
