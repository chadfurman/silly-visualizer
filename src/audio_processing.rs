use crate::analysis::{AnalysisResult, AudioAnalyzer};
use crate::uniforms::AudioUniforms;

/// Decay rate for beat indicator (drops from 1.0 to 0 over several frames).
const BEAT_DECAY: f32 = 0.15;
/// Smoothing factor for audio values (0 = no smoothing, 1 = frozen).
const SMOOTH_RETAIN: f32 = 0.95;
const SMOOTH_INCOMING: f32 = 1.0 - SMOOTH_RETAIN;
/// Auto-gain: target energy level and limits.
const TARGET_ENERGY: f32 = 0.05;
const MAX_GAIN: f32 = 50.0;
/// Auto-gain attack/release rates (asymmetric: fast attack, slow release).
const GAIN_ATTACK: f32 = 0.05;
const GAIN_RELEASE: f32 = 0.003;
/// Noise gate: raw energy below this threshold is treated as silence.
const NOISE_GATE: f32 = 0.0001;

pub struct AudioState {
    pub peak_energy: f32,
    pub auto_gain: f32,
    pub slow_energy: f32,
    pub beat_accumulator: f32,
    pub beat_pulse: f32,
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            peak_energy: 0.01,
            auto_gain: 1.0,
            slow_energy: 0.0,
            beat_accumulator: 0.0,
            beat_pulse: 0.0,
        }
    }
}

/// Analyze samples and update uniforms + auto-gain.
pub fn process_samples(
    samples: &[f32],
    analyzer: &mut AudioAnalyzer,
    uniforms: &mut AudioUniforms,
    state: &mut AudioState,
    sensitivity: f32,
) -> AnalysisResult {
    let result = analyzer.analyze(samples);
    update_auto_gain(state, result.energy);
    let gain = state.auto_gain * sensitivity;
    let gate = noise_gate(result.energy);
    apply_smoothing(uniforms, &result, gain, gate);
    apply_beat(uniforms, result.beat);
    uniforms.bands = result.bands;
    result
}

pub fn update_auto_gain(state: &mut AudioState, energy: f32) {
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

pub fn update_envelopes(uniforms: &mut AudioUniforms, state: &mut AudioState, dt: f32) {
    // Slow energy: 7s EMA of current energy
    let alpha = (dt / 7.0).min(1.0);
    state.slow_energy += (uniforms.energy - state.slow_energy) * alpha;

    // Beat accumulator: +0.05 on beat, exponential decay with 6s tau, capped at 1.0
    // Builds slowly over sustained rhythm (~20 beats to reach 0.6)
    if uniforms.beat >= 1.0 {
        state.beat_accumulator = (state.beat_accumulator + 0.05).min(1.0);
    }
    state.beat_accumulator *= (-dt / 6.0).exp();

    // Beat pulse: snap to 1.0 on beat, exponential decay with 0.6s tau
    // Slower decay = smoother swell rather than staccato flash
    if uniforms.beat >= 1.0 {
        state.beat_pulse = 1.0;
    }
    state.beat_pulse *= (-dt / 0.6).exp();

    // Write to uniforms
    uniforms.slow_energy = state.slow_energy;
    uniforms.extra[0] = state.beat_accumulator;
    uniforms.extra[1] = state.beat_pulse;
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
        assert_eq!(noise_gate(0.00005), 0.0);
    }

    #[test]
    fn noise_gate_passes_above_threshold() {
        assert_eq!(noise_gate(NOISE_GATE), 1.0);
        assert_eq!(noise_gate(0.001), 1.0);
        assert_eq!(noise_gate(0.1), 1.0);
    }

    #[test]
    fn auto_gain_adapts_to_quiet_signal() {
        let mut state = AudioState::new();
        let orig_gain = state.auto_gain;
        for _ in 0..100 {
            update_auto_gain(&mut state, 0.001);
        }
        assert!(state.auto_gain > orig_gain, "gain should ramp up for quiet signal");
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

    #[test]
    fn slow_energy_converges_to_signal() {
        let mut u = AudioUniforms::default();
        let mut state = AudioState::new();
        u.energy = 0.5;
        // 7s EMA needs ~4x tau to converge (~28s = 1750 frames at 60fps)
        for _ in 0..2000 {
            update_envelopes(&mut u, &mut state, 0.016);
        }
        assert!((state.slow_energy - 0.5).abs() < 0.05, "slow_energy should converge, got {}", state.slow_energy);
    }

    #[test]
    fn slow_energy_decays_in_silence() {
        let mut u = AudioUniforms::default();
        let mut state = AudioState::new();
        state.slow_energy = 0.5;
        u.energy = 0.0;
        for _ in 0..2000 {
            update_envelopes(&mut u, &mut state, 0.016);
        }
        assert!(state.slow_energy < 0.05, "slow_energy should decay, got {}", state.slow_energy);
    }

    #[test]
    fn beat_accumulator_increments_on_beat() {
        let mut u = AudioUniforms::default();
        let mut state = AudioState::new();
        u.beat = 1.0;
        update_envelopes(&mut u, &mut state, 0.016);
        assert!(state.beat_accumulator > 0.0);
    }

    #[test]
    fn beat_accumulator_decays_without_beats() {
        let mut u = AudioUniforms::default();
        let mut state = AudioState::new();
        state.beat_accumulator = 0.5;
        u.beat = 0.0;
        // 4s tau needs ~16s to decay to near zero (~1000 frames at 60fps)
        for _ in 0..1000 {
            update_envelopes(&mut u, &mut state, 0.016);
        }
        assert!(state.beat_accumulator < 0.05, "accumulator should decay, got {}", state.beat_accumulator);
    }

    #[test]
    fn beat_accumulator_caps_at_one() {
        let mut u = AudioUniforms::default();
        let mut state = AudioState::new();
        u.beat = 1.0;
        for _ in 0..100 {
            update_envelopes(&mut u, &mut state, 0.001);
        }
        assert!(state.beat_accumulator <= 1.0, "accumulator should be capped, got {}", state.beat_accumulator);
    }

    #[test]
    fn beat_pulse_snaps_and_decays() {
        let mut u = AudioUniforms::default();
        let mut state = AudioState::new();
        u.beat = 1.0;
        update_envelopes(&mut u, &mut state, 0.016);
        assert!((state.beat_pulse - 1.0).abs() < 0.1, "pulse should snap near 1.0");
        u.beat = 0.0;
        for _ in 0..100 {
            update_envelopes(&mut u, &mut state, 0.016);
        }
        assert!(state.beat_pulse < 0.1, "pulse should decay, got {}", state.beat_pulse);
    }

    #[test]
    fn envelopes_write_to_uniforms() {
        let mut u = AudioUniforms::default();
        let mut state = AudioState::new();
        state.slow_energy = 0.3;
        state.beat_accumulator = 0.5;
        state.beat_pulse = 0.8;
        u.energy = 0.3;
        update_envelopes(&mut u, &mut state, 0.016);
        assert!(u.slow_energy > 0.0);
        assert!(u.extra[0] > 0.0);
        assert!(u.extra[1] > 0.0);
    }
}
