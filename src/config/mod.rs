//! Configuration module for chrono-cli.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Error type for configuration operations.
#[derive(Debug)]
pub enum ConfigError {
    /// File I/O error.
    Io(std::io::Error),
    /// Error during TOML deserialization.
    TomlDe(toml::de::Error),
    /// Error during TOML serialization.
    TomlSer(toml::ser::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(err) => write!(f, "IO error: {err}"),
            ConfigError::TomlDe(err) => write!(f, "TOML deserialization error: {err}"),
            ConfigError::TomlSer(err) => write!(f, "TOML serialization error: {err}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io(err) => Some(err),
            ConfigError::TomlDe(err) => Some(err),
            ConfigError::TomlSer(err) => Some(err),
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        ConfigError::Io(err)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        ConfigError::TomlDe(err)
    }
}

impl From<toml::ser::Error> for ConfigError {
    fn from(err: toml::ser::Error) -> Self {
        ConfigError::TomlSer(err)
    }
}

/// General application configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct GeneralConfig {
    /// Whether to send desktop notifications.
    pub desktop_notifications: bool,
    /// Whether to ring the terminal bell on timer events.
    pub terminal_bell: bool,
    /// Time display format (e.g. "24h" or "12h").
    pub time_format: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            desktop_notifications: true,
            terminal_bell: true,
            time_format: "24h".to_string(),
        }
    }
}

/// Pomodoro timer settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PomodoroConfig {
    /// Work session duration in minutes.
    pub work_duration_mins: u32,
    /// Short break duration in minutes.
    pub short_break_mins: u32,
    /// Long break duration in minutes.
    pub long_break_mins: u32,
    /// Number of work sessions before a long break.
    pub sessions_until_long_break: u32,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            work_duration_mins: 25,
            short_break_mins: 5,
            long_break_mins: 15,
            sessions_until_long_break: 4,
        }
    }
}

/// Clockify cloud integration settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct ClockifyConfig {
    /// Whether Clockify sync is enabled.
    pub enabled: bool,
    /// Clockify API key.
    pub api_key: String,
    /// Clockify workspace ID.
    pub workspace_id: String,
    /// Whether to automatically sync time entries on exit.
    pub sync_on_exit: bool,
}

/// Main application configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct AppConfig {
    /// General settings.
    pub general: GeneralConfig,
    /// Pomodoro settings.
    pub pomodoro: PomodoroConfig,
    /// Clockify integration settings.
    pub clockify: ClockifyConfig,
}

impl AppConfig {
    /// Returns the default configuration file path:
    /// `~/.config/chrono-cli/config.toml` (or system equivalent).
    pub fn default_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("chrono-cli")
            .join("config.toml")
    }

    /// Loads configuration from a TOML string, falling back to defaults for missing fields.
    pub fn load_from_str(s: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(s)?;
        Ok(config)
    }

    /// Loads configuration from the specified file path.
    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        Self::load_from_str(&content)
    }

    /// Loads configuration from the default path if it exists; otherwise returns default configuration.
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::default_config_path();
        if path.exists() {
            Self::load_from_path(&path)
        } else {
            Ok(Self::default())
        }
    }

    /// Saves configuration as pretty TOML to the specified file path, creating parent directories if needed.
    pub fn save_to_path(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let toml_string = toml::to_string_pretty(self)?;
        fs::write(path, toml_string)?;
        Ok(())
    }

    /// Saves configuration to the default path.
    pub fn save(&self) -> Result<(), ConfigError> {
        self.save_to_path(&Self::default_config_path())
    }
}

impl FromStr for AppConfig {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::load_from_str(s)
    }
}
