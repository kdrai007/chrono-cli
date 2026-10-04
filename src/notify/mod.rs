//! Notifications and audio/bell alert system for Clockify TUI.
//!
//! Provides desktop notifications via `notify-rust` and terminal bell alerts (`\x07`),
//! with configuration controls and support for testable mock sinks.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::config::GeneralConfig;
use crate::domain::PomodoroPhase;

/// Error type for notification operations.
#[derive(Debug)]
pub enum NotifyError {
    /// Desktop notification failed (e.g. D-Bus daemon unavailable).
    DesktopNotification(String),
    /// I/O error when writing to stdout or flushing.
    Io(std::io::Error),
}

impl fmt::Display for NotifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DesktopNotification(msg) => write!(f, "Desktop notification error: {msg}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
        }
    }
}

impl std::error::Error for NotifyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DesktopNotification(_) => None,
            Self::Io(err) => Some(err),
        }
    }
}

impl From<notify_rust::error::Error> for NotifyError {
    fn from(err: notify_rust::error::Error) -> Self {
        Self::DesktopNotification(err.to_string())
    }
}

impl From<std::io::Error> for NotifyError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// Urgency level for notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum NotificationUrgency {
    /// Low priority.
    Low,
    /// Default normal priority.
    #[default]
    Normal,
    /// Critical priority requiring immediate attention.
    Critical,
}

impl NotificationUrgency {
    /// Converts to the `notify_rust::Urgency` equivalent.
    pub fn to_notify_rust(self) -> notify_rust::Urgency {
        match self {
            Self::Low => notify_rust::Urgency::Low,
            Self::Normal => notify_rust::Urgency::Normal,
            Self::Critical => notify_rust::Urgency::Critical,
        }
    }
}

impl From<NotificationUrgency> for notify_rust::Urgency {
    fn from(urgency: NotificationUrgency) -> Self {
        urgency.to_notify_rust()
    }
}

/// Events that trigger audio or desktop notifications.
#[derive(Debug, Clone, PartialEq)]
pub enum NotificationEvent {
    /// Completed a Pomodoro phase and transitioning to the next phase.
    PomodoroFinished {
        /// Completed Pomodoro phase.
        phase: PomodoroPhase,
        /// Upcoming Pomodoro phase.
        next_phase: PomodoroPhase,
    },
    /// Reached a designated study target for a project.
    StudyTargetReached {
        /// Name of the project.
        project_name: String,
        /// Target hours achieved.
        target_hours: f64,
    },
    /// Cloud sync operation completed.
    SyncCompleted {
        /// Whether sync was successful.
        success: bool,
        /// Informational or error message.
        message: String,
    },
    /// Arbitrary notification alert.
    Custom {
        /// Title summary.
        title: String,
        /// Detail body.
        body: String,
    },
}

impl NotificationEvent {
    /// Generates an informative title for the notification.
    pub fn title(&self) -> String {
        match self {
            Self::PomodoroFinished { phase, .. } => match phase {
                PomodoroPhase::Work(_) => "Focus Session Complete!".to_string(),
                PomodoroPhase::ShortBreak(_) => "Short Break Over!".to_string(),
                PomodoroPhase::LongBreak(_) => "Long Break Over!".to_string(),
            },
            Self::StudyTargetReached { .. } => "Study Target Reached!".to_string(),
            Self::SyncCompleted { success, .. } => {
                if *success {
                    "Sync Successful".to_string()
                } else {
                    "Sync Failed".to_string()
                }
            }
            Self::Custom { title, .. } => title.clone(),
        }
    }

    /// Generates an informative body description for the notification.
    pub fn body(&self) -> String {
        match self {
            Self::PomodoroFinished { phase, next_phase } => match (phase, next_phase) {
                (PomodoroPhase::Work(_), PomodoroPhase::ShortBreak(_)) => {
                    "Time for a short break.".to_string()
                }
                (PomodoroPhase::Work(_), PomodoroPhase::LongBreak(_)) => {
                    "Time for a long break. Great work!".to_string()
                }
                (PomodoroPhase::Work(_), PomodoroPhase::Work(_)) => {
                    "Starting next focus session.".to_string()
                }
                (
                    PomodoroPhase::ShortBreak(_) | PomodoroPhase::LongBreak(_),
                    PomodoroPhase::Work(_),
                ) => "Break is over! Time to get back to focus.".to_string(),
                _ => format!("Transitioned from {phase} to {next_phase}."),
            },
            Self::StudyTargetReached {
                project_name,
                target_hours,
            } => {
                if target_hours.fract() == 0.0 {
                    format!(
                        "Congratulations! You reached your target of {:.0} hours for {project_name}.",
                        target_hours
                    )
                } else {
                    format!(
                        "Congratulations! You reached your target of {:.1} hours for {project_name}.",
                        target_hours
                    )
                }
            }
            Self::SyncCompleted { message, .. } => message.clone(),
            Self::Custom { body, .. } => body.clone(),
        }
    }

