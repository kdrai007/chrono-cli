//! Headless CLI subcommands and dispatcher.

pub mod args;
pub mod commands;

pub use args::{Cli, Commands, ExportFormat};
pub use commands::{ExportRecord, WaybarOutput};

use crate::config::AppConfig;
use crate::notify::NotificationService;
use crate::storage::Database;

/// Executes CLI subcommands if specified.
///
/// Returns `Ok(true)` if a headless subcommand was executed,
/// or `Ok(false)` if no subcommand was given and TUI should be launched.
pub fn run_cli(
    cli: Cli,
    db: &mut Database,
    config: &AppConfig,
) -> Result<bool, Box<dyn std::error::Error>> {
    let Some(command) = cli.command else {
        return Ok(false);
    };

    let notifications = NotificationService::new(config.general.clone());

    match command {
        Commands::Start {
            description,
            project,
            tags,
            pomodoro,
        } => {
            commands::handle_start(db, &notifications, description, project, tags, pomodoro)?;
        }
        Commands::Stop => {
            commands::handle_stop(db, &notifications)?;
        }
        Commands::Status { json } => {
            commands::handle_status(db, json)?;
        }
        Commands::Export { format, output } => {
            commands::handle_export(db, format, output)?;
        }
        Commands::Sync => {
            commands::handle_sync(db, config, &notifications)?;
        }
    }

    Ok(true)
}
