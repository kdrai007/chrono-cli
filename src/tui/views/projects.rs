//! Tab 3 - Projects & Course Targets view and interactive management modals.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::domain::DEFAULT_PROJECT_COLOR;
use crate::tui::app::App;
use crate::tui::views::timer::parse_hex_color;
use crate::tui::widgets::modal::{centered_rect, Modal};

/// Palette of recommended project colors for quick visual selection.
pub const RECOMMENDED_PALETTE: &[(&str, &str)] = &[
    ("#3498db", "Blue"),
    ("#2ecc71", "Green"),
    ("#e74c3c", "Red"),
    ("#9b59b6", "Purple"),
    ("#f39c12", "Orange"),
    ("#1abc9c", "Teal"),
    ("#e67e22", "Carrot"),
    ("#34495e", "Slate"),
];

/// Formats a textual progress bar e.g. `[████████░░] 62.5%`.
///
/// If ratio is >= 1.0, the bar fills to 100% capacity with the actual percentage shown.
pub fn format_progress_bar(ratio: f64, bar_width: usize) -> String {
    let clamped_ratio = if ratio.is_nan() || ratio < 0.0 {
        0.0
    } else {
        ratio
    };

    let filled = ((clamped_ratio * bar_width as f64).round() as usize).min(bar_width);
    let empty = bar_width.saturating_sub(filled);

    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    let pct = clamped_ratio * 100.0;
    format!("[{bar}] {pct:.1}%")
}

/// Returns the status badge label and color for a project based on weekly target and actual hours.
pub fn get_status_badge(target_hours: f64, actual_hours: f64) -> (&'static str, Color) {
    if target_hours <= 0.0 {
        ("[No Target]", Color::DarkGray)
    } else {
        let ratio = actual_hours / target_hours;
        if ratio >= 1.0 {
            ("[Exceeded]", Color::Green)
        } else if ratio >= 0.50 {
            ("[On Track]", Color::Cyan)
        } else {
            ("[Behind]", Color::Red)
        }
    }
}

/// Returns the color indicator for progress based on completion percentage.
pub fn get_progress_color(target_hours: f64, actual_hours: f64) -> Color {
    if target_hours <= 0.0 {
        Color::DarkGray
    } else {
        let ratio = actual_hours / target_hours;
        if ratio >= 0.80 {
            Color::Green
        } else if ratio >= 0.50 {
            Color::Yellow
        } else {
            Color::Red
        }
    }
}

/// Formats decimal hours into a human-readable duration like `6h 15m`.
pub fn format_hours_human(hours: f64) -> String {
    if hours <= 0.0 || hours.is_nan() {
        return "0h 00m".to_string();
    }

    let total_mins = (hours * 60.0).round() as i64;
    let h = total_mins / 60;
    let m = total_mins % 60;
    format!("{h}h {m:02}m")
}

/// Renders the complete Projects & Course Targets screen into `area`.
pub fn render_projects_view(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Projects & Course Targets ",
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
            Constraint::Length(3), // Top Summary Header
            Constraint::Min(4),    // Projects list / table
            Constraint::Length(1), // Hotkey hints bar
        ])
        .split(inner);

    render_summary_header(app, frame, chunks[0]);

    if app.project_list.is_empty() {
        render_empty_state(frame, chunks[1]);
    } else {
        render_projects_table(app, frame, chunks[1]);
    }

    render_hints_bar(frame, chunks[2]);
}

/// Convenience alias matching view conventions.
pub fn projects_view(app: &App, frame: &mut Frame, area: Rect) {
    render_projects_view(app, frame, area);
}

