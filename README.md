# Voice IME Rust 2.0.1

Voice IME 是一个 Windows 优先的本地语音输入工具，使用 Rust + Tauri 2 重写。它把语音先转成确认栏文本或光标旁浮窗内容，用户确认后再粘贴到当前应用，不会自动发送回车。

![Voice IME 主界面](docs/images/voice-ime-main-ui.png)

## 主要能力

- 本地 ASR 转写：默认使用 `sherpa-onnx`，支持 `fast`、`balanced`、`fallback` 三个档位。
- 光标旁浮窗：能定位光标时在光标附近显示结果，定位失败时回到主窗口确认栏。
- 确认后输入：点击确认后恢复目标窗口焦点并粘贴文本，不自动发送。
- 智能纠错与翻译：可连接本地 OpenAI-compatible `llama-server`，用于纠错、改写和中日英翻译。
- 便携运行：发布包根目录只保留 `启动语音输入.bat`，主体程序和模型放在隐藏的 `app` 目录里。

## 下载与运行

便携版解压后，双击根目录里的：

```text
启动语音输入.bat
```

也可以用 PowerShell 启动：

```powershell
Set-Location D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable
& .\启动语音输入.bat
```

启动后会打开毛玻璃主窗口。第一次使用建议先进入“设置”页，确认 ASR 模型状态是否为 ready。

## 基本用法

1. 打开需要输入文字的软件，例如记事本、浏览器输入框、聊天窗口或文档编辑器。
2. 点击 Voice IME 主窗口里的麦克风按钮，或按 `Alt+R` 开始录音。
3. 再次点击按钮或按 `Alt+R` 停止录音。
4. 等待转写结果出现在光标旁浮窗或主窗口确认栏。
5. 检查文本，必要时手动修改。
6. 点击“确认输入”，文本会粘贴到刚才的目标窗口。

常用快捷键：

| 快捷键 | 作用 |
| --- | --- |
| `Alt+R` | 开始或停止录音 |
| `Alt+Space` | 切换识别语言 |
| `Alt+E` | 将确认栏文本翻译为英文 |
| `Alt+J` | 将确认栏文本翻译为日文 |

如果快捷键注册失败，主窗口按钮仍然可以正常使用。

## 模型放置

ASR 模型默认放在便携包的：

```text
app/models/
```

需要的目录结构：

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

设置页提供“下载”“镜像”“官网”“模型目录”按钮。下载会优先尝试 `hf-mirror.com`，失败后再尝试 `huggingface.co`。

## 智能纠错和翻译

智能纠错与翻译依赖本地 `llama-server`，默认端点是：

```text
http://127.0.0.1:18080/v1/chat/completions
```

便携包可选包含：

```text
app/models/minicpm5-1b-q4.gguf
app/llama.cpp/cpu/llama-server.exe
app/tools/Start-MiniCPM-Translate.ps1
```

如果本地服务不可用，语音转写仍可使用；智能纠错会退回到确定性词表修正，翻译会提示服务不可用。

## 设置建议

- 普通电脑：ASR 档位选 `balanced`。
- 更快响应：ASR 档位选 `fast`。
- 兼容兜底：ASR 档位选 `fallback`。
- 老电脑或鼠标卡顿：把“ASR 线程”调成 `1` 或 `2`。
- 不想使用本地大模型：关闭“智能纠错”，只保留基础转写。

## 开发构建

安装依赖：

```powershell
npm install
```

开发运行：

```powershell
npm run tauri dev
```

生产构建：

```powershell
npm run build
npm run tauri build
```

打便携包：

```powershell
powershell -ExecutionPolicy Bypass -File .\packaging\package-portable.ps1
```

不要直接拿 `cargo build --release` 生成的 exe 打包；它可能仍然指向开发地址 `127.0.0.1:1420`。

## 当前边界

- 2.0.1 不是完整 TSF 系统输入法，TSF 只做了后续阶段预留。
- 输入结果默认先进入确认栏或浮窗，不会无确认直接发送。
- 确认输入只执行粘贴，不会自动按 Enter。
- 录音只在用户明确点击按钮或按快捷键后开始。

## 版本说明

详细变更见 [CHANGELOG.md](CHANGELOG.md)。  
2.0.1 的验收、风险和 100 项优化 backlog 见 [docs/2.0.1-roadmap.md](docs/2.0.1-roadmap.md)。

英文说明保留在 [README.en.md](README.en.md)。
