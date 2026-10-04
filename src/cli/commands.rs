//! Command handlers for headless CLI operations.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::cli::args::ExportFormat;
use crate::clockify::{ClockifyClient, SyncEngine};
use crate::config::AppConfig;
use crate::domain::{EntryMode, Project, Tag, TimeEntry};
use crate::notify::{NotificationEvent, NotificationService};
use crate::storage::Database;

/// Output structure formatted for Waybar JSON custom module consumption.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaybarOutput {
    /// Text to display directly in the Waybar module.
    pub text: String,
    /// Detailed tooltip text displayed on hover.
    pub tooltip: String,
    /// CSS class name for styling (e.g. "idle", "running").
    pub class: String,
    /// Module alternative text / status indicator.
    pub alt: String,
}

/// Representation of a recorded time entry for CSV or JSON export.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportRecord {
    /// Database entry ID.
    pub id: Option<i64>,
    /// Task description.
    pub description: String,
    /// Associated project name if assigned.
    pub project: Option<String>,
    /// UTC start timestamp.
    pub start_time: DateTime<Utc>,
    /// UTC end timestamp if completed.
    pub end_time: Option<DateTime<Utc>>,
    /// Duration in seconds.
    pub duration_seconds: i64,
    /// Associated tag names.
    pub tags: Vec<String>,
}

/// Starts a new time entry, creating associated projects and tags if they do not exist.
pub fn handle_start(
    db: &mut Database,
    notifications: &NotificationService,
    description: Option<String>,
    project: Option<String>,
    tags: Vec<String>,
    pomodoro: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Resolve or create project
    let project_id = match project.as_deref() {
        Some(p_name) if !p_name.trim().is_empty() => {
            let name = p_name.trim();
            if let Some(existing) = db.get_project_by_name(name)? {
                existing.id
            } else {
                let created = db.create_project(&Project::new(name)?)?;
                created.id
            }
        }
        _ => None,
    };

    // 2. Resolve or create tags
    let mut tag_entities = Vec::new();
    let mut clean_tags = Vec::new();
    for t in &tags {
        let name = t.trim();
        if !name.is_empty() {
            let tag = if let Some(existing) = db.get_tag_by_name(name)? {
                existing
            } else {
                db.create_tag(&Tag::new(name)?)?
            };
            tag_entities.push(tag);
            clean_tags.push(name.to_string());
        }
    }

    // 3. Prepare TimeEntry
    let desc = description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Untitled");

    let entry_mode = if pomodoro {
        EntryMode::PomodoroWork
    } else {
        EntryMode::Stopwatch
    };

    let mut entry = TimeEntry::new(desc, Utc::now());
    if let Some(pid) = project_id {
        entry = entry.with_project_id(pid);
    }
    entry = entry.with_entry_mode(entry_mode).with_tags(tag_entities);

    let _started = db.start_entry(&entry)?;

    // 4. Print confirmation
    let proj_part = match &project {
        Some(p) => format!(" [Project: {p}]"),
        None => String::new(),
    };
    let tag_part = if clean_tags.is_empty() {
        String::new()
    } else {
        format!(" [Tags: {}]", clean_tags.join(", "))
    };
    println!("Started timer: \"{}\"{}{}", desc, proj_part, tag_part);

    // 5. Fire notification
    let _ = notifications.send(&NotificationEvent::Custom {
        title: "Timer Started".to_string(),
        body: format!("Started {desc}{proj_part}"),
    });

    Ok(())
}

/// Stops the currently active timer and reports the elapsed duration.
pub fn handle_stop(
    db: &mut Database,
    notifications: &NotificationService,
) -> Result<(), Box<dyn std::error::Error>> {
    let stopped = db.stop_active_entry()?;
    if let Some(entry) = stopped {
        let elapsed = entry
            .format_duration()
            .unwrap_or_else(|| "00:00:00".to_string());
        println!(
            "Stopped timer: \"{}\". Elapsed duration: {}",
            entry.description, elapsed
        );
        let _ = notifications.send(&NotificationEvent::Custom {
            title: "Timer Stopped".to_string(),
            body: format!("{}: {}", entry.description, elapsed),
        });
    } else {
        println!("No active timer to stop.");
    }
    Ok(())
}