    /// Determines the notification urgency.
    pub fn urgency(&self) -> NotificationUrgency {
        match self {
            Self::PomodoroFinished { .. } => NotificationUrgency::Normal,
            Self::StudyTargetReached { .. } => NotificationUrgency::Normal,
            Self::SyncCompleted { success: true, .. } => NotificationUrgency::Low,
            Self::SyncCompleted { success: false, .. } => NotificationUrgency::Critical,
            Self::Custom { .. } => NotificationUrgency::Normal,
        }
    }
}

/// Sink abstraction for dispatching desktop notifications and ringing the bell.
pub trait NotificationSink: Send + Sync {
    /// Sends a desktop notification with a given title, body, and urgency.
    fn send_desktop(
        &self,
        title: &str,
        body: &str,
        urgency: NotificationUrgency,
    ) -> Result<(), NotifyError>;

    /// Rings the terminal bell.
    fn ring_bell(&self) -> Result<(), NotifyError>;
}

/// Real notification sink communicating with OS desktop notification daemon and terminal stdout.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealNotificationSink;

impl NotificationSink for RealNotificationSink {
    fn send_desktop(
        &self,
        title: &str,
        body: &str,
        urgency: NotificationUrgency,
    ) -> Result<(), NotifyError> {
        let mut notif = notify_rust::Notification::new();
        notif
            .appname("clockify-tui")
            .summary(title)
            .body(body)
            .urgency(urgency.into());

        match notif.show() {
            Ok(_) => Ok(()),
            Err(e) => Err(NotifyError::DesktopNotification(e.to_string())),
        }
    }

    fn ring_bell(&self) -> Result<(), NotifyError> {
        use std::io::Write;
        let mut stdout = std::io::stdout();
        stdout.write_all(b"\x07")?;
        stdout.flush()?;
        Ok(())
    }
}

/// Thread-safe in-memory notification sink for unit tests and headless environments.
#[derive(Debug, Default)]
pub struct MockNotificationSink {
    desktop_notifications: Mutex<Vec<(String, String, NotificationUrgency)>>,
    bell_count: AtomicUsize,
    should_fail_desktop: AtomicBool,
}

impl MockNotificationSink {
    /// Creates a new mock notification sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns all desktop notifications sent to this sink.
    pub fn sent_notifications(&self) -> Vec<(String, String, NotificationUrgency)> {
        self.desktop_notifications
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Returns the number of times the terminal bell was rung.
    pub fn bell_count(&self) -> usize {
        self.bell_count.load(Ordering::SeqCst)
    }

    /// Sets whether desktop notifications should simulate failure.
    pub fn set_fail_desktop(&self, fail: bool) {
        self.should_fail_desktop.store(fail, Ordering::SeqCst);
    }

    /// Resets all recorded notifications and bell counts.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.desktop_notifications.lock() {
            guard.clear();
        }
        self.bell_count.store(0, Ordering::SeqCst);
    }
}

impl NotificationSink for MockNotificationSink {
    fn send_desktop(
        &self,
        title: &str,
        body: &str,
        urgency: NotificationUrgency,
    ) -> Result<(), NotifyError> {
        if self.should_fail_desktop.load(Ordering::SeqCst) {
            return Err(NotifyError::DesktopNotification(
                "Simulated desktop notification failure".to_string(),
            ));
        }
        if let Ok(mut guard) = self.desktop_notifications.lock() {
            guard.push((title.to_string(), body.to_string(), urgency));
        }
        Ok(())
    }

