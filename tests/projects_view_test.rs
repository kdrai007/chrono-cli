//! Integration tests for Tab 3 - Projects & Course Targets Screen, progress bars, and modals.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;

use clockify_tui::config::AppConfig;
use clockify_tui::domain::{Project, ProjectTargetProgress};
use clockify_tui::storage::Database;
use clockify_tui::tui::app::{App, Tab};
use clockify_tui::tui::ui::render;
use clockify_tui::tui::views::projects::{
    format_hours_human, format_progress_bar, get_progress_color, get_status_badge, projects_view,
    render_delete_project_modal, render_project_form_modal, render_projects_view,
};

#[test]
fn test_project_listing_and_selection_navigation() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let p1 = db
        .create_project(&Project::new("Algorithms").unwrap().with_color("#e74c3c"))
        .unwrap();
    let p2 = db
        .create_project(
            &Project::new("Operating Systems")
                .unwrap()
                .with_color("#3498db"),
        )
        .unwrap();
    let p3 = db
        .create_project(&Project::new("Databases").unwrap().with_color("#2ecc71"))
        .unwrap();

    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::Projects);
    app.refresh_projects(&db);

    // Database returns projects sorted by name ASC: Algorithms, Databases, Operating Systems
    assert_eq!(app.project_list.len(), 3);
    assert_eq!(app.selected_project_index, 0);
    assert_eq!(app.selected_project().unwrap().id, p1.id);

    // Navigate down: select_next_project
    app.select_next_project();
    assert_eq!(app.selected_project_index, 1);
    assert_eq!(app.selected_project().unwrap().id, p3.id);

    app.select_next_project();
    assert_eq!(app.selected_project_index, 2);
    assert_eq!(app.selected_project().unwrap().id, p2.id);

    // Clamped at bottom
    app.select_next_project();
    assert_eq!(app.selected_project_index, 2);

    // Navigate up: select_prev_project
    app.select_prev_project();
    assert_eq!(app.selected_project_index, 1);

    app.select_prev_project();
    assert_eq!(app.selected_project_index, 0);

    // Clamped at top
    app.select_prev_project();
    assert_eq!(app.selected_project_index, 0);

    // Vim keyboard navigation (j/k and Down/Up)
    let key_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    let key_k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    let key_up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);

    app.handle_key(key_j);
    assert_eq!(app.selected_project_index, 1);

    app.handle_key(key_down);
    assert_eq!(app.selected_project_index, 2);

    app.handle_key(key_k);
    assert_eq!(app.selected_project_index, 1);

    app.handle_key(key_up);
    assert_eq!(app.selected_project_index, 0);
}

#[test]
fn test_add_project_via_modal_form_persisting_sqlite() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");
    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::Projects);

    // Open add modal
    app.open_add_project_modal();
    assert!(app.show_add_project_modal);
    assert!(app.is_modal_open());

    // Fill form
    app.project_form.name = "Distributed Systems".to_string();
    app.project_form.color = "#9b59b6".to_string();
    app.project_form.target_hours = "12.5".to_string();

    // Save form
    let res = app.save_project_form(&mut db);
    assert!(res.is_ok(), "Failed to save project: {:?}", res);

    assert!(!app.show_add_project_modal);
    assert!(!app.is_modal_open());

    // Verify stored in SQLite
    let in_db = db
        .get_project_by_name("Distributed Systems")
        .unwrap()
        .expect("Project not found in DB");
    assert_eq!(in_db.name, "Distributed Systems");
    assert_eq!(in_db.color, "#9b59b6");
    assert!((in_db.target_hours_week - 12.5).abs() < 1e-6);
    assert!(!in_db.archived);

    // Verify app state refreshed
    assert_eq!(app.project_list.len(), 1);
    assert_eq!(app.project_list[0].name, "Distributed Systems");
}

