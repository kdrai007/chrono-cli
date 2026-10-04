//! Big digital clock widget using 3-row tall Unicode block glyphs.

use std::borrow::Cow;

use chrono::Duration;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use crate::domain::format_duration_hms;

/// A 3-row tall digital clock widget rendering block ASCII glyphs.
///
/// If terminal area is narrow (< 45 cols or < 6 rows for clock), it falls
/// back to a centered styled single-line text representation.
#[derive(Debug, Clone)]
pub struct BigClock<'a> {
    /// Formatted time string (e.g. "00:24:18").
    pub time_str: Cow<'a, str>,
    /// Styling applied to the clock glyphs.
    pub style: Style,
}

impl<'a> BigClock<'a> {
    /// Creates a new `BigClock` from a formatted time string.
    pub fn new(time_str: impl Into<Cow<'a, str>>) -> Self {
        Self {
            time_str: time_str.into(),
            style: Style::default(),
        }
    }

    /// Creates a new `BigClock` displaying the given duration in `HH:MM:SS` format.
    pub fn from_duration(duration: &Duration) -> Self {
        Self::new(Self::format_duration(duration))
    }

    /// Sets the text styling for this clock.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Returns the underlying time string.
    pub fn time_str(&self) -> &str {
        &self.time_str
    }

    /// Formats a `Duration` as `HH:MM:SS` for clock display.
    pub fn format_duration(duration: &Duration) -> String {
        format_duration_hms(duration)
    }

    /// Returns the 3 lines of block glyphs representing `time_str`.
    pub fn render_lines(time_str: &str) -> [String; 3] {
        let mut row0 = String::new();
        let mut row1 = String::new();
        let mut row2 = String::new();

        for (i, ch) in time_str.chars().enumerate() {
            if i > 0 {
                row0.push(' ');
                row1.push(' ');
                row2.push(' ');
            }
            let glyph = Self::glyph_for(ch);
            row0.push_str(glyph[0]);
            row1.push_str(glyph[1]);
            row2.push_str(glyph[2]);
        }

        [row0, row1, row2]
    }

    /// Returns the 3-row block glyphs for a single character.
    fn glyph_for(ch: char) -> [&'static str; 3] {
        match ch {
            '0' => ["█▀█", "█ █", "█▄█"],
            '1' => [" █ ", " █ ", " █ "],
            '2' => ["█▀█", " ▄▀", "█▄▄"],
            '3' => ["█▀█", " ▀█", "█▄█"],
            '4' => ["█ █", "▀▀█", "  █"],
            '5' => ["█▀▀", "▀▀█", "█▄█"],
            '6' => ["█▀▀", "█▀█", "█▄█"],
            '7' => ["▀▀█", "  █", "  █"],
            '8' => ["█▀█", "█▀█", "█▄█"],
            '9' => ["█▀█", "▀▀█", "  █"],
            ':' => [" ", "▄", "▄"],
            _ => ["   ", "   ", "   "],
        }
    }
}

impl<'a> Widget for BigClock<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Narrow or shallow area fallback: styled single-line text
        if area.width < 35 || area.height < 3 {
            let text = &self.time_str;
            let text_len = text.len() as u16;
            let x = area.x + (area.width.saturating_sub(text_len)) / 2;
            let y = area.y + area.height / 2;
            buf.set_string(x, y, text, self.style.add_modifier(Modifier::BOLD));
        } else {
            let lines = Self::render_lines(&self.time_str);
            let start_y = area.y + (area.height.saturating_sub(3)) / 2;
            for (idx, line) in lines.iter().enumerate() {
                let y = start_y + idx as u16;
                let line_len = line.chars().count() as u16;
                let x = area.x + (area.width.saturating_sub(line_len)) / 2;
                buf.set_string(x, y, line, self.style);
            }
        }
    }
}

impl<'a> Widget for &BigClock<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.clone().render(area, buf);
    }
}
