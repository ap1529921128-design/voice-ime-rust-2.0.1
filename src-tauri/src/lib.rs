mod asr;
mod audio;
mod config;
mod core;
mod doctor;
mod history;
mod itn;
mod llm;
mod ptt;
mod retention;
mod support_bundle;
mod text;
mod translation;
mod tray;
mod win_bridge;
mod window_shape;

use crate::config::{AppConfig, Paths};
use crate::core::{AppState, SessionState, UiSnapshot};
use anyhow::Result;
use std::{fs, time::Duration};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> UiSnapshot {
    state.snapshot()
}

#[tauri::command]
fn start_recording(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    state.start_recording(&app).map_err(to_string)
}

#[tauri::command]
fn stop_recording(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    state.stop_recording(&app).map_err(to_string)
}

#[tauri::command]
fn toggle_recording(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    if state.recorder.is_recording() {
        state.stop_recording(&app).map_err(to_string)
    } else {
        state.start_recording(&app).map_err(to_string)
    }
}

#[tauri::command]
fn clear_text(app: AppHandle, state: State<'_, AppState>) -> UiSnapshot {
    state.clear(&app)
}

#[tauri::command]
fn set_text(app: AppHandle, state: State<'_, AppState>, text: String) -> UiSnapshot {
    state.set_text(&app, text)
}

#[tauri::command]
fn copy_text(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    state.copy_text(&app).map_err(to_string)
}

#[tauri::command]
fn confirm_input(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    state.confirm_input(&app).map_err(to_string)
}

#[tauri::command]
fn cycle_language(app: AppHandle, state: State<'_, AppState>) -> UiSnapshot {
    state.cycle_language(&app)
}

#[tauri::command]
fn translate_text(
    app: AppHandle,
    state: State<'_, AppState>,
    target_language: String,
) -> Result<UiSnapshot, String> {
    state
        .translate_current(&app, target_language)
        .map_err(to_string)
}

#[tauri::command]
fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<UiSnapshot, String> {
    let snapshot = state.save_config(&app, config).map_err(to_string)?;
    ptt::update_config(&snapshot.config);
    Ok(snapshot)
}

#[tauri::command]
fn clear_history(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    state.clear_history(&app).map_err(to_string)
}

#[tauri::command]
fn clear_recordings(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    let removed =
        retention::clear_recording_files(&state.paths.recordings_dir).map_err(to_string)?;
    Ok(state.set_runtime_notice(
        &app,
        "录音已清理",
        format!("已删除 {} 个长录音文件", removed),
    ))
}

#[tauri::command]
fn audio_devices() -> Result<Vec<audio::AudioDeviceInfo>, String> {
    audio::input_devices().map_err(to_string)
}

#[tauri::command]
fn audio_level(
    state: State<'_, AppState>,
    device_name: Option<String>,
) -> Result<audio::AudioLevelInfo, String> {
    let configured = state.snapshot().config.asr.input_device_name;
    let selected = device_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let configured = configured.trim();
            if configured.is_empty() {
                None
            } else {
                Some(configured)
            }
        });
    audio::measure_input_level(selected, Duration::from_millis(220)).map_err(to_string)
}

#[tauri::command]
fn asr_status(state: State<'_, AppState>) -> Vec<asr::AsrModelStatus> {
    let snapshot = state.snapshot();
    asr::model_status(&snapshot.config, &state.paths)
}

#[tauri::command]
fn download_asr_model(
    app: AppHandle,
    state: State<'_, AppState>,
    profile: String,
) -> Result<UiSnapshot, String> {
    state.download_asr_model(&app, profile).map_err(to_string)
}

#[tauri::command]
fn prewarm_asr(app: AppHandle, state: State<'_, AppState>) -> UiSnapshot {
    let snapshot = state.set_runtime_notice(&app, "ASR 预热中", "正在后台加载当前模型");
    let config = snapshot.config.clone();
    let paths = state.paths.clone();
    let app_handle = app.clone();
    std::thread::spawn(move || {
        let result = asr::prewarm(&config, &paths);
        let Some(state) = app_handle.try_state::<AppState>() else {
            return;
        };
        match result {
            Ok(status) => {
                state.set_runtime_notice(
                    &app_handle,
                    "ASR 已预热",
                    format!("{} / {:.1}s", status.profile, status.elapsed_seconds),
                );
            }
            Err(err) => {
                state.set_runtime_notice(&app_handle, "ASR 预热跳过", err.to_string());
            }
        };
    });
    snapshot
}

