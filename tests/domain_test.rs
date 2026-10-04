use chrono::{Duration, TimeZone, Utc};
use chrono_cli::domain::{
    format_duration_hms, format_duration_human, DomainError, EntryMode, PomodoroPhase,
    PomodoroStateMachine, Project, Tag, TimeEntry, DEFAULT_PROJECT_COLOR,
};
use std::str::FromStr;

#[test]
fn test_project_creation_and_defaults() {
    let project = Project::new("CS101 - Algorithms").expect("Project creation should succeed");

    assert_eq!(project.name, "CS101 - Algorithms");
    assert_eq!(project.color, DEFAULT_PROJECT_COLOR);
    assert_eq!(project.color, "#3498db");
    assert_eq!(project.target_hours_week, 0.0);
    assert!(project.id.is_none());
    assert!(project.clockify_id.is_none());
    assert!(!project.archived);
    assert!(project.validate().is_ok());
}

#[test]
fn test_project_validation_empty_name() {
    let empty_err = Project::new("").unwrap_err();
    assert!(matches!(
        empty_err,
        DomainError::EmptyName(_) | DomainError::ValidationError(_)
    ));

    let whitespace_err = Project::new("   \t\n  ").unwrap_err();
    assert!(matches!(
        whitespace_err,
        DomainError::EmptyName(_) | DomainError::ValidationError(_)
    ));
}

#[test]
fn test_project_builder_and_custom_values() {
    let project = Project::new("Linear Algebra")
        .unwrap()
        .with_id(42)
        .with_color("#9b59b6")
        .with_target_hours_week(12.5)
        .with_clockify_id("clk_proj_123")
        .with_archived(true);

    assert_eq!(project.id, Some(42));
    assert_eq!(project.name, "Linear Algebra");
    assert_eq!(project.color, "#9b59b6");
    assert_eq!(project.target_hours_week, 12.5);
    assert_eq!(project.clockify_id.as_deref(), Some("clk_proj_123"));
    assert!(project.archived);
}

#[test]
fn test_project_negative_target_hours_validation() {
    let mut project = Project::new("Physics").unwrap();
    project.target_hours_week = -5.0;
    assert!(project.validate().is_err());
}

#[test]
fn test_tag_creation_and_validation() {
    let tag = Tag::new("exam-prep").expect("Tag creation should succeed");
    assert_eq!(tag.name, "exam-prep");
    assert!(tag.id.is_none());
    assert!(tag.clockify_id.is_none());

    let empty_tag = Tag::new("");
    assert!(empty_tag.is_err());

    let whitespace_tag = Tag::new("   ");
    assert!(whitespace_tag.is_err());

    let custom_tag = Tag::new("lecture")
        .unwrap()
        .with_id(7)
        .with_clockify_id("clk_tag_abc");
    assert_eq!(custom_tag.id, Some(7));
    assert_eq!(custom_tag.clockify_id.as_deref(), Some("clk_tag_abc"));
}

#[test]
fn test_entry_mode_display_and_serialization() {
    // Display
    assert_eq!(EntryMode::Stopwatch.to_string(), "Stopwatch");
    assert_eq!(EntryMode::PomodoroWork.to_string(), "Pomodoro Work");
    assert_eq!(EntryMode::PomodoroBreak.to_string(), "Pomodoro Break");

    // as_str
    assert_eq!(EntryMode::Stopwatch.as_str(), "stopwatch");
    assert_eq!(EntryMode::PomodoroWork.as_str(), "pomodoro_work");
    assert_eq!(EntryMode::PomodoroBreak.as_str(), "pomodoro_break");

    // FromStr
    assert_eq!(
        EntryMode::from_str("stopwatch").unwrap(),
        EntryMode::Stopwatch
    );
    assert_eq!(
        EntryMode::from_str("Stopwatch").unwrap(),
        EntryMode::Stopwatch
    );
    assert_eq!(
        EntryMode::from_str("pomodoro_work").unwrap(),
        EntryMode::PomodoroWork
    );
    assert_eq!(
        EntryMode::from_str("Pomodoro Work").unwrap(),
        EntryMode::PomodoroWork
    );
    assert_eq!(
        EntryMode::from_str("pomodoro_break").unwrap(),
        EntryMode::PomodoroBreak
    );
    assert_eq!(
        EntryMode::from_str("Pomodoro Break").unwrap(),
        EntryMode::PomodoroBreak
    );
    assert!(EntryMode::from_str("unknown_mode").is_err());

    // Serde JSON serialization / deserialization
    let json_stopwatch = serde_json::to_string(&EntryMode::Stopwatch).unwrap();
    assert_eq!(json_stopwatch, "\"stopwatch\"");
    let de_stopwatch: EntryMode = serde_json::from_str(&json_stopwatch).unwrap();
    assert_eq!(de_stopwatch, EntryMode::Stopwatch);

    let json_work = serde_json::to_string(&EntryMode::PomodoroWork).unwrap();
    assert_eq!(json_work, "\"pomodoro_work\"");
    let de_work: EntryMode = serde_json::from_str(&json_work).unwrap();
    assert_eq!(de_work, EntryMode::PomodoroWork);

    let json_break = serde_json::to_string(&EntryMode::PomodoroBreak).unwrap();
    assert_eq!(json_break, "\"pomodoro_break\"");
    let de_break: EntryMode = serde_json::from_str(&json_break).unwrap();
    assert_eq!(de_break, EntryMode::PomodoroBreak);
}

