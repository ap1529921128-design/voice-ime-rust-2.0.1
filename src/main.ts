import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createElement, icons } from "lucide";
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
  text: string;
  created_at: string;
  duration_seconds: number;
  transcribe_seconds: number;
  backend: string;
  model: string;
};

type AppConfig = {
  asr: {
    default_engine: string;
    profile: string;
    worker_mode: string;
    language: string;
    sample_rate: number;
    min_record_seconds: number;
    max_record_seconds: number;
    long_transcript_seconds: number;
    long_transcript_chunk_seconds: number;
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
  };
  smart: {
    enabled: boolean;
    endpoint: string;
    model: string;
    timeout_seconds: number;
  };
  translation: {
    endpoint: string;
    model: string;
    timeout_seconds: number;
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

const app = document.querySelector<HTMLDivElement>("#app")!;
const currentWindow = getCurrentWindow();
const isOverlay = currentWindow.label === "overlay";
let snapshot: Snapshot | null = null;
let statusRows: AsrModelStatus[] = [];
let activeView: "compose" | "settings" | "history" = "compose";
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

function stateTone(state: SessionState) {
  if (state === "Recording") return "recording";
  if (state === "Transcribing" || state === "LongTranscribing" || state === "Previewing") return "working";
  if (state === "Error") return "error";
  return "idle";
}

function render() {
  if (!snapshot) {
    app.innerHTML = `<div class="boot">Voice IME</div>`;
    return;
  }
  if (isOverlay) {
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
          <span>${languageLabel(data.language)} · ${data.config.asr.profile} · ${workerModeLabel(data.config.asr.worker_mode)}</span>
        </div>
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
      <label>翻译模型
        <input value="${escapeAttr(cfg.translation.model)}" data-config="translation.model" />
      </label>
      <label>翻译超时
        <input type="number" min="3" max="8" value="${cfg.translation.timeout_seconds}" data-config="translation.timeout_seconds" />
      </label>
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
              <button class="mini-action" data-action="open-model-mirror" data-profile="${escapeAttr(row.profile)}" title="打开镜像页">${icon("Cloud", "打开镜像页")}<span>镜像</span></button>
              <button class="mini-action" data-action="open-model-page" data-profile="${escapeAttr(row.profile)}" title="打开官方页">${icon("ExternalLink", "打开官方页")}<span>官网</span></button>
            </div>
          </div>`,
          )
          .join("")}
      </div>
      <footer class="settings-actions">
        <button class="tool-btn" data-action="open-model-dir">${icon("FolderOpen", "打开模型目录")}<span>模型目录</span></button>
        <button class="tool-btn primary" data-action="save-config">${icon("Check", "保存")}<span>保存设置</span></button>
      </footer>
    </section>
  `;
}

function historyView(data: Snapshot) {
  return `
    <section class="history-list">
      ${data.history
        .map(
          (record, index) => `
        <article class="history-item" data-history="${index}">
          <p>${escapeHtml(record.text)}</p>
          <footer>${record.created_at} · ${record.duration_seconds.toFixed(1)}s · ${record.transcribe_seconds.toFixed(1)}s · ${escapeHtml(record.backend)}</footer>
        </article>`,
        )
        .join("") || `<div class="empty">暂无历史</div>`}
      <button class="tool-btn danger" data-action="clear-history">${icon("Eraser", "清空历史")}<span>清空历史</span></button>
    </section>
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
      if (action === "start") await run("start_recording");
      if (action === "stop") await run("stop_recording");
      if (action === "confirm") await run("confirm_input");
      if (action === "copy") await run("copy_text");
      if (action === "clear") await run("clear_text");
      if (action === "translate-en") await run("translate_text", { targetLanguage: "en" });
      if (action === "translate-ja") await run("translate_text", { targetLanguage: "ja" });
      if (action === "translate-zh") await run("translate_text", { targetLanguage: "zh" });
      if (action === "hide") await run("hide_overlay");
      if (action === "clear-history") await run("clear_history");
      if (action === "save-config") await saveConfig();
      if (action === "download-model") await downloadModel(button.dataset.profile || "");
      if (action === "open-model-mirror") await invoke("open_model_mirror_page", { profile: button.dataset.profile || "" });
      if (action === "open-model-page") await invoke("open_model_download_page", { profile: button.dataset.profile || "" });
      if (action === "open-model-dir") await invoke("open_models_dir");
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
      }
      render();
    });
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

async function saveConfig() {
  if (!snapshot) return;
  const next = structuredClone(snapshot.config);
  app.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-config]").forEach((input) => {
    setPath(next, input.dataset.config!, input.value);
  });
  await run("save_config", { config: next });
}

async function refreshModelStatus() {
  statusRows = await invoke<AsrModelStatus[]>("asr_status");
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
