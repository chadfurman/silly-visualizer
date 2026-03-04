use crate::audio::DeviceKind;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::{
    cursor, event, execute, queue, style, terminal,
};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct LiveDevice {
    name: String,
    kind: &'static str,
    is_input: bool,
    selected: bool,
    level: Arc<Mutex<f32>>,
    _stream: Option<cpal::Stream>,
    device_kind: Option<DeviceKind>,
}

/// Run the interactive device picker. Returns selected devices.
pub fn run() -> Vec<crate::audio::DeviceInfo> {
    let mut devices = build_device_list();
    if devices.is_empty() {
        println!("No audio devices found.");
        return Vec::new();
    }
    start_all_captures(&mut devices);
    let result = interactive_loop(&mut devices);
    stop_all_captures(&mut devices);
    result
}

fn build_device_list() -> Vec<LiveDevice> {
    let host = cpal::default_host();
    let mut devices = Vec::new();
    add_system_audio_entry(&mut devices);
    add_input_devices(&host, &mut devices);
    add_output_devices(&host, &mut devices);
    devices
}

fn add_system_audio_entry(devices: &mut Vec<LiveDevice>) {
    devices.push(LiveDevice {
        name: "System Audio (ScreenCaptureKit)".into(),
        kind: "system",
        is_input: false,
        selected: false,
        level: Arc::new(Mutex::new(0.0)),
        _stream: None,
        device_kind: Some(DeviceKind::SystemAudio),
    });
}

fn add_input_devices(host: &cpal::Host, devices: &mut Vec<LiveDevice>) {
    let Ok(inputs) = host.input_devices() else { return };
    for d in inputs {
        devices.push(make_live_device(d, "in", true));
    }
}

fn add_output_devices(host: &cpal::Host, devices: &mut Vec<LiveDevice>) {
    let Ok(outputs) = host.output_devices() else { return };
    for d in outputs {
        devices.push(make_live_device(d, "loopback", false));
    }
}

fn make_live_device(d: cpal::Device, kind: &'static str, is_input: bool) -> LiveDevice {
    let name = device_desc(&d);
    LiveDevice {
        name, kind, is_input, selected: false,
        level: Arc::new(Mutex::new(0.0)),
        _stream: None, device_kind: Some(DeviceKind::Cpal(d)),
    }
}

fn device_desc(d: &cpal::Device) -> String {
    crate::audio::device_description(d)
}

fn start_all_captures(devices: &mut [LiveDevice]) {
    for dev in devices.iter_mut() {
        if matches!(dev.device_kind, Some(DeviceKind::Cpal(_))) {
            dev._stream = try_start_capture(dev);
        }
    }
}

fn try_start_capture(dev: &LiveDevice) -> Option<cpal::Stream> {
    let Some(DeviceKind::Cpal(ref device)) = dev.device_kind else { return None };
    let config = if dev.is_input {
        device.default_input_config().ok()?
    } else {
        device.default_output_config().ok()?
    };
    let level = dev.level.clone();
    let stream_config: cpal::StreamConfig = config.into();
    let stream = device.build_input_stream(
        &stream_config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            update_level(&level, data);
        },
        |_| {},
        None,
    ).ok()?;
    stream.play().ok()?;
    Some(stream)
}

fn update_level(level: &Arc<Mutex<f32>>, data: &[f32]) {
    let max = data.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let mut lvl = level.lock().unwrap();
    if max > *lvl { *lvl = max; } else { *lvl *= 0.9; }
}

fn stop_all_captures(devices: &mut [LiveDevice]) {
    for dev in devices.iter_mut() {
        dev._stream = None;
    }
}

fn interactive_loop(devices: &mut [LiveDevice]) -> Vec<crate::audio::DeviceInfo> {
    let mut stdout = std::io::stdout();
    let _ = terminal::enable_raw_mode();
    let _ = execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide);

    let mut cursor = 0usize;
    let mut last_draw = Instant::now();

    loop {
        if last_draw.elapsed() >= Duration::from_millis(50) {
            draw(&mut stdout, devices, cursor);
            last_draw = Instant::now();
        }
        let done = handle_input(devices, &mut cursor);
        if done { break; }
    }

    let _ = execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show);
    let _ = terminal::disable_raw_mode();
    collect_selected(devices)
}

/// Returns true when the user confirms or cancels.
fn handle_input(devices: &mut [LiveDevice], cursor: &mut usize) -> bool {
    let Some(action) = poll_input() else { return false };
    match action {
        Action::Up => *cursor = cursor.saturating_sub(1),
        Action::Down => *cursor = (*cursor + 1).min(devices.len() - 1),
        Action::Toggle => devices[*cursor].selected = !devices[*cursor].selected,
        Action::Confirm => return true,
        Action::Cancel => {
            for d in devices.iter_mut() { d.selected = false; }
            return true;
        }
    }
    false
}

enum Action { Up, Down, Toggle, Confirm, Cancel }

fn poll_input() -> Option<Action> {
    if !event::poll(Duration::from_millis(30)).unwrap_or(false) {
        return None;
    }
    let Ok(event::Event::Key(k)) = event::read() else { return None };
    match k.code {
        event::KeyCode::Up => Some(Action::Up),
        event::KeyCode::Down => Some(Action::Down),
        event::KeyCode::Char(' ') => Some(Action::Toggle),
        event::KeyCode::Enter => Some(Action::Confirm),
        event::KeyCode::Esc | event::KeyCode::Char('q') => Some(Action::Cancel),
        _ => None,
    }
}

fn collect_selected(devices: &mut [LiveDevice]) -> Vec<crate::audio::DeviceInfo> {
    devices.iter_mut()
        .filter(|d| d.selected)
        .filter_map(|d| {
            d.device_kind.take().map(|device_kind| crate::audio::DeviceInfo {
                name: d.name.clone(),
                kind: d.kind,
                device_kind,
                is_input: d.is_input,
            })
        })
        .collect()
}

