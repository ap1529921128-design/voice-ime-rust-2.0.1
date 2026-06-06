use anyhow::{anyhow, Result};
use arboard::Clipboard;
use serde::Serialize;
use std::{path::Path, thread, time::Duration};
use uiautomation::patterns::{UITextPattern, UITextRange};
use uiautomation::types::{ControlType, Rect as UiRect};
use uiautomation::UIAutomation;
use windows::Win32::System::{
    Com::SAFEARRAY,
    Ole::{
        SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound,
        SafeArrayGetUBound, SafeArrayGetVartype,
    },
    Variant::VT_R8,
};
use windows_sys::Win32::Foundation::{CloseHandle, HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::DataExchange::{
    CountClipboardFormats, GetClipboardSequenceNumber, IsClipboardFormatAvailable,
};
use windows_sys::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, SetActiveWindow, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_CONTROL, VK_MENU, VK_V,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, SetForegroundWindow, ShowWindow, SwitchToThisWindow,
    GUITHREADINFO, SW_RESTORE,
};

const OVERLAY_WIDTH: i32 = 480;
const OVERLAY_HEIGHT: i32 = 242;
const OVERLAY_GAP: i32 = 12;
const OVERLAY_MARGIN: i32 = 16;

const CF_TEXT_FORMAT: u32 = 1;
const CF_BITMAP_FORMAT: u32 = 2;
const CF_DIB_FORMAT: u32 = 8;
const CF_UNICODETEXT_FORMAT: u32 = 13;
const CF_HDROP_FORMAT: u32 = 15;
const CF_DIBV5_FORMAT: u32 = 17;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct OverlayRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputTargetInfo {
    pub hwnd: usize,
    pub thread_id: u32,
    pub process_id: u32,
    pub rect: Option<OverlayRect>,
    pub process_name: String,
    pub process_path: String,
    pub class_name: String,
    pub title: String,
    pub caret_source: String,
}

#[derive(Debug, Clone)]
pub struct InputTarget {
    hwnd: HWND,
    rect: Option<OverlayRect>,
    info: InputTargetInfo,
}

