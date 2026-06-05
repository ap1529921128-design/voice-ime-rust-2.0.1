use crate::config::Paths;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize)]
pub struct ModelPackInstallReport {
    pub output_dir: String,
    pub files_written: usize,
    pub files_replaced: usize,
    pub files_ignored: usize,
    pub checksum_verified: usize,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelPackMetadata {
    #[serde(default)]
    files: Vec<ModelPackFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelPackFile {
    path: String,
    bytes: u64,
    sha256: String,
}

pub fn install(zip_path: &Path, paths: &Paths) -> Result<ModelPackInstallReport> {
    if !zip_path.exists() {
        return Err(anyhow!("模型包不存在：{}", zip_path.to_string_lossy()));
    }
    fs::create_dir_all(paths.root_dir.join("models"))?;

    let file = File::open(zip_path)
        .with_context(|| format!("打开模型包失败：{}", zip_path.to_string_lossy()))?;
    let mut archive = ZipArchive::new(file).context("读取模型包失败")?;
    let checksum_verified = validate_metadata(&mut archive)?;
    let mut report = ModelPackInstallReport {
        output_dir: paths.root_dir.join("models").to_string_lossy().to_string(),
        files_written: 0,
        files_replaced: 0,
        files_ignored: 0,
        checksum_verified,
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

fn validate_metadata(archive: &mut ZipArchive<File>) -> Result<usize> {
    let Some(metadata) = read_metadata(archive)? else {
        return Ok(0);
    };
    let mut verified = 0;
    for file in metadata.files {
        if model_relative_path(&file.path).is_none() {
            return Err(anyhow!("模型包校验路径非法：{}", file.path));
        }
        let entry_name = find_archive_entry_name(archive, &file.path)
            .with_context(|| format!("模型包校验文件缺失：{}", file.path))?;
        let mut entry = archive
            .by_name(&entry_name)
            .with_context(|| format!("读取模型包校验文件失败：{}", file.path))?;
        if entry.size() != file.bytes {
            return Err(anyhow!(
                "模型包校验大小不匹配：{} expected={} actual={}",
                file.path,
                file.bytes,
                entry.size()
            ));
        }
        let digest = sha256_reader(&mut entry)?;
        if !digest.eq_ignore_ascii_case(file.sha256.trim()) {
            return Err(anyhow!("模型包校验失败：{}", file.path));
        }
        verified += 1;
    }
    Ok(verified)
}

fn find_archive_entry_name(archive: &mut ZipArchive<File>, expected: &str) -> Result<String> {
    let expected = normalized_zip_name(expected);
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if normalized_zip_name(entry.name()) == expected {
            return Ok(entry.name().to_string());
        }
    }
    Err(anyhow!("not found"))
}

fn normalized_zip_name(value: &str) -> String {
    value.replace('\\', "/")
}

fn read_metadata(archive: &mut ZipArchive<File>) -> Result<Option<ModelPackMetadata>> {
    match archive.by_name("MODEL_PACK.json") {
        Ok(mut entry) => {
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            let text = text.trim_start_matches('\u{feff}');
            Ok(Some(
                serde_json::from_str(text).context("解析 MODEL_PACK.json 失败")?,
            ))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn sha256_reader(reader: &mut impl Read) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn model_relative_path(entry_name: &str) -> Option<PathBuf> {
    let normalized = entry_name.replace('\\', "/");
    let relative = normalized
        .strip_prefix("app/models/")
        .or_else(|| normalized.strip_prefix("models/"))
        .or({
            if matches!(normalized.as_str(), "MODEL_PACK.txt" | "MODEL_PACK.json") {
                Some(normalized.as_str())
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
                ("MODEL_PACK.json", b"{}"),
                ("README.md", b"ignored"),
            ],
        );

        let report = install(&zip_path, &paths).unwrap();

        assert_eq!(report.files_written, 4);
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
        assert_eq!(
            fs::read(paths.root_dir.join("models/MODEL_PACK.json")).unwrap(),
            b"{}"
        );
    }

    #[test]
    fn rejects_zip_slip_paths() {
        assert!(model_relative_path("app/models/../evil.txt").is_none());
        assert!(model_relative_path("app/models/C:/evil.txt").is_none());
        assert!(model_relative_path("/models/evil.txt").is_none());
    }

    #[test]
    fn verifies_model_pack_metadata_before_extracting() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(&temp);
        let zip_path = temp.path().join("pack.zip");
        let metadata = r#"{
          "schema_version": 1,
          "files": [
            {
              "path": "app/models/foo/model.onnx",
              "bytes": 3,
              "sha256": "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae"
            }
          ]
        }"#;
        write_zip(
            &zip_path,
            &[
                ("app/models/foo/model.onnx", b"foo"),
                ("MODEL_PACK.json", metadata.as_bytes()),
            ],
        );

        let report = install(&zip_path, &paths).unwrap();

        assert_eq!(report.checksum_verified, 1);
        assert_eq!(
            fs::read(paths.root_dir.join("models/foo/model.onnx")).unwrap(),
            b"foo"
        );
    }

    #[test]
    fn verifies_metadata_when_zip_entries_use_backslashes() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(&temp);
        let zip_path = temp.path().join("pack.zip");
        let metadata = r#"{
          "schema_version": 1,
          "files": [
            {
              "path": "app/models/foo/model.onnx",
              "bytes": 3,
              "sha256": "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae"
            }
          ]
        }"#;
        write_zip(
            &zip_path,
            &[
                ("app\\models\\foo\\model.onnx", b"foo"),
                ("MODEL_PACK.json", metadata.as_bytes()),
            ],
        );

        let report = install(&zip_path, &paths).unwrap();

        assert_eq!(report.checksum_verified, 1);
        assert_eq!(
            fs::read(paths.root_dir.join("models/foo/model.onnx")).unwrap(),
            b"foo"
        );
    }

    #[test]
    fn accepts_metadata_with_utf8_bom() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(&temp);
        let zip_path = temp.path().join("pack.zip");
        let metadata = r#"{
          "schema_version": 1,
          "files": [
            {
              "path": "app/models/foo/model.onnx",
              "bytes": 3,
              "sha256": "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae"
            }
          ]
        }"#;
        let mut bom_metadata = vec![0xef, 0xbb, 0xbf];
        bom_metadata.extend_from_slice(metadata.as_bytes());
        write_zip(
            &zip_path,
            &[
                ("app/models/foo/model.onnx", b"foo"),
                ("MODEL_PACK.json", bom_metadata.as_slice()),
            ],
        );

        let report = install(&zip_path, &paths).unwrap();

        assert_eq!(report.checksum_verified, 1);
    }

    #[test]
    fn rejects_model_pack_metadata_hash_mismatch_before_extracting() {
        let temp = tempfile::tempdir().unwrap();
        let paths = temp_paths(&temp);
        let zip_path = temp.path().join("pack.zip");
        let metadata = r#"{
          "schema_version": 1,
          "files": [
            {
              "path": "app/models/foo/model.onnx",
              "bytes": 3,
              "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
            }
          ]
        }"#;
        write_zip(
            &zip_path,
            &[
                ("app/models/foo/model.onnx", b"foo"),
                ("MODEL_PACK.json", metadata.as_bytes()),
            ],
        );

        let err = install(&zip_path, &paths).unwrap_err();

        assert!(err.to_string().contains("模型包校验失败"));
        assert!(!paths.root_dir.join("models/foo/model.onnx").exists());
    }
}
