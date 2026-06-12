#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    if voice_ime_lib::run_cli_worker_if_requested() {
        return;
    }
    voice_ime_lib::run();
}
