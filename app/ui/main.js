const { invoke } = window.__TAURI__.core;

async function openDirectoryDialog() {
  return invoke("plugin:dialog|open", {
    options: {
      directory: true,
      multiple: false,
      title: "Choose a folder to scan",
    },
  });
}

const $ = (id) => document.getElementById(id);

const pathInput = $("path-input");
const browseBtn = $("browse-btn");
const scanBtn = $("scan-btn");
const topInput = $("top-input");
const statusEl = $("status");
const statusText = $("status-text");
const statusDetail = $("status-detail");
const resultsEl = $("results");

let scanning = false;
let stopping = false;

function setStatus(kind, text, detail = "") {
  statusEl.classList.remove("status-idle", "status-busy", "status-good", "status-error");
  statusEl.classList.add(`status-${kind}`);
  statusText.textContent = text;
  statusDetail.textContent = detail;
}

setStatus("idle", "Ready", "Pick a path and click Scan");

document.querySelectorAll(".chip").forEach((chip) => {
  chip.addEventListener("click", () => {
    pathInput.value = chip.dataset.path;
    pathInput.focus();
  });
});

browseBtn.addEventListener("click", async () => {
  try {
    const selected = await openDirectoryDialog();
    if (selected && typeof selected === "string") {
      pathInput.value = selected;
    }
  } catch (err) {
    console.error("Browse failed:", err);
    setStatus("error", "Browse failed", String(err));
  }
});

scanBtn.addEventListener("click", () => {
  if (scanning) {
    stopScan();
  } else {
    runScan();
  }
});
pathInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") runScan();
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && scanning) {
    e.preventDefault();
    stopScan();
    return;
  }

  if (e.key === "F5" || (e.ctrlKey && (e.key === "r" || e.key === "R"))) {
    e.preventDefault();
    location.reload();
  }
});

async function runScan() {
  if (scanning) return;

  const rawPath = pathInput.value.trim();
  if (!rawPath) {
    setStatus("error", "Path required", "Enter a path or click Browse");
    pathInput.focus();
    return;
  }

  scanning = true;
  stopping = false;
  setScanButtonMode("stop");
  browseBtn.disabled = true;
  pathInput.disabled = true;
  setStatus("busy", "Scanning…", `${rawPath} — large drives can take a few minutes`);
  resultsEl.classList.add("hidden");

  const startedAt = performance.now();
  try {
    const response = await invoke("scan", { path: rawPath });
    const elapsedMs = performance.now() - startedAt;
    renderResults(response, elapsedMs);
    if (response.report.cancelled) {
      setStatus(
        "idle",
        "Scan stopped",
        `Partial results: ${response.total_size_human} across ${response.report.files_scanned.toLocaleString()} files in ${formatDuration(elapsedMs)}`,
      );
    } else {
      setStatus(
        "good",
        "Scan complete",
        `${response.total_size_human} across ${response.report.files_scanned.toLocaleString()} files in ${formatDuration(elapsedMs)}`,
      );
    }
  } catch (err) {
    console.error(err);
    setStatus("error", "Scan failed", String(err));
  } finally {
    scanning = false;
    stopping = false;
    scanBtn.disabled = false;
    setScanButtonMode("scan");
    browseBtn.disabled = false;
    pathInput.disabled = false;
  }
}

async function stopScan() {
  if (!scanning || stopping) return;

  stopping = true;
  scanBtn.disabled = true;
  setStatus("busy", "Stopping…", "Finishing the current directory read, then the scan will stop");
  try {
    await invoke("stop_scan");
  } catch (err) {
    console.error(err);
    scanBtn.disabled = false;
    stopping = false;
    setStatus("error", "Could not stop scan", String(err));
  }
}

function setScanButtonMode(mode) {
  const label = scanBtn.querySelector("span");
  const key = scanBtn.querySelector("kbd");
  scanBtn.classList.toggle("btn-stop", mode === "stop");
  if (mode === "scan") scanBtn.disabled = false;
  if (label) label.textContent = mode === "stop" ? "Stop" : "Scan";
  if (key) key.textContent = mode === "stop" ? "Esc" : "↵";
}

function formatBytes(bytes) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = Number(bytes);
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  if (unit === 0) return `${value} ${units[unit]}`;
  if (value >= 100) return `${value.toFixed(0)} ${units[unit]}`;
  if (value >= 10) return `${value.toFixed(1)} ${units[unit]}`;
  return `${value.toFixed(2)} ${units[unit]}`;
}

