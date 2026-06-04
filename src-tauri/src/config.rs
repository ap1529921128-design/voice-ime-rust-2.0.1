use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub asr: AsrConfig,
    #[serde(default)]
    pub input: InputConfig,
    #[serde(default)]
    pub smart: SmartConfig,
    #[serde(default)]
    pub translation: TranslationConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfig {
    #[serde(default = "default_asr_engine")]
    pub default_engine: String,
    #[serde(default = "default_asr_profile")]
    pub profile: String,
    #[serde(default = "default_worker_mode")]
    pub worker_mode: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_min_record_seconds")]
    pub min_record_seconds: f32,
    #[serde(default = "default_max_record_seconds")]
    pub max_record_seconds: u32,
    #[serde(default = "default_long_transcript_seconds")]
    pub long_transcript_seconds: u32,
    #[serde(default = "default_long_chunk_seconds")]
    pub long_transcript_chunk_seconds: u32,
    #[serde(default = "default_num_threads")]
    pub num_threads: i32,
    #[serde(default)]
    pub models: AsrModels,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrModels {
    #[serde(default = "default_sense_voice_model")]
    pub sense_voice_model: String,
    #[serde(default = "default_sense_voice_tokens")]
    pub sense_voice_tokens: String,
    #[serde(default = "default_zipformer_model")]
    pub zipformer_ctc_model: String,
    #[serde(default = "default_zipformer_tokens")]
    pub zipformer_ctc_tokens: String,
    #[serde(default = "default_whisper_encoder")]
    pub whisper_encoder: String,
    #[serde(default = "default_whisper_decoder")]
    pub whisper_decoder: String,
    #[serde(default = "default_whisper_tokens")]
    pub whisper_tokens: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    #[serde(default = "default_input_mode")]
    pub mode: String,
    #[serde(default = "default_tsf_phase")]
    pub tsf_phase: String,
    #[serde(default = "default_paste_delay_ms")]
    pub paste_delay_ms: u64,
    #[serde(default = "default_hotkey_record")]
    pub hotkey_record: String,
    #[serde(default = "default_hotkey_language")]
    pub hotkey_language: String,
    #[serde(default = "default_hotkey_english")]
    pub hotkey_english: String,
    #[serde(default = "default_hotkey_japanese")]
    pub hotkey_japanese: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_llm_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
    #[serde(default = "default_smart_timeout")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationConfig {
    #[serde(default = "default_llm_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
    #[serde(default = "default_translation_timeout")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_true")]
    pub glass_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub root_dir: PathBuf,
    pub app_dir: PathBuf,
    pub config_path: PathBuf,
    pub history_path: PathBuf,
    pub prompt_path: PathBuf,
    pub corrections_path: PathBuf,
    pub hotwords_path: PathBuf,
    pub hot_rules_path: PathBuf,
    pub recordings_dir: PathBuf,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            asr: AsrConfig::default(),
            input: InputConfig::default(),
            smart: SmartConfig::default(),
            translation: TranslationConfig::default(),
            ui: UiConfig::default(),
            history_limit: default_history_limit(),
        }
    }
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            default_engine: default_asr_engine(),
            profile: default_asr_profile(),
            worker_mode: default_worker_mode(),
            language: default_language(),
            sample_rate: default_sample_rate(),
            min_record_seconds: default_min_record_seconds(),
            max_record_seconds: default_max_record_seconds(),
            long_transcript_seconds: default_long_transcript_seconds(),
            long_transcript_chunk_seconds: default_long_chunk_seconds(),
            num_threads: default_num_threads(),
            models: AsrModels::default(),
        }
    }
}

impl Default for AsrModels {
    fn default() -> Self {
        Self {
            sense_voice_model: default_sense_voice_model(),
            sense_voice_tokens: default_sense_voice_tokens(),
            zipformer_ctc_model: default_zipformer_model(),
            zipformer_ctc_tokens: default_zipformer_tokens(),
            whisper_encoder: default_whisper_encoder(),
            whisper_decoder: default_whisper_decoder(),
            whisper_tokens: default_whisper_tokens(),
        }
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            mode: default_input_mode(),
            tsf_phase: default_tsf_phase(),
            paste_delay_ms: default_paste_delay_ms(),
            hotkey_record: default_hotkey_record(),
            hotkey_language: default_hotkey_language(),
            hotkey_english: default_hotkey_english(),
            hotkey_japanese: default_hotkey_japanese(),
        }
    }
}

impl Default for SmartConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: default_llm_endpoint(),
            model: default_llm_model(),
            timeout_seconds: default_smart_timeout(),
        }
    }
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            endpoint: default_llm_endpoint(),
            model: default_llm_model(),
            timeout_seconds: default_translation_timeout(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            accent: default_accent(),
            glass_enabled: true,
        }
    }
}

