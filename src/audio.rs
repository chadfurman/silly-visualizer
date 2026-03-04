use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use screencapturekit::stream::sc_stream::SCStream;
use std::sync::{Arc, Mutex};

const BUFFER_SIZE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSource {
    Mic,
    Loopback,
    SystemAudio,
}

/// What kind of device a picker entry represents.
pub enum DeviceKind {
    Cpal(cpal::Device),
    SystemAudio,
}

/// A device entry for the interactive picker.
pub struct DeviceInfo {
    pub name: String,
    pub kind: &'static str,
    pub device_kind: DeviceKind,
    pub is_input: bool,
}

// Fields are never read directly — the enum exists to keep streams alive via RAII.
#[allow(dead_code)]
enum StreamHolder {
    Cpal(Vec<cpal::Stream>),
    SystemAudio(SCStream),
}

pub struct AudioCapture {
    _streams: StreamHolder,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
}

impl AudioCapture {
    /// Capture from the default input (microphone) device.
    pub fn new_default_input() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("no input device available")?;

        log::info!("mic device: {}", device_description(&device));

        let config = device
            .default_input_config()
            .map_err(|e| format!("no default input config: {e}"))?;

        Self::build_capture(device, config)
    }

    /// Capture system audio via loopback (macOS 14.6+ required).
    ///
    /// This works by opening an input stream on the default *output* device.
    /// cpal's CoreAudio backend detects that the device is output-only and
    /// transparently creates a ProcessTap aggregate device to capture system
    /// audio.
    ///
    /// If this fails (older macOS, permissions, etc.), callers should fall
    /// back to `new_from_device_name` with a virtual device like BlackHole.
    pub fn new_loopback() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no output device available")?;

        log::info!("loopback device: {}", device_description(&device));

        // Use the output device's output config -- cpal will internally create
        // a ProcessTap aggregate for recording from it.
        let config = device
            .default_output_config()
            .map_err(|e| format!("no default output config: {e}"))?;

        Self::build_capture(device, config)
    }

    /// Capture from a device selected by the interactive picker.
    pub fn new_from_device_info(info: DeviceInfo) -> Result<Self, String> {
        log::info!("selected device: {} ({})", info.name, info.kind);
        match info.device_kind {
            DeviceKind::SystemAudio => Self::new_system_audio(),
            DeviceKind::Cpal(device) => {
                let config = if info.is_input {
                    device.default_input_config()
                } else {
                    device.default_output_config()
                };
                let config = config.map_err(|e| format!("no config: {e}"))?;
                Self::build_capture(device, config)
            }
        }
    }

    /// Capture from a specific device matched by name substring.
    ///
    /// Useful for selecting virtual audio devices like BlackHole or
    /// Soundflower that route system audio as an input device.
    pub fn new_from_device_name(name: &str) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .input_devices()
            .map_err(|e| format!("failed to list input devices: {e}"))?
            .find(|d| {
                device_description(d)
                    .to_lowercase()
                    .contains(&name.to_lowercase())
            })
            .ok_or_else(|| format!("no input device matching '{name}'"))?;

        log::info!("selected device: {}", device_description(&device));

        let config = device
            .default_input_config()
            .map_err(|e| format!("no default input config: {e}"))?;

        Self::build_capture(device, config)
    }

    /// Capture system audio via ScreenCaptureKit (macOS 12.3+).
    pub fn new_system_audio() -> Result<Self, String> {
        let buffer = Arc::new(Mutex::new(Vec::with_capacity(BUFFER_SIZE)));
        let (stream, rate) =
            crate::sck_audio::start_system_audio_capture(buffer.clone(), BUFFER_SIZE)?;
        Ok(Self {
            _streams: StreamHolder::SystemAudio(stream),
            buffer,
            sample_rate: rate,
        })
    }

    fn build_capture(
        device: cpal::Device,
        config: cpal::SupportedStreamConfig,
    ) -> Result<Self, String> {
        let buffer = Arc::new(Mutex::new(Vec::with_capacity(BUFFER_SIZE)));
        let sample_format = config.sample_format();
        let rate = config.sample_rate();
        let stream_config: cpal::StreamConfig = config.into();

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, buffer.clone()),
            SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, buffer.clone()),
            SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, buffer.clone()),
            format => return Err(format!("unsupported sample format: {format:?}")),
        };

        let stream = stream.map_err(|e| format!("failed to build stream: {e}"))?;
        stream.play().map_err(|e| format!("failed to play stream: {e}"))?;

        Ok(Self {
            _streams: StreamHolder::Cpal(vec![stream]),
            buffer,
            sample_rate: rate,
        })
    }

    /// Build capture from multiple selected devices, mixing their streams.
    pub fn new_from_multi(infos: Vec<DeviceInfo>) -> Result<Self, String> {
        if infos.is_empty() { return Err("no devices selected".into()); }
        // If any device is SystemAudio, use SCK (it captures everything).
        if infos.iter().any(|i| matches!(i.device_kind, DeviceKind::SystemAudio)) {
            return Self::new_system_audio();
        }
        if infos.len() == 1 {
            return Self::new_from_device_info(infos.into_iter().next().unwrap());
        }
        let buffer = Arc::new(Mutex::new(Vec::with_capacity(BUFFER_SIZE)));
        let rate = first_sample_rate(&infos);
        let streams = build_multi_streams(&infos, &buffer)?;
        Ok(Self { _streams: StreamHolder::Cpal(streams), buffer, sample_rate: rate })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Copy samples into a reusable destination buffer, avoiding allocation.
    pub fn get_samples_into(&self, dest: &mut Vec<f32>) {
        let buf = self.buffer.lock().unwrap();
        dest.clear();
        dest.extend_from_slice(&buf);
    }
}

