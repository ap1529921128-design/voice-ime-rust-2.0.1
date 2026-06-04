import { createElement, icons } from "lucide";
import { currentWindowLabel, invoke, listen, openDialog } from "./tauri-adapter";
import "./styles.css";

type SessionState =
  | "Idle"
  | "Recording"
  | "Previewing"
  | "Transcribing"
  | "LongTranscribing"
  | "Cancelling"
  | "Error";

type TranscriptRecord = {
  session_id: number;
  text: string;
  raw_text: string;
  normalized_text: string;
  dictionary_text: string;
  hotword_text: string;
  rule_text: string;
  itn_text: string;
  llm_text: string;
  punctuation_policy: string;
  created_at: string;
  duration_seconds: number;
  transcribe_seconds: number;
  deterministic_seconds: number;
  llm_seconds: number;
  total_seconds: number;
  backend: string;
  model: string;
};

type AppConfig = {
  asr: {
    default_engine: string;
    profile: string;
    worker_mode: string;
    language: string;
    input_device_name: string;
    sample_rate: number;
    min_record_seconds: number;
    max_record_seconds: number;
    long_transcript_seconds: number;
    long_transcript_chunk_seconds: number;
    save_long_recordings: boolean;
    num_threads: number;
    models: {
      sense_voice_model: string;
      sense_voice_tokens: string;
      zipformer_ctc_model: string;
      zipformer_ctc_tokens: string;
      whisper_encoder: string;
      whisper_decoder: string;
      whisper_tokens: string;
    };
  };
  input: {
    mode: string;
    tsf_phase: string;
    paste_delay_ms: number;
    hotkey_record: string;
    hotkey_language: string;
    hotkey_english: string;
    hotkey_japanese: string;
    ptt_enabled: boolean;
    ptt_key: string;
    ptt_mouse_button: string;
    ptt_suppress: boolean;
    app_profiles: {
      name: string;
      process_name: string;
      class_name: string;
      title_contains: string;
      output_mode: string;
      paste_delay_ms: number | null;
      punctuation: string;
    }[];
  };
  smart: {
    enabled: boolean;
    endpoint: string;
    model: string;
    timeout_seconds: number;
  };
  translation: {
    engine: string;
    endpoint: string;
    model: string;
    timeout_seconds: number;
    external_command: string;
  };
  ui: {
    theme: string;
    accent: string;
    glass_enabled: boolean;
  };
  history_limit: number;
};

type Snapshot = {
  state: SessionState;
  text: string;
  status: string;
  meta: string;
  language: string;
  word_count: number;
  config: AppConfig;
  history: TranscriptRecord[];
};

type AsrModelStatus = {
  engine: string;
  profile: string;
  ready: boolean;
  download_url: string;
  mirror_url: string;
  target_dir: string;
  required_files: string[];
  missing_files: string[];
};

type AudioDeviceInfo = {
  index: number;
  name: string;
  is_default: boolean;
};

type AudioLevelInfo = {
  device_name: string;
  sample_rate: number;
  duration_ms: number;
  peak: number;
  rms: number;
  samples: number;
};

type DoctorCheck = {
  name: string;
  status: "pass" | "warn" | "fail";
  detail: string;
};

type DoctorReport = {
  output_path: string;
  summary: string;
  checks: DoctorCheck[];
};

type RepairAction = {
  name: string;
  status: "repaired" | "skipped" | "failed";
  detail: string;
};

type RepairReport = {
  summary: string;
  actions: RepairAction[];
  doctor: DoctorReport;
};

type HotkeyCheck = {
  name: string;
  shortcut: string;
  normalized: string;
  status: "pass" | "warn" | "fail";
  detail: string;
};

type ModelProfile = "fast" | "balanced" | "fallback";

const app = document.querySelector<HTMLDivElement>("#app")!;
const isOverlay = currentWindowLabel() === "overlay";
let snapshot: Snapshot | null = null;
let statusRows: AsrModelStatus[] = [];
let audioDevices: AudioDeviceInfo[] = [];
let audioLevel: AudioLevelInfo | null = null;
let audioDeviceError = "";
let audioLevelError = "";
let audioProbeTimer: number | undefined;
let audioProbeInFlight = false;
let doctorReport: DoctorReport | null = null;
let repairReport: RepairReport | null = null;
let hotkeyRows: HotkeyCheck[] = [];
let activeView: "compose" | "settings" | "history" = "compose";
let activeSettingsTab: "voice" | "models" | "smart" | "shortcuts" | "data" = "voice";
let historyQuery = "";
let historyBackend = "all";
let historyModel = "all";
let historyDate = "";
let pendingTextSync: number | undefined;

type IconName = keyof typeof icons;

const icon = (name: IconName, label: string) => {
  const node = createElement(icons[name]);
  node.setAttribute("aria-label", label);
  node.setAttribute("width", "20");
  node.setAttribute("height", "20");
  node.setAttribute("stroke-width", "1.8");
  return node.outerHTML;
};

function languageLabel(language: string) {
  return language === "en" ? "English" : language === "ja" ? "日本語" : "中文";
}

function workerModeLabel(mode: string) {
  return mode === "isolated" ? "隔离" : "常驻";
}

function pttLabel(config: AppConfig) {
  if (!config.input.ptt_enabled) return "PTT 关";
  const triggers = [config.input.ptt_key, config.input.ptt_mouse_button].filter((item) => item && item !== "off");
  if (triggers.length === 0) return "PTT 待配置";
  return `按住 ${triggers.join(" / ")}`;
}

function selectedMicrophoneLabel(config: AppConfig) {
  const configured = config.asr.input_device_name.trim();
  if (configured) return configured;
  const current = audioLevel?.device_name || audioDevices.find((device) => device.is_default)?.name;
  return current ? `系统默认 · ${current}` : "系统默认";
}

