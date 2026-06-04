# Changelog

## Unreleased

- Added a persistent ASR worker mode that keeps a hidden subprocess alive and reuses the loaded recognizer for the same profile, reducing repeated model cold-start cost.
- Kept the previous isolated per-request ASR worker as a settings option and automatic fallback if the persistent worker fails.
- Exposed the ASR worker mode in Settings as "常驻加速" and "隔离稳妥".
- Added a versioned model manifest and a core portable package strategy so the app body can be tested and upgraded separately from large ASR/LLM model packs.
- Added `hot.txt` alias replacement and `hot-rule.txt` regex replacement, with Settings buttons to open both files.
- Added optional push-to-talk recording with CapsLock and mouse X2 hold triggers, plus Settings controls for trigger key, mouse button, and event suppression.
- Added input-target logging for confirmed paste operations, recording target process, class, title, caret source, result, and paste timing under `.voice_ime/logs`.
- Added a lightweight doctor report command and Settings button for local path, microphone, clipboard, ASR model, LLM endpoint, and user text-file checks.
- Added built-in per-app input profiles for common chat, browser, document, and IDE targets, currently applying safer paste delays and logging the matched profile.
- Added deterministic ITN for common Chinese numbers, percentages, money, dates, times, ranges, and units, plus per-app short-sentence period removal.
- Added a tray menu for showing the main window, toggling recording, opening model/log/hotword files, running doctor, and exiting; closing the main window now hides it to tray.
- Added a translation engine abstraction with `llm` and `external` backends, plus Settings and Doctor support, so dedicated local MT tools can be used without routing through MiniCPM prompts.
- Added raw ASR, deterministic correction stages, LLM final text, and stage timing fields to transcript history so accuracy and latency can be diagnosed from one record.
- Split Settings into voice, models, smart input, shortcuts, and data groups, with editable ASR model paths and hotkey fields.
- Added a one-click support bundle export that zips config, history, dictionaries, logs, doctor output, and model manifests while excluding recordings and model binaries.
- Added ASR idle prewarming for persistent worker mode, plus a Settings model-page prewarm button, so the current ready profile can load before the first real dictation.
- Added history filters for text/stage content, backend, model, and date so trace records can be located quickly during accuracy and latency debugging.
- Added a real backend `Cancelling` state and worker-update token checks so clear/re-record actions ignore stale ASR, correction, translation, and error results.
- Added persisted microphone selection and a pre-recording peak/rms input meter in the main input view and Settings / Voice, with audio-device probing serialized against real recording startup.
- Added one-click history CSV export from History and Settings / Data, including raw ASR, deterministic stages, LLM text, model/backend, session id, and timing columns.
- Added Settings / Data retention controls for long recordings, a visible never-save short-recording policy, and a clear-recordings action for stored long audio files.
- Added portable packaging release gates for root layout, forbidden runtime folders, core-model cleanliness, required app files, and a generated `BUILD.txt` build stamp.
- Added an inline Settings / Data diagnostics panel showing pass, warning, and failure rows while still writing a shareable doctor report file.
- Added native model file and profile-directory pickers in Settings / Models for external or removable model packs.
- Added visible global-hotkey registration status in Settings / Shortcuts and Doctor, and re-register hotkeys immediately after saving settings.
- Added a CLI ASR benchmark harness that scans wav samples and writes timing, realtime factor, model, transcript, expected text, and errors to CSV.
- Added a model-pack packaging script that creates standalone `voice-ime-model-pack-*.zip` archives from the manifest and an existing models directory.
- Confirm paste now checks `SendInput` delivery, restores the previous text clipboard where feasible, and logs clipboard restoration fields in input-target logs.
- Added a conservative direct Unicode typing fallback for short single-line text when clipboard paste fails.
- Added a Playwright-based `npm run ui:smoke` QA harness with Tauri mock data, viewport/DPI checks, and screenshots for the main window, settings, history, and overlay.
- Added a packaged `app/tools/启动语音输入-诊断.bat` helper while keeping the portable root limited to the single main launcher.
- Added a packaged `app/tools/Notepad-Input-Acceptance.ps1` smoke test that exercises the same foreground-window paste path as confirm input.
- Added a packaged `app/tools/Browser-Input-Acceptance.ps1` smoke test for Edge/Chrome text areas using an isolated temporary browser profile.

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
