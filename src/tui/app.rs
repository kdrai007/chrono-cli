//! Application state and navigation models for chrono-cli.

use std::collections::HashMap;
use std::time::Instant;

use chrono::{DateTime, Datelike, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::AppConfig;
use crate::domain::{
    DailySummary, EntryMode, PomodoroStateMachine, PomodoroStats, Project, ProjectTargetProgress,
    StreakStats, SubjectBreakdown, Tag, TimeEntry,
};
use crate::notify::{NotificationEvent, NotificationService};
use crate::storage::Database;

/// Available primary navigation tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub enum Tab {
    /// Active stopwatch and pomodoro timer view.
    #[default]
    Timer = 0,
    /// Historical study sessions and logged time entries.
    History = 1,
    /// Course and project management.
    Projects = 2,
    /// Statistics, streaks, and subject breakdown.
    Analytics = 3,
}

impl Tab {
    /// All tabs in fixed ordering.
    pub const ALL: [Tab; 4] = [Tab::Timer, Tab::History, Tab::Projects, Tab::Analytics];

    /// Returns the 0-based tab index.
    pub fn index(self) -> usize {
        self as usize
    }

    /// Converts a 0-based index to a `Tab` variant.
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Tab::Timer),
            1 => Some(Tab::History),
            2 => Some(Tab::Projects),
            3 => Some(Tab::Analytics),
            _ => None,
        }
    }

    /// Human-readable title of the tab.
    pub fn title(self) -> &'static str {
        match self {
            Tab::Timer => "Timer",
            Tab::History => "History",
            Tab::Projects => "Projects",
            Tab::Analytics => "Analytics",
        }
    }

    /// Advances to the next tab in circular fashion.
    pub fn next(self) -> Self {
        match self {
            Tab::Timer => Tab::History,
            Tab::History => Tab::Projects,
            Tab::Projects => Tab::Analytics,
            Tab::Analytics => Tab::Timer,
        }
    }

    /// Moves to the previous tab in circular fashion.
    pub fn prev(self) -> Self {
        match self {
            Tab::Timer => Tab::Analytics,
            Tab::History => Tab::Timer,
            Tab::Projects => Tab::History,
            Tab::Analytics => Tab::Projects,
        }
    }
}

/// Parses a duration string or time range into a `chrono::Duration`.
///
/// Supported formats:
/// - `"1h 30m"`, `"1h"`, `"45m"`, `"30s"`, `"1.5h"`
/// - Plain number `"45"` (treated as minutes)
/// - Clock format `"01:30:00"` or `"01:30"`
/// - Range format `"14:00 - 15:30"` or `"14:00-15:30"`
pub fn parse_duration_input(s: &str) -> chrono::Duration {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return chrono::Duration::minutes(30);
    }

    // Check if it's a range like "14:00 - 15:30" or "14:00-15:30"
    if let Some((start_s, end_s)) = trimmed.split_once('-') {
        let start_parts: Vec<&str> = start_s.trim().split(':').collect();
        let end_parts: Vec<&str> = end_s.trim().split(':').collect();
        if start_parts.len() >= 2 && end_parts.len() >= 2 {
            if let (Ok(sh), Ok(sm), Ok(eh), Ok(em)) = (
                start_parts[0].parse::<i64>(),
                start_parts[1].parse::<i64>(),
                end_parts[0].parse::<i64>(),
                end_parts[1].parse::<i64>(),
            ) {
                let start_mins = sh * 60 + sm;
                let mut end_mins = eh * 60 + em;
                if end_mins < start_mins {
                    end_mins += 24 * 60; // spanned midnight
                }
                let diff = (end_mins - start_mins).max(0);
                return chrono::Duration::minutes(diff);
            }
        }
    }

    // Check "HH:MM:SS" or "HH:MM"
    if trimmed.contains(':') {
        let parts: Vec<&str> = trimmed.split(':').collect();
        if parts.len() == 3 {
            if let (Ok(h), Ok(m), Ok(sec)) = (
                parts[0].parse::<i64>(),
                parts[1].parse::<i64>(),
                parts[2].parse::<i64>(),
            ) {
                return chrono::Duration::hours(h)
                    + chrono::Duration::minutes(m)
                    + chrono::Duration::seconds(sec);
            }
        } else if parts.len() == 2 {
            if let (Ok(h), Ok(m)) = (parts[0].parse::<i64>(), parts[1].parse::<i64>()) {
                return chrono::Duration::hours(h) + chrono::Duration::minutes(m);
            }
        }
    }

    // Check decimal hours like "1.5h"
    if (trimmed.ends_with('h') || trimmed.ends_with('H')) && trimmed.contains('.') {
        let num_str = trimmed[..trimmed.len() - 1].trim();
        if let Ok(hours_f) = num_str.parse::<f64>() {
            let total_secs = (hours_f * 3600.0).max(0.0) as i64;
            return chrono::Duration::seconds(total_secs);
        }
    }

    // Token based: "1h 30m"
    let mut total_secs: i64 = 0;
    let mut num_buf = String::new();
    let mut has_units = false;

    for c in trimmed.chars() {
        if c.is_ascii_digit() {
            num_buf.push(c);
        } else if c.is_whitespace() {
            // spacer
        } else {
            let unit = c.to_ascii_lowercase();
            if let Ok(n) = num_buf.parse::<i64>() {
                match unit {
                    'h' => {
                        total_secs += n * 3600;
                        has_units = true;
                    }
                    'm' => {
                        total_secs += n * 60;
                        has_units = true;
                    }
                    's' => {
                        total_secs += n;
                        has_units = true;
                    }
                    _ => {}
                }
            }
            num_buf.clear();
        }
    }

    if has_units {
        return chrono::Duration::seconds(total_secs.max(0));
    }

    // If pure number like "45", treat as minutes
    if let Ok(n) = trimmed.parse::<i64>() {
        return chrono::Duration::minutes(n.max(0));
    }

    chrono::Duration::minutes(30)
}

/// Previous project option with metadata for fuzzy search and selection.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSuggestion {
    pub name: String,
    pub color: String,
    pub target_hours_week: f64,
    pub usage_count: usize,
}

/// Form state for creating or editing time entries via modal dialogs.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryForm {
    /// Local database ID if editing an existing entry.
    pub id: Option<i64>,
    /// Task description or study session title.
    pub description: String,
    /// Associated project name.
    pub project: String,
    /// Duration or time string (e.g. "1h 15m", "45m", "90", "14:00 - 15:30").
    pub duration: String,
    /// Comma-separated tag names (e.g. "homework, math").
    pub tags: String,
    /// Currently focused form field (0 = description, 1 = project, 2 = duration, 3 = tags).
    pub active_field: usize,
    /// Selected index in project suggestions list (for navigation via Up/Down/Ctrl+J/Ctrl+K).
    pub selected_project_option: usize,
    /// Whether the dedicated FZF fuzzy finder modal overlay is open.
    pub show_fzf_modal: bool,
    /// Search query string in the FZF modal.
    pub fzf_query: String,
    /// Selected index inside the FZF modal.
    pub fzf_selected_index: usize,
}

impl Default for EntryForm {
    fn default() -> Self {
        Self {
            id: None,
            description: String::new(),
            project: String::new(),
            duration: "30m".to_string(),
            tags: String::new(),
            active_field: 0,
            selected_project_option: 0,
            show_fzf_modal: false,
            fzf_query: String::new(),
            fzf_selected_index: 0,
        }
    }
}

