mod analysis;
mod app;
mod audio;
mod audio_processing;
mod device_picker;
mod genome;
mod lineage;
mod persistence;
#[allow(dead_code)] // Wired in by later tasks.
mod presets;
mod renderer;
mod replay;
mod scene;
mod sck_audio;
mod uniforms;

use winit::event_loop::EventLoop;

use app::App;

fn main() {
    env_logger::init();
    let mut app = App::build();
    app.device_name = parse_arg("--device");
    if app.device_name.is_none() {
        app.selected_devices = device_picker::run();
    }
    if let Some(path) = parse_arg("--replay") {
        app.load_replay(&std::path::PathBuf::from(&path));
    }
    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(&mut app).unwrap();
    if let Err(e) = persistence::save_lineage(&app.evolution.lineage) {
        log::warn!("failed to save lineage: {e}");
    }
}

fn parse_arg(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == flag && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
    }
    None
}