#[test]
fn test_time_entry_active_vs_completed() {
    let start = Utc.with_ymd_and_hms(2026, 10, 4, 10, 0, 0).unwrap();
    let mut entry = TimeEntry::new("Study chapter 3", start);

    // Active entry
    assert!(entry.is_active());
    assert_eq!(entry.end_time, None);
    assert_eq!(entry.duration(), None);
    assert_eq!(entry.entry_mode, EntryMode::Stopwatch);
    assert_eq!(entry.pomodoro_index, 0);
    assert!(!entry.synced);

    // Stop entry
    let end = Utc.with_ymd_and_hms(2026, 10, 4, 11, 25, 30).unwrap();
    entry.stop(end).expect("Stopping entry should succeed");

    assert!(!entry.is_active());
    assert_eq!(entry.end_time, Some(end));
    assert_eq!(entry.duration(), Some(Duration::seconds(5130))); // 1h 25m 30s
}

#[test]
fn test_time_entry_duration_until() {
    let start = Utc.with_ymd_and_hms(2026, 10, 4, 10, 0, 0).unwrap();
    let mut entry = TimeEntry::new("Writing code", start);

    // Active entry elapsed duration
    let now = Utc.with_ymd_and_hms(2026, 10, 4, 10, 45, 0).unwrap();
    assert_eq!(entry.duration_until(now), Duration::minutes(45));

    // Completed entry duration fixed
    let end = Utc.with_ymd_and_hms(2026, 10, 4, 10, 30, 0).unwrap();
    entry.stop(end).unwrap();

    let future = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
    // Fixed duration: 30 minutes, regardless of `future`
    assert_eq!(entry.duration_until(future), Duration::minutes(30));
}

#[test]
fn test_duration_formatting_helpers() {
    // 1h 25m 30s = 5130s
    let dur1 = Duration::seconds(5130);
    assert_eq!(format_duration_hms(&dur1), "01:25:30");
    assert_eq!(format_duration_human(&dur1), "1h 25m");

    // 45s
    let dur2 = Duration::seconds(45);
    assert_eq!(format_duration_hms(&dur2), "00:00:45");
    assert_eq!(format_duration_human(&dur2), "45s");

    // 0s
    let dur3 = Duration::seconds(0);
    assert_eq!(format_duration_hms(&dur3), "00:00:00");
    assert_eq!(format_duration_human(&dur3), "0s");

    // Exact 2 hours
    let dur4 = Duration::hours(2);
    assert_eq!(format_duration_hms(&dur4), "02:00:00");
    assert_eq!(format_duration_human(&dur4), "2h");

    // Exact 25 minutes
    let dur5 = Duration::minutes(25);
    assert_eq!(format_duration_hms(&dur5), "00:25:00");
    assert_eq!(format_duration_human(&dur5), "25m");

    // TimeEntry helper methods
    let start = Utc.with_ymd_and_hms(2026, 10, 4, 10, 0, 0).unwrap();
    let mut entry = TimeEntry::new("Test format", start);
    let now = Utc.with_ymd_and_hms(2026, 10, 4, 11, 25, 30).unwrap();

    assert_eq!(entry.format_duration_until(now), "01:25:30");
    assert_eq!(entry.format_human_duration_until(now), "1h 25m");
    assert_eq!(entry.format_duration(), None);

    entry.stop(now).unwrap();
    assert_eq!(entry.format_duration(), Some("01:25:30".to_string()));
    assert_eq!(entry.format_human_duration(), Some("1h 25m".to_string()));
}

#[test]
fn test_time_entry_validation_start_before_end() {
    let start = Utc.with_ymd_and_hms(2026, 10, 4, 10, 0, 0).unwrap();
    let invalid_end = Utc.with_ymd_and_hms(2026, 10, 4, 9, 0, 0).unwrap();

    let mut entry = TimeEntry::new("Invalid entry", start);
    let stop_err = entry.stop(invalid_end);
    assert!(stop_err.is_err());
    assert!(matches!(
        stop_err.unwrap_err(),
        DomainError::InvalidTimeRange { .. }
    ));

    // Direct validate
    entry.end_time = Some(invalid_end);
    assert!(entry.validate().is_err());
}

