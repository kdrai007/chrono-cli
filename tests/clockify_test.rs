use chrono::{TimeZone, Utc};
use chrono_cli::clockify::{
    entry_to_create_request, ClockifyApi, ClockifyClient, ClockifyError, ClockifyProject,
    ClockifyTag, ClockifyUser, ClockifyWorkspace, CreateTimeEntryRequest, SyncEngine, SyncError,
    SyncResult, TimeEntryResponse, TimeInterval,
};
use chrono_cli::domain::{EntryMode, Project, Tag, TimeEntry};
use chrono_cli::storage::Database;
use std::sync::Mutex;

#[derive(Default)]
struct MockClockifyClient {
    workspaces: Vec<ClockifyWorkspace>,
    projects: Vec<ClockifyProject>,
    tags: Vec<ClockifyTag>,
    created_entries: Mutex<Vec<(String, CreateTimeEntryRequest)>>,
    fail_create: bool,
    fail_projects: bool,
    fail_tags: bool,
}

impl ClockifyApi for MockClockifyClient {
    fn get_workspaces(&self) -> Result<Vec<ClockifyWorkspace>, ClockifyError> {
        Ok(self.workspaces.clone())
    }

    fn get_projects(&self, workspace_id: &str) -> Result<Vec<ClockifyProject>, ClockifyError> {
        if workspace_id.trim().is_empty() {
            return Err(ClockifyError::MissingWorkspace);
        }
        if self.fail_projects {
            return Err(ClockifyError::Other("Failed to fetch projects".into()));
        }
        Ok(self.projects.clone())
    }

    fn get_tags(&self, workspace_id: &str) -> Result<Vec<ClockifyTag>, ClockifyError> {
        if workspace_id.trim().is_empty() {
            return Err(ClockifyError::MissingWorkspace);
        }
        if self.fail_tags {
            return Err(ClockifyError::Other("Failed to fetch tags".into()));
        }
        Ok(self.tags.clone())
    }

    fn create_time_entry(
        &self,
        workspace_id: &str,
        req: &CreateTimeEntryRequest,
    ) -> Result<TimeEntryResponse, ClockifyError> {
        if workspace_id.trim().is_empty() {
            return Err(ClockifyError::MissingWorkspace);
        }
        if self.fail_create {
            return Err(ClockifyError::ApiError {
                status: 400,
                message: "Mock rejection".to_string(),
            });
        }
        let mut list = self.created_entries.lock().unwrap();
        let id = format!("remote_entry_{}", list.len() + 1);
        list.push((workspace_id.to_string(), req.clone()));

        Ok(TimeEntryResponse {
            id,
            description: Some(req.description.clone()),
            project_id: req.project_id.clone(),
            workspace_id: Some(workspace_id.to_string()),
            user_id: Some("user_123".to_string()),
            tag_ids: req.tag_ids.clone(),
            billable: req.billable,
            time_interval: Some(TimeInterval {
                start: req.start,
                end: req.end,
                duration: Some("PT1H".to_string()),
            }),
        })
    }
}

