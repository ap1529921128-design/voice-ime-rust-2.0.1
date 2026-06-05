use anyhow::{anyhow, Context, Result};
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
    #[serde(default = "default_model_root")]
    pub model_root: String,
    #[serde(default = "default_worker_mode")]
    pub worker_mode: String,
    #[serde(default = "default_accurate_external_command")]
    pub accurate_external_command: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_input_device_name")]
    pub input_device_name: String,
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
    #[serde(default = "default_save_long_recordings")]
    pub save_long_recordings: bool,
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
    #[serde(default = "default_true")]
    pub hide_overlay_after_confirm: bool,
    #[serde(default = "default_confirm_hide_delay_ms")]
    pub confirm_hide_delay_ms: u64,
    #[serde(default = "default_hotkey_record")]
    pub hotkey_record: String,
    #[serde(default = "default_hotkey_language")]
    pub hotkey_language: String,
    #[serde(default = "default_hotkey_english")]
    pub hotkey_english: String,
    #[serde(default = "default_hotkey_japanese")]
    pub hotkey_japanese: String,
    #[serde(default = "default_ptt_enabled")]
    pub ptt_enabled: bool,
    #[serde(default = "default_ptt_key")]
    pub ptt_key: String,
    #[serde(default = "default_ptt_mouse_button")]
    pub ptt_mouse_button: String,
    #[serde(default = "default_ptt_suppress")]
    pub ptt_suppress: bool,
    #[serde(default = "default_ptt_hold_threshold_ms")]
    pub ptt_hold_threshold_ms: u64,
    #[serde(default = "default_app_profiles")]
    pub app_profiles: Vec<AppInputProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInputProfile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub process_name: String,
    #[serde(default)]
    pub class_name: String,
    #[serde(default)]
    pub title_contains: String,
    #[serde(default = "default_profile_output_mode")]
    pub output_mode: String,
    #[serde(default)]
    pub paste_delay_ms: Option<u64>,
    #[serde(default = "default_profile_punctuation")]
    pub punctuation: String,
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
    #[serde(default = "default_translation_engine")]
    pub engine: String,
    #[serde(default = "default_translation_profile")]
    pub profile: String,
    #[serde(default = "default_llm_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
    #[serde(default = "default_translation_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_external_translation_command")]
    pub external_command: String,
    #[serde(default)]
    pub models: TranslationModels,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationModels {
    #[serde(default = "default_external_translation_command")]
    pub fast_command: String,
    #[serde(default = "default_external_translation_command")]
    pub balanced_command: String,
    #[serde(default = "default_external_translation_command")]
    pub accurate_command: String,
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
    pub model_dir: PathBuf,
    pub config_path: PathBuf,
    pub history_path: PathBuf,
    pub prompt_path: PathBuf,
    pub corrections_path: PathBuf,
    pub hotwords_path: PathBuf,
    pub hot_rules_path: PathBuf,
    pub recordings_dir: PathBuf,
    pub logs_dir: PathBuf,
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
            model_root: default_model_root(),
            worker_mode: default_worker_mode(),
            accurate_external_command: default_accurate_external_command(),
            language: default_language(),
            input_device_name: default_input_device_name(),
            sample_rate: default_sample_rate(),
            min_record_seconds: default_min_record_seconds(),
            max_record_seconds: default_max_record_seconds(),
            long_transcript_seconds: default_long_transcript_seconds(),
            long_transcript_chunk_seconds: default_long_chunk_seconds(),
            save_long_recordings: default_save_long_recordings(),
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
            hide_overlay_after_confirm: default_true(),
            confirm_hide_delay_ms: default_confirm_hide_delay_ms(),
            hotkey_record: default_hotkey_record(),
            hotkey_language: default_hotkey_language(),
            hotkey_english: default_hotkey_english(),
            hotkey_japanese: default_hotkey_japanese(),
            ptt_enabled: default_ptt_enabled(),
            ptt_key: default_ptt_key(),
            ptt_mouse_button: default_ptt_mouse_button(),
            ptt_suppress: default_ptt_suppress(),
            ptt_hold_threshold_ms: default_ptt_hold_threshold_ms(),
            app_profiles: default_app_profiles(),
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
            engine: default_translation_engine(),
            profile: default_translation_profile(),
            endpoint: default_llm_endpoint(),
            model: default_llm_model(),
            timeout_seconds: default_translation_timeout(),
            external_command: default_external_translation_command(),
            models: TranslationModels::default(),
        }
    }
}

