//! View implementations for each primary tab in Clockify TUI.

pub mod history;
pub mod projects;
pub mod timer;

pub use history::{history_view, render_history_view};
pub use projects::{projects_view, render_projects_view};
pub use timer::{render_timer_view, timer_view};
