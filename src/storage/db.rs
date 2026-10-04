use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::schema;
use super::StorageError;

/// SQLite database connection wrapper for Clockify TUI.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens an in-memory SQLite database, enables foreign keys, and applies migrations.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Opens a file-backed SQLite database, enables WAL mode and foreign keys, and applies migrations.
    pub fn open_file<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Returns the default database file path:
    /// `~/.local/share/clockify-tui/clockify.db` (or platform equivalent).
    pub fn default_db_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clockify-tui")
            .join("clockify.db")
    }

    /// Returns a shared reference to the inner SQLite connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Returns a mutable reference to the inner SQLite connection.
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}
