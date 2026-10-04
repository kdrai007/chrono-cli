//! CLI arguments and command definitions using Clap.

use std::env;
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::storage::Database;

/// Export file format for recorded time entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// Comma-separated values format
    Csv,
    /// JSON array format
    Json,
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Csv => write!(f, "csv"),
            Self::Json => write!(f, "json"),
        }
    }
}

/// Available CLI subcommands.
#[derive(Debug, Clone, Subcommand, PartialEq, Eq)]
pub enum Commands {
    /// Start a new time entry
    Start {
        /// Optional description or task name
        description: Option<String>,

        /// Associated project name
        #[arg(short, long)]
        project: Option<String>,

        /// Comma-separated list of tags
        #[arg(short, long, value_delimiter = ',')]
        tags: Vec<String>,

        /// Start as a Pomodoro work session
        #[arg(long)]
        pomodoro: bool,
    },

    /// Stop the currently active timer
    Stop,

    /// Show current timer status (text or Waybar JSON)
    Status {
        /// Output status formatted as JSON for Waybar
        #[arg(long)]
        json: bool,
    },

    /// Export recorded time entries to CSV or JSON
    Export {
        /// Export format (csv or json)
        #[arg(long, default_value = "csv")]
        format: ExportFormat,

        /// Output file path (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Synchronize time entries with Clockify cloud
    Sync,
}

/// Command line interface for Clockify TUI.
#[derive(Debug, Parser, Clone)]
#[command(
    name = "clockify-tui",
    author,
    version,
    about = "Terminal-based Clockify study hours and project manager for students"
)]
pub struct Cli {
    /// Custom path to SQLite database file
    #[arg(long, global = true)]
    pub db_path: Option<PathBuf>,

    /// Custom path to configuration TOML file
    #[arg(long, global = true)]
    pub config_path: Option<PathBuf>,

    /// Subcommand to run in headless mode
    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    /// Resolves the database file path by checking CLI args, `CLOCKIFY_DB_PATH` env var,
    /// or falling back to default database location.
    pub fn get_db_path(&self) -> PathBuf {
        self.db_path
            .clone()
            .or_else(|| env::var("CLOCKIFY_DB_PATH").ok().map(PathBuf::from))
            .unwrap_or_else(Database::default_db_path)
    }

    /// Resolves the configuration file path by checking CLI args or `CLOCKIFY_CONFIG_PATH` env var.
    pub fn get_config_path(&self) -> Option<PathBuf> {
        self.config_path
            .clone()
            .or_else(|| env::var("CLOCKIFY_CONFIG_PATH").ok().map(PathBuf::from))
    }
}
