use crate::analysis::{AnalysisResult, AudioAnalyzer};
use crate::audio::AudioCapture;
use crate::uniforms::AudioUniforms;

/// Decay rate for beat indicator (drops from 1.0 to 0 over several frames).
const BEAT_DECAY: f32 = 0.15;
/// Smoothing factor for audio values (0 = no smoothing, 1 = frozen).
const SMOOTH_RETAIN: f32 = 0.90;
const SMOOTH_INCOMING: f32 = 1.0 - SMOOTH_RETAIN;
/// Auto-gain: target energy level and limits.
const TARGET_ENERGY: f32 = 0.05;
const MAX_GAIN: f32 = 10.0;
/// Auto-gain attack/release rates (asymmetric: fast attack, slow release).
const GAIN_ATTACK: f32 = 0.04;
const GAIN_RELEASE: f32 = 0.005;
/// Noise gate: raw energy below this threshold is treated as silence.
const NOISE_GATE: f32 = 0.003;

pub struct AudioState {
    pub peak_energy: f32,
    pub auto_gain: f32,
}

impl AudioState {
    pub fn new() -> Self {
        Self { peak_energy: 0.01, auto_gain: 1.0 }
    }
}

/// Run analysis on audio samples and update uniforms. Returns the analysis
/// result for use by the change detector.
pub fn process_audio(
    audio: &AudioCapture,
    analyzer: &mut AudioAnalyzer,
    sample_buf: &mut Vec<f32>,
    uniforms: &mut AudioUniforms,
    state: &mut AudioState,
    sensitivity: f32,
) -> Option<AnalysisResult> {
    audio.get_samples_into(sample_buf);
    if sample_buf.is_empty() {
        return None;
    }
    let result = analyzer.analyze(sample_buf);
    update_auto_gain(state, result.energy);
    let gain = state.auto_gain * sensitivity;
    let gate = noise_gate(result.energy);
    apply_smoothing(uniforms, &result, gain, gate);
    apply_beat(uniforms, result.beat);
    uniforms.bands = result.bands;
    Some(result)
}

pub fn update_auto_gain(state: &mut AudioState, energy: f32) {
    if energy < NOISE_GATE {
        return;
    }
    let rate = if energy > state.peak_energy { GAIN_ATTACK } else { GAIN_RELEASE };
    state.peak_energy += (energy - state.peak_energy) * rate;
    state.peak_energy = state.peak_energy.max(0.001);
    let target_gain = (TARGET_ENERGY / state.peak_energy).clamp(1.0, MAX_GAIN);
    state.auto_gain += (target_gain - state.auto_gain) * 0.02;
}

pub fn noise_gate(energy: f32) -> f32 {
    if energy < NOISE_GATE { 0.0 } else { 1.0 }
}

pub fn apply_smoothing(
    uniforms: &mut AudioUniforms,
    result: &AnalysisResult,
    gain: f32,
    gate: f32,
) {
    let scale = gain * gate * SMOOTH_INCOMING;
    uniforms.bass = uniforms.bass * SMOOTH_RETAIN + result.bass * scale;
    uniforms.mids = uniforms.mids * SMOOTH_RETAIN + result.mids * scale;
    uniforms.highs = uniforms.highs * SMOOTH_RETAIN + result.highs * scale;
    uniforms.energy = uniforms.energy * SMOOTH_RETAIN + result.energy * scale;
}

pub fn apply_beat(uniforms: &mut AudioUniforms, beat: f32) {
    if beat > 0.5 {
        uniforms.beat = 1.0;
    } else {
        uniforms.beat = (uniforms.beat - BEAT_DECAY).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_gate_blocks_below_threshold() {
        assert_eq!(noise_gate(0.0), 0.0);
        assert_eq!(noise_gate(0.002), 0.0);
    }

    #[test]
    fn noise_gate_passes_above_threshold() {
        assert_eq!(noise_gate(0.003), 1.0);
        assert_eq!(noise_gate(0.1), 1.0);
    }

    #[test]
    fn auto_gain_skips_below_noise_gate() {
        let mut state = AudioState::new();
        let orig = state.clone_values();
        update_auto_gain(&mut state, 0.001);
        assert_eq!(state.peak_energy, orig.0);
        assert_eq!(state.auto_gain, orig.1);
    }

    #[test]
    fn auto_gain_attacks_on_loud_signal() {
        let mut state = AudioState::new();
        state.peak_energy = 0.01;
        update_auto_gain(&mut state, 0.5);
        assert!(state.peak_energy > 0.01);
    }

    #[test]
    fn auto_gain_releases_on_quiet_signal() {
        let mut state = AudioState::new();
        state.peak_energy = 0.5;
        update_auto_gain(&mut state, 0.01);
        assert!(state.peak_energy < 0.5);
    }

    #[test]
    fn auto_gain_clamps_minimum_peak() {
        let mut state = AudioState::new();
        state.peak_energy = 0.0001;
        update_auto_gain(&mut state, NOISE_GATE);
        assert!(state.peak_energy >= 0.001);
    }

    #[test]
    fn smoothing_applies_ema() {
        let mut u = AudioUniforms::default();
        u.bass = 1.0;
        let result = crate::analysis::AnalysisResult {
            bands: [0.0; 16],
            bass: 0.0, mids: 0.0, highs: 0.0,
            energy: 0.0, beat: 0.0,
            spectral_profile: [0.0; 5],
        };
        apply_smoothing(&mut u, &result, 1.0, 1.0);
        assert!(u.bass < 1.0, "bass should decay toward 0");
        assert!(u.bass > 0.0, "bass should retain some value");
    }

    #[test]
    fn smoothing_with_zero_gate_decays() {
        let mut u = AudioUniforms::default();
        u.bass = 1.0;
        let result = crate::analysis::AnalysisResult {
            bands: [0.0; 16],
            bass: 5.0, mids: 0.0, highs: 0.0,
            energy: 0.0, beat: 0.0,
            spectral_profile: [0.0; 5],
        };
        apply_smoothing(&mut u, &result, 1.0, 0.0);
        // gate=0 means no new signal, only decay
        assert_eq!(u.bass, SMOOTH_RETAIN);
    }

    #[test]
    fn beat_snaps_on_detection() {
        let mut u = AudioUniforms::default();
        apply_beat(&mut u, 0.6);
        assert_eq!(u.beat, 1.0);
    }

    #[test]
    fn beat_decays_when_not_detected() {
        let mut u = AudioUniforms::default();
        u.beat = 0.5;
        apply_beat(&mut u, 0.0);
        assert_eq!(u.beat, 0.5 - BEAT_DECAY);
    }

    #[test]
    fn beat_decay_floors_at_zero() {
        let mut u = AudioUniforms::default();
        u.beat = 0.01;
        apply_beat(&mut u, 0.0);
        assert_eq!(u.beat, 0.0);
    }

    impl AudioState {
        fn clone_values(&self) -> (f32, f32) {
            (self.peak_energy, self.auto_gain)
        }
    }
}
