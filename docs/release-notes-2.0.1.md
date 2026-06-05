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
   - `导入包`: import a downloaded `voice-ime-model-pack-*.zip`.
   - `下载`: download the files for that profile.
   - `选择`: point the profile to a model folder on another drive.
4. Confirm that Settings / Models shows the ASR profile as ready.
5. Hold `CapsLock` or mouse `X2`, speak, then release to transcribe.

The app never sends Enter automatically. Confirmed text is pasted into the target app; if the caret cannot be located, the main confirmation box remains the fallback.

## Verification

The release gate passed on the build machine for:

- full and core startup smoke
- `VoiceIME.exe --doctor`
- Notepad paste acceptance
- Edge/Chrome textarea paste acceptance
- external translation JSON-pipeline acceptance
- core package model-pack import acceptance with SHA-256 verification

Real target-machine checks are still needed for WeChat/Feishu, Word/document editors, IDEs, multiple microphones, and long recordings.
