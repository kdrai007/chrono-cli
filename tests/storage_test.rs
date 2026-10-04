use chrono::{Duration, TimeZone, Utc};
use clockify_tui::domain::{EntryMode, Project, Tag, TimeEntry, DEFAULT_PROJECT_COLOR};
use clockify_tui::storage::{Database, Repository, StorageError};
use tempfile::tempdir;

#[test]
fn test_schema_migration_creates_tables_and_indexes() {
    let db = Database::open_in_memory().expect("failed to open in-memory db");
    let conn = db.conn();

    // Verify tables exist
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap();
    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert!(tables.contains(&"projects".to_string()));
    assert!(tables.contains(&"tags".to_string()));
    assert!(tables.contains(&"time_entries".to_string()));
    assert!(tables.contains(&"entry_tags".to_string()));

    // Verify indexes exist
    let mut idx_stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' ORDER BY name")
        .unwrap();
    let indexes: Vec<String> = idx_stmt
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert!(indexes.contains(&"idx_time_entries_start_time".to_string()));
    assert!(indexes.contains(&"idx_time_entries_project_id".to_string()));
    assert!(indexes.contains(&"idx_time_entries_clockify_id".to_string()));
}

#[test]
fn test_foreign_keys_enforced() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let fk_enabled: i32 = db
        .conn()
        .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(fk_enabled, 1, "Foreign keys must be enabled");

    // Inserting a time entry referencing a non-existent project_id must fail
    let bad_entry = TimeEntry::new("Invalid entry", Utc::now()).with_project_id(999_999);
    let result = db.start_entry(&bad_entry);
    assert!(
        result.is_err(),
        "Inserting entry with non-existent foreign key project_id should fail"
    );
}

#[test]
fn test_project_crud() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    // 1. Create project with default color and target hours
    let p1 = Project::new("Operating Systems")
        .unwrap()
        .with_target_hours_week(10.0);
    let created1 = db.create_project(&p1).expect("failed to create project");
    assert!(created1.id.is_some());
    assert_eq!(created1.name, "Operating Systems");
    assert_eq!(created1.color, DEFAULT_PROJECT_COLOR);
    assert_eq!(created1.target_hours_week, 10.0);
    assert!(!created1.archived);

    let p1_id = created1.id.unwrap();

    // 2. Get project by ID and name
    let fetched = db.get_project(p1_id).unwrap().expect("project not found");
    assert_eq!(fetched.name, "Operating Systems");

    let fetched_by_name = db
        .get_project_by_name("Operating Systems")
        .unwrap()
        .expect("project not found by name");
    assert_eq!(fetched_by_name.id, Some(p1_id));

    // 3. Duplicate project name error handling
    let duplicate = Project::new("Operating Systems").unwrap();
    let dup_res = db.create_project(&duplicate);
    assert!(dup_res.is_err(), "Duplicate project name should fail");
    match dup_res.unwrap_err() {
        StorageError::Duplicate(_) => {}
        err => panic!("Expected StorageError::Duplicate, got {:?}", err),
    }

    // 4. Update project target hours, color, name
    let mut to_update = fetched;
    to_update.name = "Advanced Operating Systems".to_string();
    to_update.color = "#e74c3c".to_string();
    to_update.target_hours_week = 15.0;
    to_update.archived = true;
    db.update_project(&to_update)
        .expect("failed to update project");

    let updated = db.get_project(p1_id).unwrap().unwrap();
    assert_eq!(updated.name, "Advanced Operating Systems");
    assert_eq!(updated.color, "#e74c3c");
    assert_eq!(updated.target_hours_week, 15.0);
    assert!(updated.archived);

    // 5. List active projects (excluding archived if requested)
    let p2 = Project::new("Compilers").unwrap();
    db.create_project(&p2).unwrap();

    let active_projects = db.list_projects(false).unwrap();
    assert_eq!(active_projects.len(), 1);
    assert_eq!(active_projects[0].name, "Compilers");

    let all_projects = db.list_projects(true).unwrap();
    assert_eq!(all_projects.len(), 2);

    // 6. Delete project
    db.delete_project(p1_id).expect("failed to delete project");
    assert!(db.get_project(p1_id).unwrap().is_none());
}

