use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::{EntryMode, Project, Tag, TimeEntry};

use super::db::Database;
use super::StorageError;

/// Storage repository providing CRUD and query operations for projects, tags, and time entries.
pub trait Repository {
    // Project operations
    fn create_project(&mut self, project: &Project) -> Result<Project, StorageError>;
    fn get_project(&self, id: i64) -> Result<Option<Project>, StorageError>;
    fn get_project_by_name(&self, name: &str) -> Result<Option<Project>, StorageError>;
    fn list_projects(&self, include_archived: bool) -> Result<Vec<Project>, StorageError>;
    fn update_project(&mut self, project: &Project) -> Result<(), StorageError>;
    fn delete_project(&mut self, id: i64) -> Result<(), StorageError>;

    // Tag operations
    fn create_tag(&mut self, tag: &Tag) -> Result<Tag, StorageError>;
    fn get_tag(&self, id: i64) -> Result<Option<Tag>, StorageError>;
    fn get_tag_by_name(&self, name: &str) -> Result<Option<Tag>, StorageError>;
    fn list_tags(&self) -> Result<Vec<Tag>, StorageError>;
    fn delete_tag(&mut self, id: i64) -> Result<(), StorageError>;

    // TimeEntry operations
    fn start_entry(&mut self, entry: &TimeEntry) -> Result<TimeEntry, StorageError>;
    fn get_active_entry(&self) -> Result<Option<TimeEntry>, StorageError>;
    fn stop_active_entry(&mut self) -> Result<Option<TimeEntry>, StorageError>;
    fn create_manual_entry(&mut self, entry: &TimeEntry) -> Result<TimeEntry, StorageError>;
    fn get_entry(&self, id: i64) -> Result<Option<TimeEntry>, StorageError>;
    fn get_entries(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<TimeEntry>, StorageError>;
    fn update_entry(&mut self, entry: &TimeEntry) -> Result<(), StorageError>;
    fn delete_entry(&mut self, id: i64) -> Result<(), StorageError>;
}

pub(crate) fn format_dt(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

pub(crate) fn parse_dt(s: &str) -> Result<DateTime<Utc>, StorageError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f")
                        .map(|ndt| DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
                })
        })
        .map_err(|e| StorageError::InvalidData(format!("Failed to parse timestamp '{s}': {e}")))
}

fn row_to_project(row: &rusqlite::Row<'_>) -> Result<Project, StorageError> {
    let id: i64 = row.get(0)?;
    let name: String = row.get(1)?;
    let color: String = row.get(2)?;
    let target_hours_week: f64 = row.get(3)?;
    let clockify_id: Option<String> = row.get(4)?;
    let archived_int: i32 = row.get(5)?;
    let created_at_str: String = row.get(6)?;

    Ok(Project {
        id: Some(id),
        name,
        color,
        target_hours_week,
        clockify_id,
        archived: archived_int != 0,
        created_at: parse_dt(&created_at_str)?,
    })
}

fn row_to_tag(row: &rusqlite::Row<'_>) -> Result<Tag, StorageError> {
    let id: i64 = row.get(0)?;
    let name: String = row.get(1)?;
    let clockify_id: Option<String> = row.get(2)?;

    Ok(Tag {
        id: Some(id),
        name,
        clockify_id,
    })
}

fn row_to_entry_base(row: &rusqlite::Row<'_>) -> Result<TimeEntry, StorageError> {
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

    let entry_mode = mode_str.parse::<EntryMode>().unwrap_or_default();
    let start_time = parse_dt(&start_time_str)?;
    let end_time = match end_time_str {
        Some(s) => Some(parse_dt(&s)?),
        None => None,
    };

    Ok(TimeEntry {
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
    })
}

fn get_tags_for_entry(conn: &Connection, entry_id: i64) -> Result<Vec<Tag>, StorageError> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.clockify_id
         FROM tags t
         INNER JOIN entry_tags et ON et.tag_id = t.id
         WHERE et.entry_id = ?1
         ORDER BY t.name ASC;",
    )?;

    let mut rows = stmt.query([entry_id])?;
    let mut tags = Vec::new();
    while let Some(row) = rows.next()? {
        tags.push(row_to_tag(row)?);
    }
    Ok(tags)
}

