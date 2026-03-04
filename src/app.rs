use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::analysis::AudioAnalyzer;
use crate::audio::{AudioCapture, AudioSource};
use crate::audio_processing::{self, AudioState};
use crate::genome::Genome;
use crate::lineage::Lineage;
use crate::persistence;
use crate::renderer::Renderer;
use crate::scene::{ChangeDetector, Crossfade, CrossfadeMode};
use crate::uniforms::AudioUniforms;

/// Number of color palettes available in the shader.
const NUM_PALETTES: u32 = 6;
const PALETTE_NAMES: [&str; NUM_PALETTES as usize] =
    ["Electric Neon", "Inferno", "Deep Ocean", "Vaporwave", "Acid", "Monochrome"];

pub struct EvolutionState {
    pub lineage: Lineage,
    pub change_detector: ChangeDetector,
    pub crossfade: Option<Crossfade>,
    pub rng: rand::rngs::SmallRng,
}

impl EvolutionState {
    pub fn new(lineage: Lineage, rng: rand::rngs::SmallRng) -> Self {
        Self {
            lineage,
            change_detector: ChangeDetector::new(0.12, 30.0),
            crossfade: None,
            rng,
        }
    }

    pub fn check(&mut self, spectral_profile: &[f32; 5], dt: f32) {
        let changed = self.change_detector.update(spectral_profile, dt);
        if changed && self.crossfade.is_none() {
            self.trigger();
        }
    }

    pub fn trigger(&mut self) {
        let old = self.lineage.child.clone();
        self.lineage.advance_from_preset(&mut self.rng);
        let new = self.lineage.child.clone();
        let mode = CrossfadeMode::from_genome_value(new.transition_type);
        self.crossfade = Some(Crossfade::new(old, new, mode));
        self.change_detector.randomize_cooldown(&mut self.rng);
        log::info!("scene evolution → preset (gen {})", self.lineage.generation_count());
    }

    pub fn advance_crossfade(&mut self, dt: f32) -> Genome {
        match self.crossfade.as_mut() {
            Some(cf) => {
                let done = cf.advance(dt);
                let g = cf.current_genome();
                if done { self.crossfade = None; }
                g
            }
            None => self.lineage.child.clone(),
        }
    }

    pub fn inject(&mut self, genome: Genome) {
        let old = self.lineage.child.clone();
        self.lineage.inject(genome);
        let mode = CrossfadeMode::from_genome_value(self.lineage.child.transition_type);
        self.crossfade = Some(Crossfade::new(old, self.lineage.child.clone(), mode));
    }
}

pub struct App {
    pub window: Option<Arc<Window>>,
    pub renderer: Option<Renderer>,
    pub audio: Option<AudioCapture>,
    pub analyzer: Option<AudioAnalyzer>,
    pub uniforms: AudioUniforms,
    pub sensitivity: f32,
    pub audio_source: AudioSource,
    pub device_name: Option<String>,
    pub sample_buf: Vec<f32>,
    pub audio_state: AudioState,
    pub debug_mode: bool,
    pub debug_visual_mode: u32,
    pub frame_count: u32,
    pub fps_update_time: Instant,
    pub last_frame_time: Instant,
    pub evolution: EvolutionState,
    pub recorder: Option<crate::replay::AudioRecorder>,
    pub player: Option<crate::replay::AudioPlayer>,
    pub selected_devices: Vec<crate::audio::DeviceInfo>,
    pub debug_log: Option<std::fs::File>,
}

impl App {
    pub fn create_audio_capture(&mut self) -> Option<AudioCapture> {
        let capture = self.build_capture();
        match capture {
            Ok(c) => { log::info!("audio capture started"); Some(c) }
            Err(e) => { log::warn!("no audio device ({e}), running silent"); None }
        }
    }

    fn build_capture(&mut self) -> Result<AudioCapture, String> {
        let selected = std::mem::take(&mut self.selected_devices);
        if !selected.is_empty() {
            return AudioCapture::new_from_multi(selected);
        }
        if let Some(name) = &self.device_name {
            return AudioCapture::new_from_device_name(name);
        }
        self.build_default_capture()
    }

