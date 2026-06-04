# Voice IME Rust 2.0.1

Voice IME 是一个 Windows 优先的本地语音输入工具，使用 Rust + Tauri 2 重写。它把语音先转成确认栏文本或光标旁浮窗内容，用户确认后再粘贴到当前应用，不会自动发送回车。

![Voice IME 主界面](docs/images/voice-ime-main-ui.png)

## 主要能力

- 本地 ASR 转写：默认使用 `sherpa-onnx`，支持 `fast`、`balanced`、`fallback` 三个档位。
- 光标旁浮窗：能定位光标时在光标附近显示结果，定位失败时回到主窗口确认栏。
- 确认后输入：点击确认后恢复目标窗口焦点并粘贴文本，不自动发送。
- 按住说话：默认按住 `CapsLock` 或鼠标 `X2` 开始录音，松开后转写。
- 麦克风选择与电平：主界面显示输入电平，设置页可选择系统默认或指定麦克风。
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

启动后会打开毛玻璃主窗口。第一次使用建议先看主界面的“输入电平”是否会跳动，再进入“设置”页确认 ASR 模型状态是否为 ready。

## 基本用法

1. 打开需要输入文字的软件，例如记事本、浏览器输入框、聊天窗口或文档编辑器。
2. 确认主界面的“输入电平”会随说话跳动；如果不动，进入“设置 / 语音”切换麦克风。
3. 按住 `CapsLock` 或鼠标 `X2` 开始说话，松开后停止录音。
4. 也可以点击 Voice IME 主窗口里的麦克风按钮，或按 `Alt+R` 切换开始/停止。
5. 等待转写结果出现在光标旁浮窗或主窗口确认栏。
6. 检查文本，必要时手动修改。
7. 点击“确认输入”，文本会粘贴到刚才的目标窗口。

清空或重新开始录音会取消当前会话；旧转写、纠错或翻译结果不会再覆盖当前确认栏。

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

智能纠错依赖本地 `llama-server`。翻译默认也走本地 LLM，但设置页可以把“翻译引擎”切到 `external`，接入 NLLB、Bergamot 或其他本地机器翻译命令。

默认端点是：

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

外部翻译命令通过标准输入接收 JSON：

```json
{"source":"非洲之星和海洋之泪","target_language":"en","target_name":"英语"}
```

标准输出可以返回纯文本，也可以返回 JSON：

```json
{"text":"The Star of Africa and the Tear of the Ocean"}
```

当前内置引擎为 `llm` 和 `external`；`nllb`、`bergamot` 是后续内置适配预留。

## 热词和规则

设置页提供“热词”和“规则”按钮，可直接打开 `app/.voice_ime/hot.txt` 与 `app/.voice_ime/hot-rule.txt`。`hot.txt` 用 `目标词 | 别名` 做专名替换，`hot-rule.txt` 用正则做格式替换。详细格式见 [docs/hotwords.md](docs/hotwords.md)。

## 数字和格式

ASR 后处理会做基础 ITN，把常见中文数字、百分比、金额、日期、时间、范围和单位转成更适合输入的格式，例如 `一百二十三点四五`、`百分之十二点五`、`二零二六年六月五号`、`下午三点半`。

## 输入目标日志

每次点击“确认输入”后，会在 `app/.voice_ime/logs/input-target-YYYYMMDD.log` 追加一行目标窗口日志，包含进程名、窗口类名、标题、光标来源、粘贴结果和粘贴延迟。设置页的“日志”按钮可以直接打开日志目录。

## 历史追踪

历史页会保存每次转写的最终文本、原始 ASR、词表修正、热词、规则、ITN、LLM 后文本和阶段耗时。双击历史项可以把最终文本放回确认栏；展开“过程”可以看这次到底是模型识别错了，还是词表/规则/LLM 改偏了。
历史页支持按文本、后端、模型和日期筛选，排查某个模型或某天的异常更快。点击“导出 CSV”会把完整历史导出到 `app/.voice_ime/logs/history-export-YYYYMMDD-HHMMSS.csv`，便于用表格软件对比耗时和各阶段文本。

## 按应用输入画像

内置了微信、飞书/Lark、Word、Chrome/Edge、VS Code 和 JetBrains 的输入 profile。当前版本只自动应用更稳妥的粘贴延迟，并把命中的 profile 写入输入目标日志；不会自动发送 Enter。

## 本地诊断

设置页的“诊断”按钮会生成 `app/.voice_ime/logs/doctor-YYYYMMDD-HHMMSS.txt`，检查应用目录、日志写入、麦克风、剪贴板、ASR 模型、本地 LLM 端点和用户词表文件。也可以运行：

```powershell
app\VoiceIME.exe --doctor
```

设置页“数据 / 导出”会先运行诊断，再生成 `app/.voice_ime/logs/voice-ime-support-YYYYMMDD-HHMMSS.zip`。导出包包含配置、历史、个人提示词、纠错表、热词/规则、日志和模型说明，不包含录音文件和模型二进制。“历史 CSV”只导出表格格式的历史记录。
“数据”页还能控制长录音是否留存，并一键清理 `app/.voice_ime/recordings` 下的长录音文件。短录音只用于当次转写，默认不留存。

## 托盘

关闭主窗口会隐藏到系统托盘，不会退出程序。托盘菜单可以显示主窗口、开始/停止录音、打开模型目录、打开日志、打开热词/规则、运行诊断或退出。

## 设置建议

- 普通电脑：ASR 档位选 `balanced`。
- 更快响应：ASR 档位选 `fast`。
- 兼容兜底：ASR 档位选 `fallback`。
- 模型缺文件或放在移动硬盘：进入“设置 / 模型”，直接改对应模型路径后保存。
- 追求体感速度：ASR 进程选“常驻加速”，启动空闲时会尝试预热当前可用模型；也可以在“设置 / 模型”手动点“预热”。
- 多麦克风或远程桌面环境：在“设置 / 语音”选择具体麦克风并保存；主界面电平条可以快速判断是否录到有效输入。
- 习惯 CapsWriter 交互：保持“按住说话”开启；如果 CapsLock/X2 和其他软件冲突，可在设置里换成 `F8`、`F9`、`F10`、`F13` 或关闭鼠标触发。
- 遇到特殊机器或模型崩溃：ASR 进程改成“隔离稳妥”，每次转写独立运行，速度略慢但更容易排查。
- 老电脑或鼠标卡顿：把“ASR 线程”调成 `1` 或 `2`。
- 不想留下录音文件：在“设置 / 数据”把“长录音留存”改为“不保存”，并点击“清理录音”删除已有长录音。
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
