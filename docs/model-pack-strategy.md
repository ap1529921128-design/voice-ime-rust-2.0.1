# 模型与主体分离策略

Date: 2026-06-04

Voice IME 从 2.0.1 后续热修开始采用“主体程序 + 可插拔模型包”发布方式。主体包只负责 GUI、录音、输入、下载、诊断和模型清单；ASR、LLM、翻译模型按 pack 放入当前模型根目录，默认是 `app/models`。这样多台电脑测试时可以只复制轻主体包，模型放在移动硬盘或按需单独拷贝。

设置页的“模型”分组支持四种接入方式：设置一个模型根目录，直接导入 `voice-ime-model-pack-*.zip`，按 profile 选择一个模型目录并自动填入默认文件名，或逐个文件选择 `onnx` / `tokens.txt`。模型页还会显示当前有效来源，并能把当前模型根目录写入或清除 `app/MODEL_ROOT.txt`。因此模型包既可以放在便携包 `app/models` 内，也可以放在移动硬盘上的绝对路径。

模型根目录优先级为：环境变量 `VOICE_IME_MODEL_DIR`、启动脚本可选文件 `app/MODEL_ROOT.txt`、设置页 `asr.model_root`、默认 `app/models`。旧配置中的 `models/...` 相对路径会自动映射到当前模型根目录下，所以同一套 config 可以跟着主体包升级，也可以把模型仓库整体迁到移动硬盘。

## 包类型

| 包 | 目录 | 内容 | 用途 |
| --- | --- | --- | --- |
| 标准测试包 | `D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable` | 程序 + 当前本机模型缓存 | 本机继续测试，不破坏已跑通环境 |
| 轻主体包 | `D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable-core` | 程序 + 模型清单，不含大模型 | 拿去单位/其他电脑，先跑 GUI 和模型下载 |
| 模型包 | `voice-ime-model-pack-*.zip` | 单一 ASR/LLM/翻译模型目录 | 模型进步后单独替换，不重发主体 |

## 模型清单

清单源文件在：

```text
packaging/model-manifest.json
```

打包后会进入：

```text
app/models/MODELS.json
app/models/MODELS.md
```

每个 pack 声明：

- `id`：稳定模型包编号。
- `kind`：`asr`、`llm`、`translation`。
- `profile`：对应设置里的模型档位，例如 `fast`、`balanced`、`fallback`、`accurate`。
- `target_dir`：解压后应该放到哪里。
- `required_files`：程序启动前要检查的文件。
- `source`：镜像和官方来源。
- `estimated_size_mb`：估算大小，方便判断能不能拷到目标电脑。

## 当前可用档位

- `balanced`：SenseVoice int8，默认主力。
- `fast`：Zipformer CTC int8，中文短句速度优先。
- `fallback`：Whisper tiny int8，兼容兜底。
- `smart-correction-translation`：MiniCPM GGUF，可选，本地纠错/翻译。
- `translation fast/balanced/accurate`：外部 MT 命令档位已在配置和清单中预留；当前包只验证 JSON 管道，不内置真实 MT 模型。

## 未来实验档

CapsWriter v2.6 的经验说明，高准确率路线应该做成独立大模型包，而不是塞进核心包。`accurate` 档现在是外部 ASR 命令实验接口：主体包只负责录音、临时 wav、JSON stdin/stdout、CSV benchmark 和崩溃隔离；具体 Qwen3/FunASR 模型仍由独立模型包或本地服务提供。

- Qwen3-ASR-1.7B q4/q5：准确率优先，模型体积约 1.3-1.9 GB。
- Fun-ASR-Nano：体积和准确率折中，模型体积约 796 MB。
- 专用翻译模型：用于替代 MiniCPM 的聊天式翻译，目标是短句 2 秒内返回。当前配置已有 `translation.profile`、`translation.models.fast_command`、`balanced_command` 和 `accurate_command`；外部命令会收到 `source`、`target_language`、`target_name`、`profile`、`model` 和 `model_root`。

`asr.accurate_external_command` 的命令会收到 UTF-8 JSON，字段包括 `wav_path`、`sample_rate`、`language`、`profile=accurate` 和 `prompt`。命令可以输出纯文本，也可以输出 `{"text":"..."}` 或 `{"transcript":"..."}`。发布包带 `app/tools/Mock-External-Asr.ps1` 只用于验收这条管道，不代表真实准确率。

外部翻译命令同样走 UTF-8 JSON stdin/stdout。`translation.profile=fast|balanced|accurate|custom` 控制模型标签和命令选择；分档命令为空时回退到 `translation.external_command`，所以旧配置仍然能跑。`VoiceIME.exe --benchmark-translation-profile <profile> <samples-file>` 会临时切到 `external` 并写出 CSV，适合在目标机器快速确认某个 MT 模型包或脚本是否可用。

## 使用规则

