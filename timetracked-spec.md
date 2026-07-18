# TimeTracked — Application Specification

## Overview

TimeTracked is a minimal personal time tracking desktop application built with Tauri 2 and vanilla HTML/JavaScript. It allows the user to quickly log what task they are working on and automatically tracks elapsed time. All entries are persisted in a local SQLite database. The app features a system tray icon for quick access and configurable appearance settings.

## Goals

- Minimal friction to start/stop tracking
- Always visible without obstructing workflow (configurable always-on-top)
- Simple data model for easy export and analysis
- Fast, lightweight, native desktop experience
- Remember user preferences and window state between sessions
- Quick access via macOS menu bar tray icon

## User Interface

### Main Window

#### Window Behavior

- **Always on top** — configurable via Preferences (default: enabled), applied immediately on change
- **Default size** — 320×320px, compact and unobtrusive
- **Resizable** — user can resize to show more recent tasks (minimum 280×200px)
- **Native decorations** — platform-native title bar
- **Position persistence** — remembers last position and size across restarts (saved to SQLite, debounced 500ms)
- **Font size** — configurable via Preferences (10–18px, default: 12px), applied immediately on change

#### Layout (top to bottom)

1. **Task Input Field**
   - Single-line text input at the top of the window
   - Placeholder text: "What are you working on?"
   - Supports free-text entry
   - Autocomplete dropdown appears while typing, suggesting previously used task names
   - Pressing `Enter` starts tracking the entered task
   - If a task is currently being tracked, pressing `Enter` with a new task name stops the current task and immediately starts the new one
   - Keyboard navigation: Arrow Up/Down to navigate suggestions, Escape to dismiss

2. **Active Task Indicator**
   - Displays the currently tracked task name
   - Shows elapsed time (HH:MM:SS), updating every second
   - Stop button (red) to end tracking without starting a new task
   - When idle, displays "No task running" in muted italic text

3. **Recent Tasks List**
   - Header: "Recent" label
   - Shows tasks from the last 7 days, grouped by day (most recent day first)
   - Each day has a header label: "Today", "Yesterday", or weekday + date (e.g., "Wed, Jul 16")
   - Within each day, tasks are sorted by time spent descending
   - The same task appearing on multiple days shows as a separate entry under each day
   - Each entry displays:
     - Task name (truncated with ellipsis if too long)
     - Daily total time spent on that task for that specific day (e.g., "2h15m" or "45m")
     - Play button (▶) to restart tracking that task
   - Scrollable — grows with window height when resized
   - Clicking ▶ on a recent task creates a new time entry

### System Tray (macOS Menu Bar)

- Clock icon displayed in the macOS menu bar (top-right)
- Icon rendered as a template image (adapts to light/dark menu bar)
- Clicking the icon shows a dropdown menu

#### Tray Menu Items

| Item | Condition | Action |
|------|-----------|--------|
| ⏱ {task name} | Task active | Disabled, shows current task |
| ■ Stop | Task active | Stops the active task |
| No task running | No task active | Disabled, informational |
| ▶ {recent task} | Always (up to 5) | Starts tracking that task |
| Preferences... | Always | Opens preferences window |
| Edit Entries... | Always | Opens entry editor window |
| Show Window | Always | Shows and focuses main window |
| Quit | Always | Exits the application |

#### Tray Behavior

- Menu rebuilds dynamically when tasks start or stop (from either tray or main window)
- Starting a task from tray stops any currently active task first
- Active task is excluded from the recent tasks list in the tray menu
- Changes made via tray are reflected immediately in the main window

### Preferences Window

- Opened via `File > Preferences`, `Cmd+,`, or tray menu "Preferences..."
- Separate window (400×250px, non-resizable)
- If already open, focuses the existing window instead of creating a new one

#### Sections

