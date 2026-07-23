const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;
const { listen } = window.__TAURI__.event;

// DOM elements
const taskInput = document.getElementById("task-input");
const autocompleteEl = document.getElementById("autocomplete");
const activeTaskEl = document.getElementById("active-task");
const activeNameEl = document.getElementById("active-name");
const timerEl = document.getElementById("timer");
const stopBtn = document.getElementById("stop-btn");
const recentList = document.getElementById("recent-list");

// State
let activeTask = null;
let timerInterval = null;
let selectedSuggestion = -1;
let suggestions = [];

// --- Timer ---

function formatElapsed(startStr) {
  const start = new Date(startStr);
  const now = new Date();
  const diff = Math.floor((now - start) / 1000);
  const h = Math.floor(diff / 3600);
  const m = Math.floor((diff % 3600) / 60);
  const s = diff % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function formatSeconds(totalSecs) {
  const h = Math.floor(totalSecs / 3600);
  const m = Math.floor((totalSecs % 3600) / 60);
  return h > 0 ? `${h}h${String(m).padStart(2, "0")}m` : `${m}m`;
}

function startTimer() {
  stopTimer();
  updateTimerDisplay();
  timerInterval = setInterval(updateTimerDisplay, 1000);
}

function stopTimer() {
  if (timerInterval) {
    clearInterval(timerInterval);
    timerInterval = null;
  }
}

function updateTimerDisplay() {
  if (activeTask) {
    timerEl.textContent = formatElapsed(activeTask.start);
  }
}

// --- UI Updates ---

function renderActiveTask() {
  if (activeTask) {
    activeTaskEl.classList.remove("idle");
    activeNameEl.textContent = activeTask.task;
    stopBtn.style.display = "inline-block";
    startTimer();
  } else {
    activeTaskEl.classList.add("idle");
    activeNameEl.textContent = "No task running";
    timerEl.textContent = "";
    stopBtn.style.display = "none";
    stopTimer();
  }
}

async function renderRecentTasks() {
  try {
    const dailyTasks = await invoke("get_daily_tasks");
    recentList.innerHTML = "";

    // Group by date
    const grouped = {};
    for (const item of dailyTasks) {
      if (!grouped[item.date]) grouped[item.date] = [];
      grouped[item.date].push(item);
    }

    for (const date of Object.keys(grouped)) {
      // Day header
      const header = document.createElement("div");
      header.className = "day-header";
      const d = new Date(date + "T00:00:00");
      const label = d.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" });
      const dayTotal = grouped[date].reduce((sum, t) => sum + t.seconds, 0);
      const labelSpan = document.createElement("span");
      labelSpan.textContent = label;
      const totalSpan = document.createElement("span");
      totalSpan.className = "day-total";
      totalSpan.textContent = formatSeconds(dayTotal);
      header.appendChild(labelSpan);
      header.appendChild(totalSpan);
      recentList.appendChild(header);

      // Tasks for this day
      for (const task of grouped[date]) {
        const item = document.createElement("div");
        item.className = "recent-item";

        const nameSpan = document.createElement("span");
        nameSpan.className = "name";
        nameSpan.textContent = task.task;
        nameSpan.title = task.task;

        const totalSpan = document.createElement("span");
        totalSpan.className = "weekly-total";
        totalSpan.textContent = formatSeconds(task.seconds);

        const btn = document.createElement("button");
        btn.className = "start-btn";
        btn.textContent = "▶";
        btn.addEventListener("click", () => startTask(task.task));

        item.appendChild(nameSpan);
        item.appendChild(totalSpan);
        item.appendChild(btn);
        recentList.appendChild(item);
      }
    }
  } catch (e) {
    console.error("Failed to load recent tasks:", e);
  }
}

// --- Task Actions ---

async function startTask(name) {
  if (!name || !name.trim()) return;
  try {
    const entry = await invoke("start_task", { name: name.trim() });
    activeTask = entry;
    renderActiveTask();
    renderRecentTasks();
    taskInput.value = "";
    hideAutocomplete();
    await invoke("update_tray");
  } catch (e) {
    console.error("Failed to start task:", e);
  }
}

async function stopTask() {
  try {
    await invoke("stop_task");
    activeTask = null;
    renderActiveTask();
    renderRecentTasks();
    await invoke("update_tray");
  } catch (e) {
    console.error("Failed to stop task:", e);
  }
}

// --- Autocomplete ---

async function showAutocomplete(query) {
  if (!query || query.length < 1) {
    hideAutocomplete();
    return;
  }
  try {
    suggestions = await invoke("search_tasks", { query });
    if (suggestions.length === 0) {
      hideAutocomplete();
      return;
    }

    selectedSuggestion = -1;
    autocompleteEl.innerHTML = "";
    for (let i = 0; i < suggestions.length; i++) {
      const div = document.createElement("div");
      div.className = "suggestion";
      div.textContent = suggestions[i];
      div.addEventListener("mousedown", (e) => {
        e.preventDefault();
        taskInput.value = suggestions[i];
        hideAutocomplete();
        taskInput.focus();
      });
      autocompleteEl.appendChild(div);
    }
    autocompleteEl.style.display = "block";
  } catch (e) {
    hideAutocomplete();
  }
}

function hideAutocomplete() {
  autocompleteEl.style.display = "none";
  suggestions = [];
  selectedSuggestion = -1;
}

function navigateSuggestions(direction) {
  const items = autocompleteEl.querySelectorAll(".suggestion");
  if (items.length === 0) return;

  selectedSuggestion += direction;
  if (selectedSuggestion < 0) selectedSuggestion = items.length - 1;
  if (selectedSuggestion >= items.length) selectedSuggestion = 0;

  items.forEach((item, i) => {
    item.classList.toggle("selected", i === selectedSuggestion);
  });

  if (selectedSuggestion >= 0) {
    taskInput.value = suggestions[selectedSuggestion];
  }
}

// --- Event Listeners ---

taskInput.addEventListener("input", (e) => {
  showAutocomplete(e.target.value);
});

taskInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    hideAutocomplete();
    startTask(taskInput.value);
  } else if (e.key === "ArrowDown") {
    e.preventDefault();
    navigateSuggestions(1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    navigateSuggestions(-1);
  } else if (e.key === "Escape") {
    hideAutocomplete();
    taskInput.blur();
  }
});