impl Default for TranslationModels {
    fn default() -> Self {
        Self {
            fast_command: default_external_translation_command(),
            balanced_command: default_external_translation_command(),
            accurate_command: default_external_translation_command(),
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
        let model_dir = discover_model_dir(&root_dir);
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
            logs_dir: app_dir.join("logs"),
            model_dir,
            root_dir,
            app_dir,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        fs::create_dir_all(&self.app_dir)?;
        fs::create_dir_all(&self.recordings_dir)?;
        fs::create_dir_all(&self.logs_dir)?;
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
        if is_structured_2x_config(&value) {
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
    let config = normalized(config.clone());
    let body = serde_json::to_string_pretty(&config)?;
    fs::write(&paths.config_path, body)?;
    Ok(())
}

pub fn read_personal_prompt(paths: &Paths) -> Result<String> {
    paths.ensure()?;
    if !paths.prompt_path.exists() {
        fs::write(&paths.prompt_path, DEFAULT_PERSONAL_PROMPT)?;
    }
    fs::read_to_string(&paths.prompt_path).context("read personal_prompt.txt")
}

pub fn save_personal_prompt(paths: &Paths, prompt: &str) -> Result<String> {
    paths.ensure()?;
    let prompt = normalize_personal_prompt(prompt)?;
    fs::write(&paths.prompt_path, &prompt).context("write personal_prompt.txt")?;
    Ok(prompt)
}

pub fn reset_personal_prompt(paths: &Paths) -> Result<String> {
    save_personal_prompt(paths, DEFAULT_PERSONAL_PROMPT)
}

pub fn normalize_personal_prompt(prompt: &str) -> Result<String> {
    let mut normalized = prompt.replace("\r\n", "\n").replace('\r', "\n");
    while normalized.contains("\n\n\n") {
        normalized = normalized.replace("\n\n\n", "\n\n");
    }
    normalized = normalized.trim().to_string();
    let char_count = normalized.chars().count();
    if char_count == 0 {
        return Err(anyhow!("个人提示词不能为空"));
    }
    if char_count > 8_000 {
        return Err(anyhow!("个人提示词过长：{} / 8000 字", char_count));
    }
    normalized.push('\n');
    Ok(normalized)
}

pub fn normalized(mut config: AppConfig) -> AppConfig {
    normalize_config(&mut config);
    config
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
    if let Some(save_long) = get_bool("save_long_recordings") {
        config.asr.save_long_recordings = save_long;
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
    if let Some(engine) = get_str("translation_engine") {
        config.translation.engine = engine;
    }
    if let Some(profile) = get_str("translation_profile") {
        config.translation.profile = profile;
    }
    if let Some(model) = get_str("translation_model") {
        config.translation.model = model;
    }
    if let Some(timeout) = get_u64("translation_timeout") {
        config.translation.timeout_seconds = timeout;
    }
    if let Some(command) = get_str("translation_external_command") {
        config.translation.external_command = command;
    }

    config
}

fn is_structured_2x_config(value: &Value) -> bool {
    ["asr", "input", "smart", "translation", "ui"]
        .iter()
        .any(|key| value.get(key).is_some())
}

fn normalize_config(config: &mut AppConfig) {
    config.asr.default_engine = normalize_asr_engine(&config.asr.default_engine);
    config.asr.model_root = normalize_model_root(&config.asr.model_root);
    config.asr.num_threads = config.asr.num_threads.clamp(1, 4);
    config.asr.min_record_seconds = config.asr.min_record_seconds.clamp(0.1, 10.0);
    config.asr.max_record_seconds = config.asr.max_record_seconds.clamp(5, 600);
    if !matches!(config.asr.worker_mode.as_str(), "persistent" | "isolated") {
        config.asr.worker_mode = default_worker_mode();
    }
    config.asr.accurate_external_command = config.asr.accurate_external_command.trim().to_string();
    config.asr.input_device_name = normalize_input_device_name(&config.asr.input_device_name);
    config.translation.engine = normalize_translation_engine(&config.translation.engine);
    config.translation.profile = normalize_translation_profile(&config.translation.profile);
    config.translation.timeout_seconds = config.translation.timeout_seconds.clamp(3, 8);
    config.translation.external_command = config.translation.external_command.trim().to_string();
    config.translation.models.fast_command =
        config.translation.models.fast_command.trim().to_string();
    config.translation.models.balanced_command = config
        .translation
        .models
        .balanced_command
        .trim()
        .to_string();
    config.translation.models.accurate_command = config
        .translation
        .models
        .accurate_command
        .trim()
        .to_string();
    config.input.paste_delay_ms = config.input.paste_delay_ms.min(500);
    config.input.confirm_hide_delay_ms = config.input.confirm_hide_delay_ms.min(5_000);
    config.input.hotkey_record = normalize_hotkey(&config.input.hotkey_record);
    config.input.hotkey_english = normalize_hotkey(&config.input.hotkey_english);
    config.input.hotkey_japanese = normalize_hotkey(&config.input.hotkey_japanese);
    config.input.ptt_key = normalize_ptt_key(&config.input.ptt_key);
    config.input.ptt_mouse_button = normalize_ptt_mouse_button(&config.input.ptt_mouse_button);
    config.input.ptt_hold_threshold_ms = config.input.ptt_hold_threshold_ms.min(1000);
    if config.input.app_profiles.is_empty() {
        config.input.app_profiles = default_app_profiles();
    }
    normalize_app_profiles(&mut config.input.app_profiles);
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

fn normalize_ptt_key(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "off" | "none" | "disabled" | "关闭" => "off".into(),
        "capslock" | "caps_lock" | "capital" | "caps" | "大小写" => default_ptt_key(),
        "f8" => "F8".into(),
        "f9" => "F9".into(),
        "f10" => "F10".into(),
        "f13" => "F13".into(),
        _ => default_ptt_key(),
    }
}

fn normalize_ptt_mouse_button(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "off" | "none" | "disabled" | "关闭" => "off".into(),
        "x1" | "xbutton1" | "mouse4" | "back" => "X1".into(),
        "x2" | "xbutton2" | "mouse5" | "forward" => "X2".into(),
        _ => default_ptt_mouse_button(),
    }
}

fn normalize_translation_engine(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "llm" | "external" | "nllb" | "bergamot" => value.trim().to_ascii_lowercase(),
        _ => default_translation_engine(),
    }
}

fn normalize_translation_profile(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "fast" | "balanced" | "accurate" | "custom" => value.trim().to_ascii_lowercase(),
        "external" | "" => default_translation_profile(),
        _ => default_translation_profile(),
    }
}

fn normalize_asr_engine(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "sherpa-onnx" | "sherpa" | "" => default_asr_engine(),
        "mock" | "fake" | "test" => "mock".into(),
        _ => default_asr_engine(),
    }
}

