mod asr;
mod audio;
mod config;
mod core;
mod history;
mod llm;
mod text;
mod win_bridge;

use crate::config::AppConfig;
use crate::core::{AppState, UiSnapshot};
use anyhow::Result;
use std::fs;
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
    state.save_config(&app, config).map_err(to_string)
}

#[tauri::command]
fn clear_history(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    state.clear_history(&app).map_err(to_string)
}

#[tauri::command]
fn audio_devices() -> Result<Vec<audio::AudioDeviceInfo>, String> {
    audio::input_devices().map_err(to_string)
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
fn hide_overlay(app: AppHandle) {
    core::hide_overlay(&app);
}

pub fn run() {
    let app_state = AppState::load().expect("load Voice IME state");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .setup(|app| {
            let state = app.state::<AppState>();
            register_hotkeys(app.handle(), &state);
            core::emit_snapshot(app.handle(), &state);
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
            audio_devices,
            asr_status,
            download_asr_model,
            open_model_download_page,
            open_model_mirror_page,
            open_models_dir,
            hide_overlay,
        ])
        .run(tauri::generate_context!())
        .expect("run Voice IME");
}

pub fn run_cli_worker_if_requested() -> bool {
    asr::run_worker_cli_if_requested()
}

fn to_string(err: impl std::fmt::Display) -> String {
    err.to_string()
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
