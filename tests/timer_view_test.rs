//! Integration tests for Tab 1 - Timer & Pomodoro screen.

use chrono::{Duration, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use clockify_tui::config::AppConfig;
use clockify_tui::domain::{EntryMode, PomodoroPhase, Project, Tag, TimeEntry};
use clockify_tui::storage::Database;
use clockify_tui::tui::app::App;
use clockify_tui::tui::views::timer::{parse_hex_color, render_timer_view, timer_view};
use clockify_tui::tui::widgets::BigClock;

#[test]
fn test_big_clock_formatting_various_durations() {
    // Seconds duration (< 1 minute)
    let d_seconds = Duration::seconds(45);
    assert_eq!(BigClock::format_duration(&d_seconds), "00:00:45");

    // Minutes + seconds duration
    let d_mins = Duration::minutes(24) + Duration::seconds(18);
    assert_eq!(BigClock::format_duration(&d_mins), "00:24:18");

    // Hours + minutes + seconds duration
    let d_hours = Duration::hours(2) + Duration::minutes(15) + Duration::seconds(30);
    assert_eq!(BigClock::format_duration(&d_hours), "02:15:30");

    // Big clock render_lines returns 3 rows of equal display width
    let lines = BigClock::render_lines("00:24:18");
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].chars().count(), lines[1].chars().count());
    assert_eq!(lines[1].chars().count(), lines[2].chars().count());
    assert!(!lines[0].is_empty());

    // Verify block glyphs are present in the rendered lines
    assert!(lines[0].contains('█') || lines[0].contains('▀') || lines[0].contains('▄'));
}

#[test]
fn test_big_clock_narrow_fallback_and_wide_rendering() {
    // 1. Narrow terminal area (< 45 cols) falls back to styled single-line text
    let backend_narrow = TestBackend::new(35, 5);
    let mut term_narrow = Terminal::new(backend_narrow).unwrap();
    let clock = BigClock::new("00:24:18");

    term_narrow
        .draw(|f| {
            f.render_widget(clock.clone(), f.area());
        })
        .unwrap();

    let content_narrow: String = term_narrow
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        content_narrow.contains("00:24:18"),
        "Narrow buffer should display single-line time string fallback"
    );

    // 2. Wide terminal area (>= 45 cols, >= 6 rows) renders 3-row block glyphs
    let backend_wide = TestBackend::new(60, 10);
    let mut term_wide = Terminal::new(backend_wide).unwrap();

    term_wide
        .draw(|f| {
            f.render_widget(clock, f.area());
        })
        .unwrap();

    let content_wide: String = term_wide
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        content_wide.contains('█') || content_wide.contains('▀') || content_wide.contains('▄'),
        "Wide buffer should display block glyph characters"
    );
}

#[test]
fn test_starting_timer_via_space_key_updates_app_and_database(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut db = Database::open_in_memory()?;
    let mut app = App::new(AppConfig::default());

    // Initially idle
    assert!(app.active_entry.is_none());
    assert!(db.get_active_entry()?.is_none());

    // Press Space key to start timer
    let space_key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    app.handle_key_with_db(space_key, &mut db);

    // App state updated
    assert!(app.active_entry.is_some());
    let active = app.active_entry.as_ref().unwrap();
    assert_eq!(active.entry_mode, EntryMode::Stopwatch);
    assert_eq!(active.description, "Study Session");

    // SQLite database updated
    let db_active = db.get_active_entry()?;
    assert!(db_active.is_some());
    assert_eq!(db_active.unwrap().description, "Study Session");

    Ok(())
}