#[test]
fn test_edit_project_weekly_target_and_color_sqlite() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let initial = db
        .create_project(
            &Project::new("Machine Learning")
                .unwrap()
                .with_color("#3498db")
                .with_target_hours_week(8.0),
        )
        .unwrap();

    let mut app = App::new(AppConfig::default());
    app.refresh_projects(&db);

    assert_eq!(app.project_list.len(), 1);

    // Open edit modal
    app.open_edit_project_modal();
    assert!(app.show_edit_project_modal);
    assert_eq!(app.project_form.id, initial.id);
    assert_eq!(app.project_form.name, "Machine Learning");
    assert_eq!(app.project_form.color, "#3498db");
    assert_eq!(app.project_form.target_hours, "8.0");

    // Modify values
    app.project_form.target_hours = "15.0".to_string();
    app.project_form.color = "#e74c3c".to_string();

    let res = app.save_project_form(&mut db);
    assert!(res.is_ok());
    assert!(!app.show_edit_project_modal);

    // Verify updated in SQLite
    let updated = db
        .get_project(initial.id.unwrap())
        .unwrap()
        .expect("Project not found");
    assert_eq!(updated.color, "#e74c3c");
    assert!((updated.target_hours_week - 15.0).abs() < 1e-6);

    // Verify app state
    assert_eq!(app.project_list[0].color, "#e74c3c");
    assert!((app.project_list[0].target_hours_week - 15.0).abs() < 1e-6);
}

#[test]
fn test_archive_and_unarchive_projects() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let p = db
        .create_project(
            &Project::new("Computer Networks")
                .unwrap()
                .with_color("#1abc9c"),
        )
        .unwrap();

    let mut app = App::new(AppConfig::default());
    app.refresh_projects(&db);
    assert!(!app.project_list[0].archived);

    // Toggle archive -> true
    app.toggle_archive_selected_project(&mut db);
    let after_archive = db.get_project(p.id.unwrap()).unwrap().unwrap();
    assert!(after_archive.archived);
    assert!(app.project_list[0].archived);

    // Toggle archive -> false
    app.toggle_archive_selected_project(&mut db);
    let after_unarchive = db.get_project(p.id.unwrap()).unwrap().unwrap();
    assert!(!after_unarchive.archived);
    assert!(!app.project_list[0].archived);
}

#[test]
fn test_delete_project_sqlite() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let p = db
        .create_project(&Project::new("History of Computing").unwrap())
        .unwrap();

    let mut app = App::new(AppConfig::default());
    app.refresh_projects(&db);
    assert_eq!(app.project_list.len(), 1);

    app.open_delete_project_modal();
    assert!(app.show_delete_project_modal);

    app.delete_selected_project(&mut db);
    assert!(!app.show_delete_project_modal);

    // Verify deleted in DB
    let queried = db.get_project(p.id.unwrap()).unwrap();
    assert!(queried.is_none());

    // Verify app state updated
    assert_eq!(app.project_list.len(), 0);
}

#[test]
fn test_progress_bar_calculation_and_rendering() {
    // 0%
    let bar_0 = format_progress_bar(0.0, 10);
    assert_eq!(bar_0, "[░░░░░░░░░░] 0.0%");

    // 50%
    let bar_50 = format_progress_bar(0.5, 10);
    assert_eq!(bar_50, "[█████░░░░░] 50.0%");

    // 62.5%
    let bar_62 = format_progress_bar(0.625, 10);
    assert_eq!(bar_62, "[██████░░░░] 62.5%");

    // 100%
    let bar_100 = format_progress_bar(1.0, 10);
    assert_eq!(bar_100, "[██████████] 100.0%");

    // >100% (e.g. 125%)
    let bar_125 = format_progress_bar(1.25, 10);
    assert_eq!(bar_125, "[██████████] 125.0%");

    // Clamped on negative / NaN
    let bar_neg = format_progress_bar(-0.5, 10);
    assert_eq!(bar_neg, "[░░░░░░░░░░] 0.0%");

    let bar_nan = format_progress_bar(f64::NAN, 10);
    assert_eq!(bar_nan, "[░░░░░░░░░░] 0.0%");

    // Status badges
    assert_eq!(get_status_badge(0.0, 5.0).0, "[No Target]");
    assert_eq!(get_status_badge(10.0, 12.0).0, "[Exceeded]");
    assert_eq!(get_status_badge(10.0, 10.0).0, "[Exceeded]");
    assert_eq!(get_status_badge(10.0, 6.0).0, "[On Track]");
    assert_eq!(get_status_badge(10.0, 5.0).0, "[On Track]");
    assert_eq!(get_status_badge(10.0, 3.0).0, "[Behind]");

    // Progress color
    assert_eq!(get_progress_color(0.0, 5.0), Color::DarkGray);
    assert_eq!(get_progress_color(10.0, 9.0), Color::Green);
    assert_eq!(get_progress_color(10.0, 6.0), Color::Yellow);
    assert_eq!(get_progress_color(10.0, 3.0), Color::Red);

    // Duration formatting
    assert_eq!(format_hours_human(6.25), "6h 15m");
    assert_eq!(format_hours_human(0.0), "0h 00m");
    assert_eq!(format_hours_human(10.0), "10h 00m");
}

