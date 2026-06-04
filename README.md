# Voice IME Rust 2.0.1

Voice IME 是一个 Windows 优先的本地语音输入工具，使用 Rust + Tauri 2 重写。它把语音先转成确认栏文本或光标旁浮窗内容，用户确认后再粘贴到当前应用，不会自动发送回车。

![Voice IME 主界面](docs/images/voice-ime-main-ui.png)

## 主要能力

- 本地 ASR 转写：默认使用 `sherpa-onnx`，支持 `fast`、`balanced`、`fallback` 三个档位。
- 光标旁浮窗：能定位光标时在光标附近显示结果，定位失败时回到主窗口确认栏。
- 确认后输入：点击确认后恢复目标窗口焦点并粘贴文本，不自动发送。
- 按住说话：默认按住 `CapsLock` 或鼠标 `X2` 开始录音，松开后转写。
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
2. 按住 `CapsLock` 或鼠标 `X2` 开始说话，松开后停止录音。
3. 也可以点击 Voice IME 主窗口里的麦克风按钮，或按 `Alt+R` 切换开始/停止。
4. 等待转写结果出现在光标旁浮窗或主窗口确认栏。
5. 检查文本，必要时手动修改。
6. 点击“确认输入”，文本会粘贴到刚才的目标窗口。

常用快捷键：

| 快捷键 | 作用 |
| --- | --- |
| 按住 `CapsLock` / 鼠标 `X2` | 按住录音，松开转写 |
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

## 热词和规则

设置页提供“热词”和“规则”按钮，可直接打开 `app/.voice_ime/hot.txt` 与 `app/.voice_ime/hot-rule.txt`。`hot.txt` 用 `目标词 | 别名` 做专名替换，`hot-rule.txt` 用正则做格式替换。详细格式见 [docs/hotwords.md](docs/hotwords.md)。

## 数字和格式

ASR 后处理会做基础 ITN，把常见中文数字、百分比、金额、日期、时间、范围和单位转成更适合输入的格式，例如 `一百二十三点四五`、`百分之十二点五`、`二零二六年六月五号`、`下午三点半`。

## 输入目标日志

每次点击“确认输入”后，会在 `app/.voice_ime/logs/input-target-YYYYMMDD.log` 追加一行目标窗口日志，包含进程名、窗口类名、标题、光标来源、粘贴结果和粘贴延迟。设置页的“日志”按钮可以直接打开日志目录。

## 按应用输入画像

内置了微信、飞书/Lark、Word、Chrome/Edge、VS Code 和 JetBrains 的输入 profile。当前版本只自动应用更稳妥的粘贴延迟，并把命中的 profile 写入输入目标日志；不会自动发送 Enter。

## 本地诊断

设置页的“诊断”按钮会生成 `app/.voice_ime/logs/doctor-YYYYMMDD-HHMMSS.txt`，检查应用目录、日志写入、麦克风、剪贴板、ASR 模型、本地 LLM 端点和用户词表文件。也可以运行：

```powershell
app\VoiceIME.exe --doctor
```

## 托盘

关闭主窗口会隐藏到系统托盘，不会退出程序。托盘菜单可以显示主窗口、开始/停止录音、打开模型目录、打开日志、打开热词/规则、运行诊断或退出。

## 设置建议

- 普通电脑：ASR 档位选 `balanced`。
- 更快响应：ASR 档位选 `fast`。
- 兼容兜底：ASR 档位选 `fallback`。
- 追求体感速度：ASR 进程选“常驻加速”，第二次及以后会复用常驻 ASR 子进程。
- 习惯 CapsWriter 交互：保持“按住说话”开启；如果 CapsLock/X2 和其他软件冲突，可在设置里换成 `F8`、`F9`、`F10`、`F13` 或关闭鼠标触发。
- 遇到特殊机器或模型崩溃：ASR 进程改成“隔离稳妥”，每次转写独立运行，速度略慢但更容易排查。
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
- 录音只在用户明确按住触发键、点击按钮或按快捷键后开始。

## 版本说明

详细变更见 [CHANGELOG.md](CHANGELOG.md)。  
2.0.1 的验收、风险和 100 项优化 backlog 见 [docs/2.0.1-roadmap.md](docs/2.0.1-roadmap.md)。
CapsWriter-Offline v2.6 的对照落地计划见 [docs/capswriter-adaptation-plan.md](docs/capswriter-adaptation-plan.md)。
模型与主体分离策略见 [docs/model-pack-strategy.md](docs/model-pack-strategy.md)。
热词和规则词表见 [docs/hotwords.md](docs/hotwords.md)。

英文说明保留在 [README.en.md](README.en.md)。
