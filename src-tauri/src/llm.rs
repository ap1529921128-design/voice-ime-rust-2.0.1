use crate::{cancel::CancellationToken, config::AppConfig, config::Paths, text};
use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

static LAST_SERVICE_START: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
pub struct LocalServiceStatus {
    pub endpoint: String,
    pub models_url: String,
    pub is_local: bool,
    pub reachable: bool,
    pub script_path: String,
    pub script_exists: bool,
    pub model_path: String,
    pub model_exists: bool,
    pub server_path: String,
    pub server_exists: bool,
}

pub fn local_service_status(endpoint: &str, paths: &Paths) -> LocalServiceStatus {
    let endpoint = endpoint.trim();
    let is_local = is_local_endpoint(endpoint);
    let models_url = if endpoint.is_empty() {
        String::new()
    } else {
        models_endpoint(endpoint)
    };
    let script = local_service_script(paths).unwrap_or_else(|| default_local_service_script(paths));
    let model = local_model_path(paths);
    let server = local_server_path(paths);
    LocalServiceStatus {
        endpoint: endpoint.to_string(),
        models_url: models_url.clone(),
        is_local,
        reachable: is_local && !models_url.is_empty() && http_ok(&models_url),
        script_exists: script.exists(),
        script_path: script.to_string_lossy().to_string(),
        model_exists: model.exists(),
        model_path: model.to_string_lossy().to_string(),
        server_exists: server.exists(),
        server_path: server.to_string_lossy().to_string(),
    }
}

pub fn start_local_service(endpoint: &str, paths: &Paths) -> Result<LocalServiceStatus> {
    if !is_local_endpoint(endpoint) {
        return Err(anyhow!("当前端点不是本地 llama-server"));
    }
    ensure_local_service(endpoint, paths)?;
    Ok(local_service_status(endpoint, paths))
}

pub fn smart_correct(
    raw_text: &str,
    base_text: &str,
    config: &AppConfig,
    paths: &Paths,
    personal_prompt: &str,
    cancellation: &CancellationToken,
) -> String {
    let corrected = text::apply_corrections(raw_text, &paths.corrections_path);
    let base_text = text::normalize_text(base_text);
    let edit_existing = !base_text.is_empty() && text::is_confirmation_edit_command(&corrected);
    if cancellation.is_cancelled() {
        return if edit_existing { base_text } else { corrected };
    }
    if corrected.trim().is_empty() {
        return if edit_existing {
            base_text
        } else {
            String::new()
        };
    }
    if edit_existing && text::looks_like_code_command_or_path(&base_text) {
        return base_text;
    }
    if !edit_existing && text::looks_like_code_command_or_path(&corrected) {
        return corrected;
    }
    if !config.smart.enabled {
        return if edit_existing { base_text } else { corrected };
    }
    if config.smart.endpoint.trim().is_empty() {
        return if edit_existing { base_text } else { corrected };
    }
    if is_local_endpoint(&config.smart.endpoint)
        && !http_ok(&models_endpoint(&config.smart.endpoint))
    {
        return if edit_existing { base_text } else { corrected };
    }

    let correction_hint = correction_hint(paths);
    let system_prompt = "你是个人语音输入法的智能输入修正器。默认只修正 ASR 听写错误、错别字、重复词、语法、标点、大小写、技术术语、文件名、命令名和单位格式，删除无意义口癖，不新增事实。若给出了当前确认栏文本和语音编辑指令，必须按该指令修改确认栏文本，不能反问用户提供内容。只返回最终正文，不解释，不加标题。";
    let user_prompt = if edit_existing {
        format!(
            "个人词表：\n{personal_prompt}\n\n已知纠错表：\n{correction_hint}\n\n任务：用户刚刚说的是编辑指令，不是要输入的新正文。请基于“当前确认栏文本”执行“语音编辑指令”，只输出修改后的确认栏文本。禁止反问、禁止说需要提供内容、禁止解释。\n\n当前确认栏文本：\n{base_text}\n\n语音编辑指令：\n{corrected}"
        )
    } else {
        format!(
            "个人词表：\n{personal_prompt}\n\n已知纠错表：\n{correction_hint}\n\n处理规则：\n1. 保留用户原意、数字、路径、代码、命令、产品名和人名。\n2. 合并重复字词，删除纯拟声和口头填充。\n3. 没有明确改写指令时，不要扩写成新内容。\n4. 有明确改写指令且正文跟在指令后面时，只输出改写后的正文。\n\nASR 文本：\n{corrected}"
        )
    };
    let reference_len = corrected.chars().count().max(base_text.chars().count());
    let payload = json!({
        "model": config.smart.model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "temperature": 0,
        "max_tokens": (reference_len * 2 + 160).clamp(128, 2048),
        "stream": false
    });
    let output = chat_completion(
        &config.smart.endpoint,
        &payload,
        config.smart.timeout_seconds,
        Some(cancellation),
    )
    .map(|value| text::clean_llm_output(&value))
    .unwrap_or_default();
    if output.is_empty() || text::looks_like_prompt_leak(&output) {
        return if edit_existing { base_text } else { corrected };
    }
    if edit_existing && text::looks_like_missing_edit_target(&output) {
        return base_text;
    }
    if output.chars().count() > (reference_len * 3).max(reference_len + 200) {
        return if edit_existing { base_text } else { corrected };
    }
    text::apply_corrections(&output, &paths.corrections_path)
}