function microphoneOptions(current: string) {
  const options = [option("", current, "系统默认")];
  if (current && !audioDevices.some((device) => device.name === current)) {
    options.push(option(current, current, `${current} · 未枚举`));
  }
  for (const device of audioDevices) {
    const label = device.is_default ? `${device.name} · 默认` : device.name;
    options.push(option(device.name, current, label));
  }
  return options.join("");
}

function audioMeterMarkup(config: AppConfig) {
  const percent = audioMeterPercent();
  const status = audioLevelError || audioDeviceError || selectedMicrophoneLabel(config);
  const detail = audioLevel
    ? `peak ${audioLevel.peak.toFixed(3)} · rms ${audioLevel.rms.toFixed(3)} · ${audioLevel.sample_rate}Hz`
    : "等待麦克风电平";
  return `
    <div class="audio-meter" data-audio-meter>
      <div class="audio-meter-head">
        <span>输入电平</span>
        <strong data-audio-meter-label>${escapeHtml(status)}</strong>
      </div>
      <div class="audio-meter-track">
        <i data-audio-meter-fill style="width: ${percent}%"></i>
      </div>
      <small data-audio-meter-detail>${escapeHtml(detail)}</small>
    </div>
  `;
}

function stateTone(state: SessionState) {
  if (state === "Recording") return "recording";
  if (
    state === "Transcribing" ||
    state === "LongTranscribing" ||
    state === "Previewing" ||
    state === "Cancelling"
  )
    return "working";
  if (state === "Error") return "error";
  return "idle";
}

function render() {
  if (!snapshot) {
    stopAudioProbe();
    app.innerHTML = `<div class="boot">Voice IME</div>`;
    return;
  }
  if (isOverlay) {
    stopAudioProbe();
    renderOverlay(snapshot);
  } else {
    renderMain(snapshot);
  }
}

function renderOverlay(data: Snapshot) {
  app.innerHTML = `
    <main class="overlay-shell ${stateTone(data.state)}">
      <header class="overlay-head" data-tauri-drag-region>
        <div class="pulse"></div>
        <div>
          <div class="overlay-status">${data.status}</div>
          <div class="overlay-meta">${data.meta || languageLabel(data.language)}</div>
        </div>
        <button class="icon-btn ghost tiny" data-action="hide">${icon("X", "关闭")}</button>
      </header>
      <textarea class="overlay-text" data-field="text">${escapeHtml(data.text)}</textarea>
      <footer class="overlay-actions">
        <button class="tool-btn primary" data-action="${data.state === "Recording" ? "stop" : "start"}">
          ${icon(data.state === "Recording" ? "Square" : "Mic", data.state === "Recording" ? "停止" : "开始")}
          <span>${data.state === "Recording" ? "停止" : "录音"}</span>
        </button>
        <button class="icon-btn" data-action="confirm" title="确认输入">${icon("Check", "确认输入")}</button>
        <button class="icon-btn" data-action="copy" title="复制">${icon("Copy", "复制")}</button>
        <button class="icon-btn" data-action="clear" title="清空">${icon("Eraser", "清空")}</button>
      </footer>
    </main>
  `;
  wireCommon();
}

function renderMain(data: Snapshot) {
  app.innerHTML = `
    <main class="app-shell">
      <section class="window" data-tauri-drag-region>
        <header class="titlebar">
          <div>
            <h1>Voice IME 2.0.1</h1>
            <p>个人语音输入法</p>
          </div>
          <div class="status-chip ${stateTone(data.state)}">${data.status}</div>
        </header>
        <nav class="tabs">
          <button class="${activeView === "compose" ? "active" : ""}" data-view="compose">${icon("Mic", "输入")}输入</button>
          <button class="${activeView === "settings" ? "active" : ""}" data-view="settings">${icon("Settings", "设置")}设置</button>
          <button class="${activeView === "history" ? "active" : ""}" data-view="history">${icon("History", "历史")}历史</button>
        </nav>
        ${activeView === "compose" ? composeView(data) : activeView === "settings" ? settingsView(data) : historyView(data)}
      </section>
    </main>
  `;
  wireCommon();
  wireMain();
  syncAudioProbe(data);
  paintAudioMeter();
}

function composeView(data: Snapshot) {
  return `
    <section class="compose-grid">
      <div class="listen-panel">
        <button class="record-button ${data.state === "Recording" ? "active" : ""}" data-action="${data.state === "Recording" ? "stop" : "start"}">
          ${icon(data.state === "Recording" ? "Square" : "Mic", data.state === "Recording" ? "停止录音" : "开始录音")}
        </button>
        <div class="listen-copy">
          <strong>${data.state === "Recording" ? "正在录音" : "准备输入"}</strong>
          <span>${languageLabel(data.language)} · ${data.config.asr.profile} · ${workerModeLabel(data.config.asr.worker_mode)} · ${pttLabel(data.config)}</span>
        </div>
        ${audioMeterMarkup(data.config)}
      </div>
      <div class="meta-strip">
        <span>${data.word_count} 字</span>
        <span>${data.meta || "等待操作"}</span>
      </div>
      <textarea class="editor" data-field="text" spellcheck="false">${escapeHtml(data.text)}</textarea>
      <footer class="action-row">
        <button class="tool-btn success" data-action="confirm">${icon("Check", "确认输入")}<span>确认输入</span></button>
        <button class="tool-btn" data-action="copy">${icon("Copy", "复制")}<span>复制</span></button>
        <button class="tool-btn" data-action="clear">${icon("Eraser", "清空")}<span>清空</span></button>
        <div class="spacer"></div>
        <button class="tool-btn" data-action="translate-en">${icon("Languages", "英译")}<span>英</span></button>
        <button class="tool-btn" data-action="translate-ja">${icon("Languages", "日译")}<span>日</span></button>
        <button class="tool-btn" data-action="translate-zh">${icon("Languages", "中译")}<span>中</span></button>
      </footer>
    </section>
  `;
}

