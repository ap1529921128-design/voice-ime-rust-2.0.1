use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use hound::{SampleFormat as WavSampleFormat, WavSpec, WavWriter};
use once_cell::sync::Lazy;
use parking_lot::Mutex as GateMutex;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

static AUDIO_STREAM_OPEN_LOCK: Lazy<GateMutex<()>> = Lazy::new(|| GateMutex::new(()));

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceInfo {
    pub index: usize,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioLevelInfo {
    pub device_name: String,
    pub sample_rate: u32,
    pub duration_ms: u64,
    pub peak: f32,
    pub rms: f32,
    pub samples: usize,
}

#[derive(Debug, Clone)]
pub struct Recording {
    pub wav_path: PathBuf,
    pub duration_seconds: f32,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
    pub peak: f32,
    pub rms: f32,
}

struct ActiveRecording {
    _stream: Stream,
    started_at: Instant,
    source_sample_rate: u32,
    samples: Arc<Mutex<Vec<f32>>>,
}

#[derive(Default)]
pub struct Recorder {
    active: Mutex<Option<ActiveRecording>>,
}

unsafe impl Send for Recorder {}
unsafe impl Sync for Recorder {}

impl Recorder {
    pub fn is_recording(&self) -> bool {
        self.active.lock().unwrap().is_some()
    }

    pub fn start(&self, input_device_name: Option<&str>) -> Result<()> {
        let mut guard = self.active.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }
        let _open_guard = AUDIO_STREAM_OPEN_LOCK.lock();
        let host = cpal::default_host();
        let (device, _) = select_input_device(&host, input_device_name)?;
        let samples = Arc::new(Mutex::new(Vec::<f32>::with_capacity(48_000usize * 8)));
        let (stream, source_sample_rate) = build_capture_stream(&device, samples.clone())?;
        stream.play().context("启动麦克风失败")?;
        *guard = Some(ActiveRecording {
            _stream: stream,
            started_at: Instant::now(),
            source_sample_rate,
            samples,
        });
        Ok(())
    }

    pub fn stop(&self, target_sample_rate: u32) -> Result<Recording> {
        let active = self
            .active
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| anyhow!("当前没有录音"))?;
        let duration_seconds = active.started_at.elapsed().as_secs_f32();
        let mono = active.samples.lock().unwrap().clone();
        let samples = if active.source_sample_rate == target_sample_rate {
            mono
        } else {
            resample_linear(&mono, active.source_sample_rate, target_sample_rate)
        };
        let (peak, rms) = sample_stats(&samples);
        let wav_path = tempfile::Builder::new()
            .prefix("voice_ime_rust_")
            .suffix(".wav")
            .tempfile()?
            .into_temp_path()
            .keep()?;
        write_wav(&wav_path, &samples, target_sample_rate)?;
        Ok(Recording {
            wav_path,
            duration_seconds,
            sample_rate: target_sample_rate,
            samples,
            peak,
            rms,
        })
    }

    pub fn cancel(&self) {
        let _ = self.active.lock().unwrap().take();
    }
}

pub fn input_devices() -> Result<Vec<AudioDeviceInfo>> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|device| device.name().ok());
    let devices = host
        .input_devices()?
        .enumerate()
        .map(|(index, device)| {
            let name = device
                .name()
                .unwrap_or_else(|_| format!("麦克风 {}", index));
            let is_default = default_name
                .as_ref()
                .is_some_and(|default| default == &name);
            AudioDeviceInfo {
                index,
                name,
                is_default,
            }
        })
        .collect();
    Ok(devices)
}

pub fn measure_input_level(
    input_device_name: Option<&str>,
    duration: Duration,
) -> Result<AudioLevelInfo> {
    let duration = duration.clamp(Duration::from_millis(80), Duration::from_millis(1000));
    let _open_guard = AUDIO_STREAM_OPEN_LOCK.lock();
    let host = cpal::default_host();
    let (device, device_name) = select_input_device(&host, input_device_name)?;
    let samples = Arc::new(Mutex::new(Vec::<f32>::with_capacity(24_000)));
    let (stream, sample_rate) = build_capture_stream(&device, samples.clone())?;
    stream.play().context("启动麦克风电平检测失败")?;
    std::thread::sleep(duration);
    drop(stream);

    let samples = samples.lock().unwrap();
    let (peak, rms) = sample_stats(&samples);
    Ok(AudioLevelInfo {
        device_name,
        sample_rate,
        duration_ms: duration.as_millis() as u64,
        peak,
        rms,
        samples: samples.len(),
    })
}

