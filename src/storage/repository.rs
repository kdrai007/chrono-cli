use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::{
    calculate_streaks, DailyStudySummary, EntryMode, PomodoroStats, Project, ProjectTargetProgress,
    StreakStats, SubjectBreakdown, Tag, TimeEntry,
};

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

    // Stats & Analytics operations
    fn get_streak_stats(&self, today: NaiveDate) -> Result<StreakStats, StorageError>;
    fn get_daily_summaries(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<DailyStudySummary>, StorageError>;
    fn get_weekly_project_progress(
        &self,
        week_start: NaiveDate,
        week_end: NaiveDate,
    ) -> Result<Vec<ProjectTargetProgress>, StorageError>;
    fn get_subject_breakdown(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<SubjectBreakdown>, StorageError>;
    fn get_pomodoro_stats(
        &self,
        today: NaiveDate,
        week_start: NaiveDate,
    ) -> Result<PomodoroStats, StorageError>;
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

    fn get_streak_stats(&self, today: NaiveDate) -> Result<StreakStats, StorageError> {
        let mut stmt = self.conn().prepare(
            "SELECT DISTINCT SUBSTR(start_time, 1, 10)
             FROM time_entries
             WHERE entry_mode != 'pomodoro_break'
             ORDER BY 1 ASC;",
        )?;

        let rows = stmt.query_map([], |row| {
            let date_str: String = row.get(0)?;
            Ok(date_str)
        })?;

        let mut dates = Vec::new();
        for r in rows {
            let s = r?;
            if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
                dates.push(d);
            }
        }

        Ok(calculate_streaks(&dates, today))
    }

    fn get_daily_summaries(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<DailyStudySummary>, StorageError> {
        let start_dt = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_dt = (end_date + Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let mut stmt = self.conn().prepare(
            "SELECT start_time, end_time
             FROM time_entries
             WHERE start_time >= ?1 AND start_time < ?2 AND entry_mode != 'pomodoro_break'
             ORDER BY start_time ASC;",
        )?;

        let rows = stmt.query_map(params![format_dt(&start_dt), format_dt(&end_dt)], |row| {
            let start_str: String = row.get(0)?;
            let end_str: Option<String> = row.get(1)?;
            Ok((start_str, end_str))
        })?;

        let now = Utc::now();
        let mut daily_map: BTreeMap<NaiveDate, Duration> = BTreeMap::new();

        for r in rows {
            let (start_str, end_str) = r?;
            let start_time = parse_dt(&start_str)?;
            let end_time = match end_str {
                Some(ref s) => Some(parse_dt(s)?),
                None => None,
            };

            let duration = match end_time {
                Some(end) => (end - start_time).max(Duration::zero()),
                None => (now - start_time).max(Duration::zero()),
            };

            let date = start_time.date_naive();
            *daily_map.entry(date).or_insert_with(Duration::zero) += duration;
        }

        let summaries = daily_map
            .into_iter()
            .map(|(date, duration)| DailyStudySummary { date, duration })
            .collect();

        Ok(summaries)
    }

    fn get_weekly_project_progress(
        &self,
        week_start: NaiveDate,
        week_end: NaiveDate,
    ) -> Result<Vec<ProjectTargetProgress>, StorageError> {
        let start_dt = week_start.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_dt = (week_end + Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let projects = self.list_projects(false)?;

        let mut stmt = self.conn().prepare(
            "SELECT project_id, start_time, end_time
             FROM time_entries
             WHERE project_id IS NOT NULL
               AND start_time >= ?1
               AND start_time < ?2
               AND entry_mode != 'pomodoro_break';",
        )?;

        let rows = stmt.query_map(params![format_dt(&start_dt), format_dt(&end_dt)], |row| {
            let pid: i64 = row.get(0)?;
            let start_str: String = row.get(1)?;
            let end_str: Option<String> = row.get(2)?;
            Ok((pid, start_str, end_str))
        })?;

        let now = Utc::now();
        let mut actual_hours_map: HashMap<i64, f64> = HashMap::new();

        for r in rows {
            let (pid, start_str, end_str) = r?;
            let start_time = parse_dt(&start_str)?;
            let end_time = match end_str {
                Some(ref s) => Some(parse_dt(s)?),
                None => None,
            };

            let duration = match end_time {
                Some(end) => (end - start_time).max(Duration::zero()),
                None => (now - start_time).max(Duration::zero()),
            };

            let hours = duration.num_milliseconds() as f64 / 3_600_000.0;
            *actual_hours_map.entry(pid).or_insert(0.0) += hours;
        }

        let mut progress_list = Vec::with_capacity(projects.len());
        for proj in projects {
            if let Some(pid) = proj.id {
                let actual_hours = actual_hours_map.get(&pid).copied().unwrap_or(0.0);
                progress_list.push(ProjectTargetProgress::new(
                    pid,
                    proj.name,
                    proj.color,
                    proj.target_hours_week,
                    actual_hours,
                ));
            }
        }

        progress_list.sort_by(|a, b| a.project_name.cmp(&b.project_name));

        Ok(progress_list)
    }

    fn get_subject_breakdown(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<SubjectBreakdown>, StorageError> {
        let start_dt = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_dt = (end_date + Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let mut stmt = self.conn().prepare(
            "SELECT COALESCE(p.name, 'No Project'),
                    COALESCE(p.color, '#3498db'),
                    e.start_time,
                    e.end_time
             FROM time_entries e
             LEFT JOIN projects p ON e.project_id = p.id
             WHERE e.start_time >= ?1
               AND e.start_time < ?2
               AND e.entry_mode != 'pomodoro_break'
             ORDER BY e.start_time ASC;",
        )?;

        let rows = stmt.query_map(params![format_dt(&start_dt), format_dt(&end_dt)], |row| {
            let name: String = row.get(0)?;
            let color: String = row.get(1)?;
            let start_str: String = row.get(2)?;
            let end_str: Option<String> = row.get(3)?;
            Ok((name, color, start_str, end_str))
        })?;

        let now = Utc::now();
        let mut subject_map: HashMap<String, (String, Duration)> = HashMap::new();
        let mut total_duration = Duration::zero();

        for r in rows {
            let (name, color, start_str, end_str) = r?;
            let start_time = parse_dt(&start_str)?;
            let end_time = match end_str {
                Some(ref s) => Some(parse_dt(s)?),
                None => None,
            };

            let duration = match end_time {
                Some(end) => (end - start_time).max(Duration::zero()),
                None => (now - start_time).max(Duration::zero()),
            };

            total_duration += duration;
            let entry = subject_map
                .entry(name)
                .or_insert_with(|| (color, Duration::zero()));
            entry.1 += duration;
        }

        let total_secs = total_duration.num_seconds().max(0) as f64;
        let mut breakdowns = Vec::with_capacity(subject_map.len());

        for (project_name, (color, duration)) in subject_map {
            let percentage = if total_secs > 0.0 {
                (duration.num_seconds().max(0) as f64 / total_secs) * 100.0
            } else {
                0.0
            };

            breakdowns.push(SubjectBreakdown::new(
                project_name,
                color,
                duration,
                percentage,
            ));
        }

        breakdowns.sort_by(|a, b| {
            b.duration
                .cmp(&a.duration)
                .then_with(|| a.project_name.cmp(&b.project_name))
        });

        Ok(breakdowns)
    }

    fn get_pomodoro_stats(
        &self,
        today: NaiveDate,
        week_start: NaiveDate,
    ) -> Result<PomodoroStats, StorageError> {
        let today_start_dt = today.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let today_end_dt = (today + Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let week_start_dt = week_start.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let week_limit_dt = (week_start + Duration::days(7))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let query_start_dt = week_start
            .min(today)
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let query_end_dt = (week_start + Duration::days(7))
            .max(today + Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let mut stmt = self.conn().prepare(
            "SELECT start_time, end_time
             FROM time_entries
             WHERE entry_mode = 'pomodoro_work'
               AND end_time IS NOT NULL
               AND start_time >= ?1
               AND start_time < ?2
             ORDER BY start_time ASC;",
        )?;

        let rows = stmt.query_map(
            params![format_dt(&query_start_dt), format_dt(&query_end_dt)],
            |row| {
                let start_str: String = row.get(0)?;
                let end_str: String = row.get(1)?;
                Ok((start_str, end_str))
            },
        )?;

        let mut completed_today = 0u32;
        let mut completed_this_week = 0u32;
        let mut total_duration = Duration::zero();

        for r in rows {
            let (start_str, end_str) = r?;
            let start_time = parse_dt(&start_str)?;
            let end_time = parse_dt(&end_str)?;

            let duration = (end_time - start_time).max(Duration::zero());

            if start_time >= week_start_dt && start_time < week_limit_dt {
                completed_this_week += 1;
                total_duration += duration;
            }

            if start_time >= today_start_dt && start_time < today_end_dt {
                completed_today += 1;
            }
        }

        Ok(PomodoroStats {
            completed_today,
            completed_this_week,
            total_focus_mins: total_duration.num_minutes(),
        })
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

    pub fn get_streak_stats(&self, today: NaiveDate) -> Result<StreakStats, StorageError> {
        Repository::get_streak_stats(self, today)
    }

    pub fn get_daily_summaries(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<DailyStudySummary>, StorageError> {
        Repository::get_daily_summaries(self, start_date, end_date)
    }

    pub fn get_weekly_project_progress(
        &self,
        week_start: NaiveDate,
        week_end: NaiveDate,
    ) -> Result<Vec<ProjectTargetProgress>, StorageError> {
        Repository::get_weekly_project_progress(self, week_start, week_end)
    }

    pub fn get_subject_breakdown(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<SubjectBreakdown>, StorageError> {
        Repository::get_subject_breakdown(self, start_date, end_date)
    }

    pub fn get_pomodoro_stats(
        &self,
        today: NaiveDate,
        week_start: NaiveDate,
    ) -> Result<PomodoroStats, StorageError> {
        Repository::get_pomodoro_stats(self, today, week_start)
    }
}