#[test]
fn test_render_empty_and_populated_projects_view_and_modals() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::Projects);

    // 1. Render empty projects view
    terminal
        .draw(|f| {
            render_projects_view(&app, f, f.area());
        })
        .unwrap();

    // 2. Render populated projects view
    let p1 = Project::new("Operating Systems")
        .unwrap()
        .with_id(1)
        .with_color("#3498db")
        .with_target_hours_week(10.0);
    let p2 = Project::new("Compilers")
        .unwrap()
        .with_id(2)
        .with_color("#e74c3c")
        .with_target_hours_week(8.0)
        .with_archived(true);

    app.project_list = vec![p1, p2];

    let mut prog = std::collections::HashMap::new();
    prog.insert(
        1,
        ProjectTargetProgress::new(1, "Operating Systems", "#3498db", 10.0, 6.25),
    );
    app.project_progress = prog;

    terminal
        .draw(|f| {
            projects_view(&app, f, f.area());
        })
        .unwrap();

    // 3. Render Add Project modal
    app.open_add_project_modal();
    terminal
        .draw(|f| {
            render_project_form_modal(&app, f, true);
        })
        .unwrap();

    // 4. Render Edit Project modal
    app.open_edit_project_modal();
    terminal
        .draw(|f| {
            render_project_form_modal(&app, f, false);
        })
        .unwrap();

    // 5. Render Delete Project modal
    app.open_delete_project_modal();
    terminal
        .draw(|f| {
            render_delete_project_modal(&app, f);
        })
        .unwrap();

    // 6. Render complete root UI on Tab::Projects with modal open
    terminal
        .draw(|f| {
            render(&app, f);
        })
        .unwrap();

    app.close_modal();
    terminal
        .draw(|f| {
            render(&app, f);
        })
        .unwrap();
}

#[test]
fn test_keyboard_routing_in_projects_tab_and_modals() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");
    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::Projects);

    let p = db
        .create_project(
            &Project::new("Software Engineering")
                .unwrap()
                .with_target_hours_week(10.0),
        )
        .unwrap();
    app.refresh_projects(&db);

    // 'a' opens add modal
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        &mut db,
    );
    assert!(app.show_add_project_modal);

    // Tab navigates form fields
    assert_eq!(app.project_form.active_field, 0);
    app.handle_key_with_db(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &mut db);
    assert_eq!(app.project_form.active_field, 1);
    app.handle_key_with_db(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &mut db);
    assert_eq!(app.project_form.active_field, 2);
    app.handle_key_with_db(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &mut db);
    assert_eq!(app.project_form.active_field, 0);

    // BackTab navigates backwards
    app.handle_key_with_db(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE), &mut db);
    assert_eq!(app.project_form.active_field, 2);

    // Typing into fields
    app.project_form.active_field = 0;
    app.project_form.name.clear();
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('C'), KeyModifiers::NONE),
        &mut db,
    );
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE),
        &mut db,
    );
    assert_eq!(app.project_form.name, "CS");

    // Backspace
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        &mut db,
    );
    assert_eq!(app.project_form.name, "C");

    // Esc closes modal
    app.handle_key_with_db(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut db);
    assert!(!app.show_add_project_modal);

    // 'e' opens edit modal
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        &mut db,
    );
    assert!(app.show_edit_project_modal);
    app.handle_key_with_db(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut db);
    assert!(!app.show_edit_project_modal);

    // 'x' toggles archive
    assert!(!app.project_list[0].archived);
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        &mut db,
    );
    assert!(app.project_list[0].archived);

    // 'd' opens delete modal
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
        &mut db,
    );
    assert!(app.show_delete_project_modal);

    // 'n' cancels delete
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &mut db,
    );
    assert!(!app.show_delete_project_modal);
    assert_eq!(app.project_list.len(), 1);

    // 'd' then 'y' confirms delete
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
        &mut db,
    );
    app.handle_key_with_db(
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        &mut db,
    );
    assert!(!app.show_delete_project_modal);
    assert_eq!(app.project_list.len(), 0);
    assert!(db.get_project(p.id.unwrap()).unwrap().is_none());
}

