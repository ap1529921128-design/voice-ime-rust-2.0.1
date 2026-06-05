use crate::{
    config::{AppConfig, InputConfig},
    core::AppState,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        OnceLock, RwLock,
    },
    thread,
};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PttSource {
    Keyboard,
    Mouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PttEvent {
    Pressed(PttSource),
    Released(PttSource),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HookSettings {
    enabled: bool,
    key_code: Option<u32>,
    mouse_button: Option<u16>,
    suppress: bool,
    hold_threshold_ms: u64,
}

impl HookSettings {
    fn from_input(input: &InputConfig) -> Self {
        Self {
            enabled: input.ptt_enabled,
            key_code: key_code(&input.ptt_key),
            mouse_button: mouse_button(&input.ptt_mouse_button),
            suppress: input.ptt_suppress,
            hold_threshold_ms: input.ptt_hold_threshold_ms,
        }
    }
}

struct HookRuntime {
    sender: Sender<PttEvent>,
    settings: RwLock<HookSettings>,
}

static RUNTIME: OnceLock<HookRuntime> = OnceLock::new();
static KEY_IS_DOWN: AtomicBool = AtomicBool::new(false);
static KEY_RECORDING_ACTIVE: AtomicBool = AtomicBool::new(false);
static KEY_PRESS_ID: AtomicU64 = AtomicU64::new(0);
static MOUSE_IS_DOWN: AtomicBool = AtomicBool::new(false);

pub fn install(app: &AppHandle, config: &AppConfig) {
    let settings = HookSettings::from_input(&config.input);
    let runtime = RUNTIME.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        spawn_action_thread(app.clone(), receiver);
        spawn_hook_thread();
        HookRuntime {
            sender,
            settings: RwLock::new(settings.clone()),
        }
    });
    update_settings(settings);
    let _ = runtime;
}

pub fn update_config(config: &AppConfig) {
    update_settings(HookSettings::from_input(&config.input));
}

fn update_settings(settings: HookSettings) {
    KEY_IS_DOWN.store(false, Ordering::SeqCst);
    KEY_RECORDING_ACTIVE.store(false, Ordering::SeqCst);
    KEY_PRESS_ID.fetch_add(1, Ordering::SeqCst);
    MOUSE_IS_DOWN.store(false, Ordering::SeqCst);
    if let Some(runtime) = RUNTIME.get() {
        if let Ok(mut guard) = runtime.settings.write() {
            *guard = settings;
        }
    }
}

fn spawn_action_thread(app: AppHandle, receiver: Receiver<PttEvent>) {
    let _ = thread::Builder::new()
        .name("voice-ime-ptt-action".into())
        .spawn(move || {
            let mut active_source = None;
            for event in receiver {
                let Some(state) = app.try_state::<AppState>() else {
                    continue;
                };
                match event {
                    PttEvent::Pressed(source) if active_source.is_none() => {
                        active_source = Some(source);
                        if let Err(err) = state.start_recording(&app) {
                            active_source = None;
                            state.set_runtime_notice(
                                &app,
                                "按住说话不可用",
                                format!("启动录音失败：{err}"),
                            );
                        }
                    }
                    PttEvent::Released(source) if active_source == Some(source) => {
                        active_source = None;
                        if let Err(err) = state.stop_recording(&app) {
                            state.set_runtime_notice(
                                &app,
                                "按住说话不可用",
                                format!("停止录音失败：{err}"),
                            );
                        }
                    }
                    _ => {}
                }
            }
        });
}

fn send_event(event: PttEvent) {
    if let Some(runtime) = RUNTIME.get() {
        let _ = runtime.sender.send(event);
    }
}

fn current_settings() -> Option<HookSettings> {
    RUNTIME
        .get()
        .and_then(|runtime| runtime.settings.read().ok().map(|guard| guard.clone()))
}

fn key_code(value: &str) -> Option<u32> {
    match value.trim().to_ascii_lowercase().as_str() {
        "capslock" => Some(vk::CAPS_LOCK),
        "f8" => Some(vk::F8),
        "f9" => Some(vk::F9),
        "f10" => Some(vk::F10),
        "f13" => Some(vk::F13),
        _ => None,
    }
}

fn mouse_button(value: &str) -> Option<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "x1" => Some(mouse::X1),
        "x2" => Some(mouse::X2),
        _ => None,
    }
}

