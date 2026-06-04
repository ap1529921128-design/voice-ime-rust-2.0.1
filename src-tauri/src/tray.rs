use crate::{core::AppState, doctor};
use std::fs;
use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager,
};
use tauri_plugin_opener::OpenerExt;

const ID_SHOW: &str = "show";
const ID_RECORD: &str = "record";
const ID_MODELS: &str = "models";
const ID_LOGS: &str = "logs";
const ID_DOCTOR: &str = "doctor";
const ID_HOTWORDS: &str = "hotwords";
const ID_RULES: &str = "rules";
const ID_QUIT: &str = "quit";

pub fn install(app: &mut App) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text(ID_SHOW, "显示主窗口")
        .text(ID_RECORD, "开始/停止录音")
        .separator()
        .text(ID_DOCTOR, "运行诊断")
        .text(ID_MODELS, "模型目录")
        .text(ID_LOGS, "日志目录")
        .text(ID_HOTWORDS, "热词")
        .text(ID_RULES, "规则")
        .separator()
        .text(ID_QUIT, "退出")
        .build()?;

    let mut builder = TrayIconBuilder::with_id("voice-ime")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("Voice IME")
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            ) {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

pub fn hide_main_to_tray(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    if let Some(state) = app.try_state::<AppState>() {
        state.set_runtime_notice(app, "已最小化到托盘", "托盘菜单可重新显示主窗口");
    }
}

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        ID_SHOW => show_main_window(app),
        ID_RECORD => toggle_recording(app),
        ID_MODELS => open_models_dir(app),
        ID_LOGS => open_logs_dir(app),
        ID_DOCTOR => run_doctor(app),
        ID_HOTWORDS => open_hotwords_file(app),
        ID_RULES => open_hot_rules_file(app),
        ID_QUIT => app.exit(0),
        _ => {}
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn toggle_recording(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let result = if state.recorder.is_recording() {
        state.stop_recording(app)
    } else {
        state.start_recording(app)
    };
    if let Err(err) = result {
        state.set_runtime_notice(app, "录音不可用", err.to_string());
    }
}

fn open_models_dir(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let models_dir = state.paths.root_dir.join("models");
    let result = fs::create_dir_all(&models_dir).and_then(|()| {
        app.opener()
            .open_path(models_dir.to_string_lossy().to_string(), None::<&str>)
            .map_err(std::io::Error::other)
    });
    if let Err(err) = result {
        state.set_runtime_notice(app, "打开模型目录失败", err.to_string());
    }
}

fn open_logs_dir(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let result = fs::create_dir_all(&state.paths.logs_dir).and_then(|()| {
        app.opener()
            .open_path(
                state.paths.logs_dir.to_string_lossy().to_string(),
                None::<&str>,
            )
            .map_err(std::io::Error::other)
    });
    if let Err(err) = result {
        state.set_runtime_notice(app, "打开日志目录失败", err.to_string());
    }
}

fn run_doctor(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let snapshot = state.snapshot();
    match doctor::run(&state.paths, &snapshot.config) {
        Ok(report) => {
            state.set_runtime_notice(
                app,
                "诊断完成",
                format!("{}；报告：{}", report.summary, report.output_path),
            );
            open_logs_dir(app);
        }
        Err(err) => {
            state.set_runtime_notice(app, "诊断失败", err.to_string());
        }
    }
}

fn open_hotwords_file(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    open_text_file(
        app,
        &state.paths.hotwords_path,
        "# hot.txt\n",
        "打开热词失败",
    );
}

fn open_hot_rules_file(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    open_text_file(
        app,
        &state.paths.hot_rules_path,
        "# hot-rule.txt\n",
        "打开规则失败",
    );
}

fn open_text_file(app: &AppHandle, path: &std::path::Path, default_body: &str, status: &str) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let result = ensure_text_file(path, default_body).and_then(|()| {
        app.opener()
            .open_path(path.to_string_lossy().to_string(), None::<&str>)
            .map_err(std::io::Error::other)
    });
    if let Err(err) = result {
        state.set_runtime_notice(app, status, err.to_string());
    }
}

fn ensure_text_file(path: &std::path::Path, default_body: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        fs::write(path, default_body)?;
    }
    Ok(())
}
