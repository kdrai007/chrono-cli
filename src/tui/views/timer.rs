//! Tab 1 - Active Timer & Pomodoro view implementation.

use std::collections::HashSet;

use chrono::{Local, Utc};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table};
use ratatui::Frame;

use crate::domain::{format_duration_hms, format_duration_human, EntryMode, PomodoroPhase};
use crate::tui::app::App;
use crate::tui::widgets::BigClock;

/// Converts a hex color string (e.g. `"#3498db"`) to a `ratatui::style::Color`.
pub fn parse_hex_color(hex: &str) -> Color {
    let s = hex.trim().trim_start_matches('#');
    if s.len() == 6 && s.is_ascii() && s.chars().all(|c| c.is_ascii_hexdigit()) {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }
    Color::Cyan
}

/// Renders the complete Timer & Pomodoro screen into `area`.
pub fn render_timer_view(app: &App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(55), // Top Card: Active Timer / Pomodoro Status
            Constraint::Percentage(45), // Bottom Card: Today's Summary & Recent Sessions
        ])
        .split(area);

    render_top_card(app, frame, chunks[0]);
    render_bottom_card(app, frame, chunks[1]);
}

/// Convenience alias matching view conventions.
pub fn timer_view(app: &App, frame: &mut Frame, area: Rect) {
    render_timer_view(app, frame, area);
}

/// Renders the Top Card: Active Timer / Pomodoro Status.
fn render_top_card(app: &App, frame: &mut Frame, area: Rect) {
    let is_pomodoro = app
        .active_entry
        .as_ref()
        .map(|e| {
            matches!(
                e.entry_mode,
                EntryMode::PomodoroWork | EntryMode::PomodoroBreak
            )
        })
        .unwrap_or(false);

    let title = if let Some(entry) = &app.active_entry {
        match entry.entry_mode {
            EntryMode::PomodoroWork => " 🍅 Pomodoro Focus Session ",
            EntryMode::PomodoroBreak => " ☕ Pomodoro Break ",
            EntryMode::Stopwatch => " ⏱ Active Stopwatch Timer ",
        }
    } else {
        " ⏱ Active Timer / Pomodoro Status "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let sub_chunks = if is_pomodoro {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Badges / info
                Constraint::Min(4),    // Big Clock
                Constraint::Length(1), // Progress Gauge
                Constraint::Length(1), // Controls hint
            ])
            .split(inner)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Badges / info
                Constraint::Min(4),    // Big Clock
                Constraint::Length(1), // Controls hint
            ])
            .split(inner)
    };

    let now = Utc::now();

    // 1. Info / Badges line
    if let Some(entry) = &app.active_entry {
        let desc = if entry.description.is_empty() {
            "Untitled Session"
        } else {
            &entry.description
        };

        let mut spans = Vec::new();

        if is_pomodoro {
            let (phase_str, phase_style) = match app.pomodoro.current_phase() {
                PomodoroPhase::Work(i) => (
                    format!(
                        "[Focus Session #{} of {}]",
                        i,
                        app.pomodoro.sessions_until_long_break()
                    ),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                PomodoroPhase::ShortBreak(i) => (
                    format!("[Short Break #{}]", i),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
                PomodoroPhase::LongBreak(i) => (
                    format!("[Long Break #{}]", i),
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
            };
            spans.push(Span::styled(phase_str, phase_style));
            spans.push(Span::raw("  "));
        }

        spans.push(Span::styled(
            desc,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));

        // Project badge
        let (proj_name, proj_color) = if let Some(p_name) = &app.active_project_name {
            let color = entry
                .project_id
                .and_then(|pid| app.projects.get(&pid))
                .map(|p| p.color.as_str())
                .unwrap_or("#3498db");
            (p_name.as_str(), color)
        } else if let Some(pid) = entry.project_id {
            if let Some(p) = app.projects.get(&pid) {
                (p.name.as_str(), p.color.as_str())
            } else {
                ("General", "#888888")
            }
        } else {
            ("No Project", "#888888")
        };

        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("[{proj_name}]"),
            Style::default()
                .fg(parse_hex_color(proj_color))
                .add_modifier(Modifier::BOLD),
        ));

        // Tags
        if !entry.tags.is_empty() {
            let tags_formatted = entry
                .tags
                .iter()
                .map(|t| format!("#{}", t.name))
                .collect::<Vec<_>>()
                .join(" ");
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                tags_formatted,
                Style::default().fg(Color::DarkGray),
            ));
        }

        let p = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
        frame.render_widget(p, sub_chunks[0]);
    } else {
        let text = Line::from(vec![
            Span::styled(
                "[IDLE]",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  No active session. Press "),
            Span::styled(
                "[Space]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" to start Stopwatch or "),
            Span::styled(
                "[p]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" for Pomodoro."),
        ]);
        let p = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(p, sub_chunks[0]);
    }

    // 2. Big Clock widget
    if let Some(entry) = &app.active_entry {
        if is_pomodoro {
            let remaining = app.pomodoro.time_remaining(entry.start_time, now);
            let clock_color = if app.pomodoro.current_phase().is_work() {
                Color::Green
            } else {
                Color::Blue
            };
            let clock = BigClock::from_duration(&remaining).style(
                Style::default()
                    .fg(clock_color)
                    .add_modifier(Modifier::BOLD),
            );
            frame.render_widget(clock, sub_chunks[1]);
        } else {
            let elapsed = now.signed_duration_since(entry.start_time);
            let clock = BigClock::from_duration(&elapsed).style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            );
            frame.render_widget(clock, sub_chunks[1]);
        }
    } else {
        let clock = BigClock::new("00:00:00").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(clock, sub_chunks[1]);
    }

    // 3. Pomodoro session progress Gauge (if pomodoro mode)
    if is_pomodoro {
        if let Some(entry) = &app.active_entry {
            let progress = app.pomodoro.progress(entry.start_time, now);
            let percent = (progress * 100.0).clamp(0.0, 100.0) as u16;
            let gauge_color = if app.pomodoro.current_phase().is_work() {
                Color::Green
            } else {
                Color::Blue
            };
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(gauge_color).bg(Color::DarkGray))
                .percent(percent)
                .label(format!("Session Progress: {}%", percent));
            frame.render_widget(gauge, sub_chunks[2]);
        }
    }

    // 4. Controls hint
    let controls_area = if is_pomodoro {
        sub_chunks[3]
    } else {
        sub_chunks[2]
    };
    let controls_line = Line::from(vec![
        Span::styled(
            "[Space] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Start/Stop    "),
        Span::styled(
            "[p] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Pomodoro/Stopwatch    "),
        Span::styled(
            "[r] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Repeat recent"),
    ]);
    frame.render_widget(
        Paragraph::new(controls_line).alignment(Alignment::Center),
        controls_area,
    );
}

