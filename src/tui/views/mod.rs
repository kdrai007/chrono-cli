//! View implementations for each primary tab in chrono-cli.

pub mod analytics;
pub mod history;
pub mod projects;
pub mod timer;

pub use analytics::{analytics_view, render_analytics_view};
pub use history::{history_view, render_history_view};
pub use projects::{projects_view, render_projects_view};
pub use timer::{render_timer_view, timer_view};