impl EntryForm {
    /// Creates an `EntryForm` pre-populated from an existing `TimeEntry`.
    pub fn from_entry(entry: &TimeEntry, projects: &HashMap<i64, Project>) -> Self {
        let project_name = entry
            .project_id
            .and_then(|pid| projects.get(&pid))
            .map(|p| p.name.clone())
            .unwrap_or_default();

        let duration_str = if let Some(d) = entry.duration() {
            crate::domain::format_duration_human(&d)
        } else {
            "30m".to_string()
        };

        let tags_str = entry
            .tags
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        Self {
            id: entry.id,
            description: entry.description.clone(),
            project: project_name,
            duration: duration_str,
            tags: tags_str,
            active_field: 0,
            selected_project_option: 0,
            show_fzf_modal: false,
            fzf_query: String::new(),
            fzf_selected_index: 0,
        }
    }

    /// Resets project selection indices and FZF state.
    pub fn reset_project_selection(&mut self) {
        self.selected_project_option = 0;
        self.show_fzf_modal = false;
        self.fzf_query.clear();
        self.fzf_selected_index = 0;
    }

    /// Returns a mutable reference to the string buffer for the currently focused field.
    pub fn active_field_mut(&mut self) -> &mut String {
        match self.active_field {
            0 => &mut self.description,
            1 => &mut self.project,
            2 => &mut self.duration,
            _ => &mut self.tags,
        }
    }
}

/// Form state for creating or editing projects and course targets via modal dialogs.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectForm {
    /// Local database ID if editing an existing project.
    pub id: Option<i64>,
    /// Project display name.
    pub name: String,
    /// Hex color code for UI badges (e.g. "#3498db").
    pub color: String,
    /// Weekly study target hours (e.g. "10.0" or "0").
    pub target_hours: String,
    /// Currently focused form field (0 = name, 1 = color, 2 = target_hours).
    pub active_field: usize,
}

impl Default for ProjectForm {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            color: crate::domain::DEFAULT_PROJECT_COLOR.to_string(),
            target_hours: "10.0".to_string(),
            active_field: 0,
        }
    }
}

impl ProjectForm {
    /// Creates a `ProjectForm` pre-populated from an existing `Project`.
    pub fn from_project(project: &Project) -> Self {
        let target_str = if project.target_hours_week > 0.0 {
            format!("{:.1}", project.target_hours_week)
        } else {
            String::new()
        };

        Self {
            id: project.id,
            name: project.name.clone(),
            color: project.color.clone(),
            target_hours: target_str,
            active_field: 0,
        }
    }

    /// Returns a mutable reference to the string buffer for the currently focused field.
    pub fn active_field_mut(&mut self) -> &mut String {
        match self.active_field {
            0 => &mut self.name,
            1 => &mut self.color,
            _ => &mut self.target_hours,
        }
    }
}

/// Central application state for the TUI.
#[derive(Debug)]
pub struct App {
    /// Currently focused view tab.
    pub current_tab: Tab,
    /// Main event loop execution flag. Set to `false` on exit.
    pub running: bool,
    /// Currently running time entry, if any.
    pub active_entry: Option<TimeEntry>,
    /// Resolved name of the project associated with `active_entry`.
    pub active_project_name: Option<String>,
    /// State machine controlling Pomodoro intervals and cycles.
    pub pomodoro: PomodoroStateMachine,
    /// Desktop notification and terminal bell dispatcher.
    pub notifications: NotificationService,
    /// Whether the help modal dialog is displayed.
    pub show_help: bool,
    /// Temporary banner notification message with creation timestamp.
    pub status_message: Option<(String, Instant)>,
    /// Application settings.
    pub config: AppConfig,
    /// Today's logged time entries loaded from the database.
    pub today_entries: Vec<TimeEntry>,
    /// Index of the selected recent entry in the Timer view list.
    pub selected_recent_index: usize,
    /// Historical study sessions loaded from the database.
    pub history_entries: Vec<TimeEntry>,
    /// Index of the selected entry in the History timesheet table.
    pub selected_history_index: usize,
    /// Whether the manual entry creation modal dialog is displayed.
    pub show_new_entry_modal: bool,
    /// Whether the entry editing modal dialog is displayed.
    pub show_edit_entry_modal: bool,
    /// Whether the delete confirmation modal dialog is displayed.
    pub show_delete_entry_modal: bool,
    /// Whether the history search/filter modal dialog is displayed.
    pub show_filter_modal: bool,
    /// Form data for manual entry creation and editing.
    pub entry_form: EntryForm,
    /// Optional active text filter applied to historical study sessions.
    pub history_filter: Option<String>,
    /// Buffer holding current text input for the filter prompt.
    pub filter_input: String,
    /// Cached map of projects keyed by project database ID.
    pub projects: HashMap<i64, Project>,
    /// Course and project records loaded from the database.
    pub project_list: Vec<Project>,
    /// Index of the selected project in the Projects view table.
    pub selected_project_index: usize,
    /// Whether the add project modal dialog is displayed.
    pub show_add_project_modal: bool,
    /// Whether the edit project modal dialog is displayed.
    pub show_edit_project_modal: bool,
    /// Whether the delete project confirmation modal dialog is displayed.
    pub show_delete_project_modal: bool,
    /// Form data for project creation and editing.
    pub project_form: ProjectForm,
    /// Cached weekly project target progress metrics keyed by project database ID.
    pub project_progress: HashMap<i64, ProjectTargetProgress>,
    /// Study streak statistics.
    pub streak_stats: Option<StreakStats>,
    /// Daily study summaries for the current week.
    pub daily_summaries: Vec<DailySummary>,
    /// Study time breakdown by subject/project for the current week.
    pub subject_breakdown: Vec<SubjectBreakdown>,
    /// Pomodoro focus session statistics for the current week.
    pub pomodoro_stats: Option<PomodoroStats>,
}

impl App {
    /// Creates a new `App` instance initialized with the given configuration.
    pub fn new(config: AppConfig) -> Self {
        let pomodoro = PomodoroStateMachine::from_config(&config.pomodoro);
        let notifications = NotificationService::new(config.general.clone());

        Self {
            current_tab: Tab::Timer,
            running: true,
            active_entry: None,
            active_project_name: None,
            pomodoro,
            notifications,
            show_help: false,
            status_message: None,
            config,
            today_entries: Vec::new(),
            selected_recent_index: 0,
            history_entries: Vec::new(),
            selected_history_index: 0,
            show_new_entry_modal: false,
            show_edit_entry_modal: false,
            show_delete_entry_modal: false,
            show_filter_modal: false,
            entry_form: EntryForm::default(),
            history_filter: None,
            filter_input: String::new(),
            projects: HashMap::new(),
            project_list: Vec::new(),
            selected_project_index: 0,
            show_add_project_modal: false,
            show_edit_project_modal: false,
            show_delete_project_modal: false,
            project_form: ProjectForm::default(),
            project_progress: HashMap::new(),
            streak_stats: None,
            daily_summaries: Vec::new(),
            subject_breakdown: Vec::new(),
            pomodoro_stats: None,
        }
    }

    /// Builder method to attach an initial active time entry and project name.
    pub fn with_active_entry(
        mut self,
        entry: Option<TimeEntry>,
        project_name: Option<String>,
    ) -> Self {
        self.active_entry = entry;
        self.active_project_name = project_name;
        self
    }

    /// Builder method to attach today's entries.
    pub fn with_today_entries(mut self, entries: Vec<TimeEntry>) -> Self {
        self.today_entries = entries;
        self
    }

    /// Builder method to attach history entries.
    pub fn with_history_entries(mut self, entries: Vec<TimeEntry>) -> Self {
        self.history_entries = entries;
        self
    }

    /// Builder method to attach cached projects.
    pub fn with_projects(mut self, projects: HashMap<i64, Project>) -> Self {
        self.projects = projects;
        self
    }

    /// Builder method to attach a project list.
    pub fn with_project_list(mut self, list: Vec<Project>) -> Self {
        self.project_list = list;
        self
    }

    /// Builder method to attach cached project target progress metrics.
    pub fn with_project_progress(mut self, progress: HashMap<i64, ProjectTargetProgress>) -> Self {
        self.project_progress = progress;
        self
    }