    fn ring_bell(&self) -> Result<(), NotifyError> {
        self.bell_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Service orchestrating desktop notifications and audio/bell alerts according to application config.
#[derive(Clone)]
pub struct NotificationService {
    config: GeneralConfig,
    sink: Arc<dyn NotificationSink>,
    recorded_events: Arc<Mutex<Vec<NotificationEvent>>>,
}

impl fmt::Debug for NotificationService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotificationService")
            .field("config", &self.config)
            .field(
                "recorded_events_count",
                &self.recorded_events.lock().map(|e| e.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl NotificationService {
    /// Creates a notification service using the real OS notification sink.
    pub fn new(config: GeneralConfig) -> Self {
        Self::with_sink(config, Arc::new(RealNotificationSink))
    }

    /// Creates a notification service with a custom notification sink.
    pub fn with_sink(config: GeneralConfig, sink: Arc<dyn NotificationSink>) -> Self {
        Self {
            config,
            sink,
            recorded_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Convenience constructor creating an in-memory service backed by a `MockNotificationSink`.
    pub fn in_memory(config: GeneralConfig) -> (Self, Arc<MockNotificationSink>) {
        let mock = Arc::new(MockNotificationSink::new());
        let service = Self::with_sink(config, mock.clone());
        (service, mock)
    }

    /// Returns a reference to the active `GeneralConfig`.
    pub fn config(&self) -> &GeneralConfig {
        &self.config
    }

    /// Updates the service configuration.
    pub fn update_config(&mut self, config: GeneralConfig) {
        self.config = config;
    }

    /// Rings the terminal bell if enabled in configuration (writes `\x07` to stdout and flushes).
    pub fn ring_bell(&self) {
        if !self.config.terminal_bell {
            return;
        }
        let _ = self.sink.ring_bell();
    }

    /// Sends a desktop notification directly, skipping without error if disabled in configuration.
    pub fn send_desktop(&self, title: &str, body: &str) -> Result<(), NotifyError> {
        if !self.config.desktop_notifications {
            return Ok(());
        }
        self.sink
            .send_desktop(title, body, NotificationUrgency::Normal)
    }

    /// Dispatches a high-level notification event.
    ///
    /// Respects `terminal_bell` and `desktop_notifications` settings in `GeneralConfig`.
    pub fn send(&self, event: &NotificationEvent) -> Result<(), NotifyError> {
        if let Ok(mut recorded) = self.recorded_events.lock() {
            if recorded.len() >= 100 {
                recorded.remove(0);
            }
            recorded.push(event.clone());
        }

        if self.config.terminal_bell {
            self.ring_bell();
        }

        if self.config.desktop_notifications {
            self.sink
                .send_desktop(&event.title(), &event.body(), event.urgency())?;
        }

        Ok(())
    }

    /// Alias for `send`.
    pub fn notify(&self, event: &NotificationEvent) -> Result<(), NotifyError> {
        self.send(event)
    }

    /// Returns all events recorded by this service.
    pub fn recorded_events(&self) -> Vec<NotificationEvent> {
        self.recorded_events
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Clears all recorded events.
    pub fn clear_recorded_events(&self) {
        if let Ok(mut guard) = self.recorded_events.lock() {
            guard.clear();
        }
    }

    /// Convenience method to notify when a Pomodoro phase changes.
    pub fn notify_pomodoro_phase_change(
        &self,
        completed: &PomodoroPhase,
        next: &PomodoroPhase,
    ) -> Result<(), NotifyError> {
        self.send(&NotificationEvent::PomodoroFinished {
            phase: *completed,
            next_phase: *next,
        })
    }

    /// Alias for notify_pomodoro_phase_change.
    pub fn notify_pomodoro_completed(
        &self,
        completed: &PomodoroPhase,
        next: &PomodoroPhase,
    ) -> Result<(), NotifyError> {
        self.notify_pomodoro_phase_change(completed, next)
    }

    /// Convenience method to notify when a project target has been reached.
    pub fn notify_target_reached(&self, project: &str, hours: f64) -> Result<(), NotifyError> {
        self.send(&NotificationEvent::StudyTargetReached {
            project_name: project.to_string(),
            target_hours: hours,
        })
    }

    /// Convenience method to notify when a sync operation completes.
    pub fn notify_sync(&self, success: bool, msg: &str) -> Result<(), NotifyError> {
        self.send(&NotificationEvent::SyncCompleted {
            success,
            message: msg.to_string(),
        })
    }

    /// Alias for notify_sync.
    pub fn notify_sync_status(&self, success: bool, msg: &str) -> Result<(), NotifyError> {
        self.notify_sync(success, msg)
    }
}