function settingsView(data: Snapshot) {
  const cfg = data.config;
  return `
    <section class="settings-grid">
      <div class="settings-notice ${stateTone(data.state)}">
        <strong>${escapeHtml(data.status)}</strong>
        <span>${escapeHtml(data.meta || "模型状态会在这里更新")}</span>
      </div>
      <nav class="settings-tabs">
        ${settingsTabButton("voice", "SlidersHorizontal", "语音")}
        ${settingsTabButton("models", "Boxes", "模型")}
        ${settingsTabButton("smart", "Sparkles", "智能")}
        ${settingsTabButton("shortcuts", "Keyboard", "快捷键")}
        ${settingsTabButton("data", "Database", "数据")}
      </nav>
      ${settingsPanel(cfg)}
      <footer class="settings-actions">
        <button class="tool-btn primary" data-action="save-config">${icon("Check", "保存")}<span>保存设置</span></button>
      </footer>
    </section>
  `;
}

function settingsTabButton(tab: typeof activeSettingsTab, iconName: IconName, label: string) {
  return `<button class="${activeSettingsTab === tab ? "active" : ""}" data-settings-tab="${tab}">${icon(iconName, label)}<span>${label}</span></button>`;
}

function settingsPanel(cfg: AppConfig) {
  if (activeSettingsTab === "models") return modelSettingsPanel(cfg);
  if (activeSettingsTab === "smart") return smartSettingsPanel(cfg);
  if (activeSettingsTab === "shortcuts") return shortcutSettingsPanel(cfg);
  if (activeSettingsTab === "data") return dataSettingsPanel(cfg);
  return voiceSettingsPanel(cfg);
}

function voiceSettingsPanel(cfg: AppConfig) {
  return `
    <div class="settings-panel">
      <label>ASR 档位
        <select data-config="asr.profile">
          ${option("fast", cfg.asr.profile, "fast")}
          ${option("balanced", cfg.asr.profile, "balanced")}
          ${option("fallback", cfg.asr.profile, "fallback")}
        </select>
      </label>
      <label>输入语言
        <select data-config="asr.language">
          ${option("zh", cfg.asr.language, "中文")}
          ${option("en", cfg.asr.language, "English")}
          ${option("ja", cfg.asr.language, "日本語")}
        </select>
      </label>
      <label>麦克风
        <select data-config="asr.input_device_name">
          ${microphoneOptions(cfg.asr.input_device_name)}
        </select>
      </label>
      <label>ASR 进程
        <select data-config="asr.worker_mode">
          ${option("persistent", cfg.asr.worker_mode, "常驻加速")}
          ${option("isolated", cfg.asr.worker_mode, "隔离稳妥")}
        </select>
      </label>
      <label>最大录音秒数
        <input type="number" min="5" max="600" value="${cfg.asr.max_record_seconds}" data-config="asr.max_record_seconds" />
      </label>
      <label>长文阈值秒数
        <input type="number" min="10" max="600" value="${cfg.asr.long_transcript_seconds}" data-config="asr.long_transcript_seconds" />
      </label>
      <label>ASR 线程
        <input type="number" min="1" max="4" value="${cfg.asr.num_threads}" data-config="asr.num_threads" />
      </label>
      <div class="settings-tools">
        <button class="tool-btn" data-action="refresh-audio-devices">${icon("RefreshCw", "刷新麦克风")}<span>刷新麦克风</span></button>
      </div>
      <div class="settings-meter">
        ${audioMeterMarkup(cfg)}
      </div>
    </div>
  `;
}

function shortcutSettingsPanel(cfg: AppConfig) {
  return `
    <div class="settings-panel">
      <label>录音热键
        <input value="${escapeAttr(cfg.input.hotkey_record)}" data-config="input.hotkey_record" />
      </label>
      <label>语言切换
        <input value="${escapeAttr(cfg.input.hotkey_language)}" data-config="input.hotkey_language" />
      </label>
      <label>转英文
        <input value="${escapeAttr(cfg.input.hotkey_english)}" data-config="input.hotkey_english" />
      </label>
      <label>转日文
        <input value="${escapeAttr(cfg.input.hotkey_japanese)}" data-config="input.hotkey_japanese" />
      </label>
      <label>按住说话
        <select data-config="input.ptt_enabled">
          ${option("true", String(cfg.input.ptt_enabled), "开启")}
          ${option("false", String(cfg.input.ptt_enabled), "关闭")}
        </select>
      </label>
      <label>键盘触发
        <select data-config="input.ptt_key">
          ${option("CapsLock", cfg.input.ptt_key, "CapsLock")}
          ${option("F8", cfg.input.ptt_key, "F8")}
          ${option("F9", cfg.input.ptt_key, "F9")}
          ${option("F10", cfg.input.ptt_key, "F10")}
          ${option("F13", cfg.input.ptt_key, "F13")}
          ${option("off", cfg.input.ptt_key, "关闭")}
        </select>
      </label>
      <label>鼠标触发
        <select data-config="input.ptt_mouse_button">
          ${option("X2", cfg.input.ptt_mouse_button, "X2")}
          ${option("X1", cfg.input.ptt_mouse_button, "X1")}
          ${option("off", cfg.input.ptt_mouse_button, "关闭")}
        </select>
      </label>
      <label>触发键吞掉
        <select data-config="input.ptt_suppress">
          ${option("true", String(cfg.input.ptt_suppress), "开启")}
          ${option("false", String(cfg.input.ptt_suppress), "关闭")}
        </select>
      </label>
      ${hotkeyStatusPanel()}
    </div>
  `;
}

