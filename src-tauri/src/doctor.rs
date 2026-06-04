use crate::{
    asr, audio,
    config::{AppConfig, Paths},
    translation,
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
    check_app_paths(paths, &mut checks);
    check_audio(&mut checks);
    check_clipboard(&mut checks);
    check_models(paths, config, &mut checks);
    check_llm_endpoint("智能纠错端点", &config.smart.endpoint, &mut checks);
    check_translation_backend(config, &mut checks);
    check_user_text_files(paths, &mut checks);

    let output_path = write_report(paths, config, &checks, started.elapsed())?;
    Ok(DoctorReport {
        summary: summarize(&checks),
        output_path: output_path.to_string_lossy().to_string(),
        checks,
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

fn check_app_paths(paths: &Paths, checks: &mut Vec<DoctorCheck>) {
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
        "ASR: profile={} worker={} threads={}",
        config.asr.profile, config.asr.worker_mode, config.asr.num_threads
    ));
    lines.push(format!(
        "Input: mode={} ptt={} key={} mouse={}",
        config.input.mode,
        config.input.ptt_enabled,
        config.input.ptt_key,
        config.input.ptt_mouse_button
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

fn summarize(checks: &[DoctorCheck]) -> String {
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
}
