use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use std::sync::{Arc, Mutex};

const BUFFER_SIZE: usize = 4096;

pub struct AudioCapture {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
}

impl AudioCapture {
    pub fn new_default_input() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .expect("no input device available");
        let config = device
            .default_input_config()
            .expect("no default input config");

        let buffer = Arc::new(Mutex::new(Vec::with_capacity(BUFFER_SIZE)));
        let sample_format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, &stream_config, buffer.clone()),
            SampleFormat::I16 => build_stream::<i16>(&device, &stream_config, buffer.clone()),
            SampleFormat::U16 => build_stream::<u16>(&device, &stream_config, buffer.clone()),
            format => panic!("unsupported sample format: {format:?}"),
        };

        stream.play().unwrap();
        Self {
            _stream: stream,
            buffer,
        }
    }

    pub fn get_samples(&self) -> Vec<f32> {
        self.buffer.lock().unwrap().clone()
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
) -> cpal::Stream
where
    T: cpal::SizedSample + Into<f32>,
{
    device
        .build_input_stream(
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
        .unwrap()
}
