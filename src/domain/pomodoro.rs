use std::fmt;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::entry::EntryMode;

/// Current active phase in the Pomodoro cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PomodoroPhase {
    /// Work / focus session with session counter (1-based index).
    Work(u32),
    /// Short rest interval following a work session.
    ShortBreak(u32),
    /// Long rest interval triggered after a full cycle of sessions.
    LongBreak(u32),
}

impl PomodoroPhase {
    /// Returns the session index for the current phase.
    pub fn session_index(&self) -> u32 {
        match *self {
            Self::Work(i) | Self::ShortBreak(i) | Self::LongBreak(i) => i,
        }
    }

    /// Returns `true` if this phase is a work / focus interval.
    pub fn is_work(&self) -> bool {
        matches!(self, Self::Work(_))
    }

    /// Returns `true` if this phase is any break interval.
    pub fn is_break(&self) -> bool {
        !self.is_work()
    }

    /// Returns `true` if this phase is a short break.
    pub fn is_short_break(&self) -> bool {
        matches!(self, Self::ShortBreak(_))
    }

    /// Returns `true` if this phase is a long break.
    pub fn is_long_break(&self) -> bool {
        matches!(self, Self::LongBreak(_))
    }
}

impl fmt::Display for PomodoroPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Work(i) => write!(f, "Work ({i})"),
            Self::ShortBreak(i) => write!(f, "Short Break ({i})"),
            Self::LongBreak(i) => write!(f, "Long Break ({i})"),
        }
    }
}

/// State machine orchestrating Pomodoro work and break intervals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PomodoroStateMachine {
    work_mins: u32,
    short_break_mins: u32,
    long_break_mins: u32,
    sessions_until_long_break: u32,
    current_phase: PomodoroPhase,
}

impl PomodoroStateMachine {
    /// Default work interval duration in minutes.
    pub const DEFAULT_WORK_MINS: u32 = 25;
    /// Default short break duration in minutes.
    pub const DEFAULT_SHORT_BREAK_MINS: u32 = 5;
    /// Default long break duration in minutes.
    pub const DEFAULT_LONG_BREAK_MINS: u32 = 15;
    /// Default number of work sessions before triggering a long break.
    pub const DEFAULT_SESSIONS_UNTIL_LONG_BREAK: u32 = 4;

    /// Creates a new state machine with custom configuration.
    pub fn new(
        work_mins: u32,
        short_break_mins: u32,
        long_break_mins: u32,
        sessions_until_long_break: u32,
    ) -> Self {
        Self {
            work_mins: work_mins.max(1),
            short_break_mins: short_break_mins.max(1),
            long_break_mins: long_break_mins.max(1),
            sessions_until_long_break: sessions_until_long_break.max(1),
            current_phase: PomodoroPhase::Work(1),
        }
    }

    /// Creates a state machine from application `PomodoroConfig`.
    pub fn from_config(config: &crate::config::PomodoroConfig) -> Self {
        Self::new(
            config.work_duration_mins,
            config.short_break_mins,
            config.long_break_mins,
            config.sessions_until_long_break,
        )
    }

    /// Returns the currently active phase.
    pub fn current_phase(&self) -> PomodoroPhase {
        self.current_phase
    }

    /// Sets the current phase manually.
    pub fn set_phase(&mut self, phase: PomodoroPhase) {
        self.current_phase = phase;
    }

    /// Resets the state machine back to `Work(1)`.
    pub fn reset(&mut self) {
        self.current_phase = PomodoroPhase::Work(1);
    }

    /// Configured work duration in minutes.
    pub fn work_mins(&self) -> u32 {
        self.work_mins
    }

    /// Configured short break duration in minutes.
    pub fn short_break_mins(&self) -> u32 {
        self.short_break_mins
    }

    /// Configured long break duration in minutes.
    pub fn long_break_mins(&self) -> u32 {
        self.long_break_mins
    }

    /// Configured work sessions required before a long break.
    pub fn sessions_until_long_break(&self) -> u32 {
        self.sessions_until_long_break
    }

    /// Returns the duration of the current phase.
    pub fn current_phase_duration(&self) -> Duration {
        match self.current_phase {
            PomodoroPhase::Work(_) => Duration::minutes(self.work_mins as i64),
            PomodoroPhase::ShortBreak(_) => Duration::minutes(self.short_break_mins as i64),
            PomodoroPhase::LongBreak(_) => Duration::minutes(self.long_break_mins as i64),
        }
    }

    /// Transitions to the next phase in the Pomodoro cycle and returns it.
    ///
    /// Cycle:
    /// - `Work(n)` -> `ShortBreak(n)` (if `n % sessions_until_long_break != 0`)
    /// - `Work(n)` -> `LongBreak(n)` (if `n % sessions_until_long_break == 0`)
    /// - `ShortBreak(n)` -> `Work(n + 1)`
    /// - `LongBreak(n)` -> `Work(1)` (cycle reset)
    pub fn next_phase(&mut self) -> PomodoroPhase {
        let next = match self.current_phase {
            PomodoroPhase::Work(session) => {
                let divisor = self.sessions_until_long_break.max(1);
                if session % divisor == 0 {
                    PomodoroPhase::LongBreak(session)
                } else {
                    PomodoroPhase::ShortBreak(session)
                }
            }
            PomodoroPhase::ShortBreak(session) => PomodoroPhase::Work(session + 1),
            PomodoroPhase::LongBreak(_) => PomodoroPhase::Work(1),
        };
        self.current_phase = next;
        next
    }

    /// Calculates remaining time for the current phase given start and current timestamps.
    pub fn time_remaining(&self, start: DateTime<Utc>, now: DateTime<Utc>) -> Duration {
        let total = self.current_phase_duration();
        let elapsed = now.signed_duration_since(start);

        if elapsed >= total {
            Duration::zero()
        } else if elapsed < Duration::zero() {
            total
        } else {
            total - elapsed
        }
    }

    /// Calculates progress through the current phase as a fraction in `[0.0, 1.0]`.
    pub fn progress(&self, start: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
        let total_secs = self.current_phase_duration().num_seconds();
        if total_secs <= 0 {
            return 1.0;
        }

        let elapsed_secs = now.signed_duration_since(start).num_seconds();
        if elapsed_secs <= 0 {
            0.0
        } else if elapsed_secs >= total_secs {
            1.0
        } else {
            (elapsed_secs as f64 / total_secs as f64).clamp(0.0, 1.0)
        }
    }

    /// Returns `true` if the elapsed duration meets or exceeds the phase duration.
    pub fn is_phase_complete(&self, start: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        now >= start + self.current_phase_duration()
    }

    /// Returns the corresponding `EntryMode` for the current phase.
    pub fn current_entry_mode(&self) -> EntryMode {
        match self.current_phase {
            PomodoroPhase::Work(_) => EntryMode::PomodoroWork,
            PomodoroPhase::ShortBreak(_) | PomodoroPhase::LongBreak(_) => EntryMode::PomodoroBreak,
        }
    }
}

impl Default for PomodoroStateMachine {
    fn default() -> Self {
        Self::new(
            Self::DEFAULT_WORK_MINS,
            Self::DEFAULT_SHORT_BREAK_MINS,
            Self::DEFAULT_LONG_BREAK_MINS,
            Self::DEFAULT_SESSIONS_UNTIL_LONG_BREAK,
        )
    }
}