    /// Builder method to attach streak statistics.
    pub fn with_streak_stats(mut self, stats: Option<StreakStats>) -> Self {
        self.streak_stats = stats;
        self
    }

    /// Builder method to attach daily summaries.
    pub fn with_daily_summaries(mut self, summaries: Vec<DailySummary>) -> Self {
        self.daily_summaries = summaries;
        self
    }

    /// Builder method to attach subject breakdown.
    pub fn with_subject_breakdown(mut self, breakdown: Vec<SubjectBreakdown>) -> Self {
        self.subject_breakdown = breakdown;
        self
    }

    /// Builder method to attach pomodoro stats.
    pub fn with_pomodoro_stats(mut self, stats: Option<PomodoroStats>) -> Self {
        self.pomodoro_stats = stats;
        self
    }

    /// Sets the active time entry and optional project name.
    pub fn set_active_entry(&mut self, entry: Option<TimeEntry>, project_name: Option<String>) {
        self.active_entry = entry;
        self.active_project_name = project_name;
    }

    /// Advances to the next tab.
    pub fn next_tab(&mut self) {
        self.current_tab = self.current_tab.next();
    }

    /// Moves to the previous tab.
    pub fn prev_tab(&mut self) {
        self.current_tab = self.current_tab.prev();
    }

    /// Sets the active tab directly.
    pub fn set_tab(&mut self, tab: Tab) {
        self.current_tab = tab;
    }

