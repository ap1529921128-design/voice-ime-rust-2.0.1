use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptRecord {
    #[serde(default)]
    pub session_id: u64,
    pub text: String,
    #[serde(default)]
    pub raw_text: String,
    #[serde(default)]
    pub normalized_text: String,
    #[serde(default)]
    pub dictionary_text: String,
    #[serde(default)]
    pub hotword_text: String,
    #[serde(default)]
    pub rule_text: String,
    #[serde(default)]
    pub itn_text: String,
    #[serde(default)]
    pub llm_text: String,
    #[serde(default)]
    pub punctuation_policy: String,
    pub created_at: String,
    pub duration_seconds: f32,
    pub transcribe_seconds: f32,
    #[serde(default)]
    pub deterministic_seconds: f32,
    #[serde(default)]
    pub llm_seconds: f32,
    #[serde(default)]
    pub total_seconds: f32,
    pub backend: String,
    pub model: String,
}

impl TranscriptRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: u64,
        text: String,
        raw_text: String,
        normalized_text: String,
        dictionary_text: String,
        hotword_text: String,
        rule_text: String,
        itn_text: String,
        punctuation_policy: String,
        duration_seconds: f32,
        transcribe_seconds: f32,
        deterministic_seconds: f32,
        total_seconds: f32,
        backend: String,
        model: String,
    ) -> Self {
        Self {
            session_id,
            text,
            raw_text,
            normalized_text,
            dictionary_text,
            hotword_text,
            rule_text,
            itn_text,
            llm_text: String::new(),
            punctuation_policy,
            created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            duration_seconds,
            transcribe_seconds,
            deterministic_seconds,
            llm_seconds: 0.0,
            total_seconds,
            backend,
            model,
        }
    }

    pub fn update_postprocess(&mut self, text: String, llm_seconds: f32, total_seconds: f32) {
        self.llm_text = text.clone();
        self.text = text;
        self.llm_seconds = llm_seconds;
        self.total_seconds = total_seconds;
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

    pub fn update_postprocess(
        &mut self,
        session_id: u64,
        text: String,
        llm_seconds: f32,
        total_seconds: f32,
        path: &Path,
    ) -> Result<()> {
        if let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.session_id == session_id)
        {
            record.update_postprocess(text, llm_seconds, total_seconds);
            self.save(path)?;
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_legacy_records_with_defaults() {
        let value = serde_json::json!([{
            "text": "最终文本",
            "created_at": "2026-06-05 10:00:00",
            "duration_seconds": 1.0,
            "transcribe_seconds": 0.8,
            "backend": "sherpa",
            "model": "balanced"
        }]);
        let records: Vec<TranscriptRecord> = serde_json::from_value(value).unwrap();
        assert_eq!(records[0].text, "最终文本");
        assert_eq!(records[0].session_id, 0);
        assert!(records[0].raw_text.is_empty());
        assert_eq!(records[0].total_seconds, 0.0);
    }

    #[test]
    fn updates_postprocess_record_by_session() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("history.json");
        let mut store = HistoryStore {
            limit: 10,
            records: vec![TranscriptRecord::new(
                42,
                "原文".into(),
                "raw".into(),
                "raw".into(),
                "raw".into(),
                "raw".into(),
                "raw".into(),
                "raw".into(),
                "default".into(),
                1.0,
                0.5,
                0.01,
                0.51,
                "asr".into(),
                "model".into(),
            )],
        };
        store
            .update_postprocess(42, "最终".into(), 0.2, 0.8, &path)
            .unwrap();
        assert_eq!(store.records[0].text, "最终");
        assert_eq!(store.records[0].llm_text, "最终");
        assert_eq!(store.records[0].llm_seconds, 0.2);
        assert_eq!(store.records[0].total_seconds, 0.8);
    }
}
