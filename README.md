# ⏱ Clockify TUI

[![CI Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)]()
[![Rust Version](https://img.shields.io/badge/rust-1.75%2B-orange.svg)]()
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS-lightgrey.svg)]()

> **Terminal-based Clockify study hours and project manager designed specifically for students and focused learners.**

`clockify-tui` provides a distraction-free, terminal-native workspace for tracking coursework, revisions, reading sessions, and problem sets. Built with a local-first SQLite architecture in WAL mode, it ensures lightning-fast performance, rock-solid offline reliability, and zero latency, with optional two-way synchronization to Clockify cloud workspaces.

---

## 🚀 Key Features

* **⚡ Local-First & Offline Resilience**: Powered by an embedded SQLite database using Write-Ahead Logging (WAL) and foreign keys. Log hours uninterrupted on laptops, trains, or offline study spots.
* **⏱ Dual-Mode Tracking**:
  * **Stopwatch Mode**: Open-ended tracking with real-time digital ASCII clock display for deep-work marathons.
  * **Pomodoro Mode**: Structured interval tracking with automatic phase progression (Work 25m → Short Break 5m → Long Break 15m), complete with desktop notifications and terminal bell alerts.
* **🎯 Course & Project Target Tracking**: Define weekly study hour targets (e.g. 10 hrs/week for Algorithms) with colored visual progress bars, percentage indicators, and quick archive capabilities.
* **🔥 Study Streak & Analytics Dashboard**:
  * Consecutive study day streak tracking with a flame counter (`🔥 N days streak`).
  * Mon–Sun weekly study time bar chart comparing daily focus hours.
  * Subject breakdown cards detailing focus percentage per course.
* **🖥 Headless CLI & Waybar / Polybar Integration**: Full CLI subcommands for scriptable workflows, terminal hotkeys, and real-time status bar modules with native Waybar JSON support.
* **☁ Clockify Cloud Synchronization**: Optional two-way synchronization engine pulling remote projects/tags and pushing completed study logs to your Clockify account.

---

## 📦 Installation & Build