#[derive(Debug, Clone)]
pub struct PasteOutcome {
    pub method: &'static str,
    pub send_input_events: u32,
    pub focus_attempts: u32,
    pub focus_restored: bool,
    pub clipboard_previous_had_text: bool,
    pub clipboard_previous_format: &'static str,
    pub clipboard_format_count: u32,
    pub clipboard_sequence_before: u32,
    pub clipboard_sequence_after: u32,
    pub clipboard_restored: bool,
    pub clipboard_restore_error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct ClipboardSnapshot {
    sequence: u32,
    format_count: u32,
    has_text: bool,
    format_hint: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct FocusOutcome {
    attempts: u32,
    restored: bool,
}

unsafe impl Send for InputTarget {}
unsafe impl Sync for InputTarget {}

impl InputTarget {
    pub fn capture() -> Self {
        let hwnd = unsafe { GetForegroundWindow() };
        let (rect, caret_source) = match caret_rect_uia() {
            Some((rect, source)) => (Some(rect), source),
            None => match caret_rect_gui_thread() {
                Some(rect) => (Some(rect), "gui-thread"),
                None => (None, "fallback"),
            },
        };
        let info = target_info(hwnd, caret_source, rect);
        Self { hwnd, rect, info }
    }

    pub fn from_hwnd(hwnd: HWND, caret_source: &'static str) -> Self {
        let info = target_info(hwnd, caret_source, None);
        Self {
            hwnd,
            rect: None,
            info,
        }
    }

    pub fn rect(&self) -> Option<OverlayRect> {
        self.rect
    }

    pub fn info(&self) -> &InputTargetInfo {
        &self.info
    }

    pub fn paste_text(&self, text: &str, delay_ms: u64) -> Result<PasteOutcome> {
        match self.paste_via_clipboard(text, delay_ms) {
            Ok(outcome) => Ok(outcome),
            Err(err) if can_direct_type(text) => {
                let focus = self.focus_with_retry();
                if !focus.restored {
                    return Err(anyhow!("无法恢复目标窗口焦点，已取消直接输入：{err}"));
                }
                let send_input_events = send_unicode_text(text)?;
                Ok(PasteOutcome {
                    method: "direct-type-fallback",
                    send_input_events,
                    focus_attempts: focus.attempts,
                    focus_restored: focus.restored,
                    clipboard_previous_had_text: false,
                    clipboard_previous_format: "unknown",
                    clipboard_format_count: 0,
                    clipboard_sequence_before: 0,
                    clipboard_sequence_after: clipboard_sequence_number(),
                    clipboard_restored: false,
                    clipboard_restore_error: Some(format!("剪贴板粘贴失败，已直接输入：{err}")),
                })
            }
            Err(err) => Err(err),
        }
    }

    fn paste_via_clipboard(&self, text: &str, delay_ms: u64) -> Result<PasteOutcome> {
        let clipboard_before = clipboard_snapshot();
        let mut clipboard = Clipboard::new().map_err(|err| anyhow!("剪贴板不可用：{err}"))?;
        let previous_text = clipboard.get_text().ok();
        clipboard
            .set_text(text.to_string())
            .map_err(|err| anyhow!("写入剪贴板失败：{err}"))?;
        if delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
        let focus = self.focus_with_retry();
        if !focus.restored {
            let _ = restore_clipboard_text(
                &mut clipboard,
                previous_text.as_deref(),
                text,
                clipboard_before,
            );
            return Err(anyhow!("无法恢复目标窗口焦点，已取消粘贴"));
        }
        let send_input_events = match send_ctrl_v() {
            Ok(events) => events,
            Err(err) => {
                let _ = restore_clipboard_text(
                    &mut clipboard,
                    previous_text.as_deref(),
                    text,
                    clipboard_before,
                );
                return Err(err);
            }
        };
        thread::sleep(Duration::from_millis(160));
        let (clipboard_restored, clipboard_restore_error) = restore_clipboard_text(
            &mut clipboard,
            previous_text.as_deref(),
            text,
            clipboard_before,
        );
        Ok(PasteOutcome {
            method: "clipboard-paste",
            send_input_events,
            focus_attempts: focus.attempts,
            focus_restored: focus.restored,
            clipboard_previous_had_text: previous_text.is_some() || clipboard_before.has_text,
            clipboard_previous_format: clipboard_before.format_hint,
            clipboard_format_count: clipboard_before.format_count,
            clipboard_sequence_before: clipboard_before.sequence,
            clipboard_sequence_after: clipboard_sequence_number(),
            clipboard_restored,
            clipboard_restore_error,
        })
    }

    fn focus_with_retry(&self) -> FocusOutcome {
        if self.hwnd.is_null() {
            return FocusOutcome {
                attempts: 0,
                restored: false,
            };
        }
        for attempt in 1..=4 {
            unsafe {
                restore_foreground_window(self.hwnd);
            }
            thread::sleep(Duration::from_millis(45 * attempt as u64));
            if unsafe { GetForegroundWindow() } == self.hwnd {
                return FocusOutcome {
                    attempts: attempt,
                    restored: true,
                };
            }
        }
        FocusOutcome {
            attempts: 4,
            restored: false,
        }
    }
}

unsafe fn restore_foreground_window(hwnd: HWND) {
    ShowWindow(hwnd, SW_RESTORE);

    let current_thread = GetCurrentThreadId();
    let foreground = GetForegroundWindow();
    let foreground_thread = window_thread_id(foreground);
    let target_thread = window_thread_id(hwnd);
    let attach_foreground = foreground_thread != 0 && foreground_thread != current_thread;
    let attach_target = target_thread != 0 && target_thread != current_thread;

    if attach_foreground {
        AttachThreadInput(current_thread, foreground_thread, 1);
    }
    if attach_target {
        AttachThreadInput(current_thread, target_thread, 1);
    }

    send_foreground_unlock_alt_tap();
    BringWindowToTop(hwnd);
    SetForegroundWindow(hwnd);
    SetActiveWindow(hwnd);
    SetFocus(hwnd);
    SwitchToThisWindow(hwnd, 1);

    if attach_target {
        AttachThreadInput(current_thread, target_thread, 0);
    }
    if attach_foreground {
        AttachThreadInput(current_thread, foreground_thread, 0);
    }
}

unsafe fn window_thread_id(hwnd: HWND) -> u32 {
    if hwnd.is_null() {
        return 0;
    }
    let mut process_id = 0;
    GetWindowThreadProcessId(hwnd, &mut process_id)
}

unsafe fn send_foreground_unlock_alt_tap() {
    let inputs = [
        keyboard_input(VK_MENU, false),
        keyboard_input(VK_MENU, true),
    ];
    SendInput(
        inputs.len() as u32,
        inputs.as_ptr(),
        std::mem::size_of::<INPUT>() as i32,
    );
}

fn restore_clipboard_text(
    clipboard: &mut Clipboard,
    previous_text: Option<&str>,
    pasted_text: &str,
    snapshot: ClipboardSnapshot,
) -> (bool, Option<String>) {
    let Some(previous_text) = previous_text else {
        let message = if snapshot.format_count > 0 && !snapshot.has_text {
            format!(
                "原剪贴板不是文本（{}），当前版本不能恢复图片/文件等非文本内容",
                snapshot.format_hint
            )
        } else {
            "原剪贴板没有可恢复文本".to_string()
        };
        return (false, Some(message));
    };
    if previous_text == pasted_text {
        return (true, None);
    }
    match clipboard.set_text(previous_text.to_string()) {
        Ok(()) => (true, None),
        Err(err) => (false, Some(err.to_string())),
    }
}

fn clipboard_snapshot() -> ClipboardSnapshot {
    let sequence = clipboard_sequence_number();
    let format_count = unsafe { CountClipboardFormats() }.max(0) as u32;
    let has_unicode_text = unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT_FORMAT) != 0 };
    let has_ansi_text = unsafe { IsClipboardFormatAvailable(CF_TEXT_FORMAT) != 0 };
    let has_files = unsafe { IsClipboardFormatAvailable(CF_HDROP_FORMAT) != 0 };
    let has_image = unsafe {
        IsClipboardFormatAvailable(CF_DIBV5_FORMAT) != 0
            || IsClipboardFormatAvailable(CF_DIB_FORMAT) != 0
            || IsClipboardFormatAvailable(CF_BITMAP_FORMAT) != 0
    };
    ClipboardSnapshot {
        sequence,
        format_count,
        has_text: has_unicode_text || has_ansi_text,
        format_hint: clipboard_format_hint(
            format_count,
            has_unicode_text,
            has_ansi_text,
            has_files,
            has_image,
        ),
    }
}