function hotkeyStatusPanel() {
  if (hotkeyRows.length === 0) {
    return `
      <div class="hotkey-panel empty-diagnostics">
        <div class="doctor-head">
          <strong>热键状态</strong>
          <span>等待注册</span>
        </div>
      </div>
    `;
  }
  return `
    <div class="hotkey-panel">
      <div class="doctor-head">
        <strong>热键状态</strong>
        <span>${hotkeyRows.filter((row) => row.status === "pass").length}/${hotkeyRows.length} 可用</span>
      </div>
      <div class="doctor-list">
        ${hotkeyRows.map((row) => hotkeyRow(row)).join("")}
      </div>
    </div>
  `;
}

function hotkeyRow(row: HotkeyCheck) {
  const statusIcon = row.status === "pass" ? "CheckCircle2" : row.status === "warn" ? "TriangleAlert" : "CircleX";
  const label = row.status === "pass" ? "通过" : row.status === "warn" ? "提醒" : "失败";
  const detail = `${row.normalized || row.shortcut} · ${row.detail}`;
  return `
    <div class="doctor-row ${row.status}">
      ${icon(statusIcon as IconName, label)}
      <strong>${escapeHtml(row.name)}</strong>
      <span title="${escapeAttr(detail)}">${escapeHtml(detail)}</span>
    </div>
  `;
}

function smartSettingsPanel(cfg: AppConfig) {
  return `
    <div class="settings-panel">
      <label>智能纠错
        <select data-config="smart.enabled">
          ${option("true", String(cfg.smart.enabled), "开启")}
          ${option("false", String(cfg.smart.enabled), "关闭")}
        </select>
      </label>
      <label>智能端点
        <input value="${escapeAttr(cfg.smart.endpoint)}" data-config="smart.endpoint" />
      </label>
      <label>纠错模型
        <input value="${escapeAttr(cfg.smart.model)}" data-config="smart.model" />
      </label>
      <label>翻译引擎
        <select data-config="translation.engine">
          ${option("llm", cfg.translation.engine, "本地 LLM")}
          ${option("external", cfg.translation.engine, "外部命令")}
          ${option("nllb", cfg.translation.engine, "NLLB 预留")}
          ${option("bergamot", cfg.translation.engine, "Bergamot 预留")}
        </select>
      </label>
      <label>翻译端点
        <input value="${escapeAttr(cfg.translation.endpoint)}" data-config="translation.endpoint" />
      </label>
      <label>翻译模型
        <input value="${escapeAttr(cfg.translation.model)}" data-config="translation.model" />
      </label>
      <label>翻译超时
        <input type="number" min="3" max="8" value="${cfg.translation.timeout_seconds}" data-config="translation.timeout_seconds" />
      </label>
      <label>外部翻译命令
        <input value="${escapeAttr(cfg.translation.external_command)}" data-config="translation.external_command" />
      </label>
    </div>
  `;
}

function modelSettingsPanel(cfg: AppConfig) {
  return `
    <div class="settings-panel">
      <div class="model-status">
        ${statusRows
          .map(
            (row) => `
          <div class="model-row ${row.ready ? "ready" : "missing"}">
            <div class="model-main">
              <strong>${row.profile}</strong>
              <span>${row.ready ? "ready" : `${row.missing_files.length} missing`}</span>
              <small title="${escapeAttr(row.target_dir)}">${escapeHtml(shortPath(row.target_dir))}</small>
            </div>
            <div class="model-actions">
              <button class="mini-action" data-action="download-model" data-profile="${escapeAttr(row.profile)}" title="下载模型">${icon("Download", "下载模型")}<span>下载</span></button>
              <button class="mini-action" data-action="pick-model-dir" data-profile="${escapeAttr(row.profile)}" title="选择模型目录">${icon("FolderSearch", "选择模型目录")}<span>选择</span></button>
              <button class="mini-action" data-action="open-model-mirror" data-profile="${escapeAttr(row.profile)}" title="打开镜像页">${icon("Cloud", "打开镜像页")}<span>镜像</span></button>
              <button class="mini-action" data-action="open-model-page" data-profile="${escapeAttr(row.profile)}" title="打开官方页">${icon("ExternalLink", "打开官方页")}<span>官网</span></button>
            </div>
          </div>`,
          )
          .join("")}
      </div>
      ${modelPathField("fast 模型", "asr.models.zipformer_ctc_model", cfg.asr.models.zipformer_ctc_model)}
      ${modelPathField("fast tokens", "asr.models.zipformer_ctc_tokens", cfg.asr.models.zipformer_ctc_tokens)}
      ${modelPathField("balanced 模型", "asr.models.sense_voice_model", cfg.asr.models.sense_voice_model)}
      ${modelPathField("balanced tokens", "asr.models.sense_voice_tokens", cfg.asr.models.sense_voice_tokens)}
      ${modelPathField("fallback encoder", "asr.models.whisper_encoder", cfg.asr.models.whisper_encoder)}
      ${modelPathField("fallback decoder", "asr.models.whisper_decoder", cfg.asr.models.whisper_decoder)}
      ${modelPathField("fallback tokens", "asr.models.whisper_tokens", cfg.asr.models.whisper_tokens)}
      <div class="settings-tools">
        <button class="tool-btn" data-action="open-model-dir">${icon("FolderOpen", "打开模型目录")}<span>模型目录</span></button>
        <button class="tool-btn" data-action="prewarm-asr">${icon("Flame", "预热 ASR")}<span>预热</span></button>
      </div>
    </div>
  `;
}