fn link_tag(conn: &Connection, entry_id: i64, tag: &Tag) -> Result<(), StorageError> {
    let tag_id = if let Some(id) = tag.id {
        let exists: bool = conn
            .query_row("SELECT 1 FROM tags WHERE id = ?1;", [id], |_| Ok(()))
            .optional()?
            .is_some();
        if exists {
            id
        } else {
            conn.execute(
                "INSERT INTO tags (id, name, clockify_id) VALUES (?1, ?2, ?3);",
                params![id, &tag.name, &tag.clockify_id],
            )?;
            id
        }
    } else {
        let found_id: Option<i64> = conn
            .query_row("SELECT id FROM tags WHERE name = ?1;", [&tag.name], |row| {
                row.get(0)
            })
            .optional()?;
        if let Some(id) = found_id {
            id
        } else {
            conn.execute(
                "INSERT INTO tags (name, clockify_id) VALUES (?1, ?2);",
                params![&tag.name, &tag.clockify_id],
            )?;
            conn.last_insert_rowid()
        }
    };

    conn.execute(
        "INSERT INTO entry_tags (entry_id, tag_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING;",
        params![entry_id, tag_id],
    )?;

    Ok(())
}

impl Repository for Database {
    fn create_project(&mut self, project: &Project) -> Result<Project, StorageError> {
        project.validate()?;

        if self.get_project_by_name(&project.name)?.is_some() {
            return Err(StorageError::Duplicate(format!(
                "Project with name '{}' already exists",
                project.name
            )));
        }

        self.conn_mut().execute(
            "INSERT INTO projects (name, color, target_hours_week, clockify_id, archived, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
            params![
                &project.name,
                &project.color,
                project.target_hours_week,
                &project.clockify_id,
                project.archived as i32,
                format_dt(&project.created_at),
            ],
        )?;

        let id = self.conn().last_insert_rowid();
        let mut created = project.clone();
        created.id = Some(id);
        Ok(created)
    }

