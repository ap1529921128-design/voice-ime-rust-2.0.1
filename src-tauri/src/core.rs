use crate::{
    asr::{self, AsrInput},
    audio::{self, Recorder},
    config::{self, AppConfig, Paths},
    history::{HistoryStore, TranscriptRecord},
    llm, text,
    win_bridge::{self, InputTarget, OverlayRect},
};
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
    time::Instant,
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Recording,
    Previewing,
    Transcribing,
    LongTranscribing,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiSnapshot {
    pub state: SessionState,
    pub text: String,
    pub status: String,
    pub meta: String,
    pub language: String,
    pub word_count: usize,
    pub overlay_rect: Option<OverlayRect>,
    pub config: AppConfig,
    pub history: Vec<TranscriptRecord>,
}

pub struct AppState {
    pub paths: Paths,
    pub recorder: Recorder,
    pub inner: parking_lot::Mutex<InnerState>,
    session_counter: AtomicU64,
}

pub struct InnerState {
    pub config: AppConfig,
    pub history: HistoryStore,
    pub state: SessionState,
    pub text: String,
    pub status: String,
    pub meta: String,
    pub session_id: u64,
    pub target: Option<InputTarget>,
    pub overlay_rect: Option<OverlayRect>,
}

impl AppState {
    pub fn load() -> Result<Self> {
        let paths = Paths::discover()?;
        let config = config::load_or_create(&paths)?;
        let history = HistoryStore::load(&paths.history_path, config.history_limit);
        Ok(Self {
            paths,
            recorder: Recorder::default(),
            inner: parking_lot::Mutex::new(InnerState {
                config,
                history,
                state: SessionState::Idle,
                text: String::new(),
                status: "待命".into(),
                meta: "Alt+R 录音 / Alt+E 英文 / Alt+J 日文".into(),
                session_id: 0,
                target: None,
                overlay_rect: None,
            }),
            session_counter: AtomicU64::new(1),
        })
    }

    pub fn snapshot(&self) -> UiSnapshot {
        let inner = self.inner.lock();
        UiSnapshot {
            state: inner.state.clone(),
            text: inner.text.clone(),
            status: inner.status.clone(),
            meta: inner.meta.clone(),
            language: inner.config.asr.language.clone(),
            word_count: inner.text.chars().count(),
            overlay_rect: inner.overlay_rect,
            config: inner.config.clone(),
            history: inner.history.records().to_vec(),
        }
    }

    pub fn start_recording(&self, app: &AppHandle) -> Result<UiSnapshot> {
        if self.recorder.is_recording() {
            return self.stop_recording(app);
        }
        let session_id = self.session_counter.fetch_add(1, Ordering::Relaxed);
        let target = InputTarget::capture();
        let overlay_rect = win_bridge::overlay_position_from_rect(target.rect());
        {
            let mut inner = self.inner.lock();
            inner.session_id = session_id;
            inner.target = Some(target);
            inner.overlay_rect = Some(overlay_rect);
            inner.state = SessionState::Recording;
            inner.status = "录音中".into();
            inner.meta = "正在听写".into();
        }
        self.recorder.start(None)?;
        position_overlay(app, overlay_rect);
        emit_snapshot(app, self);
        Ok(self.snapshot())
    }