#[test]
fn test_stopping_timer_via_space_key_updates_database() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = Database::open_in_memory()?;
    let mut app = App::new(AppConfig::default());

    let space_key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);

    // 1. Start timer
    app.handle_key_with_db(space_key, &mut db);
    assert!(app.active_entry.is_some());
    assert!(db.get_active_entry()?.is_some());

    // 2. Stop timer
    app.handle_key_with_db(space_key, &mut db);

    // App state is now idle
    assert!(app.active_entry.is_none());
    assert!(app.active_project_name.is_none());

    // SQLite active entry is cleared
    assert!(db.get_active_entry()?.is_none());

    // Today's entries now contains the completed entry
    assert_eq!(app.today_entries.len(), 1);
    assert!(app.today_entries[0].end_time.is_some());

    Ok(())
}

#[test]
fn test_toggling_pomodoro_mode_changes_entry_mode_and_phase(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut db = Database::open_in_memory()?;
    let mut app = App::new(AppConfig::default());

    // Initially Work(1)
    assert_eq!(app.pomodoro.current_phase(), PomodoroPhase::Work(1));

    // Start timer via Space (Stopwatch mode)
    let space_key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    app.handle_key_with_db(space_key, &mut db);
    assert_eq!(
        app.active_entry.as_ref().unwrap().entry_mode,
        EntryMode::Stopwatch
    );

    // Press 'p' to switch Stopwatch -> PomodoroWork
    let p_key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
    app.handle_key_with_db(p_key, &mut db);

    assert_eq!(
        app.active_entry.as_ref().unwrap().entry_mode,
        EntryMode::PomodoroWork
    );
    assert_eq!(app.pomodoro.current_phase(), PomodoroPhase::Work(1));
    let db_active1 = db.get_active_entry()?.unwrap();
    assert_eq!(db_active1.entry_mode, EntryMode::PomodoroWork);

    // Press 'p' again to advance Pomodoro phase Work(1) -> ShortBreak(1)
    app.handle_key_with_db(p_key, &mut db);

    assert_eq!(
        app.active_entry.as_ref().unwrap().entry_mode,
        EntryMode::PomodoroBreak
    );
    assert_eq!(app.pomodoro.current_phase(), PomodoroPhase::ShortBreak(1));
    let db_active2 = db.get_active_entry()?.unwrap();
    assert_eq!(db_active2.entry_mode, EntryMode::PomodoroBreak);

    Ok(())
}

#[test]
fn test_repeat_selected_entry_copies_details_to_new_timer() -> Result<(), Box<dyn std::error::Error>>
{
    let mut db = Database::open_in_memory()?;

    // Create a project and tag in SQLite
    let project = db.create_project(&Project::new("Algorithms")?.with_color("#2ecc71"))?;
    let tag = db.create_tag(&Tag::new("GraphTheory")?)?;

    // Create a completed session for today
    let start = Utc::now() - Duration::hours(3);
    let end = Utc::now() - Duration::hours(2);
    let mut completed_entry = TimeEntry::new("Dijkstra Algorithm Homework", start);
    completed_entry.end_time = Some(end);
    completed_entry.project_id = project.id;
    completed_entry.tags = vec![tag.clone()];
    db.create_manual_entry(&completed_entry)?;

    // Initialize app and load today's entries
    let mut app = App::new(AppConfig::default());
    app.refresh_today_entries(&db);

    assert_eq!(app.today_entries.len(), 1);
    assert_eq!(app.selected_recent_index, 0);
    assert!(app.active_entry.is_none());

    // Press 'r' to repeat selected entry
    let r_key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
    app.handle_key_with_db(r_key, &mut db);

    // Verify active entry was started with target's description, project, and tags
    assert!(app.active_entry.is_some());
    let active = app.active_entry.as_ref().unwrap();
    assert_eq!(active.description, "Dijkstra Algorithm Homework");
    assert_eq!(active.project_id, project.id);
    assert_eq!(active.tags.len(), 1);
    assert_eq!(active.tags[0].name, "GraphTheory");
    assert_eq!(app.active_project_name.as_deref(), Some("Algorithms"));

    // Verify written to SQLite
    let db_active = db.get_active_entry()?.unwrap();
    assert_eq!(db_active.description, "Dijkstra Algorithm Homework");
    assert_eq!(db_active.project_id, project.id);
    assert_eq!(db_active.tags.len(), 1);
    assert_eq!(db_active.tags[0].name, "GraphTheory");

    Ok(())
}

