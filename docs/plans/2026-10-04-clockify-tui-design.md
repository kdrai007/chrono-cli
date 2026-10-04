# Design Specification: Clockify TUI for Students

**Date:** 2026-10-04  
**Project:** `clockify-tui`  
**Status:** Approved  

---

## 1. Overview & Goals

`clockify-tui` is a high-performance terminal user interface (TUI) and command-line utility built in Rust for students to track study hours, organize academic projects/courses, and stay productive. It provides a hybrid local-first architecture—allowing seamless offline tracking in an embedded SQLite database with optional on-demand two-way synchronization to the Clockify REST API and CSV/JSON export.

### Key Objectives
- **Student-Centric Time Tracking**: Track time via live stopwatch, custom Pomodoro focus/break sessions, or manual entry logging.
- **Project & Goal Management**: Organize by courses/subjects, assign colors, and track weekly study hour targets with visual progress bars and daily study streak counters.
- **Hybrid Local-First Storage**: Zero setup required; instant local SQLite persistence (`wal` mode enabled) with optional Clockify cloud sync.
- **Dual Interface**: Interactive Ratatui dashboard for full management + headless CLI subcommands (`start`, `stop`, `status`, `sync`, `export`) optimized for scripting and Waybar/status-bar integration.
- **Cross-Platform Notifications**: Native desktop notifications (`notify-send` / freedesktop) + terminal bells for Pomodoro and target events.

---

## 2. Architecture & System Design

```
                         ┌────────────────────────────────────────┐
                         │            clockify-tui CLI            │
                         │   (clap: start, stop, status, sync)    │
                         └───────────────────┬────────────────────┘
                                             │
┌────────────────────────────┐               │
│        Ratatui TUI         │               │
│ (Tabs: Timer, Log,         ├───────────────┤
│  Projects, Analytics)      │               │
└──────────────┬─────────────┘               │
               │                             │
               ▼                             ▼
        ┌─────────────────────────────────────────────────┐
        │                  Domain Layer                   │
        │   - TimeEntry, Project, Tag, PomodoroStateMachine│
        │   - Study Targets, Streaks Calculator           │
        └──────────────┬──────────────────────────┬───────┘
                       │                          │
                       ▼                          ▼
        ┌──────────────────────────────┐  ┌───────────────┐
        │        SQLite Store          │  │ Clockify Sync │
        │ (rusqlite with WAL mode)     │  │ (HTTP REST)   │
        └──────────────────────────────┘  └───────────────┘
```

### Module Organization
```
src/
├── main.rs            # Entrypoint: dispatches to CLI subcommands or launches TUI
├── cli/               # Clap CLI parsing and headless command implementations
│   ├── mod.rs
│   ├── args.rs
│   └── commands.rs
├── domain/            # Core business logic, entities, and state machines
│   ├── mod.rs
│   ├── entry.rs
│   ├── project.rs
│   ├── tag.rs
│   ├── pomodoro.rs
│   └── stats.rs
├── storage/           # SQLite migrations, transactions, and queries
│   ├── mod.rs
│   ├── db.rs
│   ├── schema.rs
│   └── repository.rs
├── clockify/          # Optional Clockify REST API client & sync engine
│   ├── mod.rs
│   ├── client.rs
│   └── models.rs
├── notify/            # Desktop notification & terminal bell handler
│   └── mod.rs
├── config/            # TOML configuration loading and defaults
│   └── mod.rs
└── tui/               # Ratatui user interface
    ├── mod.rs
    ├── app.rs
    ├── event.rs
    ├── ui.rs
    ├── views/
    │   ├── timer.rs
    │   ├── history.rs
    │   ├── projects.rs
    │   └── analytics.rs
    └── widgets/
        ├── modal.rs
        └── help.rs
```

---

## 3. Data Storage & Schema

### SQLite Database (`~/.local/share/clockify-tui/clockify.db`)

1. **`projects`**
   - `id`: INTEGER PRIMARY KEY AUTOINCREMENT
   - `name`: TEXT NOT NULL UNIQUE
   - `color`: TEXT NOT NULL DEFAULT '#3498db'
   - `target_hours_week`: REAL NOT NULL DEFAULT 0.0
   - `clockify_id`: TEXT NULL
   - `archived`: BOOLEAN NOT NULL DEFAULT 0
   - `created_at`: TEXT NOT NULL

