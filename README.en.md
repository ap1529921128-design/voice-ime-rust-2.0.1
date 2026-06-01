# Voice IME Rust 2.0.1

Windows-first personal voice input tool rewritten with Rust + Tauri 2.

## Run

Portable:

```powershell
Set-Location D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable
& .\启动语音输入.bat
```

The main GUI opens as a transparent glass window. Use the large microphone button to start/stop recording, or use `Alt+R` if the global hotkey registers successfully. `Alt+Space` cycles language, `Alt+E` translates to English, and `Alt+J` translates to Japanese. If hotkey registration fails, the GUI still opens and the window buttons remain usable. When a model is missing, open Settings and click the download button on that model row. Downloads try `hf-mirror.com` first, then fall back to `huggingface.co`.

ASR decoding runs in a worker subprocess. If a native model load or decode fails, the GUI should stay open and show the error instead of closing.

Development:

```powershell
npm install
npm run tauri dev
```

## Build

```powershell
npm install
npm run build
npm run tauri build
powershell -ExecutionPolicy Bypass -File .\packaging\package-portable.ps1
```

`package-portable.ps1` also runs `npm run tauri build` by default. Do not package an exe produced only by `cargo build --release`; that build can still point the WebView at the development URL `127.0.0.1:1420`.

## Model Layout

Models are not downloaded automatically. Put ASR files under the project root:

```text
app/models/
  sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/
    model.int8.onnx
    tokens.txt
  sherpa-onnx-zipformer-ctc-zh-int8-2025-07-03/
    model.int8.onnx
    tokens.txt
  sherpa-onnx-whisper-tiny/
    tiny-encoder.int8.onnx
    tiny-decoder.int8.onnx
    tiny-tokens.txt
```

In the portable package, `app` is hidden to keep the root clean. Enter it directly with:

```powershell
Set-Location D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable\app
```

MiniCPM/llama-server remains optional and uses:

```text
app/models/minicpm5-1b-q4.gguf
app/llama.cpp/cpu/llama-server.exe
app/tools/Start-MiniCPM-Translate.ps1
```

## Boundaries

- Recording starts only on explicit user action.
- Text is shown in the confirmation area or cursor overlay first.
- Confirm input pastes text only; it never sends Enter.
- TSF is prepared as a later phase, not registered in 2.0.1.

## 2.0.1 Notes

See `2.0.1-roadmap.md` in the portable `app` folder, or `docs/2.0.1-roadmap.md` in source, for launch steps, current test status, known problems, and the 100-point optimization backlog.