function modelPathField(label: string, configPath: string, value: string) {
  return `
    <label class="path-field"><span>${escapeHtml(label)}</span>
      <div class="path-input">
        <input value="${escapeAttr(value)}" data-config="${escapeAttr(configPath)}" />
        <button class="icon-btn tiny" data-action="pick-model-file" data-config-path="${escapeAttr(configPath)}" title="选择文件">${icon("FileSearch", "选择文件")}</button>
      </div>
    </label>
  `;
}

function dataSettingsPanel(cfg: AppConfig) {
  return `
    <div class="settings-panel">
      <label>历史上限
        <input type="number" min="0" max="500" value="${cfg.history_limit}" data-config="history_limit" />
      </label>
      <label>长录音留存
        <select data-config="asr.save_long_recordings">
          ${option("true", String(cfg.asr.save_long_recordings), "保存")}
          ${option("false", String(cfg.asr.save_long_recordings), "不保存")}
        </select>
      </label>
      <label>短录音留存
        <select disabled>
          <option>永不保存</option>
        </select>
      </label>
      <div class="settings-tools">
        <button class="tool-btn" data-action="open-logs-dir">${icon("FileText", "打开日志")}<span>日志</span></button>
        <button class="tool-btn" data-action="run-doctor">${icon("Stethoscope", "运行诊断")}<span>诊断</span></button>
        <button class="tool-btn" data-action="repair-doctor">${icon("Wrench", "修复诊断")}<span>修复</span></button>
        <button class="tool-btn" data-action="export-diagnostics">${icon("Archive", "导出诊断")}<span>导出</span></button>
        <button class="tool-btn" data-action="export-history-csv">${icon("Download", "导出历史")}<span>历史 CSV</span></button>
        <button class="tool-btn danger" data-action="clear-recordings">${icon("Trash2", "清理录音")}<span>清理录音</span></button>
        <button class="tool-btn" data-action="open-hotwords">${icon("BookOpen", "打开热词")}<span>热词</span></button>
        <button class="tool-btn" data-action="open-hot-rules">${icon("ListChecks", "打开规则")}<span>规则</span></button>
        <button class="tool-btn danger" data-action="clear-history">${icon("Eraser", "清空历史")}<span>清空历史</span></button>
      </div>
      ${doctorPanel()}
    </div>
  `;
}

function doctorPanel() {
  if (!doctorReport) {
    return `
      <div class="doctor-panel empty-diagnostics">
        <div class="doctor-head">
          <strong>本地诊断</strong>
          <span>尚未运行</span>
        </div>
      </div>
    `;
  }
  return `
    <div class="doctor-panel">
      <div class="doctor-head">
        <strong>${escapeHtml(doctorReport.summary)}</strong>
        <span title="${escapeAttr(doctorReport.output_path)}">${escapeHtml(shortPath(doctorReport.output_path))}</span>
      </div>
      <div class="doctor-list">
        ${doctorReport.checks.map((check) => doctorRow(check)).join("")}
      </div>
      ${repairActionsPanel()}
    </div>
  `;
}

function repairActionsPanel() {
  if (!repairReport) return "";
  return `
    <div class="doctor-list repair-list">
      ${repairReport.actions.map((action) => repairRow(action)).join("")}
    </div>
  `;
}

function doctorRow(check: DoctorCheck) {
  const statusIcon = check.status === "pass" ? "CheckCircle2" : check.status === "warn" ? "TriangleAlert" : "CircleX";
  const label = check.status === "pass" ? "通过" : check.status === "warn" ? "提醒" : "失败";
  return `
    <div class="doctor-row ${check.status}">
      ${icon(statusIcon as IconName, label)}
      <strong>${escapeHtml(check.name)}</strong>
      <span>${escapeHtml(check.detail)}</span>
    </div>
  `;
}

function repairRow(action: RepairAction) {
  const statusIcon = action.status === "repaired" ? "CheckCircle2" : action.status === "skipped" ? "CircleMinus" : "CircleX";
  const label = action.status === "repaired" ? "已补齐" : action.status === "skipped" ? "已跳过" : "失败";
  return `
    <div class="doctor-row repair-${action.status}">
      ${icon(statusIcon as IconName, label)}
      <strong>${escapeHtml(action.name)}</strong>
      <span>${escapeHtml(action.detail)}</span>
    </div>
  `;
}

function historyView(data: Snapshot) {
  const rows = filteredHistoryRows(data.history);
  return `
    <section class="history-list">
      <div class="history-filters">
        <label>搜索
          <input value="${escapeAttr(historyQuery)}" data-history-filter="query" placeholder="文本 / 模型 / 后端" />
        </label>
        <label>后端
          <select data-history-filter="backend">
            ${option("all", historyBackend, "全部")}
            ${historyOptions(data.history.map((record) => record.backend), historyBackend)}
          </select>
        </label>
        <label>模型
          <select data-history-filter="model">
            ${option("all", historyModel, "全部")}
            ${historyOptions(data.history.map((record) => record.model), historyModel)}
          </select>
        </label>
        <label>日期
          <input type="date" value="${escapeAttr(historyDate)}" data-history-filter="date" />
        </label>
        <button class="icon-btn" data-action="reset-history-filters" title="重置筛选">${icon("RotateCcw", "重置筛选")}</button>
      </div>
      <div class="history-count">${rows.length} / ${data.history.length}</div>
      ${rows
        .map(({ record, index }) => {
          const totalSeconds = record.total_seconds || record.transcribe_seconds;
          return `
        <article class="history-item" data-history="${index}">
          <p>${escapeHtml(record.text)}</p>
          <footer>${record.created_at} · 录音 ${record.duration_seconds.toFixed(1)}s · ASR ${record.transcribe_seconds.toFixed(1)}s · 总 ${totalSeconds.toFixed(1)}s · ${escapeHtml(record.backend)}</footer>
          ${historyTrace(record)}
        </article>`;
        })
        .join("") || `<div class="empty">暂无匹配</div>`}
      <div class="history-actions">
        <button class="tool-btn" data-action="export-history-csv">${icon("Download", "导出历史")}<span>导出 CSV</span></button>
        <button class="tool-btn danger" data-action="clear-history">${icon("Eraser", "清空历史")}<span>清空历史</span></button>
      </div>
    </section>
  `;
}