function formatDuration(ms) {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)}s`;
  const m = Math.floor(s / 60);
  const rem = Math.round(s - m * 60);
  return `${m}m ${rem}s`;
}

function renderResults(response, elapsedMs) {
  const report = response.report;
  const top = Math.max(1, Math.min(500, parseInt(topInput.value, 10) || 25));

  $("stat-total").textContent = response.total_size_human;
  $("stat-files").textContent = report.files_scanned.toLocaleString();
  $("stat-dirs").textContent = report.dirs_scanned.toLocaleString();
  $("stat-hidden").textContent = report.hidden_entries.toLocaleString();
  $("stat-symlinks").textContent = report.symlinks_skipped.toLocaleString();
  $("stat-errors").textContent = report.errors.length.toLocaleString();
  $("scan-root").textContent = report.root;
  $("scan-duration").textContent = formatDuration(elapsedMs);

  const dirRows = report.dirs.slice(0, top);
  const fileRows = report.files.slice(0, top);
  const candRows = report.candidates.slice(0, top);
  const errRows = report.errors.slice(0, 200);

  fillSizedTable("dirs-table", dirRows, dirRows[0]?.size || 1);
  fillSizedTable("files-table", fileRows, fileRows[0]?.size || 1);
  fillCandidatesTable("candidates-table", candRows, candRows[0]?.size || 1);
  fillErrorsTable("errors-table", errRows);

  $("dirs-meta").textContent = formatSectionCount(dirRows.length, report.dirs.length);
  $("files-meta").textContent = formatSectionCount(fileRows.length, report.files.length);
  $("candidates-meta").textContent = formatSectionCount(candRows.length, report.candidates.length);
  $("errors-meta").textContent = report.errors.length
    ? `${Math.min(200, report.errors.length).toLocaleString()} of ${report.errors.length.toLocaleString()}`
    : "none";

  resultsEl.classList.remove("hidden");
}

function formatSectionCount(shown, total) {
  if (total === 0) return "none";
  if (shown >= total) return `${total.toLocaleString()}`;
  return `${shown.toLocaleString()} of ${total.toLocaleString()}`;
}

function fillSizedTable(tableId, rows, maxSize) {
  const tbody = $(tableId).querySelector("tbody");
  tbody.innerHTML = "";
  if (!rows.length) {
    tbody.appendChild(emptyRow(2));
    return;
  }
  for (const row of rows) {
    const tr = document.createElement("tr");
    tr.appendChild(sizeCell(row.size, maxSize));
    tr.appendChild(td(row.path));
    tbody.appendChild(tr);
  }
}

function fillCandidatesTable(tableId, rows, maxSize) {
  const tbody = $(tableId).querySelector("tbody");
  tbody.innerHTML = "";
  if (!rows.length) {
    tbody.appendChild(emptyRow(3));
    return;
  }
  for (const row of rows) {
    const tr = document.createElement("tr");
    tr.appendChild(sizeCell(row.size, maxSize));
    tr.appendChild(td(row.path));
    tr.appendChild(td(row.reason, "reason"));
    tbody.appendChild(tr);
  }
}

function fillErrorsTable(tableId, rows) {
  const tbody = $(tableId).querySelector("tbody");
  tbody.innerHTML = "";
  if (!rows.length) {
    tbody.appendChild(emptyRow(2));
    return;
  }
  for (const row of rows) {
    const tr = document.createElement("tr");
    tr.appendChild(td(row.path));
    tr.appendChild(td(row.message, "reason"));
    tbody.appendChild(tr);
  }
}

function sizeCell(size, max) {
  const cell = document.createElement("td");
  cell.className = "num";

  const wrap = document.createElement("div");
  wrap.className = "size-cell";

  const bar = document.createElement("div");
  bar.className = "size-bar";

  const fill = document.createElement("div");
  const ratio = max > 0 ? Math.max(0, Math.min(1, size / max)) : 0;
  fill.className =
    "size-bar-fill" +
    (ratio >= 0.7 ? " size-bar-hi" : ratio < 0.15 ? " size-bar-low" : "");
  fill.style.width = `${(ratio * 100).toFixed(2)}%`;
  bar.appendChild(fill);

  const label = document.createElement("span");
  label.className = "size-text";
  label.textContent = formatBytes(size);

  wrap.appendChild(bar);
  wrap.appendChild(label);
  cell.appendChild(wrap);
  return cell;
}

function td(text, cls) {
  const cell = document.createElement("td");
  if (cls) cell.className = cls;
  cell.textContent = text;
  return cell;
}

function emptyRow(colspan) {
  const tr = document.createElement("tr");
  const cell = document.createElement("td");
  cell.colSpan = colspan;
  cell.className = "empty";
  cell.textContent = "No data";
  tr.appendChild(cell);
  return tr;
}