    /// Signals the application to terminate.
    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Toggles the help reference popup.
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    /// Sets a temporary status notice banner.
    pub fn set_status_message(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    /// Clears any active status notice banner.
    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    /// Periodic tick handler (expires status messages older than 4 seconds).
    pub fn tick(&mut self) {
        if let Some((_, instant)) = &self.status_message {
            if instant.elapsed() >= std::time::Duration::from_secs(4) {
                self.status_message = None;
            }
        }
    }

    /// Refreshes today's entries and cached projects from the database.
    pub fn refresh_today_entries(&mut self, db: &Database) {
        let today_utc = Utc::now().date_naive();
        let today_local = chrono::Local::now().date_naive();
        let start_date = today_utc.min(today_local);
        let end_date = today_utc.max(today_local) + chrono::Duration::days(1);
        let start_dt = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_dt = end_date.and_hms_opt(0, 0, 0).unwrap().and_utc();

        if let Ok(entries) = db.get_entries(start_dt, end_dt) {
            self.today_entries = entries;
            let count = self.today_entries.len().min(5);
            if count > 0 && self.selected_recent_index >= count {
                self.selected_recent_index = count - 1;
            } else if count == 0 {
                self.selected_recent_index = 0;
            }
        }

        if let Ok(projects) = db.list_projects(true) {
            self.projects = projects
                .into_iter()
                .filter_map(|p| p.id.map(|id| (id, p)))
                .collect();
        }

        self.active_entry = db.get_active_entry().ok().flatten();
        if let Some(entry) = &self.active_entry {
            if let Some(pid) = entry.project_id {
                self.active_project_name = self.projects.get(&pid).map(|p| p.name.clone());
            } else {
                self.active_project_name = None;
            }
        } else {
            self.active_project_name = None;
        }

        self.refresh_history(db);
        self.refresh_projects(db);
        self.refresh_analytics(db);
    }

    /// Refreshes all historical time entries and cached projects from the database.
    pub fn refresh_history(&mut self, db: &Database) {
        let start_dt = DateTime::<Utc>::UNIX_EPOCH;
        let end_dt = Utc::now() + chrono::Duration::days(36500);

        if let Ok(entries) = db.get_entries(start_dt, end_dt) {
            self.history_entries = entries;
            let count = self.filtered_history_entries().len();
            if count > 0 && self.selected_history_index >= count {
                self.selected_history_index = count - 1;
            } else if count == 0 {
                self.selected_history_index = 0;
            }
        }

        if let Ok(projects) = db.list_projects(true) {
            self.projects = projects
                .into_iter()
                .filter_map(|p| p.id.map(|id| (id, p)))
                .collect();
        }

        self.refresh_projects(db);
    }

    /// Refreshes projects and weekly study targets progress from the database.
    pub fn refresh_projects(&mut self, db: &Database) {
        if let Ok(projects) = db.list_projects(true) {
            self.project_list = projects;
            let count = self.project_list.len();
            if count > 0 && self.selected_project_index >= count {
                self.selected_project_index = count - 1;
            } else if count == 0 {
                self.selected_project_index = 0;
            }
        }

        self.projects = self
            .project_list
            .iter()
            .filter_map(|p| p.id.map(|id| (id, p.clone())))
            .collect();

        let today = chrono::Local::now().date_naive();
        let days_from_monday = today.weekday().num_days_from_monday();
        let monday = today - chrono::Duration::days(days_from_monday as i64);
        let sunday = monday + chrono::Duration::days(6);

        if let Ok(progress_list) = db.get_weekly_project_progress(monday, sunday) {
            self.project_progress = progress_list
                .into_iter()
                .map(|p| (p.project_id, p))
                .collect();
        }

        if let Some(entry) = &self.active_entry {
            if let Some(pid) = entry.project_id {
                self.active_project_name = self.projects.get(&pid).map(|p| p.name.clone());
            } else {
                self.active_project_name = None;
            }
        }
    }

    /// Refreshes streak statistics, daily summaries, subject breakdowns, and Pomodoro metrics.
    pub fn refresh_analytics(&mut self, db: &Database) {
        self.refresh_history(db);
        let today = chrono::Local::now().date_naive();
        let days_from_monday = today.weekday().num_days_from_monday();
        let monday = today - chrono::Duration::days(days_from_monday as i64);
        let sunday = monday + chrono::Duration::days(6);

        if let Ok(streak) = db.get_streak_stats(today) {
            self.streak_stats = Some(streak);
        }
        if let Ok(summaries) = db.get_daily_summaries(monday, sunday) {
            self.daily_summaries = summaries;
        }
        if let Ok(breakdown) = db.get_subject_breakdown(monday, sunday) {
            self.subject_breakdown = breakdown;
        }
        if let Ok(pomo) = db.get_pomodoro_stats(today, monday) {
            self.pomodoro_stats = Some(pomo);
        }
    }

    /// Triggers Clockify cloud synchronization if configured.
    pub fn sync_clockify(&mut self, db: &mut Database) {
        if !self.config.clockify.enabled
            || self.config.clockify.api_key.trim().is_empty()
            || self.config.clockify.workspace_id.trim().is_empty()
        {
            self.set_status_message("Sync disabled: configure api_key in config.toml");
            return;
        }

        self.set_status_message("Syncing with Clockify...");
        match crate::clockify::ClockifyClient::new(&self.config.clockify.api_key) {
            Ok(client) => {
                match crate::clockify::SyncEngine::sync(
                    &client,
                    db,
                    &self.config.clockify.workspace_id,
                ) {
                    Ok(result) => {
                        self.refresh_today_entries(db);
                        self.refresh_history(db);
                        self.refresh_projects(db);
                        self.refresh_analytics(db);
                        self.set_status_message(format!(
                            "Sync done: {} pushed, {} pulled",
                            result.pushed_entries, result.pulled_projects
                        ));
                    }
                    Err(e) => {
                        self.set_status_message(format!("Sync failed: {e}"));
                    }
                }
            }
            Err(e) => {
                self.set_status_message(format!("Clockify error: {e}"));
            }
        }
    }

    /// Moves the recent sessions list selection down.
    pub fn select_next_recent(&mut self) {
        let count = self.today_entries.len().min(5);
        if count > 0 && self.selected_recent_index + 1 < count {
            self.selected_recent_index += 1;
        }
    }

    /// Moves the recent sessions list selection up.
    pub fn select_prev_recent(&mut self) {
        if self.selected_recent_index > 0 {
            self.selected_recent_index -= 1;
        }
    }

    /// Moves the history timesheet selection down.
    pub fn select_next_history(&mut self) {
        let count = self.filtered_history_entries().len();
        if count > 0 && self.selected_history_index + 1 < count {
            self.selected_history_index += 1;
        }
    }

    /// Moves the history timesheet selection up.
    pub fn select_prev_history(&mut self) {
        if self.selected_history_index > 0 {
            self.selected_history_index -= 1;
        }
    }

    /// Moves the projects list selection down.
    pub fn select_next_project(&mut self) {
        let count = self.project_list.len();
        if count > 0 && self.selected_project_index + 1 < count {
            self.selected_project_index += 1;
        }
    }

    /// Moves the projects list selection up.
    pub fn select_prev_project(&mut self) {
        if self.selected_project_index > 0 {
            self.selected_project_index -= 1;
        }
    }

    /// Returns a reference to the currently selected project, if any.
    pub fn selected_project(&self) -> Option<&Project> {
        self.project_list.get(self.selected_project_index)
    }

    /// Deletes the currently selected history entry from the database.
    pub fn delete_selected_history_entry(&mut self, db: &mut Database) {
        let entries = self.filtered_history_entries();
        if entries.is_empty() || self.selected_history_index >= entries.len() {
            self.show_delete_entry_modal = false;
            return;
        }

        let target = entries[self.selected_history_index].clone();
        if let Some(id) = target.id {
            match db.delete_entry(id) {
                Ok(()) => {
                    if self.active_entry.as_ref().and_then(|a| a.id) == Some(id) {
                        self.active_entry = None;
                        self.active_project_name = None;
                    }
                    self.set_status_message(format!("Deleted entry: {}", target.description));
                }
                Err(e) => {
                    self.set_status_message(format!("DB error deleting entry: {e}"));
                }
            }
        }

        self.show_delete_entry_modal = false;
        self.refresh_history(db);
        self.refresh_today_entries(db);
    }

    /// Returns historical entries matching the active filter, or all entries if no filter is set.
    pub fn filtered_history_entries(&self) -> Vec<&TimeEntry> {
        match &self.history_filter {
            Some(query) if !query.trim().is_empty() => {
                let q = query.trim().to_lowercase();
                self.history_entries
                    .iter()
                    .filter(|e| {
                        if e.description.to_lowercase().contains(&q) {
                            return true;
                        }
                        if let Some(pid) = e.project_id {
                            if let Some(p) = self.projects.get(&pid) {
                                if p.name.to_lowercase().contains(&q) {
                                    return true;
                                }
                            }
                        }
                        if e.tags.iter().any(|t| t.name.to_lowercase().contains(&q)) {
                            return true;
                        }
                        false
                    })
                    .collect()
            }
            _ => self.history_entries.iter().collect(),
        }
    }

    /// Returns a reference to the currently selected historical time entry, if any.
    pub fn selected_history_entry(&self) -> Option<&TimeEntry> {
        let filtered = self.filtered_history_entries();
        filtered.get(self.selected_history_index).copied()
    }

    /// Opens the manual new entry creation modal dialog.
    pub fn open_new_entry_modal(&mut self) {
        self.entry_form = EntryForm::default();
        self.show_new_entry_modal = true;
        self.show_edit_entry_modal = false;
        self.show_delete_entry_modal = false;
        self.show_filter_modal = false;
    }

    /// Opens the entry editing modal dialog pre-populated with selected entry details.
    pub fn open_edit_entry_modal(&mut self) {
        let entries = self.filtered_history_entries();
        if entries.is_empty() || self.selected_history_index >= entries.len() {
            return;
        }

        let entry = entries[self.selected_history_index];
        self.entry_form = EntryForm::from_entry(entry, &self.projects);
        self.show_edit_entry_modal = true;
        self.show_new_entry_modal = false;
        self.show_delete_entry_modal = false;
        self.show_filter_modal = false;
    }

    /// Opens the deletion confirmation modal dialog for the selected entry.
    pub fn open_delete_entry_modal(&mut self) {
        let entries = self.filtered_history_entries();
        if entries.is_empty() || self.selected_history_index >= entries.len() {
            return;
        }

        self.show_delete_entry_modal = true;
        self.show_new_entry_modal = false;
        self.show_edit_entry_modal = false;
        self.show_filter_modal = false;
    }

    /// Opens the search and filter prompt modal.
    pub fn open_filter_modal(&mut self) {
        self.filter_input = self.history_filter.clone().unwrap_or_default();
        self.show_filter_modal = true;
    }

    /// Opens the project creation modal dialog.
    pub fn open_add_project_modal(&mut self) {
        self.project_form = ProjectForm::default();
        self.show_add_project_modal = true;
        self.show_edit_project_modal = false;
        self.show_delete_project_modal = false;
    }

    /// Opens the project editing modal dialog pre-populated with selected project details.
    pub fn open_edit_project_modal(&mut self) {
        if let Some(project) = self.selected_project() {
            self.project_form = ProjectForm::from_project(project);
            self.show_edit_project_modal = true;
            self.show_add_project_modal = false;
            self.show_delete_project_modal = false;
        }
    }

    /// Opens the project deletion confirmation modal dialog for the selected project.
    pub fn open_delete_project_modal(&mut self) {
        if self.selected_project().is_some() {
            self.show_delete_project_modal = true;
            self.show_add_project_modal = false;
            self.show_edit_project_modal = false;
        }
    }

    /// Closes all active modal dialogs.
    pub fn close_modal(&mut self) {
        self.show_new_entry_modal = false;
        self.show_edit_entry_modal = false;
        self.entry_form.show_fzf_modal = false;
        self.show_delete_entry_modal = false;
        self.show_filter_modal = false;
        self.show_add_project_modal = false;
        self.show_edit_project_modal = false;
        self.show_delete_project_modal = false;
    }

    /// Returns `true` if any popup or modal dialog is currently active.
    pub fn is_modal_open(&self) -> bool {
        self.show_help
            || self.show_new_entry_modal
            || self.show_edit_entry_modal
            || self.entry_form.show_fzf_modal
            || self.show_delete_entry_modal
            || self.show_filter_modal
            || self.show_add_project_modal
            || self.show_edit_project_modal
            || self.show_delete_project_modal
    }

    /// Returns a deduplicated, ranked list of previous projects from configured courses and history.
    pub fn get_previous_projects(&self) -> Vec<ProjectSuggestion> {
        let mut map: HashMap<String, ProjectSuggestion> = HashMap::new();
        let mut usage_counts: HashMap<String, usize> = HashMap::new();

        // 1. Count usage from historical entries
        for entry in &self.history_entries {
            if let Some(pid) = entry.project_id {
                if let Some(p) = self.projects.get(&pid) {
                    *usage_counts.entry(p.name.clone()).or_insert(0) += 1;
                }
            }
        }

        // 2. Add all configured projects in project_list
        for p in &self.project_list {
            let count = usage_counts.get(&p.name).copied().unwrap_or(0);
            map.insert(
                p.name.clone(),
                ProjectSuggestion {
                    name: p.name.clone(),
                    color: p.color.clone(),
                    target_hours_week: p.target_hours_week,
                    usage_count: count,
                },
            );
        }

        // 3. Add projects from self.projects map not in project_list
        for p in self.projects.values() {
            if !map.contains_key(&p.name) {
                let count = usage_counts.get(&p.name).copied().unwrap_or(0);
                map.insert(
                    p.name.clone(),
                    ProjectSuggestion {
                        name: p.name.clone(),
                        color: p.color.clone(),
                        target_hours_week: p.target_hours_week,
                        usage_count: count,
                    },
                );
            }
        }

        let mut list: Vec<ProjectSuggestion> = map.into_values().collect();
        // Sort: highest usage count first, then by target hours descending, then alphabetically
        list.sort_by(|a, b| {
            b.usage_count
                .cmp(&a.usage_count)
                .then_with(|| {
                    b.target_hours_week
                        .partial_cmp(&a.target_hours_week)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.name.cmp(&b.name))
        });
        list
    }

    /// Fuzzy filters previous projects by `query` using fzf subsequence matching.
    pub fn fuzzy_filter_projects(
        &self,
        query: &str,
    ) -> Vec<(ProjectSuggestion, crate::tui::fuzzy::FuzzyMatch)> {
        let all = self.get_previous_projects();
        let mut matches = Vec::new();
        for proj in all {
            if let Some(m) = crate::tui::fuzzy::fuzzy_match(query, &proj.name) {
                matches.push((proj, m));
            }
        }
        matches.sort_by(|a, b| b.1.score.cmp(&a.1.score));
        matches
    }

    /// Saves the project form (creating a new project or updating an existing one) into SQLite.
    pub fn save_project_form(&mut self, db: &mut Database) -> Result<(), String> {
        let name = self.project_form.name.trim();
        if name.is_empty() {
            return Err("Project name cannot be empty".to_string());
        }

        let color = if self.project_form.color.trim().is_empty() {
            crate::domain::DEFAULT_PROJECT_COLOR.to_string()
        } else {
            let mut c = self.project_form.color.trim().to_string();
            if !c.starts_with('#') {
                c = format!("#{c}");
            }
            let hex_digits = c.trim_start_matches('#');
            if hex_digits.len() != 6
                || !hex_digits.is_ascii()
                || !hex_digits.chars().all(|ch| ch.is_ascii_hexdigit())
            {
                return Err("Color must be a valid 6-character hex code (e.g. #3498db)".to_string());
            }
            c
        };

        let target_hours = if self.project_form.target_hours.trim().is_empty() {
            0.0
        } else {
            self.project_form
                .target_hours
                .trim()
                .parse::<f64>()
                .map_err(|_| "Target hours must be a valid number (e.g. 10.0)".to_string())?
        };

        if !target_hours.is_finite() || target_hours < 0.0 {
            return Err("Weekly target hours must be a finite, non-negative number".to_string());
        }

        if self.show_edit_project_modal {
            if let Some(id) = self.project_form.id {
                match db.get_project(id) {
                    Ok(Some(mut existing)) => {
                        if existing.name != name {
                            if let Ok(Some(other)) = db.get_project_by_name(name) {
                                if other.id != Some(id) {
                                    return Err(format!("Project '{name}' already exists"));
                                }
                            }
                        }
                        existing.name = name.to_string();
                        existing.color = color;
                        existing.target_hours_week = target_hours;
                        if let Err(e) = existing.validate() {
                            return Err(format!("Validation error: {e}"));
                        }
                        if let Err(e) = db.update_project(&existing) {
                            return Err(format!("Failed to update project: {e}"));
                        }
                        self.set_status_message(format!("Updated project: {name}"));
                    }
                    Ok(None) => return Err("Project to edit not found in database".to_string()),
                    Err(e) => return Err(format!("DB error querying project: {e}")),
                }
            } else {
                return Err("No project selected for editing".to_string());
            }
        } else {
            if let Ok(Some(_)) = db.get_project_by_name(name) {
                return Err(format!("Project '{name}' already exists"));
            }
            let project = Project::new(name)
                .map_err(|e| format!("Invalid project name: {e}"))?
                .with_color(color)
                .with_target_hours_week(target_hours);
            if let Err(e) = project.validate() {
                return Err(format!("Validation error: {e}"));
            }
            if let Err(e) = db.create_project(&project) {
                return Err(format!("Failed to create project: {e}"));
            }
            self.set_status_message(format!("Created project: {name}"));
        }

        self.close_modal();
        self.refresh_projects(db);
        self.refresh_history(db);
        self.refresh_today_entries(db);
        Ok(())
    }

    /// Toggles the archive status of the currently selected project in SQLite.
    pub fn toggle_archive_selected_project(&mut self, db: &mut Database) {
        if self.project_list.is_empty() || self.selected_project_index >= self.project_list.len() {
            return;
        }

        let mut target = self.project_list[self.selected_project_index].clone();
        let new_archived = !target.archived;
        target.archived = new_archived;

        match db.update_project(&target) {
            Ok(()) => {
                let status = if new_archived {
                    "Archived"
                } else {
                    "Unarchived"
                };
                self.set_status_message(format!("{status} project: {}", target.name));
            }
            Err(e) => {
                self.set_status_message(format!("DB error updating project: {e}"));
            }
        }

        self.refresh_projects(db);
        self.refresh_history(db);
        self.refresh_today_entries(db);
    }

    /// Deletes the currently selected project from the database.
    pub fn delete_selected_project(&mut self, db: &mut Database) {
        if self.project_list.is_empty() || self.selected_project_index >= self.project_list.len() {
            self.show_delete_project_modal = false;
            return;
        }

        let target = self.project_list[self.selected_project_index].clone();
        if let Some(id) = target.id {
            match db.delete_project(id) {
                Ok(()) => {
                    if self.active_project_name.as_deref() == Some(&target.name) {
                        self.active_project_name = None;
                    }
                    self.set_status_message(format!("Deleted project: {}", target.name));
                }
                Err(e) => {
                    self.set_status_message(format!("DB error deleting project: {e}"));
                }
            }
        }

        self.show_delete_project_modal = false;
        self.refresh_projects(db);
        self.refresh_history(db);
        self.refresh_today_entries(db);
    }

    /// Saves the current entry form (creating a new entry or updating an existing entry) into SQLite.
    pub fn save_entry_form(&mut self, db: &mut Database) -> Result<(), String> {
        let description = if self.entry_form.description.trim().is_empty() {
            "Manual Entry".to_string()
        } else {
            self.entry_form.description.trim().to_string()
        };

        let project_name = self.entry_form.project.trim();
        let project_id = if !project_name.is_empty() {
            match db.get_project_by_name(project_name) {
                Ok(Some(p)) => p.id,
                Ok(None) => match Project::new(project_name) {
                    Ok(new_proj) => match db.create_project(&new_proj) {
                        Ok(created) => created.id,
                        Err(e) => return Err(format!("Failed to create project: {e}")),
                    },
                    Err(e) => return Err(format!("Invalid project name: {e}")),
                },
                Err(e) => return Err(format!("DB error querying project: {e}")),
            }
        } else {
            None
        };

        let mut tags = Vec::new();
        let tag_names: Vec<&str> = self
            .entry_form
            .tags
            .split([',', ';'])
            .map(|t| t.trim().trim_start_matches('#').trim())
            .filter(|t| !t.is_empty())
            .collect();

        for t_name in tag_names {
            match db.get_tag_by_name(t_name) {
                Ok(Some(tag)) => tags.push(tag),
                Ok(None) => match Tag::new(t_name) {
                    Ok(new_tag) => match db.create_tag(&new_tag) {
                        Ok(created) => tags.push(created),
                        Err(e) => return Err(format!("Failed to create tag: {e}")),
                    },
                    Err(e) => return Err(format!("Invalid tag name: {e}")),
                },
                Err(e) => return Err(format!("DB error querying tag: {e}")),
            }
        }

        let duration = parse_duration_input(&self.entry_form.duration);

        if self.show_edit_entry_modal {
            if let Some(id) = self.entry_form.id {
                match db.get_entry(id) {
                    Ok(Some(mut existing)) => {
                        existing.description = description;
                        existing.project_id = project_id;
                        existing.tags = tags;
                        existing.synced = false;
                        existing.end_time = Some(existing.start_time + duration);
                        if let Err(e) = db.update_entry(&existing) {
                            return Err(format!("Failed to update entry: {e}"));
                        }
                        self.set_status_message("Entry updated successfully");
                    }
                    Ok(None) => return Err("Entry to edit not found in database".to_string()),
                    Err(e) => return Err(format!("DB error loading entry: {e}")),
                }
            }
        } else {
            let end_time = Utc::now();
            let start_time = end_time - duration;
            let mut entry = TimeEntry::new(description, start_time);
            entry.end_time = Some(end_time);
            entry.project_id = project_id;
            entry.tags = tags;
            entry.entry_mode = EntryMode::Stopwatch;
            if let Err(e) = db.create_manual_entry(&entry) {
                return Err(format!("Failed to create manual entry: {e}"));
            }
            self.set_status_message("Manual entry created successfully");
        }

        self.close_modal();
        self.refresh_history(db);
        self.refresh_today_entries(db);
        Ok(())
    }

    /// Starts or stops the active timer in the database and updates state.
    pub fn start_stop_timer(&mut self, db: &mut Database) {
        if self.active_entry.is_some() {
            let stopped = match db.stop_active_entry() {
                Ok(s) => s,
                Err(e) => {
                    self.set_status_message(format!("DB error stopping timer: {e}"));
                    None
                }
            };
            self.active_entry = None;
            self.active_project_name = None;

            if let Some(entry) = stopped {
                let desc = if entry.description.is_empty() {
                    "Untitled Session".to_string()
                } else {
                    entry.description.clone()
                };

                if entry.entry_mode == EntryMode::PomodoroWork {
                    let completed = self.pomodoro.current_phase();
                    let next = self.pomodoro.next_phase();
                    let _ = self
                        .notifications
                        .notify_pomodoro_phase_change(&completed, &next);
                } else {
                    let elapsed = entry
                        .format_duration()
                        .unwrap_or_else(|| "00:00:00".to_string());
                    let _ = self.notifications.send(&NotificationEvent::Custom {
                        title: "Timer Stopped".to_string(),
                        body: format!("{desc}: {elapsed}"),
                    });
                }
                self.set_status_message(format!("Stopped timer: {desc}"));
            }
            self.refresh_today_entries(db);
            self.refresh_history(db);
        } else {
            let desc = "Study Session";
            let mut entry = TimeEntry::new(desc, Utc::now());
            entry.entry_mode = EntryMode::Stopwatch;
            match db.start_entry(&entry) {
                Ok(started) => {
                    self.active_entry = Some(started);
                    self.active_project_name = None;
                    self.set_status_message(format!("Started timer: {desc}"));
                    let _ = self.notifications.send(&NotificationEvent::Custom {
                        title: "Timer Started".to_string(),
                        body: format!("Started {desc}"),
                    });
                }
                Err(e) => {
                    self.set_status_message(format!("DB error starting timer: {e}"));
                }
            }
            self.refresh_today_entries(db);
            self.refresh_history(db);
        }
    }

    /// Toggles the Pomodoro mode or advances the Pomodoro phase.
    pub fn toggle_pomodoro_mode(&mut self, db: &mut Database) {
        if let Some(mut entry) = self.active_entry.take() {
            match entry.entry_mode {
                EntryMode::Stopwatch => {
                    let phase = self.pomodoro.current_phase();
                    let new_mode = if phase.is_work() {
                        EntryMode::PomodoroWork
                    } else {
                        EntryMode::PomodoroBreak
                    };
                    entry.entry_mode = new_mode;
                    entry.pomodoro_index = phase.session_index();
                    if let Err(e) = db.update_entry(&entry) {
                        self.set_status_message(format!("DB error updating entry: {e}"));
                    }
                    self.active_entry = Some(entry);
                    self.set_status_message(format!("Switched to Pomodoro: {phase}"));
                }
                EntryMode::PomodoroWork | EntryMode::PomodoroBreak => {
                    let _ = match db.stop_active_entry() {
                        Ok(s) => s,
                        Err(e) => {
                            self.set_status_message(format!("DB error stopping phase: {e}"));
                            None
                        }
                    };
                    let next_phase = self.pomodoro.next_phase();
                    let new_mode = if next_phase.is_work() {
                        EntryMode::PomodoroWork
                    } else {
                        EntryMode::PomodoroBreak
                    };
                    let desc = if new_mode == EntryMode::PomodoroBreak {
                        "Pomodoro Break".to_string()
                    } else {
                        format!("Pomodoro Work #{}", next_phase.session_index())
                    };
                    let mut new_entry = TimeEntry::new(&desc, Utc::now());
                    new_entry.project_id = entry.project_id;
                    new_entry.tags = entry.tags.clone();
                    new_entry.entry_mode = new_mode;
                    new_entry.pomodoro_index = next_phase.session_index();

                    match db.start_entry(&new_entry) {
                        Ok(started) => {
                            self.active_entry = Some(started);
                            self.set_status_message(format!("Pomodoro phase: {next_phase}"));
                        }
                        Err(e) => {
                            self.set_status_message(format!("DB error starting phase: {e}"));
                        }
                    }
                }
            }
            self.refresh_today_entries(db);
            self.refresh_history(db);
        } else {
            let phase = self.pomodoro.current_phase();
            let mode = if phase.is_work() {
                EntryMode::PomodoroWork
            } else {
                EntryMode::PomodoroBreak
            };
            let desc = if mode == EntryMode::PomodoroBreak {
                "Pomodoro Break".to_string()
            } else {
                format!("Pomodoro Work #{}", phase.session_index())
            };
            let mut entry = TimeEntry::new(&desc, Utc::now());
            entry.entry_mode = mode;
            entry.pomodoro_index = phase.session_index();
            match db.start_entry(&entry) {
                Ok(started) => {
                    self.active_entry = Some(started);
                    self.active_project_name = None;
                    self.set_status_message(format!("Started Pomodoro: {phase}"));
                }
                Err(e) => {
                    self.set_status_message(format!("DB error starting pomodoro: {e}"));
                }
            }
            self.refresh_today_entries(db);
            self.refresh_history(db);
        }
    }

    /// Restarts a new timer with the selected entry's project, tags, and description.
    pub fn repeat_selected_entry(&mut self, db: &mut Database) {
        if self.today_entries.is_empty() || self.selected_recent_index >= self.today_entries.len() {
            return;
        }

        let target = self.today_entries[self.selected_recent_index].clone();
        if self.active_entry.is_some() {
            if let Err(e) = db.stop_active_entry() {
                self.set_status_message(format!("DB error stopping active timer: {e}"));
            }
            self.active_entry = None;
        }

        let mut new_entry = TimeEntry::new(&target.description, Utc::now());
        new_entry.project_id = target.project_id;
        new_entry.tags = target.tags.clone();
        new_entry.entry_mode = target.entry_mode;
        new_entry.pomodoro_index = target.pomodoro_index;

        match db.start_entry(&new_entry) {
            Ok(started) => {
                let mut proj_name = None;
                if let Some(pid) = started.project_id {
                    if let Ok(Some(p)) = db.get_project(pid) {
                        proj_name = Some(p.name);
                    }
                }
                self.set_status_message(format!("Repeated session: {}", target.description));
                self.active_entry = Some(started);
                self.active_project_name = proj_name;
            }
            Err(e) => {
                self.set_status_message(format!("DB error repeating entry: {e}"));
            }
        }
        self.refresh_today_entries(db);
        self.refresh_history(db);
    }

    /// Handles keyboard events without database operations.
    pub fn handle_key(&mut self, key: KeyEvent) {
        self.handle_key_inner(key, None);
    }

    /// Handles keyboard events integrating database operations for timer controls.
    pub fn handle_key_with_db(&mut self, key: KeyEvent, db: &mut Database) {
        self.handle_key_inner(key, Some(db));
    }

    fn handle_key_inner(&mut self, key: KeyEvent, db: Option<&mut Database>) {
        // Exit shortcuts: Ctrl+C
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit();
            return;
        }

        if self.show_help {
            match key.code {
                KeyCode::Esc
                | KeyCode::Char('?')
                | KeyCode::Char('q')
                | KeyCode::Enter
                | KeyCode::Char(' ') => {
                    self.show_help = false;
                }
                _ => {}
            }
            return;
        }

        // Intercept keys when modal dialogs are active
        if self.show_delete_entry_modal {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    if let Some(db_ref) = db {
                        self.delete_selected_history_entry(db_ref);
                    } else {
                        self.show_delete_entry_modal = false;
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.show_delete_entry_modal = false;
                }
                _ => {}
            }
            return;
        }

        if self.show_delete_project_modal {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    if let Some(db_ref) = db {
                        self.delete_selected_project(db_ref);
                    } else {
                        self.show_delete_project_modal = false;
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.show_delete_project_modal = false;
                }
                _ => {}
            }
            return;
        }