fn normalize_model_root(value: &str) -> String {
    let value = value.trim();
    if value.is_empty()
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "default" | "system-default" | "auto" | "app/models" | "app\\models"
        )
        || matches!(value, "默认" | "自动")
    {
        default_model_root()
    } else {
        value.to_string()
    }
}

fn normalize_input_device_name(value: &str) -> String {
    let value = value.trim();
    if value.is_empty()
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "default" | "system-default" | "auto"
        )
        || matches!(value, "默认" | "系统默认" | "自动")
    {
        String::new()
    } else {
        value.to_string()
    }
}

fn normalize_app_profiles(profiles: &mut [AppInputProfile]) {
    for profile in profiles {
        profile.output_mode = match profile.output_mode.trim().to_ascii_lowercase().as_str() {
            "paste" | "clipboard-paste" => "paste".into(),
            _ => default_profile_output_mode(),
        };
        profile.punctuation = match profile.punctuation.trim().to_ascii_lowercase().as_str() {
            "default" | "short-no-period" | "keep" => profile.punctuation.to_ascii_lowercase(),
            _ => default_profile_punctuation(),
        };
        if let Some(delay) = profile.paste_delay_ms.as_mut() {
            *delay = (*delay).min(500);
        }
    }
}

pub fn matching_app_profile<'a>(
    profiles: &'a [AppInputProfile],
    process_name: &str,
    class_name: &str,
    title: &str,
) -> Option<&'a AppInputProfile> {
    let process_name = process_name.to_ascii_lowercase();
    let class_name = class_name.to_ascii_lowercase();
    let title = title.to_ascii_lowercase();
    profiles.iter().find(|profile| {
        field_matches(&profile.process_name, &process_name, MatchMode::Equals)
            && field_matches(&profile.class_name, &class_name, MatchMode::Equals)
            && field_matches(&profile.title_contains, &title, MatchMode::Contains)
    })
}