fn clipboard_sequence_number() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

fn clipboard_format_hint(
    format_count: u32,
    has_unicode_text: bool,
    has_ansi_text: bool,
    has_files: bool,
    has_image: bool,
) -> &'static str {
    if format_count == 0 {
        return "empty";
    }
    if has_unicode_text {
        return "unicode-text";
    }
    if has_ansi_text {
        return "text";
    }
    if has_files {
        return "files";
    }
    if has_image {
        return "image";
    }
    "non-text"
}

fn target_info(hwnd: HWND, caret_source: &str, rect: Option<OverlayRect>) -> InputTargetInfo {
    let mut process_id = 0;
    let thread_id = if hwnd.is_null() {
        0
    } else {
        unsafe { GetWindowThreadProcessId(hwnd, &mut process_id) }
    };
    let process_path = process_path(process_id);
    InputTargetInfo {
        hwnd: hwnd as usize,
        thread_id,
        process_id,
        rect,
        process_name: process_name(&process_path),
        process_path,
        class_name: window_class_name(hwnd),
        title: window_title(hwnd),
        caret_source: caret_source.to_string(),
    }
}

fn window_title(hwnd: HWND) -> String {
    if hwnd.is_null() {
        return String::new();
    }
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        wide_to_string(&buffer, copied)
    }
}

