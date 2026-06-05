use crate::{
    asr::{self, AsrInput},
    audio::{self, Recorder},
    config::{self, AppConfig, Paths},
    history::{HistoryStore, TranscriptRecord},
    llm, text, translation,
    win_bridge::{self, InputTarget, InputTargetInfo, OverlayRect},
};
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::{
    any::Any,
    fs::{self, OpenOptions},
    io::Write,
    ops::Deref,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Recording,
    Previewing,
    Transcribing,
    LongTranscribing,
    Cancelling,
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

#[derive(Clone)]
pub struct AppState {
    runtime: Arc<AppRuntime>,
}

pub struct AppRuntime {
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
        Ok(Self::from_parts(paths, config, history))
    }

    fn from_parts(paths: Paths, config: AppConfig, history: HistoryStore) -> Self {
        Self {
            runtime: Arc::new(AppRuntime {
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
            }),
        }
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
            return Ok(self.snapshot());
        }
        let config = self.inner.lock().config.clone();
        self.recorder
            .start(configured_input_device(&config))
            .map_err(|err| anyhow!("麦克风启动失败：{err}"))?;
        let session_id = self.next_session_id();
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
            if !accepts_worker_update(inner.session_id, &inner.state, session_id) {
                let _ = fs::remove_file(&recording.wav_path);
                return Ok(self.snapshot());
            }
            inner.state = SessionState::Transcribing;
        }
        if recording.duration_seconds < config.asr.min_record_seconds {
            let mut inner = self.inner.lock();
            if !accepts_worker_update(inner.session_id, &inner.state, session_id) {
                let _ = fs::remove_file(&recording.wav_path);
                return Ok(self.snapshot());
            }
            inner.state = SessionState::Idle;
            inner.status = "录音时间过短，已忽略".into();
            inner.meta.clear();
            let _ = fs::remove_file(&recording.wav_path);
            emit_snapshot(app, self);
            return Ok(self.snapshot());
        }
        if recording.peak < 0.0015 || recording.rms < 0.0005 {
            let mut inner = self.inner.lock();
            if !accepts_worker_update(inner.session_id, &inner.state, session_id) {
                let _ = fs::remove_file(&recording.wav_path);
                return Ok(self.snapshot());
            }
            inner.state = SessionState::Idle;
            inner.status = "没检测到有效麦克风输入".into();
            inner.text = "没检测到有效麦克风输入。\n\n建议：\n1. 检查 Windows 麦克风权限\n2. 尝试切换默认麦克风\n3. 靠近麦克风重新录音".into();
            inner.meta = format!("peak={:.5}, rms={:.5}", recording.peak, recording.rms);
            let _ = fs::remove_file(&recording.wav_path);
            emit_snapshot(app, self);
            return Ok(self.snapshot());
        }

        let state = self.clone_for_worker();
        let app_handle = app.clone();
        std::thread::spawn(move || {
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                state.process_recording(&app_handle, session_id, recording, config, base_text)
            }));
            match result {
                Ok(Ok(())) => {}
                Ok(Err(err)) => state.set_error(&app_handle, session_id, err.to_string()),
                Err(payload) => state.set_worker_panic_error(
                    &app_handle,
                    Some(session_id),
                    "recording",
                    payload,
                ),
            }
        });
        Ok(self.snapshot())
    }

    pub fn clear(&self, app: &AppHandle) -> UiSnapshot {
        let recorder_was_active = self.recorder.is_recording();
        self.recorder.cancel();
        let cancel_session_id = self.next_session_id();
        let should_transition = {
            let mut inner = self.inner.lock();
            let had_active_task = recorder_was_active || is_busy_state(&inner.state);
            inner.session_id = cancel_session_id;
            inner.state = if had_active_task {
                SessionState::Cancelling
            } else {
                SessionState::Idle
            };
            inner.status = if had_active_task {
                "取消中".into()
            } else {
                "已清空".into()
            };
            inner.meta = if had_active_task {
                "旧任务结果会被忽略".into()
            } else {
                "当前文本已清空".into()
            };
            inner.text.clear();
            inner.target = None;
            inner.overlay_rect = None;
            had_active_task
        };
        hide_overlay(app);
        emit_snapshot(app, self);
        if should_transition {
            finish_cancelling_async(app.clone(), self.clone_for_worker(), cancel_session_id);
        }
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

    pub(crate) fn report_worker_panic(
        &self,
        app: &AppHandle,
        worker: &'static str,
        session_id: Option<u64>,
        payload: Box<dyn Any + Send>,
    ) {
        let message = panic_payload_message(payload.as_ref());
        let _ = self.write_worker_error_log(worker, session_id, &message);
        if let Some(session_id) = session_id {
            let mut inner = self.inner.lock();
            if accepts_worker_update(inner.session_id, &inner.state, session_id) {
                inner.state = SessionState::Error;
                inner.status = "后台任务异常".into();
                inner.text = format!("{worker} panic: {message}");
                inner.meta = "已写入 worker-error 日志".into();
                drop(inner);
                emit_snapshot(app, self);
            }
            return;
        }
        self.set_runtime_notice(
            app,
            "后台任务异常",
            format!("{worker} panic；已写入 worker-error 日志"),
        );
    }

    pub fn confirm_input(&self, app: &AppHandle) -> Result<UiSnapshot> {
        let (target, text, input_config) = {
            let inner = self.inner.lock();
            if inner.text.trim().is_empty() {
                return Err(anyhow!("没有可输入的文本"));
            }
            (
                inner.target.clone(),
                inner.text.clone(),
                inner.config.input.clone(),
            )
        };
        let target = target.unwrap_or_else(InputTarget::capture);
        let target_info = target.info().clone();
        let profile = config::matching_app_profile(
            &input_config.app_profiles,
            &target_info.process_name,
            &target_info.class_name,
            &target_info.title,
        );
        let profile_name = profile
            .map(|profile| profile.name.clone())
            .filter(|name| !name.trim().is_empty());
        let delay = profile
            .and_then(|profile| profile.paste_delay_ms)
            .unwrap_or(input_config.paste_delay_ms);
        let punctuation_policy = profile
            .map(|profile| profile.punctuation.as_str())
            .unwrap_or("default");
        let paste_result = target.paste_text(&text, delay);
        let paste_outcome = paste_result.as_ref().ok();
        let error = paste_result.as_ref().err().map(ToString::to_string);
        let log_entry = InputTargetLogEntry {
            created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            action: "confirm_input",
            input_profile: profile_name.as_deref(),
            punctuation_policy,
            text_chars: text.chars().count(),
            paste_delay_ms: delay,
            input_method: paste_outcome.map(|outcome| outcome.method),
            send_input_events: paste_outcome.map(|outcome| outcome.send_input_events),
            focus_attempts: paste_outcome.map(|outcome| outcome.focus_attempts),
            focus_restored: paste_outcome.map(|outcome| outcome.focus_restored),
            clipboard_previous_had_text: paste_outcome
                .map(|outcome| outcome.clipboard_previous_had_text),
            clipboard_previous_format: paste_outcome
                .map(|outcome| outcome.clipboard_previous_format),
            clipboard_format_count: paste_outcome.map(|outcome| outcome.clipboard_format_count),
            clipboard_sequence_before: paste_outcome
                .map(|outcome| outcome.clipboard_sequence_before),
            clipboard_sequence_after: paste_outcome.map(|outcome| outcome.clipboard_sequence_after),
            clipboard_restored: paste_outcome.map(|outcome| outcome.clipboard_restored),
            clipboard_restore_error: paste_outcome
                .and_then(|outcome| outcome.clipboard_restore_error.as_deref()),
            result: if paste_result.is_ok() { "ok" } else { "error" },
            error: error.as_deref(),
            target: &target_info,
        };
        let _ = self.write_input_target_log(&log_entry);
        paste_result?;
        let hide_session_id = {
            let inner = self.inner.lock();
            inner.session_id
        };
        {
            let mut inner = self.inner.lock();
            inner.status = "已粘贴".into();
            inner.meta = profile_name
                .map(|name| format!("没有自动发送 / {name}"))
                .unwrap_or_else(|| "没有自动发送".into());
        }
        emit_snapshot(app, self);
        hide_overlay_after(
            app.clone(),
            self.clone_for_worker(),
            hide_session_id,
            Duration::from_millis(650),
        );
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
            let session_id = self.next_session_id();
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
        let state = self.clone_for_worker();
        let app_handle = app.clone();
        std::thread::spawn(move || {
            if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| {
                state.process_translation(&app_handle, session_id, text, target_language, config);
            })) {
                state.set_worker_panic_error(&app_handle, Some(session_id), "translation", payload);
            }
        });
        Ok(self.snapshot())
    }

    pub fn save_config(&self, app: &AppHandle, config_value: AppConfig) -> Result<UiSnapshot> {
        let config_value = config::normalized(config_value);
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

    fn write_input_target_log(&self, entry: &InputTargetLogEntry<'_>) -> Result<()> {
        fs::create_dir_all(&self.paths.logs_dir)?;
        let path = self.paths.logs_dir.join(format!(
            "input-target-{}.log",
            chrono::Local::now().format("%Y%m%d")
        ));
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", serde_json::to_string(entry)?)?;
        Ok(())
    }

    fn write_worker_error_log(
        &self,
        worker: &'static str,
        session_id: Option<u64>,
        message: &str,
    ) -> Result<()> {
        fs::create_dir_all(&self.paths.logs_dir)?;
        let path = self.paths.logs_dir.join(format!(
            "worker-error-{}.log",
            chrono::Local::now().format("%Y%m%d")
        ));
        let entry = WorkerErrorLogEntry {
            created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            worker,
            session_id,
            message,
        };
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", serde_json::to_string(&entry)?)?;
        Ok(())
    }

    pub fn download_asr_model(&self, app: &AppHandle, profile: String) -> Result<UiSnapshot> {
        let config = {
            let mut inner = self.inner.lock();
            inner.status = "模型下载中".into();
            inner.meta = format!("{profile} 档位开始下载");
            inner.config.clone()
        };
        emit_snapshot(app, self);
        let state = self.clone_for_worker();
        let app_handle = app.clone();
        std::thread::spawn(move || {
            if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| {
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
            })) {
                state.set_worker_panic_error(&app_handle, None, "model-download", payload);
            }
        });
        Ok(self.snapshot())
    }

    fn clone_for_worker(&self) -> WorkerState {
        WorkerState {
            paths: self.paths.clone(),
            app_state: self.clone(),
        }
    }

    fn next_session_id(&self) -> u64 {
        self.session_counter.fetch_add(1, Ordering::Relaxed)
    }
}