#[test]
fn test_pomodoro_state_machine_default_cycle() {
    let mut sm = PomodoroStateMachine::default();
    assert_eq!(sm.work_mins(), 25);
    assert_eq!(sm.short_break_mins(), 5);
    assert_eq!(sm.long_break_mins(), 15);
    assert_eq!(sm.sessions_until_long_break(), 4);

    // Initial state: Work(1)
    assert_eq!(sm.current_phase(), PomodoroPhase::Work(1));
    assert!(sm.current_phase().is_work());
    assert!(!sm.current_phase().is_break());

    // Work(1) -> ShortBreak(1)
    assert_eq!(sm.next_phase(), PomodoroPhase::ShortBreak(1));
    assert!(sm.current_phase().is_short_break());
    assert!(sm.current_phase().is_break());

    // ShortBreak(1) -> Work(2)
    assert_eq!(sm.next_phase(), PomodoroPhase::Work(2));

    // Work(2) -> ShortBreak(2)
    assert_eq!(sm.next_phase(), PomodoroPhase::ShortBreak(2));

    // ShortBreak(2) -> Work(3)
    assert_eq!(sm.next_phase(), PomodoroPhase::Work(3));

    // Work(3) -> ShortBreak(3)
    assert_eq!(sm.next_phase(), PomodoroPhase::ShortBreak(3));

    // ShortBreak(3) -> Work(4)
    assert_eq!(sm.next_phase(), PomodoroPhase::Work(4));

    // Work(4) -> LongBreak(4) after 4th work session
    assert_eq!(sm.next_phase(), PomodoroPhase::LongBreak(4));
    assert!(sm.current_phase().is_long_break());
    assert!(sm.current_phase().is_break());

    // LongBreak(4) -> resets cycle to Work(1)
    assert_eq!(sm.next_phase(), PomodoroPhase::Work(1));
}

#[test]
fn test_pomodoro_state_machine_custom_config() {
    // Config: 50m work, 10m short break, 30m long break, 2 sessions until long break
    let mut sm = PomodoroStateMachine::new(50, 10, 30, 2);

    assert_eq!(sm.current_phase(), PomodoroPhase::Work(1));
    assert_eq!(sm.current_phase_duration(), Duration::minutes(50));

    // Work(1) -> ShortBreak(1)
    assert_eq!(sm.next_phase(), PomodoroPhase::ShortBreak(1));
    assert_eq!(sm.current_phase_duration(), Duration::minutes(10));

    // ShortBreak(1) -> Work(2)
    assert_eq!(sm.next_phase(), PomodoroPhase::Work(2));
    assert_eq!(sm.current_phase_duration(), Duration::minutes(50));

    // Work(2) -> LongBreak(2) because sessions_until_long_break = 2
    assert_eq!(sm.next_phase(), PomodoroPhase::LongBreak(2));
    assert_eq!(sm.current_phase_duration(), Duration::minutes(30));

    // LongBreak(2) -> Work(1)
    assert_eq!(sm.next_phase(), PomodoroPhase::Work(1));
}

#[test]
fn test_pomodoro_time_remaining_and_progress() {
    let sm = PomodoroStateMachine::new(25, 5, 15, 4);
    let start = Utc.with_ymd_and_hms(2026, 10, 4, 14, 0, 0).unwrap();

    // At start: 0% progress, 25 minutes remaining
    let now_0 = start;
    assert_eq!(sm.time_remaining(start, now_0), Duration::minutes(25));
    assert!((sm.progress(start, now_0) - 0.0).abs() < f64::EPSILON);

    // Halfway (12m 30s = 750s elapsed)
    let now_half = start + Duration::seconds(750);
    assert_eq!(sm.time_remaining(start, now_half), Duration::seconds(750));
    assert!((sm.progress(start, now_half) - 0.5).abs() < 1e-4);

    // Exactly at end (25 minutes elapsed)
    let now_end = start + Duration::minutes(25);
    assert_eq!(sm.time_remaining(start, now_end), Duration::zero());
    assert!((sm.progress(start, now_end) - 1.0).abs() < f64::EPSILON);

    // Overtime (30 minutes elapsed)
    let now_over = start + Duration::minutes(30);
    assert_eq!(sm.time_remaining(start, now_over), Duration::zero());
    assert!((sm.progress(start, now_over) - 1.0).abs() < f64::EPSILON);
}
