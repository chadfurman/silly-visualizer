mod analysis;
mod audio;
mod renderer;

use std::sync::Arc;
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
const SMOOTH_RETAIN: f32 = 0.8;
const SMOOTH_INCOMING: f32 = 1.0 - SMOOTH_RETAIN;

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    audio: Option<AudioCapture>,
    analyzer: Option<AudioAnalyzer>,
    uniforms: AudioUniforms,
    sensitivity: f32,
    audio_source: AudioSource,
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
                    let samples = audio.get_samples();
                    if !samples.is_empty() {
                        let result = analyzer.analyze(&samples);

                        // Smooth continuous values (bass, mids, highs, energy)
                        self.uniforms.bass = self.uniforms.bass * SMOOTH_RETAIN
                            + result.bass * self.sensitivity * SMOOTH_INCOMING;
                        self.uniforms.mids = self.uniforms.mids * SMOOTH_RETAIN
                            + result.mids * self.sensitivity * SMOOTH_INCOMING;
                        self.uniforms.highs = self.uniforms.highs * SMOOTH_RETAIN
                            + result.highs * self.sensitivity * SMOOTH_INCOMING;
                        self.uniforms.energy = self.uniforms.energy * SMOOTH_RETAIN
                            + result.energy * self.sensitivity * SMOOTH_INCOMING;

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
    };
    event_loop.run_app(&mut app).unwrap();
}
