//! Reusable modal popup container widget and centering utilities.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear};
use ratatui::Frame;

/// Helper function to compute a centered rectangle within an outer area given percentage dimensions.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Helper function to compute a centered rectangle with fixed width and height (clamped to area).
pub fn centered_rect_fixed(width: u16, height: u16, area: Rect) -> Rect {
    let actual_width = width.min(area.width);
    let actual_height = height.min(area.height);

    let x = area.x + (area.width.saturating_sub(actual_width)) / 2;
    let y = area.y + (area.height.saturating_sub(actual_height)) / 2;

    Rect {
        x,
        y,
        width: actual_width,
        height: actual_height,
    }
}

/// Reusable modal dialog builder.
#[derive(Debug, Clone)]
pub struct Modal<'a> {
    title: &'a str,
    percent_x: u16,
    percent_y: u16,
    border_color: Color,
    title_color: Color,
    hotkeys: Vec<(&'a str, &'a str)>,
}

impl<'a> Modal<'a> {
    /// Creates a new modal dialog builder with default dimensions (60% width, 50% height).
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            percent_x: 60,
            percent_y: 50,
            border_color: Color::Cyan,
            title_color: Color::Yellow,
            hotkeys: Vec::new(),
        }
    }

    /// Sets modal width percentage.
    pub fn width_percent(mut self, percent: u16) -> Self {
        self.percent_x = percent;
        self
    }

    /// Sets modal height percentage.
    pub fn height_percent(mut self, percent: u16) -> Self {
        self.percent_y = percent;
        self
    }

    /// Sets border highlight color.
    pub fn border_color(mut self, color: Color) -> Self {
        self.border_color = color;
        self
    }

    /// Sets title highlight color.
    pub fn title_color(mut self, color: Color) -> Self {
        self.title_color = color;
        self
    }

    /// Adds a hotkey hint to the bottom border of the dialog.
    pub fn with_hotkey(mut self, key: &'a str, desc: &'a str) -> Self {
        self.hotkeys.push((key, desc));
        self
    }

    /// Renders the modal background clear and bordered dialog container, returning the inner content area.
    pub fn render_frame(&self, frame: &mut Frame) -> Rect {
        let popup_area = centered_rect(self.percent_x, self.percent_y, frame.area());
        frame.render_widget(Clear, popup_area);

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.border_color))
            .title(Span::styled(
                format!(" {} ", self.title),
                Style::default()
                    .fg(self.title_color)
                    .add_modifier(Modifier::BOLD),
            ));

        if !self.hotkeys.is_empty() {
            let mut spans = Vec::new();
            for (i, (key, desc)) in self.hotkeys.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw("   "));
                }
                spans.push(Span::styled(
                    *key,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::raw(" "));
                spans.push(Span::styled(*desc, Style::default().fg(Color::Gray)));
            }
            block = block.title_bottom(Line::from(spans));
        }

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);
        inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_centered_rect_proportions() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = centered_rect(60, 40, area);

        assert_eq!(centered.width, 60);
        assert_eq!(centered.height, 20); // 40% of 50 = 20
        assert_eq!(centered.x, 20); // (100 - 60) / 2 = 20
        assert_eq!(centered.y, 15); // (50 - 20) / 2 = 15
    }

    #[test]
    fn test_centered_rect_fixed() {
        let area = Rect::new(0, 0, 100, 50);
        let fixed = centered_rect_fixed(40, 20, area);

        assert_eq!(fixed.width, 40);
        assert_eq!(fixed.height, 20);
        assert_eq!(fixed.x, 30);
        assert_eq!(fixed.y, 15);
    }
}
