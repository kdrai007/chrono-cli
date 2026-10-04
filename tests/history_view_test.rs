//! Integration tests for Tab 2 - History Timesheet screen and editing modals.

use chrono::{Duration, Local, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use clockify_tui::config::AppConfig;
use clockify_tui::domain::{Project, TimeEntry};
use clockify_tui::storage::Database;
use clockify_tui::tui::app::{App, Tab};
use clockify_tui::tui::ui::render;
use clockify_tui::tui::views::history::{
    group_history_entries, history_view, render_delete_modal, render_entry_form_modal,
    render_filter_modal, render_history_view, DateGroup,
};

#[test]
fn test_date_grouping_today_yesterday_earlier() {
    let now = Utc::now();
    let today = now.with_timezone(&Local).date_naive();
    let yesterday = today - Duration::days(1);
    let earlier = today - Duration::days(5);

    assert_eq!(DateGroup::from_date(today, today), DateGroup::Today);
    assert_eq!(DateGroup::from_date(yesterday, today), DateGroup::Yesterday);
    assert_eq!(DateGroup::from_date(earlier, today), DateGroup::Earlier);

    assert_eq!(DateGroup::Today.label(), "Today");
    assert_eq!(DateGroup::Yesterday.label(), "Yesterday");
    assert_eq!(DateGroup::Earlier.label(), "Earlier");

    assert_eq!(DateGroup::Today.header_text(), "── Today ──");
    assert_eq!(DateGroup::Yesterday.header_text(), "── Yesterday ──");
    assert_eq!(DateGroup::Earlier.header_text(), "── Earlier ──");

    // Grouping a list of entries spanning all 3 sections
    let e_today = TimeEntry::new("Today Entry", now - Duration::hours(1));
    let e_yesterday = TimeEntry::new("Yesterday Entry", now - Duration::hours(25));
    let e_earlier = TimeEntry::new("Earlier Entry", now - Duration::days(5));

    let entries = vec![e_today, e_yesterday, e_earlier];
    let grouped = group_history_entries(&entries, now);

    assert_eq!(grouped.len(), 3);
    assert_eq!(grouped[0].0, DateGroup::Today);
    assert_eq!(grouped[0].1.len(), 1);
    assert_eq!(grouped[0].1[0].0, 0); // Original index 0

    assert_eq!(grouped[1].0, DateGroup::Yesterday);
    assert_eq!(grouped[1].1.len(), 1);
    assert_eq!(grouped[1].1[0].0, 1); // Original index 1

    assert_eq!(grouped[2].0, DateGroup::Earlier);
    assert_eq!(grouped[2].1.len(), 1);
    assert_eq!(grouped[2].1[0].0, 2); // Original index 2
}

#[test]
fn test_history_table_selection_navigation() {
    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::History);

    let e1 = TimeEntry::new("Session 1", Utc::now() - Duration::hours(3));
    let e2 = TimeEntry::new("Session 2", Utc::now() - Duration::hours(2));
    let e3 = TimeEntry::new("Session 3", Utc::now() - Duration::hours(1));
    app.history_entries = vec![e1, e2, e3];

    assert_eq!(app.selected_history_index, 0);

    // select_next_history navigates down
    app.select_next_history();
    assert_eq!(app.selected_history_index, 1);

    app.select_next_history();
    assert_eq!(app.selected_history_index, 2);

    // Clamped at bottom
    app.select_next_history();
    assert_eq!(app.selected_history_index, 2);

    // select_prev_history navigates up
    app.select_prev_history();
    assert_eq!(app.selected_history_index, 1);

    app.select_prev_history();
    assert_eq!(app.selected_history_index, 0);

    // Clamped at top
    app.select_prev_history();
    assert_eq!(app.selected_history_index, 0);

    // Key event navigation (j/k and Down/Up)
    let key_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    let key_k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    let key_up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);

    app.handle_key(key_j);
    assert_eq!(app.selected_history_index, 1);

    app.handle_key(key_down);
    assert_eq!(app.selected_history_index, 2);

    app.handle_key(key_k);
    assert_eq!(app.selected_history_index, 1);

    app.handle_key(key_up);
    assert_eq!(app.selected_history_index, 0);
}

