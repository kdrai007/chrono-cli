use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::tag::Tag;
use super::DomainError;

/// Mode under which a time entry was recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EntryMode {
    /// Standard open-ended stopwatch timer.
    #[default]
    #[serde(alias = "Stopwatch")]
    Stopwatch,
    /// Pomodoro work interval.
    #[serde(
        alias = "PomodoroWork",
        alias = "Pomodoro Work",
        alias = "pomodoro_work"
    )]
    PomodoroWork,
    /// Pomodoro break interval (short or long break).
    #[serde(
        alias = "PomodoroBreak",
        alias = "Pomodoro Break",
        alias = "pomodoro_break"
    )]
    PomodoroBreak,
}

impl EntryMode {
    /// Returns the canonical string representation used in database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stopwatch => "stopwatch",
            Self::PomodoroWork => "pomodoro_work",
            Self::PomodoroBreak => "pomodoro_break",
        }
    }
}

impl fmt::Display for EntryMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopwatch => write!(f, "Stopwatch"),
            Self::PomodoroWork => write!(f, "Pomodoro Work"),
            Self::PomodoroBreak => write!(f, "Pomodoro Break"),
        }
    }
}

impl FromStr for EntryMode {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_lowercase().replace('-', "_");
        match normalized.as_str() {
            "stopwatch" => Ok(Self::Stopwatch),
            "pomodoro_work" | "pomodoro work" | "pomodorowork" => Ok(Self::PomodoroWork),
            "pomodoro_break" | "pomodoro break" | "pomodorobreak" => Ok(Self::PomodoroBreak),
            _ => Err(DomainError::ValidationError(format!(
                "Invalid entry mode: '{s}'"
            ))),
        }
    }
}

/// Formats a `Duration` as `HH:MM:SS` (e.g. `01:25:30`).
pub fn format_duration_hms(duration: &Duration) -> String {
    let total_seconds = duration.num_seconds().max(0);
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

/// Formats a `Duration` in human-readable style (e.g. `1h 25m`, `25m`, `45s`).
pub fn format_duration_human(duration: &Duration) -> String {
    let total_seconds = duration.num_seconds().max(0);
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{seconds}s")
    }
}

/// A logged time interval representing study or work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeEntry {
    /// Unique identifier in the local SQLite database.
    pub id: Option<i64>,
    /// Description or task name for this study session.
    pub description: String,
    /// Associated project ID.
    pub project_id: Option<i64>,
    /// Start timestamp (UTC).
    pub start_time: DateTime<Utc>,
    /// End timestamp (UTC). `None` indicates an active running timer.
    pub end_time: Option<DateTime<Utc>>,
    /// Mode used to record this entry.
    pub entry_mode: EntryMode,
    /// Pomodoro session index within cycle (0 if stopwatch).
    pub pomodoro_index: u32,
    /// List of tags associated with this entry.
    pub tags: Vec<Tag>,
    /// Whether this entry has been synced to Clockify.
    pub synced: bool,
    /// Remote Clockify time entry ID if synced.
    pub clockify_id: Option<String>,
    /// Timestamp when this entry was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp when this entry was last updated.
    pub updated_at: DateTime<Utc>,
}

impl TimeEntry {
    /// Creates a new active time entry starting at `start_time`.
    pub fn new(description: impl Into<String>, start_time: DateTime<Utc>) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            description: description.into(),
            project_id: None,
            start_time,
            end_time: None,
            entry_mode: EntryMode::Stopwatch,
            pomodoro_index: 0,
            tags: Vec::new(),
            synced: false,
            clockify_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Sets the local database ID.
    pub fn with_id(mut self, id: i64) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the associated project ID.
    pub fn with_project_id(mut self, project_id: i64) -> Self {
        self.project_id = Some(project_id);
        self
    }

    /// Sets the end time.
    pub fn with_end_time(mut self, end_time: DateTime<Utc>) -> Self {
        self.end_time = Some(end_time);
        self
    }

    /// Sets the entry mode.
    pub fn with_entry_mode(mut self, mode: EntryMode) -> Self {
        self.entry_mode = mode;
        self
    }

    /// Sets the pomodoro session index.
    pub fn with_pomodoro_index(mut self, index: u32) -> Self {
        self.pomodoro_index = index;
        self
    }

    /// Sets the tags associated with this entry.
    pub fn with_tags(mut self, tags: Vec<Tag>) -> Self {
        self.tags = tags;
        self
    }

    /// Sets the sync status.
    pub fn with_synced(mut self, synced: bool) -> Self {
        self.synced = synced;
        self
    }

    /// Sets the remote Clockify entry ID.
    pub fn with_clockify_id(mut self, clockify_id: impl Into<String>) -> Self {
        self.clockify_id = Some(clockify_id.into());
        self
    }

    /// Sets the creation timestamp.
    pub fn with_created_at(mut self, created_at: DateTime<Utc>) -> Self {
        self.created_at = created_at;
        self
    }

    /// Sets the update timestamp.
    pub fn with_updated_at(mut self, updated_at: DateTime<Utc>) -> Self {
        self.updated_at = updated_at;
        self
    }

    /// Returns `true` if this entry is currently running (no end time).
    pub fn is_active(&self) -> bool {
        self.end_time.is_none()
    }

    /// Stops the active entry at `end_time`, validating that `end_time >= start_time`.
    pub fn stop(&mut self, end_time: DateTime<Utc>) -> Result<(), DomainError> {
        if end_time < self.start_time {
            return Err(DomainError::InvalidTimeRange {
                start: self.start_time,
                end: end_time,
            });
        }
        self.end_time = Some(end_time);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Calculates the completed duration. Returns `None` if the entry is still active.
    pub fn duration(&self) -> Option<Duration> {
        self.end_time
            .map(|end| end.signed_duration_since(self.start_time))
    }

    /// Calculates the duration up to `now` for active entries, or the fixed duration for completed entries.
    pub fn duration_until(&self, now: DateTime<Utc>) -> Duration {
        match self.end_time {
            Some(end) => end.signed_duration_since(self.start_time),
            None => now.signed_duration_since(self.start_time),
        }
    }

    /// Formats the fixed duration as `HH:MM:SS` if completed.
    pub fn format_duration(&self) -> Option<String> {
        self.duration().as_ref().map(format_duration_hms)
    }

    /// Formats the elapsed duration as `HH:MM:SS` relative to `now` (or fixed if completed).
    pub fn format_duration_until(&self, now: DateTime<Utc>) -> String {
        format_duration_hms(&self.duration_until(now))
    }

    /// Formats the fixed duration in human-readable style (e.g. `1h 25m`) if completed.
    pub fn format_human_duration(&self) -> Option<String> {
        self.duration().as_ref().map(format_duration_human)
    }

    /// Formats the elapsed duration in human-readable style relative to `now`.
    pub fn format_human_duration_until(&self, now: DateTime<Utc>) -> String {
        format_duration_human(&self.duration_until(now))
    }

    /// Validates that `start_time <= end_time` when `end_time` is present.
    pub fn validate(&self) -> Result<(), DomainError> {
        if let Some(end) = self.end_time {
            if end < self.start_time {
                return Err(DomainError::InvalidTimeRange {
                    start: self.start_time,
                    end,
                });
            }
        }
        Ok(())
    }
}