function filteredHistoryRows(records: TranscriptRecord[]) {
  const query = historyQuery.trim().toLowerCase();
  return records
    .map((record, index) => ({ record, index }))
    .filter(({ record }) => {
      if (historyBackend !== "all" && record.backend !== historyBackend) return false;
      if (historyModel !== "all" && record.model !== historyModel) return false;
      if (historyDate && !record.created_at.startsWith(historyDate)) return false;
      if (!query) return true;
      return historySearchText(record).toLowerCase().includes(query);
    });
}

function historySearchText(record: TranscriptRecord) {
  return [
    record.text,
    record.raw_text,
    record.normalized_text,
    record.dictionary_text,
    record.hotword_text,
    record.rule_text,
    record.itn_text,
    record.llm_text,
    record.backend,
    record.model,
    record.created_at,
    record.punctuation_policy,
    String(record.session_id || ""),
  ].join("\n");
}

function historyOptions(values: string[], current: string) {
  return Array.from(new Set(values.filter((value) => value && value.trim().length > 0)))
    .sort((a, b) => a.localeCompare(b))
    .map((value) => option(value, current, value))
    .join("");
}

function historyTrace(record: TranscriptRecord) {
  const deterministicSeconds = Number(record.deterministic_seconds || 0);
  const llmSeconds = Number(record.llm_seconds || 0);
  const rows = [
    ["原始", record.raw_text],
    ["词表", record.dictionary_text],
    ["热词", record.hotword_text],
    ["规则", record.rule_text],
    ["ITN", record.itn_text],
    ["LLM", record.llm_text],
  ].filter(([, value]) => value && value.trim().length > 0);
  if (rows.length === 0) return "";
  return `
    <details class="history-trace">
      <summary>过程 · 清理 ${deterministicSeconds.toFixed(2)}s · LLM ${llmSeconds.toFixed(2)}s</summary>
      <dl>
        ${rows
          .map(
            ([label, value]) => `
          <div>
            <dt>${escapeHtml(label)}</dt>
            <dd>${escapeHtml(value)}</dd>
          </div>`,
          )
          .join("")}
      </dl>
    </details>
  `;
}

function wireCommon() {
  app.querySelectorAll<HTMLTextAreaElement>("[data-field='text']").forEach((field) => {
    field.addEventListener("input", () => scheduleTextSync(field.value));
    field.addEventListener("blur", () => {
      void flushTextSync(field.value);
    });
  });
  app.querySelectorAll<HTMLElement>("[data-action]").forEach((button) => {
    button.addEventListener("click", async () => {
      const action = button.dataset.action!;
      if (["confirm", "copy", "translate-en", "translate-ja", "translate-zh"].includes(action)) {
        await flushActiveTextField();
      }
      if (action === "start") {
        stopAudioProbe();
        await run("start_recording");
      }
      if (action === "stop") await run("stop_recording");
      if (action === "confirm") await run("confirm_input");
      if (action === "copy") await run("copy_text");
      if (action === "clear") await run("clear_text");
      if (action === "translate-en") await run("translate_text", { targetLanguage: "en" });
      if (action === "translate-ja") await run("translate_text", { targetLanguage: "ja" });
      if (action === "translate-zh") await run("translate_text", { targetLanguage: "zh" });
      if (action === "hide") await run("hide_overlay");
      if (action === "clear-history") await run("clear_history");
      if (action === "clear-recordings") await run("clear_recordings");
      if (action === "reset-history-filters") resetHistoryFilters();
      if (action === "save-config") await saveConfig();
      if (action === "refresh-audio-devices") await refreshAudioDevices(true);
      if (action === "download-model") await downloadModel(button.dataset.profile || "");
      if (action === "pick-model-file") await pickModelFile(button.dataset.configPath || "");
      if (action === "pick-model-dir") await pickModelDirectory(button.dataset.profile || "");
      if (action === "prewarm-asr") await run("prewarm_asr");
      if (action === "open-model-mirror") await invoke("open_model_mirror_page", { profile: button.dataset.profile || "" });
      if (action === "open-model-page") await invoke("open_model_download_page", { profile: button.dataset.profile || "" });
      if (action === "open-model-dir") await invoke("open_models_dir");
      if (action === "open-logs-dir") await invoke("open_logs_dir");
      if (action === "run-doctor") await runDoctorReport();
      if (action === "repair-doctor") await repairDoctorReport();
      if (action === "export-diagnostics") await run("export_diagnostics");
      if (action === "export-history-csv") await run("export_history_csv");
      if (action === "open-hotwords") await invoke("open_hotwords_file");
      if (action === "open-hot-rules") await invoke("open_hot_rules_file");
    });
  });
}

function scheduleTextSync(text: string) {
  if (snapshot) {
    snapshot.text = text;
    snapshot.word_count = Array.from(text).length;
  }
  if (pendingTextSync !== undefined) {
    window.clearTimeout(pendingTextSync);
  }
  pendingTextSync = window.setTimeout(() => {
    void flushTextSync(text);
  }, 180);
}

async function flushActiveTextField() {
  const active = document.activeElement;
  if (active instanceof HTMLTextAreaElement && active.dataset.field === "text") {
    await flushTextSync(active.value);
  }
}

async function flushTextSync(text: string) {
  if (pendingTextSync !== undefined) {
    window.clearTimeout(pendingTextSync);
    pendingTextSync = undefined;
  }
  try {
    const result = await invoke<Snapshot>("set_text", { text });
    snapshot = result;
  } catch (error) {
    if (snapshot) {
      snapshot.status = "出错";
      snapshot.meta = String(error);
      render();
    }
  }
}

