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
9. If the editor already has text, an accidental too-short or silent recording keeps that text and reports the reason in status/meta instead of replacing it.
10. If recording reaches `max_record_seconds`, the status warns shortly before the limit and then automatically stops into the normal transcription path; manually stopped or cleared sessions must not be stopped again by a stale timer.

## Clipboard-Safe Paste

1. Copy a known text value into the clipboard before confirming input.
2. Confirm Voice IME text into Notepad or another focused text field.
3. The recognized text is pasted into the target without sending Enter.
4. The original text clipboard is restored after paste where Windows allows it.
5. `input-target-YYYYMMDD.log` records `input_method`, `send_input_events`, `focus_attempts`, `focus_restored`, `clipboard_previous_format`, `clipboard_previous_had_text`, `clipboard_restored`, and `clipboard_restore_error`.

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
6. After completing a recording, History and exported CSV include source sample rate, ASR sample rate, whether resampling occurred, and any automatic leading/trailing silence trim.

## Smart Edit

1. Put `这个判断很准，输入法的边界就是不要替我说话。` in the editor.
2. Record `帮我改得更正式一点`.
3. If MiniCPM is reachable, the existing text is rewritten.
4. If MiniCPM is unavailable, the original editor text is retained.

## Local LLM Service

1. Open Settings / Smart.
2. Click `检查服务`.
3. The service panel shows endpoint reachability, `llama-server.exe` process status, startup script, MiniCPM model, MiniCPM size, optional sha256 status, and server file rows.
4. Click `启动服务`; if the local script and runtime are present, the app attempts to start `llama-server` hidden and refreshes the same status rows.
5. Running Settings / Data / `诊断` includes a `本地 LLM 文件` row covering the script, model, model size/optional sha256, and server binary.

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
4. The CSV includes final text, raw ASR, deterministic stages, LLM text, backend, model, session id, timing columns, source sample rate, ASR sample rate, resampling status, and leading/trailing silence-trim seconds.
5. Text that starts with spreadsheet formula trigger characters is exported safely for table tools.

## Diagnostics Panel

1. Open Settings / Data and click `诊断`.
2. A diagnostics panel appears in the page with pass, warning, or failure rows.
3. The rows cover app/log paths, microphone, clipboard, ASR models, smart-correction endpoint, local LLM files, translation backend, recent translation logs, hotword/rule stats, prompt, correction table, hotwords, and hot rules.
4. A `doctor-YYYYMMDD-HHMMSS.txt` report path is shown in the panel and the file exists under `.voice_ime/logs`.
5. Clicking `导出` still creates the support zip and includes the latest doctor output without including recordings or model binaries.
6. If the effective external model root has no `MODELS.json/md`, the support zip falls back to the packaged model manifest and records the model root source and manifest source paths in `summary.txt`.
7. `app/tools/启动语音输入-诊断.bat` exists in packaged builds and runs `VoiceIME.exe --doctor` without adding another visible root launcher.

## Conservative Repair

1. Open Settings / Data and click `修复`.
2. Missing `.voice_ime` runtime directories are created.
3. Missing `personal_prompt.txt`, `corrections.json`, `hot.txt`, and `hot-rule.txt` are restored with defaults.
4. If the effective external model root is missing `MODELS.json/md`, repair copies the packaged model manifests there.
5. Existing user files and model manifests are reported as skipped and are not overwritten.
6. The diagnostics panel refreshes after repair and writes a fresh `doctor-YYYYMMDD-HHMMSS.txt` report.
7. Repair does not download models, copy model binaries, change hotkeys, alter existing config values, upload data, or send input to other apps.

## Hotwords And Rules

1. Open Settings / Data.
2. Click `热词` and edit `hot.txt`, using `目标词 | 别名` lines.
3. Click `规则` and edit `hot-rule.txt`, using `regex = replacement` lines.
4. Click `刷新词表`; the panel shows hotword entries, aliases, valid rules, and invalid rule examples.
5. Running Settings / Data / `诊断` includes a `热词规则统计` row and warns when any regex rule is invalid.
6. Enter `mini CPM 非州之星 一千毫安时` in `词表试算` and click `试算`.
7. The preview shows changed stages for built-in corrections, hotwords, rules, and ITN, with final text `minicpm 非洲之星 1000mAh`.
8. Changes apply on the next transcription without restarting the app.

