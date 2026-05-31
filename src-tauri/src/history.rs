use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptRecord {
    pub text: String,
    pub created_at: String,
    pub duration_seconds: f32,
    pub transcribe_seconds: f32,
    pub backend: String,
    pub model: String,
}

impl TranscriptRecord {
    pub fn new(
        text: String,
        duration_seconds: f32,
        transcribe_seconds: f32,
        backend: String,
        model: String,
    ) -> Self {
        Self {
            text,
            created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            duration_seconds,
            transcribe_seconds,
            backend,
            model,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HistoryStore {
    limit: usize,
    records: Vec<TranscriptRecord>,
}

impl HistoryStore {
    pub fn load(path: &Path, limit: usize) -> Self {
        let records = fs::read_to_string(path)
            .ok()
            .and_then(|body| serde_json::from_str::<Vec<TranscriptRecord>>(&body).ok())
            .unwrap_or_default();
        Self { limit, records }
    }

    pub fn records(&self) -> &[TranscriptRecord] {
        &self.records
    }

    pub fn add(&mut self, record: TranscriptRecord, path: &Path) -> Result<()> {
        self.records.insert(0, record);
        self.records.truncate(self.limit);
        self.save(path)
    }

    pub fn clear(&mut self, path: &Path) -> Result<()> {
        self.records.clear();
        self.save(path)
    }

    fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(&self.records)?)?;
        Ok(())
    }
}