    fn get_project(&self, id: i64) -> Result<Option<Project>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id, name, color, target_hours_week, clockify_id, archived, created_at
             FROM projects WHERE id = ?1;",
        )?;

        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_project(row)?))
        } else {
            Ok(None)
        }
    }

    fn get_project_by_name(&self, name: &str) -> Result<Option<Project>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id, name, color, target_hours_week, clockify_id, archived, created_at
             FROM projects WHERE name = ?1;",
        )?;

        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_project(row)?))
        } else {
            Ok(None)
        }
    }

    fn list_projects(&self, include_archived: bool) -> Result<Vec<Project>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id, name, color, target_hours_week, clockify_id, archived, created_at
             FROM projects
             WHERE (?1 = 1 OR archived = 0)
             ORDER BY name ASC;",
        )?;

        let mut rows = stmt.query([include_archived as i32])?;
        let mut projects = Vec::new();
        while let Some(row) = rows.next()? {
            projects.push(row_to_project(row)?);
        }
        Ok(projects)
    }

    fn update_project(&mut self, project: &Project) -> Result<(), StorageError> {
        let id = project
            .id
            .ok_or_else(|| StorageError::NotFound("Project ID is missing".to_string()))?;
        project.validate()?;

        let dup: Option<i64> = self
            .conn()
            .query_row(
                "SELECT id FROM projects WHERE name = ?1 AND id != ?2;",
                params![&project.name, id],
                |row| row.get(0),
            )
            .optional()?;

        if dup.is_some() {
            return Err(StorageError::Duplicate(format!(
                "Project with name '{}' already exists",
                project.name
            )));
        }

        let rows = self.conn_mut().execute(
            "UPDATE projects
             SET name = ?1, color = ?2, target_hours_week = ?3, clockify_id = ?4, archived = ?5
             WHERE id = ?6;",
            params![
                &project.name,
                &project.color,
                project.target_hours_week,
                &project.clockify_id,
                project.archived as i32,
                id,
            ],
        )?;

        if rows == 0 {
            return Err(StorageError::NotFound(format!(
                "Project with ID {id} not found"
            )));
        }

        Ok(())
    }

    fn delete_project(&mut self, id: i64) -> Result<(), StorageError> {
        self.conn_mut()
            .execute("DELETE FROM projects WHERE id = ?1;", [id])?;
        Ok(())
    }

    fn create_tag(&mut self, tag: &Tag) -> Result<Tag, StorageError> {
        tag.validate()?;

        if self.get_tag_by_name(&tag.name)?.is_some() {
            return Err(StorageError::Duplicate(format!(
                "Tag with name '{}' already exists",
                tag.name
            )));
        }

        self.conn_mut().execute(
            "INSERT INTO tags (name, clockify_id) VALUES (?1, ?2);",
            params![&tag.name, &tag.clockify_id],
        )?;

        let id = self.conn().last_insert_rowid();
        let mut created = tag.clone();
        created.id = Some(id);
        Ok(created)
    }

    fn get_tag(&self, id: i64) -> Result<Option<Tag>, StorageError> {
        let mut stmt = self
            .conn()
            .prepare("SELECT id, name, clockify_id FROM tags WHERE id = ?1;")?;

        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_tag(row)?))
        } else {
            Ok(None)
        }
    }

    fn get_tag_by_name(&self, name: &str) -> Result<Option<Tag>, StorageError> {
        let mut stmt = self
            .conn()
            .prepare("SELECT id, name, clockify_id FROM tags WHERE name = ?1;")?;

        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_tag(row)?))
        } else {
            Ok(None)
        }
    }

    fn list_tags(&self) -> Result<Vec<Tag>, StorageError> {
        let mut stmt = self
            .conn()
            .prepare("SELECT id, name, clockify_id FROM tags ORDER BY name ASC;")?;

        let mut rows = stmt.query([])?;
        let mut tags = Vec::new();
        while let Some(row) = rows.next()? {
            tags.push(row_to_tag(row)?);
        }
        Ok(tags)
    }

    fn delete_tag(&mut self, id: i64) -> Result<(), StorageError> {
        self.conn_mut()
            .execute("DELETE FROM tags WHERE id = ?1;", [id])?;
        Ok(())
    }

    fn start_entry(&mut self, entry: &TimeEntry) -> Result<TimeEntry, StorageError> {
        entry.validate()?;

        let now = Utc::now();
        let tx = self.conn_mut().transaction()?;

        // Stop any currently active entries
        {
            let mut stmt =
                tx.prepare("SELECT id, start_time FROM time_entries WHERE end_time IS NULL;")?;
            let active_rows = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;

            let mut to_stop = Vec::new();
            for r in active_rows {
                to_stop.push(r?);
            }
            drop(stmt);

            for (active_id, active_start_str) in to_stop {
                let active_start = parse_dt(&active_start_str)?;
                let stop_time = if entry.start_time >= active_start {
                    entry.start_time
                } else {
                    now
                };
                tx.execute(
                    "UPDATE time_entries SET end_time = ?1, updated_at = ?2 WHERE id = ?3;",
                    params![format_dt(&stop_time), format_dt(&now), active_id],
                )?;
            }
        }

        // Insert new active entry
        tx.execute(
            "INSERT INTO time_entries (
                description, project_id, start_time, end_time, entry_mode,
                pomodoro_index, synced, clockify_id, created_at, updated_at
            ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9);",
            params![
                &entry.description,
                entry.project_id,
                format_dt(&entry.start_time),
                entry.entry_mode.as_str(),
                entry.pomodoro_index,
                entry.synced as i32,
                &entry.clockify_id,
                format_dt(&entry.created_at),
                format_dt(&now),
            ],
        )?;

        let entry_id = tx.last_insert_rowid();

        for tag in &entry.tags {
            link_tag(&tx, entry_id, tag)?;
        }

        tx.commit()?;

        self.get_entry(entry_id)?
            .ok_or_else(|| StorageError::NotFound(format!("Time entry {entry_id} not found")))
    }

    fn get_active_entry(&self) -> Result<Option<TimeEntry>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id, description, project_id, start_time, end_time, entry_mode,
                    pomodoro_index, synced, clockify_id, created_at, updated_at
             FROM time_entries
             WHERE end_time IS NULL
             ORDER BY start_time DESC
             LIMIT 1;",
        )?;

        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let mut entry = row_to_entry_base(row)?;
            if let Some(id) = entry.id {
                entry.tags = get_tags_for_entry(self.conn(), id)?;
            }
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    fn stop_active_entry(&mut self) -> Result<Option<TimeEntry>, StorageError> {
        let active = self.get_active_entry()?;
        let Some(active) = active else {
            return Ok(None);
        };

        let entry_id = active
            .id
            .ok_or_else(|| StorageError::NotFound("Active entry has no ID".to_string()))?;
        let now = Utc::now();
        let stop_time = if now >= active.start_time {
            now
        } else {
            active.start_time
        };

        self.conn_mut().execute(
            "UPDATE time_entries SET end_time = ?1, updated_at = ?2 WHERE id = ?3;",
            params![format_dt(&stop_time), format_dt(&now), entry_id],
        )?;

        self.get_entry(entry_id)
    }

    fn create_manual_entry(&mut self, entry: &TimeEntry) -> Result<TimeEntry, StorageError> {
        entry.validate()?;

        let tx = self.conn_mut().transaction()?;
        let end_time_str = entry.end_time.as_ref().map(format_dt);

        tx.execute(
            "INSERT INTO time_entries (
                description, project_id, start_time, end_time, entry_mode,
                pomodoro_index, synced, clockify_id, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10);",
            params![
                &entry.description,
                entry.project_id,
                format_dt(&entry.start_time),
                end_time_str,
                entry.entry_mode.as_str(),
                entry.pomodoro_index,
                entry.synced as i32,
                &entry.clockify_id,
                format_dt(&entry.created_at),
                format_dt(&entry.updated_at),
            ],
        )?;

        let entry_id = tx.last_insert_rowid();

        for tag in &entry.tags {
            link_tag(&tx, entry_id, tag)?;
        }

        tx.commit()?;

        self.get_entry(entry_id)?
            .ok_or_else(|| StorageError::NotFound(format!("Time entry {entry_id} not found")))
    }

    fn get_entry(&self, id: i64) -> Result<Option<TimeEntry>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id, description, project_id, start_time, end_time, entry_mode,
                    pomodoro_index, synced, clockify_id, created_at, updated_at
             FROM time_entries
             WHERE id = ?1;",
        )?;

        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            let mut entry = row_to_entry_base(row)?;
            entry.tags = get_tags_for_entry(self.conn(), id)?;
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    fn get_entries(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<TimeEntry>, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT id, description, project_id, start_time, end_time, entry_mode,
                    pomodoro_index, synced, clockify_id, created_at, updated_at
             FROM time_entries
             WHERE start_time >= ?1 AND start_time <= ?2
             ORDER BY start_time DESC;",
        )?;

        let mut rows = stmt.query(params![format_dt(&start_date), format_dt(&end_date)])?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            entries.push(row_to_entry_base(row)?);
        }
        drop(rows);
        drop(stmt);

        for entry in &mut entries {
            if let Some(id) = entry.id {
                entry.tags = get_tags_for_entry(self.conn(), id)?;
            }
        }

        Ok(entries)
    }

    fn update_entry(&mut self, entry: &TimeEntry) -> Result<(), StorageError> {
        let entry_id = entry
            .id
            .ok_or_else(|| StorageError::NotFound("Time entry ID is missing".to_string()))?;
        entry.validate()?;

        let now = Utc::now();
        let tx = self.conn_mut().transaction()?;
        let end_time_str = entry.end_time.as_ref().map(format_dt);

        let rows = tx.execute(
            "UPDATE time_entries
             SET description = ?1,
                 project_id = ?2,
                 start_time = ?3,
                 end_time = ?4,
                 entry_mode = ?5,
                 pomodoro_index = ?6,
                 synced = ?7,
                 clockify_id = ?8,
                 updated_at = ?9
             WHERE id = ?10;",
            params![
                &entry.description,
                entry.project_id,
                format_dt(&entry.start_time),
                end_time_str,
                entry.entry_mode.as_str(),
                entry.pomodoro_index,
                entry.synced as i32,
                &entry.clockify_id,
                format_dt(&now),
                entry_id,
            ],
        )?;

        if rows == 0 {
            return Err(StorageError::NotFound(format!(
                "Time entry {entry_id} not found"
            )));
        }

        tx.execute("DELETE FROM entry_tags WHERE entry_id = ?1;", [entry_id])?;

        for tag in &entry.tags {
            link_tag(&tx, entry_id, tag)?;
        }

        tx.commit()?;
        Ok(())
    }

    fn delete_entry(&mut self, id: i64) -> Result<(), StorageError> {
        self.conn_mut()
            .execute("DELETE FROM time_entries WHERE id = ?1;", [id])?;
        Ok(())
    }
}

