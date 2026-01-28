//! SQLite implementation of the StateStore port.

use crate::domain::{Comment, Reply};
use crate::ports::{NewComment, NewReply, StateStore, ViewedFile};
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

            CREATE TABLE IF NOT EXISTS comments (
                id INTEGER PRIMARY KEY,
                repo_path TEXT NOT NULL,
                branch TEXT NOT NULL,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                body TEXT NOT NULL,
                author TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                resolved INTEGER NOT NULL DEFAULT 0,
                resolved_at INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_comments_repo_branch
                ON comments(repo_path, branch);
            CREATE INDEX IF NOT EXISTS idx_comments_file
                ON comments(repo_path, branch, file_path);

            CREATE TABLE IF NOT EXISTS replies (
                id INTEGER PRIMARY KEY,
                comment_id INTEGER NOT NULL,
                body TEXT NOT NULL,
                author TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (comment_id) REFERENCES comments(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_replies_comment
                ON replies(comment_id);
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

    /// Internal helper to load replies for a comment.
    fn load_replies(conn: &Connection, comment_id: i64) -> Result<Vec<Reply>> {
        let mut stmt = conn.prepare(
            "SELECT id, comment_id, body, author, created_at
             FROM replies
             WHERE comment_id = ?1
             ORDER BY created_at"
        )?;

        let replies = stmt
            .query_map((comment_id,), |row| {
                Ok(Reply {
                    id: row.get(0)?,
                    comment_id: row.get(1)?,
                    body: row.get(2)?,
                    author: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(replies)
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

    // ─── Comment methods ───

    fn add_comment(&self, repo_path: &str, branch: &str, comment: NewComment) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO comments (repo_path, branch, file_path, start_line, end_line, body, author, created_at, resolved)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
            (
                repo_path,
                branch,
                &comment.file_path,
                comment.start_line as i64,
                comment.end_line as i64,
                &comment.body,
                &comment.author,
                Self::now_ms(),
            ),
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_comments(&self, repo_path: &str, branch: &str) -> Result<Vec<Comment>> {
        let conn = self.conn.lock().unwrap();

        // First, collect comment IDs and data
        let mut stmt = conn.prepare(
            "SELECT id, file_path, start_line, end_line, body, author, created_at, resolved, resolved_at
             FROM comments
             WHERE repo_path = ?1 AND branch = ?2
             ORDER BY file_path, start_line"
        )?;

        let mut comments: Vec<Comment> = stmt
            .query_map((repo_path, branch), |row| {
                Ok(Comment {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    start_line: row.get::<_, i64>(2)? as usize,
                    end_line: row.get::<_, i64>(3)? as usize,
                    body: row.get(4)?,
                    author: row.get(5)?,
                    created_at: row.get(6)?,
                    resolved: row.get::<_, i64>(7)? != 0,
                    resolved_at: row.get(8)?,
                    replies: vec![],
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Drop the statement to release the borrow on conn
        drop(stmt);

        // Load replies for each comment
        for comment in &mut comments {
            comment.replies = Self::load_replies(&conn, comment.id)?;
        }

        Ok(comments)
    }

    fn get_comments_for_file(
        &self,
        repo_path: &str,
        branch: &str,
        file_path: &str,
    ) -> Result<Vec<Comment>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, file_path, start_line, end_line, body, author, created_at, resolved, resolved_at
             FROM comments
             WHERE repo_path = ?1 AND branch = ?2 AND file_path = ?3
             ORDER BY start_line"
        )?;

        let mut comments: Vec<Comment> = stmt
            .query_map((repo_path, branch, file_path), |row| {
                Ok(Comment {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    start_line: row.get::<_, i64>(2)? as usize,
                    end_line: row.get::<_, i64>(3)? as usize,
                    body: row.get(4)?,
                    author: row.get(5)?,
                    created_at: row.get(6)?,
                    resolved: row.get::<_, i64>(7)? != 0,
                    resolved_at: row.get(8)?,
                    replies: vec![],
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        // Load replies for each comment
        for comment in &mut comments {
            comment.replies = Self::load_replies(&conn, comment.id)?;
        }

        Ok(comments)
    }

    fn resolve_comment(&self, comment_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE comments SET resolved = 1, resolved_at = ?1 WHERE id = ?2",
            (Self::now_ms(), comment_id),
        )?;
        Ok(())
    }

    fn unresolve_comment(&self, comment_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE comments SET resolved = 0, resolved_at = NULL WHERE id = ?1",
            (comment_id,),
        )?;
        Ok(())
    }

    fn delete_comment(&self, comment_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM comments WHERE id = ?1", (comment_id,))?;
        Ok(())
    }

    fn update_comment(&self, comment_id: i64, body: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE comments SET body = ?1 WHERE id = ?2",
            (body, comment_id),
        )?;
        Ok(())
    }

    // ─── Reply methods ───

    fn add_reply(&self, reply: NewReply) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO replies (comment_id, body, author, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            (reply.comment_id, &reply.body, &reply.author, Self::now_ms()),
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_replies(&self, comment_id: i64) -> Result<Vec<Reply>> {
        let conn = self.conn.lock().unwrap();
        Self::load_replies(&conn, comment_id)
    }

    fn delete_reply(&self, reply_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM replies WHERE id = ?1", (reply_id,))?;
        Ok(())
    }
}