#[tauri::command]
fn open_model_download_page(app: AppHandle, profile: String) -> Result<(), String> {
    app.opener()
        .open_url(asr::download_url_for_profile(&profile), None::<&str>)
        .map_err(to_string)
}

#[tauri::command]
fn open_model_mirror_page(app: AppHandle, profile: String) -> Result<(), String> {
    app.opener()
        .open_url(asr::mirror_url_for_profile(&profile), None::<&str>)
        .map_err(to_string)
}

#[tauri::command]
fn open_models_dir(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let models_dir = state.paths.root_dir.join("models");
    fs::create_dir_all(&models_dir).map_err(to_string)?;
    app.opener()
        .open_path(models_dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(to_string)
}

#[tauri::command]
fn open_logs_dir(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    fs::create_dir_all(&state.paths.logs_dir).map_err(to_string)?;
    app.opener()
        .open_path(
            state.paths.logs_dir.to_string_lossy().to_string(),
            None::<&str>,
        )
        .map_err(to_string)
}

#[tauri::command]
fn run_doctor(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    let snapshot = state.snapshot();
    let report = doctor::run(&state.paths, &snapshot.config).map_err(to_string)?;
    Ok(state.set_runtime_notice(
        &app,
        "诊断完成",
        format!("{}；报告：{}", report.summary, report.output_path),
    ))
}

#[tauri::command]
fn export_diagnostics(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    let snapshot = state.snapshot();
    let _ = doctor::run(&state.paths, &snapshot.config).map_err(to_string)?;
    let output_path = support_bundle::export(&state.paths, &snapshot.config).map_err(to_string)?;
    if let Some(parent) = output_path.parent() {
        let _ = app
            .opener()
            .open_path(parent.to_string_lossy().to_string(), None::<&str>);
    }
    Ok(state.set_runtime_notice(
        &app,
        "导出完成",
        format!("诊断包：{}", output_path.to_string_lossy()),
    ))
}

#[tauri::command]
fn export_history_csv(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    fs::create_dir_all(&state.paths.logs_dir).map_err(to_string)?;
    let snapshot = state.snapshot();
    let output_path = state.paths.logs_dir.join(format!(
        "history-export-{}.csv",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    history::export_csv_file(&output_path, &snapshot.history).map_err(to_string)?;
    if let Some(parent) = output_path.parent() {
        let _ = app
            .opener()
            .open_path(parent.to_string_lossy().to_string(), None::<&str>);
    }
    Ok(state.set_runtime_notice(
        &app,
        "历史已导出",
        format!(
            "{} 条 / {}",
            snapshot.history.len(),
            output_path.to_string_lossy()
        ),
    ))
}

#[tauri::command]
fn open_hotwords_file(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    ensure_text_file(&state.paths.hotwords_path, "# hot.txt\n").map_err(to_string)?;
    app.opener()
        .open_path(
            state.paths.hotwords_path.to_string_lossy().to_string(),
            None::<&str>,
        )
        .map_err(to_string)
}

#[tauri::command]
fn open_hot_rules_file(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    ensure_text_file(&state.paths.hot_rules_path, "# hot-rule.txt\n").map_err(to_string)?;
    app.opener()
        .open_path(
            state.paths.hot_rules_path.to_string_lossy().to_string(),
            None::<&str>,
        )
        .map_err(to_string)
}

#[tauri::command]
fn hide_overlay(app: AppHandle) {
    core::hide_overlay(&app);
}

pub fn run() {
    let app_state = AppState::load().expect("load Voice IME state");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    tray::hide_main_to_tray(window.app_handle());
                }
            }
        })
        .manage(app_state)
        .setup(|app| {
            window_shape::install(app);
            let tray_error = tray::install(app).err().map(to_string);
            let state = app.state::<AppState>();
            register_hotkeys(app.handle(), &state);
            let snapshot = state.snapshot();
            ptt::install(app.handle(), &snapshot.config);
            if let Some(err) = tray_error {
                state.set_runtime_notice(app.handle(), "托盘不可用", err);
            }
            core::emit_snapshot(app.handle(), &state);
            schedule_idle_asr_prewarm(app.handle().clone(), state.paths.clone(), snapshot.config);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            start_recording,
            stop_recording,
            toggle_recording,
            clear_text,
            set_text,
            copy_text,
            confirm_input,
            cycle_language,
            translate_text,
            save_config,
            clear_history,
            clear_recordings,
            audio_devices,
            audio_level,
            asr_status,
            download_asr_model,
            prewarm_asr,
            open_model_download_page,
            open_model_mirror_page,
            open_models_dir,
            open_logs_dir,
            run_doctor,
            export_diagnostics,
            export_history_csv,
            open_hotwords_file,
            open_hot_rules_file,
            hide_overlay,
        ])
        .run(tauri::generate_context!())
        .expect("run Voice IME");
}

