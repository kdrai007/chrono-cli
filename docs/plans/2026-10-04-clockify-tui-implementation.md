# Clockify TUI for Students - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a high-performance, student-centric Clockify TUI and CLI in Rust with hybrid local SQLite storage, Pomodoro & stopwatch tracking, course targets, study streak analytics, desktop notifications, Waybar status integration, and optional Clockify REST API sync.

**Architecture:** Layered clean architecture separating Domain models, SQLite storage layer (rusqlite WAL), Notification engine (notify-rust), Headless CLI parser (clap), Clockify REST sync (reqwest), and Ratatui full-screen dashboard with multi-tab navigation.

**Tech Stack:** Rust 1.98+, Ratatui, Crossterm, Rusqlite (bundled), Chrono, Clap, Serde/TOML/JSON, Csv, Notify-rust, Dirs.

---

### Task 1: Scaffolding and Cargo Configuration

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`

**Step 1: Write Cargo.toml**
Add all required dependencies with pinned compatible versions.

```toml
[package]
name = "clockify-tui"
version = "0.1.0"
edition = "2021"
authors = ["kdrai"]
description = "Terminal-based Clockify study hours and project manager for students"

[dependencies]
ratatui = "0.29"
crossterm = { version = "0.28", features = ["event-stream"] }
rusqlite = { version = "0.32", features = ["bundled", "chrono"] }
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
dirs = "5.0"
notify-rust = "4.11"
csv = "1.3"
reqwest = { version = "0.12", features = ["blocking", "json"], default-features = false, optional = true }

[features]
default = ["cloud-sync"]
cloud-sync = ["dep:reqwest"]