enum MatchMode {
    Equals,
    Contains,
}

fn field_matches(pattern: &str, value: &str, mode: MatchMode) -> bool {
    let pattern = pattern.trim().to_ascii_lowercase();
    if pattern.is_empty() || pattern == "*" {
        return true;
    }
    match mode {
        MatchMode::Equals => pattern == value,
        MatchMode::Contains => value.contains(&pattern),
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

fn discover_model_dir(root_dir: &Path) -> PathBuf {
    root_dir.join("models")
}

pub fn effective_model_root(config: &AppConfig, paths: &Paths) -> PathBuf {
    if let Some(env_root) = std::env::var_os("VOICE_IME_MODEL_DIR") {
        return absolutize(&paths.root_dir, PathBuf::from(env_root));
    }
    if let Some(file_root) = model_root_file(&paths.root_dir) {
        return file_root;
    }
    let configured = config.asr.model_root.trim();
    if configured.is_empty() || configured == "models" {
        return paths.model_dir.clone();
    }
    absolutize(&paths.root_dir, PathBuf::from(configured))
}

pub fn effective_model_root_source(config: &AppConfig, paths: &Paths) -> &'static str {
    if std::env::var_os("VOICE_IME_MODEL_DIR").is_some() {
        return "VOICE_IME_MODEL_DIR";
    }
    if model_root_file(&paths.root_dir).is_some() {
        return "MODEL_ROOT.txt";
    }
    let configured = config.asr.model_root.trim();
    if configured.is_empty() || configured == "models" {
        "default"
    } else {
        "asr.model_root"
    }
}

pub fn model_root_override_path(paths: &Paths) -> PathBuf {
    paths.root_dir.join("MODEL_ROOT.txt")
}

pub fn model_root_override_value(paths: &Paths) -> Option<String> {
    model_root_file_value(&paths.root_dir)
}

pub fn resolve_model_root_value(paths: &Paths, value: &str) -> Result<PathBuf> {
    let value = normalize_model_root(value);
    if value.trim().is_empty() {
        return Err(anyhow!("模型根目录不能为空"));
    }
    Ok(absolutize(&paths.root_dir, PathBuf::from(value)))
}

pub fn write_model_root_override(paths: &Paths, model_root: &str) -> Result<PathBuf> {
    let target = resolve_model_root_value(paths, model_root)?;
    fs::create_dir_all(&target)
        .with_context(|| format!("创建模型根目录 {}", target.to_string_lossy()))?;
    let body = format!(
        "# Voice IME portable model root\n# This file is read before .voice_ime/config.json.\n{}\n",
        target.to_string_lossy()
    );
    fs::write(model_root_override_path(paths), body).context("write MODEL_ROOT.txt")?;
    Ok(target)
}

pub fn clear_model_root_override(paths: &Paths) -> Result<bool> {
    let path = model_root_override_path(paths);
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(&path).with_context(|| format!("remove {}", path.to_string_lossy()))?;
    Ok(true)
}

pub fn resolve_model_path(config: &AppConfig, paths: &Paths, configured: &str) -> PathBuf {
    let configured = configured.trim();
    let path = PathBuf::from(configured);
    if path.is_absolute() {
        return path;
    }
    let normalized = configured.replace('\\', "/");
    let relative = normalized.strip_prefix("models/").unwrap_or(&normalized);
    if relative.is_empty() {
        effective_model_root(config, paths)
    } else {
        effective_model_root(config, paths).join(relative)
    }
}

fn absolutize(root_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root_dir.join(path)
    }
}

fn model_root_file(root_dir: &Path) -> Option<PathBuf> {
    model_root_file_value(root_dir).map(|value| absolutize(root_dir, PathBuf::from(value)))
}

