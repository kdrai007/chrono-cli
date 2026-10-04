use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::DomainError;

/// Default color assigned to projects when none is specified.
pub const DEFAULT_PROJECT_COLOR: &str = "#3498db";

/// Represents a project or academic course for grouping time entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    /// Unique identifier in the local SQLite database.
    pub id: Option<i64>,
    /// Display name of the project.
    pub name: String,
    /// Hex color code for UI badges (e.g. `#3498db`).
    pub color: String,
    /// Weekly target study hours.
    pub target_hours_week: f64,
    /// Remote Clockify project ID if synced.
    pub clockify_id: Option<String>,
    /// Whether this project is archived.
    pub archived: bool,
    /// Timestamp when this project was created.
    pub created_at: DateTime<Utc>,
}

impl Project {
    /// Creates a new project with default settings.
    pub fn new(name: impl Into<String>) -> Result<Self, DomainError> {
        let name_str = name.into();
        if name_str.trim().is_empty() {
            return Err(DomainError::EmptyName("Project name"));
        }

        let project = Self {
            id: None,
            name: name_str,
            color: DEFAULT_PROJECT_COLOR.to_string(),
            target_hours_week: 0.0,
            clockify_id: None,
            archived: false,
            created_at: Utc::now(),
        };

        Ok(project)
    }

    /// Sets the local ID for this project.
    pub fn with_id(mut self, id: i64) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the badge color for this project.
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        let c = color.into();
        if c.trim().is_empty() {
            self.color = DEFAULT_PROJECT_COLOR.to_string();
        } else {
            self.color = c;
        }
        self
    }

    /// Sets the weekly study target hours.
    pub fn with_target_hours_week(mut self, target_hours: f64) -> Self {
        self.target_hours_week = target_hours;
        self
    }

    /// Sets the remote Clockify project ID.
    pub fn with_clockify_id(mut self, clockify_id: impl Into<String>) -> Self {
        self.clockify_id = Some(clockify_id.into());
        self
    }

    /// Sets the archived status.
    pub fn with_archived(mut self, archived: bool) -> Self {
        self.archived = archived;
        self
    }

    /// Sets the creation timestamp.
    pub fn with_created_at(mut self, created_at: DateTime<Utc>) -> Self {
        self.created_at = created_at;
        self
    }

    /// Validates the project fields.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.name.trim().is_empty() {
            return Err(DomainError::EmptyName("Project name"));
        }
        if self.target_hours_week.is_nan() || self.target_hours_week < 0.0 {
            return Err(DomainError::ValidationError(
                "Weekly target hours cannot be negative or NaN".to_string(),
            ));
        }
        Ok(())
    }
}
