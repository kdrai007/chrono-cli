//! Keyboard shortcut help modal overlay widget.

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::tui::widgets::modal::centered_rect_fixed;

/// Renders a centered keyboard shortcut reference sheet modal.
pub fn render_help_modal(frame: &mut Frame) {
    let area = frame.area();
    let popup_width = area.width.clamp(20, 76);
    let popup_height = area.height.clamp(10, 18);
    let popup_area = centered_rect_fixed(popup_width, popup_height, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " ⌨  Keyboard Shortcuts Reference ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(vec![
            Span::styled(
                " [?] / [Esc] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Close Help ", Style::default().fg(Color::Gray)),
        ]));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let left_column = vec![
        category_header("Navigation"),
        shortcut_row(" [1-4]        ", "Switch tabs (1-4)"),
        shortcut_row(" [Tab]/[S-Tab]", "Next / Prev tab"),
        shortcut_row(" [j/k]/[↑/↓]  ", "Navigate lists"),
        Line::from(""),
        category_header("Timer"),
        shortcut_row(" [Space]      ", "Start / Stop timer"),
        shortcut_row(" [p]          ", "Pomodoro phase toggle"),
        shortcut_row(" [r]          ", "Repeat recent session"),
        Line::from(""),
        category_header("General"),
        shortcut_row(" [s]          ", "Clockify cloud sync"),
        shortcut_row(" [?] / Esc    ", "Toggle / Close help"),
        shortcut_row(" [q] / Ctrl+C ", "Quit application"),
    ];

    let right_column = vec![
        category_header("History"),
        shortcut_row(" [n]  ", "New manual entry"),
        shortcut_row(" [e]  ", "Edit entry"),
        shortcut_row(" [d]  ", "Delete entry"),
        shortcut_row(" [/]  ", "Filter entries"),
        Line::from(""),
        category_header("Projects"),
        shortcut_row(" [a]  ", "Add project"),
        shortcut_row(" [e]  ", "Edit target hours/color"),
        shortcut_row(" [x]  ", "Toggle archive"),
        shortcut_row(" [d]  ", "Delete project"),
        Line::from(""),
        category_header("Analytics"),
        shortcut_row(" [r]  ", "Refresh analytics data"),
    ];

    frame.render_widget(Paragraph::new(left_column), cols[0]);
    frame.render_widget(Paragraph::new(right_column), cols[1]);
}

fn category_header(title: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        title,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::UNDERLINED),
    ))
}

fn shortcut_row(key: &'static str, desc: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            key,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(desc, Style::default().fg(Color::White)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_help_modal_renders_on_80x24_without_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_help_modal(f);
            })
            .unwrap();

        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();

        // Check headers
        assert!(content.contains("Keyboard Shortcuts Reference"));
        assert!(content.contains("Navigation"));
        assert!(content.contains("Timer"));
        assert!(content.contains("General"));
        assert!(content.contains("History"));
        assert!(content.contains("Projects"));
        assert!(content.contains("Analytics"));

        // Check key shortcuts
        assert!(content.contains("[1-4]"));
        assert!(content.contains("[Tab]"));
        assert!(content.contains("[Space]"));
        assert!(content.contains("[p]"));
        assert!(content.contains("[s]"));
        assert!(content.contains("[?]"));
        assert!(content.contains("[q]"));
        assert!(content.contains("[n]"));
        assert!(content.contains("[e]"));
        assert!(content.contains("[d]"));
        assert!(content.contains("[/]"));
        assert!(content.contains("[a]"));
        assert!(content.contains("[x]"));
        assert!(content.contains("[r]"));
    }
}
