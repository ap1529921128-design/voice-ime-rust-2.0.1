# Voice IME Rust 2.0.1

Voice IME Rust 2.0.1 is the first hardened portable release after the Rust/Tauri rewrite. It focuses on a usable Windows dictation loop: local ASR, cursor overlay or main confirmation box, safe paste after confirmation, push-to-talk, model packs, diagnostics, and a calmer glass UI.

## Which File To Download

- `voice-ime-2.0.1-rust-portable.zip`: full test package with the current local model cache. Use this on the main test machine.
- `voice-ime-2.0.1-rust-portable-core.zip`: light app body without large model binaries. Use this for removable-drive or clean-machine tests, then import model packs.
- `voice-ime-model-pack-asr-balanced-sensevoice-int8.zip`: recommended default ASR model pack.
- `voice-ime-model-pack-asr-fast-zipformer-ctc-int8.zip`: faster Chinese short-dictation ASR model pack.
- `voice-ime-model-pack-asr-fallback-whisper-tiny-int8.zip`: small compatibility fallback ASR model pack.
- `voice-ime-model-pack-llm-minicpm5-1b-q4.zip`: optional local correction/rewrite/temporary translation model pack.
- `voice-ime-model-packs-2.0.1.json` / `.md`: checksum manifest for the model packs.

## How To Run

1. Extract the zip.
2. Double-click `启动语音输入.bat`.
3. If ASR models are missing, open Settings / Models and choose one of:
   - `模型根目录`: point the app to a shared model repository, for example on a removable drive.
   - `导入包`: import a downloaded `voice-ime-model-pack-*.zip`.
   - `下载`: download the files for that profile.
   - `选择`: point the profile to a model folder on another drive.
4. Confirm that Settings / Models shows the ASR profile as ready.
5. Hold `CapsLock` or mouse `X2`, speak, then release to transcribe. A short `CapsLock` tap still passes through as CapsLock.

The app never sends Enter automatically. Confirmed text is pasted into the target app; if the caret cannot be located, the main confirmation box remains the fallback.

2.0.1 now retries focus recovery before Ctrl+V, briefly shows an `已粘贴` state before hiding the overlay, and writes focus/clipboard diagnostics into `input-target-YYYYMMDD.log`. Text clipboard contents are restored where feasible; non-text clipboard formats are logged clearly instead of being reported as restored.

Task-style background workers now run with unwind panic guards in release builds. Recording/ASR, translation, model download, overlay cleanup, cancellation cleanup, prewarm, and benchmark panics report a UI error and append JSON rows to `worker-error-YYYYMMDD.log` instead of silently killing the task.

Exit paths now run a graceful shutdown. Tray quit and Tauri exit events cancel active recording, invalidate stale sessions, hide overlay state, flush history, and append `shutdown-YYYYMMDD.log`; the portable release gate checks the same core path with `VoiceIME.exe --shutdown-smoke`.

Startup and uncaught thread panics now leave `panic-YYYYMMDD.log` entries with payload, source location, thread name, and backtrace. Doctor surfaces recent panic/worker/shutdown logs, and the portable release gate checks the packaged path with `VoiceIME.exe --panic-smoke`.

## Verification

The release gate passed on the build machine for:

- full and core startup smoke
- `VoiceIME.exe --doctor`
- Notepad paste acceptance
- Edge/Chrome textarea paste acceptance
- external translation JSON-pipeline acceptance
- core package model-pack import acceptance with SHA-256 verification

Real target-machine checks are still needed for WeChat/Feishu, Word/document editors, IDEs, multiple microphones, and long recordings.

Model/app separation now supports `VOICE_IME_MODEL_DIR` and Settings / Models / `模型根目录`. Relative `models/...` paths are resolved under that effective model root, so a core app body can be moved between machines while ASR and MiniCPM model packs stay in one external repository.
