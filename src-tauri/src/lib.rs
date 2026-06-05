mod asr;
mod audio;
mod benchmark;
mod config;
mod core;
mod doctor;
mod history;
mod input_smoke;
mod itn;
mod llm;
mod model_pack;
mod ptt;
mod retention;
mod support_bundle;
mod text;
mod translation;
mod translation_benchmark;
mod tray;
mod win_bridge;
mod window_shape;

use crate::config::{AppConfig, Paths};
use crate::core::{AppState, SessionState, UiSnapshot};
use anyhow::Result;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::{
    collections::HashSet,
    fs,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    time::Duration,
};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

static HOTKEY_STATUS: Lazy<parking_lot::Mutex<Vec<HotkeyCheck>>> =
    Lazy::new(|| parking_lot::Mutex::new(Vec::new()));

#[derive(Debug, Clone, Serialize)]
struct HotkeyCheck {
    name: String,
    shortcut: String,
    normalized: String,
    status: HotkeyCheckStatus,
    detail: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum HotkeyCheckStatus {
    Pass,
    Warn,
    Fail,
}

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
    if let Err(err) = app.global_shortcut().unregister_all() {
        state.set_runtime_notice(&app, "热键刷新失败", err.to_string());
    }
    register_hotkeys(&app, &state);
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
fn install_model_pack(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_path: String,
) -> Result<UiSnapshot, String> {
    let report = model_pack::install(&PathBuf::from(pack_path), &state.paths).map_err(to_string)?;
    Ok(state.set_runtime_notice(
        &app,
        "模型包已导入",
        format!(
            "{} 个文件，覆盖 {} 个，忽略 {} 个，校验 {} 个；{}",
            report.files_written,
            report.files_replaced,
            report.files_ignored,
            report.checksum_verified,
            report.output_dir
        ),
    ))
}

#[tauri::command]
fn prewarm_asr(app: AppHandle, state: State<'_, AppState>) -> UiSnapshot {
    let snapshot = state.set_runtime_notice(&app, "ASR 预热中", "正在后台加载当前模型");
    let config = snapshot.config.clone();
    let paths = state.paths.clone();
    let app_handle = app.clone();
    std::thread::spawn(move || {
        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| {
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
        })) {
            if let Some(state) = app_handle.try_state::<AppState>() {
                state.report_worker_panic(&app_handle, "asr-prewarm", None, payload);
            }
        }
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
fn doctor_report(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<doctor::DoctorReport, String> {
    let snapshot = state.snapshot();
    let mut report = doctor::run(&state.paths, &snapshot.config).map_err(to_string)?;
    append_hotkey_checks(&mut report);
    state.set_runtime_notice(
        &app,
        "诊断完成",
        format!("{}；报告：{}", report.summary, report.output_path),
    );
    Ok(report)
}

#[tauri::command]
fn repair_doctor(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<doctor::RepairReport, String> {
    let snapshot = state.snapshot();
    let mut report = doctor::repair(&state.paths, &snapshot.config).map_err(to_string)?;
    append_hotkey_checks(&mut report.doctor);
    state.set_runtime_notice(
        &app,
        "修复完成",
        format!("{}；{}", report.summary, report.doctor.summary),
    );
    Ok(report)
}

#[tauri::command]
fn hotkey_status() -> Vec<HotkeyCheck> {
    HOTKEY_STATUS.lock().clone()
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
fn llm_service_status(state: State<'_, AppState>) -> llm::LocalServiceStatus {
    let snapshot = state.snapshot();
    llm::local_service_status(&snapshot.config.smart.endpoint, &state.paths)
}

#[tauri::command]
fn start_llm_service(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<llm::LocalServiceStatus, String> {
    state.set_runtime_notice(&app, "LLM 服务启动中", "正在检查本地 llama-server");
    let snapshot = state.snapshot();
    match llm::start_local_service(&snapshot.config.smart.endpoint, &state.paths) {
        Ok(status) => {
            state.set_runtime_notice(
                &app,
                if status.reachable {
                    "LLM 服务可用"
                } else {
                    "LLM 服务未就绪"
                },
                format!("{}；{}", status.models_url, status.script_path),
            );
            Ok(status)
        }
        Err(err) => {
            let message = err.to_string();
            state.set_runtime_notice(&app, "LLM 服务启动失败", &message);
            Err(message)
        }
    }
}

#[tauri::command]
fn run_asr_benchmark(
    app: AppHandle,
    state: State<'_, AppState>,
    samples_dir: String,
) -> UiSnapshot {
    let samples_path = PathBuf::from(samples_dir);
    let snapshot = state.set_runtime_notice(
        &app,
        "ASR 基准中",
        format!("样本目录：{}", samples_path.to_string_lossy()),
    );
    let config = snapshot.config.clone();
    let paths = state.paths.clone();
    let app_handle = app.clone();
    std::thread::spawn(move || {
        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| {
            let result = benchmark::run_asr(&samples_path, &paths, &config);
            let Some(state) = app_handle.try_state::<AppState>() else {
                return;
            };
            match result {
                Ok(report) => {
                    state.set_runtime_notice(
                        &app_handle,
                        "ASR 基准完成",
                        format!(
                            "{} 个样本，{} 个错误；{}",
                            report.sample_count, report.error_count, report.output_path
                        ),
                    );
                }
                Err(err) => {
                    state.set_runtime_notice(&app_handle, "ASR 基准失败", err.to_string());
                }
            }
        })) {
            if let Some(state) = app_handle.try_state::<AppState>() {
                state.report_worker_panic(&app_handle, "asr-benchmark", None, payload);
            }
        }
    });
    snapshot
}

#[tauri::command]
fn run_translation_benchmark(
    app: AppHandle,
    state: State<'_, AppState>,
    samples_path: String,
) -> UiSnapshot {
    let samples = samples_path.trim().to_string();
    let label = if samples.is_empty() {
        "内置样例".to_string()
    } else {
        samples.clone()
    };
    let snapshot = state.set_runtime_notice(&app, "翻译基准中", format!("样本：{label}"));
    let config = snapshot.config.clone();
    let paths = state.paths.clone();
    let app_handle = app.clone();
    std::thread::spawn(move || {
        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| {
            let sample_path = (!samples.trim().is_empty()).then(|| PathBuf::from(samples));
            let result =
                translation_benchmark::run_translation(sample_path.as_deref(), &paths, &config);
            let Some(state) = app_handle.try_state::<AppState>() else {
                return;
            };
            match result {
                Ok(report) => {
                    state.set_runtime_notice(
                        &app_handle,
                        "翻译基准完成",
                        format!(
                            "{} 个样例，{} 个错误；{}",
                            report.sample_count, report.error_count, report.output_path
                        ),
                    );
                }
                Err(err) => {
                    state.set_runtime_notice(&app_handle, "翻译基准失败", err.to_string());
                }
            }
        })) {
            if let Some(state) = app_handle.try_state::<AppState>() {
                state.report_worker_panic(&app_handle, "translation-benchmark", None, payload);
            }
        }
    });
    snapshot
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
        .plugin(tauri_plugin_dialog::init())
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
            install_model_pack,
            prewarm_asr,
            open_model_download_page,
            open_model_mirror_page,
            open_models_dir,
            open_logs_dir,
            run_doctor,
            doctor_report,
            repair_doctor,
            hotkey_status,
            export_diagnostics,
            export_history_csv,
            llm_service_status,
            start_llm_service,
            run_asr_benchmark,
            run_translation_benchmark,
            open_hotwords_file,
            open_hot_rules_file,
            hide_overlay,
        ])
        .run(tauri::generate_context!())
        .expect("run Voice IME");
}

