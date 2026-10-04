//! Clockify two-way synchronization engine.

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::domain::{TimeEntry, DEFAULT_PROJECT_COLOR};
use crate::storage::repository::{format_dt, parse_dt};
use crate::storage::Database;

use super::client::ClockifyApi;
use super::models::CreateTimeEntryRequest;
use super::SyncError;

/// Maps a local `TimeEntry` to a remote Clockify `CreateTimeEntryRequest`.
pub fn entry_to_create_request(
    entry: &TimeEntry,
    remote_project_id: Option<String>,
    remote_tag_ids: Vec<String>,
) -> CreateTimeEntryRequest {
    CreateTimeEntryRequest {
        start: entry.start_time,
        end: entry.end_time,
        description: entry.description.clone(),
        project_id: remote_project_id,
        tag_ids: if remote_tag_ids.is_empty() {
            None
        } else {
            Some(remote_tag_ids)
        },
        billable: false,
    }
}

/// Summary report of synchronization operations performed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncResult {
    /// Number of local time entries pushed to Clockify.
    pub pushed_entries: usize,
    /// Number of remote projects pulled or updated locally.
    pub pulled_projects: usize,
    /// Number of remote tags pulled or updated locally.
    pub pulled_tags: usize,
    /// Non-fatal error and warning messages encountered during sync.
    pub errors: Vec<String>,
}

impl SyncResult {
    /// Creates a new empty `SyncResult`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if no errors occurred during sync.
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the total count of synchronized entities.
    pub fn total_synced(&self) -> usize {
        self.pushed_entries + self.pulled_projects + self.pulled_tags
    }
}

/// Synchronization engine coordinating Clockify REST API calls and local SQLite database state.
pub struct SyncEngine;

impl SyncEngine {
    /// Maps a local `TimeEntry` to a `CreateTimeEntryRequest`.
    pub fn map_entry_to_request(
        entry: &TimeEntry,
        remote_project_id: Option<String>,
        remote_tag_ids: Vec<String>,
    ) -> CreateTimeEntryRequest {
        entry_to_create_request(entry, remote_project_id, remote_tag_ids)
    }

    /// Synchronizes projects, tags, and completed time entries between Clockify and local SQLite storage.
    pub fn sync<C: ClockifyApi>(
        client: &C,
        db: &Database,
        workspace_id: &str,
    ) -> Result<SyncResult, SyncError> {
        let ws = workspace_id.trim();
        if ws.is_empty() {
            return Err(SyncError::MissingWorkspace);
        }

        let mut result = SyncResult::new();

        // 1. Pull remote tags and upsert in SQLite
        let remote_tags = client.get_tags(ws)?;
        for tag in remote_tags {
            Self::upsert_tag(db, &tag)?;
            result.pulled_tags += 1;
        }

        // 2. Pull remote projects and upsert in SQLite
        let remote_projects = client.get_projects(ws)?;
        for proj in remote_projects {
            Self::upsert_project(db, &proj)?;
            result.pulled_projects += 1;
        }

        // 3. Query unsynced completed entries from SQLite
        let unsynced_entries = Self::query_unsynced_completed_entries(db)?;

        // 4. Push each unsynced entry to Clockify
        for entry in unsynced_entries {
            let entry_id = match entry.id {
                Some(id) => id,
                None => continue,
            };

            // Resolve project's remote clockify_id
            let remote_project_id = if let Some(pid) = entry.project_id {
                db.conn()
                    .query_row(
                        "SELECT clockify_id FROM projects WHERE id = ?1;",
                        [pid],
                        |row| row.get::<_, Option<String>>(0),
                    )
                    .optional()?
                    .flatten()
            } else {
                None
            };

            // Resolve tags' remote clockify_ids
            let mut tag_stmt = db.conn().prepare(
                "SELECT t.clockify_id
                 FROM tags t
                 INNER JOIN entry_tags et ON et.tag_id = t.id
                 WHERE et.entry_id = ?1;",
            )?;
            let tag_rows = tag_stmt.query_map([entry_id], |row| row.get::<_, Option<String>>(0))?;
            let mut remote_tag_ids = Vec::new();
            for r in tag_rows {
                if let Some(cid) = r? {
                    if !cid.trim().is_empty() {
                        remote_tag_ids.push(cid);
                    }
                }
            }

            let req = entry_to_create_request(&entry, remote_project_id, remote_tag_ids);
            match client.create_time_entry(ws, &req) {
                Ok(resp) => {
                    let now_str = format_dt(&Utc::now());
                    db.conn().execute(
                        "UPDATE time_entries
                         SET synced = 1, clockify_id = ?1, updated_at = ?2
                         WHERE id = ?3;",
                        params![&resp.id, now_str, entry_id],
                    )?;
                    result.pushed_entries += 1;
                }
                Err(err) => {
                    result
                        .errors
                        .push(format!("Failed to sync entry {entry_id}: {err}"));
                }
            }
        }

        Ok(result)
    }

