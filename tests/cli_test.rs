use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("clockify-tui").expect("binary clockify-tui exists");
    cmd.arg("--help");

    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    // Verify all subcommands are displayed in help
    assert!(stdout.contains("start"), "Help should display 'start'");
    assert!(stdout.contains("stop"), "Help should display 'stop'");
    assert!(stdout.contains("status"), "Help should display 'status'");
    assert!(stdout.contains("export"), "Help should display 'export'");
    assert!(stdout.contains("sync"), "Help should display 'sync'");
}

#[test]
fn test_cli_full_workflow() {
    let temp_dir = tempdir().expect("create temp dir");
    let db_path = temp_dir.path().join("clockify_test.db");
    let db_path_str = db_path.to_str().unwrap();

    let new_cmd = || {
        let mut cmd = Command::cargo_bin("clockify-tui").expect("binary clockify-tui exists");
        cmd.env("CLOCKIFY_DB_PATH", db_path_str);
        cmd
    };

    // 1. Status when idle (plain text)
    let output = new_cmd().arg("status").assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("[IDLE]") || stdout.contains("No active timer"),
        "Status text output should indicate idle state, got: {stdout}"
    );

    // 2. Status --json when idle
    let output = new_cmd().args(["status", "--json"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("Failed to parse status --json output as JSON: '{stdout}': {e}")
    });
    assert!(json.get("text").is_some(), "JSON should contain 'text'");
    assert!(
        json.get("tooltip").is_some(),
        "JSON should contain 'tooltip'"
    );
    assert_eq!(
        json.get("class").and_then(|v| v.as_str()),
        Some("idle"),
        "JSON class should be 'idle'"
    );

    // 3. Start timer
    let output = new_cmd()
        .args([
            "start",
            "Review Physics",
            "-p",
            "Physics",
            "-t",
            "Exam,Revision",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.to_lowercase().contains("started"),
        "Start command should confirm timer started, got: {stdout}"
    );

    // 4. Status when running (plain text)
    let output = new_cmd().arg("status").assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("[RUNNING]"),
        "Status text should contain [RUNNING], got: {stdout}"
    );
    assert!(
        stdout.contains("Review Physics"),
        "Status text should contain description 'Review Physics', got: {stdout}"
    );
    assert!(
        stdout.contains("Physics"),
        "Status text should contain project 'Physics', got: {stdout}"
    );

    // 5. Status --json when running
    let output = new_cmd().args(["status", "--json"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("Failed to parse status --json output: '{stdout}': {e}"));
    assert!(json.get("text").is_some(), "JSON should contain 'text'");
    assert_eq!(
        json.get("class").and_then(|v| v.as_str()),
        Some("running"),
        "JSON class should be 'running'"
    );
    let tooltip = json.get("tooltip").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        tooltip.contains("Review Physics"),
        "Tooltip should contain description 'Review Physics', got: {tooltip}"
    );

    // 6. Stop timer
    let output = new_cmd().arg("stop").assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.to_lowercase().contains("stopped")
            || stdout.to_lowercase().contains("elapsed")
            || stdout.to_lowercase().contains("duration"),
        "Stop command should confirm timer stopped and report duration, got: {stdout}"
    );

    // 7. Status returns to idle
    let output = new_cmd().arg("status").assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("[IDLE]") || stdout.contains("No active timer"),
        "Status should return to idle after stop, got: {stdout}"
    );

    // 8. Export JSON
    let output = new_cmd()
        .args(["export", "--format", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("Failed to parse export --format json as JSON: '{stdout}': {e}")
    });
    assert!(json.is_array(), "Export JSON output should be an array");
    let arr = json.as_array().unwrap();
    assert!(
        !arr.is_empty(),
        "Export JSON array should contain at least one entry"
    );
    let entry = &arr[0];
    assert_eq!(
        entry.get("description").and_then(|v| v.as_str()),
        Some("Review Physics"),
        "Entry description should be 'Review Physics'"
    );
    assert_eq!(
        entry.get("project").and_then(|v| v.as_str()),
        Some("Physics"),
        "Entry project should be 'Physics'"
    );

    // 9. Export CSV
    let output = new_cmd()
        .args(["export", "--format", "csv"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let mut reader = csv::Reader::from_reader(stdout.as_bytes());
    let headers = reader.headers().expect("valid CSV headers").clone();
    assert_eq!(
        headers.iter().collect::<Vec<_>>(),
        vec![
            "id",
            "description",
            "project",
            "start_time",
            "end_time",
            "duration_seconds",
            "tags"
        ],
        "CSV headers must match id,description,project,start_time,end_time,duration_seconds,tags"
    );

    let records: Vec<_> = reader.records().map(|r| r.unwrap()).collect();
    assert_eq!(records.len(), 1, "Should export exactly 1 CSV record");
    assert_eq!(&records[0][1], "Review Physics");
    assert_eq!(&records[0][2], "Physics");

    // 10. Export to file
    let export_file = temp_dir.path().join("exported.csv");
    new_cmd()
        .args([
            "export",
            "--format",
            "csv",
            "-o",
            export_file.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(export_file.exists(), "Export file should exist");
    let content = std::fs::read_to_string(export_file).unwrap();
    assert!(content.contains("Review Physics"));

    // 11. Sync command
    let sync_output = new_cmd().arg("sync").assert().success();
    let sync_stdout = String::from_utf8(sync_output.get_output().stdout.clone()).unwrap();
    assert!(sync_stdout.contains("Clockify") || sync_stdout.contains("sync"));
}
