//! End-to-end integration tests verifying both headless CLI workflows
//! and full interactive TUI application lifecycles across standard 80x24 terminals.

use assert_cmd::Command;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tempfile::tempdir;

use clockify_tui::config::AppConfig;
use clockify_tui::domain::{EntryMode, Project, TimeEntry};
use clockify_tui::storage::Database;
use clockify_tui::tui::app::{App, Tab};
use clockify_tui::tui::ui::render;

// 1. Full Headless CLI Lifecycle Test:
// start with project & tags -> status (plain text & JSON) -> stop -> export (CSV & JSON).
#[test]
fn test_headless_full_lifecycle() {
    let temp_dir = tempdir().expect("create temporary directory");
    let db_path = temp_dir.path().join("integration_cli.db");
    let db_path_str = db_path.to_str().expect("valid path string");

    let run_cli = || {
        let mut cmd = Command::cargo_bin("clockify-tui").expect("clockify-tui binary exists");
        cmd.env("CLOCKIFY_DB_PATH", db_path_str);
        cmd
    };

    // 1. Initial status: Idle
    let output = run_cli().arg("status").assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("[IDLE]") || stdout.contains("No active timer"),
        "Initial status should be idle, got: {stdout}"
    );

    // Initial status --json
    let output = run_cli().args(["status", "--json"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        json.get("class").and_then(|v| v.as_str()),
        Some("idle"),
        "JSON class must be 'idle'"
    );

    // 2. Start a timer with project and tags
    let output = run_cli()
        .args([
            "start",
            "Calculus Homework & Problem Set",
            "-p",
            "Mathematics",
            "-t",
            "Calculus,Homework",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.to_lowercase().contains("started"),
        "Start command should confirm timer started, got: {stdout}"
    );

    // 3. Status text output while running
    let output = run_cli().arg("status").assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("[RUNNING]"),
        "Status should show [RUNNING], got: {stdout}"
    );
    assert!(
        stdout.contains("Calculus Homework & Problem Set"),
        "Status text should display entry description, got: {stdout}"
    );
    assert!(
        stdout.contains("Mathematics"),
        "Status text should display project name, got: {stdout}"
    );
    assert!(
        stdout.contains("Calculus") || stdout.contains("Homework"),
        "Status text should contain tags, got: {stdout}"
    );

    // 4. Status JSON output while running
    let output = run_cli().args(["status", "--json"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(
        json.get("class").and_then(|v| v.as_str()),
        Some("running"),
        "JSON class must be 'running'"
    );
    let tooltip = json.get("tooltip").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        tooltip.contains("Calculus Homework & Problem Set"),
        "Tooltip must include description"
    );
    assert!(
        tooltip.contains("Mathematics"),
        "Tooltip must include project name"
    );

    // 5. Stop running timer
    let output = run_cli().arg("stop").assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.to_lowercase().contains("stopped")
            || stdout.to_lowercase().contains("elapsed")
            || stdout.to_lowercase().contains("duration"),
        "Stop command should confirm timer stopped, got: {stdout}"
    );

    // 6. Status returns to idle
    let output = run_cli().args(["status", "--json"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(json.get("class").and_then(|v| v.as_str()), Some("idle"));

    // 7. Export JSON
    let output = run_cli()
        .args(["export", "--format", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid json array");
    assert!(json.is_array());
    let entries = json.as_array().unwrap();
    assert_eq!(entries.len(), 1, "Expected 1 exported time entry");
    assert_eq!(
        entries[0].get("description").and_then(|v| v.as_str()),
        Some("Calculus Homework & Problem Set")
    );
    assert_eq!(
        entries[0].get("project").and_then(|v| v.as_str()),
        Some("Mathematics")
    );

    // 8. Export CSV
    let output = run_cli()
        .args(["export", "--format", "csv"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let mut reader = csv::Reader::from_reader(stdout.as_bytes());
    let headers = reader.headers().expect("valid CSV headers").clone();
    assert_eq!(
        headers.iter().collect::<Vec<_>>(),
        vec![
            "id",
            "description",
            "project",
            "start_time",
            "end_time",
            "duration_seconds",
            "tags"
        ]
    );
    let records: Vec<_> = reader.records().map(|r| r.unwrap()).collect();
    assert_eq!(records.len(), 1);
    assert_eq!(&records[0][1], "Calculus Homework & Problem Set");
    assert_eq!(&records[0][2], "Mathematics");
}

// 2. Full TUI App Lifecycle Test:
// App start -> switch across all 4 tabs -> start timer -> toggle Pomodoro ->
// manual entry creation in history -> course target progress -> analytics calculation ->
// help modal display -> quit.
#[test]
fn test_tui_app_full_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = Database::open_in_memory()?;
    let mut app = App::new(AppConfig::default());

    // Step 1: Initial state
    assert_eq!(app.current_tab, Tab::Timer);
    assert!(app.running);
    assert!(app.active_entry.is_none());
    assert!(!app.show_help);

    // Step 2: Switch across all 4 tabs using number keys and Tab cyclical keys
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE),
        &mut db,
    );
    assert_eq!(app.current_tab, Tab::History);

    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE),
        &mut db,
    );
    assert_eq!(app.current_tab, Tab::Projects);

    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE),
        &mut db,
    );
    assert_eq!(app.current_tab, Tab::Analytics);

    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE),
        &mut db,
    );
    assert_eq!(app.current_tab, Tab::Timer);

    // Cyclical Tab key forward and backward
    app.handle_key_with_db(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &mut db);
    assert_eq!(app.current_tab, Tab::History);

    app.handle_key_with_db(
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
        &mut db,
    );
    assert_eq!(app.current_tab, Tab::Timer);

    // Step 3: Start timer via Space key on Tab 1
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &mut db,
    );
    assert!(app.active_entry.is_some());
    assert_eq!(
        app.active_entry.as_ref().unwrap().entry_mode,
        EntryMode::Stopwatch
    );
    assert!(db.get_active_entry()?.is_some());

    // Step 4: Toggle Pomodoro mode via 'p' key
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
        &mut db,
    );
    assert_eq!(
        app.active_entry.as_ref().unwrap().entry_mode,
        EntryMode::PomodoroWork
    );

    // Step 5: Stop timer via Space key
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &mut db,
    );
    assert!(app.active_entry.is_none());
    assert!(db.get_active_entry()?.is_none());

    // Step 6: Create a Course Project with weekly study target in Tab 3
    app.set_tab(Tab::Projects);
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        &mut db,
    );
    assert!(app.show_add_project_modal);

    app.project_form.name = "Algorithms & Data Structures".to_string();
    app.project_form.color = "#2ecc71".to_string();
    app.project_form.target_hours = "10.0".to_string();
    app.save_project_form(&mut db).unwrap();
    assert!(!app.show_add_project_modal);

    assert_eq!(app.project_list.len(), 1);
    let proj_id = app.project_list[0].id.unwrap();
    assert_eq!(app.project_list[0].name, "Algorithms & Data Structures");
    assert_eq!(app.project_list[0].target_hours_week, 10.0);

    // Step 7: Manual entry creation in History (Tab 2)
    app.set_tab(Tab::History);
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &mut db,
    );
    assert!(app.show_new_entry_modal);

    app.entry_form.description = "Dynamic Programming Practice".to_string();
    app.entry_form.project = "Algorithms & Data Structures".to_string();
    app.entry_form.duration = "2h 30m".to_string();
    app.entry_form.tags = "dp, algorithms, leetcode".to_string();
    app.save_entry_form(&mut db).unwrap();
    assert!(!app.show_new_entry_modal);

    assert_eq!(app.history_entries.len(), 2); // 1 from stopped timer + 1 manual entry
    let manual_entry = app
        .history_entries
        .iter()
        .find(|e| e.description == "Dynamic Programming Practice")
        .expect("manual entry found in history");
    assert_eq!(manual_entry.duration().unwrap().num_minutes(), 150);

    // Step 8: Course target progress calculation
    app.set_tab(Tab::Projects);
    app.refresh_projects(&db);
    let progress = app
        .project_progress
        .get(&proj_id)
        .expect("project target progress calculated");
    assert_eq!(progress.target_hours(), 10.0);
    // 2.5 hours logged
    assert!((progress.actual_hours() - 2.5).abs() < 0.05);
    assert!((progress.percentage() - 25.0).abs() < 1.0);

    // Step 9: Analytics calculation in Tab 4
    app.set_tab(Tab::Analytics);
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        &mut db,
    );
    assert!(app.streak_stats.is_some());
    let streak = app.streak_stats.as_ref().unwrap();
    assert!(
        streak.current_streak >= 1,
        "Streak should be at least 1 day"
    );

    assert!(!app.daily_summaries.is_empty(), "Daily summaries populated");
    assert!(
        !app.subject_breakdown.is_empty(),
        "Subject breakdown populated"
    );
    let subject = &app.subject_breakdown[0];
    assert_eq!(subject.project_name, "Algorithms & Data Structures");

    // Step 10: Help modal display and dismissal
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
        &mut db,
    );
    assert!(app.show_help);

    app.handle_key_with_db(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut db);
    assert!(!app.show_help);

    // Step 11: Quit application
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        &mut db,
    );
    assert!(!app.running);

    Ok(())
}