#[test]
fn test_tag_crud() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    // 1. Create tag
    let tag = Tag::new("homework").unwrap();
    let created = db.create_tag(&tag).expect("failed to create tag");
    assert!(created.id.is_some());
    assert_eq!(created.name, "homework");
    let tag_id = created.id.unwrap();

    // 2. Prevent duplicate tag names
    let dup = Tag::new("homework").unwrap();
    let dup_err = db.create_tag(&dup);
    assert!(dup_err.is_err());
    assert!(matches!(dup_err.unwrap_err(), StorageError::Duplicate(_)));

    // 3. Get tag by ID and by name
    let fetched = db.get_tag(tag_id).unwrap().expect("tag should exist");
    assert_eq!(fetched.name, "homework");

    let fetched_name = db
        .get_tag_by_name("homework")
        .unwrap()
        .expect("tag should exist");
    assert_eq!(fetched_name.id, Some(tag_id));

    // 4. List all tags
    let tag2 = Tag::new("exam-prep").unwrap();
    db.create_tag(&tag2).unwrap();

    let tags = db.list_tags().unwrap();
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().any(|t| t.name == "homework"));
    assert!(tags.iter().any(|t| t.name == "exam-prep"));

    // 5. Delete tag
    db.delete_tag(tag_id).unwrap();
    assert!(db.get_tag(tag_id).unwrap().is_none());
    assert_eq!(db.list_tags().unwrap().len(), 1);
}

#[test]
fn test_time_entry_start_single_active_constraint_and_load() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let project = db
        .create_project(&Project::new("Algorithms").unwrap())
        .unwrap();
    let project_id = project.id.unwrap();

    let tag1 = db.create_tag(&Tag::new("lecture").unwrap()).unwrap();
    let tag2 = db.create_tag(&Tag::new("notes").unwrap()).unwrap();

    // Start first entry with project and tags
    let start1 = Utc.with_ymd_and_hms(2026, 10, 4, 10, 0, 0).unwrap();
    let entry1 = TimeEntry::new("Lecture 1", start1)
        .with_project_id(project_id)
        .with_tags(vec![tag1.clone(), tag2.clone()])
        .with_entry_mode(EntryMode::PomodoroWork);

    let started1 = db.start_entry(&entry1).unwrap();
    assert!(started1.id.is_some());
    assert!(started1.is_active());
    assert_eq!(started1.project_id, Some(project_id));
    assert_eq!(started1.tags.len(), 2);

    // Verify get_active_entry loads the entry with project and tags
    let active = db
        .get_active_entry()
        .unwrap()
        .expect("active entry should exist");
    assert_eq!(active.id, started1.id);
    assert_eq!(active.description, "Lecture 1");
    assert_eq!(active.project_id, Some(project_id));
    assert_eq!(active.tags.len(), 2);
    assert!(active.tags.iter().any(|t| t.name == "lecture"));
    assert!(active.tags.iter().any(|t| t.name == "notes"));

    // Starting a second entry when another is already active stops the previous active entry first
    let start2 = Utc.with_ymd_and_hms(2026, 10, 4, 11, 0, 0).unwrap();
    let entry2 = TimeEntry::new("Lecture 2", start2)
        .with_project_id(project_id)
        .with_tags(vec![tag1]);

    let started2 = db.start_entry(&entry2).unwrap();
    assert!(started2.is_active());
    assert_ne!(started2.id, started1.id);

    // Verify first entry was stopped
    let stopped1 = db.get_entry(started1.id.unwrap()).unwrap().unwrap();
    assert!(!stopped1.is_active());
    assert!(stopped1.end_time.is_some());
    assert!(stopped1.end_time.unwrap() >= start1);

    // Verify only the second entry is active now
    let current_active = db.get_active_entry().unwrap().unwrap();
    assert_eq!(current_active.id, started2.id);
    assert_eq!(current_active.description, "Lecture 2");
}

