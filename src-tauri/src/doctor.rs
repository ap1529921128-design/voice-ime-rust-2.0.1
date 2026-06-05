use crate::{
    asr, audio,
    config::{AppConfig, Paths, DEFAULT_HOTWORDS, DEFAULT_HOT_RULES, DEFAULT_PERSONAL_PROMPT},
    llm, translation,
};
use anyhow::Result;
use reqwest::blocking::Client;
use serde::Serialize;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub output_path: String,
    pub summary: String,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorCheck {
    pub name: String,
    pub status: DoctorStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairReport {
    pub summary: String,
    pub actions: Vec<RepairAction>,
    pub doctor: DoctorReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairAction {
    pub name: String,
    pub status: RepairStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RepairStatus {
    Repaired,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DoctorStatus {
    Pass,
    Warn,
    Fail,
}

pub fn run(paths: &Paths, config: &AppConfig) -> Result<DoctorReport> {
    paths.ensure()?;
    let started = Instant::now();
    let mut checks = Vec::new();
    check_app_paths(paths, config, &mut checks);
    check_audio(&mut checks);
    check_clipboard(&mut checks);
    check_models(paths, config, &mut checks);
    check_llm_endpoint("智能纠错端点", &config.smart.endpoint, &mut checks);
    check_llm_artifacts(paths, config, &mut checks);
    check_translation_backend(config, &mut checks);
    check_user_text_files(paths, &mut checks);
    check_runtime_logs(paths, &mut checks);

    let output_path = write_report(paths, config, &checks, started.elapsed())?;
    Ok(DoctorReport {
        summary: summarize(&checks),
        output_path: output_path.to_string_lossy().to_string(),
        checks,
    })
}

pub fn repair(paths: &Paths, config: &AppConfig) -> Result<RepairReport> {
    let mut actions = Vec::new();
    repair_directory(&mut actions, "应用数据目录", &paths.app_dir);
    repair_directory(&mut actions, "日志目录", &paths.logs_dir);
    repair_directory(&mut actions, "长录音目录", &paths.recordings_dir);
    repair_text_file(&mut actions, "个人提示词", &paths.prompt_path, || {
        Ok(DEFAULT_PERSONAL_PROMPT.to_string())
    });
    repair_text_file(&mut actions, "纠错表", &paths.corrections_path, || {
        Ok(serde_json::to_string_pretty(
            &crate::text::default_corrections(),
        )?)
    });
    repair_text_file(&mut actions, "热词", &paths.hotwords_path, || {
        Ok(DEFAULT_HOTWORDS.to_string())
    });
    repair_text_file(&mut actions, "规则", &paths.hot_rules_path, || {
        Ok(DEFAULT_HOT_RULES.to_string())
    });

    let doctor = run(paths, config)?;
    Ok(RepairReport {
        summary: summarize_repair(&actions),
        actions,
        doctor,
    })
}

pub fn run_cli() -> Result<()> {
    let paths = Paths::discover()?;
    let config = crate::config::load_or_create(&paths)?;
    let report = run(&paths, &config)?;
    println!("{}", report.summary);
    println!("{}", report.output_path);
    Ok(())
}

fn repair_directory(actions: &mut Vec<RepairAction>, name: &str, path: &std::path::Path) {
    let existed = path.exists();
    match fs::create_dir_all(path) {
        Ok(()) => push_repair(
            actions,
            name,
            if existed {
                RepairStatus::Skipped
            } else {
                RepairStatus::Repaired
            },
            if existed {
                format!("已存在，未改动：{}", path.to_string_lossy())
            } else {
                format!("已创建：{}", path.to_string_lossy())
            },
        ),
        Err(err) => push_repair(
            actions,
            name,
            RepairStatus::Failed,
            format!("创建失败：{}；{}", path.to_string_lossy(), err),
        ),
    }
}

fn repair_text_file<F>(
    actions: &mut Vec<RepairAction>,
    name: &str,
    path: &std::path::Path,
    content: F,
) where
    F: FnOnce() -> Result<String>,
{
    if path.exists() {
        push_repair(
            actions,
            name,
            RepairStatus::Skipped,
            format!("已存在，未覆盖：{}", path.to_string_lossy()),
        );
        return;
    }
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            push_repair(
                actions,
                name,
                RepairStatus::Failed,
                format!("目录创建失败：{}；{}", parent.to_string_lossy(), err),
            );
            return;
        }
    }
    let content = match content() {
        Ok(content) => content,
        Err(err) => {
            push_repair(actions, name, RepairStatus::Failed, err.to_string());
            return;
        }
    };
    match fs::write(path, content) {
        Ok(()) => push_repair(
            actions,
            name,
            RepairStatus::Repaired,
            format!("已创建：{}", path.to_string_lossy()),
        ),
        Err(err) => push_repair(
            actions,
            name,
            RepairStatus::Failed,
            format!("写入失败：{}；{}", path.to_string_lossy(), err),
        ),
    }
}

fn check_app_paths(paths: &Paths, config: &AppConfig, checks: &mut Vec<DoctorCheck>) {
    push_check(
        checks,
        "应用目录",
        if paths.app_dir.exists() {
            DoctorStatus::Pass
        } else {
            DoctorStatus::Fail
        },
        paths.app_dir.to_string_lossy(),
    );

    let write_test = paths.logs_dir.join(".doctor-write-test");
    let writable = fs::create_dir_all(&paths.logs_dir)
        .and_then(|()| fs::write(&write_test, "ok"))
        .and_then(|()| fs::remove_file(&write_test))
        .is_ok();
    push_check(
        checks,
        "日志目录可写",
        if writable {
            DoctorStatus::Pass
        } else {
            DoctorStatus::Fail
        },
        paths.logs_dir.to_string_lossy(),
    );

    let model_root = crate::config::effective_model_root(config, paths);
    push_check(
        checks,
        "模型根目录",
        if model_root.exists() {
            DoctorStatus::Pass
        } else {
            DoctorStatus::Warn
        },
        model_root.to_string_lossy(),
    );
}

fn check_audio(checks: &mut Vec<DoctorCheck>) {
    match audio::input_devices() {
        Ok(devices) if devices.is_empty() => {
            push_check(checks, "麦克风", DoctorStatus::Fail, "未枚举到输入设备")
        }
        Ok(devices) => {
            let default = devices
                .iter()
                .find(|device| device.is_default)
                .map(|device| device.name.as_str())
                .unwrap_or("未标记默认设备");
            push_check(
                checks,
                "麦克风",
                DoctorStatus::Pass,
                format!("{} 个输入设备；默认：{default}", devices.len()),
            );
        }
        Err(err) => push_check(checks, "麦克风", DoctorStatus::Fail, err.to_string()),
    }
}

fn check_clipboard(checks: &mut Vec<DoctorCheck>) {
    match arboard::Clipboard::new() {
        Ok(_) => push_check(checks, "剪贴板", DoctorStatus::Pass, "可打开"),
        Err(err) => push_check(checks, "剪贴板", DoctorStatus::Warn, err.to_string()),
    }
}

fn check_models(paths: &Paths, config: &AppConfig, checks: &mut Vec<DoctorCheck>) {
    let statuses = asr::model_status(config, paths);
    let ready = statuses.iter().filter(|status| status.ready).count();
    let missing = statuses.len().saturating_sub(ready);
    let detail = statuses
        .iter()
        .map(|status| {
            if status.ready {
                format!("{}: ready", status.profile)
            } else {
                format!("{}: missing {}", status.profile, status.missing_files.len())
            }
        })
        .collect::<Vec<_>>()
        .join("; ");
    let status = if ready > 0 {
        DoctorStatus::Pass
    } else if missing > 0 {
        DoctorStatus::Fail
    } else {
        DoctorStatus::Warn
    };
    push_check(checks, "ASR 模型", status, detail);
}

fn check_translation_backend(config: &AppConfig, checks: &mut Vec<DoctorCheck>) {
    match config.translation.engine.as_str() {
        "llm" | "" => check_llm_endpoint("翻译端点", &config.translation.endpoint, checks),
        "external" => match translation::split_command_line(&config.translation.external_command) {
            Ok(args) => push_check(
                checks,
                "外部翻译命令",
                DoctorStatus::Pass,
                args.first().cloned().unwrap_or_default(),
            ),
            Err(err) => push_check(checks, "外部翻译命令", DoctorStatus::Warn, err.to_string()),
        },
        "nllb" | "bergamot" => push_check(
            checks,
            "翻译引擎",
            DoctorStatus::Warn,
            format!("{} 已预留，当前版本未内置", config.translation.engine),
        ),
        other => push_check(
            checks,
            "翻译引擎",
            DoctorStatus::Warn,
            format!("{other} 未识别"),
        ),
    }
}

fn check_llm_artifacts(paths: &Paths, config: &AppConfig, checks: &mut Vec<DoctorCheck>) {
    let status = llm::local_service_status(&config.smart.endpoint, paths, config);
    if !status.is_local {
        push_check(
            checks,
            "本地 LLM 文件",
            DoctorStatus::Warn,
            "智能端点不是本地地址",
        );
        return;
    }
    let missing = [
        (!status.script_exists).then(|| format!("script={}", status.script_path)),
        (!status.model_exists).then(|| format!("model={}", status.model_path)),
        (!status.server_exists).then(|| format!("server={}", status.server_path)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    push_check(
        checks,
        "本地 LLM 文件",
        if missing.is_empty() {
            DoctorStatus::Pass
        } else {
            DoctorStatus::Warn
        },
        if missing.is_empty() {
            "启动脚本、MiniCPM 模型、llama-server 均存在".to_string()
        } else {
            missing.join("; ")
        },
    );
    push_check(
        checks,
        "本地 LLM 进程",
        if status.server_process_running {
            DoctorStatus::Pass
        } else {
            DoctorStatus::Warn
        },
        status.server_process_detail,
    );
}

fn check_llm_endpoint(name: &str, endpoint: &str, checks: &mut Vec<DoctorCheck>) {
    if endpoint.trim().is_empty() {
        push_check(checks, name, DoctorStatus::Warn, "未配置");
        return;
    }
    if !is_local_endpoint(endpoint) {
        push_check(checks, name, DoctorStatus::Warn, "非本地端点，未主动探测");
        return;
    }
    let url = models_endpoint(endpoint);
    let reachable = Client::builder()
        .timeout(Duration::from_millis(800))
        .connect_timeout(Duration::from_millis(300))
        .build()
        .and_then(|client| client.get(&url).send())
        .map(|response| response.status().is_success() || response.status().as_u16() < 500)
        .unwrap_or(false);
    push_check(
        checks,
        name,
        if reachable {
            DoctorStatus::Pass
        } else {
            DoctorStatus::Warn
        },
        if reachable {
            format!("{url} 可达")
        } else {
            format!("{url} 暂不可达")
        },
    );
}

fn check_user_text_files(paths: &Paths, checks: &mut Vec<DoctorCheck>) {
    let files = [
        ("个人提示词", &paths.prompt_path),
        ("纠错表", &paths.corrections_path),
        ("热词", &paths.hotwords_path),
        ("规则", &paths.hot_rules_path),
    ];
    for (name, path) in files {
        push_check(
            checks,
            name,
            if path.exists() {
                DoctorStatus::Pass
            } else {
                DoctorStatus::Warn
            },
            path.to_string_lossy(),
        );
    }
}

fn check_runtime_logs(paths: &Paths, checks: &mut Vec<DoctorCheck>) {
    let panic_logs = latest_log(paths, "panic-*.log");
    let worker_logs = latest_log(paths, "worker-error-*.log");
    let shutdown_logs = latest_log(paths, "shutdown-*.log");
    let mut details = Vec::new();
    if let Some(path) = panic_logs.as_ref() {
        details.push(format!("panic={}", path.to_string_lossy()));
    }
    if let Some(path) = worker_logs.as_ref() {
        details.push(format!("worker={}", path.to_string_lossy()));
    }
    if let Some(path) = shutdown_logs.as_ref() {
        details.push(format!("shutdown={}", path.to_string_lossy()));
    }
    let has_crash_log = panic_logs.is_some() || worker_logs.is_some();
    push_check(
        checks,
        "异常日志",
        if has_crash_log {
            DoctorStatus::Warn
        } else {
            DoctorStatus::Pass
        },
        if details.is_empty() {
            "未发现 panic/worker/shutdown 日志".to_string()
        } else {
            details.join("; ")
        },
    );
}

fn latest_log(paths: &Paths, pattern: &str) -> Option<PathBuf> {
    let prefix = pattern.strip_suffix("*.log")?;
    fs::read_dir(&paths.logs_dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy();
            (name.starts_with(prefix) && name.ends_with(".log"))
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

fn write_report(
    paths: &Paths,
    config: &AppConfig,
    checks: &[DoctorCheck],
    elapsed: Duration,
) -> Result<PathBuf> {
    fs::create_dir_all(&paths.logs_dir)?;
    let output_path = paths.logs_dir.join(format!(
        "doctor-{}.txt",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    let mut lines = Vec::new();
    lines.push("Voice IME Doctor".to_string());
    lines.push(format!(
        "Created: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    lines.push(format!("Root: {}", paths.root_dir.to_string_lossy()));
    lines.push(format!("App: {}", paths.app_dir.to_string_lossy()));
    lines.push(format!(
        "Models: {}",
        crate::config::effective_model_root(config, paths).to_string_lossy()
    ));
    lines.push(format!(
        "ASR: profile={} worker={} threads={}",
        config.asr.profile, config.asr.worker_mode, config.asr.num_threads
    ));
    lines.push(format!(
        "Input: mode={} ptt={} key={} mouse={} hold_threshold_ms={}",
        config.input.mode,
        config.input.ptt_enabled,
        config.input.ptt_key,
        config.input.ptt_mouse_button,
        config.input.ptt_hold_threshold_ms
    ));
    lines.push(format!(
        "Translation: engine={} timeout={}s",
        config.translation.engine, config.translation.timeout_seconds
    ));
    lines.push(format!("Elapsed: {:.2}s", elapsed.as_secs_f32()));
    lines.push(String::new());
    for check in checks {
        lines.push(format!(
            "[{}] {} - {}",
            status_label(check.status),
            check.name,
            check.detail
        ));
    }
    fs::write(&output_path, lines.join("\n"))?;
    Ok(output_path)
}

pub(crate) fn summarize(checks: &[DoctorCheck]) -> String {
    let failed = checks
        .iter()
        .filter(|check| check.status == DoctorStatus::Fail)
        .count();
    let warned = checks
        .iter()
        .filter(|check| check.status == DoctorStatus::Warn)
        .count();
    if failed == 0 && warned == 0 {
        format!("诊断完成：{} 项通过", checks.len())
    } else {
        format!("诊断完成：{} 项失败，{} 项提醒", failed, warned)
    }
}

fn push_check(
    checks: &mut Vec<DoctorCheck>,
    name: impl Into<String>,
    status: DoctorStatus,
    detail: impl Into<String>,
) {
    checks.push(DoctorCheck {
        name: name.into(),
        status,
        detail: detail.into(),
    });
}

fn push_repair(
    actions: &mut Vec<RepairAction>,
    name: impl Into<String>,
    status: RepairStatus,
    detail: impl Into<String>,
) {
    actions.push(RepairAction {
        name: name.into(),
        status,
        detail: detail.into(),
    });
}

fn summarize_repair(actions: &[RepairAction]) -> String {
    let repaired = actions
        .iter()
        .filter(|action| action.status == RepairStatus::Repaired)
        .count();
    let skipped = actions
        .iter()
        .filter(|action| action.status == RepairStatus::Skipped)
        .count();
    let failed = actions
        .iter()
        .filter(|action| action.status == RepairStatus::Failed)
        .count();
    if failed == 0 {
        format!("修复完成：{} 项补齐，{} 项已存在", repaired, skipped)
    } else {
        format!(
            "修复完成：{} 项补齐，{} 项已存在，{} 项失败",
            repaired, skipped, failed
        )
    }
}

fn status_label(status: DoctorStatus) -> &'static str {
    match status {
        DoctorStatus::Pass => "PASS",
        DoctorStatus::Warn => "WARN",
        DoctorStatus::Fail => "FAIL",
    }
}

fn models_endpoint(endpoint: &str) -> String {
    endpoint
        .trim_end_matches('/')
        .replace("/v1/chat/completions", "/v1/models")
        .replace("/chat/completions", "/models")
}

fn is_local_endpoint(endpoint: &str) -> bool {
    endpoint.contains("127.0.0.1")
        || endpoint.contains("localhost")
        || endpoint.contains("[::1]")
        || endpoint.contains("://::1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_failures_and_warnings() {
        let checks = vec![
            DoctorCheck {
                name: "a".into(),
                status: DoctorStatus::Pass,
                detail: "ok".into(),
            },
            DoctorCheck {
                name: "b".into(),
                status: DoctorStatus::Warn,
                detail: "warn".into(),
            },
            DoctorCheck {
                name: "c".into(),
                status: DoctorStatus::Fail,
                detail: "fail".into(),
            },
        ];
        assert_eq!(summarize(&checks), "诊断完成：1 项失败，1 项提醒");
    }

    #[test]
    fn derives_models_endpoint_from_chat_endpoint() {
        assert_eq!(
            models_endpoint("http://127.0.0.1:18080/v1/chat/completions"),
            "http://127.0.0.1:18080/v1/models"
        );
    }

    #[test]
    fn repair_creates_missing_text_files_without_overwriting_existing_files() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        fs::create_dir_all(&paths.app_dir).unwrap();
        let hotwords_path = paths.hotwords_path.clone();
        fs::write(&hotwords_path, "custom hotwords").unwrap();

        let report = repair(&paths, &AppConfig::default()).unwrap();

        assert!(paths.prompt_path.exists());
        assert!(paths.corrections_path.exists());
        assert!(paths.hot_rules_path.exists());
        assert_eq!(
            fs::read_to_string(hotwords_path).unwrap(),
            "custom hotwords"
        );
        assert!(report
            .actions
            .iter()
            .any(|action| { action.name == "热词" && action.status == RepairStatus::Skipped }));
        assert!(report.actions.iter().any(|action| {
            action.name == "个人提示词" && action.status == RepairStatus::Repaired
        }));
    }

    #[test]
    fn runtime_logs_warn_when_panic_log_exists() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        fs::create_dir_all(&paths.logs_dir).unwrap();
        fs::write(paths.logs_dir.join("panic-20260605.log"), "panic").unwrap();
        let mut checks = Vec::new();

        check_runtime_logs(&paths, &mut checks);

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, "异常日志");
        assert_eq!(checks[0].status, DoctorStatus::Warn);
        assert!(checks[0].detail.contains("panic-20260605.log"));
    }

    #[test]
    fn runtime_logs_pass_when_no_error_logs_exist() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        fs::create_dir_all(&paths.logs_dir).unwrap();
        let mut checks = Vec::new();

        check_runtime_logs(&paths, &mut checks);

        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].status, DoctorStatus::Pass);
    }

    fn test_paths(root: &std::path::Path) -> Paths {
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
}