    fn upsert_tag(db: &Database, tag: &super::models::ClockifyTag) -> Result<(), SyncError> {
        let existing_by_clockify_id: Option<i64> = db
            .conn()
            .query_row(
                "SELECT id FROM tags WHERE clockify_id = ?1;",
                [&tag.id],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing_by_clockify_id {
            db.conn().execute(
                "UPDATE tags SET name = ?1 WHERE id = ?2;",
                params![&tag.name, id],
            )?;
        } else {
            let existing_by_name: Option<i64> = db
                .conn()
                .query_row("SELECT id FROM tags WHERE name = ?1;", [&tag.name], |row| {
                    row.get(0)
                })
                .optional()?;

            if let Some(id) = existing_by_name {
                db.conn().execute(
                    "UPDATE tags SET clockify_id = ?1 WHERE id = ?2;",
                    params![&tag.id, id],
                )?;
            } else {
                db.conn().execute(
                    "INSERT INTO tags (name, clockify_id) VALUES (?1, ?2);",
                    params![&tag.name, &tag.id],
                )?;
            }
        }

        Ok(())
    }

    fn upsert_project(
        db: &Database,
        proj: &super::models::ClockifyProject,
    ) -> Result<(), SyncError> {
        let color = proj.color.as_deref().unwrap_or(DEFAULT_PROJECT_COLOR);
        let archived_int = if proj.archived { 1 } else { 0 };

        let existing_by_clockify_id: Option<i64> = db
            .conn()
            .query_row(
                "SELECT id FROM projects WHERE clockify_id = ?1;",
                [&proj.id],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing_by_clockify_id {
            db.conn().execute(
                "UPDATE projects SET name = ?1, color = ?2, archived = ?3 WHERE id = ?4;",
                params![&proj.name, color, archived_int, id],
            )?;
        } else {
            let existing_by_name: Option<i64> = db
                .conn()
                .query_row(
                    "SELECT id FROM projects WHERE name = ?1;",
                    [&proj.name],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(id) = existing_by_name {
                db.conn().execute(
                    "UPDATE projects SET clockify_id = ?1, color = ?2, archived = ?3 WHERE id = ?4;",
                    params![&proj.id, color, archived_int, id],
                )?;
            } else {
                let now_str = format_dt(&Utc::now());
                db.conn().execute(
                    "INSERT INTO projects (name, color, target_hours_week, clockify_id, archived, created_at)
                     VALUES (?1, ?2, 0.0, ?3, ?4, ?5);",
                    params![&proj.name, color, &proj.id, archived_int, now_str],
                )?;
            }
        }

        Ok(())
    }

    fn query_unsynced_completed_entries(db: &Database) -> Result<Vec<TimeEntry>, SyncError> {
        let mut stmt = db.conn().prepare(
            "SELECT id, description, project_id, start_time, end_time, entry_mode,
                    pomodoro_index, synced, clockify_id, created_at, updated_at
             FROM time_entries
             WHERE (synced = 0 OR synced IS NULL) AND end_time IS NOT NULL
             ORDER BY start_time ASC;",
        )?;

        let mut rows = stmt.query([])?;
        let mut entries = Vec::new();

        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let description: String = row.get(1)?;
            let project_id: Option<i64> = row.get(2)?;
            let start_time_str: String = row.get(3)?;
            let end_time_str: Option<String> = row.get(4)?;
            let mode_str: String = row.get(5)?;
            let pomodoro_index: u32 = row.get(6)?;
            let synced_int: i32 = row.get(7)?;
            let clockify_id: Option<String> = row.get(8)?;
            let created_at_str: String = row.get(9)?;
            let updated_at_str: String = row.get(10)?;

            let entry_mode = mode_str.parse().unwrap_or_default();
            let start_time = parse_dt(&start_time_str)?;
            let end_time = match end_time_str {
                Some(s) => Some(parse_dt(&s)?),
                None => None,
            };

            entries.push(TimeEntry {
                id: Some(id),
                description,
                project_id,
                start_time,
                end_time,
                entry_mode,
                pomodoro_index,
                tags: Vec::new(),
                synced: synced_int != 0,
                clockify_id,
                created_at: parse_dt(&created_at_str)?,
                updated_at: parse_dt(&updated_at_str)?,
            });
        }

        Ok(entries)
    }
}
