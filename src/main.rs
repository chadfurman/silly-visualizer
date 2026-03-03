mod analysis;
mod audio;
mod renderer;

use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use analysis::AudioAnalyzer;
use audio::{AudioCapture, AudioSource};
use renderer::{AudioUniforms, Renderer};

/// Decay rate for beat indicator (drops from 1.0 to 0 over several frames).
const BEAT_DECAY: f32 = 0.15;
/// Smoothing factor for audio values (0 = no smoothing, 1 = frozen).
const SMOOTH_RETAIN: f32 = 0.93;
const SMOOTH_INCOMING: f32 = 1.0 - SMOOTH_RETAIN;
/// Auto-gain: target energy level and limits.
const TARGET_ENERGY: f32 = 0.05;
const MAX_GAIN: f32 = 10.0;
/// Auto-gain attack/release rates (asymmetric: fast attack, slow release).
const GAIN_ATTACK: f32 = 0.04;
const GAIN_RELEASE: f32 = 0.005;
/// Number of color palettes available in the shader.
const NUM_PALETTES: u32 = 6;
const PALETTE_NAMES: [&str; NUM_PALETTES as usize] =
    ["Electric Neon", "Inferno", "Deep Ocean", "Vaporwave", "Acid", "Monochrome"];

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    audio: Option<AudioCapture>,
    analyzer: Option<AudioAnalyzer>,
    uniforms: AudioUniforms,
    sensitivity: f32,
    audio_source: AudioSource,
    sample_buf: Vec<f32>,
    peak_energy: f32,
    auto_gain: f32,
    debug_mode: bool,
    frame_count: u32,
    fps_update_time: Instant,
    last_frame_time: Instant,
}

