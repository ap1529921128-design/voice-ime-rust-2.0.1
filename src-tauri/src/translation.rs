use crate::{
    config::{AppConfig, Paths},
    llm, text,
};
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

pub fn translate(
    source: &str,
    target_language: &str,
    config: &AppConfig,
    paths: &Paths,
    personal_prompt: &str,
) -> Result<String> {
    match config
        .translation
        .engine
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "llm" => {
            llm::translate_with_llm(source, target_language, config, paths, personal_prompt)
        }
        "external" => translate_with_external(source, target_language, config, paths),
        "nllb" | "bergamot" => Err(anyhow!(
            "翻译引擎 {} 已预留，当前版本请先选择 llm 或 external",
            config.translation.engine
        )),
        other => Err(anyhow!("未知翻译引擎：{other}")),
    }
}

fn translate_with_external(
    source: &str,
    target_language: &str,
    config: &AppConfig,
    paths: &Paths,
) -> Result<String> {
    let source = prepare_source(source, target_language)?;
    if source.already_done {
        return Ok(source.text);
    }
    let command_line = config.translation.external_command.trim();
    if command_line.is_empty() {
        return Err(anyhow!("未配置外部翻译命令"));
    }
    let args = split_command_line(command_line)?;
    let executable = args.first().ok_or_else(|| anyhow!("外部翻译命令为空"))?;
    let payload = json!({
        "source": source.text,
        "target_language": target_language,
        "target_name": target_label(target_language),
    });
    let mut command = Command::new(executable);
    command
        .args(&args[1..])
        .current_dir(&paths.root_dir)
        .env("VOICE_IME_TRANSLATION_TARGET", target_language)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("启动外部翻译命令失败：{executable}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(serde_json::to_string(&payload)?.as_bytes())
            .context("写入外部翻译输入失败")?;
    }
    drop(child.stdin.take());

    wait_child(
        &mut child,
        Duration::from_secs(config.translation.timeout_seconds.clamp(1, 30)),
    )?;
    let mut stdout = String::new();
    if let Some(mut stream) = child.stdout.take() {
        stream.read_to_string(&mut stdout)?;
    }
    let mut stderr = String::new();
    if let Some(mut stream) = child.stderr.take() {
        stream.read_to_string(&mut stderr)?;
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(anyhow!(
            "外部翻译命令退出失败：{}",
            stderr.trim().chars().take(160).collect::<String>()
        ));
    }
    let raw = parse_external_output(&stdout)?;
    validate_output(&raw)
}

#[derive(Debug, PartialEq, Eq)]
struct PreparedSource {
    text: String,
    already_done: bool,
}

fn prepare_source(source: &str, target_language: &str) -> Result<PreparedSource> {
    let mut text = text::normalize_text(source);
    if text.trim().is_empty() {
        return Err(anyhow!("没有可翻译的文本"));
    }
    if text::has_translation_markup(&text) {
        let cleaned = text::clean_translation_output(&text);
        if !cleaned.is_empty() {
            text = cleaned;
        }
    }
    if text::looks_like_prompt_leak(&text) {
        return Err(anyhow!("当前文本像内部提示泄漏，已拒绝翻译"));
    }
    if target_language == "zh" && text::is_likely_chinese_text(&text) {
        let cleaned = text::clean_translation_output(&text);
        if !cleaned.is_empty() {
            return Ok(PreparedSource {
                text: cleaned,
                already_done: true,
            });
        }
        if text::looks_like_translation_chatter(&text) {
            return Err(anyhow!("当前文本像翻译说明文字，已拒绝继续套娃"));
        }
        return Ok(PreparedSource {
            text,
            already_done: true,
        });
    }
    Ok(PreparedSource {
        text,
        already_done: false,
    })
}

fn wait_child(child: &mut Child, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!("外部翻译超时"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn parse_external_output(stdout: &str) -> Result<String> {
    let trimmed = stdout.trim().trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err(anyhow!("外部翻译结果为空"));
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') || trimmed.starts_with('"') {
        let value: Value = serde_json::from_str(trimmed).context("解析外部翻译 JSON 失败")?;
        return json_text_field(&value).ok_or_else(|| anyhow!("外部翻译 JSON 缺少 text 字段"));
    }
    Ok(trimmed.to_string())
}

fn json_text_field(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.trim().to_string());
    }
    ["text", "translation", "result", "output"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .map(str::trim)
        .map(str::to_string)
}

fn validate_output(raw: &str) -> Result<String> {
    let translated = text::clean_translation_output(raw);
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

pub(crate) fn split_command_line(input: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in input.trim().chars() {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                args.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if quote.is_some() {
        return Err(anyhow!("外部翻译命令引号未闭合"));
    }
    if !current.is_empty() {
        args.push(current);
    }
    if args.is_empty() {
        return Err(anyhow!("外部翻译命令为空"));
    }
    Ok(args)
}

fn target_label(target_language: &str) -> &'static str {
    match target_language {
        "en" => "英语",
        "ja" => "日语",
        "zh" => "简体中文",
        _ => "目标语言",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_external_command() {
        let args =
            split_command_line(r#""C:\Program Files\mt\translator.exe" --mode fast"#).unwrap();
        assert_eq!(
            args,
            vec![
                r"C:\Program Files\mt\translator.exe".to_string(),
                "--mode".into(),
                "fast".into()
            ]
        );
    }

    #[test]
    fn rejects_unclosed_external_command_quote() {
        assert!(split_command_line(r#""C:\Program Files\translator.exe"#).is_err());
    }

    #[test]
    fn parses_external_plain_or_json_output() {
        assert_eq!(
            parse_external_output("The Star of Africa").unwrap(),
            "The Star of Africa"
        );
        assert_eq!(
            parse_external_output(r#"{"text":"The Star of Africa"}"#).unwrap(),
            "The Star of Africa"
        );
        assert_eq!(
            parse_external_output(r#""The Star of Africa""#).unwrap(),
            "The Star of Africa"
        );
    }

    #[test]
    fn keeps_chinese_when_target_is_chinese() {
        assert_eq!(
            prepare_source("非洲之星和海洋之泪", "zh").unwrap(),
            PreparedSource {
                text: "非洲之星和海洋之泪".into(),
                already_done: true,
            }
        );
    }

    #[test]
    fn rejects_external_translation_chatter() {
        assert!(validate_output("说明：这句话可以根据语境翻译。").is_err());
    }
}