function wireMain() {
  app.querySelectorAll<HTMLButtonElement>("[data-view]").forEach((tab) => {
    tab.addEventListener("click", async () => {
      activeView = tab.dataset.view as typeof activeView;
      if (activeView === "settings") {
        statusRows = await invoke<AsrModelStatus[]>("asr_status");
        await refreshHotkeyStatus();
      }
      render();
    });
  });
  app.querySelectorAll<HTMLButtonElement>("[data-settings-tab]").forEach((tab) => {
    tab.addEventListener("click", async () => {
      activeSettingsTab = tab.dataset.settingsTab as typeof activeSettingsTab;
      if (activeSettingsTab === "voice") await refreshAudioDevices(false);
      if (activeSettingsTab === "shortcuts") await refreshHotkeyStatus();
      render();
    });
  });
  app.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-config]").forEach((input) => {
    const syncDraft = () => {
      if (snapshot) setPath(snapshot.config, input.dataset.config!, input.value);
      if (input.dataset.config === "asr.input_device_name") {
        audioLevel = null;
        paintAudioMeter();
        void probeAudioLevel();
      }
    };
    input.addEventListener("input", syncDraft);
    input.addEventListener("change", syncDraft);
  });
  app.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-history-filter]").forEach((input) => {
    input.addEventListener("input", () => updateHistoryFilter(input));
    input.addEventListener("change", () => updateHistoryFilter(input));
  });
  app.querySelectorAll<HTMLElement>("[data-history]").forEach((item) => {
    item.addEventListener("dblclick", () => {
      const index = Number(item.dataset.history);
      const record = snapshot?.history[index];
      if (record) run("set_text", { text: record.text });
      activeView = "compose";
      render();
    });
  });
}

function updateHistoryFilter(input: HTMLInputElement | HTMLSelectElement) {
  const filter = input.dataset.historyFilter;
  const cursor = input instanceof HTMLInputElement ? input.selectionStart : null;
  if (filter === "query") historyQuery = input.value;
  if (filter === "backend") historyBackend = input.value;
  if (filter === "model") historyModel = input.value;
  if (filter === "date") historyDate = input.value;
  render();
  if (filter === "query") {
    const next = app.querySelector<HTMLInputElement>("[data-history-filter='query']");
    next?.focus();
    if (next && cursor !== null) next.setSelectionRange(cursor, cursor);
  }
}

function resetHistoryFilters() {
  historyQuery = "";
  historyBackend = "all";
  historyModel = "all";
  historyDate = "";
  render();
}

async function saveConfig() {
  if (!snapshot) return;
  await saveConfigDraft(collectConfigDraft());
}

function collectConfigDraft() {
  if (!snapshot) throw new Error("配置尚未加载");
  const next = structuredClone(snapshot.config);
  app.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-config]").forEach((input) => {
    setPath(next, input.dataset.config!, input.value);
  });
  return next;
}

async function saveConfigDraft(next: AppConfig) {
  await run("save_config", { config: next });
  if (activeView === "settings") {
    if (activeSettingsTab === "models") await refreshModelStatus();
    if (activeSettingsTab === "shortcuts") await refreshHotkeyStatus();
    render();
  }
}

async function refreshModelStatus() {
  statusRows = await invoke<AsrModelStatus[]>("asr_status");
}

async function refreshHotkeyStatus() {
  hotkeyRows = await invoke<HotkeyCheck[]>("hotkey_status");
}

async function pickModelFile(configPath: string) {
  if (!configPath || !snapshot) return;
  const selected = await openDialog({
    multiple: false,
    directory: false,
    title: "选择模型文件",
  });
  if (typeof selected !== "string") return;
  const next = collectConfigDraft();
  setPath(next, configPath, selected);
  await saveConfigDraft(next);
}

async function pickModelDirectory(profile: string) {
  if (!isModelProfile(profile) || !snapshot) return;
  const selected = await openDialog({
    multiple: false,
    directory: true,
    title: "选择模型目录",
  });
  if (typeof selected !== "string") return;
  const next = collectConfigDraft();
  applyModelDirectory(next, profile, selected);
  await saveConfigDraft(next);
}

function isModelProfile(value: string): value is ModelProfile {
  return value === "fast" || value === "balanced" || value === "fallback";
}

function applyModelDirectory(config: AppConfig, profile: ModelProfile, dir: string) {
  const files: Record<ModelProfile, Array<[string, string]>> = {
    fast: [
      ["asr.models.zipformer_ctc_model", "model.int8.onnx"],
      ["asr.models.zipformer_ctc_tokens", "tokens.txt"],
    ],
    balanced: [
      ["asr.models.sense_voice_model", "model.int8.onnx"],
      ["asr.models.sense_voice_tokens", "tokens.txt"],
    ],
    fallback: [
      ["asr.models.whisper_encoder", "tiny-encoder.int8.onnx"],
      ["asr.models.whisper_decoder", "tiny-decoder.int8.onnx"],
      ["asr.models.whisper_tokens", "tiny-tokens.txt"],
    ],
  };
  files[profile].forEach(([configPath, filename]) => setPath(config, configPath, joinPickedPath(dir, filename)));
}

function joinPickedPath(dir: string, filename: string) {
  const clean = dir.replace(/[\\/]+$/, "");
  const separator = clean.includes("\\") ? "\\" : "/";
  return `${clean}${separator}${filename}`;
}

