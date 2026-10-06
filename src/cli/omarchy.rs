//! Omarchy Quickshell status bar integration for chrono-cli.
//!
//! Provides the exact JSON API and subcommands expected by the Omarchy
//! Quickshell status bar plugin (`mrworld.clock-bar`).

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};
use std::path::PathBuf;

use chrono::{DateTime, Datelike, Duration, Local, Utc};
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::domain::{EntryMode, Project, TimeEntry};
use crate::storage::Database;

#[cfg(feature = "cloud-sync")]
use crate::clockify::{ClockifyApi, ClockifyClient};

/// Represents the active timer entry in Omarchy Quickshell payload format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OmarchyRunningEntry {
    pub id: String,
    pub description: String,
    pub project_id: String,
    pub project_name: String,
    pub project_color: String,
    pub client_name: String,
    pub billable: bool,
    pub start: String,
    pub end: String,
    pub seconds: i64,
    pub running: bool,
}

/// Project breakdown summary for today's completed study sessions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OmarchyProjectBreakdown {
    pub project_id: String,
    pub project_name: String,
    pub project_color: String,
    pub seconds: i64,
}

/// Recent finished task summary for quick resumption.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OmarchyRecentTask {
    pub id: String,
    pub description: String,
    pub project_id: String,
    pub project_name: String,
    pub project_color: String,
    pub client_name: String,
    pub billable: bool,
    pub last_start: String,
    pub last_end: String,
    pub last_duration_seconds: i64,
    pub total_seconds: i64,
    pub count: usize,
}

/// Project item for dropdown selection in Omarchy status bar panel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OmarchyProjectItem {
    pub id: String,
    pub name: String,
    pub color: String,
    pub client_name: String,
}

/// Complete snapshot payload expected by Omarchy status bar plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OmarchyPayload {
    pub ok: bool,
    pub configured: bool,
    pub error: String,
    pub note: String,
    pub user_name: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub default_project_id: String,
    pub week_start: String,
    pub running: Option<OmarchyRunningEntry>,
    pub today_seconds: i64,
    pub week_seconds: i64,
    pub today_breakdown: Vec<OmarchyProjectBreakdown>,
    pub recent_tasks: Vec<OmarchyRecentTask>,
    pub projects: Vec<OmarchyProjectItem>,
    pub projects_loaded: bool,
    pub fetched_at: String,
}

impl Default for OmarchyPayload {
    fn default() -> Self {
        Self {
            ok: true,
            configured: false,
            error: String::new(),
            note: String::new(),
            user_name: String::new(),
            workspace_id: String::new(),
            workspace_name: String::new(),
            default_project_id: String::new(),
            week_start: "monday".to_string(),
            running: None,
            today_seconds: 0,
            week_seconds: 0,
            today_breakdown: Vec::new(),
            recent_tasks: Vec::new(),
            projects: Vec::new(),
            projects_loaded: false,
            fetched_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        }
    }
}

/// Payload returned by the `set-config` subcommand.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OmarchySetConfigPayload {
    pub ok: bool,
    pub error: String,
    pub note: String,
    pub configured: bool,
    pub default_project_id: String,
    pub workspace_id: String,
    pub week_start: String,
}

/// Settings loaded from `~/.config/omarchy/clockify.json` and `AppConfig`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmarchySettings {
    #[serde(default)]
    pub api_host: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub default_project_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub workspace_name: Option<String>,
    #[serde(default)]
    pub week_start: Option<String>,
}

impl OmarchySettings {
    /// Returns the standard path to `~/.config/omarchy/clockify.json`.
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("omarchy")
            .join("clockify.json")
    }

    /// Loads settings from `~/.config/omarchy/clockify.json`, falling back to `AppConfig`.
    pub fn load(app_config: &AppConfig) -> Self {
        let mut settings = Self::default();
        let path = Self::config_path();

        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(loaded) = serde_json::from_str::<OmarchySettings>(&content) {
                    settings = loaded;
                }
            }
        }

        // Overlay with AppConfig if fields are missing in omarchy json
        if settings.api_key.as_deref().unwrap_or("").trim().is_empty()
            && !app_config.clockify.api_key.trim().is_empty()
        {
            settings.api_key = Some(app_config.clockify.api_key.clone());
        }
        if settings.workspace_id.as_deref().unwrap_or("").trim().is_empty()
            && !app_config.clockify.workspace_id.trim().is_empty()
        {
            settings.workspace_id = Some(app_config.clockify.workspace_id.clone());
        }
        if settings.week_start.as_deref().unwrap_or("").trim().is_empty() {
            settings.week_start = Some("monday".to_string());
        }

        settings
    }

    /// Saves settings back to `~/.config/omarchy/clockify.json`.
    pub fn save(&self) -> io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json_str = serde_json::to_string_pretty(self)?;
        fs::write(path, json_str)?;
        Ok(())
    }
}