    fn build_default_capture(&mut self) -> Result<AudioCapture, String> {
        match self.audio_source {
            AudioSource::Mic => AudioCapture::new_default_input(),
            AudioSource::Loopback => AudioCapture::new_loopback().or_else(|e| {
                log::warn!("loopback failed ({e}), falling back to mic");
                self.audio_source = AudioSource::Mic;
                AudioCapture::new_default_input()
            }),
            AudioSource::SystemAudio => AudioCapture::new_system_audio().or_else(|e| {
                log::warn!("system audio failed ({e}), falling back to loopback");
                self.audio_source = AudioSource::Loopback;
                AudioCapture::new_loopback().or_else(|e2| {
                    log::warn!("loopback failed ({e2}), falling back to mic");
                    self.audio_source = AudioSource::Mic;
                    AudioCapture::new_default_input()
                })
            }),
        }
    }

    pub fn handle_redraw(&mut self) {
        if self.is_minimized() {
            self.request_redraw();
            return;
        }
        if self.player.is_some() {
            self.process_replay_frame();
        } else {
            self.process_audio_frame();
        }
        self.advance_crossfade_and_upload();
        self.uniforms.debug_flags = self.debug_visual_mode as f32;
        if let Some(renderer) = &mut self.renderer {
            renderer.render(&mut self.uniforms);
        }
        self.update_fps_counter();
        self.last_frame_time = Instant::now();
        self.request_redraw();
    }

    fn is_minimized(&self) -> bool {
        self.renderer.as_ref().is_some_and(|r| r.surface_size() == (0, 0))
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn process_audio_frame(&mut self) {
        let (Some(audio), Some(analyzer)) = (&self.audio, &mut self.analyzer) else {
            return;
        };
        audio.get_samples_into(&mut self.sample_buf);
        if self.sample_buf.is_empty() {
            return;
        }
        if let Some(rec) = &mut self.recorder {
            rec.push_samples(&self.sample_buf);
        }
        let result = audio_processing::process_samples(
            &self.sample_buf, analyzer,
            &mut self.uniforms, &mut self.audio_state, self.sensitivity,
        );
        let dt = self.frame_dt();
        audio_processing::update_envelopes(&mut self.uniforms, &mut self.audio_state, dt);
        self.check_evolution(&result.spectral_profile);
    }

    fn process_replay_frame(&mut self) {
        let Some(player) = &mut self.player else { return };
        let Some(analyzer) = &mut self.analyzer else { return };
        let chunk = match player.next_chunk() {
            Some(c) => c.to_vec(),
            None => return,
        };
        audio_processing::process_samples(
            &chunk, analyzer,
            &mut self.uniforms, &mut self.audio_state, self.sensitivity,
        );
        let dt = self.frame_dt();
        audio_processing::update_envelopes(&mut self.uniforms, &mut self.audio_state, dt);
    }

    fn check_evolution(&mut self, spectral_profile: &[f32; 5]) {
        if self.player.is_some() {
            return;
        }
        self.evolution.check(spectral_profile, self.frame_dt());
    }

    fn advance_crossfade_and_upload(&mut self) {
        let genome = self.evolution.advance_crossfade(self.frame_dt());
        if let Some(renderer) = &self.renderer {
            renderer.update_scene_uniforms(&genome.to_uniforms());
        }
    }

    fn frame_dt(&self) -> f32 {
        Instant::now().duration_since(self.last_frame_time).as_secs_f32()
    }

    fn update_fps_counter(&mut self) {
        self.frame_count += 1;
        let now = Instant::now();
        let elapsed = now.duration_since(self.fps_update_time).as_secs_f32();
        if !self.debug_mode || elapsed < 0.5 {
            return;
        }
        let fps = self.frame_count as f32 / elapsed;
        let frame_ms = now.duration_since(self.last_frame_time).as_secs_f32() * 1000.0;
        if let Some(window) = &self.window {
            window.set_title(&format!("silly visualizer | {fps:.0} FPS | {frame_ms:.1} ms"));
        }
        self.log_debug_audio();
        self.frame_count = 0;
        self.fps_update_time = now;
    }

    fn log_debug_audio(&mut self) {
        let s = &self.evolution.lineage.child.shape_types;
        let max_abs = self.sample_buf.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        let line = format!(
            "bass={:.4} mids={:.4} highs={:.4} energy={:.4} beat={:.0} | raw_max={:.6} buf={} | shapes=[{},{},{},{}] gain={:.1}x src={:?} mode={}",
            self.uniforms.bass, self.uniforms.mids, self.uniforms.highs,
            self.uniforms.energy, self.uniforms.beat,
            max_abs, self.sample_buf.len(),
            s[0] as u32, s[1] as u32, s[2] as u32, s[3] as u32,
            self.audio_state.auto_gain, self.audio_source, self.debug_visual_mode,
        );
        log::info!("[debug] {line}");
        if let Some(f) = &mut self.debug_log {
            let _ = writeln!(f, "{line}");
        }
    }

    pub fn handle_resize(&mut self, width: u32, height: u32) {
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(width, height);
        }
    }