#[test]
fn test_render_standard_80x24_dimensions_and_delete_modal() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::Projects);

    let mut p = Project::new("Operating Systems").unwrap();
    p.color = "#9b59b6".to_string();
    p.target_hours_week = 8.0;
    app.project_list = vec![p];

    // 1. Base view without modal: verify 80-column responsive header and table
    terminal
        .draw(|f| {
            clockify_tui::tui::ui::render(&app, f);
        })
        .unwrap();

    let buffer_base: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    assert!(buffer_base.contains("Courses:"));
    assert!(buffer_base.contains("Operating Systems"));

    // 2. With delete modal active: verify modal fits and action buttons are visible
    app.show_delete_project_modal = true;
    terminal
        .draw(|f| {
            clockify_tui::tui::ui::render(&app, f);
        })
        .unwrap();

    let buffer_modal: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    assert!(buffer_modal.contains("Confirm Project Deletion"));
    assert!(buffer_modal.contains("[y / Enter]"));
    assert!(buffer_modal.contains("Confirm Delete"));
    assert!(buffer_modal.contains("[n / Esc]"));
}

#[test]
fn test_project_form_validation_errors() {
    let mut db = Database::open_in_memory().unwrap();
    let mut app = App::new(AppConfig::default());

    // 1. Empty name
    app.project_form.name = "   ".to_string();
    let err = app.save_project_form(&mut db).unwrap_err();
    assert!(err.contains("cannot be empty"));

    // 2. Invalid hex color
    app.project_form.name = "Robotics".to_string();
    app.project_form.color = "#invalid".to_string();
    let err = app.save_project_form(&mut db).unwrap_err();
    assert!(err.contains("valid 6-character hex"));

    // 3. Negative target hours
    app.project_form.color = "#3498db".to_string();
    app.project_form.target_hours = "-5.0".to_string();
    let err = app.save_project_form(&mut db).unwrap_err();
    assert!(err.contains("finite, non-negative"));

    // 4. Infinite target hours
    app.project_form.target_hours = "inf".to_string();
    let err = app.save_project_form(&mut db).unwrap_err();
    assert!(err.contains("finite, non-negative"));
}

#[test]
fn test_project_list_viewport_scrolling() {
    let backend = TestBackend::new(80, 15);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::Projects);

    let mut projects = Vec::new();
    for i in 1..=20 {
        let mut p = Project::new(format!("Course #{i}")).unwrap();
        p.id = Some(i as i64);
        projects.push(p);
    }
    app.project_list = projects;
    app.selected_project_index = 18; // Near bottom

    terminal
        .draw(|f| {
            clockify_tui::tui::views::projects::render_projects_view(&app, f, f.area());
        })
        .unwrap();

    let buffer: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    // Should render earlier projects indicator when scrolled down
    assert!(buffer.contains("▲"));
    assert!(buffer.contains("earlier"));
    assert!(buffer.contains("Course #19"));
}
