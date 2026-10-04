//! Application state and navigation models for Clockify TUI.

use std::collections::HashMap;
use std::time::Instant;

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::AppConfig;
use crate::domain::{EntryMode, PomodoroStateMachine, Project, TimeEntry};
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

    /// Starts or stops the active timer in the database and updates state.
    pub fn start_stop_timer(&mut self, db: &mut Database) {
        if self.active_entry.is_some() {
            if let Ok(Some(entry)) = db.stop_active_entry() {
                let desc = if entry.description.is_empty() {
                    "Untitled Session".to_string()
                } else {
                    entry.description.clone()
                };
                self.active_entry = None;
                self.active_project_name = None;

                if entry.entry_mode == EntryMode::PomodoroWork {
                    let next = self.pomodoro.next_phase();
                    let _ = self.notifications.send(&NotificationEvent::Custom {
                        title: "Pomodoro Session Complete".to_string(),
                        body: format!("Finished {desc}! Up next: {next}"),
                    });
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
        } else {
            let desc = "Study Session";
            let mut entry = TimeEntry::new(desc, Utc::now());
            entry.entry_mode = EntryMode::Stopwatch;
            if let Ok(started) = db.start_entry(&entry) {
                self.active_entry = Some(started);
                self.active_project_name = None;
                self.set_status_message(format!("Started timer: {desc}"));
                let _ = self.notifications.send(&NotificationEvent::Custom {
                    title: "Timer Started".to_string(),
                    body: format!("Started {desc}"),
                });
                self.refresh_today_entries(db);
            }
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
                    entry.start_time = Utc::now();
                    let _ = db.update_entry(&entry);
                    self.active_entry = Some(entry);
                    self.set_status_message(format!("Switched to Pomodoro: {phase}"));
                }
                EntryMode::PomodoroWork | EntryMode::PomodoroBreak => {
                    let next_phase = self.pomodoro.next_phase();
                    let new_mode = if next_phase.is_work() {
                        EntryMode::PomodoroWork
                    } else {
                        EntryMode::PomodoroBreak
                    };
                    entry.entry_mode = new_mode;
                    entry.pomodoro_index = next_phase.session_index();
                    entry.start_time = Utc::now();
                    let _ = db.update_entry(&entry);
                    self.active_entry = Some(entry);
                    self.set_status_message(format!("Pomodoro phase: {next_phase}"));
                }
            }
            self.refresh_today_entries(db);
        } else {
            let phase = self.pomodoro.current_phase();
            let mode = if phase.is_work() {
                EntryMode::PomodoroWork
            } else {
                EntryMode::PomodoroBreak
            };
            let mut entry = TimeEntry::new("Pomodoro Focus", Utc::now());
            entry.entry_mode = mode;
            entry.pomodoro_index = phase.session_index();
            if let Ok(started) = db.start_entry(&entry) {
                self.active_entry = Some(started);
                self.active_project_name = None;
                self.set_status_message(format!("Started Pomodoro: {phase}"));
                self.refresh_today_entries(db);
            }
        }
    }

    /// Restarts a new timer with the selected entry's project, tags, and description.
    pub fn repeat_selected_entry(&mut self, db: &mut Database) {
        if self.today_entries.is_empty() || self.selected_recent_index >= self.today_entries.len() {
            return;
        }

        let target = self.today_entries[self.selected_recent_index].clone();
        if self.active_entry.is_some() {
            let _ = db.stop_active_entry();
            self.active_entry = None;
        }

        let mut new_entry = TimeEntry::new(&target.description, Utc::now());
        new_entry.project_id = target.project_id;
        new_entry.tags = target.tags.clone();
        new_entry.entry_mode = target.entry_mode;
        new_entry.pomodoro_index = target.pomodoro_index;

        if let Ok(started) = db.start_entry(&new_entry) {
            let mut proj_name = None;
            if let Some(pid) = started.project_id {
                if let Ok(Some(p)) = db.get_project(pid) {
                    proj_name = Some(p.name);
                }
            }
            self.set_status_message(format!("Repeated session: {}", target.description));
            self.active_entry = Some(started);
            self.active_project_name = proj_name;
            self.refresh_today_entries(db);
        }
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
            Tab::History | Tab::Projects | Tab::Analytics => {}
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
}