    pub fn handle_key_space(&mut self) {
        self.audio_source = match self.audio_source {
            AudioSource::Mic => AudioSource::Loopback,
            AudioSource::Loopback => AudioSource::SystemAudio,
            AudioSource::SystemAudio => AudioSource::Mic,
        };
        self.audio = None;
        self.audio = self.create_audio_capture();
    }

    pub fn handle_key_fullscreen(&self) {
        let Some(window) = &self.window else { return };
        if window.fullscreen().is_some() {
            window.set_fullscreen(None);
        } else {
            window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }
    }

    pub fn handle_key_randomize(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            renderer.randomize_seed();
        }
    }

    pub fn handle_key_debug(&mut self) {
        self.debug_mode = !self.debug_mode;
        if self.debug_mode {
            self.debug_log = open_debug_log();
        } else {
            if let Some(window) = &self.window {
                window.set_title("silly visualizer");
            }
            self.debug_log = None;
        }
        log::info!("debug mode: {}", self.debug_mode);
    }

    pub fn handle_key_palette(&mut self) {
        let id = (self.uniforms.palette_id as u32 + 1) % NUM_PALETTES;
        self.uniforms.palette_id = id as f32;
        log::info!("palette: {} ({})", PALETTE_NAMES[id as usize], id);
    }

    pub fn handle_key_evolve(&mut self) {
        if self.evolution.crossfade.is_none() {
            self.evolution.trigger();
        }
    }

    pub fn handle_key_bookmark(&self) {
        match persistence::save_favorite(&self.evolution.lineage.child) {
            Ok(path) => log::info!("saved favorite: {}", path.display()),
            Err(e) => log::warn!("failed to save favorite: {e}"),
        }
    }

    pub fn handle_key_load_favorite(&mut self) {
        if self.evolution.crossfade.is_some() {
            return;
        }
        let loaded = persistence::load_random_favorite();
        self.apply_loaded_favorite(loaded);
    }

    fn apply_loaded_favorite(&mut self, result: Result<Option<Genome>, String>) {
        match result {
            Ok(Some(genome)) => {
                self.evolution.inject(genome);
                log::info!("loaded random favorite");
            }
            Ok(None) => log::info!("no favorites saved yet"),
            Err(e) => log::warn!("failed to load favorite: {e}"),
        }
    }

    pub fn handle_key_visual_debug(&mut self) {
        const NUM_DEBUG_MODES: u32 = 6;
        self.debug_visual_mode = (self.debug_visual_mode + 1) % NUM_DEBUG_MODES;
        let name = match self.debug_visual_mode {
            0 => "Normal",
            1 => "No Feedback",
            2 => "No Folding",
            3 => "Normals",
            4 => "Audio Bars",
            5 => "Depth",
            _ => "Unknown",
        };
        log::info!("visual debug mode: {} ({})", self.debug_visual_mode, name);
    }

    pub fn handle_key_sensitivity(&mut self, digit: u32) {
        self.sensitivity = digit as f32 / 5.0;
        log::info!("sensitivity: {:.1}", self.sensitivity);
    }

    pub fn handle_key_replay_step(&mut self, frames: i32) {
        if let Some(player) = &mut self.player {
            player.step_frames(frames);
        }
    }

    pub fn handle_key_replay_pause(&mut self) {
        if let Some(player) = &mut self.player {
            player.toggle_pause();
        }
    }

    pub fn handle_key_record(&mut self) {
        if self.recorder.as_ref().is_some_and(|r| r.is_recording()) {
            let mut rec = self.recorder.take().unwrap();
            rec.stop();
            save_recording(&rec);
        } else {
            let rate = self.audio.as_ref().map(|a| a.sample_rate()).unwrap_or(44100);
            let mut rec = crate::replay::AudioRecorder::new(rate, 1);
            rec.start();
            self.recorder = Some(rec);
        }
    }
}