/// Displays the current timer status in human-readable plain text or Waybar JSON format.
pub fn handle_status(db: &Database, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let active = db.get_active_entry()?;
    let now = Utc::now();

    if json {
        let waybar = match active {
            Some(entry) => {
                let elapsed_str = entry.format_duration_until(now);
                let project_name = if let Some(pid) = entry.project_id {
                    db.get_project(pid)?.map(|p| p.name)
                } else {
                    None
                };
                let start_str = entry.start_time.format("%H:%M:%S").to_string();
                let tags_str = if entry.tags.is_empty() {
                    String::new()
                } else {
                    format!(
                        " [{}]",
                        entry
                            .tags
                            .iter()
                            .map(|t| format!("#{}", t.name))
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                };

                let text = if let Some(proj) = &project_name {
                    format!("{} [{}] ({})", entry.description, proj, elapsed_str)
                } else {
                    format!("{} ({})", entry.description, elapsed_str)
                };
                let tooltip = if let Some(proj) = &project_name {
                    format!(
                        "{}\nProject: {}\nStarted: {}\nElapsed: {}{}",
                        entry.description,
                        proj,
                        start_str,
                        elapsed_str,
                        if tags_str.is_empty() {
                            String::new()
                        } else {
                            format!("\nTags:{}", tags_str)
                        }
                    )
                } else {
                    format!(
                        "{}\nStarted: {}\nElapsed: {}{}",
                        entry.description,
                        start_str,
                        elapsed_str,
                        if tags_str.is_empty() {
                            String::new()
                        } else {
                            format!("\nTags:{}", tags_str)
                        }
                    )
                };
                WaybarOutput {
                    text,
                    tooltip,
                    class: "running".to_string(),
                    alt: "running".to_string(),
                }
            }
            None => WaybarOutput {
                text: "Idle".to_string(),
                tooltip: "No active timer".to_string(),
                class: "idle".to_string(),
                alt: "idle".to_string(),
            },
        };
        println!("{}", serde_json::to_string(&waybar)?);
    } else {
        match active {
            Some(entry) => {
                let elapsed_str = entry.format_duration_until(now);
                let project_name = if let Some(pid) = entry.project_id {
                    db.get_project(pid)?.map(|p| p.name)
                } else {
                    None
                };
                let tags_suffix = if entry.tags.is_empty() {
                    String::new()
                } else {
                    format!(
                        " [{}]",
                        entry
                            .tags
                            .iter()
                            .map(|t| format!("#{}", t.name))
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                };
                if let Some(proj) = project_name {
                    println!(
                        "[RUNNING] {} [{}] - {}{}",
                        entry.description, proj, elapsed_str, tags_suffix
                    );
                } else {
                    println!(
                        "[RUNNING] {} - {}{}",
                        entry.description, elapsed_str, tags_suffix
                    );
                }
            }
            None => {
                println!("[IDLE] No active timer.");
            }
        }
    }
    Ok(())
}

/// Exports recorded time entries to CSV or JSON format.
pub fn handle_export(
    db: &Database,
    format: ExportFormat,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = DateTime::<Utc>::UNIX_EPOCH;
    let end = Utc::now() + chrono::Duration::days(36500);
    let mut entries = db.get_entries(start, end)?;
    entries.sort_by_key(|e| e.start_time);

    let projects = db.list_projects(true)?;
    let project_map: HashMap<i64, String> = projects
        .into_iter()
        .filter_map(|p| p.id.map(|id| (id, p.name)))
        .collect();

    let now = Utc::now();
    let records: Vec<ExportRecord> = entries
        .into_iter()
        .map(|e| {
            let project_name = e.project_id.and_then(|id| project_map.get(&id).cloned());
            let duration_seconds = e
                .duration()
                .map(|d| d.num_seconds())
                .unwrap_or_else(|| e.duration_until(now).num_seconds());
            let tag_names = e.tags.into_iter().map(|t| t.name).collect();
            ExportRecord {
                id: e.id,
                description: e.description,
                project: project_name,
                start_time: e.start_time,
                end_time: e.end_time,
                duration_seconds,
                tags: tag_names,
            }
        })
        .collect();

    match format {
        ExportFormat::Csv => {
            if let Some(path) = output {
                let file = File::create(path)?;
                export_csv(&records, file)?;
            } else {
                export_csv(&records, io::stdout())?;
            }
        }
        ExportFormat::Json => {
            if let Some(path) = output {
                let file = File::create(path)?;
                export_json(&records, file)?;
            } else {
                export_json(&records, io::stdout())?;
            }
        }
    }

    Ok(())
}

fn export_csv<W: Write>(
    records: &[ExportRecord],
    writer: W,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wtr = csv::Writer::from_writer(writer);
    wtr.write_record([
        "id",
        "description",
        "project",
        "start_time",
        "end_time",
        "duration_seconds",
        "tags",
    ])?;
    for r in records {
        wtr.write_record([
            &r.id.map(|i| i.to_string()).unwrap_or_default(),
            &r.description,
            r.project.as_deref().unwrap_or(""),
            &r.start_time.to_rfc3339(),
            &r.end_time.map(|t| t.to_rfc3339()).unwrap_or_default(),
            &r.duration_seconds.to_string(),
            &r.tags.join(","),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}

fn export_json<W: Write>(
    records: &[ExportRecord],
    mut writer: W,
) -> Result<(), Box<dyn std::error::Error>> {
    let json_str = serde_json::to_string_pretty(records)?;
    writer.write_all(json_str.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

/// Handles cloud synchronization command with Clockify.
pub fn handle_sync(
    db: &Database,
    config: &AppConfig,
    notifications: &NotificationService,
) -> Result<(), Box<dyn std::error::Error>> {
    if !config.clockify.enabled
        || config.clockify.api_key.trim().is_empty()
        || config.clockify.workspace_id.trim().is_empty()
    {
        println!("Clockify sync is disabled or API key is not configured.");
        println!(
            "To enable sync, configure api_key and workspace_id in ~/.config/chrono-cli/config.toml"
        );
        let _ = notifications.notify_sync(false, "Clockify sync disabled or unconfigured");
        return Ok(());
    }

    println!("Synchronizing time entries with Clockify...");
    let client = ClockifyClient::new(&config.clockify.api_key)?;
    match SyncEngine::sync(&client, db, &config.clockify.workspace_id) {
        Ok(result) => {
            println!(
                "Sync completed: {} entries pushed, {} projects pulled, {} tags pulled.",
                result.pushed_entries, result.pulled_projects, result.pulled_tags
            );
            if !result.errors.is_empty() {
                println!("Sync warnings/errors ({}):", result.errors.len());
                for err in &result.errors {
                    println!("  - {err}");
                }
            }
            let _ = notifications.notify_sync(
                result.is_success(),
                &format!(
                    "Pushed {}, Pulled {} projects, {} tags",
                    result.pushed_entries, result.pulled_projects, result.pulled_tags
                ),
            );
            Ok(())
        }
        Err(e) => {
            println!("Sync failed: {e}");
            let _ = notifications.notify_sync(false, &format!("Sync failed: {e}"));
            Err(e.into())
        }
    }
}