#[test]
fn test_models_serialization_deserialization() {
    // 1. ClockifyUser
    let user_json = r#"{
        "id": "u1",
        "email": "student@example.com",
        "name": "Jane Student",
        "activeWorkspace": "ws1",
        "defaultWorkspace": "ws1",
        "status": "ACTIVE"
    }"#;
    let user: ClockifyUser = serde_json::from_str(user_json).unwrap();
    assert_eq!(user.id, "u1");
    assert_eq!(user.email, "student@example.com");
    assert_eq!(user.name, "Jane Student");
    assert_eq!(user.active_workspace.as_deref(), Some("ws1"));
    assert_eq!(user.default_workspace.as_deref(), Some("ws1"));

    let user_ser = serde_json::to_string(&user).unwrap();
    assert!(user_ser.contains("activeWorkspace"));
    assert!(user_ser.contains("defaultWorkspace"));

    // 2. ClockifyWorkspace
    let ws_json = r#"{
        "id": "ws1",
        "name": "Study Workspace"
    }"#;
    let ws: ClockifyWorkspace = serde_json::from_str(ws_json).unwrap();
    assert_eq!(ws.id, "ws1");
    assert_eq!(ws.name, "Study Workspace");

    // 3. ClockifyProject
    let project_json = r##"{
        "id": "p1",
        "name": "CS101",
        "workspaceId": "ws1",
        "color": "#ff0000",
        "archived": false,
        "billable": false
    }"##;
    let project: ClockifyProject = serde_json::from_str(project_json).unwrap();
    assert_eq!(project.id, "p1");
    assert_eq!(project.name, "CS101");
    assert_eq!(project.workspace_id.as_deref(), Some("ws1"));
    assert_eq!(project.color.as_deref(), Some("#ff0000"));
    assert!(!project.archived);
    assert!(!project.billable);

    let proj_ser = serde_json::to_string(&project).unwrap();
    assert!(proj_ser.contains("workspaceId"));

    // 4. ClockifyTag
    let tag_json = r#"{
        "id": "t1",
        "name": "homework",
        "workspaceId": "ws1",
        "archived": false
    }"#;
    let tag: ClockifyTag = serde_json::from_str(tag_json).unwrap();
    assert_eq!(tag.id, "t1");
    assert_eq!(tag.name, "homework");
    assert_eq!(tag.workspace_id.as_deref(), Some("ws1"));
    assert!(!tag.archived);

    // 5. CreateTimeEntryRequest payload structure
    let start_dt = Utc.with_ymd_and_hms(2026, 10, 4, 10, 0, 0).unwrap();
    let end_dt = Utc.with_ymd_and_hms(2026, 10, 4, 11, 0, 0).unwrap();
    let req = CreateTimeEntryRequest {
        start: start_dt,
        end: Some(end_dt),
        description: "Studying Algorithms".to_string(),
        project_id: Some("p1".to_string()),
        tag_ids: Some(vec!["t1".to_string(), "t2".to_string()]),
        billable: false,
    };
    let val = serde_json::to_value(&req).unwrap();
    assert!(val.get("start").is_some());
    assert!(val.get("end").is_some());
    assert_eq!(val["description"], "Studying Algorithms");
    assert_eq!(val["projectId"], "p1");
    assert_eq!(val["tagIds"], serde_json::json!(["t1", "t2"]));
    assert_eq!(val["billable"], false);

    // 6. TimeEntryResponse parsing
    let resp_json = r#"{
        "id": "entry_999",
        "description": "Studying Algorithms",
        "projectId": "p1",
        "workspaceId": "ws1",
        "userId": "u1",
        "tagIds": ["t1", "t2"],
        "billable": false,
        "timeInterval": {
            "start": "2026-10-04T10:00:00Z",
            "end": "2026-10-04T11:00:00Z",
            "duration": "PT1H"
        }
    }"#;
    let resp: TimeEntryResponse = serde_json::from_str(resp_json).unwrap();
    assert_eq!(resp.id, "entry_999");
    assert_eq!(resp.description.as_deref(), Some("Studying Algorithms"));
    assert_eq!(resp.project_id.as_deref(), Some("p1"));
    assert_eq!(resp.tag_ids, Some(vec!["t1".to_string(), "t2".to_string()]));
    assert!(!resp.billable);
    let interval = resp.time_interval.unwrap();
    assert_eq!(interval.start, start_dt);
    assert_eq!(interval.end, Some(end_dt));
}

#[test]
fn test_mapping_local_time_entry_to_create_request() {
    let start = Utc.with_ymd_and_hms(2026, 10, 4, 8, 30, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 10, 4, 9, 15, 0).unwrap();

    let mut entry = TimeEntry::new("Physics Lab Prep", start);
    entry.end_time = Some(end);
    entry.entry_mode = EntryMode::Stopwatch;

    let req = entry_to_create_request(
        &entry,
        Some("remote_proj_1".to_string()),
        vec!["remote_tag_1".to_string(), "remote_tag_2".to_string()],
    );

    assert_eq!(req.start, start);
    assert_eq!(req.end, Some(end));
    assert_eq!(req.description, "Physics Lab Prep");
    assert_eq!(req.project_id.as_deref(), Some("remote_proj_1"));
    assert_eq!(
        req.tag_ids,
        Some(vec!["remote_tag_1".to_string(), "remote_tag_2".to_string()])
    );
    assert!(!req.billable);

    // Empty tags should result in None
    let req_no_tags = entry_to_create_request(&entry, None, Vec::new());
    assert_eq!(req_no_tags.project_id, None);
    assert_eq!(req_no_tags.tag_ids, None);
    assert!(!req_no_tags.billable);
}

