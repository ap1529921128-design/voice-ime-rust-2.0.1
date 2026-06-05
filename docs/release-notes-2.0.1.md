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

GUI translation clicks now append `translation-YYYYMMDD.log` rows with engine, model, timeout, elapsed seconds, character counts, status, and errors. Doctor surfaces recent translation failures or slow requests, so local LLM stalls and prompt-like translation chatter are easier to diagnose.

External translation now has model-profile slots. Settings / Smart exposes `fast`, `balanced`, `accurate`, and `custom` translation profiles with per-profile external commands; the runtime sends `profile`, `model`, and `model_root` in the JSON payload, logs labels like `mt/fast`, and adds `VoiceIME.exe --benchmark-translation-profile <profile> <samples-file>` for target-machine checks.

Settings / Data now shows hotword and hot-rule stats, including hotword entries, aliases, valid regex rules, and invalid rule examples. Doctor includes the same check so broken `hot-rule.txt` lines are visible before the next dictation.

Settings / Data also includes `词表试算`. Paste or type a sentence, click `试算`, and the app shows normalization, built-in corrections, hotword aliases, regex rules, ITN, and final cleanup with per-stage change and hit counts.

History details now include a raw-to-final character diff when the final text differs from raw ASR. Insertions and deletions are marked in place, so a single record can show whether the useful change came from hotwords, rules, ITN, or LLM cleanup.

Settings / Shortcuts now gives clearer guidance when a shortcut is duplicated, unavailable, or not recognized. Doctor includes the same suggestion text so hotkey failures are actionable instead of only reporting a raw registration error.

Settings / Models now has a per-profile `基准` action. It runs the same ASR CSV benchmark for the clicked `fast`, `balanced`, or `fallback` profile without changing the saved default profile.

The packaged CLI also supports `VoiceIME.exe --benchmark-asr-profile <profile> <samples-dir>`, making it easier to run fast/balanced/fallback/accurate comparisons on a target machine or removable drive. The portable release gate checks this path with an empty fallback sample directory.

`VoiceIME.exe --write-asr-benchmark-template <samples-dir>` now creates the 10 Chinese reference transcript files and a local README for repeatable target-machine ASR tests. It does not overwrite existing files, so real recordings and manually edited transcripts stay intact.

The same template is now available from Settings / Data / `ASR 样本`, so target-machine benchmark setup no longer requires the command line.

The `accurate` ASR profile is now an experimental external-command adapter for Qwen3/FunASR-style local backends. Configure `asr.accurate_external_command`; Voice IME sends UTF-8 JSON with a temporary wav path and accepts plain text or JSON `text`/`transcript` output. Large accurate models remain outside the core package.

2.0.1 also has deterministic test backends. `asr.default_engine=mock` bypasses model loading and lets ASR benchmark use same-name `.txt` files as transcript fixtures, while `mock://echo`, `mock://translate`, and `mock://fixed/<text>` exercise correction/translation cleanup without MiniCPM or network access. These paths are for release gates and CI-style plumbing tests, not real ASR/translation quality.

## Verification

The release gate passed on the build machine for:

- full and core startup smoke
- `VoiceIME.exe --doctor`
- ASR profile CLI smoke, no-model mock ASR CSV smoke, and accurate external-command ASR smoke
- ASR benchmark template smoke
- Notepad paste acceptance
- Edge/Chrome textarea paste acceptance
- external translation JSON-pipeline acceptance
- translation profile CLI smoke with `mt/fast`
- core package model-pack import acceptance with SHA-256 verification

Real target-machine checks are still needed for WeChat/Feishu, Word/document editors, IDEs, multiple microphones, and long recordings.

Model/app separation now supports `VOICE_IME_MODEL_DIR`, `app/MODEL_ROOT.txt`, and Settings / Models / `模型根目录`. Settings / Models can now write or clear `MODEL_ROOT.txt` from the UI, while packaged `app/models/MODELS.json/md` remains the manifest and repair source. Relative `models/...` paths are resolved under the effective model root, so a core app body can be moved between machines while ASR and MiniCPM model packs stay in one external repository.

The model manifest now also reserves planned translation packs under `app/models/mt/...`, so future MT model upgrades can be distributed as model packs without changing `VoiceIME.exe`.