async function runDoctorReport() {
  try {
    repairReport = null;
    doctorReport = await invoke<DoctorReport>("doctor_report");
    if (snapshot) {
      snapshot.status = "诊断完成";
      snapshot.meta = `${doctorReport.summary}；报告：${doctorReport.output_path}`;
    }
    render();
  } catch (error) {
    if (snapshot) {
      snapshot.status = "出错";
      snapshot.meta = String(error);
      render();
    }
    throw error;
  }
}

async function repairDoctorReport() {
  try {
    repairReport = await invoke<RepairReport>("repair_doctor");
    doctorReport = repairReport.doctor;
    if (snapshot) {
      snapshot.status = "修复完成";
      snapshot.meta = `${repairReport.summary}；${doctorReport.summary}`;
    }
    render();
  } catch (error) {
    if (snapshot) {
      snapshot.status = "出错";
      snapshot.meta = String(error);
      render();
    }
    throw error;
  }
}

async function refreshAudioDevices(shouldRender = false) {
  try {
    audioDevices = await invoke<AudioDeviceInfo[]>("audio_devices");
    audioDeviceError = "";
  } catch (error) {
    audioDevices = [];
    audioDeviceError = String(error);
  }
  if (shouldRender) render();
}

function shouldProbeAudio(data: Snapshot) {
  if (data.state !== "Idle") return false;
  if (activeView === "compose") return true;
  return activeView === "settings" && activeSettingsTab === "voice";
}

function syncAudioProbe(data: Snapshot) {
  if (!shouldProbeAudio(data)) {
    stopAudioProbe();
    return;
  }
  if (audioProbeTimer !== undefined) return;
  void probeAudioLevel();
  audioProbeTimer = window.setInterval(() => {
    void probeAudioLevel();
  }, 1300);
}

function stopAudioProbe() {
  if (audioProbeTimer !== undefined) {
    window.clearInterval(audioProbeTimer);
    audioProbeTimer = undefined;
  }
}

async function probeAudioLevel() {
  if (audioProbeInFlight || !snapshot || !shouldProbeAudio(snapshot)) return;
  audioProbeInFlight = true;
  try {
    const deviceName = snapshot.config.asr.input_device_name.trim() || null;
    audioLevel = await invoke<AudioLevelInfo>("audio_level", { deviceName });
    audioLevelError = "";
  } catch (error) {
    audioLevel = null;
    audioLevelError = String(error);
  } finally {
    audioProbeInFlight = false;
    paintAudioMeter();
  }
}

function audioMeterPercent() {
  if (!audioLevel || audioLevel.samples === 0) return 0;
  const level = Math.max(audioLevel.peak, audioLevel.rms * 2);
  return Math.min(100, Math.round(Math.sqrt(Math.max(0, level)) * 100));
}

function paintAudioMeter() {
  const percent = audioMeterPercent();
  app.querySelectorAll<HTMLElement>("[data-audio-meter-fill]").forEach((fill) => {
    fill.style.width = `${percent}%`;
  });
  const status = audioLevelError || audioDeviceError || (snapshot ? selectedMicrophoneLabel(snapshot.config) : "系统默认");
  app.querySelectorAll<HTMLElement>("[data-audio-meter-label]").forEach((label) => {
    label.textContent = status;
  });
  const detail = audioLevel
    ? `peak ${audioLevel.peak.toFixed(3)} · rms ${audioLevel.rms.toFixed(3)} · ${audioLevel.sample_rate}Hz`
    : "等待麦克风电平";
  app.querySelectorAll<HTMLElement>("[data-audio-meter-detail]").forEach((label) => {
    label.textContent = audioLevelError || audioDeviceError ? "麦克风不可用" : detail;
  });
}

async function downloadModel(profile: string) {
  if (!profile) return;
  await run("download_asr_model", { profile });
  await refreshModelStatus();
  render();
}

function setPath(config: AppConfig, path: string, value: string) {
  const keys = path.split(".");
  let target: Record<string, unknown> = config as unknown as Record<string, unknown>;
  for (const key of keys.slice(0, -1)) {
    target = target[key] as Record<string, unknown>;
  }
  const last = keys[keys.length - 1];
  const current = target[last];
  if (typeof current === "number") target[last] = Number(value);
  else if (typeof current === "boolean") target[last] = value === "true";
  else target[last] = value;
}

async function run<T = Snapshot>(command: string, args?: Record<string, unknown>) {
  try {
    const result = await invoke<T>(command, args);
    if (result && typeof result === "object" && "state" in result) {
      snapshot = result as unknown as Snapshot;
      render();
    }
    return result;
  } catch (error) {
    if (snapshot) {
      snapshot.status = "出错";
      snapshot.meta = String(error);
      render();
    }
    throw error;
  }
}

function option(value: string, current: string, label: string) {
  return `<option value="${escapeAttr(value)}" ${value === current ? "selected" : ""}>${escapeHtml(label)}</option>`;
}

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[ch]!);
}

function escapeAttr(value: string) {
  return escapeHtml(value).replace(/`/g, "&#96;");
}

function shortPath(value: string) {
  const normalized = value.replaceAll("\\", "/");
  const index = normalized.lastIndexOf("/models/");
  return index >= 0 ? normalized.slice(index + 1) : normalized;
}

async function bootstrap() {
  snapshot = await invoke<Snapshot>("get_snapshot");
  if (!isOverlay) await refreshAudioDevices(false);
  await listen<Snapshot>("voice-ime://snapshot", async (event) => {
    const active = document.activeElement;
    const isEditing =
      active instanceof HTMLTextAreaElement &&
      active.dataset.field === "text" &&
      event.payload.text === active.value &&
      event.payload.state === snapshot?.state;
    snapshot = event.payload;
    if (isEditing) {
      return;
    }
    if (!isOverlay && activeView === "settings") {
      await refreshModelStatus();
    }
    render();
  });
  if (!isOverlay) {
    await refreshModelStatus();
  }
  render();
}

bootstrap();
