use crate::clipboard;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::{thread, time::Duration};

const OVERLAY_WIDTH: i32 = 480;
const OVERLAY_HEIGHT: i32 = 242;
const OVERLAY_GAP: i32 = 12;
const OVERLAY_MARGIN: i32 = 16;

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

impl InputTarget {
    pub fn capture() -> Self {
        let rect = None;
        let info = InputTargetInfo {
            hwnd: 0,
            thread_id: 0,
            process_id: 0,
            rect,
            process_name: platform_name().into(),
            process_path: String::new(),
            class_name: "unsupported-input-target".into(),
            title: "Voice IME platform fallback".into(),
            caret_source: "platform-fallback".into(),
        };
        Self { rect, info }
    }

    pub fn rect(&self) -> Option<OverlayRect> {
        self.rect
    }

    pub fn info(&self) -> &InputTargetInfo {
        &self.info
    }

    pub fn paste_text(&self, text: &str, delay_ms: u64) -> Result<PasteOutcome> {
        if text.trim().is_empty() {
            return Err(anyhow!("没有可输入的文本"));
        }
        let status = clipboard::set_text_with_status(text)?;
        if delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
        Ok(PasteOutcome {
            method: "clipboard-only-fallback",
            send_input_events: 0,
            focus_attempts: 0,
            focus_restored: false,
            clipboard_previous_had_text: status.previous_had_text,
            clipboard_previous_format: status.previous_format,
            clipboard_format_count: 0,
            clipboard_sequence_before: 0,
            clipboard_sequence_after: 0,
            clipboard_restored: false,
            clipboard_restore_error: Some(
                "当前平台尚未实现目标窗口粘贴，已改为复制到剪贴板".into(),
            ),
        })
    }
}

pub fn overlay_position_from_rect(rect: Option<OverlayRect>) -> OverlayRect {
    overlay_position_in_bounds(
        rect,
        OverlayRect {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        },
    )
}

fn overlay_position_in_bounds(rect: Option<OverlayRect>, bounds: OverlayRect) -> OverlayRect {
    let mut x = rect.map(|rect| rect.x).unwrap_or(bounds.x + OVERLAY_MARGIN);
    let mut y = rect
        .map(|rect| rect.y + rect.height + OVERLAY_GAP)
        .unwrap_or(bounds.y + OVERLAY_MARGIN);
    let max_x = bounds.x + bounds.width - OVERLAY_WIDTH - OVERLAY_MARGIN;
    let max_y = bounds.y + bounds.height - OVERLAY_HEIGHT - OVERLAY_MARGIN;
    if x > max_x {
        x = max_x;
    }
    if x < bounds.x + OVERLAY_MARGIN {
        x = bounds.x + OVERLAY_MARGIN;
    }
    if y > max_y {
        if let Some(rect) = rect {
            y = rect.y - OVERLAY_HEIGHT - OVERLAY_GAP;
        }
    }
    if y < bounds.y + OVERLAY_MARGIN {
        y = bounds.y + OVERLAY_MARGIN;
    }
    OverlayRect {
        x,
        y,
        width: OVERLAY_WIDTH,
        height: OVERLAY_HEIGHT,
    }
}

fn platform_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else {
        "non-windows"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_position_uses_safe_default_bounds() {
        let overlay = overlay_position_from_rect(None);

        assert_eq!(overlay.x, 16);
        assert_eq!(overlay.y, 16);
        assert_eq!(overlay.width, OVERLAY_WIDTH);
        assert_eq!(overlay.height, OVERLAY_HEIGHT);
    }
}