    pub fn stop_recording(&self, app: &AppHandle) -> Result<UiSnapshot> {
        let (session_id, config, base_text) = {
            let mut inner = self.inner.lock();
            if inner.state != SessionState::Recording {
                return Ok(self.snapshot());
            }
            inner.state = SessionState::Previewing;
            inner.status = "转写中".into();
            inner.meta = "正在生成文本".into();
            (inner.session_id, inner.config.clone(), inner.text.clone())
        };
        emit_snapshot(app, self);
        let recording = self.recorder.stop(config.asr.sample_rate)?;
        {
            let mut inner = self.inner.lock();
            if inner.session_id == session_id {
                inner.state = SessionState::Transcribing;
            }
        }
        if recording.duration_seconds < config.asr.min_record_seconds {
            let mut inner = self.inner.lock();
            inner.state = SessionState::Idle;
            inner.status = "录音时间过短，已忽略".into();
            inner.meta.clear();
            let _ = fs::remove_file(&recording.wav_path);
            emit_snapshot(app, self);
            return Ok(self.snapshot());
        }
        if recording.peak < 0.0015 || recording.rms < 0.0005 {
            let mut inner = self.inner.lock();
            inner.state = SessionState::Idle;
            inner.status = "没检测到有效麦克风输入".into();
            inner.text = "没检测到有效麦克风输入。\n\n建议：\n1. 检查 Windows 麦克风权限\n2. 尝试切换默认麦克风\n3. 靠近麦克风重新录音".into();
            inner.meta = format!("peak={:.5}, rms={:.5}", recording.peak, recording.rms);
            let _ = fs::remove_file(&recording.wav_path);
            emit_snapshot(app, self);
            return Ok(self.snapshot());
        }

        let state = Arc::new(self.clone_for_worker());
        let app_handle = app.clone();
        std::thread::spawn(move || {
            if let Err(err) =
                state.process_recording(&app_handle, session_id, recording, config, base_text)
            {
                state.set_error(&app_handle, session_id, err.to_string());
            }
        });
        Ok(self.snapshot())
    }

    pub fn clear(&self, app: &AppHandle) -> UiSnapshot {
        self.recorder.cancel();
        {
            let mut inner = self.inner.lock();
            inner.session_id = self.session_counter.fetch_add(1, Ordering::Relaxed);
            inner.state = SessionState::Idle;
            inner.status = "已清空".into();
            inner.meta = "当前转写任务已取消".into();
            inner.text.clear();
            inner.target = None;
        }
        emit_snapshot(app, self);
        self.snapshot()
    }

    pub fn set_text(&self, app: &AppHandle, text: String) -> UiSnapshot {
        {
            let mut inner = self.inner.lock();
            inner.text = text;
            inner.status = "等待确认".into();
        }
        emit_snapshot(app, self);
        self.snapshot()
    }

    pub fn set_runtime_notice(
        &self,
        app: &AppHandle,
        status: impl Into<String>,
        meta: impl Into<String>,
    ) -> UiSnapshot {
        {
            let mut inner = self.inner.lock();
            inner.status = status.into();
            inner.meta = meta.into();
        }
        emit_snapshot(app, self);
        self.snapshot()
    }

    pub fn confirm_input(&self, app: &AppHandle) -> Result<UiSnapshot> {
        let (target, text, delay) = {
            let inner = self.inner.lock();
            if inner.text.trim().is_empty() {
                return Err(anyhow!("没有可输入的文本"));
            }
            (
                inner.target,
                inner.text.clone(),
                inner.config.input.paste_delay_ms,
            )
        };
        let target = target.unwrap_or_else(InputTarget::capture);
        target.paste_text(&text, delay)?;
        hide_overlay(app);
        {
            let mut inner = self.inner.lock();
            inner.status = "已粘贴到当前焦点位置".into();
            inner.meta = "没有自动发送".into();
        }
        emit_snapshot(app, self);
        Ok(self.snapshot())
    }

    pub fn copy_text(&self, app: &AppHandle) -> Result<UiSnapshot> {
        let text = self.inner.lock().text.clone();
        if text.trim().is_empty() {
            return Err(anyhow!("没有可复制的文本"));
        }
        arboard::Clipboard::new()?.set_text(text)?;
        {
            let mut inner = self.inner.lock();
            inner.status = "已复制到剪贴板".into();
        }
        emit_snapshot(app, self);
        Ok(self.snapshot())
    }

    pub fn cycle_language(&self, app: &AppHandle) -> UiSnapshot {
        {
            let mut inner = self.inner.lock();
            inner.config.asr.language = match inner.config.asr.language.as_str() {
                "zh" => "en".into(),
                "en" => "ja".into(),
                _ => "zh".into(),
            };
            let _ = config::save_config(&self.paths, &inner.config);
            inner.status = format!("ASR 语言：{}", inner.config.asr.language);
        }
        emit_snapshot(app, self);
        self.snapshot()
    }

