//! View implementations for each primary tab in Clockify TUI.

pub mod history;
pub mod timer;

pub use history::{history_view, render_history_view};
pub use timer::{render_timer_view, timer_view};
