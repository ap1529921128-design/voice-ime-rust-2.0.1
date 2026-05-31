use anyhow::{anyhow, Result};
use arboard::Clipboard;
use serde::Serialize;
use std::{thread, time::Duration};
use uiautomation::patterns::UITextPattern;
use uiautomation::types::Rect as UiRect;
use uiautomation::UIAutomation;
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_CONTROL, VK_V,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, SetForegroundWindow,
    GUITHREADINFO,
};

#[derive(Debug, Clone, Copy, Serialize)]
pub struct OverlayRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct InputTarget {
    hwnd: HWND,
    rect: Option<OverlayRect>,
}

unsafe impl Send for InputTarget {}
unsafe impl Sync for InputTarget {}

impl InputTarget {
    pub fn capture() -> Self {
        let hwnd = unsafe { GetForegroundWindow() };
        let rect = caret_rect_uia().or_else(caret_rect_gui_thread);
        Self { hwnd, rect }
    }

    pub fn rect(&self) -> Option<OverlayRect> {
        self.rect
    }

    pub fn paste_text(&self, text: &str, delay_ms: u64) -> Result<()> {
        let mut clipboard = Clipboard::new().map_err(|err| anyhow!("剪贴板不可用：{err}"))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|err| anyhow!("写入剪贴板失败：{err}"))?;
        if delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
        if !self.hwnd.is_null() {
            unsafe {
                SetForegroundWindow(self.hwnd);
            }
            thread::sleep(Duration::from_millis(80));
        }
        send_ctrl_v();
        Ok(())
    }
}

pub fn overlay_position_from_rect(rect: Option<OverlayRect>) -> OverlayRect {
    let Some(rect) = rect else {
        return OverlayRect {
            x: 120,
            y: 120,
            width: 480,
            height: 242,
        };
    };
    OverlayRect {
        x: rect.x.max(16),
        y: (rect.y + rect.height + 12).max(16),
        width: 480,
        height: 242,
    }
}

fn caret_rect_uia() -> Option<OverlayRect> {
    let automation = UIAutomation::new().ok()?;
    let element = automation.get_focused_element().ok()?;
    let text_pattern: UITextPattern = element.get_pattern().ok()?;
    let (_active, range) = text_pattern.get_caret_range().ok()?;
    let enclosing = range.get_enclosing_element().ok()?;
    let rect = enclosing.get_bounding_rectangle().ok()?;
    valid_ui_rect(rect)
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

fn send_ctrl_v() {
    unsafe {
        let inputs = [
            keyboard_input(VK_CONTROL, false),
            keyboard_input(VK_V, false),
            keyboard_input(VK_V, true),
            keyboard_input(VK_CONTROL, true),
        ];
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
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
