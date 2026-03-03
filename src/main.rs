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
use audio::AudioCapture;
use renderer::{AudioUniforms, Renderer};

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    audio: Option<AudioCapture>,
    analyzer: Option<AudioAnalyzer>,
    uniforms: AudioUniforms,
    sensitivity: f32,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes()
                .with_title("silly visualizer");
            let window = Arc::new(event_loop.create_window(attrs).unwrap());
            let renderer = Renderer::new(window.clone());
            self.window = Some(window);
            self.renderer = Some(renderer);
        }
        if self.audio.is_none() {
            self.audio = Some(AudioCapture::new_default_input());
            self.analyzer = Some(AudioAnalyzer::new(2048));
            log::info!("audio capture started");
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let (Some(audio), Some(analyzer)) =
                    (&self.audio, &mut self.analyzer)
                {
                    let samples = audio.get_samples();
                    if !samples.is_empty() {
                        let result = analyzer.analyze(&samples);
                        self.uniforms.bass = result.bass * self.sensitivity;
                        self.uniforms.mids = result.mids * self.sensitivity;
                        self.uniforms.highs = result.highs * self.sensitivity;
                        self.uniforms.energy = result.energy * self.sensitivity;
                        self.uniforms.beat = result.beat;
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
                            log::info!("toggle audio source (not yet implemented)");
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
                            if let Some(digit) = c.chars().next().and_then(|ch| ch.to_digit(10)) {
                                if digit >= 1 && digit <= 9 {
                                    self.sensitivity = digit as f32 / 5.0;
                                    log::info!("sensitivity: {:.1}", self.sensitivity);
                                }
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
    };
    event_loop.run_app(&mut app).unwrap();
}