    pub fn translate_current(
        &self,
        app: &AppHandle,
        target_language: String,
    ) -> Result<UiSnapshot> {
        let (session_id, text, config) = {
            let mut inner = self.inner.lock();
            if inner.text.trim().is_empty() {
                return Err(anyhow!("没有可翻译的文本"));
            }
            let session_id = self.session_counter.fetch_add(1, Ordering::Relaxed);
            inner.session_id = session_id;
            inner.state = SessionState::Idle;
            inner.status = format!("转{}中", target_label(&target_language));
            inner.meta = format!(
                "本地翻译最多等待 {}s；原文会保留",
                inner.config.translation.timeout_seconds
            );
            (session_id, inner.text.clone(), inner.config.clone())
        };
        emit_snapshot(app, self);
        let state = Arc::new(self.clone_for_worker());
        let app_handle = app.clone();
        std::thread::spawn(move || {
            state.process_translation(&app_handle, session_id, text, target_language, config);
        });
        Ok(self.snapshot())
    }

    pub fn save_config(&self, app: &AppHandle, config_value: AppConfig) -> Result<UiSnapshot> {
        config::save_config(&self.paths, &config_value)?;
        {
            let mut inner = self.inner.lock();
            inner.config = config_value;
            inner.status = "设置已保存".into();
        }
        emit_snapshot(app, self);
        Ok(self.snapshot())
    }

    pub fn clear_history(&self, app: &AppHandle) -> Result<UiSnapshot> {
        {
            let mut inner = self.inner.lock();
            inner.history.clear(&self.paths.history_path)?;
            inner.status = "历史已清空".into();
        }
        emit_snapshot(app, self);
        Ok(self.snapshot())
    }

    pub fn download_asr_model(&self, app: &AppHandle, profile: String) -> Result<UiSnapshot> {
        let config = {
            let mut inner = self.inner.lock();
            inner.status = "模型下载中".into();
            inner.meta = format!("{profile} 档位开始下载");
            inner.config.clone()
        };
        emit_snapshot(app, self);
        let state = Arc::new(self.clone_for_worker());
        let app_handle = app.clone();
        std::thread::spawn(move || {
            let paths = state.paths.clone();
            let result = asr::download_model(&profile, &config, &paths, |message| {
                state
                    .app_state()
                    .set_runtime_notice(&app_handle, "模型下载中", message);
            });
            match result {
                Ok(()) => state.app_state().set_runtime_notice(
                    &app_handle,
                    "模型下载完成",
                    format!("{profile} 档位已就绪"),
                ),
                Err(err) => state.app_state().set_runtime_notice(
                    &app_handle,
                    "模型下载失败",
                    format!("{profile}: {err}"),
                ),
            };
        });
        Ok(self.snapshot())
    }

    fn clone_for_worker(&self) -> WorkerState {
        WorkerState {
            paths: self.paths.clone(),
            inner_ptr: self as *const AppState,
        }
    }
}

#[derive(Clone)]
struct WorkerState {
    paths: Paths,
    inner_ptr: *const AppState,
}

unsafe impl Send for WorkerState {}
unsafe impl Sync for WorkerState {}

impl WorkerState {
    fn app_state(&self) -> &AppState {
        unsafe { &*self.inner_ptr }
    }

