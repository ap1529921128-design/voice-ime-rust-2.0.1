# Changelog

## Unreleased

- Added a persistent ASR worker mode that keeps a hidden subprocess alive and reuses the loaded recognizer for the same profile, reducing repeated model cold-start cost.
- Kept the previous isolated per-request ASR worker as a settings option and automatic fallback if the persistent worker fails.
- Exposed the ASR worker mode in Settings as "常驻加速" and "隔离稳妥".
- Added a versioned model manifest and a core portable package strategy so the app body can be tested and upgraded separately from large ASR/LLM model packs.

## 2.0.1 - 2026-05-31

- Bumped app, Tauri, Cargo, window, and portable package version from 2.0.0 to 2.0.1.
- Added explicit launch, build, packaging, test-status, and known-risk documentation.
- Added a 100-point optimization backlog for the next hardening pass.
- Changed portable packaging output to `D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable` so the user's backed-up 2.0.0 package is not overwritten.
- Added clickable ASR model download, mirror-page, official-page, and model-folder actions in Settings.
- Model downloads try `hf-mirror.com` first and then fall back to `huggingface.co`.
- Normalized legacy ASR model paths to the current Hugging Face file names.
- Fixed portable packaging so it runs the Tauri production build instead of copying a cargo-only exe that could open `127.0.0.1:1420`.
- Moved ASR decoding into a worker subprocess so native sherpa-onnx failures cannot close the GUI.
- Stopped passing the personal prompt as sherpa-onnx hotwords by default, and added the required whisper tokens path to fallback readiness checks.
- Added an in-settings status notice and clearer model action buttons so download progress/failure is visible in the GUI.
- Fixed smart correction calling MiniCPM on empty ASR output, which could leak the internal prompt as "please confirm" text.
- Added prompt-leak filtering so MiniCPM responses containing personal prompt/correction table/ASR prompt markers fall back to raw ASR text.
- Built release as a Windows GUI subsystem executable so double-click launch no longer opens a black console window.
- Tightened translation prompts and discard prompt-like translation outputs instead of inserting personal-wordlist/confirmation text.
- Patched the bundled MiniCPM startup script to launch llama-server hidden instead of minimized.
- Stopped the cursor overlay from stealing focus, hides it after confirm paste, and debounced text sync so mouse selection and typing no longer trigger full UI redraws.
- Lowered the default ASR thread count to 2 and exposed the thread setting for weaker PCs.
- Removed outer WebView scrolling and the default browser scrollbar from the main UI, keeping scrolling contained inside settings/history only.
- Shows raw ASR text immediately after decoding, then runs smart correction as a post-processing update so local LLM startup cannot make transcription appear blank.
- Fixed the packaged MiniCPM launcher root path so the script copied under `app/tools` still resolves models from the portable `app` directory.
- Removed clipped outer CSS shadows and added a native rounded Windows region so the UI no longer shows a faint rectangular WebView edge; native shadows stay enabled for Windows startup stability.
- Hardened translation cleanup so labels like "翻译结果：" and explanation sections are stripped or rejected, and translating already-Chinese text to Chinese returns the original text.
- Made translation non-blocking, capped the default translation timeout to 8 seconds, reduced short-phrase token budgets, and starts MiniCPM in the background so a cold local model cannot freeze the UI.

## 2.0.0 - 2026-05-31

- Initial Rust/Tauri rewrite scaffold with Rust audio, sherpa-onnx ASR path, local LLM correction/translation path, cursor overlay, confirmation paste, settings, and history.