## Model Path Picker

1. Open Settings / Models.
2. The page shows a `模型根目录` field with a native directory picker.
3. Each ASR profile row shows a short description, an expected 10-second latency hint, missing/required filenames, and `下载`, `选择`, `镜像`, and `官网` actions.
4. Clicking `模型根目录` opens a native directory picker, saves `asr.model_root`, and refreshes ready/missing rows against that root.
5. Clicking `选择` opens a native directory picker and fills the matching default filenames for that profile.
6. Each individual model path has a file-picker icon that updates only that config field.
7. After selecting a directory or file, the config is saved and the ready/missing rows refresh.
8. The page shows the active model-root source: default, `asr.model_root`, `MODEL_ROOT.txt`, or `VOICE_IME_MODEL_DIR`.
9. Clicking `写入便携` writes the current model root into `app\MODEL_ROOT.txt`, refreshes model readiness, and makes Doctor, MiniCPM startup, and tray `模型目录` use that effective root when `VOICE_IME_MODEL_DIR` is not already set.
10. Clicking `清除` removes `app\MODEL_ROOT.txt`; if no environment variable is set, the effective source falls back to `asr.model_root` or default `app/models`.
11. `app\models\MODELS.json/md` remains the packaged manifest and repair source even when the effective model root is external.

## Hotkey Status

1. Open Settings / Shortcuts.
2. The page shows a hotkey status panel with one row for recording, language switching, English translation, and Japanese translation.
3. Click a keyboard capture button, press a shortcut combination, and verify the matching input field is filled.
4. Duplicated shortcuts or shortcuts already taken by another app show as failure rows instead of preventing GUI startup.
5. Change a shortcut and click save; the app re-registers global hotkeys immediately without restart.
6. Running Settings / Data / `诊断` includes the hotkey rows in the diagnostics panel.

## Push To Talk

1. Open Settings / Shortcuts and keep push-to-talk enabled with keyboard trigger `CapsLock`.
2. Set `长按阈值` to the default 180 ms and save.
3. Short-tap `CapsLock`; it should toggle CapsLock normally and should not start recording.
4. Hold `CapsLock` longer than the threshold; recording should start, and releasing the key should stop and transcribe.
5. Mouse X1/X2 triggers remain hold-to-record and are not short-tap passed through.

## Input Profiles

1. Open Settings / Input.
2. The page shows the global paste delay and editable app-profile rows for chat, browser, document, and IDE targets.
3. Click `新增策略`, edit the process name, title match, paste delay, and punctuation policy, then save.
4. Click `恢复内置` and verify the built-in WeChat/Feishu/Lark/Word/Chrome/Edge/VS Code/JetBrains rows return.
5. Confirm input into a matching app and check `input-target-YYYYMMDD.log`; the row should include the matched profile name, paste delay, and punctuation policy.
6. The UI smoke suite includes Settings / Input at 150% device scale and must not show outer scrolling or control text overflow.

## ASR Benchmark

1. Prepare a directory of `.wav` files and optional same-name `.txt` expected transcripts.
2. Run `app\VoiceIME.exe --benchmark-asr <samples-dir>` from a portable package, or open Settings / Data and click `ASR 基准` to choose the same sample directory.
3. An `asr-benchmark-YYYYMMDD-HHMMSS.csv` file appears under `.voice_ime/logs`.
4. The CSV includes file, duration, profile, worker mode, backend, model, transcribe seconds, realtime factor, expected text, transcript text, expected character count, edit distance, CER, accuracy, and error.
5. If the sample directory is missing or empty, the command still writes a CSV row with `no wav samples found`.

## Translation Benchmark

