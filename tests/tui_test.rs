use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use chrono_cli::config::AppConfig;
use chrono_cli::domain::{PomodoroPhase, TimeEntry};
use chrono_cli::tui::app::{App, Tab};
use chrono_cli::tui::event::Event;
use chrono_cli::tui::ui::render;

#[test]
fn test_tui_app_state_and_tab_transitions() {
    let mut app = App::new(AppConfig::default());

    // Initially Tab 0 (Timer)
    assert_eq!(app.current_tab, Tab::Timer);
    assert!(app.running);
    assert!(!app.show_help);

    // Number keys switch directly to corresponding tab
    app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
    assert_eq!(app.current_tab, Tab::History);

    app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
    assert_eq!(app.current_tab, Tab::Projects);

    app.handle_key(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE));
    assert_eq!(app.current_tab, Tab::Analytics);

    app.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
    assert_eq!(app.current_tab, Tab::Timer);

    // Tab and Shift+Tab / BackTab cyclical navigation
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.current_tab, Tab::History);

    app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(app.current_tab, Tab::Timer);

    app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(app.current_tab, Tab::Analytics);
}

#[test]
fn test_tui_pomodoro_and_active_entry_updates() {
    let mut app = App::new(AppConfig::default());
    assert_eq!(app.pomodoro.current_phase(), PomodoroPhase::Work(1));

    // Next phase transitions
    let phase = app.pomodoro.next_phase();
    assert_eq!(phase, PomodoroPhase::ShortBreak(1));

    let entry = TimeEntry::new("Mathematics Practice", chrono::Utc::now());
    app.set_active_entry(Some(entry), Some("Mathematics".to_string()));
    assert!(app.active_entry.is_some());
    assert_eq!(app.active_project_name.as_deref(), Some("Mathematics"));

    app.set_active_entry(None, None);
    assert!(app.active_entry.is_none());
    assert!(app.active_project_name.is_none());
}

#[test]
fn test_tui_render_all_views_and_help_modal() {
    let backend = TestBackend::new(120, 35);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(AppConfig::default());

    // Render every tab
    for tab in Tab::ALL {
        app.set_tab(tab);
        terminal.draw(|f| render(&app, f)).unwrap();
        let buffer = terminal.backend().buffer();
        assert!(!buffer.content().is_empty());
    }

    // Render help overlay
    app.toggle_help();
    assert!(app.show_help);
    terminal.draw(|f| render(&app, f)).unwrap();

    // Dismiss help overlay
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.show_help);
    terminal.draw(|f| render(&app, f)).unwrap();
}

#[test]
fn test_tui_render_standard_80x24_dimensions() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(AppConfig::default());
    terminal.draw(|f| render(&app, f)).unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    assert!(content.contains("chrono"));
    assert!(content.contains("Timer"));
    assert!(content.contains("History"));
    assert!(content.contains("IDLE"));

    // Test help modal on 80x24
    app.toggle_help();
    assert!(app.show_help);
    terminal.draw(|f| render(&app, f)).unwrap();

    // Verify 'q' in modal closes modal instead of quitting app
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert!(!app.show_help);
    assert!(
        app.running,
        "App should still be running after closing modal with 'q'"
    );
}

#[test]
fn test_tui_event_loop_step() {
    let mut app = App::new(AppConfig::default());

    // Event::Key
    let key_event = Event::Key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
    if let Event::Key(k) = key_event {
        app.handle_key(k);
    }
    assert_eq!(app.current_tab, Tab::History);

    // Event::Tick
    app.set_status_message("Hello from test");
    assert!(app.status_message.is_some());
    app.tick();
    assert!(app.status_message.is_some());

    // Event::Key quit
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert!(!app.running);
}