1. **Working Hours & Appearance**
   - End of day time picker (HH:MM format), default: 17:00
   - Font size number input (10–18px), default: 12
   - Always on top checkbox, default: checked
   - Save button persists all settings to database immediately
   - Visual confirmation "✓ Saved" on success
   - Settings changes are applied immediately to the main window (no restart required)

2. **Export**
   - Description text explaining the export
   - Export CSV button — opens native file save dialog
   - User chooses location and filename
   - Default filename: `timetracked-YYYY-MM-DD.csv`
   - Filter: CSV files (*.csv)

### Entry Editor Window

- Opened via `File > Edit Entries...`, `Cmd+Shift+E`, or tray menu "Edit Entries..."
- Separate window (600×500px, resizable, min 500×300px)
- If already open, focuses the existing window instead of creating a new one

#### Layout

- Table view showing all entries, newest first
- Columns: Task, Start, End, Actions
- Entries with empty end_time show "running" badge in red italic
- Pagination: loads 50 entries at a time, "Load more..." button appends next page

#### Editing

- Click ✎ (edit) button to enter inline edit mode for that row
- Task name, start time, and end time become editable text inputs
- Save (✓) or Cancel (✕) buttons appear
- Enter key saves, Escape cancels
- Validation: task cannot be empty, start cannot be empty, end must be after start
- Error fields highlighted in red on validation failure
- Status message "✓ Entry updated" shown on success

#### Deleting

- Click 🗑 (delete) button shows inline confirmation "Delete? Yes / No"
- Confirming removes the entry permanently
- Status message "✓ Entry deleted" shown on success

### Native Menu

| Menu | Item | Shortcut | Action |
|------|------|----------|--------|
| File | Preferences... | `Cmd+,` | Opens preferences window |
| File | Edit Entries... | `Cmd+Shift+E` | Opens entry editor window |
| File | Export CSV... | `Cmd+E` | Opens save dialog, exports all entries |
| File | Quit | `Cmd+Q` | Exits application |
| Edit | Undo | `Cmd+Z` | Standard text editing |
| Edit | Redo | `Cmd+Shift+Z` | Standard text editing |
| Edit | Cut | `Cmd+X` | Standard text editing |
| Edit | Copy | `Cmd+C` | Standard text editing |
| Edit | Paste | `Cmd+V` | Standard text editing |
| Edit | Select All | `Cmd+A` | Standard text editing |

### Visual Design

- Dark theme: background `#1a1a2e`, cards `#16213e`, accent `#e94560`
- Monospace font for timer display (SF Mono / Menlo)
- System font (-apple-system) for all other text
- Compact spacing (8px padding, 4px gaps)
- Font sizes use relative `em` units, scaling with the configurable base font size
- Consistent styling across main and preferences windows

## Core Behavior

### Starting a Task

- User types a task name into the input field and presses `Enter`
- A new entry is created with the current timestamp as the start time
- The timer begins counting up from 00:00:00
- The input field is cleared after starting
- System tray menu is updated to reflect the new active task

### Stopping a Task

A task stops in one of four ways:

1. **Manual stop** — user clicks the Stop button in the main window
2. **Task switch** — user enters a new task name and presses Enter (stops current, starts new)
3. **Automatic stop** — at the configured end-of-day time, only if the task was started before that time
4. **Tray stop** — user clicks "■ Stop" in the system tray menu

### Autocomplete

- As the user types in the input field, a dropdown appears with matching task names from history
- Matching is case-insensitive, substring-based (SQL `LIKE %query%`)
- Returns up to 10 matching suggestions
- Selecting an autocomplete suggestion populates the input field (user still presses Enter to start)
- Keyboard navigation: Arrow Up/Down to cycle through suggestions
- Source: distinct task names from database history, ordered by most recently used

### Automatic Stop at End of Day

- Checked every 30 seconds while the app is running
- At the configured end-of-day time (default 17:00), the currently tracked task is automatically stopped **only if it was started before that time**
- Tasks started after the cutoff time continue running normally (evening/overtime work is not interrupted)
- The stop time recorded is exactly the cutoff time (e.g., 17:00:00)
- Cutoff time is configurable via Preferences