#[test]
fn test_manual_entry_creation_persisting_into_sqlite() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = Database::open_in_memory()?;
    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::History);

    // Open modal
    app.open_new_entry_modal();
    assert!(app.show_new_entry_modal);

    // Populate form data
    app.entry_form.description = "Compiler Design Study".to_string();
    app.entry_form.project = "Computer Science".to_string();
    app.entry_form.duration = "1h 15m".to_string();
    app.entry_form.tags = "llvm, rust, parsing".to_string();

    // Save form
    app.save_entry_form(&mut db).unwrap();

    // Modal closed
    assert!(!app.show_new_entry_modal);

    // App history entries reloaded
    assert_eq!(app.history_entries.len(), 1);
    let saved = &app.history_entries[0];
    assert_eq!(saved.description, "Compiler Design Study");
    assert!(saved.end_time.is_some());
    assert_eq!(saved.duration().unwrap().num_minutes(), 75);

    // Verify persisted directly into SQLite database
    let db_entries = db.get_entries(
        chrono::DateTime::<Utc>::UNIX_EPOCH,
        Utc::now() + Duration::days(100),
    )?;
    assert_eq!(db_entries.len(), 1);
    assert_eq!(db_entries[0].description, "Compiler Design Study");
    assert_eq!(db_entries[0].tags.len(), 3);

    // Verify project created in SQLite
    let proj = db.get_project_by_name("Computer Science")?;
    assert!(proj.is_some());
    assert_eq!(db_entries[0].project_id, proj.unwrap().id);

    Ok(())
}

#[test]
fn test_entry_editing_updating_sqlite() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = Database::open_in_memory()?;

    // Create initial project and entry
    let project = db.create_project(&Project::new("Mathematics")?.with_color("#3498db"))?;
    let start = Utc::now() - Duration::hours(2);
    let mut entry = TimeEntry::new("Calculus Problem Set 1", start);
    entry.end_time = Some(start + Duration::minutes(45));
    entry.project_id = project.id;
    let initial_entry = db.create_manual_entry(&entry)?;

    let mut app = App::new(AppConfig::default());
    app.refresh_history(&db);
    assert_eq!(app.history_entries.len(), 1);
    assert_eq!(app.selected_history_index, 0);

    // Open edit modal
    app.open_edit_entry_modal();
    assert!(app.show_edit_entry_modal);
    assert_eq!(app.entry_form.id, initial_entry.id);
    assert_eq!(app.entry_form.description, "Calculus Problem Set 1");
    assert_eq!(app.entry_form.project, "Mathematics");
    assert_eq!(app.entry_form.duration, "45m");

    // Modify fields
    app.entry_form.description = "Advanced Calculus Proofs".to_string();
    app.entry_form.duration = "2h".to_string();
    app.entry_form.tags = "analysis, proofs".to_string();

    // Save edited form
    app.save_entry_form(&mut db).unwrap();
    assert!(!app.show_edit_entry_modal);

    // Verify in SQLite
    let updated = db.get_entry(initial_entry.id.unwrap())?.unwrap();
    assert_eq!(updated.description, "Advanced Calculus Proofs");
    assert_eq!(updated.duration().unwrap().num_hours(), 2);
    assert_eq!(updated.tags.len(), 2);
    assert!(!updated.synced); // Reset sync flag

    // Verify app state reloaded
    assert_eq!(app.history_entries.len(), 1);
    assert_eq!(
        app.history_entries[0].description,
        "Advanced Calculus Proofs"
    );

    Ok(())
}

#[test]
fn test_deleting_entry_removing_from_sqlite() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = Database::open_in_memory()?;

    // Create 2 entries
    let mut e1 = TimeEntry::new("Session to keep", Utc::now() - Duration::hours(3));
    e1.end_time = Some(Utc::now() - Duration::hours(2));
    db.create_manual_entry(&e1)?;

    let mut e2 = TimeEntry::new("Session to delete", Utc::now() - Duration::hours(1));
    e2.end_time = Some(Utc::now());
    let created2 = db.create_manual_entry(&e2)?;

    let mut app = App::new(AppConfig::default());
    app.refresh_history(&db);
    assert_eq!(app.history_entries.len(), 2);

    // Select the session to delete (index 0 because start_time is latest)
    assert_eq!(app.history_entries[0].id, created2.id);
    app.selected_history_index = 0;

    app.open_delete_entry_modal();
    assert!(app.show_delete_entry_modal);

    // Confirm deletion
    app.delete_selected_history_entry(&mut db);
    assert!(!app.show_delete_entry_modal);

    // Verify SQLite no longer has the entry
    assert!(db.get_entry(created2.id.unwrap())?.is_none());

    // App state reloaded with 1 entry
    assert_eq!(app.history_entries.len(), 1);
    assert_eq!(app.history_entries[0].description, "Session to keep");

    Ok(())
}