pub fn run_cli_worker_if_requested() -> bool {
    let mut args = std::env::args_os().skip(1);
    let Some(first) = args.next() else {
        return asr::run_worker_cli_if_requested();
    };
    if first == std::ffi::OsStr::new("--doctor") {
        if let Err(err) = doctor::run_cli() {
            eprintln!("{err:?}");
            std::process::exit(2);
        }
        return true;
    }
    if first == std::ffi::OsStr::new("--benchmark-asr") {
        let samples_dir = args
            .next()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("benchmarks/asr"));
        if let Err(err) = benchmark::run_asr_cli(samples_dir) {
            eprintln!("{err:?}");
            std::process::exit(2);
        }
        return true;
    }
    if first == std::ffi::OsStr::new("--benchmark-translation") {
        let samples_path = args.next().map(std::path::PathBuf::from);
        if let Err(err) = translation_benchmark::run_translation_cli(samples_path) {
            eprintln!("{err:?}");
            std::process::exit(2);
        }
        return true;
    }
    if first == std::ffi::OsStr::new("--install-model-pack") {
        let Some(pack_path) = args.next().map(std::path::PathBuf::from) else {
            eprintln!("missing model pack zip path");
            std::process::exit(2);
        };
        let result = (|| -> anyhow::Result<()> {
            let paths = Paths::discover()?;
            let report = model_pack::install(&pack_path, &paths)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(())
        })();
        if let Err(err) = result {
            eprintln!("{err:?}");
            std::process::exit(2);
        }
        return true;
    }
    if first == std::ffi::OsStr::new("--paste-foreground") {
        let text = args
            .next()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        let delay_ms = args
            .next()
            .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
            .unwrap_or(60);
        if let Err(err) = input_smoke::paste_foreground_cli(text, delay_ms) {
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
        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| {
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
        })) {
            if let Some(state) = app.try_state::<AppState>() {
                state.report_worker_panic(&app, "idle-asr-prewarm", None, payload);
            }
        }
    });
}