fn window_class_name(hwnd: HWND) -> String {
    if hwnd.is_null() {
        return String::new();
    }
    unsafe {
        let mut buffer = vec![0u16; 256];
        let copied = GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        wide_to_string(&buffer, copied)
    }
}

fn process_path(process_id: u32) -> String {
    if process_id == 0 {
        return String::new();
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if handle.is_null() {
            return String::new();
        }
        let mut buffer = vec![0u16; 32_768];
        let mut size = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size) != 0;
        CloseHandle(handle);
        if ok {
            String::from_utf16_lossy(&buffer[..size as usize])
        } else {
            String::new()
        }
    }
}

fn process_name(process_path: &str) -> String {
    Path::new(process_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

fn wide_to_string(buffer: &[u16], copied: i32) -> String {
    if copied <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..copied as usize])
            .trim()
            .to_string()
    }
}

pub fn overlay_position_from_rect(rect: Option<OverlayRect>) -> OverlayRect {
    let desired = overlay_position_unclamped(rect);
    let anchor = rect.unwrap_or(desired);
    if let Some(bounds) = monitor_work_area(anchor) {
        overlay_position_in_bounds(rect, bounds)
    } else {
        desired
    }
}

fn overlay_position_unclamped(rect: Option<OverlayRect>) -> OverlayRect {
    let Some(rect) = rect else {
        return OverlayRect {
            x: 120,
            y: 120,
            width: OVERLAY_WIDTH,
            height: OVERLAY_HEIGHT,
        };
    };
    OverlayRect {
        x: rect.x.max(OVERLAY_MARGIN),
        y: (rect.y + rect.height + OVERLAY_GAP).max(OVERLAY_MARGIN),
        width: OVERLAY_WIDTH,
        height: OVERLAY_HEIGHT,
    }
}

fn overlay_position_in_bounds(rect: Option<OverlayRect>, bounds: OverlayRect) -> OverlayRect {
    let mut overlay = overlay_position_unclamped(rect);
    if let Some(caret) = rect {
        let bottom_limit = bounds.y + bounds.height - OVERLAY_MARGIN;
        let below_bottom = overlay.y + overlay.height;
        let above_y = caret.y - overlay.height - OVERLAY_GAP;
        if below_bottom > bottom_limit && above_y >= bounds.y + OVERLAY_MARGIN {
            overlay.y = above_y;
        }
    }
    clamp_overlay_to_bounds(overlay, bounds)
}

fn clamp_overlay_to_bounds(overlay: OverlayRect, bounds: OverlayRect) -> OverlayRect {
    let min_x = bounds.x + OVERLAY_MARGIN;
    let min_y = bounds.y + OVERLAY_MARGIN;
    let max_x = bounds.x + bounds.width - overlay.width - OVERLAY_MARGIN;
    let max_y = bounds.y + bounds.height - overlay.height - OVERLAY_MARGIN;
    OverlayRect {
        x: clamp_axis(overlay.x, min_x, max_x),
        y: clamp_axis(overlay.y, min_y, max_y),
        width: overlay.width,
        height: overlay.height,
    }
}

fn clamp_axis(value: i32, min: i32, max: i32) -> i32 {
    if max < min {
        min
    } else {
        value.clamp(min, max)
    }
}

fn monitor_work_area(anchor: OverlayRect) -> Option<OverlayRect> {
    unsafe {
        let point = POINT {
            x: anchor.x,
            y: anchor.y,
        };
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            rcWork: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            dwFlags: 0,
        };
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        Some(rect_from_win_rect(info.rcWork))
    }
}

