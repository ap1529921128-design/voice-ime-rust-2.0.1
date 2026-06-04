# Voice IME 2.0.1 Acceptance

## Basic Input

1. Run `启动语音输入.bat` from the portable root.
2. The main GUI window appears.
3. The input-level meter appears in the main input view and moves when speaking into the selected microphone.
4. Click the large microphone button or press `Alt+R` to start recording.
5. Speak a short Chinese sentence.
6. Click the stop button or press `Alt+R` again to stop.
7. Text appears in the overlay or main confirmation editor.
8. Confirm input pastes into the focused target and does not send.

## Clipboard-Safe Paste

1. Copy a known text value into the clipboard before confirming input.
2. Confirm Voice IME text into Notepad or another focused text field.
3. The recognized text is pasted into the target without sending Enter.
4. The original text clipboard is restored after paste where Windows allows it.
5. `input-target-YYYYMMDD.log` records `input_method`, `send_input_events`, `clipboard_restored`, and `clipboard_restore_error`.

## Direct Input Fallback

1. If clipboard paste fails and the text is short single-line plain text, Voice IME attempts Unicode direct typing.
2. Multi-line text, tabbed text, empty text, and long text do not use direct typing fallback.
3. Direct typing fallback does not send Enter.
4. `input-target-YYYYMMDD.log` records `input_method=direct-type-fallback` when this path is used.

## Audio Device

1. Open Settings / Voice.
2. The microphone select lists system default and enumerated input devices.
3. Pick a non-default microphone, save settings, and restart the app.
4. The chosen microphone remains selected and recording uses that device.
5. The meter reports peak/rms/sample-rate values without starting a full recording.

## Smart Edit

1. Put `这个判断很准，输入法的边界就是不要替我说话。` in the editor.
2. Record `帮我改得更正式一点`.
3. If MiniCPM is reachable, the existing text is rewritten.
4. If MiniCPM is unavailable, the original editor text is retained.

## Long Transcript

1. Record longer than `long_transcript_seconds`.
2. If Settings / Data / long recording retention is enabled, audio is copied to `.voice_ime/recordings`.
3. Status shows segmented long transcription progress.
4. Clear cancels the current session and stale results cannot overwrite the next session.

## Retention

1. Open Settings / Data.
2. `短录音留存` is shown as disabled and `永不保存`.
3. Set `长录音留存` to `不保存`, save settings, and complete a long transcription.
4. The long transcription still produces text, but no new long-recording file is kept.
5. Click `清理录音`; existing audio files directly under `.voice_ime/recordings` are removed.

## Package

1. Portable root visibly contains one user-facing file: `启动语音输入.bat`.
2. Runtime `.voice_ime` data is not included in the release.
3. Hidden `app` folder contains `VoiceIME.exe`, `BUILD.txt`, README, acceptance notes, 2.0.1 roadmap, optional local model/runtime folders, and bundled Tauri frontend resources inside the exe.
4. Packaging fails if the portable root contains unexpected visible files, `.voice_ime`, `recordings`, `backup`, or `backups` directories.
5. Core package `app/models` contains only `MODELS.json` and `MODELS.md`.

## History Export

1. Complete at least one transcription so History has a row.
2. Open History and click `导出 CSV`.
3. A `history-export-YYYYMMDD-HHMMSS.csv` file appears under `.voice_ime/logs`.
4. The CSV includes final text, raw ASR, deterministic stages, LLM text, backend, model, session id, and timing columns.
5. Text that starts with spreadsheet formula trigger characters is exported safely for table tools.

## Diagnostics Panel

1. Open Settings / Data and click `诊断`.
2. A diagnostics panel appears in the page with pass, warning, or failure rows.
3. The rows cover app/log paths, microphone, clipboard, ASR models, smart-correction endpoint, translation backend, prompt, correction table, hotwords, and hot rules.
4. A `doctor-YYYYMMDD-HHMMSS.txt` report path is shown in the panel and the file exists under `.voice_ime/logs`.
5. Clicking `导出` still creates the support zip and includes the latest doctor output without including recordings or model binaries.
6. `app/tools/启动语音输入-诊断.bat` exists in packaged builds and runs `VoiceIME.exe --doctor` without adding another visible root launcher.

## Model Path Picker

1. Open Settings / Models.
2. Each ASR profile row has `下载`, `选择`, `镜像`, and `官网` actions.
3. Clicking `选择` opens a native directory picker and fills the matching default filenames for that profile.
4. Each individual model path has a file-picker icon that updates only that config field.
5. After selecting a directory or file, the config is saved and the ready/missing rows refresh.

## Hotkey Status

1. Open Settings / Shortcuts.
2. The page shows a hotkey status panel with one row for recording, language switching, English translation, and Japanese translation.
3. Duplicated shortcuts or shortcuts already taken by another app show as failure rows instead of preventing GUI startup.
4. Change a shortcut and click save; the app re-registers global hotkeys immediately without restart.
5. Running Settings / Data / `诊断` includes the hotkey rows in the diagnostics panel.

## ASR Benchmark

1. Prepare a directory of `.wav` files and optional same-name `.txt` expected transcripts.
2. Run `app\VoiceIME.exe --benchmark-asr <samples-dir>` from a portable package.
3. An `asr-benchmark-YYYYMMDD-HHMMSS.csv` file appears under `.voice_ime/logs`.
4. The CSV includes file, duration, profile, worker mode, backend, model, transcribe seconds, realtime factor, expected text, transcript text, and error.
5. If the sample directory is missing or empty, the command still writes a CSV row with `no wav samples found`.

## Model Pack Script