    fn process_recording(
        &self,
        app: &AppHandle,
        session_id: u64,
        recording: audio::Recording,
        config: AppConfig,
        base_text: String,
    ) -> Result<()> {
        let started = Instant::now();
        let prompt = read_prompt(&self.paths);
        let app_state = self.app_state();
        if recording.duration_seconds >= config.asr.long_transcript_seconds as f32 {
            app_state.set_long_status(app, session_id, "长文转录中：0/0".into());
            self.save_long_recording(&recording)?;
            let chunks = asr::split_samples(
                &recording.samples,
                recording.sample_rate,
                config.asr.long_transcript_chunk_seconds,
            );
            let total = chunks.len().max(1);
            let mut texts = Vec::new();
            for (index, samples) in chunks.into_iter().enumerate() {
                if !app_state.is_current(session_id) {
                    return Ok(());
                }
                app_state.set_long_status(
                    app,
                    session_id,
                    format!("长文转录中：{}/{}", index + 1, total),
                );
                let input = AsrInput {
                    samples,
                    sample_rate: recording.sample_rate,
                    language: config.asr.language.clone(),
                    prompt: prompt.clone(),
                };
                let outcome = asr::transcribe(&input, &config, &self.paths)?;
                if !outcome.text.is_empty() {
                    texts.push(outcome.text);
                    let preview =
                        text::join_transcript_chunks(&texts, &self.paths.corrections_path);
                    app_state.set_partial_text(app, session_id, preview);
                }
            }
            let combined = text::join_transcript_chunks(&texts, &self.paths.corrections_path);
            let raw_finished = FinishedTranscript {
                session_id,
                text: combined.clone(),
                duration_seconds: recording.duration_seconds,
                transcribe_seconds: started.elapsed().as_secs_f32(),
                backend: config.asr.default_engine.clone(),
                model: config.asr.profile.clone(),
            };
            app_state.finish_transcription(app, raw_finished)?;
            if combined.trim().is_empty() {
                return Ok(());
            }
            if !app_state.is_current(session_id) {
                return Ok(());
            }
            app_state.set_postprocess_status(app, session_id, "智能纠错中".into());
            let final_text =
                llm::smart_correct(&combined, &base_text, &config, &self.paths, &prompt);
            app_state.update_finished_text(
                app,
                session_id,
                final_text,
                recording.duration_seconds,
                started.elapsed().as_secs_f32(),
            )?;
        } else {
            let input = AsrInput {
                samples: recording.samples,
                sample_rate: recording.sample_rate,
                language: config.asr.language.clone(),
                prompt: prompt.clone(),
            };
            let outcome = asr::transcribe(&input, &config, &self.paths)?;
            let raw_text = outcome.text.clone();
            let raw_finished = FinishedTranscript {
                session_id,
                text: raw_text.clone(),
                duration_seconds: recording.duration_seconds,
                transcribe_seconds: outcome.elapsed_seconds,
                backend: outcome.backend,
                model: outcome.model,
            };
            app_state.finish_transcription(app, raw_finished)?;
            if raw_text.trim().is_empty() {
                return Ok(());
            }
            if !app_state.is_current(session_id) {
                return Ok(());
            }
            app_state.set_postprocess_status(app, session_id, "智能纠错中".into());
            let final_text =
                llm::smart_correct(&raw_text, &base_text, &config, &self.paths, &prompt);
            app_state.update_finished_text(
                app,
                session_id,
                final_text,
                recording.duration_seconds,
                started.elapsed().as_secs_f32(),
            )?;
        }
        let _ = fs::remove_file(recording.wav_path);
        Ok(())
    }

    fn save_long_recording(&self, recording: &audio::Recording) -> Result<PathBuf> {
        fs::create_dir_all(&self.paths.recordings_dir)?;
        let filename = format!(
            "{}_{:.1}s.wav",
            chrono::Local::now().format("%Y%m%d_%H%M%S"),
            recording.duration_seconds
        )
        .replace('.', "_");
        let target = self.paths.recordings_dir.join(filename);
        fs::copy(&recording.wav_path, &target)?;
        Ok(target)
    }

    fn process_translation(
        &self,
        app: &AppHandle,
        session_id: u64,
        source: String,
        target_language: String,
        config: AppConfig,
    ) {
        let started = Instant::now();
        let prompt = read_prompt(&self.paths);
        let result = llm::translate(&source, &target_language, &config, &self.paths, &prompt);
        let elapsed = started.elapsed().as_secs_f32();
        let state = self.app_state();
        {
            let mut inner = state.inner.lock();
            if inner.session_id != session_id {
                return;
            }
            inner.state = SessionState::Idle;
            match result {
                Ok(translated) => {
                    inner.text = translated;
                    inner.status = format!("已转为{}", target_label(&target_language));
                    inner.meta =
                        format!("翻译 {:.1}s / {} 字", elapsed, inner.text.chars().count());
                }
                Err(err) => {
                    inner.status = "翻译未完成".into();
                    inner.meta = format!("{err}；耗时 {:.1}s，原文已保留", elapsed);
                }
            }
        }
        emit_snapshot(app, state);
    }

