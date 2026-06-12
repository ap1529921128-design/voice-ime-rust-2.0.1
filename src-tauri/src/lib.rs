mod asr;
mod audio;
mod benchmark;
mod cancel;
mod clipboard;
mod config;
mod core;
mod doctor;
mod history;
mod input_smoke;
mod input_target;
mod itn;
mod llm;
mod model_pack;
mod panic_log;
mod ptt;
mod retention;
mod support_bundle;
mod text;
mod translation;
mod translation_benchmark;
mod translation_log;
mod tray;
mod window_shape;

use crate::config::{AppConfig, Paths};
use crate::core::{AppState, SessionState, UiSnapshot};
use anyhow::Result;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::{
    collections::HashMap,
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
    suggestion: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum HotkeyCheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
struct ModelRootOverrideStatus {
    file_path: String,
    exists: bool,
    value: String,
    effective_root: String,
    effective_source: String,
    env_override_active: bool,
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
fn personal_prompt(state: State<'_, AppState>) -> Result<String, String> {
    config::read_personal_prompt(&state.paths).map_err(to_string)
}

#[tauri::command]
fn save_personal_prompt(
    app: AppHandle,
    state: State<'_, AppState>,
    prompt: String,
) -> Result<UiSnapshot, String> {
    let prompt = config::save_personal_prompt(&state.paths, &prompt).map_err(to_string)?;
    Ok(state.set_runtime_notice(
        &app,
        "提示词已保存",
        format!("{} 字 / 下次智能纠错生效", prompt.chars().count()),
    ))
}

#[tauri::command]
fn reset_personal_prompt(app: AppHandle, state: State<'_, AppState>) -> Result<UiSnapshot, String> {
    let prompt = config::reset_personal_prompt(&state.paths).map_err(to_string)?;
    Ok(state.set_runtime_notice(
        &app,
        "提示词已恢复",
        format!("默认提示词 / {} 字", prompt.chars().count()),
    ))
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
    let snapshot = state.snapshot();
    let report = model_pack::install(&PathBuf::from(pack_path), &state.paths, &snapshot.config)
        .map_err(to_string)?;
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
fn model_root_override_status(state: State<'_, AppState>) -> ModelRootOverrideStatus {
    let snapshot = state.snapshot();
    let file_path = config::model_root_override_path(&state.paths);
    ModelRootOverrideStatus {
        file_path: file_path.to_string_lossy().to_string(),
        exists: file_path.is_file(),
        value: config::model_root_override_value(&state.paths).unwrap_or_default(),
        effective_root: config::effective_model_root(&snapshot.config, &state.paths)
            .to_string_lossy()
            .to_string(),
        effective_source: config::effective_model_root_source(&snapshot.config, &state.paths)
            .to_string(),
        env_override_active: std::env::var_os("VOICE_IME_MODEL_DIR").is_some(),
    }
}

#[tauri::command]
fn write_model_root_override(
    app: AppHandle,
    state: State<'_, AppState>,
    model_root: String,
) -> Result<UiSnapshot, String> {
    let written =
        config::write_model_root_override(&state.paths, &model_root).map_err(to_string)?;
    Ok(state.set_runtime_notice(
        &app,
        "便携模型目录已写入",
        format!("MODEL_ROOT.txt -> {}", written.to_string_lossy()),
    ))
}

#[tauri::command]
fn clear_model_root_override(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UiSnapshot, String> {
    let removed = config::clear_model_root_override(&state.paths).map_err(to_string)?;
    let detail = if removed {
        "已清除 MODEL_ROOT.txt"
    } else {
        "MODEL_ROOT.txt 原本不存在"
    };
    Ok(state.set_runtime_notice(&app, "便携模型目录已清除", detail))
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
    let snapshot = state.snapshot();
    let models_dir = config::effective_model_root(&snapshot.config, &state.paths);
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
fn dictionary_stats(state: State<'_, AppState>) -> text::UserDictionaryStats {
    text::user_dictionary_stats(&state.paths.hotwords_path, &state.paths.hot_rules_path)
}

#[tauri::command]
fn test_dictionary_text(state: State<'_, AppState>, text: String) -> text::DictionaryTestResult {
    text::dictionary_test_result(&text, &state.paths.corrections_path)
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
    llm::local_service_status(
        &snapshot.config.smart.endpoint,
        &state.paths,
        &snapshot.config,
    )
}

#[tauri::command]
fn start_llm_service(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<llm::LocalServiceStatus, String> {
    state.set_runtime_notice(&app, "LLM 服务启动中", "正在检查本地 llama-server");
    let snapshot = state.snapshot();
    match llm::start_local_service(
        &snapshot.config.smart.endpoint,
        &state.paths,
        &snapshot.config,
    ) {
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
    profile: Option<String>,
) -> UiSnapshot {
    let samples_path = PathBuf::from(samples_dir);
    let profile_label = benchmark_profile_label(profile.as_deref());
    let snapshot = state.set_runtime_notice(
        &app,
        "ASR 基准中",
        format!(
            "{} · 样本目录：{}",
            profile_label,
            samples_path.to_string_lossy()
        ),
    );
    let config = benchmark_config_for_profile(snapshot.config.clone(), profile.as_deref());
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
                            "{} · {} 个样本，{} 个错误；{}",
                            config.asr.profile,
                            report.sample_count,
                            report.error_count,
                            report.output_path
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
fn write_asr_benchmark_template(
    app: AppHandle,
    state: State<'_, AppState>,
    samples_dir: String,
) -> Result<UiSnapshot, String> {
    let samples_dir = samples_dir.trim();
    if samples_dir.is_empty() {
        return Err("请选择 ASR 样本目录".into());
    }
    let samples_path = PathBuf::from(samples_dir);
    let report = benchmark::write_sample_template(&samples_path).map_err(to_string)?;
    Ok(state.set_runtime_notice(
        &app,
        "ASR 样本模板已生成",
        format!(
            "{} 句 / 写入 {} 个，跳过 {} 个；{}",
            report.sample_count, report.files_written, report.files_skipped, report.output_dir
        ),
    ))
}

fn benchmark_config_for_profile(mut config: AppConfig, profile: Option<&str>) -> AppConfig {
    if let Some(profile) = normalized_benchmark_profile(profile) {
        config.asr.profile = profile;
    }
    config
}

fn benchmark_profile_label(profile: Option<&str>) -> String {
    normalized_benchmark_profile(profile).unwrap_or_else(|| "当前档位".into())
}

fn normalized_benchmark_profile(profile: Option<&str>) -> Option<String> {
    let profile = profile?.trim();
    if matches!(profile, "fast" | "balanced" | "fallback" | "accurate") {
        Some(profile.to_string())
    } else {
        None
    }
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
    panic_log::install();
    let app_state = AppState::load().expect("load Voice IME state");
    let app = tauri::Builder::default()
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
            personal_prompt,
            save_personal_prompt,
            reset_personal_prompt,
            clear_history,
            clear_recordings,
            audio_devices,
            audio_level,
            asr_status,
            download_asr_model,
            install_model_pack,
            model_root_override_status,
            write_model_root_override,
            clear_model_root_override,
            prewarm_asr,
            open_model_download_page,
            open_model_mirror_page,
            open_models_dir,
            open_logs_dir,
            run_doctor,
            doctor_report,
            repair_doctor,
            hotkey_status,
            dictionary_stats,
            test_dictionary_text,
            export_diagnostics,
            export_history_csv,
            llm_service_status,
            start_llm_service,
            run_asr_benchmark,
            write_asr_benchmark_template,
            run_translation_benchmark,
            open_hotwords_file,
            open_hot_rules_file,
            hide_overlay,
        ])
        .build(tauri::generate_context!())
        .expect("build Voice IME");
    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { .. } => {
            if let Some(state) = app.try_state::<AppState>() {
                state.graceful_shutdown(app, "exit-requested");
            }
        }
        tauri::RunEvent::Exit => {
            if let Some(state) = app.try_state::<AppState>() {
                state.graceful_shutdown(app, "exit");
            }
        }
        _ => {}
    });
}

pub fn run_cli_worker_if_requested() -> bool {
    panic_log::install();
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
    if first == std::ffi::OsStr::new("--benchmark-asr-profile") {
        let Some(profile) = args.next() else {
            eprintln!("missing ASR profile, expected fast, balanced, fallback, or accurate");
            std::process::exit(2);
        };
        let profile = profile.to_string_lossy().to_string();
        let samples_dir = args
            .next()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("benchmarks/asr"));
        if let Err(err) = benchmark::run_asr_cli_with_profile(samples_dir, Some(&profile)) {
            eprintln!("{err:?}");
            std::process::exit(2);
        }
        return true;
    }
    if first == std::ffi::OsStr::new("--write-asr-benchmark-template") {
        let samples_dir = args
            .next()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("benchmarks/asr"));
        if let Err(err) = benchmark::write_sample_template_cli(samples_dir) {
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
    if first == std::ffi::OsStr::new("--benchmark-translation-profile") {
        let Some(profile) = args.next() else {
            eprintln!("missing translation profile, expected fast, balanced, accurate, or custom");
            std::process::exit(2);
        };
        let profile = profile.to_string_lossy().to_string();
        let samples_path = args.next().map(std::path::PathBuf::from);
        if let Err(err) =
            translation_benchmark::run_translation_cli_with_profile(&profile, samples_path)
        {
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
            let config = config::load_or_create(&paths)?;
            let report = model_pack::install(&pack_path, &paths, &config)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(())
        })();
        if let Err(err) = result {
            eprintln!("{err:?}");
            std::process::exit(2);
        }
        return true;
    }
    if first == std::ffi::OsStr::new("--shutdown-smoke") {
        let result = (|| -> anyhow::Result<()> {
            let state = AppState::load()?;
            let report = state.shutdown_for_cli("cli-shutdown-smoke");
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.history_flushed {
                anyhow::bail!(
                    "shutdown history flush failed: {}",
                    report.history_flush_error.unwrap_or_default()
                );
            }
            Ok(())
        })();
        if let Err(err) = result {
            eprintln!("{err:?}");
            std::process::exit(2);
        }
        return true;
    }
    if first == std::ffi::OsStr::new("--panic-smoke") {
        panic!("cli-panic-smoke");
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
    let mut seen = HashMap::new();
    for (name, shortcut, action) in shortcuts {
        let normalized = normalize_shortcut(&shortcut);
        if normalized.trim().is_empty() || normalized.eq_ignore_ascii_case("off") {
            checks.push(HotkeyCheck {
                name: name.into(),
                shortcut,
                normalized,
                status: HotkeyCheckStatus::Warn,
                detail: "已关闭".into(),
                suggestion: hotkey_disabled_suggestion(name),
            });
            continue;
        }
        let dedupe_key = normalized.to_ascii_lowercase();
        if let Some(previous) = seen.get(&dedupe_key) {
            let detail = format!("和 {previous} 重复");
            let suggestion = hotkey_duplicate_suggestion(&normalized);
            failed.push(format!("{name} {normalized}: {detail}"));
            checks.push(HotkeyCheck {
                name: name.into(),
                shortcut,
                normalized,
                status: HotkeyCheckStatus::Fail,
                detail,
                suggestion,
            });
            continue;
        }
        seen.insert(dedupe_key, name.to_string());
        match register_hotkey(app, &normalized, action) {
            Ok(()) => checks.push(HotkeyCheck {
                name: name.into(),
                shortcut,
                normalized,
                status: HotkeyCheckStatus::Pass,
                detail: "已注册".into(),
                suggestion: String::new(),
            }),
            Err(err) => {
                let detail = hotkey_failure_detail(&err);
                let suggestion = hotkey_failure_suggestion(&err);
                failed.push(format!("{name} {normalized}: {detail}"));
                checks.push(HotkeyCheck {
                    name: name.into(),
                    shortcut,
                    normalized,
                    status: HotkeyCheckStatus::Fail,
                    detail,
                    suggestion,
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
            detail: hotkey_doctor_detail(&check),
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

fn hotkey_disabled_suggestion(name: &str) -> String {
    format!("需要 {name} 时，点键盘按钮录入一个新组合后保存")
}

fn hotkey_duplicate_suggestion(normalized: &str) -> String {
    format!(
        "给其中一个动作换成未使用组合，例如 Ctrl+Alt+{}，或关闭不常用动作",
        hotkey_key_hint(normalized)
    )
}

fn hotkey_key_hint(normalized: &str) -> &str {
    normalized
        .rsplit('+')
        .next()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .unwrap_or("R")
}

fn hotkey_failure_detail(error: &str) -> String {
    if hotkey_error_looks_unrecognized(error) {
        format!("热键格式不可识别：{}", compact_hotkey_error(error))
    } else if hotkey_error_looks_taken(error) {
        format!("可能被系统或其他软件占用：{}", compact_hotkey_error(error))
    } else {
        compact_hotkey_error(error)
    }
}

fn hotkey_failure_suggestion(error: &str) -> String {
    if hotkey_error_looks_unrecognized(error) {
        "点击键盘按钮重新录入；避免 AltRight、KeyE 这类物理键名，使用 Alt+R 或 Ctrl+Alt+R".into()
    } else if hotkey_error_looks_taken(error) {
        "先用窗口按钮继续；换成 Ctrl+Alt+字母，或关闭占用该组合的软件后保存".into()
    } else {
        "先用窗口按钮继续；换一个组合后保存，诊断会重新检查".into()
    }
}

fn hotkey_doctor_detail(check: &HotkeyCheck) -> String {
    if check.suggestion.trim().is_empty() {
        format!("{}；{}", check.normalized, check.detail)
    } else {
        format!(
            "{}；{}；{}",
            check.normalized, check.detail, check.suggestion
        )
    }
}

fn compact_hotkey_error(error: &str) -> String {
    let error = error.trim().replace(['\r', '\n'], " ");
    if error.chars().count() <= 160 {
        error
    } else {
        let mut compact = error.chars().take(157).collect::<String>();
        compact.push_str("...");
        compact
    }
}

fn hotkey_error_looks_unrecognized(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("couldn't recognize")
        || lower.contains("valid key")
        || lower.contains("unrecognized")
        || lower.contains("invalid key")
}

fn hotkey_error_looks_taken(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("already registered")
        || lower.contains("already in use")
        || lower.contains("taken")
        || lower.contains("占用")
        || lower.contains("已注册")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkey_guidance_explains_unrecognized_keys() {
        let error = "Couldn't recognize \"AltRight\" as a valid key for hotkey";

        assert!(hotkey_failure_detail(error).contains("热键格式不可识别"));
        assert!(hotkey_failure_suggestion(error).contains("重新录入"));
        assert!(hotkey_failure_suggestion(error).contains("Alt+R"));
    }

    #[test]
    fn hotkey_guidance_names_duplicate_action() {
        assert_eq!(hotkey_key_hint("Ctrl+Alt+J"), "J");
        assert!(hotkey_duplicate_suggestion("Ctrl+Alt+J").contains("Ctrl+Alt+J"));
        assert!(hotkey_disabled_suggestion("转英文").contains("转英文"));
    }

    #[test]
    fn hotkey_doctor_detail_includes_suggestion_when_available() {
        let check = HotkeyCheck {
            name: "录音".into(),
            shortcut: "Alt+R".into(),
            normalized: "Alt+R".into(),
            status: HotkeyCheckStatus::Fail,
            detail: "可能被系统或其他软件占用".into(),
            suggestion: "换一个组合".into(),
        };

        assert_eq!(
            hotkey_doctor_detail(&check),
            "Alt+R；可能被系统或其他软件占用；换一个组合"
        );
    }

    #[test]
    fn asr_benchmark_profile_overrides_only_valid_profiles() {
        let mut config = AppConfig::default();
        config.asr.profile = "balanced".into();

        assert_eq!(
            benchmark_config_for_profile(config.clone(), Some("fast"))
                .asr
                .profile,
            "fast"
        );
        assert_eq!(
            benchmark_config_for_profile(config.clone(), Some("accurate"))
                .asr
                .profile,
            "accurate"
        );
        assert_eq!(
            benchmark_config_for_profile(config.clone(), Some("bad"))
                .asr
                .profile,
            "balanced"
        );
        assert_eq!(benchmark_profile_label(None), "当前档位");
        assert_eq!(benchmark_profile_label(Some("fallback")), "fallback");
        assert_eq!(benchmark_profile_label(Some("accurate")), "accurate");
    }
}
