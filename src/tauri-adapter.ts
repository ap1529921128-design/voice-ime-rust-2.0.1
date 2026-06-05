import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";

const qaParams = new URLSearchParams(window.location.search);
const qaMode = qaParams.has("qa");
let qaSnapshot = createQaSnapshot();

export function currentWindowLabel() {
  if (qaMode) return qaParams.get("window") === "overlay" ? "overlay" : "main";
  return getCurrentWindow().label;
}

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!qaMode) return tauriInvoke<T>(command, args);
  return qaInvoke(command, args) as T;
}

export async function listen<T>(
  event: string,
  handler: (event: { payload: T }) => void | Promise<void>,
): Promise<() => void> {
  if (!qaMode) return tauriListen<T>(event, handler);
  void event;
  void handler;
  return () => {};
}

export async function openDialog(options: Parameters<typeof dialogOpen>[0]) {
  if (!qaMode) return dialogOpen(options);
  void options;
  return null;
}

function qaInvoke(command: string, args?: Record<string, unknown>) {
  if (command === "get_snapshot") return qaSnapshot;
  if (command === "asr_status") return qaModelStatus();
  if (command === "audio_devices") return qaAudioDevices();
  if (command === "audio_level") return qaAudioLevel();
  if (command === "hotkey_status") return qaHotkeys();
  if (command === "doctor_report") return qaDoctorReport();
  if (command === "repair_doctor") return qaRepairReport();
  if (command === "llm_service_status") return qaLlmServiceStatus();
  if (command === "start_llm_service") return { ...qaLlmServiceStatus(), reachable: true };
  if (command === "install_model_pack") {
    return {
      ...qaSnapshot,
      status: "模型包已导入",
      meta: String(args?.packPath || "QA model pack"),
    };
  }
  if (command === "run_asr_benchmark") {
    return {
      ...qaSnapshot,
      status: "ASR 基准中",
      meta: String(args?.samplesDir || "QA samples"),
    };
  }
  if (command === "set_text") {
    const text = String(args?.text || "");
    qaSnapshot = { ...qaSnapshot, text, word_count: Array.from(text).length };
    return qaSnapshot;
  }
  if (command === "save_config") {
    qaSnapshot = {
      ...qaSnapshot,
      config: (args?.config as typeof qaSnapshot.config | undefined) || qaSnapshot.config,
    };
    return { ...qaSnapshot, status: "设置已保存", meta: "QA mock" };
  }
  if (command === "clear_text") {
    qaSnapshot = { ...qaSnapshot, text: "", word_count: 0, status: "已清空" };
    return qaSnapshot;
  }
  return { ...qaSnapshot, status: "QA mock", meta: command };
}

function createQaSnapshot() {
  return {
    state: qaParams.get("state") || "Idle",
    text:
      qaParams.get("text") ||
      "非洲之星和海洋之泪。这个句子用于检查中文、英文 Voice IME、数字 123.45 和按钮布局。",
    status: qaParams.get("status") || "待命",
    meta: "balanced · 常驻加速 · QA 长状态文本用于检查溢出",
    language: "zh",
    word_count: 45,
    config: {
      asr: {
        default_engine: "sherpa-onnx",
        profile: "balanced",
        worker_mode: "persistent",
        language: "zh",
        input_device_name: "",
        sample_rate: 16000,
        min_record_seconds: 0.2,
        max_record_seconds: 120,
        long_transcript_seconds: 30,
        long_transcript_chunk_seconds: 25,
        save_long_recordings: false,
        num_threads: 2,
        models: {
          sense_voice_model:
            "E:/voice-ime-model-packs/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/model.int8.onnx",
          sense_voice_tokens:
            "E:/voice-ime-model-packs/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/tokens.txt",
          zipformer_ctc_model:
            "models/sherpa-onnx-zipformer-ctc-zh-int8-2025-07-03/model.int8.onnx",
          zipformer_ctc_tokens:
            "models/sherpa-onnx-zipformer-ctc-zh-int8-2025-07-03/tokens.txt",
          whisper_encoder: "models/sherpa-onnx-whisper-tiny/tiny-encoder.int8.onnx",
          whisper_decoder: "models/sherpa-onnx-whisper-tiny/tiny-decoder.int8.onnx",
          whisper_tokens: "models/sherpa-onnx-whisper-tiny/tiny-tokens.txt",
        },
      },
      input: {
        mode: "floating-overlay",
        tsf_phase: "prepared",
        paste_delay_ms: 60,
        hotkey_record: "Alt+R",
        hotkey_language: "Alt+Space",
        hotkey_english: "Alt+E",
        hotkey_japanese: "Alt+J",
        ptt_enabled: true,
        ptt_key: "CapsLock",
        ptt_mouse_button: "X2",
        ptt_suppress: true,
        app_profiles: [],
      },
      smart: {
        enabled: true,
        endpoint: "http://127.0.0.1:18080/v1/chat/completions",
        model: "minicpm5-1b-q4",
        timeout_seconds: 10,
      },
      translation: {
        engine: "external",
        endpoint: "http://127.0.0.1:18080/v1/chat/completions",
        model: "minicpm5-1b-q4",
        timeout_seconds: 8,
        external_command: "E:/voice-ime-tools/translate.exe --stdin-json",
      },
      ui: {
        theme: "indigo-porcelain-glass",
        accent: "#315d93",
        glass_enabled: true,
      },
      history_limit: 100,
    },
    history: [
      qaHistory(1, "非洲之星和海洋之泪"),
      qaHistory(2, "明天上午九点提醒我检查模型目录"),
      qaHistory(3, "Voice IME 的 fast 模型应该优先保证响应速度"),
    ],
  };
}

