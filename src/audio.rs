use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use std::sync::{Arc, Mutex};

const BUFFER_SIZE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSource {
    Mic,
    Loopback,
}

pub struct AudioCapture {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
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

    /// Capture from a specific device matched by name substring.
    ///
    /// Useful for selecting virtual audio devices like BlackHole or
    /// Soundflower that route system audio as an input device.
    #[allow(dead_code)]
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

    fn build_capture(
        device: cpal::Device,
        config: cpal::SupportedStreamConfig,
    ) -> Result<Self, String> {
        let buffer = Arc::new(Mutex::new(Vec::with_capacity(BUFFER_SIZE)));
        let sample_format = config.sample_format();
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
            _stream: stream,
            buffer,
        })
    }

    pub fn get_samples(&self) -> Vec<f32> {
        self.buffer.lock().unwrap().clone()
    }
}

fn device_description(device: &cpal::Device) -> String {
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
