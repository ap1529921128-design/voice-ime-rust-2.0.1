use crate::{
    config::Paths,
    win_bridge::{InputTarget, InputTargetInfo},
};
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    thread,
    time::Duration,
};

#[derive(Serialize)]
struct PasteForegroundLogEntry<'a> {
    created_at: String,
    action: &'static str,
    text_chars: usize,
    paste_delay_ms: u64,
    input_method: Option<&'a str>,
    send_input_events: Option<u32>,
    focus_attempts: Option<u32>,
    focus_restored: Option<bool>,
    clipboard_previous_had_text: Option<bool>,
    clipboard_previous_format: Option<&'a str>,
    clipboard_format_count: Option<u32>,
    clipboard_sequence_before: Option<u32>,
    clipboard_sequence_after: Option<u32>,
    clipboard_restored: Option<bool>,
    clipboard_restore_error: Option<&'a str>,
    result: &'a str,
    error: Option<&'a str>,
    target: &'a InputTargetInfo,
}

pub fn paste_foreground_cli(text: String, delay_ms: u64) -> Result<()> {
    if text.trim().is_empty() {
        return Err(anyhow!("缺少要输入的文本"));
    }
    let paths = Paths::discover()?;
    paths.ensure()?;
    fs::create_dir_all(&paths.logs_dir)?;
    thread::sleep(Duration::from_millis(250));

    let target = InputTarget::capture();
    let target_info = target.info().clone();
    let paste_result = target.paste_text(&text, delay_ms);
    let paste_outcome = paste_result.as_ref().ok();
    let error = paste_result.as_ref().err().map(ToString::to_string);
    let entry = PasteForegroundLogEntry {
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        action: "paste_foreground_cli",
        text_chars: text.chars().count(),
        paste_delay_ms: delay_ms,
        input_method: paste_outcome.map(|outcome| outcome.method),
        send_input_events: paste_outcome.map(|outcome| outcome.send_input_events),
        focus_attempts: paste_outcome.map(|outcome| outcome.focus_attempts),
        focus_restored: paste_outcome.map(|outcome| outcome.focus_restored),
        clipboard_previous_had_text: paste_outcome
            .map(|outcome| outcome.clipboard_previous_had_text),
        clipboard_previous_format: paste_outcome.map(|outcome| outcome.clipboard_previous_format),
        clipboard_format_count: paste_outcome.map(|outcome| outcome.clipboard_format_count),
        clipboard_sequence_before: paste_outcome.map(|outcome| outcome.clipboard_sequence_before),
        clipboard_sequence_after: paste_outcome.map(|outcome| outcome.clipboard_sequence_after),
        clipboard_restored: paste_outcome.map(|outcome| outcome.clipboard_restored),
        clipboard_restore_error: paste_outcome
            .and_then(|outcome| outcome.clipboard_restore_error.as_deref()),
        result: if paste_result.is_ok() { "ok" } else { "error" },
        error: error.as_deref(),
        target: &target_info,
    };
    write_log(&paths, &entry)?;
    paste_result?;
    println!("paste_foreground_cli ok: {}", target_info.process_name);
    Ok(())
}

fn write_log(paths: &Paths, entry: &PasteForegroundLogEntry<'_>) -> Result<()> {
    let path = paths.logs_dir.join(format!(
        "input-target-{}.log",
        chrono::Local::now().format("%Y%m%d")
    ));
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(entry)?)?;
    Ok(())
}