function qaModelStatus() {
  return [
    {
      engine: "sherpa-onnx",
      profile: "fast",
      ready: false,
      download_url: "https://huggingface.co/example/fast",
      mirror_url: "https://hf-mirror.com/example/fast",
      target_dir: "D:/voice-ime-build-release/voice-ime-2.0.1-rust-portable/app/models/fast",
      required_files: ["model.int8.onnx", "tokens.txt"],
      missing_files: ["model.int8.onnx"],
    },
    {
      engine: "sherpa-onnx",
      profile: "balanced",
      ready: true,
      download_url: "https://huggingface.co/example/balanced",
      mirror_url: "https://hf-mirror.com/example/balanced",
      target_dir:
        "E:/voice-ime-model-packs/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
      required_files: ["model.int8.onnx", "tokens.txt"],
      missing_files: [],
    },
    {
      engine: "sherpa-onnx-whisper",
      profile: "fallback",
      ready: false,
      download_url: "https://huggingface.co/example/fallback",
      mirror_url: "https://hf-mirror.com/example/fallback",
      target_dir: "D:/voice-ime/models/sherpa-onnx-whisper-tiny",
      required_files: ["tiny-encoder.int8.onnx", "tiny-decoder.int8.onnx", "tiny-tokens.txt"],
      missing_files: ["tiny-decoder.int8.onnx"],
    },
  ];
}

function qaAudioDevices() {
  return [
    { index: 0, name: "系统默认麦克风", is_default: true },
    { index: 1, name: "USB Microphone Long Device Name For QA", is_default: false },
  ];
}

function qaAudioLevel() {
  return {
    device_name: "系统默认麦克风",
    sample_rate: 16000,
    duration_ms: 220,
    peak: 0.34,
    rms: 0.09,
    samples: 3520,
  };
}

function qaHotkeys() {
  return [
    { name: "录音", shortcut: "Alt+R", normalized: "Alt+R", status: "pass", detail: "已注册" },
    { name: "语言切换", shortcut: "Alt+Space", normalized: "Alt+Space", status: "pass", detail: "已注册" },
    { name: "转英文", shortcut: "Alt+E", normalized: "Alt+E", status: "pass", detail: "已注册" },
    { name: "转日文", shortcut: "Alt+J", normalized: "Alt+J", status: "warn", detail: "QA 冲突样例" },
  ];
}

function qaDoctorReport() {
  return {
    output_path: "D:/voice-ime/logs/doctor-qa.txt",
    summary: "诊断完成：1 项提醒",
    checks: [
      { name: "应用目录", status: "pass", detail: "可写" },
      { name: "ASR 模型", status: "warn", detail: "fallback 缺少 1 个文件" },
      { name: "热键 录音", status: "pass", detail: "Alt+R 已注册" },
    ],
  };
}

function qaRepairReport() {
  return {
    summary: "修复完成：2 项补齐，5 项已存在",
    actions: [
      { name: "日志目录", status: "skipped", detail: "已存在，未改动：D:/voice-ime/.voice_ime/logs" },
      { name: "个人提示词", status: "repaired", detail: "已创建：D:/voice-ime/.voice_ime/personal_prompt.txt" },
      { name: "热词", status: "skipped", detail: "已存在，未覆盖：D:/voice-ime/.voice_ime/hot.txt" },
    ],
    doctor: qaDoctorReport(),
  };
}

function qaLlmServiceStatus() {
  return {
    endpoint: "http://127.0.0.1:18080/v1/chat/completions",
    models_url: "http://127.0.0.1:18080/v1/models",
    is_local: true,
    reachable: false,
    script_path: "D:/voice-ime/app/tools/Start-MiniCPM-Translate.ps1",
    script_exists: true,
    model_path: "D:/voice-ime/app/models/minicpm5-1b-q4.gguf",
    model_exists: true,
    server_path: "D:/voice-ime/app/llama.cpp/cpu/llama-server.exe",
    server_exists: true,
  };
}

function qaHistory(sessionId: number, text: string) {
  return {
    session_id: sessionId,
    text,
    raw_text: text,
    normalized_text: text,
    dictionary_text: text,
    hotword_text: text,
    rule_text: text,
    itn_text: text,
    llm_text: text,
    punctuation_policy: "default",
    created_at: "2026-06-05 09:30:00",
    duration_seconds: 3.2,
    transcribe_seconds: 0.8,
    deterministic_seconds: 0.01,
    llm_seconds: 0.0,
    total_seconds: 0.95,
    backend: "sherpa-onnx",
    model: "balanced",
  };
}