taskInput.addEventListener("blur", () => {
  setTimeout(hideAutocomplete, 150);
});

stopBtn.addEventListener("click", stopTask);

// --- Auto-stop check ---

async function checkAutoStop() {
  try {
    const stopped = await invoke("check_auto_stop");
    if (stopped) {
      activeTask = null;
      renderActiveTask();
      renderRecentTasks();
    }
  } catch (e) {
    console.error("Auto-stop check failed:", e);
  }
}

// --- Window Position ---

let savePositionTimeout = null;

async function saveWindowPosition() {
  try {
    await invoke("save_current_window_position");
  } catch (e) {
    // Silently ignore position save errors
  }
}

function debouncedSavePosition() {
  if (savePositionTimeout) clearTimeout(savePositionTimeout);
  savePositionTimeout = setTimeout(saveWindowPosition, 500);
}

async function restoreWindowPosition() {
  try {
    await invoke("restore_window_position");
  } catch (e) {
    // Silently ignore restore errors
  }
}

// --- Init ---

async function init() {
  try {
    // Restore window position first
    await restoreWindowPosition();

    // Apply font size and always-on-top from settings
    try {
      const settings = await invoke("get_settings");
      if (settings.font_size) {
        document.body.style.fontSize = settings.font_size + "px";
      }
      const win = getCurrentWindow();
      await win.setAlwaysOnTop(settings.always_on_top);
    } catch (e) {
      // Use defaults
    }

    activeTask = await invoke("get_active_task");
    renderActiveTask();
    renderRecentTasks();

    // Check auto-stop every 30 seconds
    setInterval(checkAutoStop, 30000);

    // Listen for window move/resize to save position
    const win = getCurrentWindow();
    await win.onMoved(debouncedSavePosition);
    await win.onResized(debouncedSavePosition);

    // Listen for menu-triggered CSV export
    await listen("export-csv", async () => {
      try {
        const { save } = window.__TAURI__.dialog;
        const filePath = await save({
          defaultPath: `timetracked-${new Date().toISOString().split("T")[0]}.csv`,
          filters: [{ name: "CSV", extensions: ["csv"] }],
        });
        if (filePath) {
          await invoke("export_csv", { path: filePath });
        }
      } catch (e) {
        console.error("Export failed:", e);
      }
    });

    // Listen for settings changes from preferences window
    await listen("settings-changed", async (event) => {
      if (event.payload) {
        if (event.payload.font_size) {
          document.body.style.fontSize = event.payload.font_size + "px";
        }
        if (typeof event.payload.always_on_top === "boolean") {
          const win = getCurrentWindow();
          await win.setAlwaysOnTop(event.payload.always_on_top);
        }
      }
    });

    // Listen for tray-triggered task changes
    await listen("tray-task-changed", async () => {
      activeTask = await invoke("get_active_task");
      renderActiveTask();
      renderRecentTasks();
    });
  } catch (e) {
    console.error("Init failed:", e);
  }
}

init();