fn rect_from_win_rect(rect: RECT) -> OverlayRect {
    OverlayRect {
        x: rect.left,
        y: rect.top,
        width: (rect.right - rect.left).max(1),
        height: (rect.bottom - rect.top).max(1),
    }
}

fn caret_rect_uia() -> Option<(OverlayRect, &'static str)> {
    let automation = UIAutomation::new().ok()?;
    let element = automation.get_focused_element().ok()?;
    let focused_rect = focused_element_rect(
        element.get_control_type().ok(),
        element.get_bounding_rectangle().ok(),
    );
    if let Ok(text_pattern) = element.get_pattern::<UITextPattern>() {
        if let Ok((_active, range)) = text_pattern.get_caret_range() {
            if let Some(rect) = caret_rect_from_text_range(&range) {
                return Some((rect, "uia-caret"));
            }
            if let Ok(enclosing) = range.get_enclosing_element() {
                if let Some(rect) = enclosing
                    .get_bounding_rectangle()
                    .ok()
                    .and_then(valid_ui_rect)
                {
                    return Some((rect, "uia-element"));
                }
            }
        }
    }
    focused_rect.map(|rect| (rect, "uia-focused"))
}

fn valid_ui_rect(rect: UiRect) -> Option<OverlayRect> {
    let left = rect.get_left();
    let top = rect.get_top();
    let width = rect.get_width().max(1);
    let height = rect.get_height().max(1);
    if left <= -32_000 || top <= -32_000 || width <= 1 || height <= 1 {
        None
    } else {
        Some(OverlayRect {
            x: left,
            y: top,
            width: width.min(48),
            height: height.min(48),
        })
    }
}

fn focused_element_rect(
    control_type: Option<ControlType>,
    rect: Option<UiRect>,
) -> Option<OverlayRect> {
    let control_type = control_type?;
    if !matches!(
        control_type,
        ControlType::Edit | ControlType::Document | ControlType::Custom
    ) {
        return None;
    }
    valid_focused_ui_rect(rect?)
}

fn valid_focused_ui_rect(rect: UiRect) -> Option<OverlayRect> {
    let width = rect.get_width();
    let height = rect.get_height();
    if width > 1200 || height > 700 {
        return None;
    }
    valid_ui_rect(rect)
}

fn caret_rect_from_text_range(range: &UITextRange) -> Option<OverlayRect> {
    let values = bounding_rect_values(range)?;
    caret_rect_from_bounding_values(&values)
}

fn bounding_rect_values(range: &UITextRange) -> Option<Vec<f64>> {
    let array = unsafe { range.as_ref().GetBoundingRectangles().ok()? };
    if array.is_null() {
        return None;
    }
    let values = unsafe { safe_array_f64_values(array) };
    unsafe {
        let _ = SafeArrayDestroy(array);
    }
    values
}

unsafe fn safe_array_f64_values(array: *mut SAFEARRAY) -> Option<Vec<f64>> {
    if SafeArrayGetDim(array) != 1 || SafeArrayGetVartype(array).ok()? != VT_R8 {
        return None;
    }
    let lower = SafeArrayGetLBound(array, 1).ok()?;
    let upper = SafeArrayGetUBound(array, 1).ok()?;
    if upper < lower {
        return None;
    }
    let mut values = Vec::with_capacity((upper - lower + 1) as usize);
    for index in lower..=upper {
        let indices = [index];
        let mut value = 0.0_f64;
        SafeArrayGetElement(
            array,
            indices.as_ptr(),
            (&mut value as *mut f64).cast::<std::ffi::c_void>(),
        )
        .ok()?;
        values.push(value);
    }
    Some(values)
}

