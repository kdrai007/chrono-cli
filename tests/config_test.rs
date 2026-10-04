use std::fs;
use tempfile::tempdir;
use clockify_tui::config::{AppConfig, ClockifyConfig, GeneralConfig, PomodoroConfig};

#[test]
fn test_default_config_values() {
    let config = AppConfig::default();

    // General defaults
    assert!(config.general.desktop_notifications);
    assert!(config.general.terminal_bell);
    assert_eq!(config.general.time_format, "24h");

    // Pomodoro defaults
    assert_eq!(config.pomodoro.work_duration_mins, 25);
    assert_eq!(config.pomodoro.short_break_mins, 5);
    assert_eq!(config.pomodoro.long_break_mins, 15);
    assert_eq!(config.pomodoro.sessions_until_long_break, 4);

    // Clockify defaults
    assert!(!config.clockify.enabled);
    assert_eq!(config.clockify.api_key, "");
    assert_eq!(config.clockify.workspace_id, "");
    assert!(!config.clockify.sync_on_exit);
}

#[test]
fn test_toml_serialization_and_deserialization() {
    let mut config = AppConfig::default();
    config.pomodoro.work_duration_mins = 50;
    config.pomodoro.short_break_mins = 10;
    config.general.time_format = "12h".to_string();
    config.clockify.enabled = true;
    config.clockify.api_key = "secret_key".to_string();

    let toml_str = toml::to_string(&config).expect("Failed to serialize config to TOML");
    let deserialized: AppConfig = toml::from_str(&toml_str).expect("Failed to deserialize config from TOML");

    assert_eq!(config, deserialized);
}

#[test]
fn test_load_from_str_with_fallback_defaults() {
    // Missing general and clockify, and missing partial fields in pomodoro
    let partial_toml = r#"
        [pomodoro]
        work_duration_mins = 30
    "#;

    let config = AppConfig::load_from_str(partial_toml).expect("Failed to load partial TOML");

    // Specified field is overridden
    assert_eq!(config.pomodoro.work_duration_mins, 30);
    // Missing pomodoro fields fall back to default
    assert_eq!(config.pomodoro.short_break_mins, 5);
    assert_eq!(config.pomodoro.long_break_mins, 15);
    assert_eq!(config.pomodoro.sessions_until_long_break, 4);
    // Missing sections fall back to default
    assert_eq!(config.general, GeneralConfig::default());
    assert_eq!(config.clockify, ClockifyConfig::default());
}

#[test]
fn test_save_and_load_from_file() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("subdir").join("config.toml");

    let mut original_config = AppConfig::default();
    original_config.general.time_format = "12h".to_string();
    original_config.clockify.api_key = "test_api_key_123".to_string();

    // Saving should create parent directory if needed
    original_config
        .save_to_path(&config_path)
        .expect("Failed to save config to path");

    assert!(config_path.exists());

    // Load back from file
    let loaded_config = AppConfig::load_from_path(&config_path).expect("Failed to load config from path");
    assert_eq!(original_config, loaded_config);
}

#[test]
fn test_load_from_file_with_fallback_defaults() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("config.toml");

    let partial_toml = r#"
        [clockify]
        enabled = true
        api_key = "xyz"
    "#;
    fs::write(&config_path, partial_toml).expect("Failed to write partial config");

    let config = AppConfig::load_from_path(&config_path).expect("Failed to load config from path");
    assert!(config.clockify.enabled);
    assert_eq!(config.clockify.api_key, "xyz");
    assert_eq!(config.clockify.workspace_id, "");
    assert!(!config.clockify.sync_on_exit);
    assert_eq!(config.pomodoro, PomodoroConfig::default());
    assert_eq!(config.general, GeneralConfig::default());
}

#[test]
fn test_load_fallback_when_file_not_found() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let non_existent_path = temp_dir.path().join("does_not_exist.toml");

    // load_from_path returns error on non-existent file
    assert!(AppConfig::load_from_path(&non_existent_path).is_err());
}

#[test]
fn test_default_config_path() {
    let path = AppConfig::default_config_path();
    assert!(path.ends_with("clockify-tui/config.toml") || path.ends_with("clockify-tui\\config.toml"));
}

#[test]
fn test_app_config_load() {
    // If default path does not exist, load() returns default config
    // If it exists, it loads it.
    let result = AppConfig::load();
    assert!(result.is_ok());
}

