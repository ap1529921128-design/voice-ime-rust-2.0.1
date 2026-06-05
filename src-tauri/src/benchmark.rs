use crate::{
    asr::{self, AsrInput},
    config::{self, Paths},
};
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Debug, Clone, Serialize)]
pub struct AsrBenchmarkReport {
    pub output_path: String,
    pub sample_count: usize,
    pub error_count: usize,
}

pub fn run_asr_cli(samples_dir: PathBuf) -> Result<()> {
    run_asr_cli_with_profile(samples_dir, None)
}

pub fn run_asr_cli_with_profile(samples_dir: PathBuf, profile: Option<&str>) -> Result<()> {
    let paths = Paths::discover()?;
    let mut config = config::load_or_create(&paths)?;
    if let Some(profile) = profile {
        config.asr.profile = cli_profile(profile)?;
    }
    let report = run_asr(&samples_dir, &paths, &config)?;
    println!("{}", report.output_path);
    Ok(())
}

fn cli_profile(profile: &str) -> Result<String> {
    let profile = profile.trim();
    if matches!(profile, "fast" | "balanced" | "fallback") {
        Ok(profile.to_string())
    } else {
        Err(anyhow!(
            "unknown ASR profile '{profile}', expected fast, balanced, or fallback"
        ))
    }
}

