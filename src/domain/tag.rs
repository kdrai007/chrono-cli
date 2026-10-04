use serde::{Deserialize, Serialize};

use super::DomainError;

/// Represents a label or category that can be associated with time entries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tag {
    /// Unique identifier in the local SQLite database.
    pub id: Option<i64>,
    /// Name of the tag.
    pub name: String,
    /// Remote Clockify tag ID if synced.
    pub clockify_id: Option<String>,
}

impl Tag {
    /// Creates a new tag with validation.
    pub fn new(name: impl Into<String>) -> Result<Self, DomainError> {
        let name_str = name.into();
        if name_str.trim().is_empty() {
            return Err(DomainError::EmptyName("Tag name"));
        }

        Ok(Self {
            id: None,
            name: name_str,
            clockify_id: None,
        })
    }

    /// Sets the local ID for this tag.
    pub fn with_id(mut self, id: i64) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the remote Clockify tag ID.
    pub fn with_clockify_id(mut self, clockify_id: impl Into<String>) -> Self {
        self.clockify_id = Some(clockify_id.into());
        self
    }

    /// Validates the tag fields.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.name.trim().is_empty() {
            return Err(DomainError::EmptyName("Tag name"));
        }
        Ok(())
    }
}