const HEADER_LINES: u16 = 5;

fn term_size() -> (u16, u16) {
    terminal::size().unwrap_or((80, 24))
}

fn draw(stdout: &mut impl Write, devices: &[LiveDevice], cursor: usize) {
    let (w, _) = term_size();
    let _ = queue!(stdout, cursor::MoveTo(0, 0), terminal::Clear(terminal::ClearType::All));
    draw_header(stdout, w);
    for (i, dev) in devices.iter().enumerate() {
        draw_device_row(stdout, dev, i, cursor, w);
    }
    let _ = stdout.flush();
}

fn draw_header(stdout: &mut impl Write, w: u16) {
    let _ = queue!(stdout, cursor::MoveTo(1, 0));
    let _ = queue!(stdout, style::SetAttribute(style::Attribute::Bold));
    let _ = queue!(stdout, style::Print("Audio Device Picker"));
    let _ = queue!(stdout, style::SetAttribute(style::Attribute::Reset));
    let _ = queue!(stdout, cursor::MoveTo(1, 1));
    let _ = queue!(stdout, style::SetForegroundColor(style::Color::DarkGrey));
    let _ = queue!(stdout, style::Print("Space=toggle  Enter=confirm  Esc=cancel"));
    let _ = queue!(stdout, cursor::MoveTo(1, 2));
    let _ = queue!(stdout, style::Print("Loopback meters may not work on all devices"));
    let _ = queue!(stdout, style::SetForegroundColor(style::Color::Reset));
    let _ = queue!(stdout, cursor::MoveTo(1, 3));
    let rule_len = (w as usize).saturating_sub(2).min(72);
    let _ = queue!(stdout, style::Print("\u{2500}".repeat(rule_len)));
}

fn draw_device_row(stdout: &mut impl Write, dev: &LiveDevice, i: usize, cur: usize, w: u16) {
    let row = HEADER_LINES + i as u16;
    let is_cur = i == cur;
    if is_cur {
        let _ = queue!(stdout, style::SetAttribute(style::Attribute::Bold));
    }
    draw_row_prefix(stdout, dev, is_cur, row);
    draw_row_name(stdout, dev, row, w);
    draw_row_meter(stdout, dev, row, w);
    if is_cur {
        let _ = queue!(stdout, style::SetAttribute(style::Attribute::Reset));
    }
}

fn draw_row_prefix(stdout: &mut impl Write, dev: &LiveDevice, is_cur: bool, row: u16) {
    let arrow = if is_cur { "\u{25b6}" } else { " " };
    let check = if dev.selected { "x" } else { " " };
    let _ = queue!(stdout, cursor::MoveTo(1, row));
    let _ = queue!(stdout, style::Print(format!("{arrow} [{check}]")));
}

fn draw_row_name(stdout: &mut impl Write, dev: &LiveDevice, row: u16, w: u16) {
    let tag = format!("({})", dev.kind);
    let bar_w = bar_width(w);
    let name_end = (w as usize).saturating_sub(bar_w + 1);
    let tag_start = name_end.saturating_sub(tag.len());
    let name_budget = tag_start.saturating_sub(8);
    let name = truncate(&dev.name, name_budget);
    let _ = queue!(stdout, cursor::MoveTo(7, row), style::Print(name));
    let _ = queue!(stdout, cursor::MoveTo(tag_start as u16, row));
    let _ = queue!(stdout, style::SetForegroundColor(style::Color::DarkGrey));
    let _ = queue!(stdout, style::Print(tag));
    let _ = queue!(stdout, style::SetForegroundColor(style::Color::Reset));
}

fn draw_row_meter(stdout: &mut impl Write, dev: &LiveDevice, row: u16, w: u16) {
    let bw = bar_width(w);
    let col = (w as usize).saturating_sub(bw) as u16;
    let _ = queue!(stdout, cursor::MoveTo(col, row));
    if matches!(dev.device_kind, Some(DeviceKind::SystemAudio)) {
        let _ = queue!(stdout, style::SetForegroundColor(style::Color::Cyan));
        let _ = queue!(stdout, style::Print("[SCK]"));
        let _ = queue!(stdout, style::SetForegroundColor(style::Color::Reset));
    } else {
        draw_level_bar(stdout, *dev.level.lock().unwrap(), bw);
    }
}

fn draw_level_bar(stdout: &mut impl Write, level: f32, width: usize) {
    let db = if level > 0.0 { (20.0 * level.log10()).max(-60.0) } else { -60.0 };
    let filled = (((db + 60.0) / 60.0) * width as f32).clamp(0.0, width as f32) as usize;
    let color = bar_color(filled, width);
    let _ = queue!(stdout, style::SetForegroundColor(color));
    let _ = queue!(stdout, style::Print("\u{2588}".repeat(filled)));
    let _ = queue!(stdout, style::SetForegroundColor(style::Color::DarkGrey));
    let _ = queue!(stdout, style::Print("\u{2591}".repeat(width - filled)));
    let _ = queue!(stdout, style::SetForegroundColor(style::Color::Reset));
}

fn bar_color(filled: usize, width: usize) -> style::Color {
    if filled * 3 > width * 2 {
        style::Color::Red
    } else if filled * 3 > width {
        style::Color::Yellow
    } else {
        style::Color::Green
    }
}

fn bar_width(term_w: u16) -> usize {
    if term_w >= 100 { 25 } else if term_w >= 70 { 15 } else { 10 }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    if max <= 3 { return s.chars().take(max).collect(); }
    let mut t: String = s.chars().take(max - 1).collect();
    t.push('\u{2026}');
    t
}
