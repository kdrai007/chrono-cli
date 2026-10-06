//! Headless CLI subcommands and dispatcher.

pub mod args;
pub mod commands;
pub mod omarchy;

pub use args::{Cli, Commands, ExportFormat, OmarchyCommands};
pub use commands::{ExportRecord, WaybarOutput};
pub use omarchy::{
    build_omarchy_snapshot, handle_omarchy_clear_key, handle_omarchy_continue,
    handle_omarchy_discard, handle_omarchy_set_config, handle_omarchy_set_key,
    handle_omarchy_start, handle_omarchy_status, handle_omarchy_stop, OmarchyPayload,
    OmarchyProjectBreakdown, OmarchyProjectItem, OmarchyRecentTask, OmarchyRunningEntry,
    OmarchySetConfigPayload, OmarchySettings,
};

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
        Commands::Status { json, omarchy } => {
            if omarchy {
                omarchy::handle_omarchy_status(db, config, false)?;
            } else {
                commands::handle_status(db, json)?;
            }
        }
        Commands::Export { format, output } => {
            commands::handle_export(db, format, output)?;
        }
        Commands::Sync => {
            commands::handle_sync(db, config, &notifications)?;
        }
        Commands::Omarchy { command } => match command {
            args::OmarchyCommands::Status { projects } => {
                omarchy::handle_omarchy_status(db, config, projects)?;
            }
            args::OmarchyCommands::Start {
                description,
                project,
                billable,
            } => {
                omarchy::handle_omarchy_start(db, config, description, project, billable)?;
            }
            args::OmarchyCommands::Continue {
                description,
                project,
                billable,
            } => {
                omarchy::handle_omarchy_continue(db, config, description, project, billable)?;
            }
            args::OmarchyCommands::Stop => {
                omarchy::handle_omarchy_stop(db, config)?;
            }
            args::OmarchyCommands::Discard => {
                omarchy::handle_omarchy_discard(db, config)?;
            }
            args::OmarchyCommands::SetConfig {
                project,
                workspace,
                week_start,
            } => {
                omarchy::handle_omarchy_set_config(config, project, workspace, week_start)?;
            }
            args::OmarchyCommands::SetKey => {
                omarchy::handle_omarchy_set_key(db, config)?;
            }
            args::OmarchyCommands::ClearKey => {
                omarchy::handle_omarchy_clear_key(db, config)?;
            }
        },
    }

    Ok(true)
}