fn should_passthrough_short_key(key_code: Option<u32>) -> bool {
    key_code == Some(vk::CAPS_LOCK)
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{
        current_settings, send_event, should_passthrough_short_key, PttEvent, PttSource,
        KEY_IS_DOWN, KEY_PRESS_ID, KEY_RECORDING_ACTIVE, MOUSE_IS_DOWN,
    };
    use std::{mem::MaybeUninit, ptr::null_mut, sync::atomic::Ordering, thread, time::Duration};
    use windows_sys::Win32::{
        Foundation::{LPARAM, LRESULT, WPARAM},
        UI::{
            Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
            },
            WindowsAndMessaging::{
                CallNextHookEx, GetMessageW, SetWindowsHookExW, HHOOK, KBDLLHOOKSTRUCT, MSG,
                MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
                WM_SYSKEYUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
            },
        },
    };

    const LLKHF_INJECTED: u32 = 0x10;

    pub fn spawn_hook_thread() {
        let _ = thread::Builder::new()
            .name("voice-ime-ptt-hook".into())
            .spawn(move || unsafe {
                let keyboard_hook =
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), null_mut(), 0);
                let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), null_mut(), 0);
                if keyboard_hook.is_null() && mouse_hook.is_null() {
                    return;
                }

                let _keyboard_guard = HookGuard(keyboard_hook);
                let _mouse_guard = HookGuard(mouse_hook);
                let mut message = MaybeUninit::<MSG>::zeroed().assume_init();
                while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {}
            });
    }

    struct HookGuard(HHOOK);

    impl Drop for HookGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(self.0);
                }
            }
        }
    }

    unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            if let Some(settings) = current_settings() {
                if settings.enabled {
                    let keyboard = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
                    if keyboard.flags & LLKHF_INJECTED != 0 {
                        return unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) };
                    }
                    if settings.key_code == Some(keyboard.vkCode) {
                        match wparam as u32 {
                            WM_KEYDOWN | WM_SYSKEYDOWN => {
                                if !KEY_IS_DOWN.swap(true, Ordering::SeqCst) {
                                    KEY_RECORDING_ACTIVE.store(false, Ordering::SeqCst);
                                    let press_id = KEY_PRESS_ID.fetch_add(1, Ordering::SeqCst) + 1;
                                    if settings.hold_threshold_ms == 0 {
                                        KEY_RECORDING_ACTIVE.store(true, Ordering::SeqCst);
                                        send_event(PttEvent::Pressed(PttSource::Keyboard));
                                    } else {
                                        let threshold =
                                            Duration::from_millis(settings.hold_threshold_ms);
                                        thread::spawn(move || {
                                            thread::sleep(threshold);
                                            if KEY_IS_DOWN.load(Ordering::SeqCst)
                                                && KEY_PRESS_ID.load(Ordering::SeqCst) == press_id
                                            {
                                                KEY_RECORDING_ACTIVE.store(true, Ordering::SeqCst);
                                                send_event(PttEvent::Pressed(PttSource::Keyboard));
                                            }
                                        });
                                    }
                                }
                            }
                            WM_KEYUP | WM_SYSKEYUP if KEY_IS_DOWN.swap(false, Ordering::SeqCst) => {
                                KEY_PRESS_ID.fetch_add(1, Ordering::SeqCst);
                                if KEY_RECORDING_ACTIVE.swap(false, Ordering::SeqCst) {
                                    send_event(PttEvent::Released(PttSource::Keyboard));
                                } else if settings.suppress
                                    && should_passthrough_short_key(settings.key_code)
                                {
                                    send_key_tap(keyboard.vkCode);
                                }
                            }
                            _ => {}
                        }
                        if settings.suppress {
                            return 1;
                        }
                    }
                }
            }
        }
        unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
    }

    fn send_key_tap(key_code: u32) {
        let inputs = [
            keyboard_input(key_code as VIRTUAL_KEY, false),
            keyboard_input(key_code as VIRTUAL_KEY, true),
        ];
        unsafe {
            let _ = SendInput(
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

    unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            if let Some(settings) = current_settings() {
                if settings.enabled {
                    let mouse_event = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
                    let button = (mouse_event.mouseData >> 16) as u16;
                    if settings.mouse_button == Some(button) {
                        match wparam as u32 {
                            WM_XBUTTONDOWN => {
                                if !MOUSE_IS_DOWN.swap(true, Ordering::SeqCst) {
                                    send_event(PttEvent::Pressed(PttSource::Mouse));
                                }
                            }
                            WM_XBUTTONUP if MOUSE_IS_DOWN.swap(false, Ordering::SeqCst) => {
                                send_event(PttEvent::Released(PttSource::Mouse));
                            }
                            _ => {}
                        }
                        if settings.suppress {
                            return 1;
                        }
                    }
                }
            }
        }
        unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    pub fn spawn_hook_thread() {}
}

use platform::spawn_hook_thread;

mod vk {
    pub const CAPS_LOCK: u32 = windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_CAPITAL as u32;
    pub const F8: u32 = windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_F8 as u32;
    pub const F9: u32 = windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_F9 as u32;
    pub const F10: u32 = windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_F10 as u32;
    pub const F13: u32 = windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_F13 as u32;
}

mod mouse {
    pub const X1: u16 = windows_sys::Win32::UI::WindowsAndMessaging::XBUTTON1;
    pub const X2: u16 = windows_sys::Win32::UI::WindowsAndMessaging::XBUTTON2;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InputConfig;

    #[test]
    fn maps_configured_triggers() {
        let input = InputConfig {
            ptt_enabled: true,
            ptt_key: "CapsLock".into(),
            ptt_mouse_button: "X2".into(),
            ptt_suppress: true,
            ..InputConfig::default()
        };
        let settings = HookSettings::from_input(&input);
        assert!(settings.enabled);
        assert_eq!(settings.key_code, Some(vk::CAPS_LOCK));
        assert_eq!(settings.mouse_button, Some(mouse::X2));
        assert!(settings.suppress);
        assert_eq!(settings.hold_threshold_ms, 180);
    }

    #[test]
    fn off_disables_individual_triggers() {
        let input = InputConfig {
            ptt_key: "off".into(),
            ptt_mouse_button: "off".into(),
            ..InputConfig::default()
        };
        let settings = HookSettings::from_input(&input);
        assert_eq!(settings.key_code, None);
        assert_eq!(settings.mouse_button, None);
    }

    #[test]
    fn only_capslock_short_press_passes_through() {
        assert!(should_passthrough_short_key(Some(vk::CAPS_LOCK)));
        assert!(!should_passthrough_short_key(Some(vk::F8)));
        assert!(!should_passthrough_short_key(None));
    }
}
