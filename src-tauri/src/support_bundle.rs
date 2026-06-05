use crate::config::{self, AppConfig, Paths};
use anyhow::{Context, Result};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};
use zip::{write::FileOptions, ZipWriter};

pub fn export(paths: &Paths, config: &AppConfig) -> Result<PathBuf> {
    paths.ensure()?;
    fs::create_dir_all(&paths.logs_dir)?;
    let output_path = paths.logs_dir.join(format!(
        "voice-ime-support-{}.zip",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    let file = File::create(&output_path).context("创建诊断导出包")?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    add_text(
        &mut zip,
        "summary.txt",
        &support_summary(paths, config),
        options,
    )?;
    add_optional_file(&mut zip, &paths.config_path, "config/config.json", options)?;
    add_optional_file(&mut zip, &paths.history_path, "data/history.json", options)?;
    add_optional_file(
        &mut zip,
        &paths.prompt_path,
        "data/personal_prompt.txt",
        options,
    )?;
    add_optional_file(
        &mut zip,
        &paths.corrections_path,
        "data/corrections.json",
        options,
    )?;
    add_optional_file(&mut zip, &paths.hotwords_path, "data/hot.txt", options)?;
    add_optional_file(
        &mut zip,
        &paths.hot_rules_path,
        "data/hot-rule.txt",
        options,
    )?;
    let model_root = config::effective_model_root(config, paths);
    add_optional_file(
        &mut zip,
        &model_root.join("MODELS.json"),
        "models/MODELS.json",
        options,
    )?;
    add_optional_file(
        &mut zip,
        &model_root.join("MODELS.md"),
        "models/MODELS.md",
        options,
    )?;
    add_log_files(&mut zip, &paths.logs_dir, options)?;
    zip.finish().context("写入诊断导出包")?;
    Ok(output_path)
}

fn support_summary(paths: &Paths, config: &AppConfig) -> String {
    [
        "Voice IME Support Bundle".to_string(),
        format!(
            "Created: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ),
        format!("Root: {}", paths.root_dir.to_string_lossy()),
        format!("App: {}", paths.app_dir.to_string_lossy()),
        format!(
            "Models: {}",
            config::effective_model_root(config, paths).to_string_lossy()
        ),
        format!(
            "ASR: profile={} worker={} threads={}",
            config.asr.profile, config.asr.worker_mode, config.asr.num_threads
        ),
        format!(
            "Input: mode={} ptt={} key={} mouse={} hold_threshold_ms={}",
            config.input.mode,
            config.input.ptt_enabled,
            config.input.ptt_key,
            config.input.ptt_mouse_button,
            config.input.ptt_hold_threshold_ms
        ),
        format!(
            "Translation: engine={} timeout={}s",
            config.translation.engine, config.translation.timeout_seconds
        ),
        "Included: config, history, text dictionaries, logs, model manifest.".to_string(),
        "Excluded: recordings and model binary files.".to_string(),
    ]
    .join("\n")
}

fn add_log_files(zip: &mut ZipWriter<File>, logs_dir: &Path, options: FileOptions) -> Result<()> {
    let Ok(entries) = fs::read_dir(logs_dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file()
            || path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        add_optional_file(zip, &path, &format!("logs/{file_name}"), options)?;
    }
    Ok(())
}

fn add_optional_file(
    zip: &mut ZipWriter<File>,
    path: &Path,
    archive_name: &str,
    options: FileOptions,
) -> Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    zip.start_file(normalize_archive_name(archive_name), options)?;
    let mut file = File::open(path).with_context(|| format!("读取 {}", path.display()))?;
    io::copy(&mut file, zip)?;
    Ok(())
}

fn add_text(
    zip: &mut ZipWriter<File>,
    archive_name: &str,
    body: &str,
    options: FileOptions,
) -> Result<()> {
    zip.start_file(normalize_archive_name(archive_name), options)?;
    zip.write_all(body.as_bytes())?;
    Ok(())
}

fn normalize_archive_name(name: &str) -> String {
    name.replace('\\', "/")
        .trim_start_matches('/')
        .trim_start_matches("./")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(root: &Path) -> Paths {
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
    fn exports_support_bundle_without_recordings() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        paths.ensure().unwrap();
        fs::write(&paths.config_path, "{}").unwrap();
        fs::write(&paths.history_path, "[]").unwrap();
        fs::write(&paths.logs_dir.join("doctor-test.txt"), "ok").unwrap();
        fs::write(&paths.recordings_dir.join("secret.wav"), "audio").unwrap();
        fs::create_dir_all(temp.path().join("models")).unwrap();
        fs::write(temp.path().join("models").join("MODELS.json"), "{}").unwrap();

        let output = export(&paths, &AppConfig::default()).unwrap();
        let file = File::open(output).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().to_string())
            .collect::<Vec<_>>();
        assert!(names.contains(&"summary.txt".to_string()));
        assert!(names.contains(&"config/config.json".to_string()));
        assert!(names.contains(&"data/history.json".to_string()));
        assert!(names.contains(&"logs/doctor-test.txt".to_string()));
        assert!(names.contains(&"models/MODELS.json".to_string()));
        assert!(!names.iter().any(|name| name.contains("recordings")));
        assert!(!names.iter().any(|name| name.ends_with(".zip")));
    }

    #[test]
    fn normalizes_archive_names() {
        assert_eq!(
            normalize_archive_name(r"\logs\doctor.txt"),
            "logs/doctor.txt"
        );
        assert_eq!(
            normalize_archive_name("./data/history.json"),
            "data/history.json"
        );
    }
}
