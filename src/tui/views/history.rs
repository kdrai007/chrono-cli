//! Tab 2 - History Timesheet table view and modal dialogs.

use chrono::{DateTime, Local, NaiveDate, Utc};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::domain::TimeEntry;
use crate::tui::app::App;
use crate::tui::views::timer::parse_hex_color;
use crate::tui::widgets::modal::{centered_rect, Modal};

/// Date grouping categorizing historical study sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DateGroup {
    /// Entries recorded today.
    Today,
    /// Entries recorded yesterday.
    Yesterday,
    /// Entries recorded before yesterday.
    Earlier,
}

impl DateGroup {
    /// Human-readable label for the date group.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Yesterday => "Yesterday",
            Self::Earlier => "Earlier",
        }
    }

    /// Table separator header text with horizontal bar decoration.
    pub fn header_text(&self) -> &'static str {
        match self {
            Self::Today => "── Today ──",
            Self::Yesterday => "── Yesterday ──",
            Self::Earlier => "── Earlier ──",
        }
    }

    /// Determines the date group given an entry date and reference today date.
    pub fn from_date(entry_date: NaiveDate, today: NaiveDate) -> Self {
        if entry_date == today {
            Self::Today
        } else if entry_date == today - chrono::Duration::days(1) {
            Self::Yesterday
        } else {
            Self::Earlier
        }
    }

    /// Determines the date group given UTC timestamps.
    pub fn from_timestamp(dt: &DateTime<Utc>, now: DateTime<Utc>) -> Self {
        let entry_date = dt.with_timezone(&Local).date_naive();
        let today = now.with_timezone(&Local).date_naive();
        Self::from_date(entry_date, today)
    }
}