#[test]
fn test_time_entry_stop_active_entry() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let entry = TimeEntry::new("Reading chapter 1", Utc::now());
    let started = db.start_entry(&entry).unwrap();
    assert!(started.is_active());

    // Stop active entry
    let stopped = db
        .stop_active_entry()
        .unwrap()
        .expect("should return stopped entry");
    assert!(!stopped.is_active());
    assert!(stopped.end_time.is_some());

    // Calling get_active_entry returns None
    assert!(db.get_active_entry().unwrap().is_none());

    // Stopping when no entry is active returns Ok(None)
    let none_stopped = db.stop_active_entry().unwrap();
    assert!(none_stopped.is_none());
}

#[test]
fn test_time_entry_create_manual_entry() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let start = Utc.with_ymd_and_hms(2026, 10, 4, 8, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 10, 4, 9, 30, 0).unwrap();

    let manual = TimeEntry::new("Past study session", start)
        .with_end_time(end)
        .with_entry_mode(EntryMode::Stopwatch);

    let created = db.create_manual_entry(&manual).unwrap();
    assert!(created.id.is_some());
    assert!(!created.is_active());
    assert_eq!(created.start_time, start);
    assert_eq!(created.end_time, Some(end));

    // Manual entry must not affect active entry
    assert!(db.get_active_entry().unwrap().is_none());

    let fetched = db.get_entry(created.id.unwrap()).unwrap().unwrap();
    assert_eq!(fetched.description, "Past study session");
    assert_eq!(fetched.end_time, Some(end));
}

#[test]
fn test_time_entry_get_entries_range_and_ordering() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let t1 = Utc.with_ymd_and_hms(2026, 10, 1, 10, 0, 0).unwrap();
    let t2 = Utc.with_ymd_and_hms(2026, 10, 2, 10, 0, 0).unwrap();
    let t3 = Utc.with_ymd_and_hms(2026, 10, 3, 10, 0, 0).unwrap();
    let t4 = Utc.with_ymd_and_hms(2026, 10, 4, 10, 0, 0).unwrap();

    let e1 = TimeEntry::new("Day 1", t1).with_end_time(t1 + Duration::hours(1));
    let e2 = TimeEntry::new("Day 2", t2).with_end_time(t2 + Duration::hours(1));
    let e3 = TimeEntry::new("Day 3", t3).with_end_time(t3 + Duration::hours(1));
    let e4 = TimeEntry::new("Day 4", t4).with_end_time(t4 + Duration::hours(1));

    db.create_manual_entry(&e1).unwrap();
    db.create_manual_entry(&e2).unwrap();
    db.create_manual_entry(&e3).unwrap();
    db.create_manual_entry(&e4).unwrap();

    // Query for Oct 2 to Oct 3
    let range_start = Utc.with_ymd_and_hms(2026, 10, 2, 0, 0, 0).unwrap();
    let range_end = Utc.with_ymd_and_hms(2026, 10, 3, 23, 59, 59).unwrap();

    let entries = db.get_entries(range_start, range_end).unwrap();
    assert_eq!(entries.len(), 2);
    // Newest first
    assert_eq!(entries[0].description, "Day 3");
    assert_eq!(entries[1].description, "Day 2");
}

