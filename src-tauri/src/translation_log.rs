use crate::config::Paths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranslationLogEntry {
    pub created_at: String,
    pub session_id: u64,
    pub target_language: String,
    pub engine: String,
    pub model: String,
    pub timeout_seconds: u64,
    pub elapsed_seconds: f32,
    pub source_chars: usize,
    pub output_chars: usize,
    pub status: TranslationLogStatus,
    pub error: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranslationLogStatus {
    Ok,
    Error,
}

pub fn append(paths: &Paths, entry: &TranslationLogEntry) -> Result<PathBuf> {
    fs::create_dir_all(&paths.logs_dir)?;
    let path = paths.logs_dir.join(format!(
        "translation-{}.log",
        chrono::Local::now().format("%Y%m%d")
    ));
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{}", serde_json::to_string(entry)?)?;
    Ok(path)
}

pub fn recent(paths: &Paths, limit: usize) -> Vec<TranslationLogEntry> {
    if limit == 0 {
        return Vec::new();
    }
    let Some(path) = latest_translation_log(&paths.logs_dir) else {
        return Vec::new();
    };
    read_entries(&path)
        .into_iter()
        .rev()
        .take(limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn latest_translation_log(logs_dir: &Path) -> Option<PathBuf> {
    fs::read_dir(logs_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy();
            (name.starts_with("translation-") && name.ends_with(".log"))
                .then(|| {
                    entry
                        .metadata()
                        .ok()
                        .and_then(|meta| meta.modified().ok())
                        .map(|modified| (modified, path))
                })
                .flatten()
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn read_entries(path: &Path) -> Vec<TranslationLogEntry> {
    let Ok(file) = fs::File::open(path) else {
        return Vec::new();
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<TranslationLogEntry>(&line).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;

    fn paths(root: &Path) -> Paths {
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

    fn entry(session_id: u64, status: TranslationLogStatus) -> TranslationLogEntry {
        TranslationLogEntry {
            created_at: "2026-06-05 18:00:00".into(),
            session_id,
            target_language: "en".into(),
            engine: "llm".into(),
            model: "minicpm".into(),
            timeout_seconds: 8,
            elapsed_seconds: 1.2,
            source_chars: 4,
            output_chars: if status == TranslationLogStatus::Ok {
                6
            } else {
                0
            },
            status,
            error: if status == TranslationLogStatus::Ok {
                String::new()
            } else {
                "timeout".into()
            },
        }
    }

    #[test]
    fn appends_and_reads_recent_entries() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(temp.path());

        append(&paths, &entry(1, TranslationLogStatus::Ok)).unwrap();
        append(&paths, &entry(2, TranslationLogStatus::Error)).unwrap();

        let rows = recent(&paths, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session_id, 2);
        assert_eq!(rows[0].status, TranslationLogStatus::Error);
    }
}