/// Groups historical time entries preserving their 0-based indices in the source list.
pub fn group_history_entries<'a>(
    entries: &'a [TimeEntry],
    now: DateTime<Utc>,
) -> Vec<(DateGroup, Vec<(usize, &'a TimeEntry)>)> {
    let today = now.with_timezone(&Local).date_naive();
    let mut groups: Vec<(DateGroup, Vec<(usize, &'a TimeEntry)>)> = Vec::new();

    for (idx, entry) in entries.iter().enumerate() {
        let entry_date = entry.start_time.with_timezone(&Local).date_naive();
        let group = DateGroup::from_date(entry_date, today);

        if let Some((last_group, list)) = groups.last_mut() {
            if *last_group == group {
                list.push((idx, entry));
                continue;
            }
        }
        groups.push((group, vec![(idx, entry)]));
    }

    groups
}

/// Renders the complete History Timesheet screen into `area`.
pub fn render_history_view(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Study History & Timesheet ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if app.history_filter.is_some() { 1 } else { 0 }), // Filter banner
            Constraint::Min(4),                                                   // Timesheet table
            Constraint::Length(1), // Hotkey hints bar
        ])
        .split(inner);

    // 1. Optional active filter banner
    if let Some(query) = &app.history_filter {
        let filter_line = Line::from(vec![
            Span::styled("  🔍 Filtered: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("\"{query}\""),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " (showing {} of {} entries)   [Esc] Clear filter   [/] Modify",
                    app.filtered_history_entries().len(),
                    app.history_entries.len()
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(Paragraph::new(filter_line), chunks[0]);
    }

    // 2. Table of entries or empty state message
    let filtered = app.filtered_history_entries();
    if filtered.is_empty() {
        let empty_text = if app.history_filter.is_some() {
            "  No history entries matched your filter. Press [/] to change or [Esc] to clear."
        } else {
            "  No study sessions recorded yet. Press [n] to create a manual entry or [Space] on Timer tab."
        };
        let p = Paragraph::new(vec![Line::from(""), Line::from(empty_text)])
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, chunks[1]);
    } else {
        render_timesheet_table(app, &filtered, frame, chunks[1]);
    }

    // 3. Hotkey hints bar
    let hints_line = Line::from(vec![
        Span::styled(
            " [n] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("New Manual Entry   "),
        Span::styled(
            "[e] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Edit Entry   "),
        Span::styled(
            "[d] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Delete Entry   "),
        Span::styled(
            "[/] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Filter   "),
        Span::styled(
            "[j/k] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Navigate"),
    ]);
    frame.render_widget(
        Paragraph::new(hints_line).alignment(Alignment::Center),
        chunks[2],
    );
}

/// Convenience alias matching view conventions.
pub fn history_view(app: &App, frame: &mut Frame, area: Rect) {
    render_history_view(app, frame, area);
}

/// Renders the timesheet table grouped by date sections.
fn render_timesheet_table(
    app: &App,
    filtered_entries: &[&TimeEntry],
    frame: &mut Frame,
    area: Rect,
) {
    let now = Utc::now();
    let today = now.with_timezone(&Local).date_naive();

    let header = Row::new([
        "  Date / Time",
        "Duration",
        "Project",
        "Tags",
        "Description",
        "Sync",
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::UNDERLINED),
    );

    let mut rows: Vec<Row> = Vec::new();
    let mut current_group: Option<DateGroup> = None;

    let total = filtered_entries.len();
    let capacity = (area.height.saturating_sub(4) as usize).max(4);
    let (start_idx, end_idx) = if total <= capacity {
        (0, total)
    } else {
        let half = capacity / 2;
        let start = if app.selected_history_index < half {
            0
        } else if app.selected_history_index + (capacity - half) >= total {
            total.saturating_sub(capacity)
        } else {
            app.selected_history_index - half
        };
        let end = (start + capacity).min(total);
        (start, end)
    };

    if start_idx > 0 {
        rows.push(
            Row::new([
                Cell::from(Span::styled(
                    format!("  ▲ ... {} earlier records ...", start_idx),
                    Style::default().fg(Color::DarkGray),
                )),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().fg(Color::DarkGray)),
        );
    }

    for (rel_idx, entry) in filtered_entries[start_idx..end_idx].iter().enumerate() {
        let idx = start_idx + rel_idx;
        let entry_date = entry.start_time.with_timezone(&Local).date_naive();
        let group = DateGroup::from_date(entry_date, today);

        // Section header divider
        if current_group != Some(group) {
            current_group = Some(group);
            rows.push(
                Row::new([
                    Cell::from(Span::styled(
                        group.header_text(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                ])
                .style(Style::default().fg(Color::Cyan)),
            );
        }

        let is_selected = idx == app.selected_history_index;
        let prefix = if is_selected { "▶ " } else { "  " };

        let time_str = match entry.end_time {
            Some(end) => {
                let start_fmt = entry.start_time.with_timezone(&Local).format("%H:%M");
                let end_fmt = end.with_timezone(&Local).format("%H:%M");
                if group == DateGroup::Today {
                    format!("{prefix}{start_fmt} - {end_fmt}")
                } else {
                    let date_fmt = entry.start_time.with_timezone(&Local).format("%b %d");
                    format!("{prefix}{date_fmt} {start_fmt}")
                }
            }
            None => {
                let start_fmt = entry.start_time.with_timezone(&Local).format("%H:%M");
                format!("{prefix}{start_fmt} - active")
            }
        };

        let duration_str = match entry.duration() {
            Some(d) => crate::domain::format_duration_human(&d),
            None => entry.format_human_duration_until(now),
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

        let sync_span = if entry.synced {
            Span::styled("✓", Style::default().fg(Color::Green))
        } else {
            Span::styled("⟳", Style::default().fg(Color::DarkGray))
        };

        let row_style = if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        rows.push(
            Row::new([
                Cell::from(time_str),
                Cell::from(duration_str),
                Cell::from(Span::styled(
                    proj_name,
                    Style::default().fg(parse_hex_color(proj_color)),
                )),
                Cell::from(Span::styled(tags_str, Style::default().fg(Color::DarkGray))),
                Cell::from(desc_str),
                Cell::from(sync_span),
            ])
            .style(row_style),
        );
    }

    if end_idx < total {
        rows.push(
            Row::new([
                Cell::from(Span::styled(
                    format!("  ▼ ... {} more records below ...", total - end_idx),
                    Style::default().fg(Color::DarkGray),
                )),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().fg(Color::DarkGray)),
        );
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(17), // Date / Time
            Constraint::Length(10), // Duration
            Constraint::Length(16), // Project
            Constraint::Length(18), // Tags
            Constraint::Min(20),    // Description
            Constraint::Length(6),  // Sync status
        ],
    )
    .header(header);

    frame.render_widget(table, area);
}

/// Renders the modal dialog for creating or editing an entry.
pub fn render_entry_form_modal(app: &App, frame: &mut Frame, is_new: bool) {
    let title = if is_new {
        "New Manual Time Entry"
    } else {
        "Edit Time Entry"
    };

    let form = &app.entry_form;

    let modal = if form.active_field == 1 {
        Modal::new(title)
            .width_percent(70)
            .height_percent(65)
            .border_color(Color::Cyan)
            .with_hotkey("[↑/↓]", "Pick Previous")
            .with_hotkey("[Ctrl+F]", "FZF Search")
            .with_hotkey("[Enter]", "Select")
            .with_hotkey("[Tab]", "Next")
    } else {
        Modal::new(title)
            .width_percent(70)
            .height_percent(65)
            .border_color(Color::Cyan)
            .with_hotkey("[Tab/↓]", "Next")
            .with_hotkey("[Enter]", "Save")
            .with_hotkey("[Esc]", "Cancel")
    };

    let inner = modal.render_frame(frame);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let fields = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Description
            Constraint::Length(3), // Project
            Constraint::Length(3), // Duration / Time
            Constraint::Length(3), // Tags
            Constraint::Min(1),    // Formatting hint
        ])
        .split(inner);

    // Field 0: Description
    render_input_field(
        frame,
        fields[0],
        "Description",
        &form.description,
        form.active_field == 0,
        "e.g. Linear Algebra Problem Set",
    );

    // Field 1: Project
    render_input_field(
        frame,
        fields[1],
        "Project / Subject",
        &form.project,
        form.active_field == 1,
        "e.g. Mathematics",
    );

    // Field 2: Duration / Time
    render_input_field(
        frame,
        fields[2],
        "Duration or Time Range",
        &form.duration,
        form.active_field == 2,
        "e.g. 1h 30m, 45m, 90, or 14:00 - 15:30",
    );

    // Field 3: Tags
    render_input_field(
        frame,
        fields[3],
        "Tags (comma-separated)",
        &form.tags,
        form.active_field == 3,
        "e.g. homework, exam-prep",
    );

    // When Field 1 (Project) is focused, render the floating previous projects dropdown over fields[2..3]
    if form.active_field == 1 && !form.show_fzf_modal {
        let matches = app.fuzzy_filter_projects(&form.project);
        let dropdown_y = fields[1].y + fields[1].height;
        let max_h = (inner.y + inner.height).saturating_sub(dropdown_y);
        if max_h >= 3 {
            let dropdown_h = max_h.min(7).max(3);
            let dropdown_area = Rect {
                x: fields[1].x,
                y: dropdown_y,
                width: fields[1].width,
                height: dropdown_h,
            };
            frame.render_widget(Clear, dropdown_area);

            let dd_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(Span::styled(
                    " Previous Projects (FZF: type to filter · ↑/↓ pick · Enter apply · Ctrl+F search) ",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
            let dd_inner = dd_block.inner(dropdown_area);
            frame.render_widget(dd_block, dropdown_area);

            let visible_count = dd_inner.height as usize;
            let selected = form.selected_project_option.min(matches.len().saturating_sub(1));
            let scroll_top = if selected >= visible_count {
                selected + 1 - visible_count
            } else {
                0
            };

            let mut lines = Vec::new();
            if matches.is_empty() {
                if form.project.trim().is_empty() {
                    lines.push(Line::from(Span::styled(
                        "  No previous projects found. Type a new name to create one.",
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "  [New Project] ",
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("Press Tab to create and use \"{}\"", form.project),
                            Style::default().fg(Color::White),
                        ),
                    ]));
                }
            } else {
                for (i, (proj, fuzzy_match)) in matches
                    .iter()
                    .enumerate()
                    .skip(scroll_top)
                    .take(visible_count)
                {
                    let is_sel = i == selected;
                    let prefix = if is_sel { "> " } else { "  " };
                    let p_color = parse_hex_color(&proj.color);

                    let mut line_spans = vec![
                        Span::styled(
                            prefix,
                            if is_sel {
                                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            },
                        ),
                        Span::styled("● ", Style::default().fg(p_color)),
                    ];

                    for (c_idx, c) in proj.name.chars().enumerate() {
                        if fuzzy_match.matched_indices.contains(&c_idx) {
                            line_spans.push(Span::styled(
                                c.to_string(),
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                            ));
                        } else {
                            line_spans.push(Span::styled(
                                c.to_string(),
                                if is_sel {
                                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(Color::Gray)
                                },
                            ));
                        }
                    }

                    let meta = if proj.target_hours_week > 0.0 {
                        format!("  ({:.1}h/wk · {}x)", proj.target_hours_week, proj.usage_count)
                    } else if proj.usage_count > 0 {
                        format!("  ({}x used)", proj.usage_count)
                    } else {
                        String::new()
                    };
                    if !meta.is_empty() {
                        line_spans.push(Span::styled(meta, Style::default().fg(Color::DarkGray)));
                    }

                    let mut line = Line::from(line_spans);
                    if is_sel {
                        line = line.style(Style::default().bg(Color::Rgb(30, 45, 65)));
                    }
                    lines.push(line);
                }
            }

            frame.render_widget(Paragraph::new(lines), dd_inner);
        }
    }

    // Bottom helper note
    let hint_text = if form.active_field == 1 {
        Paragraph::new(
            "💡 [↑/↓] Select from previous projects · [Ctrl+F] Full FZF modal · [Enter] Apply project · [Tab] Next field",
        )
        .style(Style::default().fg(Color::Cyan))
    } else {
        Paragraph::new(
            "💡 Enter duration as '1h 30m' or time range as '14:00 - 15:30'. New projects/tags are created automatically.",
        )
        .style(Style::default().fg(Color::DarkGray))
    };
    frame.render_widget(hint_text, fields[4]);

    // Dedicated FZF Modal on top
    if form.show_fzf_modal {
        render_project_fzf_modal(app, frame);
    }
}

/// Renders the dedicated FZF fuzzy finder modal for picking from previous projects.
pub fn render_project_fzf_modal(app: &App, frame: &mut Frame) {
    let area = centered_rect(65, 55, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Span::styled(
            " 🔍 Select Previous Project (FZF) ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search input
            Constraint::Min(3),    // Matches list
            Constraint::Length(1), // Footer hotkeys
        ])
        .split(inner);

    // Search input
    let query = &app.entry_form.fzf_query;
    let search_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(" Search / Filter ", Style::default().fg(Color::Cyan)));
    let search_inner = search_block.inner(chunks[0]);
    frame.render_widget(search_block, chunks[0]);

    let search_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(query, Style::default().fg(Color::White)),
        Span::styled("▌", Style::default().fg(Color::Yellow)),
    ]);
    frame.render_widget(Paragraph::new(search_line), search_inner);

    // Filtered matches
    let matches = app.fuzzy_filter_projects(query);
    let list_inner = chunks[1];
    let visible_count = list_inner.height as usize;
    let selected = app.entry_form.fzf_selected_index.min(matches.len().saturating_sub(1));
    let scroll_top = if selected >= visible_count {
        selected + 1 - visible_count
    } else {
        0
    };

    let mut lines = Vec::new();
    if matches.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                "  No matching previous projects found. Press Enter to use \"",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(query, Style::default().fg(Color::White)),
            Span::styled("\".", Style::default().fg(Color::DarkGray)),
        ]));
    } else {
        for (i, (proj, fuzzy_match)) in matches
            .iter()
            .enumerate()
            .skip(scroll_top)
            .take(visible_count)
        {
            let is_sel = i == selected;
            let prefix = if is_sel { "> " } else { "  " };
            let p_color = parse_hex_color(&proj.color);

            let mut line_spans = vec![
                Span::styled(
                    prefix,
                    if is_sel {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled("● ", Style::default().fg(p_color)),
            ];

            for (c_idx, c) in proj.name.chars().enumerate() {
                if fuzzy_match.matched_indices.contains(&c_idx) {
                    line_spans.push(Span::styled(
                        c.to_string(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ));
                } else {
                    line_spans.push(Span::styled(
                        c.to_string(),
                        if is_sel {
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Gray)
                        },
                    ));
                }
            }

            let meta = if proj.target_hours_week > 0.0 {
                format!("  ({:.1}h/wk · {}x)", proj.target_hours_week, proj.usage_count)
            } else {
                format!("  ({}x used)", proj.usage_count)
            };
            line_spans.push(Span::styled(meta, Style::default().fg(Color::DarkGray)));

            let mut line = Line::from(line_spans);
            if is_sel {
                line = line.style(Style::default().bg(Color::Rgb(30, 45, 65)));
            }
            lines.push(line);
        }
    }

    frame.render_widget(Paragraph::new(lines), list_inner);

    // Footer hotkeys
    let footer_text = Line::from(vec![
        Span::styled("[Enter]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" Select  ", Style::default().fg(Color::Gray)),
        Span::styled(
            "[↑/↓ / Ctrl+j/k]",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Navigate  ", Style::default().fg(Color::Gray)),
        Span::styled("[Esc]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" Cancel", Style::default().fg(Color::Gray)),
    ]);
    frame.render_widget(Paragraph::new(footer_text), chunks[2]);
}

/// Renders a single input field with title and cursor.
fn render_input_field(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    is_focused: bool,
    placeholder: &str,
) {
    let border_color = if is_focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let title_style = if is_focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(format!(" {label} "), title_style));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let display_text = if value.is_empty() {
        if is_focused {
            Line::from(vec![
                Span::styled(placeholder, Style::default().fg(Color::DarkGray)),
                Span::styled("▌", Style::default().fg(Color::Yellow)),
            ])
        } else {
            Line::from(Span::styled(
                placeholder,
                Style::default().fg(Color::DarkGray),
            ))
        }
    } else if is_focused {
        Line::from(vec![
            Span::styled(value, Style::default().fg(Color::White)),
            Span::styled("▌", Style::default().fg(Color::Yellow)),
        ])
    } else {
        Line::from(Span::styled(value, Style::default().fg(Color::White)))
    };

    let p = Paragraph::new(display_text);
    frame.render_widget(p, inner);
}

/// Renders the confirmation modal for deleting an entry.
pub fn render_delete_modal(app: &App, frame: &mut Frame) {
    let popup_area = centered_rect(60, 35, frame.area());
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(Span::styled(
            " Confirm Deletion ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let filtered = app.filtered_history_entries();
    let entry_desc = if !filtered.is_empty() && app.selected_history_index < filtered.len() {
        let entry = filtered[app.selected_history_index];
        let duration = entry
            .format_human_duration()
            .unwrap_or_else(|| "active".to_string());
        format!("\"{}\" ({})", entry.description, duration)
    } else {
        "selected entry".to_string()
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Are you sure you want to delete this entry?",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Target: "),
            Span::styled(entry_desc, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  [y / Enter] ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Confirm    "),
            Span::styled(
                "[n / Esc] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("Cancel"),
        ]),
    ];

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}

/// Renders the filter prompt modal dialog.
pub fn render_filter_modal(app: &App, frame: &mut Frame) {
    let popup_area = centered_rect(60, 30, frame.area());
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Filter History Entries ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Prompt
            Constraint::Length(3), // Input box
            Constraint::Length(1), // Hints
        ])
        .split(inner);

    let prompt = Paragraph::new("Enter search keyword (matches description, project, or tags):")
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(prompt, chunks[0]);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let input_inner = input_block.inner(chunks[1]);
    frame.render_widget(input_block, chunks[1]);

    let display_line = Line::from(vec![
        Span::styled(&app.filter_input, Style::default().fg(Color::White)),
        Span::styled("▌", Style::default().fg(Color::Yellow)),
    ]);
    frame.render_widget(Paragraph::new(display_line), input_inner);

    let hints = Line::from(vec![
        Span::styled(
            "[Enter] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Apply filter    "),
        Span::styled(
            "[Esc] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Clear / Cancel"),
    ]);
    frame.render_widget(Paragraph::new(hints), chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_group_categorization() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        let yesterday = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
        let earlier = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();

        assert_eq!(DateGroup::from_date(today, today), DateGroup::Today);
        assert_eq!(DateGroup::from_date(yesterday, today), DateGroup::Yesterday);
        assert_eq!(DateGroup::from_date(earlier, today), DateGroup::Earlier);

        assert_eq!(DateGroup::Today.header_text(), "── Today ──");
        assert_eq!(DateGroup::Yesterday.header_text(), "── Yesterday ──");
        assert_eq!(DateGroup::Earlier.header_text(), "── Earlier ──");
    }
}