[dev-dependencies]
tempfile = "3.10"
assert_cmd = "2.0"
predicates = "3.1"
```

**Step 2: Create minimal src/lib.rs and src/main.rs**
Scaffold initial crate entrypoints.

**Step 3: Verify build**
Run: `cargo check`  
Expected: PASS with 0 warnings.

**Step 4: Commit**
```bash
git add Cargo.toml src/
git commit -m "chore: scaffold project structure and cargo dependencies"
```

---

### Task 2: Configuration Module

**Files:**
- Create: `src/config/mod.rs`
- Test: `tests/config_test.rs`

**Step 1: Write the failing test**
Verify default config values, serialization/deserialization, and custom file loading.

```rust
#[test]
fn test_default_config() {
    let cfg = Config::default();
    assert_eq!(cfg.pomodoro.work_duration_mins, 25);
    assert_eq!(cfg.pomodoro.short_break_mins, 5);
    assert!(cfg.general.desktop_notifications);
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test --test config_test`  
Expected: FAIL (module not defined)

**Step 3: Implement config module**
Implement `AppConfig`, `GeneralConfig`, `PomodoroConfig`, `ClockifyConfig`, with `dirs::config_dir` path resolution.

**Step 4: Run test to verify it passes**
Run: `cargo test --test config_test`  
Expected: PASS

**Step 5: Commit**
```bash
git add src/config/ tests/config_test.rs
git commit -m "feat(config): implement toml configuration loading and defaults"
```

---

### Task 3: Core Domain Models

**Files:**
- Create: `src/domain/mod.rs`
- Create: `src/domain/project.rs`
- Create: `src/domain/tag.rs`
- Create: `src/domain/entry.rs`
- Create: `src/domain/pomodoro.rs`
- Test: `tests/domain_test.rs`

**Step 1: Write the failing tests**
Test `TimeEntry::duration()`, `PomodoroStateMachine` transitions (Work -> ShortBreak -> Work -> LongBreak), and formatting helper functions.

**Step 2: Run test to verify it fails**
Run: `cargo test --test domain_test`  
Expected: FAIL

**Step 3: Implement domain models**
- Define `Project`, `Tag`, `TimeEntry`, `EntryMode` (`Stopwatch`, `PomodoroWork`, `PomodoroBreak`).
- Implement `PomodoroState` transition machine: counts completed work sessions and rotates through short and long breaks.

**Step 4: Run test to verify it passes**
Run: `cargo test --test domain_test`  
Expected: PASS

**Step 5: Commit**
```bash
git add src/domain/ tests/domain_test.rs
git commit -m "feat(domain): implement core entities and pomodoro state machine"
```

---

### Task 4: SQLite Database Storage & Migrations

**Files:**
- Create: `src/storage/mod.rs`
- Create: `src/storage/schema.rs`
- Create: `src/storage/db.rs`
- Create: `src/storage/repository.rs`
- Test: `tests/storage_test.rs`

**Step 1: Write failing integration tests**
Test in-memory SQLite schema initialization (`PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;`), CRUD for Projects, Tags, and Time Entries, and fetching active vs completed timers.

**Step 2: Run test to verify it fails**
Run: `cargo test --test storage_test`  
Expected: FAIL

**Step 3: Implement storage repository**
- Setup tables: `projects`, `tags`, `time_entries`, `entry_tags`.
- Implement `Repository`:
  - `start_entry(desc, project_id, tags, mode)`
  - `stop_active_entry()`
  - `get_active_entry()`
  - `get_entries(start_date, end_date)`
  - `create_project(name, color, target_hours)`
  - `list_projects()`
  - `delete_entry(id)`
  - `update_entry(entry)`

**Step 4: Run test to verify it passes**
Run: `cargo test --test storage_test`  
Expected: PASS

**Step 5: Commit**
```bash
git add src/storage/ tests/storage_test.rs
git commit -m "feat(storage): implement sqlite persistence, schema migrations, and repository"
```

---

### Task 5: Study Streak & Analytics Aggregator

**Files:**
- Create: `src/domain/stats.rs`
- Test: `tests/stats_test.rs`

**Step 1: Write the failing tests**
Verify streak calculation (counting consecutive calendar days with at least 1 study session), daily hour totals, and weekly project target percentages.

**Step 2: Run test to verify it fails**
Run: `cargo test --test stats_test`  
Expected: FAIL

**Step 3: Implement stats calculation**
- Query daily study durations.
- Calculate current active streak & all-time longest streak.
- Calculate weekly progress against project target hours.

**Step 4: Run test to verify it passes**
Run: `cargo test --test stats_test`  
Expected: PASS

**Step 5: Commit**
```bash
git add src/domain/stats.rs tests/stats_test.rs
git commit -m "feat(stats): implement streak calculator and weekly target metrics"
```

---

### Task 6: Notifications & Audio/Bell System

**Files:**
- Create: `src/notify/mod.rs`
- Test: `tests/notify_test.rs`

**Step 1: Write tests for notification dispatcher**
Mock or verify safe fallbacks when desktop notification daemon is offline or in test environments.

**Step 2: Implement NotificationService**
- Desktop notification dispatch via `notify_rust::Notification`.
- Terminal bell (`\x07`) emit to stdout.
- Respect user preferences in `AppConfig`.

**Step 3: Verify execution**
Run: `cargo test --test notify_test`  
Expected: PASS

**Step 4: Commit**
```bash
git add src/notify/ tests/notify_test.rs
git commit -m "feat(notify): implement desktop notifications and terminal bell alerts"
```

---

### Task 7: Headless CLI Subcommands & Waybar JSON Support

**Files:**
- Create: `src/cli/mod.rs`
- Create: `src/cli/args.rs`
- Create: `src/cli/commands.rs`
- Test: `tests/cli_test.rs`

**Step 1: Write CLI integration test**
Test `start`, `stop`, and `status --json` commands via `assert_cmd`.

```rust
#[test]
fn test_cli_status_json() {
    let mut cmd = Command::cargo_bin("clockify-tui").unwrap();
    cmd.arg("status").arg("--json");
    cmd.assert().success();
}
```

**Step 2: Implement Clap parser and handlers**
- `clockify-tui start <DESC> [-p PROJECT] [-t TAGS] [--pomodoro]`
- `clockify-tui stop`
- `clockify-tui status [--json]` (Waybar friendly format with text, tooltip, class)
- `clockify-tui export --format <csv|json> --output <FILE>`

**Step 3: Run test to verify it passes**
Run: `cargo test --test cli_test`  
Expected: PASS

**Step 4: Commit**
```bash
git add src/cli/ tests/cli_test.rs
git commit -m "feat(cli): add headless subcommands with waybar json support and exporter"
```

---

### Task 8: Clockify REST API Sync Engine

**Files:**
- Create: `src/clockify/mod.rs`
- Create: `src/clockify/models.rs`
- Create: `src/clockify/client.rs`
- Create: `src/clockify/sync.rs`
- Test: `tests/clockify_test.rs`

**Step 1: Write serialization & sync payload unit tests**
Test Clockify API request and response JSON mapping.

**Step 2: Implement ClockifyClient**
- Endpoints: `/v1/user`, `/v1/workspaces/{id}/projects`, `/v1/workspaces/{id}/time-entries`.
- Implement push for unsynced local entries.
- Implement pull for remote projects and tags.
- Graceful offline fallback with structured error logging.

**Step 3: Run tests to verify**
Run: `cargo test --test clockify_test`  
Expected: PASS

**Step 4: Commit**
```bash
git add src/clockify/ tests/clockify_test.rs
git commit -m "feat(clockify): add clockify v1 rest api client and two-way sync engine"
```

---

### Task 9: TUI Framework, Event Loop & Terminal Setup

**Files:**
- Create: `src/tui/mod.rs`
- Create: `src/tui/event.rs`
- Create: `src/tui/app.rs`
- Create: `src/tui/ui.rs`

**Step 1: Implement terminal initialization & teardown**
Raw mode, alternate screen enter/leave, panic hook restoration to prevent broken terminal states.

**Step 2: Implement async/thread event reader**
Tick events (250ms) for live timer updates + Crossterm keyboard events channel.

**Step 3: Implement main tab bar navigation and header**
Tabs 1–4 switching via keys `1`–`4`, `Tab`, `q` to exit.

**Step 4: Verify manual launch**
Run: `cargo run -- --help`  
Run: `cargo test`

**Step 5: Commit**
```bash
git add src/tui/
git commit -m "feat(tui): scaffold ratatui event loop, app state, and root shell layout"
```

---

### Task 10: TUI Tab 1 - Timer & Pomodoro Screen

**Files:**
- Create: `src/tui/views/timer.rs`
- Create: `src/tui/widgets/big_clock.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/ui.rs`

**Step 1: Implement Big Digital Clock widget**
ASCII or clean segmented numbers rendering elapsed/remaining time (`00:25:00`).

**Step 2: Implement Timer View**
- Active status banner (Stopwatch vs Pomodoro Work/Break).
- Progress bar for Pomodoro phase.
- Project badge with custom color styling.
- Quick summary of today's total study time and recent sessions.
- Controls: `Space` (Start/Pause), `p` (Toggle Pomodoro), `r` (Repeat selected entry).

**Step 3: Verify build and test**
Run: `cargo check`

**Step 4: Commit**
```bash
git add src/tui/views/timer.rs src/tui/widgets/big_clock.rs src/tui/
git commit -m "feat(tui): implement live timer and pomodoro view with big clock display"
```

---

### Task 11: TUI Tab 2 - History Timesheet Screen & Modals

**Files:**
- Create: `src/tui/views/history.rs`
- Create: `src/tui/widgets/modal.rs`
- Modify: `src/tui/app.rs`

**Step 1: Implement Timesheet Table**
- Group entries by date (Today, Yesterday, Earlier).
- Format columns: Time, Duration, Project, Tags, Description, Sync status.
- Vim navigation (`j`/`k`, `Up`/`Down`).

**Step 2: Implement Entry Edit & New Manual Entry Modal**
Inputs for Description, Project selector, Start/End times, and Tag selector.

**Step 3: Implement Delete confirmation modal (`d`)**

**Step 4: Commit**
```bash
git add src/tui/views/history.rs src/tui/widgets/modal.rs src/tui/
git commit -m "feat(tui): implement history timesheet table and entry editing modals"
```

---

### Task 12: TUI Tab 3 - Projects & Course Targets Screen

**Files:**
- Create: `src/tui/views/projects.rs`
- Modify: `src/tui/app.rs`

**Step 1: Implement Project List & Course Target View**
- Display project name, color badge, total logged time, and weekly target progress bar (`[████████░░] 80%`).
- Add project modal (`a`), Edit target modal (`e`), Archive (`x`).

**Step 2: Verify database integration**
Ensure edits instantly update SQLite and reflect in all views.

**Step 3: Commit**
```bash
git add src/tui/views/projects.rs src/tui/
git commit -m "feat(tui): implement projects view with weekly study targets and progress bars"
```

---

### Task 13: TUI Tab 4 - Analytics & Study Streaks Screen

**Files:**
- Create: `src/tui/views/analytics.rs`
- Modify: `src/tui/app.rs`

**Step 1: Implement Analytics Dashboard**
- Daily study streak counter (`🔥 5 Days`).
- Weekly breakdown BarChart (Mon–Sun study hours).
- Subject distribution progress gauges (percentage of time spent per subject).
- Pomodoro sessions counter.

**Step 2: Commit**
```bash
git add src/tui/views/analytics.rs src/tui/
git commit -m "feat(tui): implement analytics view with streak counter and study charts"
```

---

### Task 14: Help Modal, Keybinding Polish & End-to-End Verification

**Files:**
- Create: `src/tui/widgets/help.rs`
- Create: `README.md`
- Test: `tests/integration_test.rs`

**Step 1: Implement Help dialog (`?`)**
Overlay displaying all navigation and action shortcuts.

**Step 2: Add README.md with Student Usage Guide & Waybar Setup**
Include Waybar configuration example, keybindings, and CLI shortcuts.

**Step 3: Run comprehensive verification**
Run: `cargo test`  
Run: `cargo clippy -- -D warnings`  
Run: `cargo fmt -- --check`

**Step 4: Commit**
```bash
git add src/ README.md tests/
git commit -m "feat(polish): add help overlay, documentation, and end-to-end verification"
```