fn model_root_file_value(root_dir: &Path) -> Option<String> {
    let text = fs::read_to_string(root_dir.join("MODEL_ROOT.txt")).ok()?;
    text.lines()
        .map(|line| line.trim().trim_start_matches('\u{feff}').trim())
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
}

pub(crate) const DEFAULT_PERSONAL_PROMPT: &str = "请优先识别为简体中文，保留常见英文工具名和技术名词。\n常用词：Codex, Claude Code, ChatGPT, OpenAI, GitHub, Python, PowerShell, Windows, macOS, ASR, GUI, MVP, PRD, faster-whisper, FunASR, SenseVoice, sherpa-onnx, whisper.cpp, llama-server, MiniCPM, Rust, Tauri。\n常用表达：不要自动发送，放到输入框等我确认；帮我整理需求；帮我判断有没有搞头；问问老金；先做最小验证；移动硬盘环境；最小化到托盘。\n";
pub(crate) const DEFAULT_HOTWORDS: &str = "# hot.txt\n# One entry per line. Use the first item as output and aliases after |.\n# Example:\n# Voice IME | voice ime | 语音输入法\n# GitHub | git hub | 机特哈布\n";
pub(crate) const DEFAULT_HOT_RULES: &str = "# hot-rule.txt\n# One rule per line: regex = replacement\n# Example:\n# 毫安时 = mAh\n# 艾特\\s*(\\w+)\\s*点\\s*(\\w+) = @\\1.\\2\n";