#[test]
fn test_time_entry_update() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let p1 = db
        .create_project(&Project::new("Project 1").unwrap())
        .unwrap();
    let p2 = db
        .create_project(&Project::new("Project 2").unwrap())
        .unwrap();

    let tag1 = db.create_tag(&Tag::new("tag1").unwrap()).unwrap();
    let tag2 = db.create_tag(&Tag::new("tag2").unwrap()).unwrap();

    let start = Utc.with_ymd_and_hms(2026, 10, 4, 9, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 10, 4, 10, 0, 0).unwrap();

    let entry = TimeEntry::new("Initial description", start)
        .with_end_time(end)
        .with_project_id(p1.id.unwrap())
        .with_tags(vec![tag1]);

    let created = db.create_manual_entry(&entry).unwrap();
    let entry_id = created.id.unwrap();

    // Update entry fields and tags
    let new_start = Utc.with_ymd_and_hms(2026, 10, 4, 9, 15, 0).unwrap();
    let new_end = Utc.with_ymd_and_hms(2026, 10, 4, 10, 45, 0).unwrap();
    let mut updated_entry = created;
    updated_entry.description = "Updated description".to_string();
    updated_entry.project_id = Some(p2.id.unwrap());
    updated_entry.start_time = new_start;
    updated_entry.end_time = Some(new_end);
    updated_entry.tags = vec![tag2];

    db.update_entry(&updated_entry).unwrap();

    let fetched = db.get_entry(entry_id).unwrap().unwrap();
    assert_eq!(fetched.description, "Updated description");
    assert_eq!(fetched.project_id, Some(p2.id.unwrap()));
    assert_eq!(fetched.start_time, new_start);
    assert_eq!(fetched.end_time, Some(new_end));
    assert_eq!(fetched.tags.len(), 1);
    assert_eq!(fetched.tags[0].name, "tag2");
}

#[test]
fn test_time_entry_delete_cascades_entry_tags() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let tag1 = db.create_tag(&Tag::new("tag1").unwrap()).unwrap();
    let tag2 = db.create_tag(&Tag::new("tag2").unwrap()).unwrap();

    let entry = TimeEntry::new("To be deleted", Utc::now()).with_tags(vec![tag1, tag2]);
    let created = db.start_entry(&entry).unwrap();
    let entry_id = created.id.unwrap();

    // Verify entry_tags has 2 rows
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM entry_tags WHERE entry_id = ?1;",
            [entry_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);

    // Delete entry
    db.delete_entry(entry_id).unwrap();

    // Entry should no longer exist
    assert!(db.get_entry(entry_id).unwrap().is_none());

    // entry_tags rows must be removed by cascade
    let count_after: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM entry_tags WHERE entry_id = ?1;",
            [entry_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count_after, 0);

    // Tags themselves must still exist
    assert_eq!(db.list_tags().unwrap().len(), 2);
}

#[test]
fn test_project_deletion_sets_null_on_linked_entries() {
    let mut db = Database::open_in_memory().expect("failed to open in-memory db");

    let project = db
        .create_project(&Project::new("ToDelete").unwrap())
        .unwrap();
    let project_id = project.id.unwrap();

    let entry = TimeEntry::new("Linked entry", Utc::now()).with_project_id(project_id);
    let created = db.start_entry(&entry).unwrap();
    let entry_id = created.id.unwrap();

    // Delete project
    db.delete_project(project_id).unwrap();

    // Project is gone
    assert!(db.get_project(project_id).unwrap().is_none());

    // Time entry still exists, but project_id is now None (ON DELETE SET NULL)
    let fetched_entry = db.get_entry(entry_id).unwrap().unwrap();
    assert_eq!(fetched_entry.project_id, None);
}

#[test]
fn test_database_open_file_wal_and_default_path() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_clockify.db");

    let db = Database::open_file(&db_path).expect("failed to open file db");
    assert!(db_path.exists());

    // File DB must use WAL mode
    let journal_mode: String = db
        .conn()
        .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal_mode.to_lowercase(), "wal");

    // Check default path format
    let default_path = Database::default_db_path();
    assert!(
        default_path.ends_with("clockify-tui/clockify.db"),
        "Default DB path should end with clockify-tui/clockify.db, got {:?}",
        default_path
    );
}

fn create_via_repo<R: Repository + ?Sized>(repo: &mut R, name: &str) -> Project {
    let p = Project::new(name).unwrap();
    repo.create_project(&p).unwrap()
}

#[test]
fn test_repository_trait_polymorphism() {
    let mut db = Database::open_in_memory().unwrap();
    let created = create_via_repo(&mut db, "Trait Project");
    assert_eq!(created.name, "Trait Project");

    let fetched = Repository::get_project(&db, created.id.unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(fetched.name, "Trait Project");
}