#[test]
fn test_rendering_history_view_empty_and_populated() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    // 1. Empty state
    let mut app_empty = App::new(AppConfig::default());
    app_empty.set_tab(Tab::History);

    terminal
        .draw(|f| render_history_view(&app_empty, f, f.area()))
        .unwrap();

    let buffer_empty: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(buffer_empty.contains("Study History & Timesheet"));
    assert!(buffer_empty.contains("No study sessions recorded yet"));
    assert!(buffer_empty.contains("[n]"));
    assert!(buffer_empty.contains("[e]"));
    assert!(buffer_empty.contains("[d]"));

    // 2. Populated state with Today, Yesterday, Earlier entries
    let mut app_populated = App::new(AppConfig::default());
    app_populated.set_tab(Tab::History);

    let now = Utc::now();
    let mut e_today = TimeEntry::new("Modern Physics Lab", now - Duration::hours(2));
    e_today.end_time = Some(now - Duration::hours(1));
    e_today.synced = true;

    let mut e_yesterday = TimeEntry::new("Chemistry Reading", now - Duration::hours(26));
    e_yesterday.end_time = Some(now - Duration::hours(25));

    let mut e_earlier = TimeEntry::new("Literature Review", now - Duration::days(4));
    e_earlier.end_time = Some(now - Duration::days(4) + Duration::minutes(45));

    app_populated.history_entries = vec![e_today, e_yesterday, e_earlier];
    app_populated.selected_history_index = 0;

    terminal
        .draw(|f| history_view(&app_populated, f, f.area()))
        .unwrap();

    let buffer_pop: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    assert!(buffer_pop.contains("Today"));
    assert!(buffer_pop.contains("Yesterday"));
    assert!(buffer_pop.contains("Earlier"));
    assert!(buffer_pop.contains("Modern Physics Lab"));
    assert!(buffer_pop.contains("Chemistry Reading"));
    assert!(buffer_pop.contains("Literature Review"));
    assert!(buffer_pop.contains('✓')); // Sync mark for synced entry
    assert!(buffer_pop.contains("▶")); // Selected row indicator
}

#[test]
fn test_rendering_all_history_modals() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::History);

    // 1. New entry modal
    app.show_new_entry_modal = true;
    terminal
        .draw(|f| render_entry_form_modal(&app, f, true))
        .unwrap();

    let buf_new: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(buf_new.contains("New Manual Time Entry"));
    assert!(buf_new.contains("Description"));
    assert!(buf_new.contains("Project / Subject"));
    assert!(buf_new.contains("Duration"));
    assert!(buf_new.contains("Tags"));
    assert!(buf_new.contains("Save"));
    assert!(buf_new.contains("Cancel"));

    // 2. Edit entry modal
    app.show_new_entry_modal = false;
    app.show_edit_entry_modal = true;
    app.entry_form.description = "Differential Equations".to_string();
    terminal
        .draw(|f| render_entry_form_modal(&app, f, false))
        .unwrap();

    let buf_edit: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(buf_edit.contains("Edit Time Entry"));
    assert!(buf_edit.contains("Differential Equations"));

    // 3. Delete modal
    app.show_edit_entry_modal = false;
    app.show_delete_entry_modal = true;
    let mut e = TimeEntry::new(
        "Linear Algebra Problem Set",
        Utc::now() - Duration::hours(1),
    );
    e.end_time = Some(Utc::now());
    app.history_entries = vec![e];
    app.selected_history_index = 0;

    terminal.draw(|f| render_delete_modal(&app, f)).unwrap();

    let buf_del: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(buf_del.contains("Confirm Deletion"));
    assert!(buf_del.contains("Are you sure you want to delete this entry?"));
    assert!(buf_del.contains("Linear Algebra Problem Set"));
    assert!(buf_del.contains("[y / Enter]"));
    assert!(buf_del.contains("[n / Esc]"));

    // 4. Filter modal
    app.show_delete_entry_modal = false;
    app.show_filter_modal = true;
    app.filter_input = "Algorithms".to_string();

    terminal.draw(|f| render_filter_modal(&app, f)).unwrap();

    let buf_filter: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(buf_filter.contains("Filter History Entries"));
    assert!(buf_filter.contains("Algorithms"));
    assert!(buf_filter.contains("Apply filter"));
}

