//! Integration and unit tests for Tab 4 - Analytics & Study Streaks Screen.

use chrono::{Duration, Local, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;

use chrono_cli::config::AppConfig;
use chrono_cli::domain::{
    DailyStudySummary, EntryMode, PomodoroStats, Project, StreakStats, SubjectBreakdown, TimeEntry,
};
use chrono_cli::storage::Database;
use chrono_cli::tui::app::{App, Tab};
use chrono_cli::tui::ui::render;
use chrono_cli::tui::views::analytics::{
    analytics_view, format_day_bar, format_efficiency_ratio, format_gauge, format_hours_human,
    format_streak_badge, format_streak_status, render_analytics_view,
};

#[test]
fn test_data_formatting_helpers() {
    // 1. Streak badges
    assert_eq!(format_streak_badge(0), "🔥 0 Days");
    assert_eq!(format_streak_badge(1), "🔥 1 Day");
    assert_eq!(format_streak_badge(7), "🔥 7 Days");

    // 2. Streak status
    let (studied, studied_color) = format_streak_status(true);
    assert_eq!(studied, "✓ Studied today");
    assert_eq!(studied_color, Color::Green);

    let (not_studied, not_color) = format_streak_status(false);
    assert_eq!(not_studied, "✗ Not yet studied today");
    assert_eq!(not_color, Color::Yellow);

    // 3. Day bar chart
    let bar_mon = format_day_bar("Mon", 4.2, 5.0, 10);
    assert!(bar_mon.contains("Mon:"));
    assert!(bar_mon.contains("4.2h"));
    assert!(bar_mon.contains('█'));

    let bar_zero = format_day_bar("Tue", 0.0, 5.0, 8);
    assert!(bar_zero.contains("Tue:"));
    assert!(bar_zero.contains("0.0h"));
    assert!(!bar_zero.contains('█'));
    assert!(bar_zero.contains("░░░░░░░░"));

    // 4. Progress gauge
    assert_eq!(format_gauge(0.5, 10), "[█████░░░░░]");
    assert_eq!(format_gauge(1.0, 8), "[████████]");
    assert_eq!(format_gauge(0.0, 6), "[░░░░░░]");

    // 5. Pomodoro efficiency ratio
    assert!((format_efficiency_ratio(100, 20) - 0.8333).abs() < 0.001);
    assert_eq!(format_efficiency_ratio(0, 0), 0.0);
    assert_eq!(format_efficiency_ratio(50, 0), 1.0);

    // 6. Hours human formatting
    assert_eq!(format_hours_human(4.2), "4h 12m");
    assert_eq!(format_hours_human(0.0), "0h 00m");
    assert_eq!(format_hours_human(-1.0), "0h 00m");
    assert_eq!(format_hours_human(1.5), "1h 30m");
}

#[test]
fn test_render_analytics_view_empty_state() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::Analytics);

    terminal
        .draw(|f| {
            render(&app, f);
        })
        .unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    // Verify main card headers and empty state indicators
    assert!(content.contains("Study Analytics & Streaks"));
    assert!(content.contains("Streak & Consistency"));
    assert!(content.contains("Weekly Study Hours"));
    assert!(content.contains("Subject Distribution"));
    assert!(content.contains("Pomodoro Session Metrics"));

    // Empty state specific texts
    assert!(content.contains("No study data recorded this week"));
    assert!(content.contains("✗ Not yet studied today"));
    assert!(content.contains("🔥"));
    assert!(content.contains("0 Days"));
    assert!(content.contains("Best Streak: 0 Days"));
    assert!(content.contains("Total: 0.0h"));
    assert!(content.contains("Sessions: 0 today"));

    // Footer hotkey hints
    assert!(content.contains("[1-4]"));
    assert!(content.contains("[r]"));
    assert!(content.contains("[?]"));
    assert!(content.contains("[q]"));
}

#[test]
fn test_render_analytics_view_populated_state() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let today = Local::now().date_naive();
    let streak_stats = StreakStats {
        current_streak: 5,
        longest_streak: 12,
        studied_today: true,
        last_study_date: Some(today),
    };

    let daily_summaries = vec![
        DailyStudySummary {
            date: today,
            duration: Duration::minutes(252), // 4.2h
        },
        DailyStudySummary {
            date: today - Duration::days(1),
            duration: Duration::minutes(120), // 2.0h
        },
    ];

    let subject_breakdown = vec![
        SubjectBreakdown::new("Algorithms", "#e74c3c", Duration::minutes(180), 45.0),
        SubjectBreakdown::new("Databases", "#3498db", Duration::minutes(120), 30.0),
        SubjectBreakdown::new("Operating Systems", "#2ecc71", Duration::minutes(100), 25.0),
    ];

    let pomodoro_stats = PomodoroStats {
        completed_today: 4,
        completed_this_week: 16,
        total_focus_mins: 400,
    };

    let app = App::new(AppConfig::default())
        .with_streak_stats(Some(streak_stats))
        .with_daily_summaries(daily_summaries)
        .with_subject_breakdown(subject_breakdown)
        .with_pomodoro_stats(Some(pomodoro_stats));

    terminal
        .draw(|f| {
            render_analytics_view(&app, f, f.area());
        })
        .unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    // Verify streak card
    assert!(content.contains("🔥"));
    assert!(content.contains("5 Days"));
    assert!(content.contains("Best Streak: 12 Days"));
    assert!(content.contains("✓ Studied today"));

    // Verify subjects
    assert!(content.contains("Algorithms"));
    assert!(content.contains("Databases"));
    assert!(content.contains("Operating Systems"));
    assert!(content.contains("45.0%"));
    assert!(content.contains("30.0%"));

    // Verify Pomodoro
    assert!(content.contains("4 today"));
    assert!(content.contains("16 this week"));
    assert!(content.contains("Focus:"));
    assert!(content.contains("Break:"));
    assert!(content.contains("Efficiency:"));
}

