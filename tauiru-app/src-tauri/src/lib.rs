use chrono::{Datelike, Local, NaiveTime};
use csv::ReaderBuilder;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub task: String,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub auto_stop_times: Vec<String>, // Vec of HH:MM strings
    pub font_size: u32,               // px, 10–18
    pub always_on_top: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeeklyTotal {
    pub task: String,
    pub seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyTask {
    pub date: String,  // YYYY-MM-DD
    pub task: String,
    pub seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntryWithId {
    pub id: i64,
    pub task: String,
    pub start: String,
    pub end: String,
}

pub struct AppState {
    db: Connection,
}

impl AppState {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("Could not find home directory");
        let dir = home.join(".timetracked");
        fs::create_dir_all(&dir).expect("Could not create .timetracked directory");

        let db_path = dir.join("timetracked.db");
        let is_new_db = !db_path.exists();
        let db = Connection::open(&db_path).expect("Could not open SQLite database");

        // Create tables
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task TEXT NOT NULL,
                start TEXT NOT NULL,
                end_time TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_entries_start ON entries(start);
            CREATE INDEX IF NOT EXISTS idx_entries_task ON entries(task);
            CREATE INDEX IF NOT EXISTS idx_entries_end ON entries(end_time);
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .expect("Could not create database schema");

        let state = AppState { db };

        // Migrate from CSV if DB is new and CSV exists
        if is_new_db {
            let csv_path = dir.join("timetracked.csv");
            if csv_path.exists() {
                state.migrate_from_csv(&csv_path);
            }
        }

        state
    }

    fn migrate_from_csv(&self, csv_path: &PathBuf) {
        let rdr = ReaderBuilder::new().has_headers(true).from_path(csv_path);
        if let Ok(mut rdr) = rdr {
            for result in rdr.records() {
                if let Ok(record) = result {
                    if record.len() >= 3 {
                        let task = &record[0];
                        let start = &record[1];
                        let end = &record[2];
                        let _ = self.db.execute(
                            "INSERT INTO entries (task, start, end_time) VALUES (?1, ?2, ?3)",
                            params![task, start, end],
                        );
                    }
                }
            }
        }
    }

    fn get_auto_stop_times(&self) -> Vec<NaiveTime> {
        // Try new key first, fall back to legacy end_of_day
        let result: Option<String> = self
            .db
            .query_row(
                "SELECT value FROM settings WHERE key = 'auto_stop_times'",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(times_str) = result {
            let times: Vec<NaiveTime> = times_str
                .split(',')
                .filter_map(|s| NaiveTime::parse_from_str(s.trim(), "%H:%M").ok())
                .collect();
            if !times.is_empty() {
                return times;
            }
        }

        // Legacy fallback
        let legacy: Option<String> = self
            .db
            .query_row(
                "SELECT value FROM settings WHERE key = 'end_of_day'",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(time_str) = legacy {
            if let Ok(t) = NaiveTime::parse_from_str(&time_str, "%H:%M") {
                return vec![t];
            }
        }

        vec![NaiveTime::from_hms_opt(17, 0, 0).unwrap()]
    }
}

// --- Task Commands ---

#[tauri::command]
fn start_task(name: String, state: State<Mutex<AppState>>) -> Result<Entry, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    state
        .db
        .execute(
            "UPDATE entries SET end_time = ?1 WHERE end_time = ''",
            params![&now],
        )
        .map_err(|e| e.to_string())?;

    state
        .db
        .execute(
            "INSERT INTO entries (task, start, end_time) VALUES (?1, ?2, '')",
            params![name.trim(), &now],
        )
        .map_err(|e| e.to_string())?;

    Ok(Entry {
        task: name.trim().to_string(),
        start: now,
        end: String::new(),
    })
}

#[tauri::command]
fn stop_task(state: State<Mutex<AppState>>) -> Result<Option<Entry>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let active: Option<Entry> = state
        .db
        .query_row(
            "SELECT task, start, end_time FROM entries WHERE end_time = '' ORDER BY id DESC LIMIT 1",
            [],
            |row| {
                Ok(Entry {
                    task: row.get(0)?,
                    start: row.get(1)?,
                    end: row.get(2)?,
                })
            },
        )
        .ok();

    if let Some(entry) = active {
        state
            .db
            .execute(
                "UPDATE entries SET end_time = ?1 WHERE end_time = ''",
                params![&now],
            )
            .map_err(|e| e.to_string())?;

        return Ok(Some(Entry {
            task: entry.task,
            start: entry.start,
            end: now,
        }));
    }

    Ok(None)
}

#[tauri::command]
fn get_active_task(state: State<Mutex<AppState>>) -> Result<Option<Entry>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;

    let result = state
        .db
        .query_row(
            "SELECT task, start, end_time FROM entries WHERE end_time = '' ORDER BY id DESC LIMIT 1",
            [],
            |row| {
                Ok(Entry {
                    task: row.get(0)?,
                    start: row.get(1)?,
                    end: row.get(2)?,
                })
            },
        )
        .ok();

    Ok(result)
}