fn default_asr_engine() -> String {
    "sherpa-onnx".into()
}
fn default_asr_profile() -> String {
    "balanced".into()
}
fn default_model_root() -> String {
    "models".into()
}
fn default_worker_mode() -> String {
    "persistent".into()
}
fn default_accurate_external_command() -> String {
    String::new()
}
fn default_language() -> String {
    "zh".into()
}
fn default_input_device_name() -> String {
    String::new()
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
fn default_save_long_recordings() -> bool {
    true
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
fn default_confirm_hide_delay_ms() -> u64 {
    650
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
fn default_ptt_enabled() -> bool {
    true
}
fn default_ptt_key() -> String {
    "CapsLock".into()
}
fn default_ptt_mouse_button() -> String {
    "X2".into()
}
fn default_ptt_suppress() -> bool {
    true
}
fn default_ptt_hold_threshold_ms() -> u64 {
    180
}
fn default_app_profiles() -> Vec<AppInputProfile> {
    vec![
        app_profile("微信", "WeChat.exe", 80, "short-no-period"),
        app_profile("飞书", "Feishu.exe", 80, "short-no-period"),
        app_profile("Lark", "Lark.exe", 80, "short-no-period"),
        app_profile("Word", "WINWORD.EXE", 120, "keep"),
        app_profile("Chrome", "chrome.exe", 20, "default"),
        app_profile("Edge", "msedge.exe", 20, "default"),
        app_profile("VS Code", "Code.exe", 30, "default"),
        app_profile("JetBrains", "idea64.exe", 50, "default"),
    ]
}
fn app_profile(
    name: &str,
    process_name: &str,
    paste_delay_ms: u64,
    punctuation: &str,
) -> AppInputProfile {
    AppInputProfile {
        name: name.into(),
        process_name: process_name.into(),
        class_name: String::new(),
        title_contains: String::new(),
        output_mode: default_profile_output_mode(),
        paste_delay_ms: Some(paste_delay_ms),
        punctuation: punctuation.into(),
    }
}
fn default_profile_output_mode() -> String {
    "paste".into()
}
fn default_profile_punctuation() -> String {
    "default".into()
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
fn default_translation_engine() -> String {
    "llm".into()
}
fn default_translation_profile() -> String {
    "balanced".into()
}
fn default_external_translation_command() -> String {
    String::new()
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
    use std::path::Path;

    fn temp_paths(root: &Path) -> Paths {
        let app_dir = root.join(".voice_ime");
        Paths {
            root_dir: root.to_path_buf(),
            app_dir: app_dir.clone(),
            model_dir: root.join("models"),
            config_path: app_dir.join("config.json"),
            history_path: app_dir.join("history.json"),
            prompt_path: app_dir.join("personal_prompt.txt"),
            corrections_path: app_dir.join("corrections.json"),
            hotwords_path: app_dir.join("hot.txt"),
            hot_rules_path: app_dir.join("hot-rule.txt"),
            recordings_dir: app_dir.join("recordings"),
            logs_dir: app_dir.join("logs"),
        }
    }

    #[test]
    fn migrates_legacy_fields() {
        let value = serde_json::json!({
            "language": "ja",
            "max_record_seconds": 60,
            "smart_correction_enabled": false,
            "translation_engine": "external",
            "translation_profile": "fast",
            "translation_external_command": "translator.exe --compact",
            "translation_model": "local",
            "save_long_recordings": false
        });
        let cfg = migrate_legacy_config(value);
        assert_eq!(cfg.asr.language, "ja");
        assert_eq!(cfg.asr.max_record_seconds, 60);
        assert!(!cfg.smart.enabled);
        assert_eq!(cfg.translation.engine, "external");
        assert_eq!(cfg.translation.profile, "fast");
        assert_eq!(cfg.translation.external_command, "translator.exe --compact");
        assert_eq!(cfg.translation.model, "local");
        assert!(!cfg.asr.save_long_recordings);
        assert_eq!(cfg.input.mode, "floating-overlay");
    }

    #[test]
    fn recognizes_translation_only_structured_config() {
        let value = serde_json::json!({
            "translation": {
                "engine": "external",
                "profile": "fast",
                "timeout_seconds": 3,
                "models": {
                    "fast_command": "translator.exe --json"
                }
            }
        });

        assert!(is_structured_2x_config(&value));
        let mut cfg: AppConfig = serde_json::from_value(value).unwrap();
        normalize_config(&mut cfg);

        assert_eq!(cfg.translation.engine, "external");
        assert_eq!(cfg.translation.profile, "fast");
        assert_eq!(cfg.translation.timeout_seconds, 3);
        assert_eq!(cfg.translation.models.fast_command, "translator.exe --json");
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
    fn normalizes_asr_engine() {
        let mut cfg = AppConfig::default();
        assert_eq!(cfg.asr.default_engine, "sherpa-onnx");

        cfg.asr.default_engine = "FAKE".into();
        normalize_config(&mut cfg);
        assert_eq!(cfg.asr.default_engine, "mock");

        cfg.asr.default_engine = "unknown".into();
        normalize_config(&mut cfg);
        assert_eq!(cfg.asr.default_engine, "sherpa-onnx");
    }

    #[test]
    fn normalizes_accurate_external_command() {
        let mut cfg = AppConfig::default();
        cfg.asr.accurate_external_command = "  powershell -File asr.ps1  ".into();
        normalize_config(&mut cfg);

        assert_eq!(
            cfg.asr.accurate_external_command,
            "powershell -File asr.ps1"
        );
    }

    #[test]
    fn clamps_recording_duration_bounds() {
        let mut cfg = AppConfig::default();
        cfg.asr.min_record_seconds = 0.0;
        cfg.asr.max_record_seconds = 1;
        normalize_config(&mut cfg);
        assert_eq!(cfg.asr.min_record_seconds, 0.1);
        assert_eq!(cfg.asr.max_record_seconds, 5);

        cfg.asr.min_record_seconds = 30.0;
        cfg.asr.max_record_seconds = 1_000;
        normalize_config(&mut cfg);
        assert_eq!(cfg.asr.min_record_seconds, 10.0);
        assert_eq!(cfg.asr.max_record_seconds, 600);
    }

    #[test]
    fn resolves_relative_model_paths_against_model_root() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(temp.path());
        let mut cfg = AppConfig::default();

        assert_eq!(
            resolve_model_path(&cfg, &paths, "models/foo/bar.onnx"),
            temp.path().join("models/foo/bar.onnx")
        );

        cfg.asr.model_root = "E:/voice-ime-models".into();
        normalize_config(&mut cfg);

        assert_eq!(
            resolve_model_path(&cfg, &paths, "models/foo/bar.onnx"),
            PathBuf::from("E:/voice-ime-models/foo/bar.onnx")
        );
        assert_eq!(
            resolve_model_path(&cfg, &paths, "foo/bar.onnx"),
            PathBuf::from("E:/voice-ime-models/foo/bar.onnx")
        );
    }

    #[test]
    fn model_root_file_overrides_saved_config_for_portable_model_repo() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(temp.path());
        let shared = temp.path().join("shared-models");
        std::fs::write(
            temp.path().join("MODEL_ROOT.txt"),
            format!("# shared repo\n{}\n", shared.display()),
        )
        .unwrap();
        let mut cfg = AppConfig::default();
        cfg.asr.model_root = "E:/old-models".into();
        normalize_config(&mut cfg);

        assert_eq!(effective_model_root(&cfg, &paths), shared.clone());
        assert_eq!(effective_model_root_source(&cfg, &paths), "MODEL_ROOT.txt");
        assert_eq!(
            resolve_model_path(&cfg, &paths, "models/foo/bar.onnx"),
            shared.join("foo/bar.onnx")
        );
    }

    #[test]
    fn packaged_model_dir_stays_under_app_when_override_exists() {
        let temp = tempfile::tempdir().unwrap();
        let shared = temp.path().join("shared-models");
        std::fs::write(
            temp.path().join("MODEL_ROOT.txt"),
            format!("{}\n", shared.display()),
        )
        .unwrap();

        assert_eq!(discover_model_dir(temp.path()), temp.path().join("models"));

        let paths = temp_paths(temp.path());
        assert_eq!(paths.model_dir, temp.path().join("models"));
        assert_eq!(
            effective_model_root(&AppConfig::default(), &paths),
            shared.clone()
        );
    }

    #[test]
    fn writes_and_clears_portable_model_root_override() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(temp.path());
        let shared = temp.path().join("shared-models");

        let written = write_model_root_override(&paths, &shared.to_string_lossy()).unwrap();

        assert_eq!(written, shared);
        assert!(model_root_override_path(&paths).is_file());
        assert_eq!(
            model_root_override_value(&paths),
            Some(shared.to_string_lossy().to_string())
        );
        assert_eq!(
            effective_model_root(&AppConfig::default(), &paths),
            shared.clone()
        );
        assert_eq!(
            effective_model_root_source(&AppConfig::default(), &paths),
            "MODEL_ROOT.txt"
        );

        assert!(clear_model_root_override(&paths).unwrap());
        assert!(!model_root_override_path(&paths).exists());
        assert_eq!(
            effective_model_root(&AppConfig::default(), &paths),
            temp.path().join("models")
        );
        assert_eq!(
            effective_model_root_source(&AppConfig::default(), &paths),
            "default"
        );
        assert!(!clear_model_root_override(&paths).unwrap());
    }

    #[test]
    fn normalizes_input_device_name() {
        let mut cfg = AppConfig::default();
        cfg.asr.input_device_name = "  USB Microphone  ".into();
        normalize_config(&mut cfg);
        assert_eq!(cfg.asr.input_device_name, "USB Microphone");

        cfg.asr.input_device_name = "系统默认".into();
        normalize_config(&mut cfg);
        assert_eq!(cfg.asr.input_device_name, "");
    }

    #[test]
    fn normalizes_push_to_talk_options() {
        let mut cfg = AppConfig::default();
        assert!(cfg.input.ptt_enabled);
        assert_eq!(cfg.input.ptt_key, "CapsLock");
        assert_eq!(cfg.input.ptt_mouse_button, "X2");
        assert!(cfg.input.ptt_suppress);
        assert_eq!(cfg.input.ptt_hold_threshold_ms, 180);

        cfg.input.ptt_key = "caps".into();
        cfg.input.ptt_mouse_button = "mouse4".into();
        cfg.input.ptt_hold_threshold_ms = 2_000;
        normalize_config(&mut cfg);
        assert_eq!(cfg.input.ptt_key, "CapsLock");
        assert_eq!(cfg.input.ptt_mouse_button, "X1");
        assert_eq!(cfg.input.ptt_hold_threshold_ms, 1_000);

        cfg.input.ptt_key = "unknown".into();
        cfg.input.ptt_mouse_button = "unknown".into();
        normalize_config(&mut cfg);
        assert_eq!(cfg.input.ptt_key, "CapsLock");
        assert_eq!(cfg.input.ptt_mouse_button, "X2");
    }

    #[test]
    fn normalizes_confirm_overlay_settings() {
        let mut cfg = AppConfig::default();
        assert!(cfg.input.hide_overlay_after_confirm);
        assert_eq!(cfg.input.confirm_hide_delay_ms, 650);

        cfg.input.paste_delay_ms = 900;
        cfg.input.confirm_hide_delay_ms = 30_000;
        cfg.input.hide_overlay_after_confirm = false;
        normalize_config(&mut cfg);

        assert_eq!(cfg.input.paste_delay_ms, 500);
        assert_eq!(cfg.input.confirm_hide_delay_ms, 5_000);
        assert!(!cfg.input.hide_overlay_after_confirm);
    }

    #[test]
    fn personal_prompt_roundtrip_validates_and_resets() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(temp.path());

        assert!(normalize_personal_prompt(" \n ").is_err());
        assert!(normalize_personal_prompt(&"长".repeat(8_001)).is_err());

        let saved = save_personal_prompt(&paths, " 常用词：Voice IME\r\n\r\n\r\n命令：cargo test ")
            .unwrap();
        assert_eq!(saved, "常用词：Voice IME\n\n命令：cargo test\n");
        assert_eq!(read_personal_prompt(&paths).unwrap(), saved);

        let reset = reset_personal_prompt(&paths).unwrap();
        assert_eq!(reset, DEFAULT_PERSONAL_PROMPT);
        assert_eq!(
            read_personal_prompt(&paths).unwrap(),
            DEFAULT_PERSONAL_PROMPT
        );
    }

    #[test]
    fn matches_app_profiles_case_insensitively() {
        let mut cfg = AppConfig::default();
        normalize_config(&mut cfg);
        let profile = matching_app_profile(
            &cfg.input.app_profiles,
            "wechat.exe",
            "Chrome_WidgetWin_1",
            "聊天",
        )
        .unwrap();
        assert_eq!(profile.name, "微信");
        assert_eq!(profile.paste_delay_ms, Some(80));
    }

    #[test]
    fn normalizes_app_profile_defaults() {
        let mut cfg = AppConfig::default();
        cfg.input.app_profiles = vec![AppInputProfile {
            name: "custom".into(),
            process_name: "*".into(),
            class_name: String::new(),
            title_contains: "note".into(),
            output_mode: "type".into(),
            paste_delay_ms: Some(900),
            punctuation: "bad".into(),
        }];
        normalize_config(&mut cfg);
        let profile = &cfg.input.app_profiles[0];
        assert_eq!(profile.output_mode, "paste");
        assert_eq!(profile.paste_delay_ms, Some(500));
        assert_eq!(profile.punctuation, "default");
        assert!(matching_app_profile(&cfg.input.app_profiles, "x.exe", "", "OneNote").is_some());
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

    #[test]
    fn normalizes_translation_engine() {
        let mut cfg = AppConfig::default();
        assert_eq!(cfg.translation.engine, "llm");
        assert_eq!(cfg.translation.profile, "balanced");

        cfg.translation.engine = "EXTERNAL".into();
        cfg.translation.profile = "ACCURATE".into();
        cfg.translation.external_command = "  translator.exe --json  ".into();
        cfg.translation.models.fast_command = "  fast.exe --json  ".into();
        cfg.translation.models.balanced_command = "  balanced.exe --json  ".into();
        cfg.translation.models.accurate_command = "  accurate.exe --json  ".into();
        normalize_config(&mut cfg);
        assert_eq!(cfg.translation.engine, "external");
        assert_eq!(cfg.translation.profile, "accurate");
        assert_eq!(cfg.translation.external_command, "translator.exe --json");
        assert_eq!(cfg.translation.models.fast_command, "fast.exe --json");
        assert_eq!(
            cfg.translation.models.balanced_command,
            "balanced.exe --json"
        );
        assert_eq!(
            cfg.translation.models.accurate_command,
            "accurate.exe --json"
        );

        cfg.translation.engine = "bad".into();
        cfg.translation.profile = "bad".into();
        normalize_config(&mut cfg);
        assert_eq!(cfg.translation.engine, "llm");
        assert_eq!(cfg.translation.profile, "balanced");
    }
}