impl App {
    /// Create (or recreate) the audio capture for the current `audio_source`.
    ///
    /// On failure the source may be changed to a fallback. Returns the capture
    /// on success, or `None` if no audio device is available.
    fn create_audio_capture(&mut self) -> Option<AudioCapture> {
        let capture = match self.audio_source {
            AudioSource::Mic => AudioCapture::new_default_input(),
            AudioSource::Loopback => AudioCapture::new_loopback().or_else(|e| {
                log::warn!("loopback failed ({e}), falling back to mic");
                self.audio_source = AudioSource::Mic;
                AudioCapture::new_default_input()
            }),
        };

        match capture {
            Ok(c) => {
                log::info!("audio capture started ({:?})", self.audio_source);
                Some(c)
            }
            Err(e) => {
                log::warn!("no audio device available ({e}), running silent");
                None
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes().with_title("silly visualizer");
            let window = Arc::new(event_loop.create_window(attrs).unwrap());
            let renderer = Renderer::new(window.clone());
            self.window = Some(window);
            self.renderer = Some(renderer);
        }
        if self.audio.is_none() {
            self.audio = self.create_audio_capture();
            self.analyzer = Some(AudioAnalyzer::new(2048));
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                // Skip rendering when minimized (surface size 0)
                if let Some(renderer) = &self.renderer
                    && renderer.surface_size() == (0, 0)
                {
                    // Still request redraw so we resume when un-minimized
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    return;
                }

                // Analyze audio and apply exponential smoothing
                if let (Some(audio), Some(analyzer)) = (&self.audio, &mut self.analyzer) {
                    audio.get_samples_into(&mut self.sample_buf);
                    if !self.sample_buf.is_empty() {
                        let result = analyzer.analyze(&self.sample_buf);

                        // Auto-gain: boost quiet signals (vocals, speech) so
                        // they still drive the visualization. Uses asymmetric
                        // attack/release — responds quickly to loud signals,
                        // holds gain up during quiet passages.
                        if result.energy > self.peak_energy {
                            self.peak_energy +=
                                (result.energy - self.peak_energy) * GAIN_ATTACK;
                        } else {
                            self.peak_energy +=
                                (result.energy - self.peak_energy) * GAIN_RELEASE;
                        }
                        self.peak_energy = self.peak_energy.max(0.001);
                        let target_gain =
                            (TARGET_ENERGY / self.peak_energy).clamp(1.0, MAX_GAIN);
                        self.auto_gain += (target_gain - self.auto_gain) * 0.02;

                        let gain = self.auto_gain * self.sensitivity;

                        // Smooth continuous values (bass, mids, highs, energy)
                        self.uniforms.bass = self.uniforms.bass * SMOOTH_RETAIN
                            + result.bass * gain * SMOOTH_INCOMING;
                        self.uniforms.mids = self.uniforms.mids * SMOOTH_RETAIN
                            + result.mids * gain * SMOOTH_INCOMING;
                        self.uniforms.highs = self.uniforms.highs * SMOOTH_RETAIN
                            + result.highs * gain * SMOOTH_INCOMING;
                        self.uniforms.energy = self.uniforms.energy * SMOOTH_RETAIN
                            + result.energy * gain * SMOOTH_INCOMING;

                        // Beat: snap to 1.0 on detection, otherwise decay toward 0
                        if result.beat > 0.5 {
                            self.uniforms.beat = 1.0;
                        } else {
                            self.uniforms.beat = (self.uniforms.beat - BEAT_DECAY).max(0.0);
                        }

                        self.uniforms.bands = result.bands;
                    }
                }

                if let Some(renderer) = &mut self.renderer {
                    renderer.render(&mut self.uniforms);
                }

                // FPS counter
                self.frame_count += 1;
                let now = Instant::now();
                let elapsed = now.duration_since(self.fps_update_time).as_secs_f32();
                if self.debug_mode && elapsed >= 0.5 {
                    let fps = self.frame_count as f32 / elapsed;
                    let frame_ms = now.duration_since(self.last_frame_time).as_secs_f32() * 1000.0;
                    if let Some(window) = &self.window {
                        window.set_title(&format!(
                            "silly visualizer | {fps:.0} FPS | {frame_ms:.1} ms"
                        ));
                    }
                    self.frame_count = 0;
                    self.fps_update_time = now;
                }
                self.last_frame_time = now;

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    match event.logical_key.as_ref() {
                        Key::Named(NamedKey::Escape) => event_loop.exit(),
                        Key::Named(NamedKey::Space) => {
                            self.audio_source = match self.audio_source {
                                AudioSource::Mic => AudioSource::Loopback,
                                AudioSource::Loopback => AudioSource::Mic,
                            };
                            // Drop current capture, create new one with toggled source
                            self.audio = None;
                            self.audio = self.create_audio_capture();
                        }
                        Key::Character("f") => {
                            if let Some(window) = &self.window {
                                if window.fullscreen().is_some() {
                                    window.set_fullscreen(None);
                                } else {
                                    window.set_fullscreen(Some(
                                        winit::window::Fullscreen::Borderless(None),
                                    ));
                                }
                            }
                        }
                        Key::Character("r") => {
                            if let Some(renderer) = &mut self.renderer {
                                renderer.randomize_seed();
                            }
                        }
                        Key::Character("d") => {
                            self.debug_mode = !self.debug_mode;
                            if !self.debug_mode {
                                if let Some(window) = &self.window {
                                    window.set_title("silly visualizer");
                                }
                            }
                            log::info!("debug mode: {}", self.debug_mode);
                        }
                        Key::Character("c") => {
                            let id = (self.uniforms.palette_id as u32 + 1) % NUM_PALETTES;
                            self.uniforms.palette_id = id as f32;
                            log::info!("palette: {} ({})", PALETTE_NAMES[id as usize], id);
                        }
                        Key::Character(c) => {
                            if let Some(digit) =
                                c.chars().next().and_then(|ch| ch.to_digit(10))
                                && (1..=9).contains(&digit)
                            {
                                self.sensitivity = digit as f32 / 5.0;
                                log::info!("sensitivity: {:.1}", self.sensitivity);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = App {
        window: None,
        renderer: None,
        audio: None,
        analyzer: None,
        uniforms: AudioUniforms::default(),
        sensitivity: 1.0,
        audio_source: AudioSource::Mic,
        sample_buf: Vec::with_capacity(4096),
        peak_energy: 0.01,
        auto_gain: 1.0,
        debug_mode: false,
        frame_count: 0,
        fps_update_time: Instant::now(),
        last_frame_time: Instant::now(),
    };
    event_loop.run_app(&mut app).unwrap();
}
