# CapsWriter-Offline 对照落地改造清单

Date: 2026-06-03

本文件记录 Voice IME Rust 2.0.1 对 CapsWriter-Offline v2.6 的可借鉴点、差距判断和后续落地任务。目标不是复制它的 Python/C/S 架构，而是把它已经验证过的产品经验迁移到 Rust/Tauri 路线：轻主体、模型分包、低延迟按键交互、强热词、按应用输入兼容、可诊断。

## 取证摘要

- CapsWriter-Offline 最新 release：`v2.6 大量细节改进`，发布时间为 2026-05-30 UTC，程序资产包括 `CapsWriter-Offline-20260531.zip` 约 99.3 MB 和 Client 包约 37.7 MB。
- 它的模型并不包含在 99.3 MB 主包里，而是在 `models` release 独立分发：`Sensevoice-Small-ONNX.zip` 约 413.7 MB，`Fun-ASR-Nano-GGUF.zip` 约 795.6 MB，`Qwen3-ASR-1.7B-q4_k.zip` 约 1345.2 MB，`Qwen3-ASR-1.7B-q5_k.zip` 约 1861.2 MB。
- v2.6 release notes 的关键新增点包括 GPU 预加速、标点策略优化、按应用附加回车、活动窗口日志、热词体验简化、托盘重启、集显兼容配置、隐藏 Windows Terminal 控制台。
- README 描述的核心体验是按住 CapsLock 或鼠标侧键说话，松开即输入；并支持文件转录、ITN、热词、规则替换、LLM 角色、托盘菜单、C/S 分离和本地日志。
- 本地浅克隆参考 commit：`1015197412b00cb18de8807cf379169d24241194`。参考链接：[v2.6 release](https://github.com/HaujetZhao/CapsWriter-Offline/releases/tag/v2.6)、[models release](https://github.com/HaujetZhao/CapsWriter-Offline/releases/tag/models)、[readme](https://github.com/HaujetZhao/CapsWriter-Offline/blob/master/readme.md)、[热词文档](https://github.com/HaujetZhao/CapsWriter-Offline/blob/master/docs/%E7%83%AD%E8%AF%8D%E5%8A%9F%E8%83%BD%E5%A6%82%E4%BD%95%E4%BD%BF%E7%94%A8.md)。

## 判断

CapsWriter 当前强在“无感输入”和“工程细节”。它不是靠一个 99 MB 全能包完成高准确率，而是把程序主体压轻、模型按能力分包、交互压到按键级、输入兼容交给应用配置、热词交给普通文本文件热重载。

Voice IME Rust 不应该在 2.0.x 直接硬抄它的 C/S Python 体系。我们的胜负手是保留 Rust/Tauri 的稳定发布、圆角毛玻璃 GUI、光标旁确认、安全粘贴、翻译/改写和诊断面板，然后把 CapsWriter 的低延迟输入、词表、分包和兼容经验补上。

## 对照差距

| 方向 | CapsWriter v2.6 已验证 | Voice IME Rust 2.0.1 当前 | 落地结论 |
| --- | --- | --- | --- |
| 主体体积 | 主程序包轻，模型独立下载 | 便携包容易被旧模型和 LLM 运行时拖大 | 做 Core/Standard/Accuracy 三档包，核心包不捆大模型 |
| 默认交互 | CapsLock/X2 长按录音，松开上屏 | GUI 按钮和 `Alt+R` 切换录音 | 新增 push-to-talk，不替换确认模式 |
| 模型矩阵 | Paraformer、SenseVoice、Fun-ASR-Nano、Qwen3-ASR | sherpa-onnx fast/balanced/fallback | 增加模型 manifest 和可插拔后端说明，Qwen3 作为实验准确率后端 |
| 热词 | `hot.txt`、`hot-rule.txt`、`hot-server.txt` 分层 | 内置确定性 corrections，用户可控弱 | 新增热词文件、规则文件、热重载、GUI 编辑入口 |
| 输入兼容 | `paste_apps`、`enter_apps`、活动窗口日志 | 统一剪贴板 Ctrl+V，已禁止自动 Enter | 增加 per-app input profile，但默认仍不自动 Enter |
| 标点/格式 | 短句去末尾标点，按应用强制去标点 | 基础 ASR 清理 | 新增标点策略和 ITN 模块 |
| 性能 | release notes 提到 GPU 预加速和多模型延迟参考 | ASR 约 2.9s，模型冷启动仍明显 | 模型预热、常驻 worker、短句流式、阶段计时 |
| 诊断 | 活动窗口写 debug log，日志帮助配置 | 诊断和导出不足 | 添加 doctor、日志导出、活动窗口记录 |
| UI | 托盘优先，少 GUI | GUI 已成优势，但设置仍浅 | GUI 管理模型、词表、应用策略和诊断 |
| 翻译/改写 | LLM 角色 | MiniCPM 翻译/改写已可用但慢 | 翻译改走专用 MT 后端，LLM 只做润色/角色 |

## 版本路线

## 当前落地进度

- 已完成主体/模型分离：新增 Core 便携包与模型 manifest，便于多机测试和后续模型替换。
- 已完成常驻 ASR worker：同一档位可复用已加载 recognizer，保留隔离 worker 作为崩溃兜底。
- 已完成用户热词和规则：`hot.txt` 与 `hot-rule.txt` 可由设置页直接打开，确定性应用在 LLM 之前。
- 已完成基础 push-to-talk：默认 `CapsLock` 与鼠标 `X2` 长按录音、松开转写，设置页可改触发键/侧键/吞键；当前版本不补发短按 CapsLock。
- 已完成活动窗口日志底座：确认输入时记录目标进程、类名、标题、caret 来源、粘贴结果和延迟，用于后续 per-app profile。
- 已完成 doctor 诊断报告底座：可检查目录写入、麦克风、剪贴板、ASR 模型、本地 LLM 端点和用户词表文件。

### 2.0.2：先追体感

目标：让用户感觉“按下就能用、出错知道为什么、输入到哪里可控”。

- 新增 push-to-talk 输入模式：默认可选 `CapsLock` 和鼠标 `X2`，支持长按录音、松开停止；短按 CapsLock 时补发原按键，避免破坏大小写功能。
- 新增托盘常驻：启动后可最小化到托盘，托盘提供显示主窗、开始/停止、打开模型目录、打开热词、重启、退出。
- 新增 app profile：记录目标进程名、窗口标题、类名、caret 来源、输出方式、标点策略、粘贴延迟；默认内置 Notepad、Chrome/Edge、WeChat/Feishu、Word、VS Code/JetBrains。
- 新增活动窗口日志：每次确认输入后写入 `logs/input-target-YYYYMMDD.log`，用于发现哪些应用需要强制粘贴或延迟。
- 新增模型 manifest：`app/models/MODELS.json` 列出每个 profile 的模型名、大小、来源、目标目录、必需文件、校验值、推荐硬件。
- 新增 `VoiceIME.exe --doctor`：检查 WebView2、麦克风、模型文件、热键、剪贴板、llama-server、日志权限，并生成可分享诊断文件。
- 保持默认安全：不自动 Enter，不默认保存短录音，不默认云端上传，不把确认栏内容交给编辑指令覆盖。

2.0.2 验收：双击启动无控制台黑框；按住 CapsLock 说一句话，松开后 5 秒内在 Notepad/Chrome/微信任一目标成功进入确认或粘贴；失败时 doctor 能说清楚缺哪个模型或哪个权限。

### 2.0.3：补词表和格式化

目标：把“纠错不准”从 LLM 问题改成用户可控的本地词表问题。

- 新增 `hot.txt`：每行一个目标词，支持 `目标词 | 别名1 | 别名2`；第一个字段作为最终输出。
- 新增 `hot-rule.txt`：支持简单等号规则和 Rust regex 捕获替换；用于单位、邮箱、符号、命令短语。
- 新增 `hot-server.txt`：作为 ASR 后端上下文提示源，只有后端支持时才注入，不做强制承诺。
- 新增文件 watcher：热词保存后 1-3 秒内重载，状态栏显示词条数和规则数。
- 新增 GUI 热词页：打开文件、添加词条、测试一句、查看命中日志。
- 新增 ITN 模块：中文数字、范围、日期、时间、金额、百分比、型号常见表达转换。
- 新增标点策略：短句可去末尾标点，长句保留；不同应用可配置。
- 新增 raw/corrected 对比：历史中保存原始 ASR、热词后文本、规则后文本、LLM 后文本。

2.0.3 验收：`非洲之星 | Africa Star | 非州之星` 这类词表能稳定命中；`一百二十三点四五` 能转为 `123.45`；微信短句默认不带句号，Word 长句保留标点。

### 2.1.0：模型分层和预热

目标：让体积和速度有清晰档位，不再让用户面对一个巨大的不透明包。

- Core 包：只含程序、GUI、doctor、下载器和说明，目标小于 150 MB。
- Standard 包：含推荐轻量 ASR 模型，目标小于 700 MB，适合普通电脑。
- Accuracy 包：含更高准确率模型，允许 1.5-2.5 GB，明确标注硬件建议。
- `fast` profile：短句优先，模型常驻，目标 10 秒以内语音转写耗时小于 1.5 秒。
- `balanced` profile：默认推荐，目标准确率和速度均衡。
- `accurate` profile：实验性接 Qwen3-ASR/Fun-ASR-Nano 路线，优先研究 Rust 可维护集成；必要时作为外部本地服务适配，不进入主链路。
- recognizer pool：常用 profile 后台预热，切换 profile 时不阻塞 UI。
- GPU/DML/Vulkan 能力检测：只做可见推荐和显式开关，不默认执行危险命令。
- 阶段计时：保存录音时长、模型加载、推理、热词、LLM/翻译、粘贴延迟。

2.1.0 验收：Core 包不捆模型仍能清晰指导下载；Standard 包在普通 Windows 电脑上免配置可转写；模型缺失、校验失败、后端崩溃不会关闭 GUI。

### 2.2.0：翻译换引擎

目标：让翻译从“本地小模型聊天”变成“专用机器翻译”，解决英语/日语等待和解释性输出。

- 新增 translation engine 抽象：`llm`、`nllb`、`bergamot`、`external`。
- 默认短文本翻译走专用 MT 后端，LLM 只负责润色、风格化和角色任务。
- 翻译输出做语言检测：目标语言不匹配则丢弃并提示重试。
- 翻译流式状态：开始、加载模型、翻译中、超时、完成。
- 保留 MiniCPM fallback，但所有 prompt-like 输出必须被过滤。

2.2.0 验收：10 个 20 字以内中日英互译样例，95% 在 2 秒内返回，且不会出现“翻译结果：”“以下是”等说明性前缀。

## 模块级任务清单

| ID | 文件/模块 | 改造项 | Pass condition |
| --- | --- | --- | --- |
| A01 | `src-tauri/src/config.rs` | 增加 `input.shortcuts[]`，支持 keyboard/mouse、hold_mode、suppress、threshold | 旧 `Alt+R` 配置可迁移，新 CapsLock/X2 配置可保存 |
| A02 | `src-tauri/src/lib.rs` | 全局快捷键改为按下/抬起事件驱动，无法注册时不阻塞 GUI | CapsLock/X2 冲突时主窗仍能打开 |
| A03 | `src-tauri/src/audio.rs` | 支持 hold-to-record，抬起立即 stop | 连续 20 次按下/抬起无 stuck recording |
| A04 | `src-tauri/src/tray.rs` | 新增托盘模块 | 托盘可显示/隐藏/重启/退出 |
| A05 | `src-tauri/src/win_bridge.rs` | 增加 active window info：hwnd、pid、process、class、title | 历史记录能看到目标进程 |
| A06 | `src-tauri/src/win_bridge.rs` | 增加 per-app output strategy：paste/type/paste_delay/restore_clipboard | 微信强制粘贴，普通输入框可选择策略 |
| A07 | `src-tauri/src/text.rs` | 新增 punctuation policy | 短句去标点、长句保留、指定 App 强制去标点 |
| A08 | `src-tauri/src/hotword.rs` | 新增 `hot.txt` 解析和热重载 | 修改文件 3 秒内生效 |
| A09 | `src-tauri/src/hot_rule.rs` | 新增规则替换 | 支持 `查找 = 替换` 和捕获组 |
| A10 | `src-tauri/src/itn.rs` | 新增中文 ITN | 数字、范围、日期、金额样例通过 |
| A11 | `src-tauri/src/asr.rs` | 增加 profile manifest 和 checksum | 模型缺失提示精确到文件 |
| A12 | `src-tauri/src/asr.rs` | recognizer worker 常驻/预热 | 第二次短句明显快于冷启动 |
| A13 | `src-tauri/src/asr.rs` | `accurate` 实验后端接口 | Qwen3/Fun-ASR 可作为外部本地服务接入 |
| A14 | `src-tauri/src/doctor.rs` | 新增诊断命令 | 输出环境报告和修复建议 |
| A15 | `src-tauri/src/history.rs` | 历史加入 raw/hot/rule/final/stage timings | 调试准确率不再靠猜 |
| A16 | `src-tauri/src/llm.rs` | LLM 角色和翻译分离 | 翻译不会进入词表/角色解释 |
| A17 | `src-tauri/src/translation.rs` | 新增专用 MT 后端抽象 | 中日英短句可脱离 MiniCPM 翻译 |
| A18 | `src/main.ts` | 设置拆为语音、模型、词表、输入、翻译、数据 | 设置页不再挤成一屏 |
| A19 | `src/styles.css` | 保持毛玻璃，但给词表/模型/诊断做密集工具布局 | 没有网页感滚条和文字溢出 |
| A20 | `packaging/package-portable.ps1` | 产出 Core/Standard/Accuracy 三档包 | 根目录仍只暴露一个启动脚本 |

## 竞争验收尺

- 体积：Core 包小于 150 MB；Standard 包小于 700 MB；Accuracy 包允许大，但下载页必须说清楚模型大小。
- 速度：10 秒中文短句，第二次及以后从停止录音到出结果小于 1.5 秒；30 秒长文 real-time factor 小于 0.35。
- 准确率：10 条中文验收句 raw ASR 可用分不低于当前 2.0.1；加热词后专名命中率 95% 以上。
- 输入兼容：Notepad、Chrome/Edge、微信/飞书、Word、VS Code/JetBrains 至少 5 类通过。
- 稳定：ASR/翻译后端崩溃不带走 GUI；doctor 能定位模型缺失、热键失败、剪贴板失败。
- 安全：默认不自动发送 Enter；默认不保存短录音；剪贴板尽量恢复；LLM 不覆盖确认栏原文。
- UI：主窗口无网页边、无全局滚条、无文字溢出；托盘、设置、浮窗三处状态一致。

## 不做的事

- 不直接搬 CapsWriter 的 Python 源码进主链路。
- 不把 Qwen3-ASR 大模型塞进默认核心包。
- 不为了追求“松开即上屏”而取消确认栏；默认仍保留安全确认，用户可开快速上屏模式。
- 不默认执行 GPU 锁频命令；只能作为高级选项，必须提示管理员权限和恢复命令。
- 不把 LLM 翻译当作长期唯一方案；它可以做风格化，但不适合低延迟实时翻译主路径。
