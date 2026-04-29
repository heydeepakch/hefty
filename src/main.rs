use std::cmp::Reverse;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

const DEFAULT_TOP: usize = 15;

fn main() {
    let command = match parse_args(env::args().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("error: {message}\n");
            eprintln!("{}", usage());
            process::exit(2);
        }
    };

    match command {
        Command::Help => println!("{}", usage()),
        Command::Run(config) => match analyze(&config.root) {
            Ok(report) => print_report(&report, config.top),
            Err(error) => {
                eprintln!("error: could not scan {}: {error}", config.root.display());
                process::exit(1);
            }
        },
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Run(Config),
}

#[derive(Debug, PartialEq, Eq)]
struct Config {
    root: PathBuf,
    top: usize,
}

#[derive(Debug)]
struct ScanReport {
    root: PathBuf,
    total_size: u64,
    files_scanned: u64,
    dirs_scanned: u64,
    hidden_entries: u64,
    symlinks_skipped: u64,
    other_entries: u64,
    errors: Vec<ScanError>,
    files: Vec<SizedEntry>,
    dirs: Vec<SizedEntry>,
    candidates: Vec<CleanupCandidate>,
}

#[derive(Debug)]
struct ScanError {
    path: PathBuf,
    message: String,
}

#[derive(Clone, Debug)]
struct SizedEntry {
    path: PathBuf,
    size: u64,
}

#[derive(Debug)]
struct CleanupCandidate {
    path: PathBuf,
    size: u64,
    reason: &'static str,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut root = None;
    let mut top = DEFAULT_TOP;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "-n" | "--top" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{arg} requires a number"))?;
                top = value
                    .parse::<usize>()
                    .map_err(|_| format!("{arg} requires a positive number"))?;
                if top == 0 {
                    return Err(format!("{arg} requires a number greater than zero"));
                }
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}")),
            _ => {
                if root.is_some() {
                    return Err("only one scan path is supported".to_string());
                }
                root = Some(PathBuf::from(arg));
            }
        }
    }

    let root = root.unwrap_or(env::current_dir().map_err(|error| error.to_string())?);
    Ok(Command::Run(Config { root, top }))
}

fn usage() -> String {
    format!(
        "Usage: storage-cleanup-helper [PATH] [--top N]\n\n\
         Recursively scans PATH and reports the largest files, largest directories,\n\
         likely cleanup candidates, and directories that could not be accessed.\n\n\
         Examples:\n\
           storage-cleanup-helper C:\\ --top 25\n\
           storage-cleanup-helper \"%LOCALAPPDATA%\\Temp\"\n\n\
         This tool is read-only. It reports cleanup candidates but never deletes files."
    )
}

fn analyze(root: &Path) -> io::Result<ScanReport> {
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

fn print_report(report: &ScanReport, top: usize) {
    println!("Scan root: {}", report.root.display());
    println!("Total file size: {}", format_bytes(report.total_size));
    println!(
        "Scanned: {} files, {} directories, {} hidden entries",
        report.files_scanned, report.dirs_scanned, report.hidden_entries
    );
    println!(
        "Skipped: {} symlinks, {} other entries, {} access errors",
        report.symlinks_skipped,
        report.other_entries,
        report.errors.len()
    );

    print_sized_section("Largest directories", &report.dirs, top);
    print_sized_section("Largest files", &report.files, top);
    print_candidate_section(&report.candidates, top);

    if !report.errors.is_empty() {
        println!("\nAccess errors");
        for error in report.errors.iter().take(top) {
            println!("  {}  {}", error.path.display(), error.message);
        }
        if report.errors.len() > top {
            println!("  ... {} more", report.errors.len() - top);
        }
    }
}

fn print_sized_section(title: &str, entries: &[SizedEntry], top: usize) {
    println!("\n{title}");
    for entry in entries.iter().take(top) {
        println!(
            "  {:>10}  {}",
            format_bytes(entry.size),
            entry.path.display()
        );
    }
    if entries.is_empty() {
        println!("  none");
    }
}

fn print_candidate_section(candidates: &[CleanupCandidate], top: usize) {
    println!("\nLikely cleanup candidates");
    for candidate in candidates.iter().take(top) {
        println!(
            "  {:>10}  {}  ({})",
            format_bytes(candidate.size),
            candidate.path.display(),
            candidate.reason
        );
    }
    if candidates.is_empty() {
        println!("  none");
    }
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

fn format_bytes(bytes: u64) -> String {
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
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_path_and_top_count() {
        let command = parse_args(["C:\\".to_string(), "--top".to_string(), "25".to_string()])
            .expect("args should parse");

        assert_eq!(
            command,
            Command::Run(Config {
                root: PathBuf::from("C:\\"),
                top: 25
            })
        );
    }

    #[test]
    fn rejects_zero_top_count() {
        let error = parse_args(["--top".to_string(), "0".to_string()])
            .expect_err("zero should be rejected");

        assert!(error.contains("greater than zero"));
    }

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