1. Run `app\VoiceIME.exe --benchmark-translation` from a portable package, or open Settings / Data and click `翻译基准`.
2. A `translation-benchmark-YYYYMMDD-HHMMSS.csv` file appears under `.voice_ime/logs`.
3. The built-in samples cover `zh`, `en`, and `ja` targets, including translation-label cleanup cases.
4. Custom TSV/CSV samples can be passed as `app\VoiceIME.exe --benchmark-translation <samples-file>`.
5. The CSV includes target language, engine, model, timeout, elapsed seconds, language match, optional hint match, source, output, and error.
6. Backend failures and prompt-like explanatory output are recorded as error rows instead of closing the GUI.
7. Normal GUI translation clicks append `translation-YYYYMMDD.log` rows with engine, model, timeout, elapsed seconds, source/output character counts, status, and error text; Doctor warns when recent rows contain failures or slow requests.

## Translation External Acceptance

1. Build or unpack a portable package.
2. From the package root, run `powershell -NoProfile -ExecutionPolicy Bypass -File .\app\tools\Translation-Acceptance.ps1`.
3. The script creates a temporary `VOICE_IME_APP_DIR`, writes a 2.0 config that selects `translation.engine=external`, and points it at packaged `Mock-External-Translate.ps1`.
4. It runs `VoiceIME.exe --benchmark-translation` with 3 zh/en/ja samples.
5. The benchmark CSV must contain 3 rows, no error values, `language_match=true`, and matching optional hints.
6. The script does not require a real MT model or MiniCPM service and deletes its temporary app data unless `-KeepAppDir` is passed.

## Model Pack Script

1. Prepare a models directory containing the required files for one profile from `packaging/model-manifest.json`.
2. Run `packaging/package-model-pack.ps1 -Profile <profile> -SourceModelsDir <models-dir> -OutputRoot <out-dir>`.
3. A `voice-ime-model-pack-<id>.zip` file is created.
4. The zip contains `app/models/...`, `app/models/MODELS.json`, `MODEL_PACK.txt`, and `MODEL_PACK.json`.
5. Missing required files fail the script before a model pack is produced.

## Model Pack Batch Script

1. Prepare a full portable package or another models directory containing available model files.
2. Run `powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\package-available-model-packs.ps1`.
3. The script generates every non-`planned` pack whose required files exist, and skips missing packs with explicit missing-file paths.
4. Each generated zip is reopened to verify root `MODEL_PACK.json`, and every metadata file entry is checked against its zip-internal byte size and SHA-256.
5. The batch manifest records zip bytes, zip SHA-256, target dir, source dir, and metadata file count.
6. `voice-ime-model-packs-<version>.json` and `.md` are written to the output root.
7. Passing `-FailOnMissing` makes any missing requested pack fail the batch after writing the manifest.

## Model Pack Import

1. Open Settings / Models and click `导入包`.
2. Select a `voice-ime-model-pack-*.zip`.
3. If root `MODEL_PACK.json` is present, every listed file is checked for size and SHA-256 before extraction.
4. Only zip entries under `app/models/`, `models/`, or root `MODEL_PACK.txt` / `MODEL_PACK.json` are extracted.
5. Entries with absolute paths, drive prefixes, or `..` are rejected and cannot write outside the effective model root.
6. Model status refreshes after import, and the status line reports written, replaced, ignored, and verified files.
7. If `VOICE_IME_MODEL_DIR` or Settings / Models / `模型根目录` is set, imported files are written there instead of the package's default `app/models`.

## Model Pack Import Acceptance

1. Generate model packs, or place `voice-ime-model-pack-asr-fallback-whisper-tiny-int8.zip` under `D:\voice-ime-build-release`.
2. From the full package root, run `powershell -NoProfile -ExecutionPolicy Bypass -File .\app\tools\Model-Pack-Import-Acceptance.ps1`.
3. The script copies `voice-ime-2.0.1-rust-portable-core` to a temporary folder, leaving the real core package untouched.
4. It runs copied `app\VoiceIME.exe --install-model-pack <zip>` so the Rust model-pack importer performs extraction and metadata validation.
5. It then verifies every installable `MODEL_PACK.json` entry exists in the copied package's effective model root with matching byte size and SHA-256.
6. The temporary copy is deleted unless `-KeepWorkDir` is passed.