### Prerequisites
* Rust toolchain (1.75+ recommended): [rustup.rs](https://rustup.rs/)
* SQLite development headers (`libsqlite3-dev` on Debian/Ubuntu, `sqlite` on Arch Linux / Fedora / macOS)
* Desktop notification daemon (e.g., `dunst`, `mako`, or `swaync` on Linux)

### Building from Source

```bash
# Clone the repository
git clone https://github.com/kdrai/clockify-tui.git
cd clockify-tui

# Build optimized release binary
cargo build --release

# Install locally to ~/.cargo/bin
cargo install --path .
```

To build without cloud synchronization dependencies (offline-only binary):
```bash
cargo build --release --no-default-features
```

---

## ⚙ Configuration Guide

The configuration file is stored at `~/.config/clockify-tui/config.toml` (or custom path via `CLOCKIFY_CONFIG_PATH` / `--config-path`). If not found, `clockify-tui` automatically applies sensible student defaults.

### Complete TOML Schema

```toml
[general]
# Whether to display desktop notification banners on phase and target events
desktop_notifications = true

# Whether to emit terminal audio/bell character (\x07) on session completion
terminal_bell = true

# Time format for timestamps: "24h" or "12h"
time_format = "24h"

[pomodoro]
# Duration of focused study session in minutes (default: 25)
work_duration_mins = 25

# Duration of short relaxation break in minutes (default: 5)
short_break_mins = 5

# Duration of extended relaxation break in minutes (default: 15)
long_break_mins = 15

# Number of work sessions before triggering a long break (default: 4)
sessions_until_long_break = 4

[clockify]
# Enable cloud synchronization with Clockify API
enabled = false

# Your Clockify Personal API key (Clockify Settings -> API -> Generate)
api_key = "your_clockify_api_key_here"

# Clockify Workspace ID where time entries will be pushed
workspace_id = "your_workspace_id_here"

# Automatically push unsynced time entries when exiting the application
sync_on_exit = false
```

---

## 📊 Waybar Status Bar Integration

`clockify-tui` outputs custom Waybar JSON via `clockify-tui status --json`. It reports formatted time elapsed, active project, description, and status classes (`"running"` or `"idle"`).

### JSON Output Schema

```json
{
  "text": "Calculus Problem Set 4 [Mathematics] (00:24:12)",
  "tooltip": "Calculus Problem Set 4\nProject: Mathematics\nStarted: 14:15:00\nElapsed: 00:24:12\nTags: #Calculus #Homework",
  "class": "running",
  "alt": "running"
}
```

When idle:
```json
{
  "text": "Idle",
  "tooltip": "No active timer",
  "class": "idle",
  "alt": "idle"
}
```

### Waybar Configuration (`~/.config/waybar/config`)

Add `custom/clockify` to your `modules-left`, `modules-center`, or `modules-right`:

```json
"custom/clockify": {
    "format": "⏱ {}",
    "return-type": "json",
    "interval": 2,
    "exec": "clockify-tui status --json",
    "on-click": "clockify-tui stop",
    "on-click-middle": "clockify-tui start --pomodoro",
    "tooltip": true
}
```

### Waybar Style (`~/.config/waybar/style.css`)

```css
#custom-clockify {
    padding: 0 10px;
    border-radius: 6px;
    background-color: #282c34;
    color: #abb2bf;
}

#custom-clockify.running {
    background-color: #98c379;
    color: #1e1e1e;
    font-weight: bold;
}

#custom-clockify.idle {
    color: #5c6370;
}
```

---

## ⌨ Complete TUI Keybindings Reference

Press `?` at any point in the TUI to open the built-in cheatsheet modal.

| Keybinding | Action | Category |
|:---|:---|:---|
| `[1]` – `[4]` | Switch directly to Tab 1 (Timer), 2 (History), 3 (Projects), 4 (Analytics) | Navigation |
| `[Tab]` / `[S-Tab]` | Cycle to Next / Previous tab | Navigation |
| `[j]` / `[k]` or `[↑]` / `[↓]` | Navigate lists and tables up/down | Navigation |
| `[Space]` | Start / Stop active timer | Timer |
| `[p]` | Toggle Pomodoro mode / cycle phase | Timer |
| `[r]` | Repeat selected recent study session | Timer |
| `[n]` | Log a new manual time entry | History |
| `[e]` | Edit selected time entry | History |
| `[d]` | Delete selected time entry | History |
| `[/]` | Filter history by description or project name | History |
| `[a]` | Add new project / course with weekly target | Projects |
| `[e]` | Edit selected project target hours or color | Projects |
| `[x]` | Toggle archive status on selected project | Projects |
| `[d]` | Delete selected project | Projects |
| `[r]` | Refresh analytics, streaks, and subject breakdown | Analytics |
| `[s]` | Trigger Clockify cloud synchronization | General |
| `[?]` / `[Esc]` | Toggle / close help modal | General |
| `[q]` / `Ctrl+C` | Quit application | General |

---

## 💻 CLI Reference & Headless Commands

Manage your study sessions directly from scripts, shell aliases, or window manager hotkeys:

### 1. `start` — Start a Timer
```bash
# Start an open stopwatch timer with description
clockify-tui start "Reviewing Chapter 4 Linear Equations"

# Start with an associated course/project and tags
clockify-tui start "Physics Lab Report" -p "Physics" -t "Lab,Draft"

# Start directly as a structured Pomodoro work session
clockify-tui start "Midterm Revision" -p "Algorithms" --pomodoro
```

### 2. `stop` — Stop Current Active Timer
```bash
clockify-tui stop
```
*Outputs session duration and confirms database write.*

### 3. `status` — Check Current State
```bash
# Plain-text human-readable format
clockify-tui status

# JSON format for status bars (Waybar, Polybar, etc.)
clockify-tui status --json
```

### 4. `export` — Export Data to CSV or JSON
```bash
# Output CSV to stdout
clockify-tui export --format csv

# Export JSON to a target file
clockify-tui export --format json -o ~/backup_study_sessions.json
```

### 5. `sync` — Clockify Cloud Sync
```bash
clockify-tui sync
```
*Pushes unsynced local study logs and pulls remote courses and tags.*

---

## 🗄 Architecture & Design Principles

* **Local-First Reliability**: All writes occur immediately in the local SQLite database inside an isolated transaction. No network call can ever block or drop a user action.
* **Responsive 80x24 Terminal Layouts**: All four viewports (Timer, History, Projects, Analytics) and all modal dialogs are explicitly designed and tested to fit standard 80×24 terminal displays without overflowing or clipping.
* **Notification Decoupling**: Desktop alerts and terminal bells operate asynchronously via a decoupled notification subsystem, gracefully handling headless, SSH, and GUI desktop environments alike.

---

## 📄 License

Distributed under the MIT License. See `LICENSE` for details.
