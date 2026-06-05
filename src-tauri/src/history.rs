use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

const CSV_HEADERS: [&str; 21] = [
    "session_id",
    "created_at",
    "text",
    "raw_text",
    "normalized_text",
    "dictionary_text",
    "hotword_text",
    "rule_text",
    "itn_text",
    "llm_text",
    "punctuation_policy",
    "duration_seconds",
    "source_sample_rate",
    "sample_rate",
    "resampled",
    "transcribe_seconds",
    "deterministic_seconds",
    "llm_seconds",
    "total_seconds",
    "backend",
    "model",
];

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
    #[serde(default)]
    pub source_sample_rate: u32,
    #[serde(default)]
    pub sample_rate: u32,
    #[serde(default)]
    pub resampled: bool,
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
        source_sample_rate: u32,
        sample_rate: u32,
        resampled: bool,
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
            source_sample_rate,
            sample_rate,
            resampled,
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

pub fn export_csv_file(path: &Path, records: &[TranscriptRecord]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut body = String::from('\u{feff}');
    body.push_str(&records_to_csv(records));
    fs::write(path, body)?;
    Ok(())
}

fn records_to_csv(records: &[TranscriptRecord]) -> String {
    let mut output = String::new();
    output.push_str(&CSV_HEADERS.join(","));
    output.push('\n');
    for record in records {
        let row = [
            record.session_id.to_string(),
            record.created_at.clone(),
            record.text.clone(),
            record.raw_text.clone(),
            record.normalized_text.clone(),
            record.dictionary_text.clone(),
            record.hotword_text.clone(),
            record.rule_text.clone(),
            record.itn_text.clone(),
            record.llm_text.clone(),
            record.punctuation_policy.clone(),
            format!("{:.3}", record.duration_seconds),
            record.source_sample_rate.to_string(),
            record.sample_rate.to_string(),
            record.resampled.to_string(),
            format!("{:.3}", record.transcribe_seconds),
            format!("{:.3}", record.deterministic_seconds),
            format!("{:.3}", record.llm_seconds),
            format!("{:.3}", record.total_seconds),
            record.backend.clone(),
            record.model.clone(),
        ];
        output.push_str(
            &row.iter()
                .map(|value| csv_cell(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    output
}

fn csv_cell(value: &str) -> String {
    let mut value = value.replace("\r\n", "\n").replace('\r', "\n");
    if value
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, '=' | '+' | '-' | '@' | '\t'))
    {
        value.insert(0, '\'');
    }
    format!("\"{}\"", value.replace('"', "\"\""))
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

    pub fn flush(&self, path: &Path) -> Result<()> {
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
                48_000,
                16_000,
                true,
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

    #[test]
    fn exports_full_trace_csv_with_safe_cells() {
        let record = TranscriptRecord {
            session_id: 7,
            text: "=cmd".into(),
            raw_text: "hello, \"world\"".into(),
            normalized_text: "line\r\nnext".into(),
            dictionary_text: String::new(),
            hotword_text: String::new(),
            rule_text: String::new(),
            itn_text: String::new(),
            llm_text: String::new(),
            punctuation_policy: "default".into(),
            created_at: "2026-06-05 12:00:00".into(),
            duration_seconds: 1.23456,
            source_sample_rate: 48_000,
            sample_rate: 16_000,
            resampled: true,
            transcribe_seconds: 0.5,
            deterministic_seconds: 0.01,
            llm_seconds: 0.2,
            total_seconds: 0.71,
            backend: "sherpa".into(),
            model: "balanced".into(),
        };
        let csv = records_to_csv(&[record]);
        assert!(csv.starts_with("session_id,created_at,text,raw_text"));
        assert!(csv.contains("\"'=cmd\""));
        assert!(csv.contains("\"48000\",\"16000\",\"true\""));
        assert!(csv.contains("\"hello, \"\"world\"\"\""));
        assert!(csv.contains("\"line\nnext\""));
        assert!(csv.contains("\"1.235\""));
    }
}