## UI Smoke

1. Run `npm run ui:smoke`.
2. The command starts a local Vite QA page with mocked Tauri data.
3. Main compose, Settings / Models, Settings / Input, Settings / Shortcuts, Settings / Smart, History, and Overlay render at 100%, 125%, 150%, and 200% device-scale combinations.
4. The command fails on outer page scroll, shell viewport overflow, or overflowing button/control text.
5. Screenshots are written under `work/ui-smoke/`.

## Portable Release Gate

1. Build and package the release.
2. From the repo root, run `powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\Test-PortableRelease.ps1`.
3. The script checks the full and core root layouts, hidden `app` directory, required app files, `BUILD.txt`, and core model cleanliness.
4. It starts the full and core apps with temporary `VOICE_IME_APP_DIR` values and requires each GUI process to stay alive for 5 seconds.
5. It runs packaged `VoiceIME.exe --doctor` with a temporary app data directory and requires a doctor report containing the local LLM file check.
6. Unless skipped, it runs the packaged Notepad, Browser, Translation, and model-pack import acceptance scripts.
7. At the end it removes any `.voice_ime` runtime data created under the portable package.

## Release Asset Packaging

1. Run `powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\package-release-assets.ps1`.
2. The script creates `voice-ime-2.0.1-rust-portable.zip` from the full package and `voice-ime-2.0.1-rust-portable-core.zip` from the core package.
3. It opens each zip and verifies required root entries, `app/VoiceIME.exe`, docs, `BUILD.txt`, model manifests, and forbidden runtime-data paths.
4. For the core zip, it also verifies `app/models` contains only `MODELS.json` and `MODELS.md`.
5. It writes `voice-ime-release-assets-2.0.1.json/.md` with every portable zip, model pack, model-pack manifest, byte size, and SHA-256.
6. If `gh` or a GitHub API token is available, `packaging\publish-github-release.ps1` can publish those assets to `v2.0.1`.

## Notepad Input Acceptance

1. Build or unpack a portable package.
2. From the package root, run `powershell -NoProfile -ExecutionPolicy Bypass -File .\app\tools\Notepad-Input-Acceptance.ps1`.
3. The script opens Notepad, focuses it, runs `VoiceIME.exe --paste-foreground <text> 80`, copies Notepad content back, and compares it with the expected text.
4. A `notepad-acceptance-YYYYMMDD-HHMMSS.txt` report appears under `app/.voice_ime/logs`.
5. The same run also appends an `input-target-YYYYMMDD.log` row with the captured target process, window class, paste method, `SendInput` count, focus retry count, clipboard restore status, previous clipboard format, `caret_source`, and captured `rect`.
6. The report must show `target_ok=True` and `target_process=Notepad.exe`; otherwise the script fails because another foreground app received the paste.
7. This is an automated smoke for Notepad only; WeChat/Feishu, Word/document editors, and IDE input boxes still need manual target-machine acceptance.

## Browser Input Acceptance

1. Build or unpack a portable package.
2. From the package root, run `powershell -NoProfile -ExecutionPolicy Bypass -File .\app\tools\Browser-Input-Acceptance.ps1`.
3. The script launches Microsoft Edge or Google Chrome with an isolated temporary user profile, forces renderer accessibility for the test browser, and opens a local textarea page.
4. It focuses the browser text area, runs `VoiceIME.exe --paste-foreground <text> 80`, and verifies the pasted value through the page window title.
5. A `browser-acceptance-YYYYMMDD-HHMMSS.txt` report appears under `app/.voice_ime/logs`.
6. The report must show `target_ok=True` and `target_process=msedge.exe` or `chrome.exe`; otherwise the script fails because another foreground app received the paste.
7. The browser profile and temporary page are deleted after the run; existing user browser profiles are not modified.

