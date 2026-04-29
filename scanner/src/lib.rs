use serde::Serialize;
use std::cmp::Reverse;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub root: PathBuf,
    pub total_size: u64,
    pub files_scanned: u64,
    pub dirs_scanned: u64,
    pub hidden_entries: u64,
    pub symlinks_skipped: u64,
    pub other_entries: u64,
    pub errors: Vec<ScanError>,
    pub files: Vec<SizedEntry>,
    pub dirs: Vec<SizedEntry>,
    pub candidates: Vec<CleanupCandidate>,
}

#[derive(Debug, Serialize)]
pub struct ScanError {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SizedEntry {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct CleanupCandidate {
    pub path: PathBuf,
    pub size: u64,
    pub reason: &'static str,
}

pub fn analyze(root: &Path) -> io::Result<ScanReport> {
    let root = root.to_path_buf();
    let metadata = fs::symlink_metadata(&root)?;
    let mut report = ScanReport {
        root: root.clone(),
        total_size: 0,
        files_scanned: 0,
        dirs_scanned: 0,
        hidden_entries: 0,
        symlinks_skipped: 0,
        other_entries: 0,
        errors: Vec::new(),
        files: Vec::new(),
        dirs: Vec::new(),
        candidates: Vec::new(),
    };

    report.total_size = scan_entry(&root, &metadata, &mut report);
    report.files.sort_by_key(|entry| Reverse(entry.size));
    report.dirs.sort_by_key(|entry| Reverse(entry.size));
    report.candidates.sort_by_key(|entry| Reverse(entry.size));

    Ok(report)
}

fn scan_entry(path: &Path, metadata: &fs::Metadata, report: &mut ScanReport) -> u64 {
    if metadata.file_type().is_symlink() {
        report.symlinks_skipped += 1;
        return 0;
    }

    if is_hidden(path, metadata) {
        report.hidden_entries += 1;
    }

    if metadata.is_file() {
        let size = metadata.len();
        report.files_scanned += 1;
        report.files.push(SizedEntry {
            path: path.to_path_buf(),
            size,
        });
        if let Some(reason) = cleanup_reason(path, false) {
            report.candidates.push(CleanupCandidate {
                path: path.to_path_buf(),
                size,
                reason,
            });
        }
        return size;
    }

    if metadata.is_dir() {
        report.dirs_scanned += 1;
        let mut total = 0;

        match fs::read_dir(path) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let child_path = entry.path();
                            match fs::symlink_metadata(&child_path) {
                                Ok(child_metadata) => {
                                    total += scan_entry(&child_path, &child_metadata, report);
                                }
                                Err(error) => report.errors.push(ScanError {
                                    path: child_path,
                                    message: error.to_string(),
                                }),
                            }
                        }
                        Err(error) => report.errors.push(ScanError {
                            path: path.to_path_buf(),
                            message: error.to_string(),
                        }),
                    }
                }
            }
            Err(error) => report.errors.push(ScanError {
                path: path.to_path_buf(),
                message: error.to_string(),
            }),
        }

        report.dirs.push(SizedEntry {
            path: path.to_path_buf(),
            size: total,
        });
        if let Some(reason) = cleanup_reason(path, true) {
            report.candidates.push(CleanupCandidate {
                path: path.to_path_buf(),
                size: total,
                reason,
            });
        }
        return total;
    }

    report.other_entries += 1;
    0
}

fn cleanup_reason(path: &Path, is_dir: bool) -> Option<&'static str> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if is_dir && is_temp_or_cache_name(&name) {
        return Some("temporary/cache-like directory");
    }

    if !is_dir {
        if matches!(
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.to_ascii_lowercase())
                .as_deref(),
            Some("tmp" | "temp" | "log" | "bak" | "old" | "dmp" | "etl")
        ) {
            return Some("temporary/log/backup-like file extension");
        }

        if name == "thumbs.db" || name == "desktop.ini" {
            return Some("windows metadata file");
        }
    }

    if path_components_contain_temp_or_cache(path) {
        return Some("inside temporary/cache-like location");
    }

    None
}

fn path_components_contain_temp_or_cache(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .map(|value| is_temp_or_cache_name(&value.to_ascii_lowercase()))
            .unwrap_or(false)
    })
}

fn is_temp_or_cache_name(name: &str) -> bool {
    matches!(
        name,
        "temp"
            | "tmp"
            | "cache"
            | "caches"
            | ".cache"
            | "logs"
            | "log"
            | "dumps"
            | "crashdumps"
            | "$recycle.bin"
    )
}

#[cfg(windows)]
fn is_hidden(path: &Path, metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0 || has_dot_prefix(path)
}

#[cfg(not(windows))]
fn is_hidden(path: &Path, _metadata: &fs::Metadata) -> bool {
    has_dot_prefix(path)
}

fn has_dot_prefix(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with('.') && name != "." && name != "..")
        .unwrap_or(false)
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else if value >= 10.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn aggregates_sizes_and_candidates() {
        let root = test_root("storage-cleanup-helper-scan");
        fs::create_dir_all(root.join("nested").join("cache")).expect("create fixture dirs");
        write_bytes(&root.join("large.bin"), 10);
        write_bytes(&root.join("nested").join("note.tmp"), 5);
        write_bytes(&root.join("nested").join("cache").join("debug.log"), 7);

        let report = analyze(&root).expect("scan should succeed");

        assert_eq!(report.total_size, 22);
        assert_eq!(report.files_scanned, 3);
        assert!(report.dirs_scanned >= 3);
        assert_eq!(report.files.first().expect("largest file").size, 10);
        assert!(
            report
                .candidates
                .iter()
                .any(|candidate| candidate.path.ends_with("note.tmp"))
        );
        assert!(
            report
                .candidates
                .iter()
                .any(|candidate| candidate.path.ends_with("cache"))
        );

        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn formats_bytes_for_humans() {
        assert_eq!(format_bytes(900), "900 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(10 * 1024 * 1024), "10 MB");
    }

    fn test_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        env::temp_dir().join(format!("{name}-{unique}"))
    }

    fn write_bytes(path: &Path, size: usize) {
        fs::write(path, vec![0; size]).expect("write fixture file");
    }
}
