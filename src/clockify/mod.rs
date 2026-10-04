//! Clockify REST API client and two-way sync engine.
//!
//! Provides integration with the Clockify v1 REST API, supporting remote entity
//! fetching (workspaces, projects, tags) and pushing completed local time entries.

use std::fmt;

use crate::storage::StorageError;

pub mod client;
pub mod models;
pub mod sync;

pub use client::{ClockifyApi, ClockifyClient, DEFAULT_CLOCKIFY_BASE_URL};
pub use models::{
    ClockifyProject, ClockifyTag, ClockifyUser, ClockifyWorkspace, CreateTimeEntryRequest,
    TimeEntryResponse, TimeInterval,
};
pub use sync::{entry_to_create_request, SyncEngine, SyncResult};

/// Errors encountered during Clockify API client operations.
#[derive(Debug)]
pub enum ClockifyError {
    /// Clockify API key was not provided or is empty.
    MissingApiKey,
    /// Clockify workspace ID was not provided or is empty.
    MissingWorkspace,
    /// Authentication failure (HTTP 401).
    Unauthorized,
    /// Remote resource not found (HTTP 404).
    NotFound(String),
    /// Clockify API error response with status code and message.
    ApiError { status: u16, message: String },
    /// HTTP transport failure.
    Http(String),
    /// JSON serialization or deserialization failure.
    Serialization(String),
    /// Other client error.
    Other(String),
}

impl fmt::Display for ClockifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey => write!(f, "Clockify API key is missing or empty"),
            Self::MissingWorkspace => write!(f, "Clockify workspace ID is missing or empty"),
            Self::Unauthorized => write!(f, "Clockify API unauthorized: invalid API key"),
            Self::NotFound(msg) => write!(f, "Clockify resource not found: {msg}"),
            Self::ApiError { status, message } => {
                write!(f, "Clockify API error (status {status}): {message}")
            }
            Self::Http(err) => write!(f, "HTTP error: {err}"),
            Self::Serialization(err) => write!(f, "Serialization error: {err}"),
            Self::Other(err) => write!(f, "Clockify error: {err}"),
        }
    }
}

impl std::error::Error for ClockifyError {}

/// Errors encountered during database synchronization with Clockify.
#[derive(Debug)]
pub enum SyncError {
    /// Clockify API communication error.
    Clockify(ClockifyError),
    /// Local storage domain or migration error.
    Storage(StorageError),
    /// SQLite database query or execution error.
    Database(rusqlite::Error),
    /// Workspace ID is missing or unconfigured.
    MissingWorkspace,
    /// API key is missing or unconfigured.
    MissingApiKey,
    /// Other synchronization failure.
    Other(String),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clockify(err) => write!(f, "Clockify error during sync: {err}"),
            Self::Storage(err) => write!(f, "Storage error during sync: {err}"),
            Self::Database(err) => write!(f, "Database error during sync: {err}"),
            Self::MissingWorkspace => write!(f, "Cannot sync: workspace ID is not configured"),
            Self::MissingApiKey => write!(f, "Cannot sync: API key is not configured"),
            Self::Other(err) => write!(f, "Sync error: {err}"),
        }
    }
}

impl std::error::Error for SyncError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Clockify(err) => Some(err),
            Self::Storage(err) => Some(err),
            Self::Database(err) => Some(err),
            _ => None,
        }
    }
}

impl From<ClockifyError> for SyncError {
    fn from(err: ClockifyError) -> Self {
        SyncError::Clockify(err)
    }
}

impl From<StorageError> for SyncError {
    fn from(err: StorageError) -> Self {
        SyncError::Storage(err)
    }
}

impl From<rusqlite::Error> for SyncError {
    fn from(err: rusqlite::Error) -> Self {
        SyncError::Database(err)
    }
}