## Foreground App Input Acceptance

1. Build or unpack a portable package.
2. Open the target app, such as WeChat/Feishu, Word/document editor, VS Code, or a JetBrains IDE.
3. From the package root, run `powershell -NoProfile -ExecutionPolicy Bypass -File .\app\tools\Foreground-Input-Acceptance.ps1 -ExpectedProcess <process.exe>`.
4. During the countdown, focus the target input box.
5. The script runs `VoiceIME.exe --paste-foreground <text> 80` against the current foreground window.
6. A `foreground-acceptance-YYYYMMDD-HHMMSS.txt` report appears under `app/.voice_ime/logs`.
7. The report records `target_process`, `target_class`, `target_title`, `caret_source`, `rect`, `input_method`, `send_input_events`, focus recovery fields, previous clipboard format fields, and clipboard restoration status.
8. The report must show `target_ok=True` for the expected process/class/title filters; the pasted content itself remains a manual visual check for real apps that do not expose text content for automated readback.

## Current 2.0.1 Test Boundary

- Automated regression covers Rust unit tests, Rust compile, clippy, frontend build, release build, and portable packaging.
- Startup smoke test covers that `VoiceIME.exe` stays alive for 5 seconds after launch instead of panicking before GUI startup. Smoke tests must use a temporary `VOICE_IME_APP_DIR` so they do not write `.voice_ime` into the portable package.
- ASR smoke now covers `balanced`, `fast`, and `fallback` as worker subprocesses. If sherpa-onnx exits badly, the GUI should show an error instead of closing.
- Empty ASR output must not call MiniCPM; prompt-like MiniCPM output containing "个人词表", "纠错表", or "ASR 文本" must be discarded.
- Translation must translate the current editor text only; prompt-like translation output must be discarded.
- Portable release must not open a console window for `VoiceIME.exe`; local llama-server is launched hidden.
- Manual Windows integration still needs a real pass on WeChat/Feishu, Word/document editors, and IDE input boxes; Notepad and Edge/Chrome textarea paste have automated smoke scripts, and `Foreground-Input-Acceptance.ps1` records target/process/caret data for the remaining real apps.
- Real ASR acceptance requires sherpa-onnx model files matching the 2.0.1 default config. The copied 1.1.5 `faster-whisper-small` folder is reference material only and does not satisfy the new sherpa-onnx model paths.
- Each missing model row in Settings has clickable download, mirror page, official page, and model-folder actions. The downloader tries `hf-mirror.com` first, then `huggingface.co`.
- Settings shows download progress/failure in an in-panel notice, not only in the title status chip.
- Settings / Voice now exposes microphone selection and the main input view shows a pre-recording input meter; real multi-device manual coverage is still required on the target machines.
- History CSV export is automated and unit-tested for escaping; real spreadsheet review still depends on manual sample data from target machines.
- Long recording retention can be disabled and existing long recordings can be cleared from Settings / Data; short recordings remain non-retained by design.
- Portable packaging now includes an automated layout/release gate and `BUILD.txt`; manual smoke is still useful after packaging because it proves WebView startup on this machine.
- Settings / Data now shows an inline diagnostics panel after running Doctor; support export records the effective model root source and falls back to packaged model manifests when an external model root lacks `MODELS.json/md`; repair can also copy packaged model manifests into the effective model root without overwriting or copying model binaries.
- Settings / Data now includes `词表试算`, which previews a sentence through normalization, built-in corrections, hotwords, hot rules, ITN, and final cleanup with per-stage change and match rows.
- Settings / Models now has native file and directory pickers; real removable-drive acceptance should still be tested on target machines.
- Settings / Shortcuts now shows global-hotkey registration status and re-registers after save; manual conflict coverage is still required with real third-party apps.
- `--benchmark-asr` and Settings / Data / `ASR 基准` now provide a repeatable timing and CER/accuracy CSV harness; real quality still depends on recorded sample audio from target machines.
- `--benchmark-translation` and Settings / Data / `翻译基准` now provide a repeatable CSV harness for translation latency, backend errors, target-language hints, and prompt-like chatter filtering.
- Confirm paste now restores previous text clipboard where feasible, retries focus recovery before Ctrl+V, logs previous clipboard format/status, and exposes a short "pasted" UI state; manual image/file clipboard preservation is still future work.
- Settings / Smart now includes a personal prompt editor backed by `.voice_ime/personal_prompt.txt`, with save validation and restore-default action.
- Cursor positioning now logs `uia-caret` when UI Automation exposes text-range caret rectangles, then falls back to `uia-element`, guarded `uia-focused`, `gui-thread`, or `fallback`; real overlay placement still needs visual target-machine coverage.
- Recording/transcribing/result/postprocess/pasted UI meta now shows compact target diagnostics such as `Notepad.exe / GUI thread`, while detailed process/class/title/caret-source data remains in input-target logs.
- Overlay placement now clamps to the nearest monitor work area and flips above the caret when the lower edge would run off-screen; multi-monitor and unusual taskbar layouts still need manual target-machine coverage.
- Settings / Input now controls whether the overlay auto-hides after confirmation and the hide delay; default behavior remains auto-hide after 650ms.
- Clipboard failure can now fall back to direct Unicode typing for short single-line text; broad app coverage still needs manual acceptance.
- Smart correction now skips LLM rewriting for obvious code snippets, shell commands, URLs, and file paths, and edit commands do not rewrite code-like confirmation text.
- Task-style background workers now catch unwind panics in release builds, update the UI with a worker error, and write JSON rows to `worker-error-YYYYMMDD.log`; low-level OS hook loop panic coverage is still future work.
- A global panic hook now writes `panic-YYYYMMDD.log` with thread, location, payload, and backtrace; Doctor warns when panic or worker-error logs exist, and `Test-PortableRelease.ps1` runs `VoiceIME.exe --panic-smoke` to prove the packaged panic log path.
- Tray quit and Tauri exit events now run graceful shutdown: active recording is cancelled, stale worker sessions are invalidated, overlay state is hidden, history is flushed, and `shutdown-YYYYMMDD.log` is written. `Test-PortableRelease.ps1` also runs `VoiceIME.exe --shutdown-smoke` against a temporary app dir.
- Active ASR/LLM/translation workers now have an explicit cancellation token; clear, new recording, new translation, and shutdown cancel the previous token. ASR/LLM check cancellation at safe boundaries, while external translation child processes are killed promptly during wait.
- `npm run ui:smoke` now covers main/settings/history/overlay layout with QA mock data across 100%, 125%, 150%, and 200% device scale; true OS DPI and WebView screenshots still need manual release checks.
- Packaged builds now include `app/tools/启动语音输入-诊断.bat`; portable root layout still visibly exposes only the main launcher.
- Packaged builds now include `app/tools/Notepad-Input-Acceptance.ps1`; Notepad has an automated paste-path smoke, while other real apps still need manual coverage.
- Packaged builds now include `app/tools/Browser-Input-Acceptance.ps1`; Edge/Chrome textarea paste has an automated smoke with an isolated temporary browser profile.
- Packaged builds now include `app/tools/Foreground-Input-Acceptance.ps1`; WeChat/Feishu, Word/document editors, and IDEs can be checked with the same foreground paste path and target-log validation.
- Packaged builds now include `app/tools/Translation-Acceptance.ps1` and `Mock-External-Translate.ps1`; the external translation JSON path has an offline acceptance smoke.
- Packaged builds now include `app/tools\Model-Pack-Import-Acceptance.ps1`; the Rust `--install-model-pack` importer is checked against a copied core package and a real model pack zip.
- Repo packaging now includes `packaging/Test-PortableRelease.ps1`, which runs the full/core package layout gate, startup smoke, doctor report check, `MODEL_ROOT.txt` model-root smoke, shutdown smoke, panic-log smoke, and automated Notepad/Browser/Translation/model-pack import acceptance in one pass.
