//! Root shell UI layouts, view dispatching, and modal dialogs.

use chrono::Utc;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Tabs};
use ratatui::Frame;

use crate::domain::{format_duration_hms, EntryMode};
use crate::tui::app::{App, Tab};

/// Renders the root TUI shell layout for the given application state.
pub fn render(app: &App, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top header
            Constraint::Length(3), // Tab navigation bar
            Constraint::Min(6),    // Main tab content body
            Constraint::Length(3), // Bottom status / hotkey hints footer
        ])
        .split(frame.area());

    render_header(app, frame, chunks[0]);
    render_tab_bar(app, frame, chunks[1]);
    render_body(app, frame, chunks[2]);
    render_footer(app, frame, chunks[3]);

    if app.show_help {
        render_help_modal(frame);
    }
}

/// Convenience alias accepting `(frame, app)`.
pub fn draw(frame: &mut Frame, app: &App) {
    render(app, frame);
}

/// Formats the global status badge string and styling.
fn format_status_badge(app: &App) -> (String, Style) {
    if let Some(entry) = &app.active_entry {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(entry.start_time);

        match entry.entry_mode {
            EntryMode::PomodoroWork | EntryMode::PomodoroBreak => {
                let remaining = app.pomodoro.time_remaining(entry.start_time, now);
                let badge = format!(
                    "[POMO: {} ({} left)]",
                    app.pomodoro.current_phase(),
                    format_duration_hms(&remaining)
                );
                (
                    badge,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            }
            EntryMode::Stopwatch => {
                let proj = if let Some(p) = &app.active_project_name {
                    format!(" ({p})")
                } else if !entry.description.is_empty() {
                    format!(" ({})", entry.description)
                } else {
                    String::new()
                };
                let badge = format!("[RUNNING: {}{}]", format_duration_hms(&elapsed), proj);
                (
                    badge,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
            }
        }
    } else {
        (
            "[IDLE]".to_string(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
    }
}

/// Renders the top header bar with title, status badge, and current date.
fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let date_len = if inner.width < 85 { 0 } else { 20 };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16),       // App title
            Constraint::Min(20),          // Global status badge
            Constraint::Length(date_len), // Current date/time
        ])
        .split(inner);

    // Left: App title
    let title_p = Paragraph::new(Span::styled(
        "⏱ Clockify TUI",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(title_p, cols[0]);

    // Center: Global status badge
    let (status_text, status_style) = format_status_badge(app);
    let status_p =
        Paragraph::new(Span::styled(status_text, status_style)).alignment(Alignment::Center);
    frame.render_widget(status_p, cols[1]);

    // Right: Date (if room available)
    if date_len > 0 {
        let date_str = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        let date_p = Paragraph::new(Span::styled(date_str, Style::default().fg(Color::Gray)))
            .alignment(Alignment::Right);
        frame.render_widget(date_p, cols[2]);
    }
}

/// Renders the primary navigation tab bar.
fn render_tab_bar(app: &App, frame: &mut Frame, area: Rect) {
    let tab_titles = vec!["[1] Timer", "[2] History", "[3] Projects", "[4] Analytics"];

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Views "),
        )
        .select(app.current_tab.index())
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )
        .divider(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));

    frame.render_widget(tabs, area);
}

/// Renders the central body dispatching according to active tab.
fn render_body(app: &App, frame: &mut Frame, area: Rect) {
    match app.current_tab {
        Tab::Timer => render_timer_tab(app, frame, area),
        Tab::History => render_history_tab(frame, area),
        Tab::Projects => render_projects_tab(frame, area),
        Tab::Analytics => render_analytics_tab(frame, area),
    }
}

/// Renders the initial layout for Tab 1 (Timer).
fn render_timer_tab(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Study Timer ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = if let Some(entry) = &app.active_entry {
        let elapsed = Utc::now().signed_duration_since(entry.start_time);
        let project_str = app
            .active_project_name
            .as_deref()
            .unwrap_or("No project assigned");
        let desc = if entry.description.is_empty() {
            "Untitled Session"
        } else {
            &entry.description
        };

        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Mode:         ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{}", entry.entry_mode),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Task:         ", Style::default().fg(Color::Cyan)),
                Span::raw(desc),
            ]),
            Line::from(vec![
                Span::styled("  Project:      ", Style::default().fg(Color::Cyan)),
                Span::styled(project_str, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled("  Elapsed:      ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format_duration_hms(&elapsed),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Quick Actions: ", Style::default().fg(Color::DarkGray)),
                Span::styled("[Space] ", Style::default().fg(Color::Yellow)),
                Span::raw("Stop Timer    "),
                Span::styled("[p] ", Style::default().fg(Color::Yellow)),
                Span::raw("Advance Pomodoro Phase"),
            ]),
        ]
    } else {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Status:       ", Style::default().fg(Color::Cyan)),
                Span::styled("IDLE (No active timer)", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Start Study Session:", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("    [Space]     ", Style::default().fg(Color::Yellow)),
                Span::raw("Start standard Stopwatch timer"),
            ]),
            Line::from(vec![
                Span::styled("    [p]         ", Style::default().fg(Color::Yellow)),
                Span::raw("Start Pomodoro focus session"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Pomodoro Cycle Settings:", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(
                        "    Focus: {} mins  │  Short Break: {} mins  │  Long Break: {} mins  │  Interval: {} sessions",
                        app.pomodoro.work_mins(),
                        app.pomodoro.short_break_mins(),
                        app.pomodoro.long_break_mins(),
                        app.pomodoro.sessions_until_long_break(),
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ]
    };

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}

