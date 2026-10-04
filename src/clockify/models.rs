//! Clockify v1 REST API data models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a Clockify user account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyUser {
    /// Remote Clockify user ID.
    pub id: String,
    /// User email address.
    pub email: String,
    /// User display name.
    pub name: String,
    /// Active workspace identifier.
    pub active_workspace: Option<String>,
    /// Default workspace identifier.
    pub default_workspace: Option<String>,
    /// Account status (e.g. "ACTIVE").
    pub status: Option<String>,
}

/// Represents a Clockify workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyWorkspace {
    /// Workspace ID.
    pub id: String,
    /// Workspace name.
    pub name: String,
    /// Workspace hourly rate settings.
    pub hourly_rate: Option<serde_json::Value>,
    /// Workspace logo/image URL.
    pub image_url: Option<String>,
}

/// Represents a project in Clockify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyProject {
    /// Remote Clockify project ID.
    pub id: String,
    /// Name of the project.
    pub name: String,
    /// Associated workspace ID.
    pub workspace_id: Option<String>,
    /// Hex color code (e.g. "#3498db").
    pub color: Option<String>,
    /// Whether the project is archived.
    #[serde(default)]
    pub archived: bool,
    /// Whether the project is billable.
    #[serde(default)]
    pub billable: bool,
    /// Remote client ID if assigned.
    pub client_id: Option<String>,
}

/// Represents a tag in Clockify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockifyTag {
    /// Remote Clockify tag ID.
    pub id: String,
    /// Tag name.
    pub name: String,
    /// Associated workspace ID.
    pub workspace_id: Option<String>,
    /// Whether the tag is archived.
    #[serde(default)]
    pub archived: bool,
}

/// Request payload to create a new time entry in Clockify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTimeEntryRequest {
    /// Start timestamp (UTC).
    pub start: DateTime<Utc>,
    /// End timestamp (UTC). Optional if running timer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<DateTime<Utc>>,
    /// Description of the study session or task.
    pub description: String,
    /// Associated Clockify project ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// List of associated Clockify tag IDs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_ids: Option<Vec<String>>,
    /// Whether this entry is billable. Defaults to false.
    #[serde(default)]
    pub billable: bool,
}

impl CreateTimeEntryRequest {
    /// Creates a new time entry request.
    pub fn new(start: DateTime<Utc>, description: impl Into<String>) -> Self {
        Self {
            start,
            end: None,
            description: description.into(),
            project_id: None,
            tag_ids: None,
            billable: false,
        }
    }

    /// Sets the end timestamp.
    pub fn with_end(mut self, end: DateTime<Utc>) -> Self {
        self.end = Some(end);
        self
    }

    /// Sets the remote project ID.
    pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    /// Sets the remote tag IDs.
    pub fn with_tag_ids(mut self, tag_ids: Vec<String>) -> Self {
        self.tag_ids = Some(tag_ids);
        self
    }

    /// Sets whether the entry is billable.
    pub fn with_billable(mut self, billable: bool) -> Self {
        self.billable = billable;
        self
    }
}

/// Represents the time interval of a time entry response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeInterval {
    /// Start timestamp (UTC).
    pub start: DateTime<Utc>,
    /// End timestamp (UTC) if completed.
    pub end: Option<DateTime<Utc>>,
    /// Duration string in ISO 8601 duration format (e.g. "PT1H30M").
    pub duration: Option<String>,
}

/// Response returned by Clockify upon creating or fetching a time entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntryResponse {
    /// Remote Clockify time entry ID.
    pub id: String,
    /// Task description.
    pub description: Option<String>,
    /// Associated project ID.
    pub project_id: Option<String>,
    /// Associated workspace ID.
    pub workspace_id: Option<String>,
    /// Associated user ID.
    pub user_id: Option<String>,
    /// List of tag IDs.
    pub tag_ids: Option<Vec<String>>,
    /// Whether the entry is billable.
    #[serde(default)]
    pub billable: bool,
    /// Time interval details.
    pub time_interval: Option<TimeInterval>,
}

impl TimeEntryResponse {
    /// Helper to get the start timestamp.
    pub fn start_time(&self) -> Option<DateTime<Utc>> {
        self.time_interval.as_ref().map(|ti| ti.start)
    }

    /// Helper to get the end timestamp.
    pub fn end_time(&self) -> Option<DateTime<Utc>> {
        self.time_interval.as_ref().and_then(|ti| ti.end)
    }
}