pub fn translate_with_llm(
    source: &str,
    target_language: &str,
    config: &AppConfig,
    paths: &Paths,
    _personal_prompt: &str,
    cancellation: &CancellationToken,
) -> Result<String> {
    ensure_not_cancelled(cancellation)?;
    let mut source = text::normalize_text(source);
    if source.trim().is_empty() {
        return Err(anyhow!("没有可翻译的文本"));
    }
    if text::has_translation_markup(&source) {
        let cleaned_source = text::clean_translation_output(&source);
        if !cleaned_source.is_empty() {
            source = cleaned_source;
        }
    }
    if text::looks_like_prompt_leak(&source) {
        return Err(anyhow!("当前文本像内部提示泄漏，已拒绝翻译"));
    }
    if target_language == "zh" && text::is_likely_chinese_text(&source) {
        let cleaned = text::clean_translation_output(&source);
        if !cleaned.is_empty() {
            return Ok(cleaned);
        }
        if text::looks_like_translation_chatter(&source) {
            return Err(anyhow!("当前文本像翻译说明文字，已拒绝继续套娃"));
        }
        return Ok(source);
    }
    if config.translation.endpoint.trim().is_empty() {
        return Err(anyhow!("未配置翻译端点"));
    }
    if is_local_endpoint(&config.translation.endpoint) {
        ensure_not_cancelled(cancellation)?;
        ensure_local_service(&config.translation.endpoint, paths)?;
    }
    ensure_not_cancelled(cancellation)?;
    let language_name = match target_language {
        "en" => "英语",
        "ja" => "日语",
        "zh" => "简体中文",
        other => other,
    };
    let max_tokens = (source.chars().count() * 2 + 24).clamp(24, 256);
    let payload = json!({
        "model": config.translation.model,
        "messages": [
            { "role": "system", "content": "你是翻译引擎。只翻译用户提供的原文，保留产品名、文件名、代码、命令和数字。输出只能是译文本身；禁止标题、禁止“翻译结果”、禁止解释、禁止候选、禁止列表、禁止询问确认。" },
            { "role": "user", "content": format!("把下面原文翻译为{language_name}。只输出译文正文，不要加任何标签。\n\n{source}") }
        ],
        "temperature": 0,
        "max_tokens": max_tokens,
        "stream": false,
        "stop": ["\n原文：", "\n解释", "\n说明", "\n备注", "\nNote", "\nExplanation", "[原文]", "[/原文]", "【原文】", "【/原文】"]
    });
    let raw_translated = chat_completion(
        &config.translation.endpoint,
        &payload,
        config.translation.timeout_seconds,
        Some(cancellation),
    )?;
    ensure_not_cancelled(cancellation)?;
    let translated = text::clean_translation_output(&raw_translated);
    if translated.is_empty() {
        Err(anyhow!("翻译结果为空"))
    } else if text::looks_like_prompt_leak(&translated)
        || text::looks_like_missing_edit_target(&translated)
        || text::looks_like_translation_chatter(&translated)
    {
        Err(anyhow!("翻译模型输出了说明文字，已丢弃"))
    } else {
        Ok(translated)
    }
}

fn chat_completion(
    endpoint: &str,
    payload: &Value,
    timeout_seconds: u64,
    cancellation: Option<&CancellationToken>,
) -> Result<String> {
    if let Some(cancellation) = cancellation {
        ensure_not_cancelled(cancellation)?;
    }
    let timeout = Duration::from_secs(timeout_seconds.clamp(1, 30));
    let client = Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_millis(800))
        .build()?;
    let value: Value = client
        .post(endpoint)
        .json(payload)
        .send()?
        .error_for_status()?
        .json()?;
    if let Some(cancellation) = cancellation {
        ensure_not_cancelled(cancellation)?;
    }
    let content = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(content)
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(anyhow!("任务已取消"))
    } else {
        Ok(())
    }
}

fn ensure_local_service(endpoint: &str, paths: &Paths) -> Result<()> {
    if http_ok(&models_endpoint(endpoint)) {
        return Ok(());
    }
    spawn_local_service_once(paths)?;
    for _ in 0..8 {
        std::thread::sleep(Duration::from_millis(400));
        if http_ok(&models_endpoint(endpoint)) {
            return Ok(());
        }
    }
    Err(anyhow!(
        "本地 MiniCPM/llama-server 正在启动，请 3-5 秒后再点翻译"
    ))
}