#[test]
fn test_sync_result_summary_reporting() {
    let mut res = SyncResult::new();
    assert_eq!(res.pushed_entries, 0);
    assert_eq!(res.pulled_projects, 0);
    assert_eq!(res.pulled_tags, 0);
    assert!(res.errors.is_empty());
    assert!(res.is_success());
    assert_eq!(res.total_synced(), 0);

    res.pushed_entries = 5;
    res.pulled_projects = 2;
    res.pulled_tags = 3;
    assert_eq!(res.total_synced(), 10);
    assert!(res.is_success());

    res.errors.push("Failed to push entry 42".to_string());
    assert!(!res.is_success());
    assert_eq!(res.errors.len(), 1);
}

#[test]
fn test_offline_client_missing_config_error_handling() {
    // Missing API key on ClockifyClient
    let client_res = ClockifyClient::new("");
    assert!(matches!(client_res, Err(ClockifyError::MissingApiKey)));

    let client_res2 = ClockifyClient::new("   ");
    assert!(matches!(client_res2, Err(ClockifyError::MissingApiKey)));

    // Missing workspace in SyncEngine
    let db = Database::open_in_memory().unwrap();
    let mock = MockClockifyClient::default();
    let sync_res = SyncEngine::sync(&mock, &db, "");
    assert!(matches!(sync_res, Err(SyncError::MissingWorkspace)));

    let sync_res2 = SyncEngine::sync(&mock, &db, "   ");
    assert!(matches!(sync_res2, Err(SyncError::MissingWorkspace)));
}

#[test]
fn test_sync_engine_pulls_remote_projects_and_tags_into_sqlite() {
    let mut db = Database::open_in_memory().unwrap();

    // Pre-insert an existing local project without clockify_id
    let local_math = Project::new("Math").unwrap();
    let created_math = db.create_project(&local_math).unwrap();
    assert_eq!(created_math.clockify_id, None);

    // Pre-insert an existing local tag without clockify_id
    let local_exam = Tag::new("exam").unwrap();
    let created_exam = db.create_tag(&local_exam).unwrap();
    assert_eq!(created_exam.clockify_id, None);

    let mock = MockClockifyClient {
        workspaces: vec![ClockifyWorkspace {
            id: "ws1".to_string(),
            name: "University".to_string(),
            hourly_rate: None,
            image_url: None,
        }],
        projects: vec![
            ClockifyProject {
                id: "remote_math_id".to_string(),
                name: "Math".to_string(),
                workspace_id: Some("ws1".to_string()),
                color: Some("#123456".to_string()),
                archived: false,
                billable: false,
                client_id: None,
            },
            ClockifyProject {
                id: "remote_cs_id".to_string(),
                name: "Computer Science".to_string(),
                workspace_id: Some("ws1".to_string()),
                color: Some("#abcdef".to_string()),
                archived: false,
                billable: false,
                client_id: None,
            },
        ],
        tags: vec![
            ClockifyTag {
                id: "remote_exam_id".to_string(),
                name: "exam".to_string(),
                workspace_id: Some("ws1".to_string()),
                archived: false,
            },
            ClockifyTag {
                id: "remote_lab_id".to_string(),
                name: "lab".to_string(),
                workspace_id: Some("ws1".to_string()),
                archived: false,
            },
        ],
        ..Default::default()
    };

    let result = SyncEngine::sync(&mock, &db, "ws1").unwrap();
    assert_eq!(result.pulled_projects, 2);
    assert_eq!(result.pulled_tags, 2);
    assert_eq!(result.pushed_entries, 0);
    assert!(result.is_success());

    // Verify existing Math project now has clockify_id
    let math = db.get_project(created_math.id.unwrap()).unwrap().unwrap();
    assert_eq!(math.clockify_id.as_deref(), Some("remote_math_id"));
    assert_eq!(math.color, "#123456");

    // Verify newly pulled Computer Science project was created with clockify_id
    let cs = db
        .get_project_by_name("Computer Science")
        .unwrap()
        .expect("CS project should be created");
    assert_eq!(cs.clockify_id.as_deref(), Some("remote_cs_id"));
    assert_eq!(cs.color, "#abcdef");

    // Verify existing exam tag now has clockify_id
    let exam = db.get_tag(created_exam.id.unwrap()).unwrap().unwrap();
    assert_eq!(exam.clockify_id.as_deref(), Some("remote_exam_id"));

    // Verify newly pulled lab tag was created with clockify_id
    let lab = db
        .get_tag_by_name("lab")
        .unwrap()
        .expect("lab tag should be created");
    assert_eq!(lab.clockify_id.as_deref(), Some("remote_lab_id"));
}