### Daily Time Totals (Recent List)

- The recent list shows the last 7 days of tracked data, grouped by day
- For each task on each day, the total time spent that day is displayed
- Includes time from the currently active task (calculated in real-time via SQL using current timestamp)
- Format: "Xh YYm" for hours+minutes, or "Ym" for less than an hour
- Days are ordered most recent first; tasks within a day are ordered by most time spent

### App Startup Behavior

- Restores window position and size from last session
- Applies saved font size to the UI
- Applies saved always-on-top preference
- Checks for an active task (entry with empty end_time in database)
- If found, resumes the timer from the original start time
- Recent tasks list is populated immediately
- System tray icon is created with current state

### Settings Live Update

- When settings are saved in the Preferences window, a `settings-changed` event is emitted
- The main window listens for this event and applies changes immediately:
  - Font size: updates `document.body.style.fontSize`
  - Always on top: calls `window.setAlwaysOnTop()`
- No application restart is required for any setting change

### CSV Export

- Exports all time entries from the database
- Opens a native file save dialog for the user to choose the save location
- Default filename: `timetracked-YYYY-MM-DD.csv`
- File filter: CSV files (*.csv)
- Accessible from: `File > Export CSV` menu (`Cmd+E`) or Preferences export button

## Data Storage

### Database

SQLite database stored in the application's data directory.

**File Location:** `~/.timetracked/timetracked.db`

### Schema

```sql
CREATE TABLE entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task TEXT NOT NULL,
    start TEXT NOT NULL,
    end_time TEXT NOT NULL DEFAULT ''
);

CREATE INDEX idx_entries_start ON entries(start);
CREATE INDEX idx_entries_task ON entries(task);
CREATE INDEX idx_entries_end ON entries(end_time);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### Settings Keys

| Key | Format | Default | Description |
|-----|--------|---------|-------------|
| `end_of_day` | `HH:MM` | `17:00` | Auto-stop cutoff time |
| `font_size` | integer string | `12` | UI base font size in pixels (10–18) |
| `always_on_top` | `true`/`false` | `true` | Whether main window stays on top |
| `window_position` | `x,y,width,height` | (none) | Last window position/size |

### CSV Export Format

```csv
task,start,end
"Write timetracked spec",2026-07-14T09:15:00,2026-07-14T10:02:33
"Code review CR-12345",2026-07-14T10:02:33,2026-07-14T10:45:12
"Write timetracked spec",2026-07-14T10:45:12,
```

| Column | Type   | Description                                      |
|--------|--------|--------------------------------------------------|
| task   | string | Free-text task name (quoted if contains commas)  |
| start  | string | ISO 8601 local datetime (YYYY-MM-DDTHH:MM:SS)   |
| end    | string | ISO 8601 local datetime, empty if still running  |

### Data Rules

- Every started task creates a new row (even if the same task was tracked before)
- The `end_time` column is empty string while the task is actively being tracked
- On application startup, if an entry has an empty `end_time`, it is treated as still running (timer resumes)
- The database and directory are created automatically on first run
- If a legacy CSV file exists at `~/.timetracked/timetracked.csv` and the database is new, data is automatically migrated

## Technical Details

### Stack

- **Runtime**: Tauri 2 (Rust backend + WebView frontend)
- **Frontend**: Vanilla HTML, CSS, JavaScript (no framework, no bundler)
- **Backend**: Rust (SQLite I/O, system time, window management, native dialogs, system tray)
- **Storage**: SQLite via rusqlite (bundled)
- **Plugins**: tauri-plugin-opener, tauri-plugin-dialog

### Dependencies (Rust)

| Crate | Purpose |
|-------|---------|
| `tauri` (features: tray-icon, image-png) | Application framework with tray support |
| `tauri-plugin-opener` | Default opener plugin |
| `tauri-plugin-dialog` | Native file save dialog |
| `rusqlite` (bundled) | SQLite database |
| `chrono` | Date/time handling |
| `csv` | CSV migration from legacy format |
| `dirs` | Home directory resolution |
| `serde` / `serde_json` | Serialization |

### Tauri Capabilities

```json
{
  "permissions": [
    "core:default",
    "core:window:allow-set-always-on-top",
    "opener:default",
    "dialog:default"
  ]
}
```

### Rust Backend Commands

| Command | Parameters | Returns | Description |
|---------|-----------|---------|-------------|
| `start_task` | `name: string` | `Entry` | Stop current (if any), start new |
| `stop_task` | — | `Entry \| null` | Stop currently running task |
| `get_active_task` | — | `Entry \| null` | Return currently running entry |
| `get_recent_tasks` | — | `string[]` | Last 10 unique task names |
| `search_tasks` | `query: string` | `string[]` | Up to 10 matching task names for autocomplete |
| `get_weekly_totals` | — | `WeeklyTotal[]` | Total seconds per task for current calendar week |
| `get_daily_tasks` | — | `DailyTask[]` | Last 7 days of tasks grouped by day+task with daily totals |
| `check_auto_stop` | — | `Entry \| null` | Auto-stop task if started before cutoff |
| `get_entries_page` | `offset: number, limit: number` | `EntryWithId[]` | Paginated entries, newest first |
| `update_entry` | `id: number, task: string, start: string, end_time: string` | `EntryWithId` | Update an existing entry |
| `delete_entry` | `id: number` | `()` | Delete an entry by ID |
| `get_settings` | — | `AppSettings` | Current app settings |
| `save_settings` | `settings: AppSettings` | — | Persist settings |
| `export_csv` | `path: string` | — | Write all entries to file at path |
| `save_current_window_position` | — (reads from window) | — | Persist window geometry |
| `restore_window_position` | — | — | Apply saved window geometry |
| `update_tray` | — | — | Rebuild system tray menu to reflect current state |

### Data Objects

```typescript
interface Entry {
  task: string;
  start: string;  // "YYYY-MM-DDTHH:MM:SS"
  end: string;    // "YYYY-MM-DDTHH:MM:SS" or ""
}

