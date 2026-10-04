//! Application state and navigation models for Clockify TUI.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::AppConfig;
use crate::domain::{PomodoroStateMachine, TimeEntry};
use crate::notify::NotificationService;

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

    /// Handles keyboard events according to defined bindings.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Exit shortcuts: Ctrl+C or 'q'
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

        match key.code {
            KeyCode::Char('q') => self.quit(),
            KeyCode::Char('?') => self.toggle_help(),
            KeyCode::Esc => {}
            KeyCode::Char('1') => self.set_tab(Tab::Timer),
            KeyCode::Char('2') => self.set_tab(Tab::History),
            KeyCode::Char('3') => self.set_tab(Tab::Projects),
            KeyCode::Char('4') => self.set_tab(Tab::Analytics),
            KeyCode::BackTab => self.prev_tab(),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.prev_tab(),
            KeyCode::Tab => self.next_tab(),
            _ => {}
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
}