#[test]
fn test_sync_engine_pushes_unsynced_completed_entries_to_clockify() {
    let mut db = Database::open_in_memory().unwrap();

    let proj = db
        .create_project(
            &Project::new("Physics")
                .unwrap()
                .with_clockify_id("remote_phys_id"),
        )
        .unwrap();

    let tag = db
        .create_tag(&Tag::new("hw").unwrap().with_clockify_id("remote_hw_id"))
        .unwrap();

    let start1 = Utc.with_ymd_and_hms(2026, 10, 4, 14, 0, 0).unwrap();
    let end1 = Utc.with_ymd_and_hms(2026, 10, 4, 15, 0, 0).unwrap();

    // 1. Unsynced completed entry
    let mut entry1 = TimeEntry::new("Completed Assignment", start1);
    entry1.end_time = Some(end1);
    entry1.project_id = proj.id;
    entry1.tags = vec![tag.clone()];
    entry1.synced = false;
    let saved_entry1 = db.create_manual_entry(&entry1).unwrap();
    assert!(!saved_entry1.synced);
    assert_eq!(saved_entry1.clockify_id, None);

    // 2. Active running entry (end_time = None) -> should NOT be pushed
    let start2 = Utc.with_ymd_and_hms(2026, 10, 4, 16, 0, 0).unwrap();
    let mut entry2 = TimeEntry::new("Still Running Task", start2);
    entry2.project_id = proj.id;
    entry2.synced = false;
    let saved_entry2 = db.start_entry(&entry2).unwrap();
    assert!(!saved_entry2.synced);

    let mock = MockClockifyClient::default();
    let result = SyncEngine::sync(&mock, &db, "ws1").unwrap();

    assert_eq!(result.pushed_entries, 1);
    assert!(result.is_success());

    // Verify completed entry is now marked synced = true with clockify_id populated
    let reloaded1 = db.get_entry(saved_entry1.id.unwrap()).unwrap().unwrap();
    assert!(reloaded1.synced);
    assert!(reloaded1.clockify_id.is_some());
    assert_eq!(reloaded1.clockify_id.as_deref(), Some("remote_entry_1"));

    // Verify active entry is NOT marked synced
    let reloaded2 = db.get_entry(saved_entry2.id.unwrap()).unwrap().unwrap();
    assert!(!reloaded2.synced);
    assert_eq!(reloaded2.clockify_id, None);

    // Verify payload sent to Clockify
    let sent = mock.created_entries.lock().unwrap();
    assert_eq!(sent.len(), 1);
    let (ws, req) = &sent[0];
    assert_eq!(ws, "ws1");
    assert_eq!(req.description, "Completed Assignment");
    assert_eq!(req.project_id.as_deref(), Some("remote_phys_id"));
    assert_eq!(req.tag_ids, Some(vec!["remote_hw_id".to_string()]));
    assert_eq!(req.start, start1);
    assert_eq!(req.end, Some(end1));
    assert!(!req.billable);
}

#[test]
fn test_sync_engine_handles_entry_push_failure_gracefully() {
    let mut db = Database::open_in_memory().unwrap();

    let start = Utc.with_ymd_and_hms(2026, 10, 4, 14, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 10, 4, 15, 0, 0).unwrap();

    let mut entry = TimeEntry::new("Failing Entry", start);
    entry.end_time = Some(end);
    entry.synced = false;
    let saved_entry = db.create_manual_entry(&entry).unwrap();

    let mock = MockClockifyClient {
        fail_create: true,
        ..Default::default()
    };

    let result = SyncEngine::sync(&mock, &db, "ws1").unwrap();
    assert_eq!(result.pushed_entries, 0);
    assert!(!result.is_success());
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains("Mock rejection"));

    // Entry should remain unsynced
    let reloaded = db.get_entry(saved_entry.id.unwrap()).unwrap().unwrap();
    assert!(!reloaded.synced);
    assert_eq!(reloaded.clockify_id, None);
}

#[test]
fn test_clockify_client_initialization_with_https_base_url() {
    let client = ClockifyClient::new("test-api-key-12345");
    assert!(client.is_ok(), "Client should initialize with TLS support");
    let c = client.unwrap();
    assert_eq!(c.api_key(), "test-api-key-12345");
    assert!(c.base_url().starts_with("https://"));
}
