use crate::config::Paths;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::{
    fs::{self, File},
    io,
    path::{Component, Path, PathBuf},
};
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize)]
pub struct ModelPackInstallReport {
    pub output_dir: String,
    pub files_written: usize,
    pub files_replaced: usize,
    pub files_ignored: usize,
    pub bytes_written: u64,
}

pub fn install(zip_path: &Path, paths: &Paths) -> Result<ModelPackInstallReport> {
    if !zip_path.exists() {
        return Err(anyhow!("模型包不存在：{}", zip_path.to_string_lossy()));
    }
    fs::create_dir_all(paths.root_dir.join("models"))?;

    let file = File::open(zip_path)
        .with_context(|| format!("打开模型包失败：{}", zip_path.to_string_lossy()))?;
    let mut archive = ZipArchive::new(file).context("读取模型包失败")?;
    let mut report = ModelPackInstallReport {
        output_dir: paths.root_dir.join("models").to_string_lossy().to_string(),
        files_written: 0,
        files_replaced: 0,
        files_ignored: 0,
        bytes_written: 0,
    };

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let Some(relative) = model_relative_path(entry.name()) else {
            report.files_ignored += 1;
            continue;
        };
        let destination = paths.root_dir.join("models").join(relative);
        if destination.exists() {
            report.files_replaced += 1;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&destination)
            .with_context(|| format!("写入模型文件失败：{}", destination.to_string_lossy()))?;
        let bytes = io::copy(&mut entry, &mut output)?;
        report.bytes_written += bytes;
        report.files_written += 1;
    }

    if report.files_written == 0 {
        return Err(anyhow!("模型包里没有可导入的 app/models 或 models 文件"));
    }
    Ok(report)
}

fn model_relative_path(entry_name: &str) -> Option<PathBuf> {
    let normalized = entry_name.replace('\\', "/");
    let relative = normalized
        .strip_prefix("app/models/")
        .or_else(|| normalized.strip_prefix("models/"))
        .or_else(|| {
            if normalized == "MODEL_PACK.txt" {
                Some("MODEL_PACK.txt")
            } else {
                None
            }
        })?;
    safe_relative_path(relative)
}

fn safe_relative_path(value: &str) -> Option<PathBuf> {
    let mut output = PathBuf::new();
    for component in Path::new(value).components() {
        match component {
            Component::Normal(part) => output.push(part),
            _ => return None,
        }
    }
    if output.as_os_str().is_empty() {
        None
    } else {
        Some(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{write::FileOptions, ZipWriter};

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

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = File::create(path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default();
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn installs_only_model_entries_from_pack() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(&temp);
        let zip_path = temp.path().join("pack.zip");
        write_zip(
            &zip_path,
            &[
                ("app/models/foo/model.onnx", b"model"),
                ("models/foo/tokens.txt", b"tokens"),
                ("MODEL_PACK.txt", b"meta"),
                ("README.md", b"ignored"),
            ],
        );

        let report = install(&zip_path, &paths).unwrap();

        assert_eq!(report.files_written, 3);
        assert_eq!(report.files_ignored, 1);
        assert_eq!(
            fs::read(paths.root_dir.join("models/foo/model.onnx")).unwrap(),
            b"model"
        );
        assert_eq!(
            fs::read(paths.root_dir.join("models/foo/tokens.txt")).unwrap(),
            b"tokens"
        );
        assert_eq!(
            fs::read(paths.root_dir.join("models/MODEL_PACK.txt")).unwrap(),
            b"meta"
        );
    }

    #[test]
    fn rejects_zip_slip_paths() {
        assert!(model_relative_path("app/models/../evil.txt").is_none());
        assert!(model_relative_path("app/models/C:/evil.txt").is_none());
        assert!(model_relative_path("/models/evil.txt").is_none());
    }
}
