use crate::config::{self, AppConfig, Paths};
use crate::text;
use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use serde::Serialize;
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
    OfflineWhisperModelConfig, OfflineZipformerCtcModelConfig,
};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Instant;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static ASR_DAEMON: Lazy<parking_lot::Mutex<Option<AsrDaemonProcess>>> =
    Lazy::new(|| parking_lot::Mutex::new(None));

#[derive(Debug, Clone, Serialize)]
pub struct AsrModelStatus {
    pub engine: String,
    pub profile: String,
    pub description: String,
    pub expected_latency: String,
    pub ready: bool,
    pub download_url: String,
    pub mirror_url: String,
    pub target_dir: String,
    pub required_files: Vec<String>,
    pub missing_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct AsrOutcome {
    #[serde(default)]
    pub raw_text: String,
    pub text: String,
    pub backend: String,
    pub model: String,
    pub elapsed_seconds: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AsrPrewarmStatus {
    pub profile: String,
    pub backend: String,
    pub model: String,
    pub elapsed_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct AsrInput {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub language: String,
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub struct ModelDownloadFile {
    pub urls: Vec<String>,
    pub filename: String,
}

#[derive(Debug, Clone)]
pub struct ModelDownloadSpec {
    pub profile: String,
    pub target_dir: PathBuf,
    pub files: Vec<ModelDownloadFile>,
}

pub fn transcribe(input: &AsrInput, config: &AppConfig, paths: &Paths) -> Result<AsrOutcome> {
    let started = Instant::now();
    if is_mock_engine(&config.asr.default_engine) {
        let mut outcome = transcribe_mock(input, &config.asr.profile);
        outcome.elapsed_seconds = started.elapsed().as_secs_f32();
        return Ok(outcome);
    }
    let profiles = profile_order(&config.asr.profile);
    let mut last_error: Option<anyhow::Error> = None;
    for profile in profiles {
        match transcribe_profile_with_configured_worker(input, config, paths, profile) {
            Ok(mut outcome) => {
                outcome.elapsed_seconds = started.elapsed().as_secs_f32();
                return Ok(outcome);
            }
            Err(err) => last_error = Some(err),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("没有可用 ASR 后端")))
}

pub fn prewarm(config: &AppConfig, paths: &Paths) -> Result<AsrPrewarmStatus> {
    if config.asr.worker_mode != "persistent" {
        return Err(anyhow!("ASR 进程不是常驻模式"));
    }
    if is_mock_engine(&config.asr.default_engine) {
        return Ok(AsrPrewarmStatus {
            profile: config.asr.profile.clone(),
            backend: "mock-asr".into(),
            model: format!("mock/{}", config.asr.profile),
            elapsed_seconds: 0.0,
        });
    }
    let profile = prewarm_profile(config, paths)
        .ok_or_else(|| anyhow!("没有可预热的 ASR 模型，请先下载模型"))?;
    let started = Instant::now();
    let sample_rate = config.asr.sample_rate;
    let samples = vec![0.0; (sample_rate as usize / 10).max(800)];
    let input = AsrInput {
        samples,
        sample_rate,
        language: config.asr.language.clone(),
        prompt: String::new(),
    };
    let outcome = transcribe_profile_in_daemon(&input, config, paths, &profile)?;
    Ok(AsrPrewarmStatus {
        profile,
        backend: outcome.backend,
        model: outcome.model,
        elapsed_seconds: started.elapsed().as_secs_f32(),
    })
}

fn prewarm_profile(config: &AppConfig, paths: &Paths) -> Option<String> {
    profile_order(&config.asr.profile)
        .into_iter()
        .find(|profile| {
            let files = required_files_for_profile(config, paths, profile);
            !files.is_empty() && files.iter().all(|path| Path::new(path).is_file())
        })
        .map(str::to_string)
}

fn transcribe_profile_with_configured_worker(
    input: &AsrInput,
    config: &AppConfig,
    paths: &Paths,
    profile: &str,
) -> Result<AsrOutcome> {
    if config.asr.worker_mode == "persistent" {
        match transcribe_profile_in_daemon(input, config, paths, profile) {
            Ok(outcome) => return Ok(outcome),
            Err(daemon_err) => {
                let isolated = transcribe_profile_in_worker(input, config, paths, profile);
                return isolated.map_err(|worker_err| {
                    anyhow!("隔离 worker 也失败：{worker_err}；常驻 worker 失败：{daemon_err}")
                });
            }
        }
    }
    transcribe_profile_in_worker(input, config, paths, profile)
}

pub fn run_worker_cli_if_requested() -> bool {
    let mut args = env::args_os().skip(1);
    let Some(mode) = args.next() else {
        return false;
    };
    if mode == "--asr-daemon" {
        if let Err(err) = run_asr_daemon_cli() {
            eprintln!("{err:?}");
            process::exit(2);
        }
        return true;
    }
    if mode != "--asr-worker" {
        return false;
    }
    let result = (|| -> Result<()> {
        let request_path = args
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("缺少 ASR worker request 路径"))?;
        let request: AsrWorkerRequest =
            serde_json::from_slice(&fs::read(&request_path).context("读取 ASR worker request")?)?;
        let (sample_rate, samples) = read_wav_file(&request.wav_path)?;
        let paths = Paths {
            root_dir: request.root_dir.clone(),
            app_dir: request.app_dir.clone(),
            model_dir: request.model_dir.clone(),
            config_path: request.app_dir.join("config.json"),
            history_path: request.app_dir.join("history.json"),
            prompt_path: request.app_dir.join("personal_prompt.txt"),
            corrections_path: request.app_dir.join("corrections.json"),
            hotwords_path: request.app_dir.join("hot.txt"),
            hot_rules_path: request.app_dir.join("hot-rule.txt"),
            recordings_dir: request.app_dir.join("recordings"),
            logs_dir: request.app_dir.join("logs"),
        };
        let input = AsrInput {
            samples,
            sample_rate,
            language: request.language,
            prompt: request.prompt,
        };
        let started = Instant::now();
        let mut outcome = transcribe_profile(&input, &request.config, &paths, &request.profile)?;
        if outcome.raw_text.is_empty() {
            outcome.raw_text = outcome.text.clone();
        }
        outcome.elapsed_seconds = started.elapsed().as_secs_f32();
        outcome.text = text::clean_asr_text(&outcome.text, &paths.corrections_path);
        fs::write(
            &request.output_path,
            serde_json::to_vec_pretty(&outcome).context("序列化 ASR worker output")?,
        )?;
        Ok(())
    })();
    if let Err(err) = result {
        eprintln!("{err:?}");
        process::exit(2);
    }
    true
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AsrWorkerRequest {
    wav_path: PathBuf,
    output_path: PathBuf,
    root_dir: PathBuf,
    app_dir: PathBuf,
    model_dir: PathBuf,
    config: AppConfig,
    profile: String,
    language: String,
    prompt: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AsrDaemonRequest {
    id: u64,
    request: AsrWorkerRequest,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AsrDaemonResponse {
    id: u64,
    outcome: Option<AsrOutcome>,
    error: Option<String>,
}

struct AsrDaemonProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl AsrDaemonProcess {
    fn spawn(paths: &Paths) -> Result<Self> {
        let exe = env::current_exe().context("定位 ASR daemon exe 失败")?;
        let mut command = Command::new(exe);
        command
            .arg("--asr-daemon")
            .current_dir(&paths.root_dir)
            .env("VOICE_IME_ROOT", &paths.root_dir)
            .env("VOICE_IME_APP_DIR", &paths.app_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(target_os = "windows")]
        {
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().context("启动 ASR 常驻子进程失败")?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("ASR 常驻子进程 stdin 不可用"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("ASR 常驻子进程 stdout 不可用"))?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }

    fn transcribe(&mut self, request: AsrWorkerRequest) -> Result<AsrOutcome> {
        if let Some(status) = self.child.try_wait()? {
            return Err(anyhow!("ASR 常驻子进程已退出：{status}"));
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let request = AsrDaemonRequest { id, request };
        serde_json::to_writer(&mut self.stdin, &request).context("写入 ASR daemon request")?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;

        let mut line = String::new();
        let bytes = self.stdout.read_line(&mut line)?;
        if bytes == 0 {
            return Err(anyhow!("ASR 常驻子进程没有返回结果"));
        }
        let response: AsrDaemonResponse =
            serde_json::from_str(&line).context("解析 ASR daemon response")?;
        if response.id != id {
            return Err(anyhow!(
                "ASR daemon response id 不匹配：期望 {id}，收到 {}",
                response.id
            ));
        }
        if let Some(outcome) = response.outcome {
            Ok(outcome)
        } else {
            Err(anyhow!(
                "{}",
                response
                    .error
                    .unwrap_or_else(|| "ASR daemon 未知错误".into())
            ))
        }
    }
}

impl Drop for AsrDaemonProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
struct DaemonRecognizerCache {
    current: Option<CachedRecognizer>,
}

struct CachedRecognizer {
    key: String,
    recognizer: OfflineRecognizer,
    backend: String,
    model: String,
}

impl DaemonRecognizerCache {
    fn transcribe_profile(
        &mut self,
        input: &AsrInput,
        config: &AppConfig,
        paths: &Paths,
        profile: &str,
    ) -> Result<AsrOutcome> {
        if is_mock_engine(&config.asr.default_engine) {
            return Ok(transcribe_mock(input, profile));
        }
        let spec = recognizer_spec(input, config, paths, profile)?;
        let needs_reload = self
            .current
            .as_ref()
            .map(|cached| cached.key != spec.cache_key)
            .unwrap_or(true);
        if needs_reload {
            let recognizer = OfflineRecognizer::create(&spec.config)
                .ok_or_else(|| anyhow!("ASR 初始化失败：{}", spec.backend))?;
            self.current = Some(CachedRecognizer {
                key: spec.cache_key,
                recognizer,
                backend: spec.backend,
                model: display_path(paths, &spec.model_label),
            });
        }
        let cached = self
            .current
            .as_ref()
            .ok_or_else(|| anyhow!("ASR recognizer 缓存不可用"))?;
        decode_with_recognizer(&cached.recognizer, input, &cached.backend, &cached.model)
    }
}

fn run_asr_daemon_cli() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut cache = DaemonRecognizerCache::default();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<AsrDaemonRequest>(&line) {
            Ok(request) => {
                let id = request.id;
                match handle_daemon_request(&mut cache, request.request) {
                    Ok(outcome) => AsrDaemonResponse {
                        id,
                        outcome: Some(outcome),
                        error: None,
                    },
                    Err(err) => AsrDaemonResponse {
                        id,
                        outcome: None,
                        error: Some(format!("{err:?}")),
                    },
                }
            }
            Err(err) => AsrDaemonResponse {
                id: 0,
                outcome: None,
                error: Some(format!("解析 ASR daemon request 失败：{err}")),
            },
        };
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}

fn handle_daemon_request(
    cache: &mut DaemonRecognizerCache,
    request: AsrWorkerRequest,
) -> Result<AsrOutcome> {
    let (sample_rate, samples) = read_wav_file(&request.wav_path)?;
    let paths = paths_from_worker_request(&request);
    let input = AsrInput {
        samples,
        sample_rate,
        language: request.language,
        prompt: request.prompt,
    };
    let started = Instant::now();
    let mut outcome =
        cache.transcribe_profile(&input, &request.config, &paths, &request.profile)?;
    if outcome.raw_text.is_empty() {
        outcome.raw_text = outcome.text.clone();
    }
    outcome.elapsed_seconds = started.elapsed().as_secs_f32();
    outcome.text = text::clean_asr_text(&outcome.text, &paths.corrections_path);
    Ok(outcome)
}

fn paths_from_worker_request(request: &AsrWorkerRequest) -> Paths {
    Paths {
        root_dir: request.root_dir.clone(),
        app_dir: request.app_dir.clone(),
        model_dir: request.model_dir.clone(),
        config_path: request.app_dir.join("config.json"),
        history_path: request.app_dir.join("history.json"),
        prompt_path: request.app_dir.join("personal_prompt.txt"),
        corrections_path: request.app_dir.join("corrections.json"),
        hotwords_path: request.app_dir.join("hot.txt"),
        hot_rules_path: request.app_dir.join("hot-rule.txt"),
        recordings_dir: request.app_dir.join("recordings"),
        logs_dir: request.app_dir.join("logs"),
    }
}

fn transcribe_profile_in_worker(
    input: &AsrInput,
    config: &AppConfig,
    paths: &Paths,
    profile: &str,
) -> Result<AsrOutcome> {
    let request_file = tempfile::Builder::new()
        .prefix("voice_ime_asr_request_")
        .suffix(".json")
        .tempfile()?
        .into_temp_path()
        .keep()?;
    let wav_file = tempfile::Builder::new()
        .prefix("voice_ime_asr_audio_")
        .suffix(".wav")
        .tempfile()?
        .into_temp_path()
        .keep()?;
    let output_file = tempfile::Builder::new()
        .prefix("voice_ime_asr_output_")
        .suffix(".json")
        .tempfile()?
        .into_temp_path()
        .keep()?;

    let result = (|| -> Result<AsrOutcome> {
        write_wav_file(&wav_file, &input.samples, input.sample_rate)?;
        let request = AsrWorkerRequest {
            wav_path: wav_file.clone(),
            output_path: output_file.clone(),
            root_dir: paths.root_dir.clone(),
            app_dir: paths.app_dir.clone(),
            model_dir: config::effective_model_root(config, paths),
            config: config.clone(),
            profile: profile.into(),
            language: input.language.clone(),
            prompt: input.prompt.clone(),
        };
        fs::write(&request_file, serde_json::to_vec_pretty(&request)?)?;
        let exe = env::current_exe().context("定位 ASR worker exe 失败")?;
        let mut command = Command::new(exe);
        command
            .arg("--asr-worker")
            .arg(&request_file)
            .current_dir(&paths.root_dir)
            .env("VOICE_IME_ROOT", &paths.root_dir)
            .env("VOICE_IME_APP_DIR", &paths.app_dir);
        #[cfg(target_os = "windows")]
        {
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let output = command.output().context("启动 ASR 子进程失败")?;
        if !output.status.success() {
            return Err(anyhow!(
                "ASR 子进程退出：{}{}",
                output
                    .status
                    .code()
                    .map(|code| format!("code={code}"))
                    .unwrap_or_else(|| "进程被终止".into()),
                output_tail(&output.stderr, &output.stdout)
            ));
        }
        let body = fs::read(&output_file).context("读取 ASR 子进程结果失败")?;
        serde_json::from_slice(&body).context("解析 ASR 子进程结果失败")
    })();

    let _ = fs::remove_file(&request_file);
    let _ = fs::remove_file(&wav_file);
    let _ = fs::remove_file(&output_file);
    result
}

fn transcribe_profile_in_daemon(
    input: &AsrInput,
    config: &AppConfig,
    paths: &Paths,
    profile: &str,
) -> Result<AsrOutcome> {
    let wav_file = tempfile::Builder::new()
        .prefix("voice_ime_asr_audio_")
        .suffix(".wav")
        .tempfile()?
        .into_temp_path()
        .keep()?;
    let output_file = tempfile::Builder::new()
        .prefix("voice_ime_asr_output_")
        .suffix(".json")
        .tempfile()?
        .into_temp_path()
        .keep()?;

    let result = (|| -> Result<AsrOutcome> {
        write_wav_file(&wav_file, &input.samples, input.sample_rate)?;
        let request = AsrWorkerRequest {
            wav_path: wav_file.clone(),
            output_path: output_file.clone(),
            root_dir: paths.root_dir.clone(),
            app_dir: paths.app_dir.clone(),
            model_dir: config::effective_model_root(config, paths),
            config: config.clone(),
            profile: profile.into(),
            language: input.language.clone(),
            prompt: input.prompt.clone(),
        };
        let mut daemon = ASR_DAEMON.lock();
        if daemon.is_none() {
            *daemon = Some(AsrDaemonProcess::spawn(paths)?);
        }
        let response = daemon
            .as_mut()
            .ok_or_else(|| anyhow!("ASR 常驻子进程不可用"))?
            .transcribe(request);
        if response.is_err() {
            *daemon = None;
        }
        response
    })();

    let _ = fs::remove_file(&wav_file);
    let _ = fs::remove_file(&output_file);
    result
}

pub fn model_status(config: &AppConfig, paths: &Paths) -> Vec<AsrModelStatus> {
    ["fast", "balanced", "fallback"]
        .into_iter()
        .map(|profile| {
            if is_mock_engine(&config.asr.default_engine) {
                return AsrModelStatus {
                    engine: "mock-asr".into(),
                    profile: profile.into(),
                    description: profile_description(profile).into(),
                    expected_latency: "测试后端，立即返回".into(),
                    ready: true,
                    download_url: String::new(),
                    mirror_url: String::new(),
                    target_dir: String::new(),
                    required_files: Vec::new(),
                    missing_files: Vec::new(),
                };
            }
            let required_files = required_files_for_profile(config, paths, profile);
            let target_dir = model_target_dir(config, paths, profile)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            let missing_files = required_files
                .iter()
                .filter(|path| !Path::new(path).exists())
                .cloned()
                .collect::<Vec<_>>();
            AsrModelStatus {
                engine: if profile == "fallback" {
                    "sherpa-onnx-whisper".into()
                } else {
                    "sherpa-onnx".into()
                },
                profile: profile.into(),
                description: profile_description(profile).into(),
                expected_latency: profile_expected_latency(profile).into(),
                ready: missing_files.is_empty(),
                download_url: download_url_for_profile(profile).into(),
                mirror_url: mirror_url_for_profile(profile).into(),
                target_dir,
                required_files,
                missing_files,
            }
        })
        .collect()
}

fn profile_description(profile: &str) -> &'static str {
    match profile {
        "fast" => "中文短句速度优先，适合老电脑和即时输入",
        "balanced" => "默认主力，中文/英文/日文兼顾，准确率和速度平衡",
        "fallback" => "小体积多语种兜底，适合先验证环境是否可用",
        _ => "自定义 ASR 档位",
    }
}

fn profile_expected_latency(profile: &str) -> &'static str {
    match profile {
        "fast" => "10 秒短句约 1-3 秒",
        "balanced" => "10 秒短句约 2-5 秒",
        "fallback" => "10 秒短句约 3-8 秒",
        _ => "视模型而定",
    }
}

pub fn download_model<F>(
    profile: &str,
    config: &AppConfig,
    paths: &Paths,
    mut progress: F,
) -> Result<()>
where
    F: FnMut(String),
{
    let spec = download_spec(profile, config, paths)?;
    fs::create_dir_all(&spec.target_dir)?;
    let client = Client::builder().build()?;
    let total = spec.files.len().max(1);
    for (index, file) in spec.files.iter().enumerate() {
        let target = spec.target_dir.join(&file.filename);
        if target.exists() {
            continue;
        }
        progress(format!(
            "{} 下载中：{}/{} {}",
            spec.profile,
            index + 1,
            total,
            file.filename
        ));
        let mut last_error = None;
        let mut response = None;
        for url in &file.urls {
            match client
                .get(url)
                .send()
                .and_then(|res| res.error_for_status())
            {
                Ok(res) => {
                    response = Some(res);
                    break;
                }
                Err(err) => last_error = Some(err),
            }
        }
        let mut response = response.ok_or_else(|| {
            anyhow!(
                "{} 下载失败：{}",
                file.filename,
                last_error
                    .map(|err| err.to_string())
                    .unwrap_or_else(|| "无可用下载地址".into())
            )
        })?;
        let tmp = target.with_extension("download");
        let mut out = fs::File::create(&tmp)?;
        io::copy(&mut response, &mut out)?;
        fs::rename(&tmp, &target)?;
    }
    Ok(())
}

pub fn download_spec(
    profile: &str,
    config: &AppConfig,
    paths: &Paths,
) -> Result<ModelDownloadSpec> {
    let target_dir = model_target_dir(config, paths, profile)
        .ok_or_else(|| anyhow!("未知 ASR profile：{profile}"))?;
    let (repo, files): (&str, &[&str]) = match profile {
        "fast" => (
            "csukuangfj/sherpa-onnx-zipformer-ctc-zh-int8-2025-07-03",
            &["model.int8.onnx", "tokens.txt", "bbpe.model"],
        ),
        "balanced" => (
            "chris-cao/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
            &["model.int8.onnx", "tokens.txt"],
        ),
        "fallback" => (
            "csukuangfj/sherpa-onnx-whisper-tiny",
            &[
                "tiny-encoder.int8.onnx",
                "tiny-decoder.int8.onnx",
                "tiny-tokens.txt",
            ],
        ),
        other => return Err(anyhow!("未知 ASR profile：{other}")),
    };
    let mirror_base = format!("https://hf-mirror.com/{repo}/resolve/main");
    let official_base = format!("https://huggingface.co/{repo}/resolve/main");
    Ok(ModelDownloadSpec {
        profile: profile.into(),
        target_dir,
        files: files
            .iter()
            .map(|filename| ModelDownloadFile {
                urls: vec![
                    format!("{mirror_base}/{filename}"),
                    format!("{official_base}/{filename}"),
                ],
                filename: (*filename).into(),
            })
            .collect(),
    })
}

fn transcribe_profile(
    input: &AsrInput,
    config: &AppConfig,
    paths: &Paths,
    profile: &str,
) -> Result<AsrOutcome> {
    if is_mock_engine(&config.asr.default_engine) {
        return Ok(transcribe_mock(input, profile));
    }
    let spec = recognizer_spec(input, config, paths, profile)?;
    let recognizer = OfflineRecognizer::create(&spec.config)
        .ok_or_else(|| anyhow!("ASR 初始化失败：{}", spec.backend))?;
    decode_with_recognizer(
        &recognizer,
        input,
        &spec.backend,
        &display_path(paths, &spec.model_label),
    )
}

pub(crate) fn is_mock_engine(engine: &str) -> bool {
    matches!(
        engine.trim().to_ascii_lowercase().as_str(),
        "mock" | "fake" | "test"
    )
}

fn transcribe_mock(input: &AsrInput, profile: &str) -> AsrOutcome {
    let text = mock_asr_text(input);
    AsrOutcome {
        raw_text: text.clone(),
        text,
        backend: "mock-asr".into(),
        model: format!("mock/{profile}"),
        elapsed_seconds: 0.0,
    }
}

fn mock_asr_text(input: &AsrInput) -> String {
    for line in input.prompt.lines() {
        let trimmed = line.trim();
        for prefix in ["mock-asr:", "mock_asr:", "mock:"] {
            if let Some(text) = trimmed.strip_prefix(prefix) {
                return text.trim().to_string();
            }
        }
    }
    "Voice IME mock transcript".into()
}

struct RecognizerSpec {
    config: OfflineRecognizerConfig,
    backend: String,
    model_label: PathBuf,
    cache_key: String,
}

fn recognizer_spec(
    input: &AsrInput,
    config: &AppConfig,
    paths: &Paths,
    profile: &str,
) -> Result<RecognizerSpec> {
    let mut recognizer_config = OfflineRecognizerConfig::default();
    recognizer_config.feat_config.sample_rate = input.sample_rate as i32;
    recognizer_config.model_config.num_threads = config.asr.num_threads.max(1);
    recognizer_config.model_config.provider = Some("cpu".into());

    let (backend, model_label, cache_key) = match profile {
        "fast" => {
            let model = resolve_existing(config, paths, &config.asr.models.zipformer_ctc_model)?;
            let tokens = resolve_existing(config, paths, &config.asr.models.zipformer_ctc_tokens)?;
            recognizer_config.model_config.zipformer_ctc = OfflineZipformerCtcModelConfig {
                model: Some(model.to_string_lossy().to_string()),
            };
            recognizer_config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
            recognizer_config.model_config.modeling_unit = Some("cjkchar".into());
            recognizer_config.decoding_method = Some("greedy_search".into());
            (
                "sherpa-onnx/zipformer-ctc".to_string(),
                model.clone(),
                format!(
                    "fast|sr={}|threads={}|model={}|tokens={}",
                    input.sample_rate,
                    config.asr.num_threads.max(1),
                    model.display(),
                    tokens.display()
                ),
            )
        }
        "balanced" => {
            let model = resolve_existing(config, paths, &config.asr.models.sense_voice_model)?;
            let tokens = resolve_existing(config, paths, &config.asr.models.sense_voice_tokens)?;
            let language = sense_voice_language(&input.language);
            recognizer_config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
                model: Some(model.to_string_lossy().to_string()),
                language: Some(language.clone()),
                use_itn: true,
            };
            recognizer_config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
            (
                "sherpa-onnx/sense-voice".to_string(),
                model.clone(),
                format!(
                    "balanced|sr={}|threads={}|lang={language}|model={}|tokens={}",
                    input.sample_rate,
                    config.asr.num_threads.max(1),
                    model.display(),
                    tokens.display()
                ),
            )
        }
        "fallback" => {
            let encoder = resolve_existing(config, paths, &config.asr.models.whisper_encoder)?;
            let decoder = resolve_existing(config, paths, &config.asr.models.whisper_decoder)?;
            let tokens = resolve_existing(config, paths, &config.asr.models.whisper_tokens)?;
            let language = whisper_language(&input.language);
            recognizer_config.model_config.whisper = OfflineWhisperModelConfig {
                encoder: Some(encoder.to_string_lossy().to_string()),
                decoder: Some(decoder.to_string_lossy().to_string()),
                language: Some(language.clone()),
                task: Some("transcribe".into()),
                tail_paddings: -1,
                enable_token_timestamps: false,
                enable_segment_timestamps: false,
            };
            recognizer_config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
            (
                "sherpa-onnx/whisper".to_string(),
                encoder.clone(),
                format!(
                    "fallback|sr={}|threads={}|lang={language}|encoder={}|decoder={}|tokens={}",
                    input.sample_rate,
                    config.asr.num_threads.max(1),
                    encoder.display(),
                    decoder.display(),
                    tokens.display()
                ),
            )
        }
        other => return Err(anyhow!("未知 ASR profile：{other}")),
    };

    Ok(RecognizerSpec {
        config: recognizer_config,
        backend,
        model_label,
        cache_key,
    })
}

fn decode_with_recognizer(
    recognizer: &OfflineRecognizer,
    input: &AsrInput,
    backend: &str,
    model: &str,
) -> Result<AsrOutcome> {
    let stream = recognizer.create_stream();
    stream.accept_waveform(input.sample_rate as i32, &input.samples);
    recognizer.decode(&stream);
    let result = stream.get_result().context("ASR 没有返回结果")?;
    Ok(AsrOutcome {
        raw_text: result.text.clone(),
        text: result.text,
        backend: backend.into(),
        model: model.into(),
        elapsed_seconds: 0.0,
    })
}

pub fn split_samples(samples: &[f32], sample_rate: u32, chunk_seconds: u32) -> Vec<Vec<f32>> {
    let chunk_len = (sample_rate * chunk_seconds.max(1)) as usize;
    samples
        .chunks(chunk_len)
        .map(|chunk| chunk.to_vec())
        .collect()
}

fn profile_order(profile: &str) -> Vec<&'static str> {
    match profile {
        "fast" => vec!["fast", "balanced", "fallback"],
        "fallback" => vec!["fallback", "balanced", "fast"],
        _ => vec!["balanced", "fast", "fallback"],
    }
}

fn required_files_for_profile(config: &AppConfig, paths: &Paths, profile: &str) -> Vec<String> {
    let candidates = match profile {
        "fast" => vec![
            &config.asr.models.zipformer_ctc_model,
            &config.asr.models.zipformer_ctc_tokens,
        ],
        "balanced" => vec![
            &config.asr.models.sense_voice_model,
            &config.asr.models.sense_voice_tokens,
        ],
        "fallback" => vec![
            &config.asr.models.whisper_encoder,
            &config.asr.models.whisper_decoder,
            &config.asr.models.whisper_tokens,
        ],
        _ => vec![],
    };
    candidates
        .into_iter()
        .map(|item| {
            config::resolve_model_path(config, paths, item)
                .to_string_lossy()
                .to_string()
        })
        .collect()
}

fn model_target_dir(config: &AppConfig, paths: &Paths, profile: &str) -> Option<PathBuf> {
    let first = match profile {
        "fast" => &config.asr.models.zipformer_ctc_model,
        "balanced" => &config.asr.models.sense_voice_model,
        "fallback" => &config.asr.models.whisper_encoder,
        _ => return None,
    };
    config::resolve_model_path(config, paths, first)
        .parent()
        .map(Path::to_path_buf)
}

pub fn download_url_for_profile(profile: &str) -> &'static str {
    match profile {
        "fast" => "https://huggingface.co/csukuangfj/sherpa-onnx-zipformer-ctc-zh-int8-2025-07-03/tree/main",
        "balanced" => "https://huggingface.co/chris-cao/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/tree/main",
        "fallback" => "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny/tree/main",
        _ => "https://k2-fsa.github.io/sherpa/onnx/index.html",
    }
}

pub fn mirror_url_for_profile(profile: &str) -> &'static str {
    match profile {
        "fast" => "https://hf-mirror.com/csukuangfj/sherpa-onnx-zipformer-ctc-zh-int8-2025-07-03/tree/main",
        "balanced" => "https://hf-mirror.com/chris-cao/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/tree/main",
        "fallback" => "https://hf-mirror.com/csukuangfj/sherpa-onnx-whisper-tiny/tree/main",
        _ => "https://hf-mirror.com/",
    }
}

fn resolve_existing(config: &AppConfig, paths: &Paths, configured: &str) -> Result<PathBuf> {
    let path = config::resolve_model_path(config, paths, configured);
    if path.exists() {
        Ok(path)
    } else {
        Err(anyhow!("缺少 ASR 模型文件：{}", path.display()))
    }
}

fn display_path(paths: &Paths, path: &Path) -> String {
    path.strip_prefix(&paths.root_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn sense_voice_language(language: &str) -> String {
    match language {
        "zh" | "en" | "ja" | "ko" | "yue" => language.into(),
        _ => "auto".into(),
    }
}

fn whisper_language(language: &str) -> String {
    match language {
        "zh" => "zh",
        "ja" => "ja",
        "en" => "en",
        _ => "auto",
    }
    .into()
}

fn write_wav_file(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        writer.write_sample((clamped * i16::MAX as f32) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

pub(crate) fn read_wav_file(path: &Path) -> Result<(u32, Vec<f32>)> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int if spec.bits_per_sample <= 16 => reader
            .samples::<i16>()
            .map(|sample| sample.map(|value| value as f32 / i16::MAX as f32))
            .collect::<Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => {
            let denom = ((1_i64 << (spec.bits_per_sample - 1).min(31)) - 1) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / denom))
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok((spec.sample_rate, mix_to_mono(&samples, channels)))
}

fn mix_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
        .collect()
}

fn output_tail(stderr: &[u8], stdout: &[u8]) -> String {
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(stderr));
    if !stdout.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&String::from_utf8_lossy(stdout));
    }
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }
    let mut tail = text.chars().rev().take(1200).collect::<Vec<_>>();
    tail.reverse();
    format!("：{}", tail.into_iter().collect::<String>().trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(root: &Path) -> Paths {
        Paths {
            root_dir: root.to_path_buf(),
            app_dir: root.join(".voice_ime"),
            model_dir: root.join("models"),
            config_path: root.join(".voice_ime/config.json"),
            history_path: root.join(".voice_ime/history.json"),
            prompt_path: root.join(".voice_ime/personal_prompt.txt"),
            corrections_path: root.join(".voice_ime/corrections.json"),
            hotwords_path: root.join(".voice_ime/hot.txt"),
            hot_rules_path: root.join(".voice_ime/hot-rule.txt"),
            recordings_dir: root.join(".voice_ime/recordings"),
            logs_dir: root.join(".voice_ime/logs"),
        }
    }

    #[test]
    fn profile_order_falls_back() {
        assert_eq!(
            profile_order("balanced"),
            vec!["balanced", "fast", "fallback"]
        );
        assert_eq!(profile_order("fast")[0], "fast");
    }

    #[test]
    fn splits_samples() {
        let samples = vec![0.0; 16_000 * 25];
        let chunks = split_samples(&samples, 16_000, 10);
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn download_specs_match_configured_files() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let config = AppConfig::default();
        let balanced = download_spec("balanced", &config, &paths).unwrap();
        let fallback = download_spec("fallback", &config, &paths).unwrap();
        assert!(balanced
            .files
            .iter()
            .any(|file| file.filename == "model.int8.onnx"));
        assert!(fallback
            .files
            .iter()
            .any(|file| file.filename == "tiny-encoder.int8.onnx"));
        assert_eq!(
            fallback.target_dir,
            temp.path().join("models/sherpa-onnx-whisper-tiny")
        );
    }

    #[test]
    fn model_status_uses_configured_model_root() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let external = temp.path().join("external-models");
        let mut config = AppConfig::default();
        config.asr.model_root = external.to_string_lossy().to_string();
        for relative in [
            "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/model.int8.onnx",
            "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17/tokens.txt",
        ] {
            let path = external.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "placeholder").unwrap();
        }

        let balanced = model_status(&config, &paths)
            .into_iter()
            .find(|status| status.profile == "balanced")
            .unwrap();

        assert!(balanced.ready);
        assert!(balanced.description.contains("默认主力"));
        assert!(balanced.expected_latency.contains("10 秒"));
        assert_eq!(
            balanced.target_dir,
            external
                .join("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17")
                .to_string_lossy()
                .to_string()
        );
    }

    #[test]
    fn prewarm_selects_first_ready_profile() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let config = AppConfig::default();
        for file in required_files_for_profile(&config, &paths, "fast") {
            let path = PathBuf::from(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "placeholder").unwrap();
        }
        assert_eq!(prewarm_profile(&config, &paths), Some("fast".into()));
    }

    #[test]
    fn prewarm_skips_when_no_profile_is_ready() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        assert_eq!(prewarm_profile(&AppConfig::default(), &paths), None);
    }

    #[test]
    fn mock_engine_transcribes_without_model_files() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let mut config = AppConfig::default();
        config.asr.default_engine = "mock".into();
        config.asr.profile = "fast".into();
        let input = AsrInput {
            samples: vec![0.0; 1600],
            sample_rate: 16_000,
            language: "zh".into(),
            prompt: "mock-asr:非洲之星和海洋之泪".into(),
        };

        let outcome = transcribe(&input, &config, &paths).unwrap();

        assert_eq!(outcome.text, "非洲之星和海洋之泪");
        assert_eq!(outcome.raw_text, outcome.text);
        assert_eq!(outcome.backend, "mock-asr");
        assert_eq!(outcome.model, "mock/fast");
    }

    #[test]
    fn mock_model_status_is_ready_without_files() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let mut config = AppConfig::default();
        config.asr.default_engine = "mock".into();

        let statuses = model_status(&config, &paths);

        assert_eq!(statuses.len(), 3);
        assert!(statuses.iter().all(|status| status.ready));
        assert!(statuses.iter().all(|status| status.engine == "mock-asr"));
        assert!(statuses
            .iter()
            .all(|status| status.required_files.is_empty()));
    }
}
