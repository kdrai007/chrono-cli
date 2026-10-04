use rusqlite::Connection;

use super::StorageError;

/// Current schema version applied by migrations.
pub const CURRENT_SCHEMA_VERSION: i32 = 1;

/// SQL statement creating the `projects` table.
pub const CREATE_PROJECTS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL DEFAULT '#3498db',
    target_hours_week REAL NOT NULL DEFAULT 0.0,
    clockify_id TEXT,
    archived INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
"#;

/// SQL statement creating the `tags` table.
pub const CREATE_TAGS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    clockify_id TEXT
);
"#;

/// SQL statement creating the `time_entries` table.
pub const CREATE_TIME_ENTRIES_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS time_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    description TEXT NOT NULL DEFAULT '',
    project_id INTEGER REFERENCES projects(id) ON DELETE SET NULL,
    start_time TEXT NOT NULL,
    end_time TEXT,
    entry_mode TEXT NOT NULL DEFAULT 'stopwatch',
    pomodoro_index INTEGER NOT NULL DEFAULT 0,
    synced INTEGER NOT NULL DEFAULT 0,
    clockify_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

/// SQL statement creating the `entry_tags` join table.
pub const CREATE_ENTRY_TAGS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS entry_tags (
    entry_id INTEGER NOT NULL REFERENCES time_entries(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (entry_id, tag_id)
);
"#;

/// Index definitions for efficient lookups.
pub const CREATE_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_time_entries_start_time ON time_entries(start_time);",
    "CREATE INDEX IF NOT EXISTS idx_time_entries_project_id ON time_entries(project_id);",
    "CREATE INDEX IF NOT EXISTS idx_time_entries_clockify_id ON time_entries(clockify_id);",
    "CREATE INDEX IF NOT EXISTS idx_projects_clockify_id ON projects(clockify_id);",
    "CREATE INDEX IF NOT EXISTS idx_tags_clockify_id ON tags(clockify_id);",
    "CREATE INDEX IF NOT EXISTS idx_entry_tags_tag_id ON entry_tags(tag_id);",
];

/// Applies schema migrations up to `CURRENT_SCHEMA_VERSION`.
pub fn migrate(conn: &Connection) -> Result<(), StorageError> {
    let current_version: i32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;

    if current_version < 1 {
        conn.execute(CREATE_PROJECTS_TABLE, [])?;
        conn.execute(CREATE_TAGS_TABLE, [])?;
        conn.execute(CREATE_TIME_ENTRIES_TABLE, [])?;
        conn.execute(CREATE_ENTRY_TAGS_TABLE, [])?;

        for index_sql in CREATE_INDEXES {
            conn.execute(index_sql, [])?;
        }

        conn.pragma_update(None, "user_version", 1)?;
    }

    Ok(())
}
