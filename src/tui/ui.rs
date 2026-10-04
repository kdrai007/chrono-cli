//! Root shell UI layouts, view dispatching, and modal dialogs.

use chrono::Utc;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Tabs};
use ratatui::Frame;

use crate::domain::{format_duration_hms, EntryMode};
use crate::tui::app::{App, Tab};
use crate::tui::widgets::modal::centered_rect;

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
    } else if app.show_delete_entry_modal {
        crate::tui::views::history::render_delete_modal(app, frame);
    } else if app.show_new_entry_modal {
        crate::tui::views::history::render_entry_form_modal(app, frame, true);
    } else if app.show_edit_entry_modal {
        crate::tui::views::history::render_entry_form_modal(app, frame, false);
    } else if app.show_filter_modal {
        crate::tui::views::history::render_filter_modal(app, frame);
    } else if app.show_delete_project_modal {
        crate::tui::views::projects::render_delete_project_modal(app, frame);
    } else if app.show_add_project_modal {
        crate::tui::views::projects::render_project_form_modal(app, frame, true);
    } else if app.show_edit_project_modal {
        crate::tui::views::projects::render_project_form_modal(app, frame, false);
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
        Tab::History => render_history_tab(app, frame, area),
        Tab::Projects => render_projects_tab(app, frame, area),
        Tab::Analytics => render_analytics_tab(app, frame, area),
    }
}

/// Renders Tab 1 (Timer & Pomodoro View).
pub fn render_timer_tab(app: &App, frame: &mut Frame, area: Rect) {
    crate::tui::views::timer::render_timer_view(app, frame, area);
}

/// Renders Tab 2 (History Timesheet view).
pub fn render_history_tab(app: &App, frame: &mut Frame, area: Rect) {
    crate::tui::views::history::render_history_view(app, frame, area);
}

/// Renders Tab 3 (Projects & Course Targets view).
pub fn render_projects_tab(app: &App, frame: &mut Frame, area: Rect) {
    crate::tui::views::projects::render_projects_view(app, frame, area);
}

/// Renders Tab 4 (Analytics & Study Streaks view).
pub fn render_analytics_tab(app: &App, frame: &mut Frame, area: Rect) {
    crate::tui::views::analytics::render_analytics_view(app, frame, area);
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
        let char_count = msg.chars().count();
        let display_msg = if char_count > max_msg_len {
            let limit = max_msg_len.saturating_sub(3);
            let prefix: String = msg.chars().take(limit).collect();
            format!("{prefix}...")
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

    let default_hints: &[(&str, &str)] = &[
        ("[Space]", " Timer  "),
        ("[p]", " Pomo  "),
        ("[1-4]", " Tabs  "),
        ("[s]", " Sync  "),
        ("[?]", " Help  "),
        ("[q]", " Quit"),
    ];

    let history_hints: &[(&str, &str)] = &[
        ("[n]", " New  "),
        ("[e]", " Edit  "),
        ("[d]", " Del  "),
        ("[/]", " Filter  "),
        ("[1-4]", " Tabs  "),
        ("[?]", " Help  "),
        ("[q]", " Quit"),
    ];

    let projects_hints: &[(&str, &str)] = &[
        ("[a]", " Add  "),
        ("[e]", " Edit  "),
        ("[x]", " Archive  "),
        ("[d]", " Del  "),
        ("[j/k]", " Nav  "),
        ("[1-4]", " Tabs  "),
        ("[?]", " Help  "),
        ("[q]", " Quit"),
    ];

    let analytics_hints: &[(&str, &str)] = &[
        ("[r]", " Refresh  "),
        ("[1-4]", " Tabs  "),
        ("[?]", " Help  "),
        ("[q]", " Quit"),
    ];

    let hints = match app.current_tab {
        Tab::History => history_hints,
        Tab::Projects => projects_hints,
        Tab::Analytics => analytics_hints,
        _ => default_hints,
    };

    for (key, label) in hints {
        spans.push(Span::styled(
            *key,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(*label, Style::default().fg(Color::Gray)));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Left);
    frame.render_widget(paragraph, inner);
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
        Line::from(vec![
            Span::styled(" [r]       ", Style::default().fg(Color::Yellow)),
            Span::raw("Refresh analytics"),
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