impl Database {
    pub fn create_project(&mut self, project: &Project) -> Result<Project, StorageError> {
        Repository::create_project(self, project)
    }

    pub fn get_project(&self, id: i64) -> Result<Option<Project>, StorageError> {
        Repository::get_project(self, id)
    }

    pub fn get_project_by_name(&self, name: &str) -> Result<Option<Project>, StorageError> {
        Repository::get_project_by_name(self, name)
    }

    pub fn list_projects(&self, include_archived: bool) -> Result<Vec<Project>, StorageError> {
        Repository::list_projects(self, include_archived)
    }

    pub fn update_project(&mut self, project: &Project) -> Result<(), StorageError> {
        Repository::update_project(self, project)
    }

    pub fn delete_project(&mut self, id: i64) -> Result<(), StorageError> {
        Repository::delete_project(self, id)
    }

    pub fn create_tag(&mut self, tag: &Tag) -> Result<Tag, StorageError> {
        Repository::create_tag(self, tag)
    }

    pub fn get_tag(&self, id: i64) -> Result<Option<Tag>, StorageError> {
        Repository::get_tag(self, id)
    }

    pub fn get_tag_by_name(&self, name: &str) -> Result<Option<Tag>, StorageError> {
        Repository::get_tag_by_name(self, name)
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>, StorageError> {
        Repository::list_tags(self)
    }

    pub fn delete_tag(&mut self, id: i64) -> Result<(), StorageError> {
        Repository::delete_tag(self, id)
    }

    pub fn start_entry(&mut self, entry: &TimeEntry) -> Result<TimeEntry, StorageError> {
        Repository::start_entry(self, entry)
    }

    pub fn get_active_entry(&self) -> Result<Option<TimeEntry>, StorageError> {
        Repository::get_active_entry(self)
    }

    pub fn stop_active_entry(&mut self) -> Result<Option<TimeEntry>, StorageError> {
        Repository::stop_active_entry(self)
    }

    pub fn create_manual_entry(&mut self, entry: &TimeEntry) -> Result<TimeEntry, StorageError> {
        Repository::create_manual_entry(self, entry)
    }

    pub fn get_entry(&self, id: i64) -> Result<Option<TimeEntry>, StorageError> {
        Repository::get_entry(self, id)
    }

    pub fn get_entries(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<TimeEntry>, StorageError> {
        Repository::get_entries(self, start_date, end_date)
    }

    pub fn update_entry(&mut self, entry: &TimeEntry) -> Result<(), StorageError> {
        Repository::update_entry(self, entry)
    }

    pub fn delete_entry(&mut self, id: i64) -> Result<(), StorageError> {
        Repository::delete_entry(self, id)
    }
}
