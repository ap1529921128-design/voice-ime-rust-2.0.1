#![allow(deprecated)]

use crate::config::Paths;
use chrono::Local;
use serde::Serialize;
use std::{
    backtrace::Backtrace,
    fs::{self, OpenOptions},
    io::Write,
    panic::{self, PanicInfo},
    sync::Once,
};

static INSTALL: Once = Once::new();

#[derive(Serialize)]
struct PanicLogEntry<'a> {
    created_at: String,
    thread: String,
    message: String,
    location: Option<String>,
    backtrace: &'a str,
}

pub fn install() {
    INSTALL.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            write_panic_log(info);
            previous(info);
        }));
    });
}

fn write_panic_log(info: &PanicInfo<'_>) {
    let Some(path) = panic_log_path() else {
        return;
    };
    let backtrace = Backtrace::force_capture().to_string();
    let entry = PanicLogEntry {
        created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        thread: std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string(),
        message: panic_message(info),
        location: info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line())),
        backtrace: &backtrace,
    };
    let Ok(line) = serde_json::to_string(&entry) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

fn panic_log_path() -> Option<std::path::PathBuf> {
    match Paths::discover() {
        Ok(paths) => Some(
            paths
                .logs_dir
                .join(format!("panic-{}.log", Local::now().format("%Y%m%d"))),
        ),
        Err(_) => Some(std::env::temp_dir().join(format!(
            "voice-ime-panic-{}.log",
            Local::now().format("%Y%m%d")
        ))),
    }
}

fn panic_message(info: &PanicInfo<'_>) -> String {
    if let Some(message) = info.payload().downcast_ref::<&'static str>() {
        return (*message).to_string();
    }
    if let Some(message) = info.payload().downcast_ref::<String>() {
        return message.clone();
    }
    "unknown panic payload".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_panic_log_path_is_some() {
        assert!(panic_log_path().is_some());
    }

    #[test]
    fn panic_log_entry_serializes_as_json_line() {
        let entry = PanicLogEntry {
            created_at: "2026-06-05 12:00:00".into(),
            thread: "test-thread".into(),
            message: "boom".into(),
            location: Some("src/main.rs:7".into()),
            backtrace: "trace",
        };
        let body = serde_json::to_string(&entry).unwrap();
        assert!(body.contains("\"thread\":\"test-thread\""));
        assert!(body.contains("\"message\":\"boom\""));
    }
}