1. Prepare a models directory containing the required files for one profile from `packaging/model-manifest.json`.
2. Run `packaging/package-model-pack.ps1 -Profile <profile> -SourceModelsDir <models-dir> -OutputRoot <out-dir>`.
3. A `voice-ime-model-pack-<id>.zip` file is created.
4. The zip contains `app/models/...`, `app/models/MODELS.json`, and `MODEL_PACK.txt`.
5. Missing required files fail the script before a model pack is produced.

## UI Smoke

1. Run `npm run ui:smoke`.
2. The command starts a local Vite QA page with mocked Tauri data.
3. Main compose, Settings / Models, Settings / Shortcuts, History, and Overlay render at multiple viewport/device-scale combinations.
4. The command fails on outer page scroll, shell viewport overflow, or overflowing button/control text.
5. Screenshots are written under `work/ui-smoke/`.

## Notepad Input Acceptance

1. Build or unpack a portable package.
2. From the package root, run `powershell -NoProfile -ExecutionPolicy Bypass -File .\app\tools\Notepad-Input-Acceptance.ps1`.
3. The script opens Notepad, focuses it, runs `VoiceIME.exe --paste-foreground <text> 80`, copies Notepad content back, and compares it with the expected text.
4. A `notepad-acceptance-YYYYMMDD-HHMMSS.txt` report appears under `app/.voice_ime/logs`.
5. The same run also appends an `input-target-YYYYMMDD.log` row with the captured target process, window class, paste method, `SendInput` count, and clipboard restore status.
6. This is an automated smoke for Notepad only; WeChat/Feishu, Word/document editors, and IDE input boxes still need manual target-machine acceptance.

## Browser Input Acceptance

1. Build or unpack a portable package.
2. From the package root, run `powershell -NoProfile -ExecutionPolicy Bypass -File .\app\tools\Browser-Input-Acceptance.ps1`.
3. The script launches Microsoft Edge or Google Chrome with an isolated temporary user profile and opens a local textarea page.
4. It focuses the browser text area, runs `VoiceIME.exe --paste-foreground <text> 80`, and verifies the pasted value through the page window title.
5. A `browser-acceptance-YYYYMMDD-HHMMSS.txt` report appears under `app/.voice_ime/logs`.
6. The browser profile and temporary page are deleted after the run; existing user browser profiles are not modified.

## Current 2.0.1 Test Boundary

- Automated regression covers Rust unit tests, Rust compile, clippy, frontend build, release build, and portable packaging.
- Startup smoke test covers that `VoiceIME.exe` stays alive for 5 seconds after launch instead of panicking before GUI startup. Smoke tests must use a temporary `VOICE_IME_APP_DIR` so they do not write `.voice_ime` into the portable package.
- ASR smoke now covers `balanced`, `fast`, and `fallback` as worker subprocesses. If sherpa-onnx exits badly, the GUI should show an error instead of closing.
- Empty ASR output must not call MiniCPM; prompt-like MiniCPM output containing "个人词表", "纠错表", or "ASR 文本" must be discarded.
- Translation must translate the current editor text only; prompt-like translation output must be discarded.
- Portable release must not open a console window for `VoiceIME.exe`; local llama-server is launched hidden.
- Manual Windows integration still needs a real pass on WeChat/Feishu, Word/document editors, and IDE input boxes; Notepad and Edge/Chrome textarea paste have automated smoke scripts.
- Real ASR acceptance requires sherpa-onnx model files matching the 2.0.1 default config. The copied 1.1.5 `faster-whisper-small` folder is reference material only and does not satisfy the new sherpa-onnx model paths.
- Each missing model row in Settings has clickable download, mirror page, official page, and model-folder actions. The downloader tries `hf-mirror.com` first, then `huggingface.co`.
- Settings shows download progress/failure in an in-panel notice, not only in the title status chip.
- Settings / Voice now exposes microphone selection and the main input view shows a pre-recording input meter; real multi-device manual coverage is still required on the target machines.
- History CSV export is automated and unit-tested for escaping; real spreadsheet review still depends on manual sample data from target machines.
- Long recording retention can be disabled and existing long recordings can be cleared from Settings / Data; short recordings remain non-retained by design.
- Portable packaging now includes an automated layout/release gate and `BUILD.txt`; manual smoke is still useful after packaging because it proves WebView startup on this machine.
- Settings / Data now shows an inline diagnostics panel after running Doctor; one-click repair actions are still future work.
- Settings / Models now has native file and directory pickers; real removable-drive acceptance should still be tested on target machines.
- Settings / Shortcuts now shows global-hotkey registration status and re-registers after save; manual conflict coverage is still required with real third-party apps.
- `--benchmark-asr` now provides a repeatable timing CSV harness; real quality scoring still depends on recorded sample audio.
- Confirm paste now restores previous text clipboard where feasible and logs restore status; manual image/file clipboard preservation is still future work.
- Clipboard failure can now fall back to direct Unicode typing for short single-line text; broad app coverage still needs manual acceptance.
- `npm run ui:smoke` now covers main/settings/history/overlay layout with QA mock data; true OS DPI and WebView screenshots still need manual release checks.
- Packaged builds now include `app/tools/启动语音输入-诊断.bat`; portable root layout still visibly exposes only the main launcher.
- Packaged builds now include `app/tools/Notepad-Input-Acceptance.ps1`; Notepad has an automated paste-path smoke, while other real apps still need manual coverage.
- Packaged builds now include `app/tools/Browser-Input-Acceptance.ps1`; Edge/Chrome textarea paste has an automated smoke with an isolated temporary browser profile.