#[test]
fn test_standard_80x24_dimensions_no_overflow() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let today = Local::now().date_naive();
    let streak_stats = StreakStats {
        current_streak: 3,
        longest_streak: 8,
        studied_today: true,
        last_study_date: Some(today),
    };

    let daily_summaries = vec![DailyStudySummary {
        date: today,
        duration: Duration::minutes(180),
    }];

    let subject_breakdown = vec![
        SubjectBreakdown::new("Algorithms", "#e74c3c", Duration::minutes(120), 66.7),
        SubjectBreakdown::new("Databases", "#3498db", Duration::minutes(60), 33.3),
    ];

    let pomodoro_stats = PomodoroStats {
        completed_today: 3,
        completed_this_week: 8,
        total_focus_mins: 200,
    };

    let mut app = App::new(AppConfig::default())
        .with_streak_stats(Some(streak_stats))
        .with_daily_summaries(daily_summaries)
        .with_subject_breakdown(subject_breakdown)
        .with_pomodoro_stats(Some(pomodoro_stats));
    app.set_tab(Tab::Analytics);

    // 1. Render through root UI
    terminal
        .draw(|f| {
            render(&app, f);
        })
        .unwrap();

    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();

    assert!(content.contains("Study Analytics & Streaks"));
    assert!(content.contains("🔥"));
    assert!(content.contains("3 Days"));
    assert!(content.contains("Best Streak: 8 Days"));
    assert!(content.contains("✓ Studied today"));
    assert!(content.contains("Algorithms"));

    // 2. Direct view alias
    terminal
        .draw(|f| {
            analytics_view(&app, f, f.area());
        })
        .unwrap();
}

#[test]
fn test_narrow_and_short_layout_responsiveness() {
    // Test stacked layout for narrow terminals (width < 60)
    let backend_narrow = TestBackend::new(55, 20);
    let mut terminal_narrow = Terminal::new(backend_narrow).unwrap();

    let app = App::new(AppConfig::default());
    terminal_narrow
        .draw(|f| {
            render_analytics_view(&app, f, f.area());
        })
        .unwrap();

    let content: String = terminal_narrow
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(content.contains("Study Analytics & Streaks"));

    // Test short layout (height < 10)
    let backend_short = TestBackend::new(70, 8);
    let mut terminal_short = Terminal::new(backend_short).unwrap();

    terminal_short
        .draw(|f| {
            render_analytics_view(&app, f, f.area());
        })
        .unwrap();

    // Test zero area safety
    let backend_zero = TestBackend::new(0, 0);
    let mut terminal_zero = Terminal::new(backend_zero).unwrap();
    terminal_zero
        .draw(|f| {
            render_analytics_view(&app, f, f.area());
        })
        .unwrap();
}

#[test]
fn test_manual_refresh_shortcut_on_analytics_tab() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let p = db
        .create_project(
            &Project::new("Computer Science")
                .unwrap()
                .with_color("#3498db"),
        )
        .unwrap();

    let now = Utc::now();
    let mut entry = TimeEntry::new("Study Graphs", now - Duration::hours(2))
        .with_project_id(p.id.unwrap())
        .with_entry_mode(EntryMode::PomodoroWork);
    entry.end_time = Some(now - Duration::hours(1));
    db.create_manual_entry(&entry).unwrap();

    let mut app = App::new(AppConfig::default());
    app.set_tab(Tab::Analytics);

    assert!(app.streak_stats.is_none());
    assert!(app.daily_summaries.is_empty());

    // Press 'r' to trigger manual refresh
    let key_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
    app.handle_key_with_db(key_r, &mut db);

    // Analytics data should now be loaded from database
    assert!(app.streak_stats.is_some());
    let streak = app.streak_stats.as_ref().unwrap();
    assert!(streak.current_streak >= 1);
    assert!(streak.studied_today);

    assert!(!app.daily_summaries.is_empty());
    assert!(!app.subject_breakdown.is_empty());
    assert!(app.pomodoro_stats.is_some());

    // Status message should indicate refresh
    assert!(app.status_message.is_some());
    assert_eq!(
        app.status_message.as_ref().unwrap().0,
        "Analytics refreshed"
    );

    // Also test uppercase 'R'
    app.clear_status_message();
    let key_upper_r = KeyEvent::new(KeyCode::Char('R'), KeyModifiers::NONE);
    app.handle_key_with_db(key_upper_r, &mut db);
    assert_eq!(
        app.status_message.as_ref().unwrap().0,
        "Analytics refreshed"
    );
}

#[test]
fn test_refresh_analytics_updates_app_fields() {
    let db = Database::open_in_memory().expect("failed to open in-memory db");

    let mut app = App::new(AppConfig::default());
    assert!(app.streak_stats.is_none());

    app.refresh_analytics(&db);

    // Empty database still returns valid stats
    assert!(app.streak_stats.is_some());
    assert_eq!(app.streak_stats.as_ref().unwrap().current_streak, 0);
    assert_eq!(app.streak_stats.as_ref().unwrap().longest_streak, 0);
    assert!(!app.streak_stats.as_ref().unwrap().studied_today);
    assert!(app.daily_summaries.is_empty());
    assert!(app.subject_breakdown.is_empty());
    assert!(app.pomodoro_stats.is_some());
}
