//! Terminal user interface (TUI) framework, event loop, and views.

pub mod app;
pub mod event;
pub mod ui;

use std::io::{self, Stdout};

use crossterm::cursor::Show;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

pub use app::{App, Tab};
pub use event::{Event, EventHandler};
pub use ui::render;

use crate::config::AppConfig;
use crate::storage::Database;

/// Initializes the terminal by entering raw mode, switching to the alternate screen,
/// and setting up a panic hook to safely restore the terminal.
pub fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(e) = execute!(stdout, EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(e);
    }

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
        original_hook(panic_info);
    }));

    let backend = CrosstermBackend::new(stdout);
    match Terminal::new(backend) {
        Ok(t) => Ok(t),
        Err(e) => {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            Err(e)
        }
    }
}

/// Restores the terminal by disabling raw mode, leaving alternate screen,
/// and showing the cursor.
pub fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), io::Error> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, Show)?;
    terminal.show_cursor()?;
    Ok(())
}

/// Runs the main TUI event loop on an initialized terminal.
pub fn run_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    events: &EventHandler,
) -> Result<(), Box<dyn std::error::Error>> {
    while app.running {
        terminal.draw(|f| ui::render(app, f))?;

        match events.next()? {
            Event::Key(key) => {
                app.handle_key(key);
            }
            Event::Tick => {
                app.tick();
            }
            Event::Resize(_, _) => {}
        }
    }
    Ok(())
}

/// Main entry point for launching the interactive Clockify TUI.
pub fn run_tui(db: &mut Database, config: AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = init_terminal()?;
    let event_handler = EventHandler::default();

    let active_entry = db.get_active_entry().ok().flatten();
    let mut active_project_name = None;
    if let Some(entry) = &active_entry {
        if let Some(pid) = entry.project_id {
            if let Ok(Some(proj)) = db.get_project(pid) {
                active_project_name = Some(proj.name);
            }
        }
    }

    let mut app = App::new(config).with_active_entry(active_entry, active_project_name);

    let run_res = run_loop(&mut terminal, &mut app, &event_handler);
    let restore_res = restore_terminal(&mut terminal);

    run_res?;
    restore_res?;
    Ok(())
}