/// Renders the top summary header card with aggregated metrics.
fn render_summary_header(app: &App, frame: &mut Frame, area: Rect) {
    let total_projects = app.project_list.len();
    let active_projects = app.project_list.iter().filter(|p| !p.archived).count();
    let total_target_hours: f64 = app
        .project_list
        .iter()
        .filter(|p| !p.archived)
        .map(|p| p.target_hours_week)
        .sum();
    let total_logged_hours: f64 = app
        .project_progress
        .values()
        .map(|p| p.actual_hours_week)
        .sum();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Weekly Target Overview ",
            Style::default().fg(Color::Yellow),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let summary_line = Line::from(vec![
        Span::styled(" Total Courses: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{total_projects}"),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   │   Active: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{active_projects}"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "   │   Weekly Target: ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{total_target_hours:.1}h"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "   │   Logged This Week: ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format_hours_human(total_logged_hours),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let p = Paragraph::new(summary_line).alignment(Alignment::Center);
    frame.render_widget(p, inner);
}

/// Renders the empty state message when no projects exist.
fn render_empty_state(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  No projects or academic courses configured yet.",
            Style::default().fg(Color::White),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[a]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " to create a new project with weekly study targets!",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let p = Paragraph::new(text);
    frame.render_widget(p, area);
}

/// Renders the table of projects with progress indicators and scrolling.
fn render_projects_table(app: &App, frame: &mut Frame, area: Rect) {
    let header = Row::new([
        "  Project / Course",
        "Target",
        "Logged (Week)",
        "Progress",
        "Status",
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::UNDERLINED),
    );

    let total = app.project_list.len();
    let capacity = (area.height.saturating_sub(4) as usize).max(3);
    let (start_idx, end_idx) = if total <= capacity {
        (0, total)
    } else {
        let half = capacity / 2;
        let start = if app.selected_project_index < half {
            0
        } else if app.selected_project_index + (capacity - half) >= total {
            total.saturating_sub(capacity)
        } else {
            app.selected_project_index - half
        };
        let end = (start + capacity).min(total);
        (start, end)
    };

    let mut rows: Vec<Row> = Vec::new();

    if start_idx > 0 {
        rows.push(
            Row::new([
                Cell::from(Span::styled(
                    format!("  ▲ ... {start_idx} earlier projects ..."),
                    Style::default().fg(Color::DarkGray),
                )),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().fg(Color::DarkGray)),
        );
    }

    for (idx, project) in app.project_list[start_idx..end_idx].iter().enumerate() {
        let global_idx = start_idx + idx;
        let is_selected = global_idx == app.selected_project_index;

        let prefix = if is_selected { "▶ " } else { "  " };

        let badge_color = parse_hex_color(&project.color);
        let mut proj_spans = vec![
            Span::styled(
                prefix,
                if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled("■ ", Style::default().fg(badge_color)),
            Span::styled(
                &project.name,
                if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            ),
        ];

        if project.archived {
            proj_spans.push(Span::styled(
                " [ARCHIVED]",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ));
        }

        let target_str = if project.target_hours_week > 0.0 {
            format!("{:.1}h", project.target_hours_week)
        } else {
            "None".to_string()
        };

        let actual_hours = project
            .id
            .and_then(|pid| app.project_progress.get(&pid))
            .map(|p| p.actual_hours_week)
            .unwrap_or(0.0);

        let logged_str = format_hours_human(actual_hours);

        let ratio = if project.target_hours_week > 0.0 {
            actual_hours / project.target_hours_week
        } else {
            0.0
        };

        let progress_str = if project.target_hours_week > 0.0 {
            format_progress_bar(ratio, 10)
        } else {
            "[──────────] None".to_string()
        };
        let progress_color = get_progress_color(project.target_hours_week, actual_hours);

        let (status_text, status_color) = get_status_badge(project.target_hours_week, actual_hours);

        let row_style = if is_selected {
            Style::default()
                .bg(Color::Rgb(30, 42, 56))
                .add_modifier(Modifier::BOLD)
        } else if project.archived {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        rows.push(
            Row::new([
                Cell::from(Line::from(proj_spans)),
                Cell::from(Span::styled(target_str, Style::default().fg(Color::Gray))),
                Cell::from(Span::styled(logged_str, Style::default().fg(Color::White))),
                Cell::from(Span::styled(
                    progress_str,
                    Style::default().fg(progress_color),
                )),
                Cell::from(Span::styled(
                    status_text,
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                )),
            ])
            .style(row_style),
        );
    }

    if end_idx < total {
        rows.push(
            Row::new([
                Cell::from(Span::styled(
                    format!("  ▼ ... {} more projects below ...", total - end_idx),
                    Style::default().fg(Color::DarkGray),
                )),
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
            Constraint::Percentage(32), // Project / Course name
            Constraint::Length(10),     // Target
            Constraint::Length(16),     // Logged
            Constraint::Length(24),     // Progress bar
            Constraint::Length(14),     // Status badge
        ],
    )
    .header(header);

    frame.render_widget(table, area);
}

/// Renders the bottom hotkey hints bar for Tab 3.
fn render_hints_bar(frame: &mut Frame, area: Rect) {
    let hints_line = Line::from(vec![
        Span::styled(
            " [a] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Add Project   "),
        Span::styled(
            "[e] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Edit Target   "),
        Span::styled(
            "[x] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Toggle Archive   "),
        Span::styled(
            "[d] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Delete   "),
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
        area,
    );
}

/// Renders a single input field with title, value, and cursor.
fn render_input_field(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    is_focused: bool,
    placeholder: &str,
    badge: Option<(&str, Color)>,
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

    let mut spans = Vec::new();

    if let Some((b_text, b_color)) = badge {
        spans.push(Span::styled(b_text, Style::default().fg(b_color)));
        spans.push(Span::raw(" "));
    }

    if value.is_empty() {
        if is_focused {
            spans.push(Span::styled(
                placeholder,
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::styled("▌", Style::default().fg(Color::Yellow)));
        } else {
            spans.push(Span::styled(
                placeholder,
                Style::default().fg(Color::DarkGray),
            ));
        }
    } else if is_focused {
        spans.push(Span::styled(value, Style::default().fg(Color::White)));
        spans.push(Span::styled("▌", Style::default().fg(Color::Yellow)));
    } else {
        spans.push(Span::styled(value, Style::default().fg(Color::White)));
    }

    let p = Paragraph::new(Line::from(spans));
    frame.render_widget(p, inner);
}

/// Renders the modal dialog for creating or editing a project and course targets.
pub fn render_project_form_modal(app: &App, frame: &mut Frame, is_new: bool) {
    let title = if is_new {
        "Add Project / Course"
    } else {
        "Edit Project Target & Color"
    };

    let modal = Modal::new(title)
        .width_percent(65)
        .height_percent(58)
        .border_color(Color::Cyan)
        .with_hotkey("[Tab/↓]", "Next")
        .with_hotkey("[Enter]", "Save")
        .with_hotkey("[Esc]", "Cancel");

    let inner = modal.render_frame(frame);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let fields = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Name
            Constraint::Length(3), // Color
            Constraint::Length(3), // Target hours
            Constraint::Min(2),    // Palette hints
        ])
        .split(inner);

    let form = &app.project_form;

    // Field 0: Project Name
    render_input_field(
        frame,
        fields[0],
        "Course / Project Name",
        &form.name,
        form.active_field == 0,
        "e.g. CS101, Algorithms, Thesis",
        None,
    );

    // Field 1: Hex Color Code
    let color_str = if form.color.is_empty() {
        DEFAULT_PROJECT_COLOR
    } else {
        &form.color
    };
    let parsed_color = parse_hex_color(color_str);

    render_input_field(
        frame,
        fields[1],
        "Badge Hex Color (#RRGGBB)",
        &form.color,
        form.active_field == 1,
        "e.g. #3498db, #2ecc71, #e74c3c",
        Some(("■", parsed_color)),
    );

    // Field 2: Weekly Target Hours
    render_input_field(
        frame,
        fields[2],
        "Weekly Target Hours (e.g. 10.0 or 0 for none)",
        &form.target_hours,
        form.active_field == 2,
        "e.g. 10.0, 15, 5.5",
        None,
    );

    // Field 3: Palette notes
    let mut palette_spans = vec![Span::styled(
        "🎨 Palette: ",
        Style::default().fg(Color::Yellow),
    )];
    for (hex, name) in RECOMMENDED_PALETTE {
        let c = parse_hex_color(hex);
        palette_spans.push(Span::styled("■ ", Style::default().fg(c)));
        palette_spans.push(Span::styled(
            format!("{name} "),
            Style::default().fg(Color::Gray),
        ));
    }

    let note_text = vec![
        Line::from(palette_spans),
        Line::from(Span::styled(
            "💡 Leave color blank for default blue (#3498db). Target hours can be decimal.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(note_text);
    frame.render_widget(p, fields[3]);
}

/// Renders the confirmation modal for deleting a project.
pub fn render_delete_project_modal(app: &App, frame: &mut Frame) {
    let popup_area = centered_rect(60, 35, frame.area());
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(Span::styled(
            " Confirm Project Deletion ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let target_name = app
        .selected_project()
        .map(|p| p.name.as_str())
        .unwrap_or("selected project");

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Are you sure you want to delete this project?",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Project: "),
            Span::styled(
                format!("\"{target_name}\""),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            "  (Logged time entries will remain in history, unlinked)",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  [y / Enter] ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Confirm Delete    "),
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