/// Helper to get local midnight converted to UTC DateTime.
fn get_local_midnight_utc() -> DateTime<Utc> {
    let now_local = Local::now();
    let midnight_local = now_local.date_naive().and_hms_opt(0, 0, 0).unwrap();
    midnight_local
        .and_local_timezone(Local)
        .earliest()
        .unwrap_or_else(|| now_local)
        .with_timezone(&Utc)
}

/// Helper to get week start midnight converted to UTC DateTime.
fn get_week_start_utc(week_start_day: &str) -> DateTime<Utc> {
    let now_local = Local::now();
    let midnight_local = now_local.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let offset = if week_start_day.eq_ignore_ascii_case("sunday") {
        (now_local.weekday().num_days_from_sunday()) as i64
    } else {
        (now_local.weekday().num_days_from_monday()) as i64
    };
    let week_start_local = midnight_local - Duration::days(offset);
    week_start_local
        .and_local_timezone(Local)
        .earliest()
        .unwrap_or_else(|| now_local)
        .with_timezone(&Utc)
}

/// Synchronizes remote Clockify projects into SQLite if projects table is empty
/// and Clockify credentials are available.
pub fn sync_projects_if_needed(db: &mut Database, settings: &OmarchySettings) {
    #[cfg(feature = "cloud-sync")]
    {
        if let (Some(key), Some(ws)) = (&settings.api_key, &settings.workspace_id) {
            let key = key.trim();
            let ws = ws.trim();
            if !key.is_empty() && !ws.is_empty() {
                if let Ok(projects) = db.list_projects(true) {
                    if projects.is_empty() {
                        if let Ok(client) = ClockifyClient::new(key) {
                            if let Ok(remote_projects) = client.get_projects(ws) {
                                for rp in remote_projects {
                                    let mut p = Project::new(&rp.name).unwrap_or_else(|_| {
                                        Project {
                                            id: None,
                                            name: rp.name.clone(),
                                            color: rp.color.clone().unwrap_or_else(|| "#3498db".to_string()),
                                            target_hours_week: 0.0,
                                            clockify_id: Some(rp.id.clone()),
                                            archived: rp.archived,
                                            created_at: Utc::now(),
                                        }
                                    });
                                    if let Some(col) = rp.color {
                                        p = p.with_color(col);
                                    }
                                    p = p.with_clockify_id(rp.id).with_archived(rp.archived);
                                    let _ = db.create_project(&p);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    #[cfg(not(feature = "cloud-sync"))]
    {
        let _ = (db, settings);
    }
}

/// Builds the complete snapshot payload matching the schema expected by `mrworld.clock-bar`.
pub fn build_omarchy_snapshot(
    db: &Database,
    settings: &OmarchySettings,
    with_projects: bool,
    note: Option<String>,
    error: Option<String>,
) -> OmarchyPayload {
    let now_utc = Utc::now();
    let midnight_utc = get_local_midnight_utc();
    let week_start_day = settings.week_start.as_deref().unwrap_or("monday");
    let week_start_utc = get_week_start_utc(week_start_day);

    let mut payload = OmarchyPayload::default();
    payload.ok = error.is_none();
    payload.error = error.unwrap_or_default();
    payload.note = note.unwrap_or_default();
    payload.user_name = settings.user_name.clone().unwrap_or_else(|| {
        std::env::var("USER").unwrap_or_else(|_| "User".to_string())
    });
    payload.workspace_id = settings.workspace_id.clone().unwrap_or_default();
    payload.workspace_name = settings.workspace_name.clone().unwrap_or_default();
    payload.default_project_id = settings.default_project_id.clone().unwrap_or_default();
    payload.week_start = week_start_day.to_string();
    payload.configured = true;
    payload.fetched_at = now_utc.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    // 1. Projects map and listing
    let all_projects = db.list_projects(true).unwrap_or_default();
    let mut projects_by_id: HashMap<i64, Project> = HashMap::new();
    let mut projects_by_clockify_id: HashMap<String, Project> = HashMap::new();

    for p in &all_projects {
        if let Some(id) = p.id {
            projects_by_id.insert(id, p.clone());
        }
        if let Some(cid) = &p.clockify_id {
            projects_by_clockify_id.insert(cid.clone(), p.clone());
        }
    }

    if with_projects {
        payload.projects = all_projects
            .iter()
            .filter(|p| !p.archived)
            .map(|p| OmarchyProjectItem {
                id: p.clockify_id.clone().unwrap_or_else(|| p.id.unwrap_or(0).to_string()),
                name: p.name.clone(),
                color: p.color.clone(),
                client_name: String::new(),
            })
            .collect();
        payload.projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        payload.projects_loaded = true;
    }

    // 2. Active running timer
    if let Ok(Some(active)) = db.get_active_entry() {
        let (p_id, p_name, p_color) = if let Some(pid) = active.project_id {
            if let Some(p) = projects_by_id.get(&pid) {
                let id_str = p.clockify_id.clone().unwrap_or_else(|| pid.to_string());
                (id_str, p.name.clone(), p.color.clone())
            } else {
                (pid.to_string(), "General".to_string(), "#3498db".to_string())
            }
        } else {
            (String::new(), String::new(), String::new())
        };

        let start_str = active.start_time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let active_id = active.clockify_id.unwrap_or_else(|| active.id.unwrap_or(0).to_string());

        payload.running = Some(OmarchyRunningEntry {
            id: active_id,
            description: active.description,
            project_id: p_id,
            project_name: p_name,
            project_color: p_color,
            client_name: String::new(),
            billable: false,
            start: start_str,
            end: String::new(),
            seconds: 0,
            running: true,
        });
    }

    // 3. Query finished entries for today and this week
    let past_start = week_start_utc.min(midnight_utc) - Duration::days(30);
    let all_recent_entries = db.get_entries(past_start, now_utc).unwrap_or_default();

    let mut today_secs: i64 = 0;
    let mut week_secs: i64 = 0;
    let mut today_projects_map: HashMap<String, (String, String, i64)> = HashMap::new();

    for entry in &all_recent_entries {
        // Skip active entries and pomodoro breaks for study time calculation
        if entry.end_time.is_none() || entry.entry_mode == EntryMode::PomodoroBreak {
            continue;
        }

        let end = entry.end_time.unwrap();
        let dur = (end - entry.start_time).num_seconds().max(0);

        if entry.start_time >= week_start_utc {
            week_secs += dur;
        }

        if entry.start_time >= midnight_utc {
            today_secs += dur;

            let (pid_str, pname, pcolor) = if let Some(pid) = entry.project_id {
                if let Some(p) = projects_by_id.get(&pid) {
                    let id_str = p.clockify_id.clone().unwrap_or_else(|| pid.to_string());
                    (id_str, p.name.clone(), p.color.clone())
                } else {
                    (pid.to_string(), "General".to_string(), "#3498db".to_string())
                }
            } else {
                (String::new(), "General".to_string(), String::new())
            };

            let entry_slot = today_projects_map
                .entry(pid_str)
                .or_insert_with(|| (pname, pcolor, 0));
            entry_slot.2 += dur;
        }
    }

    payload.today_seconds = today_secs;
    payload.week_seconds = week_secs;

    let mut breakdown_vec: Vec<OmarchyProjectBreakdown> = today_projects_map
        .into_iter()
        .map(|(pid, (name, color, secs))| OmarchyProjectBreakdown {
            project_id: pid,
            project_name: name,
            project_color: color,
            seconds: secs,
        })
        .collect();
    breakdown_vec.sort_by(|a, b| b.seconds.cmp(&a.seconds));
    payload.today_breakdown = breakdown_vec;

    // 4. Group recent tasks
    let mut task_groups: Vec<OmarchyRecentTask> = Vec::new();
    let mut seen_keys: HashMap<(String, String), usize> = HashMap::new();

    for entry in &all_recent_entries {
        if entry.end_time.is_none() || entry.entry_mode == EntryMode::PomodoroBreak {
            continue;
        }

        let desc = entry.description.trim().to_string();
        let (pid_str, pname, pcolor) = if let Some(pid) = entry.project_id {
            if let Some(p) = projects_by_id.get(&pid) {
                let id_str = p.clockify_id.clone().unwrap_or_else(|| pid.to_string());
                (id_str, p.name.clone(), p.color.clone())
            } else {
                (pid.to_string(), "General".to_string(), "#3498db".to_string())
            }
        } else {
            (String::new(), String::new(), String::new())
        };

        let key = (desc.clone(), pid_str.clone());
        let dur = (entry.end_time.unwrap() - entry.start_time).num_seconds().max(0);

        if let Some(&idx) = seen_keys.get(&key) {
            task_groups[idx].total_seconds += dur;
            task_groups[idx].count += 1;
        } else {
            let task_id = entry.clockify_id.clone().unwrap_or_else(|| entry.id.unwrap_or(0).to_string());
            let start_str = entry.start_time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let end_str = entry.end_time.unwrap().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

            let new_task = OmarchyRecentTask {
                id: task_id,
                description: desc,
                project_id: pid_str,
                project_name: pname,
                project_color: pcolor,
                client_name: String::new(),
                billable: false,
                last_start: start_str,
                last_end: end_str,
                last_duration_seconds: dur,
                total_seconds: dur,
                count: 1,
            };
            seen_keys.insert(key, task_groups.len());
            task_groups.push(new_task);
        }

        if task_groups.len() >= 15 {
            break;
        }
    }

    payload.recent_tasks = task_groups;
    payload
}

/// Resolves a project from SQLite by ID, Clockify ID, or name.
fn resolve_project(db: &mut Database, project_ident: &str) -> Option<i64> {
    let trimmed = project_ident.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try parsing as integer local ID
    if let Ok(id) = trimmed.parse::<i64>() {
        if let Ok(Some(p)) = db.get_project(id) {
            return p.id;
        }
    }

    // Try matching clockify_id
    if let Ok(all) = db.list_projects(true) {
        for p in all {
            if let Some(ref cid) = p.clockify_id {
                if cid.eq_ignore_ascii_case(trimmed) {
                    return p.id;
                }
            }
        }
    }

    // Try matching name
    if let Ok(Some(p)) = db.get_project_by_name(trimmed) {
        return p.id;
    }

    // If not found, create new project with this name
    if let Ok(created) = db.create_project(&Project::new(trimmed).ok()?) {
        return created.id;
    }

    None
}

/// Handles `chrono omarchy status [--projects]`.
pub fn handle_omarchy_status(
    db: &mut Database,
    app_config: &AppConfig,
    projects: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = OmarchySettings::load(app_config);
    if projects {
        sync_projects_if_needed(db, &settings);
    }
    let payload = build_omarchy_snapshot(db, &settings, projects, None, None);
    println!("{}", serde_json::to_string(&payload)?);
    Ok(())
}

/// Handles `chrono omarchy start [--description <desc>] [--project <proj>] [--billable]`.
pub fn handle_omarchy_start(
    db: &mut Database,
    app_config: &AppConfig,
    description: Option<String>,
    project: Option<String>,
    _billable: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut settings = OmarchySettings::load(app_config);

    // If no project passed, fall back to default project
    let proj_str = project
        .as_deref()
        .map(str::trim)
        .or_else(|| settings.default_project_id.as_deref())
        .unwrap_or("");

    let project_id = if !proj_str.is_empty() {
        resolve_project(db, proj_str)
    } else {
        None
    };

    if let Some(ref p) = project {
        let p_trimmed = p.trim();
        if !p_trimmed.is_empty() && settings.default_project_id.as_deref() != Some(p_trimmed) {
            settings.default_project_id = Some(p_trimmed.to_string());
            let _ = settings.save();
        }
    }

    let desc = description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");

    let mut entry = TimeEntry::new(desc, Utc::now());
    if let Some(pid) = project_id {
        entry = entry.with_project_id(pid);
    }

    let _ = db.start_entry(&entry)?;
    let payload = build_omarchy_snapshot(
        db,
        &settings,
        false,
        Some("Timer started".to_string()),
        None,
    );
    println!("{}", serde_json::to_string(&payload)?);
    Ok(())
}

/// Handles `chrono omarchy stop`.
pub fn handle_omarchy_stop(
    db: &mut Database,
    app_config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = OmarchySettings::load(app_config);
    let stopped = db.stop_active_entry()?;
    let (note, error) = if stopped.is_some() {
        (Some("Session stopped".to_string()), None)
    } else {
        (None, Some("No session is running".to_string()))
    };

    let payload = build_omarchy_snapshot(db, &settings, false, note, error);
    println!("{}", serde_json::to_string(&payload)?);
    Ok(())
}

/// Handles `chrono omarchy discard`.
pub fn handle_omarchy_discard(
    db: &mut Database,
    app_config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = OmarchySettings::load(app_config);
    let active = db.get_active_entry()?;

    let (note, error) = if let Some(entry) = active {
        if let Some(id) = entry.id {
            let _ = db.delete_entry(id);
        }
        (Some("Session discarded".to_string()), None)
    } else {
        (None, Some("No running session to discard".to_string()))
    };

    let payload = build_omarchy_snapshot(db, &settings, false, note, error);
    println!("{}", serde_json::to_string(&payload)?);
    Ok(())
}

/// Handles `chrono omarchy continue [--description <desc>] [--project <proj>]`.
pub fn handle_omarchy_continue(
    db: &mut Database,
    app_config: &AppConfig,
    description: Option<String>,
    project: Option<String>,
    billable: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = OmarchySettings::load(app_config);

    let (desc_to_use, proj_to_use) = match (&description, &project) {
        (Some(d), Some(p)) => (d.clone(), p.clone()),
        (Some(d), None) => (
            d.clone(),
            settings.default_project_id.clone().unwrap_or_default(),
        ),
        (None, Some(p)) => (String::new(), p.clone()),
        (None, None) => {
            // Find most recently finished task
            let snapshot = build_omarchy_snapshot(db, &settings, false, None, None);
            if let Some(task) = snapshot.recent_tasks.first() {
                (task.description.clone(), task.project_id.clone())
            } else {
                let payload = build_omarchy_snapshot(
                    db,
                    &settings,
                    false,
                    None,
                    Some("No previous session to continue".to_string()),
                );
                println!("{}", serde_json::to_string(&payload)?);
                return Ok(());
            }
        }
    };

    handle_omarchy_start(
        db,
        app_config,
        Some(desc_to_use),
        Some(proj_to_use),
        billable,
    )
}

/// Handles `chrono omarchy set-config [--project <proj>] [--workspace <ws>] [--week-start <day>]`.
pub fn handle_omarchy_set_config(
    app_config: &AppConfig,
    project: Option<String>,
    workspace: Option<String>,
    week_start: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut settings = OmarchySettings::load(app_config);

    if let Some(p) = project {
        settings.default_project_id = Some(p.trim().to_string());
    }
    if let Some(w) = workspace {
        settings.workspace_id = Some(w.trim().to_string());
    }
    if let Some(ws) = week_start {
        settings.week_start = Some(ws.trim().to_lowercase());
    }

    let _ = settings.save();

    let res = OmarchySetConfigPayload {
        ok: true,
        error: String::new(),
        note: "Saved".to_string(),
        configured: true,
        default_project_id: settings.default_project_id.unwrap_or_default(),
        workspace_id: settings.workspace_id.unwrap_or_default(),
        week_start: settings.week_start.unwrap_or_else(|| "monday".to_string()),
    };
    println!("{}", serde_json::to_string(&res)?);
    Ok(())
}

/// Handles `chrono omarchy set-key`.
pub fn handle_omarchy_set_key(
    db: &mut Database,
    app_config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    let key = line.trim().to_string();

    if key.is_empty() {
        let payload = OmarchyPayload {
            ok: false,
            error: "No API key received".to_string(),
            ..Default::default()
        };
        println!("{}", serde_json::to_string(&payload)?);
        return Ok(());
    }

    let mut settings = OmarchySettings::load(app_config);
    settings.api_key = Some(key.clone());

    #[cfg(feature = "cloud-sync")]
    {
        if let Ok(client) = ClockifyClient::new(&key) {
            if let Ok(user) = client.get_user() {
                settings.user_name = Some(user.name);
                if let Some(ws) = user.active_workspace.or(user.default_workspace) {
                    settings.workspace_id = Some(ws);
                }
            }
        }
    }

    let _ = settings.save();
    sync_projects_if_needed(db, &settings);

    let user_display = settings.user_name.as_deref().unwrap_or("your account");
    let payload = build_omarchy_snapshot(
        db,
        &settings,
        true,
        Some(format!("Connected as {}", user_display)),
        None,
    );
    println!("{}", serde_json::to_string(&payload)?);
    Ok(())
}

/// Handles `chrono omarchy clear-key`.
pub fn handle_omarchy_clear_key(
    db: &mut Database,
    app_config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut settings = OmarchySettings::load(app_config);
    settings.api_key = None;
    settings.workspace_id = None;
    settings.user_id = None;
    settings.user_name = None;
    let _ = settings.save();

    let mut payload = build_omarchy_snapshot(
        db,
        &settings,
        false,
        Some("API key removed".to_string()),
        None,
    );
    payload.configured = false;
    println!("{}", serde_json::to_string(&payload)?);
    Ok(())
}