2. **`tags`**
   - `id`: INTEGER PRIMARY KEY AUTOINCREMENT
   - `name`: TEXT NOT NULL UNIQUE
   - `clockify_id`: TEXT NULL

3. **`time_entries`**
   - `id`: INTEGER PRIMARY KEY AUTOINCREMENT
   - `description`: TEXT NOT NULL DEFAULT ''
   - `project_id`: INTEGER REFERENCES projects(id) ON DELETE SET NULL
   - `start_time`: TEXT NOT NULL (RFC3339 UTC)
   - `end_time`: TEXT NULL (NULL indicates an actively running timer)
   - `entry_mode`: TEXT NOT NULL DEFAULT 'stopwatch' ('stopwatch', 'pomodoro_work', 'pomodoro_break')
   - `pomodoro_index`: INTEGER NOT NULL DEFAULT 0
   - `synced`: BOOLEAN NOT NULL DEFAULT 0
   - `clockify_id`: TEXT NULL
   - `created_at`: TEXT NOT NULL
   - `updated_at`: TEXT NOT NULL

4. **`entry_tags`**
   - `entry_id`: INTEGER NOT NULL REFERENCES time_entries(id) ON DELETE CASCADE
   - `tag_id`: INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE
   - PRIMARY KEY (entry_id, tag_id)

### Configuration (`~/.config/clockify-tui/config.toml`)
```toml
[general]
desktop_notifications = true
terminal_bell = true
time_format = "24h" # "12h" or "24h"

[pomodoro]
work_duration_mins = 25
short_break_mins = 5
long_break_mins = 15
sessions_until_long_break = 4

[clockify]
enabled = false
api_key = ""
workspace_id = ""
sync_on_exit = false
```

---

## 4. TUI Layout & Navigation

### Views
1. **[1] Timer**:
   - Live stopwatch or Pomodoro countdown timer with large digital display.
   - Project badge, active tags, and task description.
   - Pomodoro interval status gauge (e.g. Session 2 of 4) and break timer.
   - Quick summary of today's study hours and recent session list with instant repeat key (`r`).
2. **[2] History**:
   - Timesheet table grouped by date (Today, Yesterday, Earlier this week).
   - CRUD controls: New manual entry (`n`), Edit entry (`e`), Delete entry (`d`), Search/Filter (`/`).
3. **[3] Projects**:
   - Subject/Course list with color indicators and total logged time.
   - Weekly target progress bars (e.g., `8.5h / 10.0h [████████░░] 85%`).
   - Add/Edit/Archive subjects.
4. **[4] Analytics**:
   - Consecutive study streak counter (`🔥 5 Days`).
   - Weekly daily distribution bar chart (Mon–Sun).
   - Subject breakdown percentages.
   - Completed Pomodoro counts.

### Keybindings
- `1`–`4` or `Tab` / `Shift+Tab`: Switch tabs
- `Space`: Start / Pause / Resume timer
- `p`: Switch between Stopwatch and Pomodoro modes
- `n`: New entry (live or past)
- `e`: Edit selected entry or project
- `d`: Delete selected item
- `r`: Repeat selected entry
- `s`: Trigger Clockify sync
- `j` / `k` or `Down` / `Up`: Navigation
- `?`: Toggle help modal
- `q`: Quit (running timers persist safely in SQLite)

---

## 5. CLI & Waybar Integration

### Subcommands
- `clockify-tui` (no arguments): Launch interactive Ratatui TUI.
- `clockify-tui start [DESCRIPTION] [-p PROJECT] [-t TAGS] [--pomodoro]`: Start timer from shell.
- `clockify-tui stop`: Stop active timer.
- `clockify-tui status`: Print human-readable status.
- `clockify-tui status --json`: Emit JSON payload formatted for Waybar / Polybar:
  ```json
  {"text": "⏱ 00:25 (Math)", "tooltip": "Review Chapter 4\nProject: Linear Algebra\nStarted: 08:30", "class": "running"}
  ```
- `clockify-tui sync`: Trigger manual Clockify two-way sync.
- `clockify-tui export --format <csv|json> --output <FILE>`: Export study logs.

---

## 6. Testing & Quality Assurance
- **Domain Unit Tests**: Pomodoro state transitions, streak calculation across edge cases (midnight rollover, leap years), and target percentage math.
- **Storage Integration Tests**: In-memory SQLite tests (`:memory:`) ensuring schema consistency, cascading deletes, and query correctness.
- **CLI Tests**: `assert_cmd` end-to-end tests for CLI commands and `--json` format output.
