use crate::{config::AppConfig, config::Paths, text};
use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{process::Command, time::Duration};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

pub fn smart_correct(
    raw_text: &str,
    base_text: &str,
    config: &AppConfig,
    paths: &Paths,
    personal_prompt: &str,
) -> String {
    let corrected = text::apply_corrections(raw_text, &paths.corrections_path);
    let base_text = text::normalize_text(base_text);
    let edit_existing = !base_text.is_empty() && text::is_confirmation_edit_command(&corrected);
    if corrected.trim().is_empty() {
        return if edit_existing {
            base_text
        } else {
            String::new()
        };
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

pub fn translate(
    source: &str,
    target_language: &str,
    config: &AppConfig,
    paths: &Paths,
    _personal_prompt: &str,
) -> Result<String> {
    let source = text::normalize_text(source);
    if source.trim().is_empty() {
        return Err(anyhow!("没有可翻译的文本"));
    }
    if text::looks_like_prompt_leak(&source) {
        return Err(anyhow!("当前文本像内部提示泄漏，已拒绝翻译"));
    }
    if config.translation.endpoint.trim().is_empty() {
        return Err(anyhow!("未配置翻译端点"));
    }
    if is_local_endpoint(&config.translation.endpoint) {
        ensure_local_service(&config.translation.endpoint, paths)?;
    }
    let language_name = match target_language {
        "en" => "英语",
        "ja" => "日语",
        "zh" => "简体中文",
        other => other,
    };
    let max_tokens = (source.chars().count() * 3 + 48).clamp(64, 1024);
    let payload = json!({
        "model": config.translation.model,
        "messages": [
            { "role": "system", "content": "你是翻译引擎。只翻译用户提供的原文，保留产品名、文件名、代码、命令和数字。不解释，不提要求，不列清单，不询问确认。" },
            { "role": "user", "content": format!("目标语言：{language_name}\n\n原文：\n{source}\n\n只输出译文。") }
        ],
        "temperature": 0,
        "max_tokens": max_tokens,
        "stream": false,
        "stop": ["\n原文：", "[原文]", "[/原文]", "【原文】", "【/原文】"]
    });
    let translated = text::clean_llm_output(&chat_completion(
        &config.translation.endpoint,
        &payload,
        config.translation.timeout_seconds,
    )?);
    if translated.is_empty() {
        Err(anyhow!("翻译结果为空"))
    } else if text::looks_like_prompt_leak(&translated)
        || text::looks_like_missing_edit_target(&translated)
    {
        Err(anyhow!("翻译模型输出了提示词/确认清单，已丢弃"))
    } else {
        Ok(translated)
    }
}

fn chat_completion(endpoint: &str, payload: &Value, timeout_seconds: u64) -> Result<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_seconds.max(1)))
        .build()?;
    let value: Value = client
        .post(endpoint)
        .json(payload)
        .send()?
        .error_for_status()?
        .json()?;
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

fn ensure_local_service(endpoint: &str, paths: &Paths) -> Result<()> {
    if http_ok(&models_endpoint(endpoint)) {
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
        .current_dir(&paths.root_dir);
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(0x08000000);
    }
    let _ = command.output()?;
    if http_ok(&models_endpoint(endpoint)) {
        Ok(())
    } else {
        Err(anyhow!("本地 MiniCPM/llama-server 未响应"))
    }
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

fn correction_hint(paths: &Paths) -> String {
    text::load_corrections(&paths.corrections_path)
        .into_iter()
        .map(|(wrong, right)| format!("{wrong} => {right}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn http_ok(url: &str) -> bool {
    Client::builder()
        .timeout(Duration::from_secs(1))
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