fn caret_rect_from_bounding_values(values: &[f64]) -> Option<OverlayRect> {
    values
        .chunks_exact(4)
        .filter_map(|chunk| valid_uia_caret_rect(chunk[0], chunk[1], chunk[2], chunk[3]))
        .min_by_key(|rect| i64::from(rect.width) * i64::from(rect.height))
}

fn valid_uia_caret_rect(left: f64, top: f64, width: f64, height: f64) -> Option<OverlayRect> {
    if !left.is_finite() || !top.is_finite() || !width.is_finite() || !height.is_finite() {
        return None;
    }
    if left <= -32_000.0 || top <= -32_000.0 || height <= 1.0 {
        return None;
    }
    Some(OverlayRect {
        x: left.round() as i32,
        y: top.round() as i32,
        width: (width.round() as i32).clamp(2, 48),
        height: (height.round() as i32).clamp(18, 64),
    })
}

fn caret_rect_gui_thread() -> Option<OverlayRect> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let thread_id = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
        if thread_id == GetCurrentThreadId() {
            return None;
        }
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            flags: 0,
            hwndActive: std::ptr::null_mut(),
            hwndFocus: std::ptr::null_mut(),
            hwndCapture: std::ptr::null_mut(),
            hwndMenuOwner: std::ptr::null_mut(),
            hwndMoveSize: std::ptr::null_mut(),
            hwndCaret: std::ptr::null_mut(),
            rcCaret: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
        };
        if GetGUIThreadInfo(thread_id, &mut info) == 0 || info.hwndCaret.is_null() {
            return None;
        }
        let mut point = POINT {
            x: info.rcCaret.left,
            y: info.rcCaret.top,
        };
        if ClientToScreen(info.hwndCaret, &mut point) == 0 {
            return None;
        }
        let width = (info.rcCaret.right - info.rcCaret.left).max(2);
        let height = (info.rcCaret.bottom - info.rcCaret.top).max(18);
        Some(OverlayRect {
            x: point.x,
            y: point.y,
            width,
            height,
        })
    }
}

fn send_ctrl_v() -> Result<u32> {
    unsafe {
        let inputs = [
            keyboard_input(VK_CONTROL, false),
            keyboard_input(VK_V, false),
            keyboard_input(VK_V, true),
            keyboard_input(VK_CONTROL, true),
        ];
        let sent = SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
        if sent != inputs.len() as u32 {
            return Err(anyhow!(
                "发送 Ctrl+V 失败：{} / {} 个输入事件",
                sent,
                inputs.len()
            ));
        }
        Ok(sent)
    }
}

fn can_direct_type(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty()
        && text.chars().count() <= 160
        && !text.chars().any(|ch| matches!(ch, '\r' | '\n' | '\t'))
}

fn send_unicode_text(text: &str) -> Result<u32> {
    let mut inputs = Vec::new();
    for unit in text.encode_utf16() {
        inputs.push(unicode_input(unit, false));
        inputs.push(unicode_input(unit, true));
    }
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent != inputs.len() as u32 {
        return Err(anyhow!(
            "直接输入失败：{} / {} 个输入事件",
            sent,
            inputs.len()
        ));
    }
    Ok(sent)
}