        if self.show_new_entry_modal || self.show_edit_entry_modal {
            // Sub-case A: Dedicated FZF Fuzzy Finder Modal is open
            if self.entry_form.show_fzf_modal {
                let matches = self.fuzzy_filter_projects(&self.entry_form.fzf_query);
                match key.code {
                    KeyCode::Esc => {
                        self.entry_form.show_fzf_modal = false;
                    }
                    KeyCode::Enter => {
                        if let Some((proj, _)) = matches.get(self.entry_form.fzf_selected_index) {
                            self.entry_form.project = proj.name.clone();
                        } else if !self.entry_form.fzf_query.trim().is_empty() {
                            self.entry_form.project = self.entry_form.fzf_query.trim().to_string();
                        }
                        self.entry_form.show_fzf_modal = false;
                        self.entry_form.active_field = 2; // Advance to Duration
                    }
                    KeyCode::Up | KeyCode::BackTab => {
                        if self.entry_form.fzf_selected_index > 0 {
                            self.entry_form.fzf_selected_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Tab => {
                        if !matches.is_empty() && self.entry_form.fzf_selected_index + 1 < matches.len() {
                            self.entry_form.fzf_selected_index += 1;
                        }
                    }
                    KeyCode::Char('k')
                        if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        if self.entry_form.fzf_selected_index > 0 {
                            self.entry_form.fzf_selected_index -= 1;
                        }
                    }
                    KeyCode::Char('j')
                        if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        if !matches.is_empty() && self.entry_form.fzf_selected_index + 1 < matches.len() {
                            self.entry_form.fzf_selected_index += 1;
                        }
                    }
                    KeyCode::Char('p')
                        if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        if self.entry_form.fzf_selected_index > 0 {
                            self.entry_form.fzf_selected_index -= 1;
                        }
                    }
                    KeyCode::Char('n')
                        if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        if !matches.is_empty() && self.entry_form.fzf_selected_index + 1 < matches.len() {
                            self.entry_form.fzf_selected_index += 1;
                        }
                    }
                    KeyCode::Backspace => {
                        self.entry_form.fzf_query.pop();
                        self.entry_form.fzf_selected_index = 0;
                    }
                    KeyCode::Char(c) => {
                        self.entry_form.fzf_query.push(c);
                        self.entry_form.fzf_selected_index = 0;
                    }
                    _ => {}
                }
                return;
            }

            // Sub-case B: Standard Form Modal
            match key.code {
                KeyCode::Esc => {
                    self.close_modal();
                }
                // Ctrl+F or Ctrl+P: Launch FZF Project Picker Modal
                KeyCode::Char('f')
                    if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    self.entry_form.show_fzf_modal = true;
                    self.entry_form.fzf_query = self.entry_form.project.clone();
                    self.entry_form.fzf_selected_index = 0;
                }
                KeyCode::Char('p')
                    if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                        && self.entry_form.active_field == 1 =>
                {
                    self.entry_form.show_fzf_modal = true;
                    self.entry_form.fzf_query = self.entry_form.project.clone();
                    self.entry_form.fzf_selected_index = 0;
                }
                KeyCode::Enter => {
                    // If on Field 1 (Project) and a suggestion is selected, apply it!
                    if self.entry_form.active_field == 1 {
                        let matches = self.fuzzy_filter_projects(&self.entry_form.project);
                        if let Some((proj, _)) = matches.get(self.entry_form.selected_project_option) {
                            self.entry_form.project = proj.name.clone();
                            self.entry_form.active_field = 2; // Advance to Duration
                            return;
                        }
                    }
                    if let Some(db_ref) = db {
                        if let Err(e) = self.save_entry_form(db_ref) {
                            self.set_status_message(e);
                        }
                    } else {
                        self.close_modal();
                    }
                }
                KeyCode::Tab => {
                    if self.entry_form.active_field == 1 {
                        let matches = self.fuzzy_filter_projects(&self.entry_form.project);
                        if !self.entry_form.project.is_empty() {
                            if let Some((proj, _)) = matches.get(self.entry_form.selected_project_option) {
                                self.entry_form.project = proj.name.clone();
                            }
                        }
                    }
                    self.entry_form.active_field = (self.entry_form.active_field + 1) % 4;
                }
                KeyCode::BackTab => {
                    self.entry_form.active_field = (self.entry_form.active_field + 3) % 4;
                }
                KeyCode::Down => {
                    if self.entry_form.active_field == 1 {
                        let matches = self.fuzzy_filter_projects(&self.entry_form.project);
                        if !matches.is_empty() && self.entry_form.selected_project_option + 1 < matches.len() {
                            self.entry_form.selected_project_option += 1;
                        } else if matches.is_empty() {
                            self.entry_form.active_field = 2;
                        }
                    } else {
                        self.entry_form.active_field = (self.entry_form.active_field + 1) % 4;
                    }
                }
                KeyCode::Up => {
                    if self.entry_form.active_field == 1 {
                        if self.entry_form.selected_project_option > 0 {
                            self.entry_form.selected_project_option -= 1;
                        } else {
                            self.entry_form.active_field = 0;
                        }
                    } else {
                        self.entry_form.active_field = (self.entry_form.active_field + 3) % 4;
                    }
                }
                KeyCode::Char('j')
                    if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                        && self.entry_form.active_field == 1 =>
                {
                    let matches = self.fuzzy_filter_projects(&self.entry_form.project);
                    if !matches.is_empty() && self.entry_form.selected_project_option + 1 < matches.len() {
                        self.entry_form.selected_project_option += 1;
                    }
                }
                KeyCode::Char('k')
                    if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                        && self.entry_form.active_field == 1 =>
                {
                    if self.entry_form.selected_project_option > 0 {
                        self.entry_form.selected_project_option -= 1;
                    }
                }
                KeyCode::Backspace => {
                    self.entry_form.active_field_mut().pop();
                    if self.entry_form.active_field == 1 {
                        self.entry_form.selected_project_option = 0;
                    }
                }
                KeyCode::Char(c) => {
                    self.entry_form.active_field_mut().push(c);
                    if self.entry_form.active_field == 1 {
                        self.entry_form.selected_project_option = 0;
                    }
                }
                _ => {}
            }
            return;
        }

        if self.show_add_project_modal || self.show_edit_project_modal {
            match key.code {
                KeyCode::Esc => {
                    self.close_modal();
                }
                KeyCode::Enter => {
                    if let Some(db_ref) = db {
                        if let Err(e) = self.save_project_form(db_ref) {
                            self.set_status_message(e);
                        }
                    } else {
                        self.close_modal();
                    }
                }
                KeyCode::Tab | KeyCode::Down => {
                    self.project_form.active_field = (self.project_form.active_field + 1) % 3;
                }
                KeyCode::BackTab | KeyCode::Up => {
                    self.project_form.active_field = (self.project_form.active_field + 2) % 3;
                }
                KeyCode::Backspace => {
                    self.project_form.active_field_mut().pop();
                }
                KeyCode::Char(c) => {
                    self.project_form.active_field_mut().push(c);
                }
                _ => {}
            }
            return;
        }

        if self.show_filter_modal {
            match key.code {
                KeyCode::Esc => {
                    self.show_filter_modal = false;
                }
                KeyCode::Enter => {
                    let trimmed = self.filter_input.trim();
                    self.history_filter = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    };
                    self.selected_history_index = 0;
                    self.show_filter_modal = false;
                }
                KeyCode::Backspace => {
                    self.filter_input.pop();
                }
                KeyCode::Char(c) => {
                    self.filter_input.push(c);
                }
                _ => {}
            }
            return;
        }

        // Global navigation
        match key.code {
            KeyCode::Char('q') => {
                self.quit();
                return;
            }
            KeyCode::Char('?') => {
                self.toggle_help();
                return;
            }
            KeyCode::Char('1') => {
                self.set_tab(Tab::Timer);
                return;
            }
            KeyCode::Char('2') => {
                self.set_tab(Tab::History);
                return;
            }
            KeyCode::Char('3') => {
                self.set_tab(Tab::Projects);
                return;
            }
            KeyCode::Char('4') => {
                self.set_tab(Tab::Analytics);
                return;
            }
            KeyCode::BackTab => {
                self.prev_tab();
                return;
            }
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.prev_tab();
                return;
            }
            KeyCode::Tab => {
                self.next_tab();
                return;
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if let Some(db_ref) = db {
                    self.sync_clockify(db_ref);
                } else {
                    self.set_status_message("Sync unavailable (offline)");
                }
                return;
            }
            _ => {}
        }

        // Tab-specific key handlers
        match self.current_tab {
            Tab::Timer => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.select_next_recent();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.select_prev_recent();
                }
                KeyCode::Char(' ') => {
                    if let Some(db_ref) = db {
                        self.start_stop_timer(db_ref);
                    }
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    if let Some(db_ref) = db {
                        self.toggle_pomodoro_mode(db_ref);
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if let Some(db_ref) = db {
                        self.repeat_selected_entry(db_ref);
                    }
                }
                _ => {}
            },
            Tab::History => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.select_next_history();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.select_prev_history();
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.open_new_entry_modal();
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    self.open_edit_entry_modal();
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.open_delete_entry_modal();
                }
                KeyCode::Char('/') => {
                    self.open_filter_modal();
                }
                KeyCode::Esc if self.history_filter.is_some() => {
                    self.history_filter = None;
                    self.selected_history_index = 0;
                }
                _ => {}
            },
            Tab::Projects => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.select_next_project();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.select_prev_project();
                }
                KeyCode::Char('a')
                | KeyCode::Char('A')
                | KeyCode::Char('n')
                | KeyCode::Char('N') => {
                    self.open_add_project_modal();
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    self.open_edit_project_modal();
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    if let Some(db_ref) = db {
                        self.toggle_archive_selected_project(db_ref);
                    }
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.open_delete_project_modal();
                }
                _ => {}
            },
            Tab::Analytics => match key.code {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if let Some(db_ref) = db {
                        self.refresh_analytics(db_ref);
                        self.set_status_message("Analytics refreshed");
                    }
                }
                _ => {}
            },
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new(AppConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_enumeration_and_indexing() {
        assert_eq!(Tab::Timer.index(), 0);
        assert_eq!(Tab::History.index(), 1);
        assert_eq!(Tab::Projects.index(), 2);
        assert_eq!(Tab::Analytics.index(), 3);

        assert_eq!(Tab::from_index(0), Some(Tab::Timer));
        assert_eq!(Tab::from_index(1), Some(Tab::History));
        assert_eq!(Tab::from_index(2), Some(Tab::Projects));
        assert_eq!(Tab::from_index(3), Some(Tab::Analytics));
        assert_eq!(Tab::from_index(4), None);
    }

    #[test]
    fn test_tab_circular_navigation() {
        let mut tab = Tab::Timer;

        tab = tab.next();
        assert_eq!(tab, Tab::History);
        tab = tab.next();
        assert_eq!(tab, Tab::Projects);
        tab = tab.next();
        assert_eq!(tab, Tab::Analytics);
        tab = tab.next();
        assert_eq!(tab, Tab::Timer);

        tab = tab.prev();
        assert_eq!(tab, Tab::Analytics);
        tab = tab.prev();
        assert_eq!(tab, Tab::Projects);
        tab = tab.prev();
        assert_eq!(tab, Tab::History);
        tab = tab.prev();
        assert_eq!(tab, Tab::Timer);
    }

    #[test]
    fn test_app_tab_key_navigation() {
        let mut app = App::default();
        assert_eq!(app.current_tab, Tab::Timer);

        // Tab -> next
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.current_tab, Tab::History);

        // BackTab -> prev
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.current_tab, Tab::Timer);

        // Numeric keys 1..4
        app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        assert_eq!(app.current_tab, Tab::Projects);

        app.handle_key(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE));
        assert_eq!(app.current_tab, Tab::Analytics);

        app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        assert_eq!(app.current_tab, Tab::History);

        app.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(app.current_tab, Tab::Timer);
    }

    #[test]
    fn test_app_quit_signals() {
        let mut app = App::default();
        assert!(app.running);

        // 'q' quits
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(!app.running);

        // Ctrl+C quits
        let mut app2 = App::default();
        app2.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app2.running);
    }

    #[test]
    fn test_app_help_toggle_and_dismiss() {
        let mut app = App::default();
        assert!(!app.show_help);

        // '?' opens help
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert!(app.show_help);

        // Esc closes help
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.show_help);

        // '?' toggles help again
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert!(app.show_help);
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert!(!app.show_help);
    }

    #[test]
    fn test_status_message_lifecycle() {
        let mut app = App::default();
        assert!(app.status_message.is_none());

        app.set_status_message("Syncing data...");
        assert!(app.status_message.is_some());
        assert_eq!(app.status_message.as_ref().unwrap().0, "Syncing data...");

        app.clear_status_message();
        assert!(app.status_message.is_none());
    }

    #[test]
    fn test_recent_entries_selection_navigation() {
        let mut app = App::default();
        let e1 = TimeEntry::new("Entry 1", Utc::now());
        let e2 = TimeEntry::new("Entry 2", Utc::now());
        let e3 = TimeEntry::new("Entry 3", Utc::now());
        app.today_entries = vec![e1, e2, e3];
        assert_eq!(app.selected_recent_index, 0);

        app.select_next_recent();
        assert_eq!(app.selected_recent_index, 1);
        app.select_next_recent();
        assert_eq!(app.selected_recent_index, 2);
        // Clamped at max
        app.select_next_recent();
        assert_eq!(app.selected_recent_index, 2);

        app.select_prev_recent();
        assert_eq!(app.selected_recent_index, 1);
        app.select_prev_recent();
        assert_eq!(app.selected_recent_index, 0);
        // Clamped at 0
        app.select_prev_recent();
        assert_eq!(app.selected_recent_index, 0);
    }

    #[test]
    fn test_parse_duration_input_cases() {
        assert_eq!(
            parse_duration_input("1h 15m"),
            chrono::Duration::minutes(75)
        );
        assert_eq!(parse_duration_input("45m"), chrono::Duration::minutes(45));
        assert_eq!(parse_duration_input("2h"), chrono::Duration::hours(2));
        assert_eq!(parse_duration_input("90"), chrono::Duration::minutes(90));
        assert_eq!(parse_duration_input("1.5h"), chrono::Duration::minutes(90));
        assert_eq!(
            parse_duration_input("01:20:00"),
            chrono::Duration::minutes(80)
        );
        assert_eq!(
            parse_duration_input("14:00 - 15:30"),
            chrono::Duration::minutes(90)
        );
    }
}
