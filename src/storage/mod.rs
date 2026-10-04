//! Storage module for chrono-cli.
//!
//! Handles SQLite persistence, schema migrations, and domain entity repository operations.

use std::fmt;
use std::io;

use crate::domain::DomainError;

pub mod db;
pub mod repository;
pub mod schema;

pub use db::Database;
pub use repository::Repository;

/// Errors arising from storage and database operations.
#[derive(Debug)]
pub enum StorageError {
    /// SQLite engine or connection error.
    Database(rusqlite::Error),
    /// Schema migration error.
    Migration(String),
    /// Entity not found.
    NotFound(String),
    /// Unique constraint or duplicate record error.
    Duplicate(String),
    /// Domain validation failure.
    Domain(DomainError),
    /// Underlying filesystem I/O error.
    Io(io::Error),
    /// Malformed or unparseable data stored in database.
    InvalidData(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(err) => write!(f, "Database error: {err}"),
            Self::Migration(msg) => write!(f, "Migration error: {msg}"),
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::Duplicate(msg) => write!(f, "Duplicate record: {msg}"),
            Self::Domain(err) => write!(f, "Domain error: {err}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::InvalidData(msg) => write!(f, "Invalid data: {msg}"),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(err) => Some(err),
            Self::Domain(err) => Some(err),
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(err: rusqlite::Error) -> Self {
        match &err {
            rusqlite::Error::SqliteFailure(ffi_err, msg) => {
                if ffi_err.code == rusqlite::ErrorCode::ConstraintViolation {
                    let text = msg.as_deref().unwrap_or("Constraint violation");
                    if text.contains("UNIQUE") {
                        return StorageError::Duplicate(text.to_string());
                    }
                }
                StorageError::Database(err)
            }
            _ => StorageError::Database(err),
        }
    }
}

impl From<DomainError> for StorageError {
    fn from(err: DomainError) -> Self {
        StorageError::Domain(err)
    }
}

impl From<io::Error> for StorageError {
    fn from(err: io::Error) -> Self {
        StorageError::Io(err)
    }
}
