//! Tab 4 - Analytics & Study Streaks view, consistency tracking, and Pomodoro metrics.

use chrono::Datelike;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::domain::{format_duration_human, EntryMode};
use crate::tui::app::App;
use crate::tui::views::timer::parse_hex_color;

/// Formats a streak badge string with flame icon.
pub fn format_streak_badge(streak: u32) -> String {
    if streak == 1 {
        "🔥 1 Day".to_string()
    } else {
        format!("🔥 {streak} Days")
    }
}

/// Returns the status label and color for today's study activity.
pub fn format_streak_status(studied_today: bool) -> (&'static str, Color) {
    if studied_today {
        ("✓ Studied today", Color::Green)
    } else {
        ("✗ Not yet studied today", Color::Yellow)
    }
}

/// Formats an ASCII day bar chart string for horizontal bar display.
pub fn format_day_bar(day: &str, hours: f64, max_hours: f64, bar_width: usize) -> String {
    let ratio = if max_hours > 0.0 {
        (hours / max_hours).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = (ratio * bar_width as f64).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    format!("{day}: {bar} {hours:.1}h")
}

/// Formats a textual progress gauge like `[██████░░░░]`.
pub fn format_gauge(ratio: f64, width: usize) -> String {
    let clamped = ratio.clamp(0.0, 1.0);
    let filled = (clamped * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

/// Calculates Pomodoro focus efficiency ratio (focus time / total time).
pub fn format_efficiency_ratio(focus_mins: i64, break_mins: i64) -> f64 {
    let total = focus_mins + break_mins;
    if total > 0 {
        (focus_mins as f64 / total as f64).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Formats decimal hours into human-readable duration like `4h 12m`.
pub fn format_hours_human(hours: f64) -> String {
    if hours <= 0.0 || hours.is_nan() {
        return "0h 00m".to_string();
    }
    let total_mins = (hours * 60.0).round() as i64;
    let h = total_mins / 60;
    let m = total_mins % 60;
    format!("{h}h {m:02}m")
}

/// Renders the complete Study Analytics & Streaks screen into `area`.
pub fn render_analytics_view(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Study Analytics & Streaks ",
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
            Constraint::Min(4),    // Cards Grid / Stacked Area
            Constraint::Length(1), // Hotkeys hints bar
        ])
        .split(inner);

    render_cards(app, frame, chunks[0]);
    render_hints_bar(frame, chunks[1]);
}

/// Convenience alias matching view conventions.
pub fn analytics_view(app: &App, frame: &mut Frame, area: Rect) {
    render_analytics_view(app, frame, area);
}

/// Renders the 2x2 grid (or stacked layout for narrow/short terminals).
fn render_cards(app: &App, frame: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    // Only stack vertically when terminal is narrow (< 60 cols).
    // When height is short, a 2x2 grid preserves more rows per card than 4-way vertical splitting.
    let is_stacked = area.width < 60;

    if is_stacked {
        let card_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
            ])
            .split(area);

        render_streak_card(app, frame, card_rows[0]);
        render_weekly_hours_card(app, frame, card_rows[1]);
        render_subject_distribution_card(app, frame, card_rows[2]);
        render_pomodoro_metrics_card(app, frame, card_rows[3]);
    } else {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(area);

        let top_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(rows[0]);

        let bottom_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(rows[1]);

        render_streak_card(app, frame, top_cols[0]);
        render_weekly_hours_card(app, frame, top_cols[1]);
        render_subject_distribution_card(app, frame, bottom_cols[0]);
        render_pomodoro_metrics_card(app, frame, bottom_cols[1]);
    }
}

/// Card 1 (Top-Left): Study Streak & Consistency.
fn render_streak_card(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Streak & Consistency ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let (current_streak, longest_streak, studied_today) = match &app.streak_stats {
        Some(stats) => (
            stats.current_streak,
            stats.longest_streak,
            stats.studied_today,
        ),
        None => (0, 0, false),
    };

    let streak_str = format_streak_badge(current_streak);
    let best_str = if longest_streak == 1 {
        "🏆 Best Streak: 1 Day".to_string()
    } else {
        format!("🏆 Best Streak: {longest_streak} Days")
    };

    let (status_text, status_color) = format_streak_status(studied_today);

    let today = chrono::Local::now().date_naive();
    let days_from_monday = today.weekday().num_days_from_monday();
    let monday = today - chrono::Duration::days(days_from_monday as i64);
    let sunday = monday + chrono::Duration::days(6);

    let days_studied_week = app
        .daily_summaries
        .iter()
        .filter(|s| s.date >= monday && s.date <= sunday && s.duration > chrono::Duration::zero())
        .count()
        .min(7);
    let consistency_pct = (days_studied_week as f64 / 7.0 * 100.0).round() as u32;

    let mut lines = Vec::new();

    if inner.height <= 2 {
        lines.push(Line::from(vec![
            Span::styled(
                streak_str,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                best_str,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(
                status_text,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({days_studied_week}/7d {consistency_pct}%)"),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled(
                streak_str,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Current Streak", Style::default().fg(Color::White)),
        ]));

        lines.push(Line::from(vec![Span::styled(
            best_str,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]));

        lines.push(Line::from(vec![Span::styled(
            status_text,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        )]));

        let consistency_label = if inner.width < 39 {
            format!("Consistency: {days_studied_week}/7d ({consistency_pct}%)")
        } else {
            format!("Consistency: {days_studied_week}/7 days this week ({consistency_pct}%)")
        };

        lines.push(Line::from(vec![Span::styled(
            consistency_label,
            Style::default().fg(Color::Gray),
        )]));
    }

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}

/// Card 2 (Top-Right): Weekly Study Hours (Mon-Sun Bar Chart).
fn render_weekly_hours_card(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Weekly Study Hours ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let today = chrono::Local::now().date_naive();
    let days_from_monday = today.weekday().num_days_from_monday();
    let monday = today - chrono::Duration::days(days_from_monday as i64);

    let day_labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut day_hours = [0.0f64; 7];

    for (i, hours_slot) in day_hours.iter_mut().enumerate() {
        let target_date = monday + chrono::Duration::days(i as i64);
        if let Some(summary) = app.daily_summaries.iter().find(|s| s.date == target_date) {
            *hours_slot = summary.duration.num_seconds() as f64 / 3600.0;
        }
    }

    let total_weekly_hours: f64 = day_hours.iter().sum();
    let max_hours = day_hours.iter().cloned().fold(0.0f64, f64::max).max(1.0);

    let mut lines = Vec::new();

    if inner.height >= 8 {
        // Vertical stack of 7 days + 1 summary line
        let bar_width: usize = if inner.width < 32 {
            6
        } else if inner.width < 45 {
            10
        } else {
            14
        };

        for (i, label) in day_labels.iter().enumerate() {
            let h = day_hours[i];
            let ratio = (h / max_hours).clamp(0.0, 1.0);
            let filled = (ratio * bar_width as f64).round() as usize;
            let empty = bar_width.saturating_sub(filled);

            lines.push(Line::from(vec![
                Span::styled(format!("{label}: "), Style::default().fg(Color::Gray)),
                Span::styled(
                    "█".repeat(filled),
                    Style::default().fg(if h > 0.0 {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled("░".repeat(empty), Style::default().fg(Color::DarkGray)),
                Span::styled(format!(" {:4.1}h", h), Style::default().fg(Color::White)),
            ]));
        }

        let summary_text = format!(
            "Total: {:.1}h ({})",
            total_weekly_hours,
            format_hours_human(total_weekly_hours)
        );
        lines.push(Line::from(vec![Span::styled(
            summary_text,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]));
    } else {
        // Compact 2-column view fitting in 4 lines:
        // Line 0: Mon ... │ Fri ...
        // Line 1: Tue ... │ Sat ...
        // Line 2: Wed ... │ Sun ...
        // Line 3: Thu ... │ Total: X.Xh
        let bar_width: usize = if inner.width < 36 { 3 } else { 5 };

        for row in 0..4 {
            let left_idx = row;
            let left_day = day_labels[left_idx];
            let left_h = day_hours[left_idx];
            let left_ratio = (left_h / max_hours).clamp(0.0, 1.0);
            let left_filled = (left_ratio * bar_width as f64).round() as usize;
            let left_empty = bar_width.saturating_sub(left_filled);

            let mut spans = vec![
                Span::styled(format!("{left_day}: "), Style::default().fg(Color::Gray)),
                Span::styled(
                    "█".repeat(left_filled),
                    Style::default().fg(if left_h > 0.0 {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled("░".repeat(left_empty), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!(" {:3.1}h", left_h),
                    Style::default().fg(Color::White),
                ),
                Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            ];

            if row < 3 {
                let right_idx = row + 4;
                let right_day = day_labels[right_idx];
                let right_h = day_hours[right_idx];
                let right_ratio = (right_h / max_hours).clamp(0.0, 1.0);
                let right_filled = (right_ratio * bar_width as f64).round() as usize;
                let right_empty = bar_width.saturating_sub(right_filled);

                spans.push(Span::styled(
                    format!("{right_day}: "),
                    Style::default().fg(Color::Gray),
                ));
                spans.push(Span::styled(
                    "█".repeat(right_filled),
                    Style::default().fg(if right_h > 0.0 {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ));
                spans.push(Span::styled(
                    "░".repeat(right_empty),
                    Style::default().fg(Color::DarkGray),
                ));
                spans.push(Span::styled(
                    format!(" {:3.1}h", right_h),
                    Style::default().fg(Color::White),
                ));
            } else {
                spans.push(Span::styled(
                    "Total: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!("{:.1}h", total_weekly_hours),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            lines.push(Line::from(spans));
        }
    }

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}

/// Card 3 (Bottom-Left): Subject Distribution.
fn render_subject_distribution_card(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Subject Distribution ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    if app.subject_breakdown.is_empty() {
        let mut text = vec![];
        if inner.height > 2 {
            text.push(Line::from(""));
        }
        text.push(Line::from(Span::styled(
            "No study data recorded this week",
            Style::default().fg(Color::DarkGray),
        )));
        if inner.height > 3 {
            text.push(Line::from(Span::styled(
                "Log time to see subject distribution",
                Style::default().fg(Color::DarkGray),
            )));
        }
        let p = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(p, inner);
        return;
    }

    let mut subjects = app.subject_breakdown.clone();
    subjects.sort_by(|a, b| {
        b.percentage
            .partial_cmp(&a.percentage)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let max_rows = inner.height as usize;
    let gauge_width: usize = if inner.width < 36 {
        5
    } else if inner.width < 45 {
        7
    } else {
        10
    };
    // Non-name content: badge (2) + space (1) + gauge (gauge_width + 2) + space (1) + pct (6) + space/dur (approx 8)
    let non_name_width = 2 + 1 + gauge_width + 2 + 1 + 6 + 8;
    let name_width = (inner.width as usize)
        .saturating_sub(non_name_width)
        .max(10);

    let show_more = subjects.len() > max_rows && max_rows > 1;
    let display_count = if show_more {
        max_rows - 1
    } else {
        subjects.len().min(max_rows)
    };

    let mut lines = Vec::new();

    for s in subjects.iter().take(display_count) {
        let color = parse_hex_color(&s.color);

        let truncated_name = if s.project_name.chars().count() > name_width {
            let prefix: String = s.project_name.chars().take(name_width - 1).collect();
            format!("{prefix}…")
        } else {
            format!("{:<width$}", s.project_name, width = name_width)
        };

        let ratio = (s.percentage / 100.0).clamp(0.0, 1.0);
        let filled = (ratio * gauge_width as f64).round() as usize;
        let empty = gauge_width.saturating_sub(filled);

        let dur_str = format_duration_human(&s.duration);

        let spans = vec![
            Span::styled("● ", Style::default().fg(color)),
            Span::styled(truncated_name, Style::default().fg(Color::White)),
            Span::raw(" "),
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled("█".repeat(filled), Style::default().fg(color)),
            Span::styled("░".repeat(empty), Style::default().fg(Color::DarkGray)),
            Span::styled("] ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:4.1}%", s.percentage),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(format!(" ({dur_str})"), Style::default().fg(Color::Gray)),
        ];

        lines.push(Line::from(spans));
    }

    if show_more {
        let remaining = subjects.len() - display_count;
        lines.push(Line::from(Span::styled(
            format!("  ... and {remaining} more subjects"),
            Style::default().fg(Color::DarkGray),
        )));
    }

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}

/// Card 4 (Bottom-Right): Pomodoro Session Metrics.
fn render_pomodoro_metrics_card(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Pomodoro Session Metrics ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let (completed_today, completed_this_week, total_focus_mins) = match &app.pomodoro_stats {
        Some(stats) => (
            stats.completed_today,
            stats.completed_this_week,
            stats.total_focus_mins,
        ),
        None => (0, 0, 0),
    };

    let today = chrono::Local::now().date_naive();
    let days_from_monday = today.weekday().num_days_from_monday();
    let monday = today - chrono::Duration::days(days_from_monday as i64);
    let sunday = monday + chrono::Duration::days(6);
    let monday_dt = monday.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let sunday_end_dt = (sunday + chrono::Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();

    let mut break_mins: i64 = 0;
    for entry in &app.history_entries {
        if entry.entry_mode == EntryMode::PomodoroBreak
            && entry.start_time >= monday_dt
            && entry.start_time < sunday_end_dt
        {
            if let Some(dur) = entry.duration() {
                break_mins += dur.num_minutes();
            }
        }
    }
    for entry in &app.today_entries {
        if entry.entry_mode == EntryMode::PomodoroBreak {
            let already_counted = app
                .history_entries
                .iter()
                .any(|h| h.id == entry.id && h.id.is_some());
            if !already_counted {
                if let Some(dur) = entry.duration() {
                    break_mins += dur.num_minutes();
                }
            }
        }
    }

    if break_mins == 0 && completed_this_week > 0 {
        break_mins = (completed_this_week as i64) * (app.config.pomodoro.short_break_mins as i64);
    }

    let efficiency_ratio = format_efficiency_ratio(total_focus_mins, break_mins);
    let efficiency_pct = efficiency_ratio * 100.0;

    let focus_dur = chrono::Duration::minutes(total_focus_mins);
    let break_dur = chrono::Duration::minutes(break_mins);
    let focus_str = format_duration_human(&focus_dur);
    let break_str = format_duration_human(&break_dur);

    let mut lines = Vec::new();

    // Line 0: Sessions count
    lines.push(Line::from(vec![
        Span::styled("Sessions: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{completed_today} today"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{completed_this_week} this week"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Line 1: Focus vs Break time
    lines.push(Line::from(vec![
        Span::styled("Focus: ", Style::default().fg(Color::Gray)),
        Span::styled(
            focus_str,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ Break: ", Style::default().fg(Color::Gray)),
        Span::styled(
            break_str,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Line 2: Efficiency gauge
    let eff_bar_width: usize = if inner.width < 34 { 6 } else { 8 };
    let eff_filled = (efficiency_ratio * eff_bar_width as f64).round() as usize;
    let eff_empty = eff_bar_width.saturating_sub(eff_filled);

    lines.push(Line::from(vec![
        Span::styled("Efficiency: ", Style::default().fg(Color::Gray)),
        Span::styled("[", Style::default().fg(Color::DarkGray)),
        Span::styled("█".repeat(eff_filled), Style::default().fg(Color::Cyan)),
        Span::styled("░".repeat(eff_empty), Style::default().fg(Color::DarkGray)),
        Span::styled("] ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{efficiency_pct:.1}%"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // Line 3: Today's target / tip
    if inner.height >= 4 {
        if completed_today > 0 || completed_this_week > 0 {
            let (label, note) = if inner.width < 42 {
                let n = if completed_today >= 8 { " (Done)" } else { "" };
                (format!("Target: {completed_today}/8"), n)
            } else {
                let n = if completed_today >= 8 {
                    " (Target met! 🎉)"
                } else {
                    " (In progress)"
                };
                (format!("Daily Target: {completed_today}/8 sessions"), n)
            };
            lines.push(Line::from(vec![
                Span::styled(label, Style::default().fg(Color::Gray)),
                Span::styled(
                    note,
                    Style::default().fg(if completed_today >= 8 {
                        Color::Green
                    } else {
                        Color::Yellow
                    }),
                ),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                "Tip: Start Pomodoro in Tab [1]",
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}

/// Renders the hotkeys bar at the bottom.
fn render_hints_bar(frame: &mut Frame, area: Rect) {
    let hints: &[(&str, &str)] = &[
        ("[1-4]", " Switch Tabs   "),
        ("[r]", " Refresh Data   "),
        ("[?]", " Help   "),
        ("[q]", " Quit"),
    ];

    let spans: Vec<Span> = hints
        .iter()
        .flat_map(|(key, desc)| {
            vec![
                Span::styled(
                    *key,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*desc, Style::default().fg(Color::Gray)),
            ]
        })
        .collect();

    let p = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(p, area);
}