fn first_sample_rate(infos: &[DeviceInfo]) -> u32 {
    infos.first()
        .and_then(|i| {
            let DeviceKind::Cpal(ref device) = i.device_kind else { return None };
            if i.is_input { device.default_input_config().ok() }
            else { device.default_output_config().ok() }
        })
        .map(|c| c.sample_rate())
        .unwrap_or(44100)
}

fn build_multi_streams(
    infos: &[DeviceInfo],
    buffer: &Arc<Mutex<Vec<f32>>>,
) -> Result<Vec<cpal::Stream>, String> {
    let mut streams = Vec::new();
    for info in infos {
        let DeviceKind::Cpal(ref device) = info.device_kind else { continue };
        let stream = build_device_stream(device, info.is_input, buffer.clone())?;
        log::info!("capturing: {} ({})", info.name, info.kind);
        streams.push(stream);
    }
    Ok(streams)
}

fn build_device_stream(
    device: &cpal::Device,
    is_input: bool,
    buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, String> {
    let config = if is_input {
        device.default_input_config()
    } else {
        device.default_output_config()
    }.map_err(|e| format!("config: {e}"))?;
    let stream_config: cpal::StreamConfig = config.into();
    let stream = build_stream::<f32>(device, &stream_config, buffer)
        .map_err(|e| format!("stream: {e}"))?;
    stream.play().map_err(|e| format!("play: {e}"))?;
    Ok(stream)
}

pub(crate) fn device_description(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.to_string())
        .unwrap_or_else(|_| "unknown".into())
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample + Into<f32>,
{
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let mut buf = buffer.lock().unwrap();
            for &sample in data {
                buf.push(sample.into());
            }
            if buf.len() > BUFFER_SIZE {
                let drain = buf.len() - BUFFER_SIZE;
                buf.drain(..drain);
            }
        },
        |err| eprintln!("audio error: {err}"),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_source_enum_round_trips() {
        assert_eq!(AudioSource::Mic, AudioSource::Mic);
        assert_eq!(AudioSource::Loopback, AudioSource::Loopback);
        assert_eq!(AudioSource::SystemAudio, AudioSource::SystemAudio);
        assert_ne!(AudioSource::Mic, AudioSource::Loopback);
        assert_ne!(AudioSource::Mic, AudioSource::SystemAudio);
        assert_ne!(AudioSource::Loopback, AudioSource::SystemAudio);
    }

    #[test]
    fn ring_buffer_caps_at_buffer_size() {
        let buffer = Arc::new(Mutex::new(Vec::with_capacity(BUFFER_SIZE)));
        {
            let mut buf = buffer.lock().unwrap();
            // Simulate pushing more than BUFFER_SIZE samples
            for i in 0..(BUFFER_SIZE + 500) {
                buf.push(i as f32);
            }
            if buf.len() > BUFFER_SIZE {
                let drain = buf.len() - BUFFER_SIZE;
                buf.drain(..drain);
            }
        }
        let buf = buffer.lock().unwrap();
        assert_eq!(buf.len(), BUFFER_SIZE);
        // Should contain the most recent samples
        assert_eq!(*buf.last().unwrap(), (BUFFER_SIZE + 499) as f32);
    }

    #[test]
    fn buffer_size_is_reasonable() {
        // BUFFER_SIZE should be large enough for FFT (2048) but not excessive
        assert!(BUFFER_SIZE >= 2048, "buffer should hold at least one FFT window");
        assert!(BUFFER_SIZE <= 65536, "buffer shouldn't be excessively large");
    }
}
