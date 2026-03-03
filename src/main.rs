mod audio;
mod renderer;

use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use audio::AudioCapture;
use renderer::{AudioUniforms, Renderer};

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    audio: Option<AudioCapture>,
    uniforms: AudioUniforms,
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
                // Grab audio samples (Task 5 will feed these into FFT analysis)
                if let Some(audio) = &self.audio {
                    let _samples = audio.get_samples();
                }
                if let Some(renderer) = &self.renderer {
                    renderer.render(&mut self.uniforms);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
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
        uniforms: AudioUniforms::default(),
    };
    event_loop.run_app(&mut app).unwrap();
}
