use std::sync::Arc;
use std::time::Instant;
use winit::window::Window;

use crate::analysis::AudioAnalyzer;
use crate::audio::{AudioCapture, AudioSource};
use crate::audio_processing::{self, AudioState};
use crate::lineage::Lineage;
use crate::persistence;
use crate::renderer::Renderer;
use crate::scene::{ChangeDetector, Crossfade, CrossfadeMode};
use crate::uniforms::AudioUniforms;

/// Number of color palettes available in the shader.
const NUM_PALETTES: u32 = 6;
const PALETTE_NAMES: [&str; NUM_PALETTES as usize] =
    ["Electric Neon", "Inferno", "Deep Ocean", "Vaporwave", "Acid", "Monochrome"];

pub struct App {
    pub window: Option<Arc<Window>>,
    pub renderer: Option<Renderer>,
    pub audio: Option<AudioCapture>,
    pub analyzer: Option<AudioAnalyzer>,
    pub uniforms: AudioUniforms,
    pub sensitivity: f32,
    pub audio_source: AudioSource,
    pub sample_buf: Vec<f32>,
    pub audio_state: AudioState,
    pub debug_mode: bool,
    pub debug_visual_mode: u32,
    pub frame_count: u32,
    pub fps_update_time: Instant,
    pub last_frame_time: Instant,
    pub lineage: Lineage,
    pub change_detector: ChangeDetector,
    pub crossfade: Option<Crossfade>,
    pub rng: rand::rngs::SmallRng,
}

impl App {
    pub fn create_audio_capture(&mut self) -> Option<AudioCapture> {
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

    pub fn handle_redraw(&mut self) {
        if self.is_minimized() {
            self.request_redraw();
            return;
        }
        self.process_audio_frame();
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
        let result = audio_processing::process_audio(
            audio, analyzer, &mut self.sample_buf,
            &mut self.uniforms, &mut self.audio_state, self.sensitivity,
        );
        if let Some(result) = result {
            self.check_evolution(&result.spectral_profile);
        }
    }

    fn check_evolution(&mut self, spectral_profile: &[f32; 5]) {
        let dt = self.frame_dt();
        let changed = self.change_detector.update(spectral_profile, dt);
        if changed && self.crossfade.is_none() {
            self.trigger_evolution();
        }
    }

    fn trigger_evolution(&mut self) {
        let old_genome = self.lineage.child.clone();
        self.lineage.advance(&mut self.rng, 0.5);
        let new_genome = self.lineage.child.clone();
        let mode = CrossfadeMode::from_genome_value(new_genome.transition_type);
        self.crossfade = Some(Crossfade::new(old_genome, new_genome, mode));
        log::info!("scene evolution triggered (gen {})", self.lineage.generation_count());
    }

    fn advance_crossfade_and_upload(&mut self) {
        let dt = self.frame_dt();
        let genome = match self.crossfade.as_mut() {
            Some(cf) => {
                let done = cf.advance(dt);
                let g = cf.current_genome();
                if done { self.crossfade = None; }
                g
            }
            None => self.lineage.child.clone(),
        };
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
        log::info!(
            "[debug] bass={:.2} mids={:.2} highs={:.2} energy={:.2} beat={:.0} | shapes=[{},{},{},{}] gain={:.1}x | mode={}",
            self.uniforms.bass, self.uniforms.mids, self.uniforms.highs,
            self.uniforms.energy, self.uniforms.beat,
            self.lineage.child.shape_types[0] as u32,
            self.lineage.child.shape_types[1] as u32,
            self.lineage.child.shape_types[2] as u32,
            self.lineage.child.shape_types[3] as u32,
            self.audio_state.auto_gain,
            self.debug_visual_mode,
        );
        self.frame_count = 0;
        self.fps_update_time = now;
    }

    pub fn handle_resize(&mut self, width: u32, height: u32) {
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(width, height);
        }
    }

    pub fn handle_key_space(&mut self) {
        self.audio_source = match self.audio_source {
            AudioSource::Mic => AudioSource::Loopback,
            AudioSource::Loopback => AudioSource::Mic,
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
        if !self.debug_mode
            && let Some(window) = &self.window
        {
            window.set_title("silly visualizer");
        }
        log::info!("debug mode: {}", self.debug_mode);
    }

    pub fn handle_key_palette(&mut self) {
        let id = (self.uniforms.palette_id as u32 + 1) % NUM_PALETTES;
        self.uniforms.palette_id = id as f32;
        log::info!("palette: {} ({})", PALETTE_NAMES[id as usize], id);
    }

    pub fn handle_key_evolve(&mut self) {
        if self.crossfade.is_none() {
            self.trigger_evolution();
            log::info!("manual evolution (gen {})", self.lineage.generation_count());
        }
    }

    pub fn handle_key_bookmark(&self) {
        match persistence::save_favorite(&self.lineage.child) {
            Ok(path) => log::info!("saved favorite: {}", path.display()),
            Err(e) => log::warn!("failed to save favorite: {e}"),
        }
    }

    pub fn handle_key_load_favorite(&mut self) {
        if self.crossfade.is_some() {
            return;
        }
        match persistence::load_random_favorite() {
            Ok(Some(genome)) => self.inject_favorite(genome),
            Ok(None) => log::info!("no favorites saved yet"),
            Err(e) => log::warn!("failed to load favorite: {e}"),
        }
    }

    fn inject_favorite(&mut self, genome: crate::genome::Genome) {
        let old = self.lineage.child.clone();
        self.lineage.inject(genome);
        let mode = CrossfadeMode::from_genome_value(self.lineage.child.transition_type);
        self.crossfade = Some(Crossfade::new(old, self.lineage.child.clone(), mode));
        log::info!("loaded random favorite");
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
}