pub fn run_asr(
    samples_dir: &Path,
    paths: &Paths,
    config: &config::AppConfig,
) -> Result<AsrBenchmarkReport> {
    paths.ensure()?;
    fs::create_dir_all(&paths.logs_dir)?;
    let output_path = paths.logs_dir.join(format!(
        "asr-benchmark-{}.csv",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    let files = collect_wav_files(samples_dir);
    let sample_count = files.len();
    let mut error_count = 0;
    let mut rows = vec![[
        "file",
        "duration_seconds",
        "profile",
        "worker_mode",
        "backend",
        "model",
        "transcribe_seconds",
        "rtf",
        "expected_chars",
        "edit_distance",
        "cer",
        "accuracy",
        "expected",
        "text",
        "error",
    ]
    .join(",")];

    if files.is_empty() {
        error_count += 1;
        let samples_label = samples_dir.to_string_lossy().to_string();
        rows.push(csv_row(&[
            &samples_label,
            "",
            &config.asr.profile,
            &config.asr.worker_mode,
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "no wav samples found",
        ]));
    }

    for file in files {
        let file_label = file.to_string_lossy().to_string();
        let expected = expected_text_for(&file).unwrap_or_default();
        let started = Instant::now();
        let result = asr::read_wav_file(&file).and_then(|(sample_rate, samples)| {
            let duration_seconds = samples.len() as f32 / sample_rate.max(1) as f32;
            let input = AsrInput {
                samples,
                sample_rate,
                language: config.asr.language.clone(),
                prompt: String::new(),
            };
            asr::transcribe(&input, config, paths).map(|outcome| (duration_seconds, outcome))
        });
        match result {
            Ok((duration_seconds, outcome)) => {
                let elapsed = started.elapsed().as_secs_f32().max(outcome.elapsed_seconds);
                let rtf = if duration_seconds > 0.0 {
                    elapsed / duration_seconds
                } else {
                    0.0
                };
                let score = score_text(&expected, &outcome.text);
                let expected_chars = score
                    .as_ref()
                    .map(|score| score.expected_chars.to_string())
                    .unwrap_or_default();
                let edit_distance = score
                    .as_ref()
                    .map(|score| score.edit_distance.to_string())
                    .unwrap_or_default();
                let cer = score
                    .as_ref()
                    .map(|score| format!("{:.4}", score.char_error_rate))
                    .unwrap_or_default();
                let accuracy = score
                    .as_ref()
                    .map(|score| format!("{:.4}", score.accuracy))
                    .unwrap_or_default();
                rows.push(csv_row(&[
                    &file_label,
                    &format!("{duration_seconds:.3}"),
                    &config.asr.profile,
                    &config.asr.worker_mode,
                    &outcome.backend,
                    &outcome.model,
                    &format!("{elapsed:.3}"),
                    &format!("{rtf:.3}"),
                    &expected_chars,
                    &edit_distance,
                    &cer,
                    &accuracy,
                    &expected,
                    &outcome.text,
                    "",
                ]));
            }
            Err(err) => {
                error_count += 1;
                rows.push(csv_row(&[
                    &file_label,
                    "",
                    &config.asr.profile,
                    &config.asr.worker_mode,
                    "",
                    "",
                    &format!("{:.3}", started.elapsed().as_secs_f32()),
                    "",
                    "",
                    "",
                    "",
                    "",
                    &expected,
                    "",
                    &err.to_string(),
                ]));
            }
        }
    }

    fs::write(&output_path, rows.join("\n"))?;
    Ok(AsrBenchmarkReport {
        output_path: output_path.to_string_lossy().to_string(),
        sample_count,
        error_count,
    })
}

fn collect_wav_files(samples_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(samples_dir) else {
        return Vec::new();
    };
    let mut files = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(OsStr::to_str)
                .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn expected_text_for(wav_path: &Path) -> Option<String> {
    let expected_path = wav_path.with_extension("txt");
    fs::read_to_string(expected_path)
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

#[derive(Debug, Clone, PartialEq)]
struct TextScore {
    expected_chars: usize,
    edit_distance: usize,
    char_error_rate: f32,
    accuracy: f32,
}

fn score_text(expected: &str, actual: &str) -> Option<TextScore> {
    let expected_chars = normalized_score_chars(expected);
    if expected_chars.is_empty() {
        return None;
    }
    let actual_chars = normalized_score_chars(actual);
    let edit_distance = levenshtein_distance(&expected_chars, &actual_chars);
    let char_error_rate = edit_distance as f32 / expected_chars.len() as f32;
    Some(TextScore {
        expected_chars: expected_chars.len(),
        edit_distance,
        char_error_rate,
        accuracy: (1.0 - char_error_rate).max(0.0),
    })
}

fn normalized_score_chars(text: &str) -> Vec<char> {
    text.chars()
        .filter(|ch| !ch.is_whitespace())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn levenshtein_distance(left: &[char], right: &[char]) -> usize {
    if left.is_empty() {
        return right.len();
    }
    if right.is_empty() {
        return left.len();
    }
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_char) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right.iter().enumerate() {
            let substitution = previous[right_index] + usize::from(left_char != right_char);
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            current[right_index + 1] = substitution.min(insertion).min(deletion);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
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

    #[test]
    fn collects_wav_files_sorted() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("b.wav"), "").unwrap();
        fs::write(temp.path().join("a.WAV"), "").unwrap();
        fs::write(temp.path().join("c.txt"), "").unwrap();
        let files = collect_wav_files(temp.path());
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("a.WAV"));
        assert!(files[1].ends_with("b.wav"));
    }

    #[test]
    fn reads_expected_text_next_to_wav() {
        let temp = tempfile::tempdir().unwrap();
        let wav = temp.path().join("sample.wav");
        fs::write(&wav, "").unwrap();
        fs::write(temp.path().join("sample.txt"), "  你好世界  \n").unwrap();
        assert_eq!(expected_text_for(&wav).as_deref(), Some("你好世界"));
    }

    #[test]
    fn scores_text_by_character_error_rate() {
        let score = score_text("你好世界", "你好").unwrap();
        assert_eq!(score.expected_chars, 4);
        assert_eq!(score.edit_distance, 2);
        assert_eq!(score.char_error_rate, 0.5);
        assert_eq!(score.accuracy, 0.5);
    }

    #[test]
    fn score_ignores_spacing_and_case() {
        let score = score_text("Voice IME", "voiceime").unwrap();
        assert_eq!(score.edit_distance, 0);
        assert_eq!(score.char_error_rate, 0.0);
        assert_eq!(score.accuracy, 1.0);
    }

    #[test]
    fn benchmark_empty_directory_writes_no_samples_row() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths {
            root_dir: temp.path().join("root"),
            app_dir: temp.path().join(".voice_ime"),
            model_dir: temp.path().join("root/models"),
            config_path: temp.path().join(".voice_ime/config.json"),
            history_path: temp.path().join(".voice_ime/history.json"),
            prompt_path: temp.path().join(".voice_ime/personal_prompt.txt"),
            corrections_path: temp.path().join(".voice_ime/corrections.json"),
            hotwords_path: temp.path().join(".voice_ime/hot.txt"),
            hot_rules_path: temp.path().join(".voice_ime/hot-rule.txt"),
            recordings_dir: temp.path().join(".voice_ime/recordings"),
            logs_dir: temp.path().join(".voice_ime/logs"),
        };
        let samples = temp.path().join("samples");
        fs::create_dir_all(&samples).unwrap();

        let report = run_asr(&samples, &paths, &config::AppConfig::default()).unwrap();

        assert_eq!(report.sample_count, 0);
        assert_eq!(report.error_count, 1);
        let csv = fs::read_to_string(report.output_path).unwrap();
        assert!(csv.contains("expected_chars,edit_distance,cer,accuracy"));
        assert!(csv.contains("no wav samples found"));
    }

    #[test]
    fn cli_profile_accepts_known_profiles_only() {
        assert_eq!(cli_profile("fast").unwrap(), "fast");
        assert_eq!(cli_profile(" balanced ").unwrap(), "balanced");
        assert!(cli_profile("accurate").is_err());
    }
}
