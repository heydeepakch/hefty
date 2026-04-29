use scanner::{CleanupCandidate, ScanReport, SizedEntry, analyze, format_bytes};
use std::env;
use std::path::PathBuf;
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
        "Usage: storage-cleanup-cli [PATH] [--top N]\n\n\
         Recursively scans PATH and reports the largest files, largest directories,\n\
         likely cleanup candidates, and directories that could not be accessed.\n\n\
         Examples:\n\
           storage-cleanup-cli C:\\ --top 25\n\
           storage-cleanup-cli \"%LOCALAPPDATA%\\Temp\"\n\n\
         This tool is read-only. It reports cleanup candidates but never deletes files."
    )
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
