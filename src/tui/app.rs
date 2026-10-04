//! Application state and navigation models for Clockify TUI.

use std::collections::HashMap;
use std::time::Instant;

use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::AppConfig;
use crate::domain::{EntryMode, PomodoroStateMachine, Project, Tag, TimeEntry};
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

/// Form state for creating or editing time entries via modal dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        }
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

        if let Some(entry) = &self.active_entry {
            if let Some(pid) = entry.project_id {
                self.active_project_name = self.projects.get(&pid).map(|p| p.name.clone());
            }
        }
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

    /// Closes all active modal dialogs.
    pub fn close_modal(&mut self) {
        self.show_new_entry_modal = false;
        self.show_edit_entry_modal = false;
        self.show_delete_entry_modal = false;
        self.show_filter_modal = false;
    }

    /// Returns `true` if any popup or modal dialog is currently active.
    pub fn is_modal_open(&self) -> bool {
        self.show_help
            || self.show_new_entry_modal
            || self.show_edit_entry_modal
            || self.show_delete_entry_modal
            || self.show_filter_modal
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

        if self.show_new_entry_modal || self.show_edit_entry_modal {
            match key.code {
                KeyCode::Esc => {
                    self.close_modal();
                }
                KeyCode::Enter => {
                    if let Some(db_ref) = db {
                        let _ = self.save_entry_form(db_ref);
                    } else {
                        self.close_modal();
                    }
                }
                KeyCode::Tab | KeyCode::Down => {
                    self.entry_form.active_field = (self.entry_form.active_field + 1) % 4;
                }
                KeyCode::BackTab | KeyCode::Up => {
                    self.entry_form.active_field = (self.entry_form.active_field + 3) % 4;
                }
                KeyCode::Backspace => {
                    self.entry_form.active_field_mut().pop();
                }
                KeyCode::Char(c) => {
                    self.entry_form.active_field_mut().push(c);
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
            Tab::Projects | Tab::Analytics => {}
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
