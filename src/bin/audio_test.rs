use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn out(file: &mut std::fs::File, msg: &str) {
    println!("{msg}");
    let _ = writeln!(file, "{msg}");
}

fn main() {
    let host = cpal::default_host();
    let path = timestamped_log_path();
    let mut f = std::fs::File::create(&path).expect("failed to create log file");

    out(&mut f, "=== Audio Device Test ===\n");
    list_input_devices(&host, &mut f);
    list_output_devices(&host, &mut f);
    test_default_input(&host, &mut f);
    test_default_output(&host, &mut f);
    test_all_inputs(&host, &mut f);
    println!("\nLog saved to: {path}");
}

fn timestamped_log_path() -> String {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    format!("audio_test_{ts}.log")
}

fn device_desc(d: &cpal::Device) -> String {
    d.description().map(|d| d.to_string()).unwrap_or_else(|_| "unknown".into())
}

fn list_input_devices(host: &cpal::Host, f: &mut std::fs::File) {
    out(f, "--- Input Devices ---");
    let Ok(devices) = host.input_devices() else { return };
    for (i, d) in devices.enumerate() {
        let name = device_desc(&d);
        let cfg = d.default_input_config().map(|c| format!("{c:?}")).unwrap_or_else(|e| format!("err: {e}"));
        out(f, &format!("  [{i}] {name}  ({cfg})"));
    }
}

fn list_output_devices(host: &cpal::Host, f: &mut std::fs::File) {
    out(f, "\n--- Output Devices ---");
    let Ok(devices) = host.output_devices() else { return };
    for (i, d) in devices.enumerate() {
        let name = device_desc(&d);
        let cfg = d.default_output_config().map(|c| format!("{c:?}")).unwrap_or_else(|e| format!("err: {e}"));
        out(f, &format!("  [{i}] {name}  ({cfg})"));
    }
}

fn test_default_input(host: &cpal::Host, f: &mut std::fs::File) {
    out(f, "\n--- Default Input ---");
    let Some(d) = host.default_input_device() else { out(f, "  (none)"); return };
    let name = device_desc(&d);
    out(f, &format!("  {name}"));
    test_capture(&d, true, &name, f);
}

fn test_default_output(host: &cpal::Host, f: &mut std::fs::File) {
    out(f, "\n--- Default Output (loopback test) ---");
    let Some(d) = host.default_output_device() else { out(f, "  (none)"); return };
    let name = device_desc(&d);
    out(f, &format!("  {name}"));
    test_capture(&d, false, &name, f);
}

fn test_all_inputs(host: &cpal::Host, f: &mut std::fs::File) {
    out(f, "\n--- Testing All Input Devices (2s each) ---");
    let Ok(devices) = host.input_devices() else { return };
    for d in devices {
        let name = device_desc(&d);
        test_capture(&d, true, &name, f);
    }
}

fn test_capture(device: &cpal::Device, is_input: bool, name: &str, f: &mut std::fs::File) {
    let config = if is_input { device.default_input_config() } else { device.default_output_config() };
    let config = match config {
        Ok(c) => c,
        Err(e) => { out(f, &format!("  [{name}] SKIP - no config: {e}")); return; }
    };
    run_capture(device, config, name, f);
}

fn run_capture(device: &cpal::Device, config: cpal::SupportedStreamConfig, name: &str, f: &mut std::fs::File) {
    let buf: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let stream = match build_capture_stream(device, config, buf.clone()) {
        Ok(s) => s,
        Err(msg) => { out(f, &format!("  [{name}] FAIL - {msg}")); return; }
    };
    capture_loop(&stream, &buf, name, f);
    drop(stream);
}

fn build_capture_stream(
    device: &cpal::Device,
    config: cpal::SupportedStreamConfig,
    buf: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, String> {
    let stream_config: cpal::StreamConfig = config.clone().into();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_f32_stream(device, &stream_config, buf),
        cpal::SampleFormat::I16 => build_i16_stream(device, &stream_config, buf),
        fmt => return Err(format!("unsupported format: {fmt:?}")),
    };
    let stream = stream.map_err(|e| format!("build stream: {e}"))?;
    stream.play().map_err(|e| format!("play: {e}"))?;
    Ok(stream)
}

fn build_f32_stream(
    device: &cpal::Device, config: &cpal::StreamConfig, buf: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    device.build_input_stream(
        config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| { buf.lock().unwrap().extend_from_slice(data); },
        |err| eprintln!("  stream error: {err}"),
        None,
    )
}

fn build_i16_stream(
    device: &cpal::Device, config: &cpal::StreamConfig, buf: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    device.build_input_stream(
        config,
        move |data: &[i16], _: &cpal::InputCallbackInfo| {
            let floats: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
            buf.lock().unwrap().extend_from_slice(&floats);
        },
        |err| eprintln!("  stream error: {err}"),
        None,
    )
}

fn capture_loop(
    _stream: &cpal::Stream, buf: &Arc<Mutex<Vec<f32>>>, name: &str, f: &mut std::fs::File,
) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(500));
        let mut data = buf.lock().unwrap();
        let msg = format_capture_stats(name, &data);
        out(f, &msg);
        data.clear();
    }
}

fn format_capture_stats(name: &str, data: &[f32]) -> String {
    if data.is_empty() {
        return format!("  [{name}] ... 0 samples (no data)");
    }
    let max = data.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let rms = (data.iter().map(|s| s * s).sum::<f32>() / data.len() as f32).sqrt();
    format!("  [{name}] {} samples | max={max:.6} rms={rms:.6}", data.len())
}