fn register_hotkeys(app: &AppHandle, state: &State<'_, AppState>) {
    let config = state.snapshot().config;
    let shortcuts = [
        (
            "录音",
            config.input.hotkey_record,
            HotkeyAction::ToggleRecording,
        ),
        (
            "语言切换",
            config.input.hotkey_language,
            HotkeyAction::CycleLanguage,
        ),
        (
            "转英文",
            config.input.hotkey_english,
            HotkeyAction::TranslateEnglish,
        ),
        (
            "转日文",
            config.input.hotkey_japanese,
            HotkeyAction::TranslateJapanese,
        ),
    ];
    let mut failed = Vec::new();
    let mut checks = Vec::new();
    let mut seen = HashSet::new();
    for (name, shortcut, action) in shortcuts {
        let normalized = normalize_shortcut(&shortcut);
        if normalized.trim().is_empty() || normalized.eq_ignore_ascii_case("off") {
            checks.push(HotkeyCheck {
                name: name.into(),
                shortcut,
                normalized,
                status: HotkeyCheckStatus::Warn,
                detail: "已关闭".into(),
            });
            continue;
        }
        let dedupe_key = normalized.to_ascii_lowercase();
        if !seen.insert(dedupe_key) {
            let detail = "和其他动作重复".to_string();
            failed.push(format!("{name} {normalized}: {detail}"));
            checks.push(HotkeyCheck {
                name: name.into(),
                shortcut,
                normalized,
                status: HotkeyCheckStatus::Fail,
                detail,
            });
            continue;
        }
        match register_hotkey(app, &normalized, action) {
            Ok(()) => checks.push(HotkeyCheck {
                name: name.into(),
                shortcut,
                normalized,
                status: HotkeyCheckStatus::Pass,
                detail: "已注册".into(),
            }),
            Err(err) => {
                failed.push(format!("{name} {normalized}: {err}"));
                checks.push(HotkeyCheck {
                    name: name.into(),
                    shortcut,
                    normalized,
                    status: HotkeyCheckStatus::Fail,
                    detail: err,
                });
            }
        }
    }
    *HOTKEY_STATUS.lock() = checks;
    if !failed.is_empty() {
        state.set_runtime_notice(
            app,
            "热键不可用",
            format!("GUI 已启动；请先用窗口按钮。{}", failed.join("；")),
        );
    }
}

fn append_hotkey_checks(report: &mut doctor::DoctorReport) {
    let checks = hotkey_status();
    report
        .checks
        .extend(checks.into_iter().map(|check| doctor::DoctorCheck {
            name: format!("热键 {}", check.name),
            status: match check.status {
                HotkeyCheckStatus::Pass => doctor::DoctorStatus::Pass,
                HotkeyCheckStatus::Warn => doctor::DoctorStatus::Warn,
                HotkeyCheckStatus::Fail => doctor::DoctorStatus::Fail,
            },
            detail: format!("{}；{}", check.normalized, check.detail),
        }));
    report.summary = doctor::summarize(&report.checks);
}

#[derive(Debug, Clone, Copy)]
enum HotkeyAction {
    ToggleRecording,
    CycleLanguage,
    TranslateEnglish,
    TranslateJapanese,
}

fn register_hotkey(
    app: &AppHandle,
    normalized_shortcut: &str,
    action: HotkeyAction,
) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(normalized_shortcut, move |app, shortcut, event| {
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