pub fn run_cli_worker_if_requested() -> bool {
    let is_doctor = std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == std::ffi::OsStr::new("--doctor"));
    if is_doctor {
        if let Err(err) = doctor::run_cli() {
            eprintln!("{err:?}");
            std::process::exit(2);
        }
        return true;
    }
    asr::run_worker_cli_if_requested()
}

fn to_string(err: impl std::fmt::Display) -> String {
    err.to_string()
}

fn ensure_text_file(path: &std::path::Path, default_body: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        fs::write(path, default_body)?;
    }
    Ok(())
}

fn schedule_idle_asr_prewarm(app: AppHandle, paths: Paths, config: AppConfig) {
    if config.asr.worker_mode != "persistent" {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1800));
        let Some(state) = app.try_state::<AppState>() else {
            return;
        };
        let snapshot = state.snapshot();
        if snapshot.state != SessionState::Idle || state.recorder.is_recording() {
            return;
        }
        let Ok(status) = asr::prewarm(&config, &paths) else {
            return;
        };
        if state.snapshot().state == SessionState::Idle {
            state.set_runtime_notice(
                &app,
                "ASR 已预热",
                format!("{} / {:.1}s", status.profile, status.elapsed_seconds),
            );
        }
    });
}

fn register_hotkeys(app: &AppHandle, state: &State<'_, AppState>) {
    let config = state.snapshot().config;
    let shortcuts = [
        (config.input.hotkey_record, HotkeyAction::ToggleRecording),
        (config.input.hotkey_language, HotkeyAction::CycleLanguage),
        (config.input.hotkey_english, HotkeyAction::TranslateEnglish),
        (
            config.input.hotkey_japanese,
            HotkeyAction::TranslateJapanese,
        ),
    ];
    let mut failed = Vec::new();
    for (shortcut, action) in shortcuts {
        if let Err(err) = register_hotkey(app, &shortcut, action) {
            failed.push(format!("{shortcut}: {err}"));
        }
    }
    if !failed.is_empty() {
        state.set_runtime_notice(
            app,
            "热键不可用",
            format!("GUI 已启动；请先用窗口按钮。{}", failed.join("；")),
        );
    }
}

#[derive(Debug, Clone, Copy)]
enum HotkeyAction {
    ToggleRecording,
    CycleLanguage,
    TranslateEnglish,
    TranslateJapanese,
}

fn register_hotkey(app: &AppHandle, shortcut: &str, action: HotkeyAction) -> Result<(), String> {
    let normalized = normalize_shortcut(shortcut);
    app.global_shortcut()
        .on_shortcut(normalized.as_str(), move |app, shortcut, event| {
            if event.state != ShortcutState::Released {
                return;
            }
            let _ = shortcut;
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            match action {
                HotkeyAction::ToggleRecording => {
                    let _ = if state.recorder.is_recording() {
                        state.stop_recording(app)
                    } else {
                        state.start_recording(app)
                    };
                }
                HotkeyAction::CycleLanguage => {
                    let _ = state.cycle_language(app);
                }
                HotkeyAction::TranslateEnglish => {
                    let _ = state.translate_current(app, "en".into());
                }
                HotkeyAction::TranslateJapanese => {
                    let _ = state.translate_current(app, "ja".into());
                }
            }
        })
        .map_err(to_string)
}

fn normalize_shortcut(shortcut: &str) -> String {
    match shortcut.trim() {
        "AltRight" => "Alt+R".into(),
        "Alt+KeyE" => "Alt+E".into(),
        "Alt+KeyJ" => "Alt+J".into(),
        other => other.to_string(),
    }
}