#[tauri::command]
fn get_recent_tasks(state: State<Mutex<AppState>>) -> Result<Vec<String>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;

    let mut stmt = state
        .db
        .prepare(
            "SELECT DISTINCT task FROM (
                SELECT task, MAX(id) as max_id FROM entries GROUP BY task
            ) ORDER BY max_id DESC LIMIT 10",
        )
        .map_err(|e| e.to_string())?;

    let tasks: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(tasks)
}

#[tauri::command]
fn search_tasks(query: String, state: State<Mutex<AppState>>) -> Result<Vec<String>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let pattern = format!("%{}%", query);

    let mut stmt = state
        .db
        .prepare(
            "SELECT DISTINCT task FROM (
                SELECT task, MAX(id) as max_id FROM entries WHERE task LIKE ?1 GROUP BY task
            ) ORDER BY max_id DESC LIMIT 10",
        )
        .map_err(|e| e.to_string())?;

    let tasks: Vec<String> = stmt
        .query_map(params![&pattern], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(tasks)
}

#[tauri::command]
fn get_weekly_totals(state: State<Mutex<AppState>>) -> Result<Vec<WeeklyTotal>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let now = Local::now().naive_local();

    let days_since_monday = now.weekday().num_days_from_monday();
    let week_start = now.date() - chrono::Duration::days(days_since_monday as i64);
    let week_start_str = week_start
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
    let now_str = now.format("%Y-%m-%dT%H:%M:%S").to_string();

    let mut stmt = state
        .db
        .prepare(
            "SELECT task,
                SUM(
                    CAST(
                        (julianday(CASE WHEN end_time = '' THEN ?1 ELSE end_time END)
                         - julianday(start)) * 86400
                    AS INTEGER)
                ) as total_seconds
            FROM entries
            WHERE start >= ?2
            GROUP BY task",
        )
        .map_err(|e| e.to_string())?;

    let totals: Vec<WeeklyTotal> = stmt
        .query_map(params![&now_str, &week_start_str], |row| {
            Ok(WeeklyTotal {
                task: row.get(0)?,
                seconds: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(totals)
}

#[tauri::command]
fn get_daily_tasks(state: State<Mutex<AppState>>) -> Result<Vec<DailyTask>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let now = Local::now().naive_local();
    let week_ago = (now.date() - chrono::Duration::days(6))
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
    let now_str = now.format("%Y-%m-%dT%H:%M:%S").to_string();

    let mut stmt = state
        .db
        .prepare(
            "SELECT date(start) as day, task,
                SUM(
                    CAST(
                        (julianday(CASE WHEN end_time = '' THEN ?1 ELSE end_time END)
                         - julianday(start)) * 86400
                    AS INTEGER)
                ) as total_seconds
            FROM entries
            WHERE start >= ?2
            GROUP BY day, task
            ORDER BY day DESC, total_seconds DESC",
        )
        .map_err(|e| e.to_string())?;

    let tasks: Vec<DailyTask> = stmt
        .query_map(params![&now_str, &week_ago], |row| {
            Ok(DailyTask {
                date: row.get(0)?,
                task: row.get(1)?,
                seconds: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(tasks)
}

#[tauri::command]
fn check_auto_stop(state: State<Mutex<AppState>>) -> Result<Option<Entry>, String> {
    let now = Local::now();
    let state = state.lock().map_err(|e| e.to_string())?;
    let times = state.get_auto_stop_times();

    // Find the latest auto-stop time that has already passed today
    let cutoff = times.iter().filter(|&&t| now.time() >= t).max().copied();

    if let Some(cutoff) = cutoff {
        let today_cutoff = now
            .date_naive()
            .and_time(cutoff)
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        let today_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();

        let active: Option<Entry> = state
            .db
            .query_row(
                "SELECT task, start, end_time FROM entries
                 WHERE end_time = '' AND start >= ?1 AND start < ?2
                 ORDER BY id DESC LIMIT 1",
                params![&today_start, &today_cutoff],
                |row| {
                    Ok(Entry {
                        task: row.get(0)?,
                        start: row.get(1)?,
                        end: row.get(2)?,
                    })
                },
            )
            .ok();

        if let Some(entry) = active {
            state
                .db
                .execute(
                    "UPDATE entries SET end_time = ?1 WHERE end_time = '' AND start >= ?2 AND start < ?3",
                    params![&today_cutoff, &today_start, &today_cutoff],
                )
                .map_err(|e| e.to_string())?;

            return Ok(Some(Entry {
                task: entry.task,
                start: entry.start,
                end: today_cutoff,
            }));
        }
    }
    Ok(None)
}

// --- Settings Commands ---

#[tauri::command]
fn get_settings(state: State<Mutex<AppState>>) -> Result<AppSettings, String> {
    let state = state.lock().map_err(|e| e.to_string())?;

    // Try new key first, fall back to legacy
    let auto_stop_times: Vec<String> = state
        .db
        .query_row(
            "SELECT value FROM settings WHERE key = 'auto_stop_times'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
        .unwrap_or_else(|| {
            // Legacy fallback
            let legacy = state
                .db
                .query_row(
                    "SELECT value FROM settings WHERE key = 'end_of_day'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap_or_else(|_| "17:00".to_string());
            vec![legacy]
        });

    let font_size: u32 = state
        .db
        .query_row(
            "SELECT value FROM settings WHERE key = 'font_size'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12);

    let always_on_top: bool = state
        .db
        .query_row(
            "SELECT value FROM settings WHERE key = 'always_on_top'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|v| v == "true")
        .unwrap_or(true);

    Ok(AppSettings { auto_stop_times, font_size, always_on_top })
}

#[tauri::command]
fn save_settings(settings: AppSettings, state: State<Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let times_str = settings.auto_stop_times.join(",");
    state
        .db
        .execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('auto_stop_times', ?1)",
            params![&times_str],
        )
        .map_err(|e| e.to_string())?;
    state
        .db
        .execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('font_size', ?1)",
            params![settings.font_size.to_string()],
        )
        .map_err(|e| e.to_string())?;
    state
        .db
        .execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('always_on_top', ?1)",
            params![if settings.always_on_top { "true" } else { "false" }],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

// --- CSV Export Command ---

#[tauri::command]
fn export_csv(path: String, state: State<Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;

    let mut stmt = state
        .db
        .prepare("SELECT task, start, end_time FROM entries ORDER BY id ASC")
        .map_err(|e| e.to_string())?;

    let mut csv_content = String::from("task,start,end\n");

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        if let Ok((task, start, end)) = row {
            let quoted_task = if task.contains(',') || task.contains('"') {
                format!("\"{}\"", task.replace('"', "\"\""))
            } else {
                task
            };
            csv_content.push_str(&format!("{},{},{}\n", quoted_task, start, end));
        }
    }

    fs::write(&path, csv_content).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}

// --- Entry Editor Commands ---

#[tauri::command]
fn get_entries_page(offset: i64, limit: i64, state: State<Mutex<AppState>>) -> Result<Vec<EntryWithId>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;

    let mut stmt = state
        .db
        .prepare(
            "SELECT id, task, start, end_time FROM entries ORDER BY id DESC LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;

    let entries: Vec<EntryWithId> = stmt
        .query_map(params![limit, offset], |row| {
            Ok(EntryWithId {
                id: row.get(0)?,
                task: row.get(1)?,
                start: row.get(2)?,
                end: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(entries)
}

#[tauri::command]
fn update_entry(id: i64, task: String, start: String, end_time: String, state: State<Mutex<AppState>>) -> Result<EntryWithId, String> {
    let task = task.trim().to_string();
    if task.is_empty() {
        return Err("Task name cannot be empty".to_string());
    }
    if start.is_empty() {
        return Err("Start time cannot be empty".to_string());
    }
    // If end_time is non-empty, validate start < end
    if !end_time.is_empty() && end_time <= start {
        return Err("End time must be after start time".to_string());
    }

    let state = state.lock().map_err(|e| e.to_string())?;
    let affected = state
        .db
        .execute(
            "UPDATE entries SET task = ?1, start = ?2, end_time = ?3 WHERE id = ?4",
            params![&task, &start, &end_time, id],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err("Entry not found".to_string());
    }

    Ok(EntryWithId { id, task, start, end: end_time })
}

#[tauri::command]
fn delete_entry(id: i64, state: State<Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let affected = state
        .db
        .execute("DELETE FROM entries WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err("Entry not found".to_string());
    }

    Ok(())
}

// --- Window Position Commands ---

#[derive(Debug, Clone, Serialize)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[tauri::command]
fn save_current_window_position(
    window: tauri::Window,
    state: State<Mutex<AppState>>,
) -> Result<(), String> {
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let state = state.lock().map_err(|e| e.to_string())?;
    let value = format!("{},{},{},{}", pos.x, pos.y, size.width, size.height);
    state
        .db
        .execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('window_position', ?1)",
            params![&value],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn restore_window_position(
    window: tauri::Window,
    state: State<Mutex<AppState>>,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let result: Option<String> = state
        .db
        .query_row(
            "SELECT value FROM settings WHERE key = 'window_position'",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(value) = result {
        let parts: Vec<&str> = value.split(',').collect();
        if parts.len() == 4 {
            if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
                parts[0].parse::<i32>(),
                parts[1].parse::<i32>(),
                parts[2].parse::<u32>(),
                parts[3].parse::<u32>(),
            ) {
                use tauri::{PhysicalPosition, PhysicalSize};
                let _ = window.set_position(PhysicalPosition::new(x, y));
                let _ = window.set_size(PhysicalSize::new(w, h));
            }
        }
    }
    Ok(())
}

// --- App Setup ---

fn build_tray_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let state_mutex = app.state::<Mutex<AppState>>();
    let state = state_mutex.lock().unwrap();

    // Get active task
    let active: Option<String> = state
        .db
        .query_row(
            "SELECT task FROM entries WHERE end_time = '' ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    // Get recent tasks (up to 5 for the tray)
    let recent: Vec<String> = {
        let mut stmt = state
            .db
            .prepare(
                "SELECT DISTINCT task FROM (
                    SELECT task, MAX(id) as max_id FROM entries GROUP BY task
                ) ORDER BY max_id DESC LIMIT 5",
            )
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };

    drop(state);

    // Build menu
    let mut builder = MenuBuilder::new(app);

    // Current task status
    if let Some(ref task_name) = active {
        let status = MenuItemBuilder::with_id("tray_status", format!("⏱ {}", task_name))
            .enabled(false)
            .build(app)?;
        builder = builder.item(&status);

        let stop = MenuItemBuilder::with_id("tray_stop", "■ Stop")
            .build(app)?;
        builder = builder.item(&stop);
        builder = builder.separator();
    } else {
        let status = MenuItemBuilder::with_id("tray_status", "No task running")
            .enabled(false)
            .build(app)?;
        builder = builder.item(&status);
        builder = builder.separator();
    }

    // Recent tasks to start
    for (i, name) in recent.iter().enumerate() {
        // Skip the currently active task
        if active.as_deref() == Some(name.as_str()) {
            continue;
        }
        let item = MenuItemBuilder::with_id(format!("tray_start_{}", i), format!("▶ {}", name))
            .build(app)?;
        builder = builder.item(&item);
    }

    builder = builder.separator();

    let prefs = MenuItemBuilder::with_id("tray_preferences", "Preferences...")
        .build(app)?;
    builder = builder.item(&prefs);

    let entries = MenuItemBuilder::with_id("tray_edit_entries", "Edit Entries...")
        .build(app)?;
    builder = builder.item(&entries);

    let show = MenuItemBuilder::with_id("tray_show", "Show Window")
        .build(app)?;
    builder = builder.item(&show);

    builder = builder.separator();

    let quit = MenuItemBuilder::with_id("tray_quit", "Quit")
        .build(app)?;
    builder = builder.item(&quit);

    builder.build()
}

fn refresh_tray_menu(app: &tauri::AppHandle) {
    if let Some(tray) = app.tray_by_id("main_tray") {
        if let Ok(menu) = build_tray_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

#[tauri::command]
fn update_tray(app: tauri::AppHandle) -> Result<(), String> {
    refresh_tray_menu(&app);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(AppState::new()))
        .setup(|app| {
            // Build native menu
            let preferences = MenuItemBuilder::with_id("preferences", "Preferences...")
                .accelerator("CmdOrCtrl+,")
                .build(app)?;
            let edit_entries = MenuItemBuilder::with_id("edit_entries", "Edit Entries...")
                .accelerator("CmdOrCtrl+Shift+E")
                .build(app)?;
            let export = MenuItemBuilder::with_id("export_csv", "Export CSV...")
                .accelerator("CmdOrCtrl+E")
                .build(app)?;

            let file_menu = SubmenuBuilder::new(app, "File")
                .item(&preferences)
                .item(&edit_entries)
                .item(&export)
                .separator()
                .quit()
                .build()?;

            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            let menu = MenuBuilder::new(app)
                .item(&file_menu)
                .item(&edit_menu)
                .build()?;

            app.set_menu(menu)?;

            // Build system tray
            let tray_menu = build_tray_menu(app.handle())?;
            let tray_icon = include_bytes!("../icons/tray-icon.png");

            let _tray = TrayIconBuilder::with_id("main_tray")
                .icon(tauri::image::Image::from_bytes(tray_icon)?)
                .icon_as_template(true)
                .menu(&tray_menu)
                .on_menu_event(|app_handle, event| {
                    let id = event.id().as_ref();
                    match id {
                        "tray_stop" => {
                            let state_mutex = app_handle.state::<Mutex<AppState>>();
                            let state = state_mutex.lock().unwrap();
                            let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
                            let _ = state.db.execute(
                                "UPDATE entries SET end_time = ?1 WHERE end_time = ''",
                                params![&now],
                            );
                            drop(state);
                            refresh_tray_menu(app_handle);
                            // Notify frontend
                            if let Some(win) = app_handle.get_webview_window("main") {
                                let _ = win.emit("tray-task-changed", ());
                            }
                        }
                        "tray_preferences" => {
                            let existing = app_handle.get_webview_window("preferences");
                            if let Some(win) = existing {
                                let _ = win.set_focus();
                                return;
                            }
                            let _ = tauri::WebviewWindowBuilder::new(
                                app_handle,
                                "preferences",
                                tauri::WebviewUrl::App("preferences.html".into()),
                            )
                            .title("Preferences")
                            .inner_size(420.0, 380.0)
                            .resizable(true)
                            .build();
                        }
                        "tray_edit_entries" => {
                            let existing = app_handle.get_webview_window("entries");
                            if let Some(win) = existing {
                                let _ = win.set_focus();
                                return;
                            }
                            let _ = tauri::WebviewWindowBuilder::new(
                                app_handle,
                                "entries",
                                tauri::WebviewUrl::App("entries.html".into()),
                            )
                            .title("Edit Entries")
                            .inner_size(600.0, 500.0)
                            .resizable(true)
                            .min_inner_size(500.0, 300.0)
                            .build();
                        }
                        "tray_show" => {
                            if let Some(win) = app_handle.get_webview_window("main") {
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                        "tray_quit" => {
                            app_handle.exit(0);
                        }
                        _ if id.starts_with("tray_start_") => {
                            // Parse index and start that task
                            if let Some(idx_str) = id.strip_prefix("tray_start_") {
                                if let Ok(idx) = idx_str.parse::<usize>() {
                                    let state_mutex = app_handle.state::<Mutex<AppState>>();
                                    let state = state_mutex.lock().unwrap();

                                    // Get recent tasks to find the name
                                    let recent: Vec<String> = {
                                        let mut stmt = state.db.prepare(
                                            "SELECT DISTINCT task FROM (
                                                SELECT task, MAX(id) as max_id FROM entries GROUP BY task
                                            ) ORDER BY max_id DESC LIMIT 5"
                                        ).unwrap();
                                        stmt.query_map([], |row| row.get(0))
                                            .unwrap()
                                            .filter_map(|r| r.ok())
                                            .collect()
                                    };

                                    // Get active to know which ones were skipped
                                    let active: Option<String> = state.db.query_row(
                                        "SELECT task FROM entries WHERE end_time = '' ORDER BY id DESC LIMIT 1",
                                        [],
                                        |row| row.get(0),
                                    ).ok();

                                    let filtered: Vec<&String> = recent.iter()
                                        .filter(|n| active.as_deref() != Some(n.as_str()))
                                        .collect();

                                    if let Some(task_name) = filtered.get(idx) {
                                        let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
                                        // Stop current
                                        let _ = state.db.execute(
                                            "UPDATE entries SET end_time = ?1 WHERE end_time = ''",
                                            params![&now],
                                        );
                                        // Start new
                                        let _ = state.db.execute(
                                            "INSERT INTO entries (task, start, end_time) VALUES (?1, ?2, '')",
                                            params![task_name.as_str(), &now],
                                        );
                                    }
                                    drop(state);
                                    refresh_tray_menu(app_handle);
                                    if let Some(win) = app_handle.get_webview_window("main") {
                                        let _ = win.emit("tray-task-changed", ());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // Handle menu events
            app.on_menu_event(move |app_handle, event| {
                match event.id().as_ref() {
                    "preferences" => {
                        // Open preferences window
                        let existing = app_handle.get_webview_window("preferences");
                        if let Some(win) = existing {
                            let _ = win.set_focus();
                            return;
                        }
                        let _prefs_window = tauri::WebviewWindowBuilder::new(
                            app_handle,
                            "preferences",
                            tauri::WebviewUrl::App("preferences.html".into()),
                        )
                        .title("Preferences")
                        .inner_size(420.0, 380.0)
                        .resizable(true)
                        .build()
                        .expect("Failed to create preferences window");
                    }
                    "edit_entries" => {
                        // Open entries editor window
                        let existing = app_handle.get_webview_window("entries");
                        if let Some(win) = existing {
                            let _ = win.set_focus();
                            return;
                        }
                        let _ = tauri::WebviewWindowBuilder::new(
                            app_handle,
                            "entries",
                            tauri::WebviewUrl::App("entries.html".into()),
                        )
                        .title("Edit Entries")
                        .inner_size(600.0, 500.0)
                        .resizable(true)
                        .min_inner_size(500.0, 300.0)
                        .build();
                    }
                    "export_csv" => {
                        // Trigger CSV export via the main window
                        if let Some(win) = app_handle.get_webview_window("main") {
                            let _ = win.emit("export-csv", ());
                        }
                    }
                    _ => {}
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_task,
            stop_task,
            get_active_task,
            get_recent_tasks,
            search_tasks,
            get_weekly_totals,
            get_daily_tasks,
            check_auto_stop,
            get_entries_page,
            update_entry,
            delete_entry,
            save_current_window_position,
            restore_window_position,
            get_settings,
            save_settings,
            export_csv,
            update_tray,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
