#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use scanner::{ScanReport, analyze_with_cancel, format_bytes};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::State;

struct ScanState {
    cancel: Arc<AtomicBool>,
}

#[derive(Serialize)]
struct ScanResponse {
    report: ScanReport,
    total_size_human: String,
}

#[tauri::command]
async fn scan(path: String, state: State<'_, ScanState>) -> Result<ScanResponse, String> {
    let expanded = expand_env_vars(&path);
    let pb = PathBuf::from(&expanded);
    state.cancel.store(false, Ordering::SeqCst);

    let cancel = Arc::clone(&state.cancel);
    let report = tauri::async_runtime::spawn_blocking(move || analyze_with_cancel(&pb, &cancel))
        .await
        .map_err(|error| format!("scan task panicked: {error}"))?
        .map_err(|error| format!("could not scan {expanded}: {error}"))?;

    let total_size_human = format_bytes(report.total_size);
    Ok(ScanResponse {
        report,
        total_size_human,
    })
}

#[tauri::command]
fn stop_scan(state: State<'_, ScanState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

fn expand_env_vars(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut name = String::new();
        let mut closed = false;
        while let Some(&next) = chars.peek() {
            if next == '%' {
                chars.next();
                closed = true;
                break;
            }
            name.push(next);
            chars.next();
        }
        if closed
            && !name.is_empty()
            && let Ok(value) = std::env::var(&name)
        {
            out.push_str(&value);
        } else {
            out.push('%');
            out.push_str(&name);
            if closed {
                out.push('%');
            }
        }
    }
    out
}

#[tauri::command]
fn format_size(bytes: u64) -> String {
    format_bytes(bytes)
}

fn main() {
    tauri::Builder::default()
        .manage(ScanState {
            cancel: Arc::new(AtomicBool::new(false)),
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scan, stop_scan, format_size])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