fn unicode_input(unit: u16, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE | if key_up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn keyboard_input(vk: VIRTUAL_KEY, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if key_up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_type_allows_short_plain_text() {
        assert!(can_direct_type("非洲之星和海洋之泪"));
        assert!(can_direct_type("hello 123"));
    }

    #[test]
    fn direct_type_rejects_multiline_or_long_text() {
        assert!(!can_direct_type("第一行\n第二行"));
        assert!(!can_direct_type(&"长".repeat(161)));
        assert!(!can_direct_type("   "));
    }

    #[test]
    fn uia_caret_rect_accepts_zero_width_insertion_point() {
        let rect = caret_rect_from_bounding_values(&[100.2, 200.4, 0.0, 17.6]).unwrap();
        assert_eq!(rect.x, 100);
        assert_eq!(rect.y, 200);
        assert_eq!(rect.width, 2);
        assert_eq!(rect.height, 18);
    }

    #[test]
    fn uia_caret_rect_chooses_smallest_valid_rect() {
        let rect =
            caret_rect_from_bounding_values(&[10.0, 20.0, 300.0, 80.0, 120.0, 240.0, 0.0, 22.0])
                .unwrap();
        assert_eq!(rect.x, 120);
        assert_eq!(rect.y, 240);
        assert_eq!(rect.width, 2);
        assert_eq!(rect.height, 22);
    }

    #[test]
    fn uia_caret_rect_rejects_offscreen_or_empty_values() {
        assert!(caret_rect_from_bounding_values(&[]).is_none());
        assert!(caret_rect_from_bounding_values(&[-40_000.0, 10.0, 2.0, 18.0]).is_none());
        assert!(caret_rect_from_bounding_values(&[10.0, 10.0, 2.0, 0.5]).is_none());
    }

    #[test]
    fn overlay_position_clamps_to_right_edge() {
        let bounds = OverlayRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };
        let caret = OverlayRect {
            x: 760,
            y: 120,
            width: 2,
            height: 22,
        };

        let overlay = overlay_position_in_bounds(Some(caret), bounds);

        assert_eq!(overlay.x, 304);
        assert_eq!(overlay.y, 154);
        assert_eq!(overlay.width, OVERLAY_WIDTH);
        assert_eq!(overlay.height, OVERLAY_HEIGHT);
    }

    #[test]
    fn overlay_position_flips_above_when_bottom_would_overflow() {
        let bounds = OverlayRect {
            x: 0,
            y: 0,
            width: 1000,
            height: 600,
        };
        let caret = OverlayRect {
            x: 300,
            y: 520,
            width: 2,
            height: 22,
        };

        let overlay = overlay_position_in_bounds(Some(caret), bounds);

        assert_eq!(overlay.x, 300);
        assert_eq!(overlay.y, 266);
    }

    #[test]
    fn overlay_position_handles_tiny_bounds() {
        let bounds = OverlayRect {
            x: 50,
            y: 40,
            width: 300,
            height: 180,
        };

        let overlay = overlay_position_in_bounds(None, bounds);

        assert_eq!(overlay.x, 66);
        assert_eq!(overlay.y, 56);
        assert_eq!(overlay.width, OVERLAY_WIDTH);
        assert_eq!(overlay.height, OVERLAY_HEIGHT);
    }

    #[test]
    fn focused_rect_accepts_edit_document_and_custom_controls() {
        let rect = Some(UiRect::new(20, 30, 320, 160));
        assert!(focused_element_rect(Some(ControlType::Edit), rect).is_some());
        assert!(focused_element_rect(Some(ControlType::Document), rect).is_some());
        assert!(focused_element_rect(Some(ControlType::Custom), rect).is_some());
    }

    #[test]
    fn focused_rect_rejects_non_text_or_large_controls() {
        assert!(focused_element_rect(
            Some(ControlType::Window),
            Some(UiRect::new(20, 30, 320, 160))
        )
        .is_none());
        assert!(
            focused_element_rect(Some(ControlType::Edit), Some(UiRect::new(0, 0, 1800, 900)))
                .is_none()
        );
    }

    #[test]
    fn clipboard_hint_prefers_text_then_files_then_image() {
        assert_eq!(
            clipboard_format_hint(0, false, false, false, false),
            "empty"
        );
        assert_eq!(
            clipboard_format_hint(2, true, false, true, true),
            "unicode-text"
        );
        assert_eq!(clipboard_format_hint(2, false, true, true, true), "text");
        assert_eq!(clipboard_format_hint(1, false, false, true, false), "files");
        assert_eq!(clipboard_format_hint(1, false, false, false, true), "image");
        assert_eq!(
            clipboard_format_hint(1, false, false, false, false),
            "non-text"
        );
    }
}