interface EntryWithId {
  id: number;
  task: string;
  start: string;  // "YYYY-MM-DDTHH:MM:SS"
  end: string;    // "YYYY-MM-DDTHH:MM:SS" or ""
}

interface WeeklyTotal {
  task: string;
  seconds: number;
}

interface DailyTask {
  date: string;    // "YYYY-MM-DD"
  task: string;
  seconds: number;
}

interface AppSettings {
  end_of_day: string;    // "HH:MM"
  font_size: number;     // 10–18
  always_on_top: boolean;
}

interface WindowPosition {
  x: number;
  y: number;
  width: number;
  height: number;
}
```

### Inter-Window Communication

| Event | Emitted by | Listened by | Payload | Purpose |
|-------|-----------|-------------|---------|---------|
| `settings-changed` | Preferences window | Main window | `{ font_size, always_on_top }` | Apply settings without restart |
| `tray-task-changed` | Rust tray handler | Main window | — | Refresh UI after tray action |
| `export-csv` | Rust menu handler | Main window | — | Trigger CSV export dialog |

### Window Configuration (tauri.conf.json)

- `width: 320`
- `height: 320`
- `resizable: true`
- `minWidth: 280`
- `minHeight: 200`
- `decorations: true` (platform-native)
- `withGlobalTauri: true`
- `alwaysOnTop`: not set in config (controlled by settings at runtime)

## Future Enhancements (Out of Scope for v1)

- Keyboard shortcut to focus the window (global hotkey)
- Tags/categories for tasks
- Dark/light theme toggle
- Idle detection (pause tracking when computer is inactive)
- Multiple export formats (JSON, XLSX)
- Date range filter for export
- Statistics dashboard
