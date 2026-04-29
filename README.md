# Storage Cleanup Helper

A read-only Rust CLI for finding what is consuming disk space.

It recursively scans a path, then reports:

- total scanned file size
- largest directories
- largest files
- likely cleanup candidates such as temp, cache, log, dump, and backup-like files
- directories or files that could not be accessed

It does not delete anything.

## Build

```powershell
cargo build --release
```

## Usage

Scan the current directory:

```powershell
cargo run --release
```

Scan `C:\` and show the top 25 results per section:

```powershell
cargo run --release -- C:\ --top 25
```

Scan a temp directory:

```powershell
cargo run --release -- "$env:LOCALAPPDATA\Temp"
```

For full-drive scans on Windows, run the terminal as Administrator if you want fewer access-denied entries.

## Safety

Treat the cleanup candidates as hints, not deletion instructions. Some logs, caches, dump files, or metadata files may still be useful to applications or Windows.
