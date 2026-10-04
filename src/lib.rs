//! Clockify TUI library.

pub mod cli;
pub mod clockify;
pub mod config;
pub mod domain;
pub mod notify;
pub mod storage;

pub use cli::{run_cli, Cli, Commands, ExportFormat, ExportRecord, WaybarOutput};
pub use clockify::{
    entry_to_create_request, ClockifyApi, ClockifyClient, ClockifyError, ClockifyProject,
    ClockifyTag, ClockifyUser, ClockifyWorkspace, CreateTimeEntryRequest, SyncEngine, SyncError,
    SyncResult, TimeEntryResponse, TimeInterval,
};
pub use config::AppConfig;
pub use domain::{
    calculate_streaks, format_duration_hms, format_duration_human, DailyStudySummary, DomainError,
    EntryMode, PomodoroPhase, PomodoroStateMachine, PomodoroStats, Project, ProjectTargetProgress,
    StreakStats, SubjectBreakdown, Tag, TimeEntry, DEFAULT_PROJECT_COLOR,
};
pub use notify::{
    MockNotificationSink, NotificationEvent, NotificationService, NotificationSink,
    NotificationUrgency, NotifyError,
};
pub use storage::{Database, Repository, StorageError};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello from clockify-tui!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_run() {
        assert!(run().is_ok());
    }
}
