//! Clockify TUI library.

pub mod config;
pub mod domain;
pub mod storage;

pub use config::AppConfig;
pub use domain::{
    format_duration_hms, format_duration_human, DomainError, EntryMode, PomodoroPhase,
    PomodoroStateMachine, Project, Tag, TimeEntry, DEFAULT_PROJECT_COLOR,
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