// 3. Standard 80x24 Terminal Assertions:
// Verifies that all tabs, views, and modal dialogs render properly without clipping or panic
// on standard 80x24 terminal emulator dimensions.
#[test]
fn test_all_views_and_modals_render_on_80x24_terminal() -> Result<(), Box<dyn std::error::Error>> {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend)?;

    let mut db = Database::open_in_memory()?;
    let mut app = App::new(AppConfig::default());

    // Create a dummy project and entries for realistic rendering
    let mut project = Project::new("Operating Systems")?;
    project.color = "#e74c3c".to_string();
    project.target_hours_week = 8.0;
    let created_project = db.create_project(&project)?;

    let now = chrono::Utc::now();
    let mut entry = TimeEntry::new("Virtual Memory & Paging", now - chrono::Duration::hours(1));
    entry.end_time = Some(now);
    entry.project_id = created_project.id;
    db.create_manual_entry(&entry)?;

    app.refresh_today_entries(&db);
    app.refresh_history(&db);
    app.refresh_projects(&db);
    app.refresh_analytics(&db);

    // 1. Verify rendering for all 4 primary tabs on 80x24
    for tab in Tab::ALL {
        app.set_tab(tab);
        terminal.draw(|f| render(&app, f))?;
        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();

        assert!(content.contains("Clockify TUI"), "Header should render");
        assert!(content.contains("Timer"), "Tab bar should render");
        assert!(content.contains("History"), "Tab bar should render");
        assert!(content.contains("Projects"), "Tab bar should render");
        assert!(content.contains("Analytics"), "Tab bar should render");
    }

    // 2. Verify Stopwatch running state on 80x24
    let mut active_entry = TimeEntry::new("Kernel Synchronization", chrono::Utc::now());
    active_entry.entry_mode = EntryMode::Stopwatch;
    app.set_active_entry(Some(active_entry), Some("Operating Systems".to_string()));
    terminal.draw(|f| render(&app, f))?;
    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(content.contains("[RUNNING:"));

    // 3. Verify Pomodoro running state on 80x24
    let mut pomo_entry = TimeEntry::new("Kernel Synchronization", chrono::Utc::now());
    pomo_entry.entry_mode = EntryMode::PomodoroWork;
    app.set_active_entry(Some(pomo_entry), Some("Operating Systems".to_string()));
    terminal.draw(|f| render(&app, f))?;
    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(content.contains("[POMO:"));

    app.set_active_entry(None, None);

    // 4. Verify Help modal on 80x24
    app.show_help = true;
    terminal.draw(|f| render(&app, f))?;
    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(content.contains("Keyboard Shortcuts Reference"));
    assert!(content.contains("Navigation"));
    assert!(content.contains("Projects"));
    assert!(content.contains("[?] / [Esc]"));
    app.show_help = false;

    // 5. Verify History Modals on 80x24
    app.set_tab(Tab::History);

    // New Entry Modal
    app.open_new_entry_modal();
    assert!(app.show_new_entry_modal);
    terminal.draw(|f| render(&app, f))?;
    app.show_new_entry_modal = false;

    // Edit Entry Modal
    if !app.history_entries.is_empty() {
        app.open_edit_entry_modal();
        assert!(app.show_edit_entry_modal);
        terminal.draw(|f| render(&app, f))?;
        app.show_edit_entry_modal = false;

        // Delete Entry Modal
        app.open_delete_entry_modal();
        assert!(app.show_delete_entry_modal);
        terminal.draw(|f| render(&app, f))?;
        app.show_delete_entry_modal = false;
    }

    // Filter Modal
    app.open_filter_modal();
    assert!(app.show_filter_modal);
    terminal.draw(|f| render(&app, f))?;
    app.show_filter_modal = false;

    // 6. Verify Projects Modals on 80x24
    app.set_tab(Tab::Projects);

    // Add Project Modal
    app.open_add_project_modal();
    assert!(app.show_add_project_modal);
    terminal.draw(|f| render(&app, f))?;
    app.show_add_project_modal = false;

    // Edit Project Modal
    if !app.project_list.is_empty() {
        app.open_edit_project_modal();
        assert!(app.show_edit_project_modal);
        terminal.draw(|f| render(&app, f))?;
        app.show_edit_project_modal = false;

        // Delete Project Modal
        app.open_delete_project_modal();
        assert!(app.show_delete_project_modal);
        terminal.draw(|f| render(&app, f))?;
        app.show_delete_project_modal = false;
    }

    Ok(())
}
