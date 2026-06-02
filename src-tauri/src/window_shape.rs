#[cfg(target_os = "windows")]
use tauri::{Manager, WindowEvent};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn, HGDIOBJ};

#[cfg(target_os = "windows")]
pub fn install<R: tauri::Runtime + 'static>(app: &tauri::App<R>) {
    for (label, radius) in [("main", 30.0), ("overlay", 24.0)] {
        let Some(window) = app.get_webview_window(label) else {
            continue;
        };
        apply_rounded_region(&window, radius);

        let tracked = window.clone();
        window.on_window_event(move |event| {
            if matches!(
                event,
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
            ) {
                apply_rounded_region(&tracked, radius);
            }
        });
    }
}

#[cfg(not(target_os = "windows"))]
pub fn install<R: tauri::Runtime>(_app: &tauri::App<R>) {}

#[cfg(target_os = "windows")]
fn apply_rounded_region<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>, radius: f64) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let Ok(size) = window.inner_size() else {
        return;
    };
    if size.width == 0 || size.height == 0 {
        return;
    }

    let scale = window.scale_factor().unwrap_or(1.0).max(1.0);
    let corner = (radius * scale * 2.0).round().clamp(2.0, 240.0) as i32;
    let width = size.width.min(i32::MAX as u32) as i32;
    let height = size.height.min(i32::MAX as u32) as i32;

    unsafe {
        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, corner, corner);
        if region.is_null() {
            return;
        }
        if SetWindowRgn(hwnd.0 as _, region, 1) == 0 {
            let _ = DeleteObject(region as HGDIOBJ);
        }
    }
}