impl Deref for AppState {
    type Target = AppRuntime;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

#[derive(Clone)]
struct WorkerState {
    paths: Paths,
    app_state: AppState,
}

impl WorkerState {
    fn app_state(&self) -> &AppState {
        &self.app_state
    }

    fn set_worker_panic_error(
        &self,
        app: &AppHandle,
        session_id: Option<u64>,
        worker: &'static str,
        payload: Box<dyn Any + Send>,
    ) {
        self.app_state()
            .report_worker_panic(app, worker, session_id, payload);
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
            if config.asr.save_long_recordings {
                self.save_long_recording(&recording)?;
            }
            let chunks = asr::split_samples(
                &recording.samples,
                recording.sample_rate,
                config.asr.long_transcript_chunk_seconds,
            );
            let total = chunks.len().max(1);
            let mut texts = Vec::new();
            let mut raw_texts = Vec::new();
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
                    if !outcome.raw_text.trim().is_empty() {
                        raw_texts.push(outcome.raw_text);
                    }
                    texts.push(outcome.text);
                    let preview =
                        text::join_transcript_chunks(&texts, &self.paths.corrections_path);
                    app_state.set_partial_text(app, session_id, preview);
                }
            }
            let combined = text::join_transcript_chunks(&texts, &self.paths.corrections_path);
            let raw_combined = {
                let joined = join_raw_transcript_chunks(&raw_texts);
                if joined.trim().is_empty() {
                    combined.clone()
                } else {
                    joined
                }
            };
            let raw_finished = FinishedTranscript {
                session_id,
                raw_text: raw_combined,
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
            let correction_started = Instant::now();
            let final_text =
                llm::smart_correct(&combined, &base_text, &config, &self.paths, &prompt);
            let llm_seconds = correction_started.elapsed().as_secs_f32();
            app_state.update_finished_text(
                app,
                session_id,
                final_text,
                recording.duration_seconds,
                started.elapsed().as_secs_f32(),
                llm_seconds,
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
                raw_text: if outcome.raw_text.trim().is_empty() {
                    raw_text.clone()
                } else {
                    outcome.raw_text
                },
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
            let correction_started = Instant::now();
            let final_text =
                llm::smart_correct(&raw_text, &base_text, &config, &self.paths, &prompt);
            let llm_seconds = correction_started.elapsed().as_secs_f32();
            app_state.update_finished_text(
                app,
                session_id,
                final_text,
                recording.duration_seconds,
                started.elapsed().as_secs_f32(),
                llm_seconds,
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
        let result =
            translation::translate(&source, &target_language, &config, &self.paths, &prompt);
        let elapsed = started.elapsed().as_secs_f32();
        let state = self.app_state();
        {
            let mut inner = state.inner.lock();
            if !accepts_worker_update(inner.session_id, &inner.state, session_id) {
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
            if !accepts_worker_update(inner.session_id, &inner.state, session_id) {
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
        let inner = self.inner.lock();
        accepts_worker_update(inner.session_id, &inner.state, session_id)
    }

    fn set_long_status(&self, app: &AppHandle, session_id: u64, status: String) {
        {
            let mut inner = self.inner.lock();
            if !accepts_worker_update(inner.session_id, &inner.state, session_id) {
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
            if !accepts_worker_update(inner.session_id, &inner.state, session_id) {
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
            if !accepts_worker_update(inner.session_id, &inner.state, session_id) {
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
            if !accepts_worker_update(inner.session_id, &inner.state, finished.session_id) {
                return Ok(());
            }
            let deterministic_started = Instant::now();
            let trace = text::correction_trace(&finished.raw_text, &self.paths.corrections_path);
            let deterministic_seconds = deterministic_started.elapsed().as_secs_f32();
            let punctuation_policy = current_punctuation_policy(&inner);
            let final_text =
                text::apply_punctuation_policy(&finished.text, punctuation_policy.as_str());
            if final_text.trim().is_empty() {
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
                    final_text.chars().count()
                );
                inner.text = final_text.clone();
                let record = TranscriptRecord::new(
                    finished.session_id,
                    final_text,
                    trace.raw_text,
                    trace.normalized_text,
                    trace.dictionary_text,
                    trace.hotword_text,
                    trace.rule_text,
                    trace.itn_text,
                    punctuation_policy,
                    finished.duration_seconds,
                    finished.transcribe_seconds,
                    deterministic_seconds,
                    finished.transcribe_seconds + deterministic_seconds,
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
        total_seconds: f32,
        llm_seconds: f32,
    ) -> Result<()> {
        {
            let mut inner = self.inner.lock();
            if !accepts_worker_update(inner.session_id, &inner.state, session_id)
                || text.trim().is_empty()
            {
                return Ok(());
            }
            let text =
                text::apply_punctuation_policy(&text, current_punctuation_policy(&inner).as_str());
            inner.state = SessionState::Idle;
            inner.status = "等待确认".into();
            inner.meta = format!(
                "录音 {:.1}s / 总耗时 {:.1}s / LLM {:.1}s / {} 字",
                duration_seconds,
                total_seconds,
                llm_seconds,
                text.chars().count()
            );
            inner.text = text;
            let history_path = self.paths.history_path.clone();
            let final_text = inner.text.clone();
            inner.history.update_postprocess(
                session_id,
                final_text,
                llm_seconds,
                total_seconds,
                &history_path,
            )?;
        }
        emit_snapshot(app, self);
        Ok(())
    }
}

struct FinishedTranscript {
    session_id: u64,
    raw_text: String,
    text: String,
    duration_seconds: f32,
    transcribe_seconds: f32,
    backend: String,
    model: String,
}

#[derive(Serialize)]
struct InputTargetLogEntry<'a> {
    created_at: String,
    action: &'static str,
    input_profile: Option<&'a str>,
    punctuation_policy: &'a str,
    text_chars: usize,
    paste_delay_ms: u64,
    input_method: Option<&'a str>,
    send_input_events: Option<u32>,
    focus_attempts: Option<u32>,
    focus_restored: Option<bool>,
    clipboard_previous_had_text: Option<bool>,
    clipboard_previous_format: Option<&'a str>,
    clipboard_format_count: Option<u32>,
    clipboard_sequence_before: Option<u32>,
    clipboard_sequence_after: Option<u32>,
    clipboard_restored: Option<bool>,
    clipboard_restore_error: Option<&'a str>,
    result: &'a str,
    error: Option<&'a str>,
    target: &'a InputTargetInfo,
}

#[derive(Serialize)]
struct WorkerErrorLogEntry<'a> {
    created_at: String,
    worker: &'static str,
    session_id: Option<u64>,
    message: &'a str,
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

fn hide_overlay_after(app: AppHandle, worker: WorkerState, session_id: u64, delay: Duration) {
    std::thread::spawn(move || {
        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| {
            std::thread::sleep(delay);
            let state = worker.app_state();
            {
                let inner = state.inner.lock();
                if inner.session_id != session_id || inner.state != SessionState::Idle {
                    return;
                }
            }
            hide_overlay(&app);
        })) {
            worker.set_worker_panic_error(&app, Some(session_id), "overlay-hide", payload);
        }
    });
}

fn read_prompt(paths: &Paths) -> String {
    fs::read_to_string(&paths.prompt_path).unwrap_or_default()
}

fn finish_cancelling_async(app: AppHandle, worker: WorkerState, cancel_session_id: u64) {
    std::thread::spawn(move || {
        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| {
            std::thread::sleep(Duration::from_millis(120));
            let state = worker.app_state();
            {
                let mut inner = state.inner.lock();
                if inner.session_id != cancel_session_id || inner.state != SessionState::Cancelling
                {
                    return;
                }
                inner.state = SessionState::Idle;
                inner.status = "已清空".into();
                inner.meta = "旧任务结果会被忽略".into();
            }
            emit_snapshot(&app, state);
        })) {
            worker.set_worker_panic_error(&app, Some(cancel_session_id), "cancelling", payload);
        }
    });
}

fn accepts_worker_update(
    current_session_id: u64,
    current_state: &SessionState,
    incoming: u64,
) -> bool {
    current_session_id == incoming && *current_state != SessionState::Cancelling
}

fn is_busy_state(state: &SessionState) -> bool {
    matches!(
        state,
        SessionState::Recording
            | SessionState::Previewing
            | SessionState::Transcribing
            | SessionState::LongTranscribing
            | SessionState::Cancelling
    )
}

fn join_raw_transcript_chunks(chunks: &[String]) -> String {
    let mut result = String::new();
    for chunk in chunks {
        let chunk = text::normalize_text(chunk);
        if chunk.is_empty() {
            continue;
        }
        if result
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphanumeric())
            && chunk
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric())
        {
            result.push(' ');
        }
        result.push_str(&chunk);
    }
    text::normalize_text(&result)
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return (*message).to_string();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "unknown panic payload".into()
}

fn current_punctuation_policy(inner: &InnerState) -> String {
    let Some(target) = inner.target.as_ref() else {
        return "default".into();
    };
    let info = target.info();
    config::matching_app_profile(
        &inner.config.input.app_profiles,
        &info.process_name,
        &info.class_name,
        &info.title,
    )
    .map(|profile| profile.punctuation.clone())
    .unwrap_or_else(|| "default".into())
}

fn configured_input_device(config: &AppConfig) -> Option<&str> {
    let device = config.asr.input_device_name.trim();
    if device.is_empty() {
        None
    } else {
        Some(device)
    }
}

fn target_label(language: &str) -> &'static str {
    match language {
        "en" => "英语",
        "ja" => "日语",
        "zh" => "中文",
        _ => "目标语言",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn cancelling_rejects_stale_worker_updates() {
        assert!(accepts_worker_update(7, &SessionState::Idle, 7));
        assert!(!accepts_worker_update(7, &SessionState::Idle, 8));
        assert!(!accepts_worker_update(7, &SessionState::Cancelling, 7));
    }

    #[test]
    fn busy_state_includes_cancelling_and_worker_states() {
        for state in [
            SessionState::Recording,
            SessionState::Previewing,
            SessionState::Transcribing,
            SessionState::LongTranscribing,
            SessionState::Cancelling,
        ] {
            assert!(is_busy_state(&state));
        }
        assert!(!is_busy_state(&SessionState::Idle));
        assert!(!is_busy_state(&SessionState::Error));
    }

    #[test]
    fn configured_input_device_uses_empty_as_default() {
        let mut config = AppConfig::default();
        assert_eq!(configured_input_device(&config), None);

        config.asr.input_device_name = "USB Mic".into();
        assert_eq!(configured_input_device(&config), Some("USB Mic"));
    }

    #[test]
    fn cloned_app_state_shares_runtime_and_session_counter() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let history = HistoryStore::load(&paths.history_path, 10);
        let state = AppState::from_parts(paths, AppConfig::default(), history);
        let cloned = state.clone();

        assert!(Arc::ptr_eq(&state.runtime, &cloned.runtime));
        assert_eq!(state.next_session_id(), 1);
        assert_eq!(cloned.next_session_id(), 2);

        {
            let mut inner = state.inner.lock();
            inner.status = "共享运行时".into();
        }
        assert_eq!(cloned.snapshot().status, "共享运行时");
    }

    #[test]
    fn panic_payload_message_reads_common_payloads() {
        let static_message: Box<dyn Any + Send> = Box::new("static panic");
        assert_eq!(
            panic_payload_message(static_message.as_ref()),
            "static panic"
        );

        let owned_message: Box<dyn Any + Send> = Box::new(String::from("owned panic"));
        assert_eq!(panic_payload_message(owned_message.as_ref()), "owned panic");

        let unknown: Box<dyn Any + Send> = Box::new(42_u32);
        assert_eq!(
            panic_payload_message(unknown.as_ref()),
            "unknown panic payload"
        );
    }

    fn test_paths(root: &Path) -> Paths {
        let app_dir = root.join(".voice_ime");
        Paths {
            config_path: app_dir.join("config.json"),
            history_path: app_dir.join("history.json"),
            prompt_path: app_dir.join("personal_prompt.txt"),
            corrections_path: app_dir.join("corrections.json"),
            hotwords_path: app_dir.join("hot.txt"),
            hot_rules_path: app_dir.join("hot-rule.txt"),
            recordings_dir: app_dir.join("recordings"),
            logs_dir: app_dir.join("logs"),
            root_dir: PathBuf::from(root),
            app_dir,
        }
    }
}
