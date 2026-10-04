//! Reusable custom TUI widgets.

pub mod big_clock;
pub mod help;
pub mod modal;

pub use big_clock::BigClock;
pub use help::render_help_modal;
pub use modal::{centered_rect, centered_rect_fixed, Modal};
