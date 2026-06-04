# 模型与主体分离策略

Date: 2026-06-04

Voice IME 从 2.0.1 后续热修开始采用“主体程序 + 可插拔模型包”发布方式。主体包只负责 GUI、录音、输入、下载、诊断和模型清单；ASR、LLM、翻译模型按 pack 放入 `app/models`。这样多台电脑测试时可以只复制轻主体包，模型放在移动硬盘或按需单独拷贝。

## 包类型

| 包 | 目录 | 内容 | 用途 |
| --- | --- | --- | --- |
| 标准测试包 | `D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable` | 程序 + 当前本机模型缓存 | 本机继续测试，不破坏已跑通环境 |
| 轻主体包 | `D:\voice-ime-build-release\voice-ime-2.0.1-rust-portable-core` | 程序 + 模型清单，不含大模型 | 拿去单位/其他电脑，先跑 GUI 和模型下载 |
| 未来模型包 | `voice-ime-model-pack-*.zip` | 单一 ASR/LLM/翻译模型目录 | 模型进步后单独替换，不重发主体 |

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
