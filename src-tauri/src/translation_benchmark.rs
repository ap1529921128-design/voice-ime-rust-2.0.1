use crate::{
    config::{self, Paths},
    text, translation,
};
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Debug, Clone, Serialize)]
pub struct TranslationBenchmarkReport {
    pub output_path: String,
    pub sample_count: usize,
    pub error_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TranslationSample {
    target_language: String,
    source: String,
    expected_hint: String,
}

pub fn run_translation_cli(samples_path: Option<PathBuf>) -> Result<()> {
    let paths = Paths::discover()?;
    let config = config::load_or_create(&paths)?;
    let report = run_translation(samples_path.as_deref(), &paths, &config)?;
    println!("{}", report.output_path);
    Ok(())
}

pub fn run_translation(
    samples_path: Option<&Path>,
    paths: &Paths,
    config: &config::AppConfig,
) -> Result<TranslationBenchmarkReport> {
    paths.ensure()?;
    fs::create_dir_all(&paths.logs_dir)?;
    let output_path = paths.logs_dir.join(format!(
        "translation-benchmark-{}.csv",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    let samples = load_samples(samples_path)?;
    let prompt = fs::read_to_string(&paths.prompt_path).unwrap_or_default();
    let mut error_count = 0;
    let mut rows = vec![[
        "sample_index",
        "target_language",
        "target_name",
        "engine",
        "model",
        "timeout_seconds",
        "elapsed_seconds",
        "language_match",
        "expected_hint",
        "expected_hint_match",
        "source",
        "output",
        "error",
    ]
    .join(",")];

    if samples.is_empty() {
        error_count += 1;
        rows.push(csv_row(&[
            "",
            "",
            "",
            &config.translation.engine,
            &config.translation.model,
            &config.translation.timeout_seconds.to_string(),
            "",
            "",
            "",
            "",
            samples_path
                .map(|path| path.to_string_lossy().to_string())
                .as_deref()
                .unwrap_or("built-in"),
            "",
            "no translation samples found",
        ]));
    }

    for (index, sample) in samples.iter().enumerate() {
        let started = Instant::now();
        let result = translation::translate(
            &sample.source,
            &sample.target_language,
            config,
            paths,
            &prompt,
        );
        let elapsed = started.elapsed().as_secs_f32();
        match result {
            Ok(output) => {
                let chatter = text::looks_like_prompt_leak(&output)
                    || text::looks_like_missing_edit_target(&output)
                    || text::looks_like_translation_chatter(&output);
                let language_match = language_matches(&sample.target_language, &output);
                let hint_match = expected_hint_matches(&sample.expected_hint, &output);
                if chatter || !hint_match.unwrap_or(true) {
                    error_count += 1;
                }
                rows.push(csv_row(&[
                    &(index + 1).to_string(),
                    &sample.target_language,
                    target_label(&sample.target_language),
                    &config.translation.engine,
                    &config.translation.model,
                    &config.translation.timeout_seconds.to_string(),
                    &format!("{elapsed:.3}"),
                    bool_cell(language_match),
                    &sample.expected_hint,
                    hint_match.map(bool_cell).unwrap_or(""),
                    &sample.source,
                    &output,
                    if chatter { "translation chatter" } else { "" },
                ]));
            }
            Err(err) => {
                error_count += 1;
                rows.push(csv_row(&[
                    &(index + 1).to_string(),
                    &sample.target_language,
                    target_label(&sample.target_language),
                    &config.translation.engine,
                    &config.translation.model,
                    &config.translation.timeout_seconds.to_string(),
                    &format!("{elapsed:.3}"),
                    "",
                    &sample.expected_hint,
                    "",
                    &sample.source,
                    "",
                    &err.to_string(),
                ]));
            }
        }
    }

    fs::write(&output_path, rows.join("\n"))?;
    Ok(TranslationBenchmarkReport {
        output_path: output_path.to_string_lossy().to_string(),
        sample_count: samples.len(),
        error_count,
    })
}

fn load_samples(samples_path: Option<&Path>) -> Result<Vec<TranslationSample>> {
    let Some(path) = samples_path.filter(|path| !path.as_os_str().is_empty()) else {
        return Ok(default_samples());
    };
    let path = if path.is_dir() {
        path.join("translation-samples.tsv")
    } else {
        path.to_path_buf()
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    let body = fs::read_to_string(&path)?;
    parse_samples(&body)
}

fn parse_samples(body: &str) -> Result<Vec<TranslationSample>> {
    let mut samples = Vec::new();
    for (line_index, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let cells = if trimmed.contains('\t') {
            trimmed.split('\t').map(str::trim).collect::<Vec<_>>()
        } else {
            split_csv_line(trimmed)?
        };
        if cells.len() < 2 {
            return Err(anyhow!(
                "translation sample line {} needs target and source",
                line_index + 1
            ));
        }
        samples.push(TranslationSample {
            target_language: normalize_target(cells[0]),
            source: cells[1].to_string(),
            expected_hint: cells.get(2).copied().unwrap_or("").trim().to_string(),
        });
    }
    Ok(samples)
}

fn default_samples() -> Vec<TranslationSample> {
    [
        ("en", "非洲之星和海洋之泪", ""),
        ("ja", "非洲之星和海洋之泪", ""),
        ("zh", "翻译结果：非洲之星和海洋之泪", "非洲之星"),
        ("en", "请在明天上午九点提醒我检查模型目录", ""),
        ("ja", "Voice IME 的 fast 模型应该优先保证响应速度", ""),
        (
            "zh",
            "中文：模型包已经导入\n如果需要更诗意的翻译，可以调整。",
            "模型包",
        ),
        ("en", "这个版本不会自动发送回车", ""),
        ("ja", "本地优先，不默认上传云端", ""),
        ("zh", "The translation is: 本地翻译最多等待八秒", "本地翻译"),
        ("en", "设置页可以检查本地 LLM 服务", ""),
    ]
    .into_iter()
    .map(
        |(target_language, source, expected_hint)| TranslationSample {
            target_language: target_language.into(),
            source: source.into(),
            expected_hint: expected_hint.into(),
        },
    )
    .collect()
}

fn split_csv_line(line: &str) -> Result<Vec<&str>> {
    let mut cells = Vec::new();
    let mut start = 0;
    let mut in_quote = false;
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                if in_quote && bytes.get(index + 1) == Some(&b'"') {
                    index += 1;
                } else {
                    in_quote = !in_quote;
                }
            }
            b',' if !in_quote => {
                cells.push(line[start..index].trim().trim_matches('"'));
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    if in_quote {
        return Err(anyhow!("translation sample CSV quote is not closed"));
    }
    cells.push(line[start..].trim().trim_matches('"'));
    Ok(cells)
}

fn normalize_target(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "chinese" | "中文" | "简体中文" => "zh".into(),
        "english" | "英语" | "英文" => "en".into(),
        "japanese" | "日语" | "日文" => "ja".into(),
        other => other.to_string(),
    }
}

fn language_matches(target_language: &str, output: &str) -> bool {
    match target_language {
        "zh" => text::is_likely_chinese_text(output),
        "en" => output.chars().any(|ch| ch.is_ascii_alphabetic()),
        "ja" => {
            output.chars().any(is_japanese_kana) || output.contains('の') || output.contains("です")
        }
        _ => true,
    }
}

fn is_japanese_kana(ch: char) -> bool {
    matches!(ch as u32, 0x3040..=0x30ff)
}

fn expected_hint_matches(expected_hint: &str, output: &str) -> Option<bool> {
    let expected_hint = expected_hint.trim();
    if expected_hint.is_empty() {
        return None;
    }
    let output = output.to_lowercase();
    Some(
        expected_hint
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .all(|part| output.contains(&part.to_lowercase())),
    )
}

fn target_label(target_language: &str) -> &'static str {
    match target_language {
        "en" => "英语",
        "ja" => "日语",
        "zh" => "简体中文",
        _ => "目标语言",
    }
}

fn bool_cell(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn csv_row(cells: &[&str]) -> String {
    cells
        .iter()
        .map(|cell| csv_cell(cell))
        .collect::<Vec<_>>()
        .join(",")
}

fn csv_cell(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths(temp: &tempfile::TempDir) -> Paths {
        let app_dir = temp.path().join(".voice_ime");
        Paths {
            root_dir: temp.path().join("root"),
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
    fn parses_tsv_and_csv_samples() {
        let parsed =
            parse_samples("# comment\n中文\t非洲之星\t非洲\n\"english\",\"本地优先\",\"local\"\n")
                .unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].target_language, "zh");
        assert_eq!(parsed[1].target_language, "en");
        assert_eq!(parsed[1].expected_hint, "local");
    }

    #[test]
    fn default_samples_cover_three_targets() {
        let samples = default_samples();
        assert!(samples.iter().any(|sample| sample.target_language == "zh"));
        assert!(samples.iter().any(|sample| sample.target_language == "en"));
        assert!(samples.iter().any(|sample| sample.target_language == "ja"));
    }

    #[test]
    fn benchmark_writes_csv_even_when_backend_is_unavailable() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(&temp);
        let sample_path = temp.path().join("translation-samples.tsv");
        fs::write(&sample_path, "zh\t翻译结果：非洲之星\t非洲\n").unwrap();
        let mut config = config::AppConfig::default();
        config.translation.engine = "external".into();
        config.translation.external_command = String::new();

        let report = run_translation(Some(&sample_path), &paths, &config).unwrap();

        assert_eq!(report.sample_count, 1);
        assert_eq!(report.error_count, 0);
        let csv = fs::read_to_string(report.output_path).unwrap();
        assert!(csv.contains("target_language"));
        assert!(csv.contains("非洲之星"));
    }
}
