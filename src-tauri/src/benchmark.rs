use crate::{
    asr::{self, AsrInput},
    config::{self, Paths},
};
use anyhow::Result;
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

pub fn run_asr_cli(samples_dir: PathBuf) -> Result<()> {
    let paths = Paths::discover()?;
    let config = config::load_or_create(&paths)?;
    paths.ensure()?;
    fs::create_dir_all(&paths.logs_dir)?;
    let output_path = paths.logs_dir.join(format!(
        "asr-benchmark-{}.csv",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    let files = collect_wav_files(&samples_dir);
    let mut rows = vec![[
        "file",
        "duration_seconds",
        "profile",
        "worker_mode",
        "backend",
        "model",
        "transcribe_seconds",
        "rtf",
        "expected",
        "text",
        "error",
    ]
    .join(",")];

    if files.is_empty() {
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
            asr::transcribe(&input, &config, &paths).map(|outcome| (duration_seconds, outcome))
        });
        match result {
            Ok((duration_seconds, outcome)) => {
                let elapsed = started.elapsed().as_secs_f32().max(outcome.elapsed_seconds);
                let rtf = if duration_seconds > 0.0 {
                    elapsed / duration_seconds
                } else {
                    0.0
                };
                rows.push(csv_row(&[
                    &file_label,
                    &format!("{duration_seconds:.3}"),
                    &config.asr.profile,
                    &config.asr.worker_mode,
                    &outcome.backend,
                    &outcome.model,
                    &format!("{elapsed:.3}"),
                    &format!("{rtf:.3}"),
                    &expected,
                    &outcome.text,
                    "",
                ]));
            }
            Err(err) => rows.push(csv_row(&[
                &file_label,
                "",
                &config.asr.profile,
                &config.asr.worker_mode,
                "",
                "",
                &format!("{:.3}", started.elapsed().as_secs_f32()),
                "",
                &expected,
                "",
                &err.to_string(),
            ])),
        }
    }

    fs::write(&output_path, rows.join("\n"))?;
    println!("{}", output_path.to_string_lossy());
    Ok(())
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
}