pub fn write_wav(path: &PathBuf, samples: &[f32], sample_rate: u32) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: WavSampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec)?;
    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        writer.write_sample((clamped * i16::MAX as f32) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

fn select_input_device(
    host: &cpal::Host,
    input_device_name: Option<&str>,
) -> Result<(cpal::Device, String)> {
    if let Some(name) = normalized_device_name(input_device_name) {
        for device in host.input_devices()? {
            let device_name = device.name().unwrap_or_default();
            if device_name == name {
                return Ok((device, device_name));
            }
        }
        return Err(anyhow!("找不到麦克风设备：{}", name));
    }

    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("找不到默认麦克风"))?;
    let name = device.name().unwrap_or_else(|_| "默认麦克风".into());
    Ok((device, name))
}

fn normalized_device_name(value: Option<&str>) -> Option<&str> {
    let value = value?.trim();
    if value.is_empty()
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "default" | "system-default" | "auto"
        )
        || matches!(value, "默认" | "系统默认" | "自动")
    {
        None
    } else {
        Some(value)
    }
}

fn build_capture_stream(
    device: &cpal::Device,
    samples: Arc<Mutex<Vec<f32>>>,
) -> Result<(Stream, u32)> {
    let supported = device
        .default_input_config()
        .context("读取麦克风配置失败")?;
    let source_sample_rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    let stream_config = supported.config();
    let err_fn = |err| eprintln!("Voice IME audio stream error: {err}");
    let stream = match supported.sample_format() {
        SampleFormat::F32 => {
            build_input_stream_f32(device, &stream_config, samples.clone(), channels, err_fn)?
        }
        SampleFormat::I16 => {
            build_input_stream_i16(device, &stream_config, samples.clone(), channels, err_fn)?
        }
        SampleFormat::U16 => {
            build_input_stream_u16(device, &stream_config, samples.clone(), channels, err_fn)?
        }
        other => return Err(anyhow!("暂不支持此麦克风采样格式：{other:?}")),
    };
    Ok((stream, source_sample_rate))
}

fn build_input_stream_f32(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    channels: usize,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream> {
    Ok(device.build_input_stream(
        config,
        move |data: &[f32], _| push_frames_f32(data, channels, &samples),
        err_fn,
        None,
    )?)
}

fn build_input_stream_i16(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    channels: usize,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream> {
    Ok(device.build_input_stream(
        config,
        move |data: &[i16], _| {
            let converted = data
                .iter()
                .map(|s| *s as f32 / i16::MAX as f32)
                .collect::<Vec<_>>();
            push_frames_f32(&converted, channels, &samples);
        },
        err_fn,
        None,
    )?)
}

fn build_input_stream_u16(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    channels: usize,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream> {
    Ok(device.build_input_stream(
        config,
        move |data: &[u16], _| {
            let converted = data
                .iter()
                .map(|s| (*s as f32 - 32768.0) / 32768.0)
                .collect::<Vec<_>>();
            push_frames_f32(&converted, channels, &samples);
        },
        err_fn,
        None,
    )?)
}

fn push_frames_f32(data: &[f32], channels: usize, samples: &Arc<Mutex<Vec<f32>>>) {
    let mut out = samples.lock().unwrap();
    if channels <= 1 {
        out.extend_from_slice(data);
        return;
    }
    for frame in data.chunks(channels) {
        out.push(frame.iter().copied().sum::<f32>() / frame.len() as f32);
    }
}

fn sample_stats(samples: &[f32]) -> (f32, f32) {
    let peak = samples
        .iter()
        .fold(0.0_f32, |acc, item| acc.max(item.abs()));
    let rms = if samples.is_empty() {
        0.0
    } else {
        (samples.iter().map(|v| v * v).sum::<f32>() / samples.len() as f32).sqrt()
    };
    (peak, rms)
}

fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if samples.is_empty() || from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = ((samples.len() as f64) / ratio).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let left = src_pos.floor() as usize;
        let right = (left + 1).min(samples.len() - 1);
        let frac = (src_pos - left as f64) as f32;
        out.push(samples[left] * (1.0 - frac) + samples[right] * frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{normalized_device_name, resample_linear, sample_stats};

    #[test]
    fn resample_changes_length() {
        let source = vec![0.0; 48_000];
        let out = resample_linear(&source, 48_000, 16_000);
        assert!((15_900..=16_100).contains(&out.len()));
    }

    #[test]
    fn default_device_aliases_are_empty_selection() {
        assert_eq!(normalized_device_name(None), None);
        assert_eq!(normalized_device_name(Some("")), None);
        assert_eq!(normalized_device_name(Some(" default ")), None);
        assert_eq!(normalized_device_name(Some("系统默认")), None);
        assert_eq!(normalized_device_name(Some("Mic 1")), Some("Mic 1"));
    }

    #[test]
    fn sample_stats_report_peak_and_rms() {
        let (peak, rms) = sample_stats(&[0.0, 0.5, -1.0, 0.5]);
        assert_eq!(peak, 1.0);
        assert!((0.60..0.62).contains(&rms));

        let (peak, rms) = sample_stats(&[]);
        assert_eq!(peak, 0.0);
        assert_eq!(rms, 0.0);
    }
}
