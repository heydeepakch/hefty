# Storage Cleanup Helper

A read-only Windows desktop app (built with Tauri + Rust) for finding what's
filling up your disk.

It recursively scans a path and reports:

- total scanned file size
- largest directories
- largest files
- likely cleanup candidates such as temp, cache, log, dump, and backup-like files
- directories or files that could not be accessed

It does not delete anything.

## Project layout

```
.
├── scanner/   # library: scan logic + types
├── cli/       # CLI binary (storage-cleanup-cli)
├── app/       # Tauri 2 desktop app (storage-cleanup-helper)
│   ├── src/   # Rust backend (Tauri commands)
│   ├── ui/    # vanilla HTML/CSS/JS frontend
│   ├── icons/
│   ├── capabilities/
│   ├── tauri.conf.json
│   └── Cargo.toml
└── scripts/   # helper scripts (icon generation)
```

## Prerequisites

- Rust 1.85+ (`rustup install stable`)
- Microsoft C++ Build Tools (installed alongside `rustup` on Windows)
- WebView2 runtime (preinstalled on Windows 11; bundled by the installer for Win10)
- For producing installers: `cargo install tauri-cli --version "^2.0" --locked`

## Run the desktop app in dev mode

```powershell
cd app
cargo tauri dev
```

The first run downloads/compiles ~500 Tauri-related crates and takes 10–15
minutes. Subsequent runs are fast.

### Live-editing the UI

`cargo tauri dev` automatically starts a tiny Python HTTP server on
`http://localhost:1420` serving `app/ui/`, and the Tauri window loads from
there.

Workflow:

1. Run `cargo tauri dev` from `app/` (leave it running).
2. Edit any file under `app/ui/` (`index.html`, `styles.css`, `main.js`).
3. Save.
4. In the app window, press **F5** or **Ctrl+R** to reload — your changes appear
   immediately. **No Rust rebuild needed.**

For interactive CSS experimentation, press **F12** in the app window to open
Edge DevTools (debug builds only), live-edit styles in the Elements panel, then
copy the final values back to `app/ui/styles.css`.

Requires Python 3 in `PATH` (any 3.x). To use a different static server, edit
`beforeDevCommand` in `app/tauri.conf.json`.

## Build a Windows installer

```powershell
cd app
cargo tauri build
```

Output is placed under `target/release/bundle/nsis/`. Look for
`Storage Cleanup Helper_0.1.0_x64-setup.exe`. Double-click it to install;
WebView2 is auto-installed if missing.

## Use the CLI

```powershell
cargo run --release -p storage-cleanup-cli -- C:\ --top 25
cargo run --release -p storage-cleanup-cli -- "$env:LOCALAPPDATA\Temp"
```

For full-drive scans on Windows, run the terminal as Administrator if you want
fewer access-denied entries.

## Tests

```powershell
cargo test -p scanner -p storage-cleanup-cli
```

## Replace the placeholder app icon

`scripts/make-icons.ps1` produces simple solid-color placeholder icons. To use
your own:

1. Drop a square PNG (1024×1024 recommended) somewhere.
2. With `tauri-cli` installed:
   ```powershell
   cd app
   cargo tauri icon path\to\source.png
   ```

## Safety

Treat the cleanup candidates as **hints, not deletion instructions**. Some logs,
caches, dump files, or metadata files may still be useful to applications or
Windows. This tool is read-only.
