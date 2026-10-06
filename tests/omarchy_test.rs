use assert_cmd::Command;
use tempfile::tempdir;

use chrono_cli::cli::OmarchyPayload;
use chrono_cli::domain::Project;
use chrono_cli::storage::Database;

#[test]
fn test_omarchy_status_snapshot_schema() {
    let temp_dir = tempdir().expect("create temp dir");
    let db_path = temp_dir.path().join("omarchy_test.db");
    let db_path_str = db_path.to_str().unwrap();

    let mut cmd = Command::cargo_bin("chrono").expect("binary chrono exists");
    cmd.env("CLOCKIFY_DB_PATH", db_path_str);
    cmd.args(["omarchy", "status"]);

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    let payload: OmarchyPayload = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("Failed to parse omarchy status as OmarchyPayload: '{stdout}': {e}")
    });

    assert!(payload.ok);
    assert!(payload.configured);
    assert_eq!(payload.error, "");
    assert!(payload.running.is_none());
    assert_eq!(payload.today_seconds, 0);
    assert_eq!(payload.week_seconds, 0);
    assert!(!payload.fetched_at.is_empty());
    assert!(!payload.week_start.is_empty());

    // Raw JSON key validation for the 16 exact keys expected by Model.js and Panel.qml
    let raw_json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let expected_keys = [
        "ok",
        "configured",
        "error",
        "note",
        "userName",
        "workspaceId",
        "workspaceName",
        "defaultProjectId",
        "weekStart",
        "running",
        "todaySeconds",
        "weekSeconds",
        "todayBreakdown",
        "recentTasks",
        "projects",
        "projectsLoaded",
        "fetchedAt",
    ];

    for key in expected_keys {
        assert!(
            raw_json.get(key).is_some(),
            "Omarchy JSON payload must contain key '{key}'"
        );
    }
}

#[test]
fn test_omarchy_start_stop_discard_lifecycle() {
    let temp_dir = tempdir().expect("create temp dir");
    let db_path = temp_dir.path().join("omarchy_lifecycle.db");
    let db_path_str = db_path.to_str().unwrap();

    let run_cmd = |args: &[&str]| {
        let mut cmd = Command::cargo_bin("chrono").expect("binary chrono exists");
        cmd.env("CLOCKIFY_DB_PATH", db_path_str);
        cmd.args(args);
        let output = cmd.assert().success();
        let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
        let payload: OmarchyPayload = serde_json::from_str(&stdout).unwrap();
        payload
    };

    // 1. Start a timer
    let payload = run_cmd(&[
        "omarchy",
        "start",
        "--description",
        "Quantum Physics Research",
        "--project",
        "Physics",
    ]);

    assert!(payload.ok);
    assert_eq!(payload.note, "Timer started");
    assert!(payload.running.is_some());
    let running = payload.running.unwrap();
    assert_eq!(running.description, "Quantum Physics Research");
    assert_eq!(running.project_name, "Physics");
    assert!(running.running);

    // 2. Status while running
    let status_payload = run_cmd(&["omarchy", "status"]);
    assert!(status_payload.running.is_some());
    assert_eq!(
        status_payload.running.unwrap().description,
        "Quantum Physics Research"
    );

    // 3. Stop timer
    let stop_payload = run_cmd(&["omarchy", "stop"]);
    assert!(stop_payload.ok);
    assert_eq!(stop_payload.note, "Session stopped");
    assert!(stop_payload.running.is_none());
    assert!(!stop_payload.recent_tasks.is_empty());
    assert_eq!(
        stop_payload.recent_tasks[0].description,
        "Quantum Physics Research"
    );

    // 4. Continue last task
    let cont_payload = run_cmd(&["omarchy", "continue"]);
    assert!(cont_payload.ok);
    assert!(cont_payload.running.is_some());
    assert_eq!(
        cont_payload.running.unwrap().description,
        "Quantum Physics Research"
    );

    // 5. Discard running timer
    let discard_payload = run_cmd(&["omarchy", "discard"]);
    assert!(discard_payload.ok);
    assert_eq!(discard_payload.note, "Session discarded");
    assert!(discard_payload.running.is_none());

    // 6. Stop when nothing is running gives error note
    let empty_stop = run_cmd(&["omarchy", "stop"]);
    assert_eq!(empty_stop.error, "No session is running");
}

#[test]
fn test_omarchy_status_with_projects() {
    let temp_dir = tempdir().expect("create temp dir");
    let db_path = temp_dir.path().join("omarchy_projects.db");

    // Pre-populate with projects in SQLite
    {
        let mut db = Database::open_file(&db_path).unwrap();
        db.create_project(&Project::new("Mathematics").unwrap().with_color("#2ecc71"))
            .unwrap();
        db.create_project(&Project::new("History").unwrap().with_color("#e74c3c"))
            .unwrap();
    }

    let mut cmd = Command::cargo_bin("chrono").expect("binary chrono exists");
    cmd.env("CLOCKIFY_DB_PATH", db_path.to_str().unwrap());
    cmd.args(["omarchy", "status", "--projects"]);

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let payload: OmarchyPayload = serde_json::from_str(&stdout).unwrap();

    assert!(payload.projects_loaded);
    assert_eq!(payload.projects.len(), 2);
    let names: Vec<String> = payload.projects.iter().map(|p| p.name.clone()).collect();
    assert!(names.contains(&"Mathematics".to_string()));
    assert!(names.contains(&"History".to_string()));
}

#[test]
fn test_status_omarchy_flag() {
    let temp_dir = tempdir().expect("create temp dir");
    let db_path = temp_dir.path().join("status_flag.db");

    let mut cmd = Command::cargo_bin("chrono").expect("binary chrono exists");
    cmd.env("CLOCKIFY_DB_PATH", db_path.to_str().unwrap());
    cmd.args(["status", "--omarchy"]);

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let payload: OmarchyPayload = serde_json::from_str(&stdout).unwrap();
    assert!(payload.ok);
    assert!(payload.configured);
}

#[test]
fn test_omarchy_set_config() {
    let mut cmd = Command::cargo_bin("chrono").expect("binary chrono exists");
    cmd.args([
        "omarchy",
        "set-config",
        "--project",
        "proj_algorithms",
        "--workspace",
        "ws_study",
        "--week-start",
        "sunday",
    ]);

    let output = cmd.assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let raw: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(raw.get("ok").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        raw.get("defaultProjectId").and_then(|v| v.as_str()),
        Some("proj_algorithms")
    );
    assert_eq!(
        raw.get("workspaceId").and_then(|v| v.as_str()),
        Some("ws_study")
    );
    assert_eq!(
        raw.get("weekStart").and_then(|v| v.as_str()),
        Some("sunday")
    );
}