fn spawn_local_service_once(paths: &Paths) -> Result<()> {
    let guard = LAST_SERVICE_START.get_or_init(|| Mutex::new(None));
    let mut last_start = guard.lock().map_err(|_| anyhow!("翻译服务启动锁异常"))?;
    if last_start
        .as_ref()
        .is_some_and(|started| started.elapsed() < Duration::from_secs(20))
    {
        return Ok(());
    }
    let script = local_service_script(paths).ok_or_else(|| {
        anyhow!("翻译/纠错服务未启动，且缺少启动脚本：Start-MiniCPM-Translate.ps1")
    })?;
    let mut command = Command::new("powershell.exe");
    command
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&script)
        .current_dir(&paths.root_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(0x08000000);
    }
    let _child = command.spawn()?;
    *last_start = Some(Instant::now());
    Ok(())
}

fn local_service_script(paths: &Paths) -> Option<std::path::PathBuf> {
    [
        paths.root_dir.join("Start-MiniCPM-Translate.ps1"),
        paths
            .root_dir
            .join("tools")
            .join("Start-MiniCPM-Translate.ps1"),
    ]
    .into_iter()
    .find(|path| path.exists())
}

fn default_local_service_script(paths: &Paths) -> PathBuf {
    paths
        .root_dir
        .join("tools")
        .join("Start-MiniCPM-Translate.ps1")
}

fn local_model_path(paths: &Paths) -> PathBuf {
    paths.root_dir.join("models").join("minicpm5-1b-q4.gguf")
}

fn local_server_path(paths: &Paths) -> PathBuf {
    [
        paths
            .root_dir
            .join("llama.cpp")
            .join("cpu")
            .join("llama-server.exe"),
        paths.root_dir.join("llama-server.exe"),
    ]
    .into_iter()
    .find(|path| path.exists())
    .unwrap_or_else(|| {
        paths
            .root_dir
            .join("llama.cpp")
            .join("cpu")
            .join("llama-server.exe")
    })
}

fn correction_hint(paths: &Paths) -> String {
    text::load_corrections(&paths.corrections_path)
        .into_iter()
        .map(|(wrong, right)| format!("{wrong} => {right}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn http_ok(url: &str) -> bool {
    Client::builder()
        .timeout(Duration::from_millis(700))
        .connect_timeout(Duration::from_millis(300))
        .build()
        .and_then(|client| client.get(url).send())
        .map(|response| response.status().is_success() || response.status().as_u16() < 500)
        .unwrap_or(false)
}

fn models_endpoint(endpoint: &str) -> String {
    endpoint
        .trim_end_matches('/')
        .replace("/v1/chat/completions", "/v1/models")
        .replace("/chat/completions", "/models")
}

fn is_local_endpoint(endpoint: &str) -> bool {
    endpoint.contains("127.0.0.1")
        || endpoint.contains("localhost")
        || endpoint.contains("[::1]")
        || endpoint.contains("://::1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(temp: &tempfile::TempDir) -> Paths {
        let app_dir = temp.path().join(".voice_ime");
        Paths {
            root_dir: temp.path().join("app"),
            app_dir: app_dir.clone(),
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
    fn local_service_status_reports_packaged_artifacts() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(&temp);
        let script = paths.root_dir.join("tools/Start-MiniCPM-Translate.ps1");
        let model = paths.root_dir.join("models/minicpm5-1b-q4.gguf");
        let server = paths.root_dir.join("llama.cpp/cpu/llama-server.exe");
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::create_dir_all(model.parent().unwrap()).unwrap();
        std::fs::create_dir_all(server.parent().unwrap()).unwrap();
        std::fs::write(&script, "").unwrap();
        std::fs::write(&model, "").unwrap();
        std::fs::write(&server, "").unwrap();

        let status = local_service_status("http://127.0.0.1:18080/v1/chat/completions", &paths);

        assert!(status.is_local);
        assert!(status.script_exists);
        assert!(status.model_exists);
        assert!(status.server_exists);
        assert!(status.models_url.ends_with("/v1/models"));
    }

    #[test]
    fn smart_correct_preserves_code_commands_and_paths() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(&temp);
        let mut config = AppConfig::default();
        config.smart.enabled = true;
        config.smart.endpoint = "http://203.0.113.1:65535/v1/chat/completions".into();

        assert_eq!(
            smart_correct(
                "cargo test -- --nocapture",
                "",
                &config,
                &paths,
                "",
                &CancellationToken::new(),
            ),
            "cargo test -- --nocapture"
        );
        assert_eq!(
            smart_correct(
                "帮我改得更正式一点",
                "fn main() { println!(\"hi\"); }",
                &config,
                &paths,
                "",
                &CancellationToken::new(),
            ),
            "fn main() { println!(\"hi\"); }"
        );
    }
}