/// Renders the Bottom Card: Today's Summary & Recent Sessions.
fn render_bottom_card(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Today's Summary & Recent Sessions ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let sub_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Summary banner
            Constraint::Min(3),    // Table list of sessions
        ])
        .split(inner);

    // 1. Calculate today's total logged study hours
    let now = Utc::now();
    let mut total_seconds: i64 = 0;
    let mut distinct_projects = HashSet::new();

    for entry in &app.today_entries {
        if entry.entry_mode == EntryMode::PomodoroBreak {
            continue;
        }
        if let Some(end) = entry.end_time {
            total_seconds += (end - entry.start_time).num_seconds().max(0);
        } else {
            total_seconds += (now - entry.start_time).num_seconds().max(0);
        }
        if let Some(pid) = entry.project_id {
            distinct_projects.insert(pid);
        }
    }

    let total_dur = chrono::Duration::seconds(total_seconds);
    let total_formatted = format_duration_human(&total_dur);
    let subject_count = distinct_projects.len();

    let mut spans = vec![
        Span::styled("  Total Study: ", Style::default().fg(Color::Cyan)),
        Span::styled(
            total_formatted,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " across {} subject{}",
                subject_count,
                if subject_count == 1 { "" } else { "s" }
            ),
            Style::default().fg(Color::Gray),
        ),
    ];

    if inner.width >= 90 {
        spans.push(Span::styled(
            "   [j/k or ↑/↓]: select session   [r]: restart selected",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let summary_line = Line::from(spans);
    frame.render_widget(Paragraph::new(summary_line), sub_chunks[0]);

    // 2. Table / List of today's recent 5 study sessions
    let recent_entries: Vec<&crate::domain::TimeEntry> = app.today_entries.iter().take(5).collect();

    if recent_entries.is_empty() {
        let empty_msg = Paragraph::new(
            "  No study sessions recorded today yet. Press [Space] to start your first session!",
        )
        .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty_msg, sub_chunks[1]);
        return;
    }

    let header = Row::new(["  Time", "Duration", "Project", "Description", "Tags"]).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::UNDERLINED),
    );

    let rows: Vec<Row> = recent_entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let is_selected = idx == app.selected_recent_index;
            let prefix = if is_selected { "> " } else { "  " };

            let time_str = format!(
                "{}{}",
                prefix,
                entry.start_time.with_timezone(&Local).format("%H:%M")
            );

            let duration_str = if let Some(end) = entry.end_time {
                format_duration_hms(&(end - entry.start_time))
            } else {
                format!(
                    "{} (active)",
                    format_duration_hms(&(now - entry.start_time))
                )
            };

            let (proj_name, proj_color) = if let Some(pid) = entry.project_id {
                if let Some(p) = app.projects.get(&pid) {
                    (p.name.as_str(), p.color.as_str())
                } else {
                    ("-", "#888888")
                }
            } else {
                ("-", "#888888")
            };

            let desc_str = if entry.description.is_empty() {
                "(no description)"
            } else {
                &entry.description
            };

            let tags_str = if entry.tags.is_empty() {
                "-".to_string()
            } else {
                entry
                    .tags
                    .iter()
                    .map(|t| t.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            let row_style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            Row::new([
                Cell::from(time_str),
                Cell::from(duration_str),
                Cell::from(Span::styled(
                    proj_name,
                    Style::default().fg(parse_hex_color(proj_color)),
                )),
                Cell::from(desc_str),
                Cell::from(Span::styled(tags_str, Style::default().fg(Color::DarkGray))),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(9),  // Time
            Constraint::Length(16), // Duration
            Constraint::Length(18), // Project
            Constraint::Min(20),    // Description
            Constraint::Length(20), // Tags
        ],
    )
    .header(header);

    frame.render_widget(table, sub_chunks[1]);
}