#[test]
fn test_render_timer_view_in_both_running_and_idle_states() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    // 1. Idle state rendering
    let app_idle = App::new(AppConfig::default());
    terminal
        .draw(|f| render_timer_view(&app_idle, f, f.area()))
        .unwrap();

    let content_idle: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(content_idle.contains("IDLE") || content_idle.contains("Active Timer"));
    assert!(content_idle.contains("[Space]"));

    // 2. Running Stopwatch state rendering
    let mut app_running = App::new(AppConfig::default());
    let mut entry = TimeEntry::new(
        "Distributed Systems Lab",
        Utc::now() - Duration::minutes(20),
    );
    entry.entry_mode = EntryMode::Stopwatch;
    app_running.set_active_entry(Some(entry), Some("Computer Science".to_string()));

    terminal
        .draw(|f| timer_view(&app_running, f, f.area()))
        .unwrap();

    let content_running: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(content_running.contains("Distributed Systems Lab"));
    assert!(content_running.contains("Computer Science"));

    // 3. Running Pomodoro state rendering
    let mut app_pomodoro = App::new(AppConfig::default());
    let mut pomo_entry = TimeEntry::new("Calculus Review", Utc::now() - Duration::minutes(10));
    pomo_entry.entry_mode = EntryMode::PomodoroWork;
    app_pomodoro.set_active_entry(Some(pomo_entry), Some("Mathematics".to_string()));

    terminal
        .draw(|f| render_timer_view(&app_pomodoro, f, f.area()))
        .unwrap();

    let content_pomodoro: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(content_pomodoro.contains("Focus Session"));
    assert!(content_pomodoro.contains("Mathematics"));
    assert!(content_pomodoro.contains('%'));

    // 4. Test rendering on standard 80x24 dimensions
    let backend_80x24 = TestBackend::new(80, 24);
    let mut terminal_80x24 = Terminal::new(backend_80x24).unwrap();

    terminal_80x24
        .draw(|f| render_timer_view(&app_pomodoro, f, f.area()))
        .unwrap();
    let content_80x24: String = terminal_80x24
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(content_80x24.contains("Calculus Review"));
}

#[test]
fn test_recent_sessions_list_navigation() {
    let mut app = App::new(AppConfig::default());
    let e1 = TimeEntry::new("Session 1", Utc::now() - Duration::hours(3));
    let e2 = TimeEntry::new("Session 2", Utc::now() - Duration::hours(2));
    let e3 = TimeEntry::new("Session 3", Utc::now() - Duration::hours(1));
    app.today_entries = vec![e1, e2, e3];

    assert_eq!(app.selected_recent_index, 0);

    // j / Down navigates down
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.selected_recent_index, 1);

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selected_recent_index, 2);

    // Clamped at bottom
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selected_recent_index, 2);

    // k / Up navigates up
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
    assert_eq!(app.selected_recent_index, 1);

    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.selected_recent_index, 0);

    // Clamped at top
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.selected_recent_index, 0);
}

#[test]
fn test_parse_hex_color_utility() {
    use ratatui::style::Color;

    assert_eq!(parse_hex_color("#3498db"), Color::Rgb(0x34, 0x98, 0xdb));
    assert_eq!(parse_hex_color("e74c3c"), Color::Rgb(0xe7, 0x4c, 0x3c));
    assert_eq!(parse_hex_color("#000000"), Color::Rgb(0, 0, 0));
    assert_eq!(parse_hex_color("#ffffff"), Color::Rgb(255, 255, 255));

    // Fallback for invalid hex strings
    assert_eq!(parse_hex_color("invalid"), Color::Cyan);
    assert_eq!(parse_hex_color(""), Color::Cyan);
}