fn open_debug_log() -> Option<std::fs::File> {
    let dir = persistence::data_dir()?;
    let path = dir.join("debug.log");
    match std::fs::File::create(&path) {
        Ok(f) => {
            log::info!("debug log: {}", path.display());
            Some(f)
        }
        Err(e) => {
            log::warn!("failed to create debug log: {e}");
            None
        }
    }
}

fn save_recording(rec: &crate::replay::AudioRecorder) {
    let Some(dir) = persistence::data_dir().map(|d| d.join("recordings")) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let path = dir.join(format!("{timestamp}.svrx"));
    match rec.save(&path) {
        Ok(()) => log::info!("saved recording: {}", path.display()),
        Err(e) => log::warn!("failed to save recording: {e}"),
    }
}

impl App {
    pub fn build() -> Self {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::from_os_rng();
        let lineage = load_or_create_lineage(&mut rng);
        Self {
            window: None, renderer: None, audio: None, analyzer: None,
            uniforms: AudioUniforms::default(), sensitivity: 1.0,
            audio_source: AudioSource::SystemAudio, device_name: None,
            sample_buf: Vec::with_capacity(4096),
            audio_state: AudioState::new(), debug_mode: false, debug_visual_mode: 0,
            frame_count: 0, fps_update_time: Instant::now(), last_frame_time: Instant::now(),
            evolution: EvolutionState::new(lineage, rng),
            recorder: None, player: None, selected_devices: Vec::new(), debug_log: None,
        }
    }

    pub fn load_replay(&mut self, path: &Path) {
        match crate::replay::AudioPlayer::load(path) {
            Ok(player) => {
                log::info!("replay: {} ({} frames)", path.display(), player.total_frames());
                self.player = Some(player);
            }
            Err(e) => {
                log::error!("failed to load replay: {e}");
                std::process::exit(1);
            }
        }
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: &Key) {
        match key.as_ref() {
            Key::Named(NamedKey::Escape) => event_loop.exit(),
            Key::Named(NamedKey::Space) => self.handle_key_space(),
            Key::Named(NamedKey::ArrowLeft) => self.handle_key_replay_step(-1),
            Key::Named(NamedKey::ArrowRight) => self.handle_key_replay_step(1),
            Key::Named(NamedKey::ArrowUp) => self.handle_key_replay_step(60),
            Key::Named(NamedKey::ArrowDown) => self.handle_key_replay_step(-60),
            Key::Character(c) => self.handle_char_key(c),
            _ => {}
        }
    }

    fn handle_char_key(&mut self, c: &str) {
        match c {
            "f" => self.handle_key_fullscreen(),
            "R" => self.handle_key_record(),
            "r" => self.handle_key_randomize(),
            "d" => self.handle_key_debug(),
            "c" => self.handle_key_palette(),
            "n" => self.handle_key_evolve(),
            "b" => self.handle_key_bookmark(),
            "l" => self.handle_key_load_favorite(),
            "v" => self.handle_key_visual_debug(),
            "p" => self.handle_key_replay_pause(),
            c => self.handle_digit_key(c),
        }
    }

    fn handle_digit_key(&mut self, c: &str) {
        if let Some(digit) = c.chars().next().and_then(|ch| ch.to_digit(10))
            && (1..=9).contains(&digit)
        {
            self.handle_key_sensitivity(digit);
        }
    }
}

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
                    self.handle_key(event_loop, &event.logical_key);
                }
            }
            _ => {}
        }
    }
}

fn load_or_create_lineage(rng: &mut impl rand::Rng) -> Lineage {
    match persistence::load_lineage() {
        Ok(l) => {
            log::info!("loaded lineage ({} generations)", l.generation_count());
            l
        }
        Err(_) => {
            log::info!("starting from curated preset");
            Lineage::new(crate::presets::random_preset_mutated(rng))
        }
    }
}
