mod analysis;
mod app;
mod audio;
mod audio_processing;
mod genome;
mod lineage;
mod persistence;
mod renderer;
#[allow(dead_code)]
mod replay;
mod scene;
mod uniforms;

use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use analysis::AudioAnalyzer;
use app::App;
use audio_processing::AudioState;
use genome::Genome;
use lineage::Lineage;
use renderer::Renderer;
use scene::ChangeDetector;
use uniforms::AudioUniforms;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes().with_title("silly visualizer");
            let window = Arc::new(event_loop.create_window(attrs).unwrap());
            self.renderer = Some(Renderer::new(window.clone()));
            self.window = Some(window);
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
            WindowEvent::Resized(size) => self.handle_resize(size.width, size.height),
            WindowEvent::RedrawRequested => self.handle_redraw(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    handle_key_press(self, event_loop, &event.logical_key);
                }
            }
            _ => {}
        }
    }
}

fn handle_key_press(app: &mut App, event_loop: &ActiveEventLoop, key: &Key) {
    match key.as_ref() {
        Key::Named(NamedKey::Escape) => event_loop.exit(),
        Key::Named(NamedKey::Space) => app.handle_key_space(),
        Key::Character("f") => app.handle_key_fullscreen(),
        Key::Character("r") => app.handle_key_randomize(),
        Key::Character("d") => app.handle_key_debug(),
        Key::Character("c") => app.handle_key_palette(),
        Key::Character("n") => app.handle_key_evolve(),
        Key::Character("b") => app.handle_key_bookmark(),
        Key::Character("l") => app.handle_key_load_favorite(),
        Key::Character("v") => app.handle_key_visual_debug(),
        Key::Character(c) => handle_digit_key(app, c),
        _ => {}
    }
}

fn handle_digit_key(app: &mut App, c: &str) {
    if let Some(digit) = c.chars().next().and_then(|ch| ch.to_digit(10))
        && (1..=9).contains(&digit)
    {
        app.handle_key_sensitivity(digit);
    }
}

fn main() {
    env_logger::init();
    let mut app = build_app();
    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(&mut app).unwrap();
    if let Err(e) = persistence::save_lineage(&app.lineage) {
        log::warn!("failed to save lineage: {e}");
    }
}

fn build_app() -> App {
    use rand::SeedableRng;
    let mut rng = rand::rngs::SmallRng::from_os_rng();
    let lineage = load_or_create_lineage(&mut rng);
    new_app(rng, lineage)
}

fn new_app(rng: rand::rngs::SmallRng, lineage: Lineage) -> App {
    App {
        window: None, renderer: None, audio: None, analyzer: None,
        uniforms: AudioUniforms::default(), sensitivity: 1.0,
        audio_source: audio::AudioSource::Mic, sample_buf: Vec::with_capacity(4096),
        audio_state: AudioState::new(), debug_mode: false, debug_visual_mode: 0,
        frame_count: 0, fps_update_time: Instant::now(), last_frame_time: Instant::now(),
        lineage, change_detector: ChangeDetector::new(0.08, 8.0), crossfade: None, rng,
    }
}

fn load_or_create_lineage(rng: &mut impl rand::Rng) -> Lineage {
    match persistence::load_lineage() {
        Ok(l) => {
            log::info!("loaded lineage ({} generations)", l.generation_count());
            l
        }
        Err(_) => {
            log::info!("starting fresh lineage");
            Lineage::new(Genome::random(rng))
        }
    }
}