/// Renders Tab 2 (History placeholder).
fn render_history_tab(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Study History ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Study Session History & Logs",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::raw(
            "  View past study sessions, inspect tags, filter by date, or delete entries.",
        )]),
        Line::from(vec![Span::styled(
            "  (Full table navigation, date filtering, and entry editor will be enabled in Tab 2.)",
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let p = Paragraph::new(text);
    frame.render_widget(p, inner);
}

/// Renders Tab 3 (Projects placeholder).
fn render_projects_tab(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Course & Project Management ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Courses & Projects", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Organize study hours by academic subjects and track weekly study hour targets."),
        ]),
        Line::from(vec![
            Span::styled("  (Project list view, target hours configuration, and color tags will be enabled in Tab 3.)", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let p = Paragraph::new(text);
    frame.render_widget(p, inner);
}

/// Renders Tab 4 (Analytics placeholder).
fn render_analytics_tab(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Study Analytics & Streaks ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Statistics & Study Insights", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Track current and best study streaks, weekly goal completion bars, and subject distribution."),
        ]),
        Line::from(vec![
            Span::styled("  (Streak counters, ASCII progress bars, and breakdown charts will be enabled in Tab 4.)", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let p = Paragraph::new(text);
    frame.render_widget(p, inner);
}

/// Renders the bottom footer bar with hotkey hints and temporary status banner.
fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut spans = Vec::new();

    if let Some((msg, _)) = &app.status_message {
        let max_msg_len = if inner.width < 90 { 20 } else { 40 };
        let display_msg = if msg.len() > max_msg_len {
            format!("{}...", &msg[..max_msg_len.saturating_sub(3)])
        } else {
            msg.clone()
        };
        spans.push(Span::styled("🔔 ", Style::default().fg(Color::Yellow)));
        spans.push(Span::styled(
            display_msg,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled("  │  ", Style::default().fg(Color::DarkGray)));
    }

    let hints = [
        ("[Space]", " Timer  "),
        ("[p]", " Pomo  "),
        ("[1-4]", " Tabs  "),
        ("[s]", " Sync  "),
        ("[?]", " Help  "),
        ("[q]", " Quit"),
    ];

    for (key, label) in hints {
        spans.push(Span::styled(
            key,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(label, Style::default().fg(Color::Gray)));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Left);
    frame.render_widget(paragraph, inner);
}

/// Calculates a centered rectangle with given width and height percentages.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Renders the centered modal help reference sheet.
fn render_help_modal(frame: &mut Frame) {
    let popup_area = centered_rect(80, 70, frame.area());
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " ⌨  Shortcuts Reference (Press [?] to Close) ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let left_text = vec![
        Line::from(Span::styled(
            "Navigation",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(" [1]–[4]   ", Style::default().fg(Color::Yellow)),
            Span::raw("Switch tabs"),
        ]),
        Line::from(vec![
            Span::styled(" [Tab]     ", Style::default().fg(Color::Yellow)),
            Span::raw("Next tab"),
        ]),
        Line::from(vec![
            Span::styled(" [S-Tab]   ", Style::default().fg(Color::Yellow)),
            Span::raw("Previous tab"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "General",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(" [?] / Esc ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle / Close help"),
        ]),
        Line::from(vec![
            Span::styled(" [q]       ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit application"),
        ]),
    ];

    let right_text = vec![
        Line::from(Span::styled(
            "Timer & Actions",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(" [Space]   ", Style::default().fg(Color::Yellow)),
            Span::raw("Start / Stop timer"),
        ]),
        Line::from(vec![
            Span::styled(" [p]       ", Style::default().fg(Color::Yellow)),
            Span::raw("Pomodoro phase"),
        ]),
        Line::from(vec![
            Span::styled(" [s]       ", Style::default().fg(Color::Yellow)),
            Span::raw("Clockify cloud sync"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Lists & CRUD",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(" [j] / [k] ", Style::default().fg(Color::Yellow)),
            Span::raw("Navigate lists"),
        ]),
        Line::from(vec![
            Span::styled(" [n] / [e] ", Style::default().fg(Color::Yellow)),
            Span::raw("New / Edit entry"),
        ]),
        Line::from(vec![
            Span::styled(" [d]       ", Style::default().fg(Color::Yellow)),
            Span::raw("Delete item"),
        ]),
    ];

    frame.render_widget(Paragraph::new(left_text), cols[0]);
    frame.render_widget(Paragraph::new(right_text), cols[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use crate::config::AppConfig;
    use crate::domain::TimeEntry;

    #[test]
    fn test_render_all_tabs_idle_state() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        for tab in [Tab::Timer, Tab::History, Tab::Projects, Tab::Analytics] {
            let mut app = App::new(AppConfig::default());
            app.set_tab(tab);

            terminal.draw(|f| render(&app, f)).unwrap();
            let buffer = terminal.backend().buffer();
            assert!(!buffer.content().is_empty());
        }
    }

    #[test]
    fn test_render_active_entry_and_pomodoro() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut app = App::new(AppConfig::default());
        let mut entry = TimeEntry::new("Linear Algebra Homework", Utc::now());
        entry.entry_mode = EntryMode::Stopwatch;
        app.set_active_entry(Some(entry), Some("Mathematics".to_string()));

        terminal.draw(|f| render(&app, f)).unwrap();

        // Switch to Pomodoro mode
        let mut pom_entry = TimeEntry::new("Physics Revision", Utc::now());
        pom_entry.entry_mode = EntryMode::PomodoroWork;
        app.set_active_entry(Some(pom_entry), Some("Physics".to_string()));

        terminal.draw(|f| render(&app, f)).unwrap();
    }

    #[test]
    fn test_render_help_modal() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut app = App::new(AppConfig::default());
        app.show_help = true;

        terminal.draw(|f| render(&app, f)).unwrap();
    }
}