impl Paths {
    pub fn discover() -> Result<Self> {
        let root_dir = discover_root_dir();
        let app_dir = std::env::var_os("VOICE_IME_APP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root_dir.join(".voice_ime"));
        Ok(Self {
            config_path: app_dir.join("config.json"),
            history_path: app_dir.join("history.json"),
            prompt_path: app_dir.join("personal_prompt.txt"),
            corrections_path: app_dir.join("corrections.json"),
            hotwords_path: app_dir.join("hot.txt"),
            hot_rules_path: app_dir.join("hot-rule.txt"),
            recordings_dir: app_dir.join("recordings"),
            root_dir,
            app_dir,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        fs::create_dir_all(&self.app_dir)?;
        fs::create_dir_all(&self.recordings_dir)?;
        Ok(())
    }
}

pub fn load_or_create(paths: &Paths) -> Result<AppConfig> {
    paths.ensure()?;
    ensure_text_files(paths)?;
    let mut config = AppConfig::default();
    if paths.config_path.exists() {
        let raw = fs::read_to_string(&paths.config_path).context("read config.json")?;
        let value: Value = serde_json::from_str(&raw).context("parse config.json")?;
        if value.get("asr").is_some() || value.get("input").is_some() {
            config = serde_json::from_value(value).context("load 2.0 config")?;
        } else {
            config = migrate_legacy_config(value);
        }
    }
    normalize_config(&mut config);
    save_config(paths, &config)?;
    Ok(config)
}

pub fn save_config(paths: &Paths, config: &AppConfig) -> Result<()> {
    paths.ensure()?;
    let body = serde_json::to_string_pretty(config)?;
    fs::write(&paths.config_path, body)?;
    Ok(())
}

fn migrate_legacy_config(value: Value) -> AppConfig {
    let mut config = AppConfig::default();
    let get_str = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
    let get_u64 = |key: &str| value.get(key).and_then(Value::as_u64);
    let get_bool = |key: &str| value.get(key).and_then(Value::as_bool);

    if let Some(language) = get_str("language") {
        config.asr.language = language;
    }
    if let Some(max) = get_u64("max_record_seconds") {
        config.asr.max_record_seconds = max as u32;
    }
    if let Some(long) = get_u64("long_transcript_seconds") {
        config.asr.long_transcript_seconds = long as u32;
    }
    if let Some(chunk) = get_u64("long_transcript_chunk_seconds") {
        config.asr.long_transcript_chunk_seconds = chunk as u32;
    }
    if let Some(delay) = get_u64("paste_delay_ms") {
        config.input.paste_delay_ms = delay;
    }
    if let Some(limit) = get_u64("history_limit") {
        config.history_limit = limit as usize;
    }
    if let Some(enabled) = get_bool("smart_correction_enabled") {
        config.smart.enabled = enabled;
    }
    if let Some(endpoint) = get_str("smart_correction_endpoint") {
        config.smart.endpoint = endpoint;
    }
    if let Some(model) = get_str("smart_correction_model") {
        config.smart.model = model;
    }
    if let Some(timeout) = get_u64("smart_correction_timeout") {
        config.smart.timeout_seconds = timeout.max(5);
    }
    if let Some(endpoint) = get_str("translation_endpoint") {
        config.translation.endpoint = endpoint;
    }
    if let Some(model) = get_str("translation_model") {
        config.translation.model = model;
    }
    if let Some(timeout) = get_u64("translation_timeout") {
        config.translation.timeout_seconds = timeout;
    }

    config
}

fn normalize_config(config: &mut AppConfig) {
    config.asr.num_threads = config.asr.num_threads.clamp(1, 4);
    if !matches!(config.asr.worker_mode.as_str(), "persistent" | "isolated") {
        config.asr.worker_mode = default_worker_mode();
    }
    config.translation.timeout_seconds = config.translation.timeout_seconds.clamp(3, 8);
    config.input.hotkey_record = normalize_hotkey(&config.input.hotkey_record);
    config.input.hotkey_english = normalize_hotkey(&config.input.hotkey_english);
    config.input.hotkey_japanese = normalize_hotkey(&config.input.hotkey_japanese);
    normalize_model_paths(config);
}

fn normalize_hotkey(value: &str) -> String {
    match value.trim() {
        "AltRight" => "Alt+R".into(),
        "Alt+KeyE" => "Alt+E".into(),
        "Alt+KeyJ" => "Alt+J".into(),
        other => other.to_string(),
    }
}

fn normalize_model_paths(config: &mut AppConfig) {
    if config.asr.models.sense_voice_model
        == "models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17-int8/model.int8.onnx"
    {
        config.asr.models.sense_voice_model = default_sense_voice_model();
    }
    if config.asr.models.sense_voice_tokens
        == "models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17-int8/tokens.txt"
    {
        config.asr.models.sense_voice_tokens = default_sense_voice_tokens();
    }
    if config.asr.models.whisper_encoder == "models/sherpa-onnx-whisper-tiny/encoder.int8.onnx" {
        config.asr.models.whisper_encoder = default_whisper_encoder();
    }
    if config.asr.models.whisper_decoder == "models/sherpa-onnx-whisper-tiny/decoder.int8.onnx" {
        config.asr.models.whisper_decoder = default_whisper_decoder();
    }
    if config.asr.models.whisper_tokens == "models/sherpa-onnx-whisper-tiny/tokens.txt" {
        config.asr.models.whisper_tokens = default_whisper_tokens();
    }
}

fn ensure_text_files(paths: &Paths) -> Result<()> {
    if !paths.prompt_path.exists() {
        fs::write(&paths.prompt_path, DEFAULT_PERSONAL_PROMPT)?;
    }
    if !paths.corrections_path.exists() {
        fs::write(
            &paths.corrections_path,
            serde_json::to_string_pretty(&crate::text::default_corrections())?,
        )?;
    }
    if !paths.hotwords_path.exists() {
        fs::write(&paths.hotwords_path, DEFAULT_HOTWORDS)?;
    }
    if !paths.hot_rules_path.exists() {
        fs::write(&paths.hot_rules_path, DEFAULT_HOT_RULES)?;
    }
    Ok(())
}

fn discover_root_dir() -> PathBuf {
    if let Some(root) = std::env::var_os("VOICE_IME_ROOT") {
        return PathBuf::from(root);
    }
    if let Ok(exe) = std::env::current_exe() {
        for candidate in exe.ancestors() {
            if candidate.join("models").exists()
                || candidate.join(".voice_ime").exists()
                || candidate.join("启动语音输入.bat").exists()
            {
                return candidate.to_path_buf();
            }
        }
        if let Some(parent) = exe.parent() {
            return parent.to_path_buf();
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

const DEFAULT_PERSONAL_PROMPT: &str = "请优先识别为简体中文，保留常见英文工具名和技术名词。\n常用词：Codex, Claude Code, ChatGPT, OpenAI, GitHub, Python, PowerShell, Windows, macOS, ASR, GUI, MVP, PRD, faster-whisper, FunASR, SenseVoice, sherpa-onnx, whisper.cpp, llama-server, MiniCPM, Rust, Tauri。\n常用表达：不要自动发送，放到输入框等我确认；帮我整理需求；帮我判断有没有搞头；问问老金；先做最小验证；移动硬盘环境；最小化到托盘。\n";
const DEFAULT_HOTWORDS: &str = "# hot.txt\n# One entry per line. Use the first item as output and aliases after |.\n# Example:\n# Voice IME | voice ime | 语音输入法\n# GitHub | git hub | 机特哈布\n";
const DEFAULT_HOT_RULES: &str = "# hot-rule.txt\n# One rule per line: regex = replacement\n# Example:\n# 毫安时 = mAh\n# 艾特\\s*(\\w+)\\s*点\\s*(\\w+) = @\\1.\\2\n";

fn default_asr_engine() -> String {
    "sherpa-onnx".into()
}
fn default_asr_profile() -> String {
    "balanced".into()
}
fn default_worker_mode() -> String {
    "persistent".into()
}
fn default_language() -> String {
    "zh".into()
}
fn default_sample_rate() -> u32 {
    16_000
}
fn default_min_record_seconds() -> f32 {
    0.5
}
fn default_max_record_seconds() -> u32 {
    120
}
fn default_long_transcript_seconds() -> u32 {
    30
}
fn default_long_chunk_seconds() -> u32 {
    10
}
fn default_num_threads() -> i32 {
    2
}
fn default_history_limit() -> usize {
    50
}
fn default_input_mode() -> String {
    "floating-overlay".into()
}
fn default_tsf_phase() -> String {
    "prepared".into()
}
fn default_paste_delay_ms() -> u64 {
    0
}
fn default_hotkey_record() -> String {
    "Alt+R".into()
}
fn default_hotkey_language() -> String {
    "Alt+Space".into()
}
fn default_hotkey_english() -> String {
    "Alt+E".into()
}
fn default_hotkey_japanese() -> String {
    "Alt+J".into()
}
fn default_true() -> bool {
    true
}
fn default_llm_endpoint() -> String {
    "http://127.0.0.1:18080/v1/chat/completions".into()
}
fn default_llm_model() -> String {
    "minicpm5-1b-q4".into()
}
fn default_smart_timeout() -> u64 {
    10
}
fn default_translation_timeout() -> u64 {
    8
}
fn default_theme() -> String {
    "indigo-porcelain-glass".into()
}
fn default_accent() -> String {
    "#315d93".into()
}
fn default_sense_voice_model() -> String {
    "models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/model.int8.onnx".into()
}
fn default_sense_voice_tokens() -> String {
    "models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/tokens.txt".into()
}
fn default_zipformer_model() -> String {
    "models/sherpa-onnx-zipformer-ctc-zh-int8-2025-07-03/model.int8.onnx".into()
}
fn default_zipformer_tokens() -> String {
    "models/sherpa-onnx-zipformer-ctc-zh-int8-2025-07-03/tokens.txt".into()
}
fn default_whisper_encoder() -> String {
    "models/sherpa-onnx-whisper-tiny/tiny-encoder.int8.onnx".into()
}
fn default_whisper_decoder() -> String {
    "models/sherpa-onnx-whisper-tiny/tiny-decoder.int8.onnx".into()
}
fn default_whisper_tokens() -> String {
    "models/sherpa-onnx-whisper-tiny/tiny-tokens.txt".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_fields() {
        let value = serde_json::json!({
            "language": "ja",
            "max_record_seconds": 60,
            "smart_correction_enabled": false,
            "translation_model": "local"
        });
        let cfg = migrate_legacy_config(value);
        assert_eq!(cfg.asr.language, "ja");
        assert_eq!(cfg.asr.max_record_seconds, 60);
        assert!(!cfg.smart.enabled);
        assert_eq!(cfg.translation.model, "local");
        assert_eq!(cfg.input.mode, "floating-overlay");
    }

    #[test]
    fn normalizes_legacy_hotkeys() {
        let mut cfg = AppConfig::default();
        cfg.input.hotkey_record = "AltRight".into();
        cfg.input.hotkey_english = "Alt+KeyE".into();
        cfg.input.hotkey_japanese = "Alt+KeyJ".into();
        normalize_config(&mut cfg);
        assert_eq!(cfg.input.hotkey_record, "Alt+R");
        assert_eq!(cfg.input.hotkey_english, "Alt+E");
        assert_eq!(cfg.input.hotkey_japanese, "Alt+J");
    }

    #[test]
    fn defaults_to_persistent_asr_worker() {
        let mut cfg = AppConfig::default();
        assert_eq!(cfg.asr.worker_mode, "persistent");

        cfg.asr.worker_mode = "unknown".into();
        normalize_config(&mut cfg);
        assert_eq!(cfg.asr.worker_mode, "persistent");

        cfg.asr.worker_mode = "isolated".into();
        normalize_config(&mut cfg);
        assert_eq!(cfg.asr.worker_mode, "isolated");
    }

    #[test]
    fn normalizes_legacy_model_paths() {
        let mut cfg = AppConfig::default();
        cfg.asr.models.sense_voice_model =
            "models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17-int8/model.int8.onnx".into();
        cfg.asr.models.sense_voice_tokens =
            "models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17-int8/tokens.txt".into();
        cfg.asr.models.whisper_encoder = "models/sherpa-onnx-whisper-tiny/encoder.int8.onnx".into();
        cfg.asr.models.whisper_decoder = "models/sherpa-onnx-whisper-tiny/decoder.int8.onnx".into();
        cfg.asr.models.whisper_tokens = "models/sherpa-onnx-whisper-tiny/tokens.txt".into();
        normalize_config(&mut cfg);
        assert_eq!(
            cfg.asr.models.sense_voice_model,
            "models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/model.int8.onnx"
        );
        assert_eq!(
            cfg.asr.models.whisper_encoder,
            "models/sherpa-onnx-whisper-tiny/tiny-encoder.int8.onnx"
        );
        assert_eq!(
            cfg.asr.models.whisper_tokens,
            "models/sherpa-onnx-whisper-tiny/tiny-tokens.txt"
        );
    }

    #[test]
    fn clamps_translation_timeout_for_responsive_ui() {
        let mut cfg = AppConfig::default();
        cfg.translation.timeout_seconds = 30;
        normalize_config(&mut cfg);
        assert_eq!(cfg.translation.timeout_seconds, 8);

        cfg.translation.timeout_seconds = 1;
        normalize_config(&mut cfg);
        assert_eq!(cfg.translation.timeout_seconds, 3);
    }
}
