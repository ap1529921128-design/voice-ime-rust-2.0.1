# 模型与主体分离策略

Date: 2026-06-04

Voice IME 从 2.0.1 后续热修开始采用“主体程序 + 可插拔模型包”发布方式。主体包只负责 GUI、录音、输入、下载、诊断和模型清单；ASR、LLM、翻译模型按 pack 放入 `app/models`。这样多台电脑测试时可以只复制轻主体包，模型放在移动硬盘或按需单独拷贝。

设置页的“模型”分组支持三种接入方式：直接导入 `voice-ime-model-pack-*.zip`，按 profile 选择一个模型目录并自动填入默认文件名，或逐个文件选择 `onnx` / `tokens.txt`。因此模型包既可以放在便携包 `app/models` 内，也可以放在移动硬盘上的绝对路径。

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

## 未来实验档

CapsWriter v2.6 的经验说明，高准确率路线应该做成独立大模型包，而不是塞进核心包。后续 `accurate` 档优先评估：

- Qwen3-ASR-1.7B q4/q5：准确率优先，模型体积约 1.3-1.9 GB。
- Fun-ASR-Nano：体积和准确率折中，模型体积约 796 MB。
- 专用翻译模型：用于替代 MiniCPM 的聊天式翻译，目标是短句 2 秒内返回。

## 使用规则

1. 轻主体包可以直接双击启动，但 ASR 模型缺失时只能进入 GUI 和设置页。
2. 模型包解压后，保持目录名不变，放入 `app/models` 下对应位置。
3. 同一份模型目录可以在多台主体包之间复制；主体升级时不要删除 `app/models`。
4. 遇到老电脑测试，先用 core 包启动，再只放 `fallback` 或 `balanced` 模型。
5. 新模型进入清单前，必须先通过模型缺失检查、启动烟测、短句测速和崩溃隔离测试。

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

生成的 zip 根目录包含 `app/models/...`、`MODEL_PACK.txt` 和 `MODEL_PACK.json`。`MODEL_PACK.json` 记录包内文件的大小与 SHA-256；Settings / Models 里的 `导入包` 会先校验这些条目，再写入 `app/models` 对应目录，并拒绝绝对路径、盘符路径和 `..` 路径。旧的无 metadata 模型包仍可导入，但不会显示校验数量。

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