1. 轻主体包可以直接双击启动，但 ASR 模型缺失时只能进入 GUI 和设置页。
2. 模型包解压后，保持目录名不变，放入当前模型根目录下对应位置；默认模型根目录是 `app/models`。
3. 同一份模型目录可以在多台主体包之间复制；主体升级时不要删除或移动正在使用的模型根目录。
4. 遇到老电脑测试，先用 core 包启动，再只放 `fallback` 或 `balanced` 模型。
5. 新模型进入清单前，必须先通过模型缺失检查、启动烟测、短句测速、`accurate` 外部命令 benchmark 和崩溃隔离测试。

如果不想在每台机器重复改配置，可以先在“设置 / 模型”里选择共享模型仓库，再点“写入便携”。这会在隐藏的 `app` 目录里创建 `MODEL_ROOT.txt`，第一行写共享模型仓库路径，例如：

```text
E:\voice-ime-model-packs
```

后端、启动脚本和 MiniCPM 启动脚本都会在没有外部 `VOICE_IME_MODEL_DIR` 环境变量时读取这个文件，并把它作为当前模型根目录。托盘菜单里的“模型目录”和设置页/诊断页会打开同一个有效目录，避免一边导入到外置盘、一边托盘仍打开内置 `app/models`。主体包内置的 `app/models/MODELS.json` 和 `MODELS.md` 始终作为发布清单和修复来源保留，即使当前有效模型根目录已经切到移动硬盘。

core 包也可以不进 GUI，直接用工具脚本写入或清除模型根目录：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\app\tools\Model-Root.ps1 -ModelRoot E:\voice-ime-models
powershell -NoProfile -ExecutionPolicy Bypass -File .\app\tools\Model-Root.ps1 -Clear
```

该脚本会生成 `app/.voice_ime/logs/model-root-YYYYMMDD-HHMMSS.txt`，记录 `VOICE_IME_MODEL_DIR`、`MODEL_ROOT.txt`、`asr.model_root` 或默认 `app/models` 中哪一个正在生效，并按 `MODELS.json` 列出各模型包的 READY、MISSING、PLANNED 状态。这样在移动硬盘和多台旧电脑之间切换时，不需要先启动 GUI 才知道模型目录有没有接对。

## 打模型包

从当前 full 包或任意模型目录打出单独模型包：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\package-model-pack.ps1 -Profile balanced
```

也可以指定来源和输出位置：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\package-model-pack.ps1 `
  -Profile fallback `
  -SourceModelsDir E:\voice-ime-models `
  -OutputRoot D:\voice-ime-build-release
```

生成的 zip 根目录包含 `app/models/...`、`MODEL_PACK.txt` 和 `MODEL_PACK.json`。`MODEL_PACK.json` 记录包内文件的大小与 SHA-256；Settings / Models 里的 `导入包` 会先校验这些条目，再写入当前模型根目录对应目录，并拒绝绝对路径、盘符路径和 `..` 路径。旧的无 metadata 模型包仍可导入，但不会显示校验数量。

从当前机器已有模型批量生成所有非 `planned` 包：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\package-available-model-packs.ps1
```

该脚本会调用单包脚本生成可用的 `voice-ime-model-pack-*.zip`，跳过缺文件的包，并额外写出：

```text
voice-ime-model-packs-2.0.1.json
voice-ime-model-packs-2.0.1.md
```

批量脚本会重新打开每个 zip，逐项验证 `MODEL_PACK.json` 里的文件大小和 SHA-256，然后在批量清单里记录每个 zip 的大小、SHA-256、源目录、目标目录和 metadata 文件数。需要严格发布时加 `-FailOnMissing`，这样任何请求的模型包缺文件都会让命令失败。

## 验证模型包导入

便携包带一个自动验收脚本，会复制一份 core 包到临时目录，并用 Rust CLI importer 导入模型包：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\app\tools\Model-Pack-Import-Acceptance.ps1 `
  -CoreReleaseRoot D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable-core `
  -ModelPackZip D:\voice-ime-build-release\voice-ime-model-pack-asr-fallback-whisper-tiny-int8.zip
```

该脚本会调用 `VoiceIME.exe --install-model-pack <zip>`，然后按 `MODEL_PACK.json` 校验导入后的文件大小和 SHA-256。默认使用 fallback 小模型包，避免每次 release gate 都复制几百 MB 到 1GB 的大包。

## 生成 GitHub Release 资产

full/core 目录通过验收后，运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\package-release-assets.ps1
```

它会生成：

```text
voice-ime-2.0.1-rust-portable.zip
voice-ime-2.0.1-rust-portable-core.zip
voice-ime-release-assets-2.0.1.json
voice-ime-release-assets-2.0.1.md
```

同时会把已有 `voice-ime-model-pack-*.zip` 和 `voice-ime-model-packs-2.0.1.json/.md` 写入发布资产清单，记录每个文件的大小和 SHA-256。需要自动上传 GitHub Release 时，在已安装 `gh` 或已配置 `GH_TOKEN/GITHUB_TOKEN` 的环境运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\packaging\publish-github-release.ps1
```

当前机器如果只有 SSH push 权限、没有 GitHub API token，则只能推代码，不能创建带附件的 GitHub Release；这时可用生成出的资产手动上传。
