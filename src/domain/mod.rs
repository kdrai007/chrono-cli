//! Core domain models and business logic for time tracking, projects, tags, and pomodoro timer.

use std::fmt;

use chrono::{DateTime, Utc};

pub mod entry;
pub mod pomodoro;
pub mod project;
pub mod stats;
pub mod tag;

pub use entry::{format_duration_hms, format_duration_human, EntryMode, TimeEntry};
pub use pomodoro::{PomodoroPhase, PomodoroStateMachine};
pub use project::{Project, DEFAULT_PROJECT_COLOR};
pub use stats::{
    calculate_streaks, DailyStudySummary, PomodoroStats, ProjectTargetProgress, StreakStats,
    SubjectBreakdown,
};
pub use tag::Tag;

/// Errors arising from domain entity validation or state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// Entity name cannot be empty or only whitespace.
    EmptyName(&'static str),
    /// Start time occurs after end time.
    InvalidTimeRange {
        /// Provided start timestamp.
        start: DateTime<Utc>,
        /// Provided end timestamp.
        end: DateTime<Utc>,
    },
    /// Generic domain validation error.
    ValidationError(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName(field) => write!(f, "{field} cannot be empty"),
            Self::InvalidTimeRange { start, end } => {
                write!(
                    f,
                    "Invalid time range: start ({start}) must be before or equal to end ({end})"
                )
            }
            Self::ValidationError(msg) => write!(f, "Validation error: {msg}"),
        }
    }
}

impl std::error::Error for DomainError {}
