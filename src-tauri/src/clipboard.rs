use anyhow::{anyhow, Result};

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, Copy)]
pub struct ClipboardWriteStatus {
    pub previous_had_text: bool,
    pub previous_format: &'static str,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn check_available() -> Result<()> {
    arboard::Clipboard::new()
        .map(|_| ())
        .map_err(|err| anyhow!("剪贴板不可用：{err}"))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn check_available() -> Result<()> {
    Err(anyhow!("移动端剪贴板待接入 Tauri/mobile 原生能力"))
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn set_text(text: String) -> Result<()> {
    arboard::Clipboard::new()?
        .set_text(text)
        .map_err(|err| anyhow!("写入剪贴板失败：{err}"))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn set_text(_text: String) -> Result<()> {
    Err(anyhow!("移动端剪贴板待接入 Tauri/mobile 原生能力"))
}

#[cfg(all(
    not(target_os = "windows"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub fn set_text_with_status(text: &str) -> Result<ClipboardWriteStatus> {
    let mut clipboard = arboard::Clipboard::new().map_err(|err| anyhow!("剪贴板不可用：{err}"))?;
    let previous_text = clipboard.get_text().ok();
    clipboard
        .set_text(text.to_string())
        .map_err(|err| anyhow!("写入剪贴板失败：{err}"))?;
    Ok(ClipboardWriteStatus {
        previous_had_text: previous_text.is_some(),
        previous_format: if previous_text.is_some() {
            "text"
        } else {
            "unknown"
        },
    })
}

#[cfg(all(
    not(target_os = "windows"),
    any(target_os = "android", target_os = "ios")
))]
pub fn set_text_with_status(_text: &str) -> Result<ClipboardWriteStatus> {
    Err(anyhow!("移动端剪贴板待接入 Tauri/mobile 原生能力"))
}