#[test]
fn test_history_filtering_behavior() {
    let mut app = App::new(AppConfig::default());
    let e1 = TimeEntry::new("Abstract Algebra Homework", Utc::now());
    let e2 = TimeEntry::new("Organic Chemistry Lab", Utc::now());
    let e3 = TimeEntry::new("Linear Algebra Exam Prep", Utc::now());
    app.history_entries = vec![e1, e2, e3];

    // No filter: all 3 returned
    assert_eq!(app.filtered_history_entries().len(), 3);

    // Apply filter "algebra" (case-insensitive)
    app.history_filter = Some("algebra".to_string());
    let filtered = app.filtered_history_entries();
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].description, "Abstract Algebra Homework");
    assert_eq!(filtered[1].description, "Linear Algebra Exam Prep");

    // Clear filter
    app.history_filter = None;
    assert_eq!(app.filtered_history_entries().len(), 3);
}

#[test]
fn test_keyboard_routing_in_history_tab_and_modals() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = Database::open_in_memory()?;
    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::History);

    // Press 'n' to open new entry modal
    let key_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
    app.handle_key(key_n);
    assert!(app.show_new_entry_modal);

    // Type "Algorithms" into active field 0 (description)
    for c in "Algorithms".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    assert_eq!(app.entry_form.description, "Algorithms");

    // Press Tab to switch to field 1 (project)
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.entry_form.active_field, 1);

    // Press Esc to cancel/close modal
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.show_new_entry_modal);

    // Press 'n' again, fill and press Enter with DB to submit
    app.handle_key(key_n);
    assert!(app.show_new_entry_modal);
    app.entry_form.description = "Computer Graphics".to_string();
    app.entry_form.duration = "45m".to_string();

    let key_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.handle_key_with_db(key_enter, &mut db);

    assert!(!app.show_new_entry_modal);
    assert_eq!(app.history_entries.len(), 1);
    assert_eq!(app.history_entries[0].description, "Computer Graphics");

    // Press 'd' to open delete modal
    let key_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
    app.handle_key(key_d);
    assert!(app.show_delete_entry_modal);

    // Press 'n' to cancel delete modal
    app.handle_key(key_n);
    assert!(!app.show_delete_entry_modal);
    assert_eq!(app.history_entries.len(), 1);

    // Press 'd' again and press 'y' with DB to confirm deletion
    app.handle_key(key_d);
    assert!(app.show_delete_entry_modal);

    let key_y = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
    app.handle_key_with_db(key_y, &mut db);

    assert!(!app.show_delete_entry_modal);
    assert!(app.history_entries.is_empty());

    // Press '/' to open filter modal
    let key_slash = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    app.handle_key(key_slash);
    assert!(app.show_filter_modal);

    // Type query and press Enter
    for c in "graph".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    app.handle_key(key_enter);
    assert!(!app.show_filter_modal);
    assert_eq!(app.history_filter.as_deref(), Some("graph"));

    // Press Esc to clear filter
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.history_filter.is_none());

    Ok(())
}

#[test]
fn test_render_root_ui_with_history_tab_and_active_modals() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::History);

    // 1. Render normal History tab
    terminal.draw(|f| render(&app, f)).unwrap();
    let buf: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(buf.contains("Study History & Timesheet"));

    // 2. Render with active delete modal
    app.show_delete_entry_modal = true;
    terminal.draw(|f| render(&app, f)).unwrap();
    let buf_del: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(buf_del.contains("Confirm Deletion"));

    // 3. Render with active new entry modal
    app.show_delete_entry_modal = false;
    app.show_new_entry_modal = true;
    terminal.draw(|f| render(&app, f)).unwrap();
    let buf_new: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(buf_new.contains("New Manual Time Entry"));

    // 4. Render with active filter modal
    app.show_new_entry_modal = false;
    app.show_filter_modal = true;
    terminal.draw(|f| render(&app, f)).unwrap();
    let buf_filt: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(buf_filt.contains("Filter History Entries"));
}