    fn set_error(&self, app: &AppHandle, session_id: u64, message: String) {
        let state = self.app_state();
        {
            let mut inner = state.inner.lock();
            if inner.session_id != session_id {
                return;
            }
            inner.state = SessionState::Error;
            inner.status = "出错".into();
            inner.text = message;
        }
        emit_snapshot(app, state);
    }
}

impl AppState {
    fn is_current(&self, session_id: u64) -> bool {
        self.inner.lock().session_id == session_id
    }

    fn set_long_status(&self, app: &AppHandle, session_id: u64, status: String) {
        {
            let mut inner = self.inner.lock();
            if inner.session_id != session_id {
                return;
            }
            inner.state = SessionState::LongTranscribing;
            inner.status = status;
            inner.meta = "长文会分片处理".into();
        }
        emit_snapshot(app, self);
    }

    fn set_partial_text(&self, app: &AppHandle, session_id: u64, text: String) {
        {
            let mut inner = self.inner.lock();
            if inner.session_id != session_id {
                return;
            }
            inner.text = text;
            inner.status = "实时预览".into();
        }
        emit_snapshot(app, self);
    }

    fn set_postprocess_status(&self, app: &AppHandle, session_id: u64, status: String) {
        {
            let mut inner = self.inner.lock();
            if inner.session_id != session_id {
                return;
            }
            inner.state = SessionState::Idle;
            inner.status = status;
            inner.meta = "ASR 原文已显示，可直接确认".into();
        }
        emit_snapshot(app, self);
    }

    fn finish_transcription(&self, app: &AppHandle, finished: FinishedTranscript) -> Result<()> {
        {
            let mut inner = self.inner.lock();
            if inner.session_id != finished.session_id {
                return Ok(());
            }
            if finished.text.trim().is_empty() {
                inner.state = SessionState::Idle;
                inner.status = "未识别到有效语音".into();
                inner.meta.clear();
            } else {
                inner.state = SessionState::Idle;
                inner.status = "等待确认".into();
                inner.meta = format!(
                    "录音 {:.1}s / 转写 {:.1}s / {} 字",
                    finished.duration_seconds,
                    finished.transcribe_seconds,
                    finished.text.chars().count()
                );
                inner.text = finished.text.clone();
                let record = TranscriptRecord::new(
                    finished.text,
                    finished.duration_seconds,
                    finished.transcribe_seconds,
                    finished.backend,
                    finished.model,
                );
                let history_path = self.paths.history_path.clone();
                inner.history.add(record, &history_path)?;
            }
        }
        emit_snapshot(app, self);
        Ok(())
    }

    fn update_finished_text(
        &self,
        app: &AppHandle,
        session_id: u64,
        text: String,
        duration_seconds: f32,
        transcribe_seconds: f32,
    ) -> Result<()> {
        {
            let mut inner = self.inner.lock();
            if inner.session_id != session_id || text.trim().is_empty() {
                return Ok(());
            }
            inner.state = SessionState::Idle;
            inner.status = "等待确认".into();
            inner.meta = format!(
                "录音 {:.1}s / 处理 {:.1}s / {} 字",
                duration_seconds,
                transcribe_seconds,
                text.chars().count()
            );
            inner.text = text;
        }
        emit_snapshot(app, self);
        Ok(())
    }
}

struct FinishedTranscript {
    session_id: u64,
    text: String,
    duration_seconds: f32,
    transcribe_seconds: f32,
    backend: String,
    model: String,
}

pub fn emit_snapshot(app: &AppHandle, state: &AppState) {
    let snapshot = state.snapshot();
    let _ = app.emit("voice-ime://snapshot", snapshot);
}

pub fn position_overlay(app: &AppHandle, rect: OverlayRect) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.set_position(tauri::PhysicalPosition::new(rect.x, rect.y));
        let _ = window.set_size(tauri::PhysicalSize::new(
            rect.width as u32,
            rect.height as u32,
        ));
        let _ = window.show();
    }
}

pub fn hide_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
}

fn read_prompt(paths: &Paths) -> String {
    fs::read_to_string(&paths.prompt_path).unwrap_or_default()
}

fn target_label(language: &str) -> &'static str {
    match language {
        "en" => "英语",
        "ja" => "日语",
        "zh" => "中文",
        _ => "目标语言",
    }
}
