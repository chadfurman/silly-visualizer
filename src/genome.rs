use rand::Rng;
use serde::{Deserialize, Serialize};

/// Number of distinct audio routing targets (geometry, camera, color).
const AUDIO_TARGET_COUNT: f32 = 3.0;

/// Range definitions for each continuous parameter.
/// (min, max) inclusive.
struct Range(f32, f32);

impl Range {
    fn clamp(&self, v: f32) -> f32 {
        v.clamp(self.0, self.1)
    }

    fn random(&self, rng: &mut impl Rng) -> f32 {
        rng.random::<f32>() * (self.1 - self.0) + self.0
    }
}

// -- Continuous ranges --
const SHAPE_TYPE_RANGE: Range = Range(0.0, 4.0);
const SHAPE_SCALE_RANGE: Range = Range(0.3, 2.0);
const SHAPE_OFFSET_RANGE: Range = Range(-1.0, 1.0);
const SHAPE_ROT_SPEED_RANGE: Range = Range(0.0, 2.0);
const COMBINATOR_TYPE_RANGE: Range = Range(0.0, 2.0);
const COMBINATOR_SMOOTH_RANGE: Range = Range(0.1, 1.5);
const FOLD_ITERATIONS_RANGE: Range = Range(1.0, 5.0);
const FOLD_SCALE_RANGE: Range = Range(1.0, 3.0);
const FOLD_OFFSET_RANGE: Range = Range(0.0, 1.0);
const REP_Z_RANGE: Range = Range(2.0, 8.0);
const KALEIDOSCOPE_FOLDS_RANGE: Range = Range(2.0, 8.0);
const CAM_DISTANCE_RANGE: Range = Range(3.0, 8.0);
const ORBIT_SPEED_RANGE: Range = Range(0.1, 0.5);
const WOBBLE_AMOUNT_RANGE: Range = Range(0.0, 1.0);

/// A Genome defines the structural appearance of a scene.
///
/// All fields are f32 for easy interpolation and GPU transfer.
/// Discrete parameters (shape types, combinator types, audio routing targets,
/// transition type) are stored as f32 but take integer values.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Genome {
    /// Shape types: 0=off, 1=sphere, 2=torus, 3=octahedron, 4=box
    pub shape_types: [f32; 4],
    /// Shape scales: 0.3–2.0
    pub shape_scales: [f32; 4],
    /// Shape offsets: -1.0–1.0
    pub shape_offsets: [f32; 4],
    /// Shape rotation speeds: 0.0–2.0
    pub shape_rot_speeds: [f32; 4],
    /// Combinator types: 0=smooth_union, 1=smooth_subtraction, 2=smooth_intersection
    pub combinator_types: [f32; 3],
    /// Combinator smoothness: 0.1–1.5
    pub combinator_smoothness: [f32; 3],
    /// Fold iterations: 1.0–5.0
    pub fold_iterations: f32,
    /// Fold scale: 1.0–3.0
    pub fold_scale: f32,
    /// Fold offset: 0.0–1.0
    pub fold_offset: f32,
    /// Repetition in Z: 2.0–8.0
    pub rep_z: f32,
    /// Kaleidoscope fold count: 2.0–8.0
    pub kaleidoscope_folds: f32,
    /// Camera distance: 3.0–8.0
    pub cam_distance: f32,
    /// Orbit speed: 0.1–0.5
    pub orbit_speed: f32,
    /// Wobble amount: 0.0–1.0
    pub wobble_amount: f32,
    /// Bass audio routing target: 0=geometry, 1=camera, 2=color
    pub bass_target: f32,
    /// Mids audio routing target: 0=geometry, 1=camera, 2=color
    pub mids_target: f32,
    /// Highs audio routing target: 0=geometry, 1=camera, 2=color
    pub highs_target: f32,
    /// Energy audio routing target: 0=geometry, 1=camera, 2=color
    pub energy_target: f32,
    /// Beat audio routing target: 0=geometry, 1=camera, 2=color
    pub beat_target: f32,
    /// Transition type: 0=interp, 1=melt, 2=both
    pub transition_type: f32,
}

impl Genome {
    /// Convert to GPU-friendly SceneUniforms layout.
    pub fn to_uniforms(&self) -> crate::renderer::SceneUniforms {
        crate::renderer::SceneUniforms {
            shapes: std::array::from_fn(|i| {
                [self.shape_types[i], self.shape_scales[i], self.shape_offsets[i], self.shape_rot_speeds[i]]
            }),
            combinators: [
                [self.combinator_types[0], self.combinator_smoothness[0],
                 self.combinator_types[1], self.combinator_smoothness[1]],
                [self.combinator_types[2], self.combinator_smoothness[2], 0.0, 0.0],
            ],
            folding: [self.fold_iterations, self.fold_scale, self.fold_offset, self.rep_z],
            camera: [self.kaleidoscope_folds, self.cam_distance, self.orbit_speed, self.wobble_amount],
            audio_routing: [self.bass_target, self.mids_target, self.highs_target, self.energy_target],
            transition: [self.beat_target, self.transition_type, 0.0, 0.0],
        }
    }

    /// Generate a random valid genome.
    pub fn random(rng: &mut impl Rng) -> Self {
        let mut shape_types = [0.0f32; 4];
        let mut shape_scales = [0.0f32; 4];
        let mut shape_offsets = [0.0f32; 4];
        let mut shape_rot_speeds = [0.0f32; 4];
        for i in 0..4 {
            shape_types[i] = (rng.random::<u32>() % 5) as f32;
            shape_scales[i] = SHAPE_SCALE_RANGE.random(rng);
            shape_offsets[i] = SHAPE_OFFSET_RANGE.random(rng);
            shape_rot_speeds[i] = SHAPE_ROT_SPEED_RANGE.random(rng);
        }

        let mut combinator_types = [0.0f32; 3];
        let mut combinator_smoothness = [0.0f32; 3];
        for i in 0..3 {
            combinator_types[i] = (rng.random::<u32>() % 3) as f32;
            combinator_smoothness[i] = COMBINATOR_SMOOTH_RANGE.random(rng);
        }

        Self {
            shape_types,
            shape_scales,
            shape_offsets,
            shape_rot_speeds,
            combinator_types,
            combinator_smoothness,
            fold_iterations: FOLD_ITERATIONS_RANGE.random(rng),
            fold_scale: FOLD_SCALE_RANGE.random(rng),
            fold_offset: FOLD_OFFSET_RANGE.random(rng),
            rep_z: REP_Z_RANGE.random(rng),
            kaleidoscope_folds: KALEIDOSCOPE_FOLDS_RANGE.random(rng),
            cam_distance: CAM_DISTANCE_RANGE.random(rng),
            orbit_speed: ORBIT_SPEED_RANGE.random(rng),
            wobble_amount: WOBBLE_AMOUNT_RANGE.random(rng),
            bass_target: (rng.random::<u32>() % AUDIO_TARGET_COUNT as u32) as f32,
            mids_target: (rng.random::<u32>() % AUDIO_TARGET_COUNT as u32) as f32,
            highs_target: (rng.random::<u32>() % AUDIO_TARGET_COUNT as u32) as f32,
            energy_target: (rng.random::<u32>() % AUDIO_TARGET_COUNT as u32) as f32,
            beat_target: (rng.random::<u32>() % AUDIO_TARGET_COUNT as u32) as f32,
            transition_type: (rng.random::<u32>() % 3) as f32,
        }
    }

    /// Mutate this genome, returning a new genome.
    ///
    /// `rate` controls overall mutation intensity (0.0 = no mutation, 1.0 = full).
    /// Discrete params mutate with probability `0.15 * rate`.
    /// Continuous params get Gaussian-like noise scaled by `0.3 * rate`.
    pub fn mutate(&self, rng: &mut impl Rng, rate: f32) -> Self {
        let mut result = self.clone();

        let discrete_prob = 0.15 * rate;
        let noise_scale = 0.3 * rate;

        // Helper closures can't capture rng mutably in arrays easily,
        // so we do it inline.

        // Mutate shape arrays (continuous)
        for i in 0..4 {
            result.shape_types[i] = mutate_discrete(self.shape_types[i], 5.0, discrete_prob, rng);
            result.shape_scales[i] =
                mutate_continuous(self.shape_scales[i], &SHAPE_SCALE_RANGE, noise_scale, rng);
            result.shape_offsets[i] =
                mutate_continuous(self.shape_offsets[i], &SHAPE_OFFSET_RANGE, noise_scale, rng);
            result.shape_rot_speeds[i] = mutate_continuous(
                self.shape_rot_speeds[i],
                &SHAPE_ROT_SPEED_RANGE,
                noise_scale,
                rng,
            );
        }

        // Mutate combinator arrays
        for i in 0..3 {
            result.combinator_types[i] =
                mutate_discrete(self.combinator_types[i], 3.0, discrete_prob, rng);
            result.combinator_smoothness[i] = mutate_continuous(
                self.combinator_smoothness[i],
                &COMBINATOR_SMOOTH_RANGE,
                noise_scale,
                rng,
            );
        }

        // Mutate scalar continuous params
        result.fold_iterations =
            mutate_continuous(self.fold_iterations, &FOLD_ITERATIONS_RANGE, noise_scale, rng);
        result.fold_scale =
            mutate_continuous(self.fold_scale, &FOLD_SCALE_RANGE, noise_scale, rng);
        result.fold_offset =
            mutate_continuous(self.fold_offset, &FOLD_OFFSET_RANGE, noise_scale, rng);
        result.rep_z = mutate_continuous(self.rep_z, &REP_Z_RANGE, noise_scale, rng);
        result.kaleidoscope_folds = mutate_continuous(
            self.kaleidoscope_folds,
            &KALEIDOSCOPE_FOLDS_RANGE,
            noise_scale,
            rng,
        );
        result.cam_distance =
            mutate_continuous(self.cam_distance, &CAM_DISTANCE_RANGE, noise_scale, rng);
        result.orbit_speed =
            mutate_continuous(self.orbit_speed, &ORBIT_SPEED_RANGE, noise_scale, rng);
        result.wobble_amount =
            mutate_continuous(self.wobble_amount, &WOBBLE_AMOUNT_RANGE, noise_scale, rng);

        // Mutate discrete audio routing params
        result.bass_target =
            mutate_discrete(self.bass_target, AUDIO_TARGET_COUNT, discrete_prob, rng);
        result.mids_target =
            mutate_discrete(self.mids_target, AUDIO_TARGET_COUNT, discrete_prob, rng);
        result.highs_target =
            mutate_discrete(self.highs_target, AUDIO_TARGET_COUNT, discrete_prob, rng);
        result.energy_target =
            mutate_discrete(self.energy_target, AUDIO_TARGET_COUNT, discrete_prob, rng);
        result.beat_target =
            mutate_discrete(self.beat_target, AUDIO_TARGET_COUNT, discrete_prob, rng);
        result.transition_type =
            mutate_discrete(self.transition_type, 3.0, discrete_prob, rng);

        result
    }

    /// Linearly interpolate between this genome and another.
    ///
    /// Continuous params are lerped. Discrete params snap at `t = 0.5`.
    pub fn lerp(&self, other: &Genome, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);

        let mut shape_types = [0.0f32; 4];
        let mut shape_scales = [0.0f32; 4];
        let mut shape_offsets = [0.0f32; 4];
        let mut shape_rot_speeds = [0.0f32; 4];
        for i in 0..4 {
            shape_types[i] = lerp_discrete(self.shape_types[i], other.shape_types[i], t);
            shape_scales[i] = lerp_f32(self.shape_scales[i], other.shape_scales[i], t);
            shape_offsets[i] = lerp_f32(self.shape_offsets[i], other.shape_offsets[i], t);
            shape_rot_speeds[i] =
                lerp_f32(self.shape_rot_speeds[i], other.shape_rot_speeds[i], t);
        }

        let mut combinator_types = [0.0f32; 3];
        let mut combinator_smoothness = [0.0f32; 3];
        for i in 0..3 {
            combinator_types[i] =
                lerp_discrete(self.combinator_types[i], other.combinator_types[i], t);
            combinator_smoothness[i] = lerp_f32(
                self.combinator_smoothness[i],
                other.combinator_smoothness[i],
                t,
            );
        }

        Self {
            shape_types,
            shape_scales,
            shape_offsets,
            shape_rot_speeds,
            combinator_types,
            combinator_smoothness,
            fold_iterations: lerp_f32(self.fold_iterations, other.fold_iterations, t),
            fold_scale: lerp_f32(self.fold_scale, other.fold_scale, t),
            fold_offset: lerp_f32(self.fold_offset, other.fold_offset, t),
            rep_z: lerp_f32(self.rep_z, other.rep_z, t),
            kaleidoscope_folds: lerp_f32(
                self.kaleidoscope_folds,
                other.kaleidoscope_folds,
                t,
            ),
            cam_distance: lerp_f32(self.cam_distance, other.cam_distance, t),
            orbit_speed: lerp_f32(self.orbit_speed, other.orbit_speed, t),
            wobble_amount: lerp_f32(self.wobble_amount, other.wobble_amount, t),
            bass_target: lerp_discrete(self.bass_target, other.bass_target, t),
            mids_target: lerp_discrete(self.mids_target, other.mids_target, t),
            highs_target: lerp_discrete(self.highs_target, other.highs_target, t),
            energy_target: lerp_discrete(self.energy_target, other.energy_target, t),
            beat_target: lerp_discrete(self.beat_target, other.beat_target, t),
            transition_type: lerp_discrete(self.transition_type, other.transition_type, t),
        }
    }

    /// Check that all values are within their expected bounds.
    pub fn is_valid(&self) -> bool {
        for i in 0..4 {
            if !in_range_inclusive(self.shape_types[i], SHAPE_TYPE_RANGE.0, SHAPE_TYPE_RANGE.1) {
                return false;
            }
            // shape_types should be integer values
            if self.shape_types[i] != self.shape_types[i].floor() {
                return false;
            }
            if !in_range_inclusive(self.shape_scales[i], SHAPE_SCALE_RANGE.0, SHAPE_SCALE_RANGE.1) {
                return false;
            }
            if !in_range_inclusive(
                self.shape_offsets[i],
                SHAPE_OFFSET_RANGE.0,
                SHAPE_OFFSET_RANGE.1,
            ) {
                return false;
            }
            if !in_range_inclusive(
                self.shape_rot_speeds[i],
                SHAPE_ROT_SPEED_RANGE.0,
                SHAPE_ROT_SPEED_RANGE.1,
            ) {
                return false;
            }
        }

        for i in 0..3 {
            if !in_range_inclusive(
                self.combinator_types[i],
                COMBINATOR_TYPE_RANGE.0,
                COMBINATOR_TYPE_RANGE.1,
            ) {
                return false;
            }
            if self.combinator_types[i] != self.combinator_types[i].floor() {
                return false;
            }
            if !in_range_inclusive(
                self.combinator_smoothness[i],
                COMBINATOR_SMOOTH_RANGE.0,
                COMBINATOR_SMOOTH_RANGE.1,
            ) {
                return false;
            }
        }

        if !in_range_inclusive(self.fold_iterations, FOLD_ITERATIONS_RANGE.0, FOLD_ITERATIONS_RANGE.1) {
            return false;
        }
        if !in_range_inclusive(self.fold_scale, FOLD_SCALE_RANGE.0, FOLD_SCALE_RANGE.1) {
            return false;
        }
        if !in_range_inclusive(self.fold_offset, FOLD_OFFSET_RANGE.0, FOLD_OFFSET_RANGE.1) {
            return false;
        }
        if !in_range_inclusive(self.rep_z, REP_Z_RANGE.0, REP_Z_RANGE.1) {
            return false;
        }
        if !in_range_inclusive(
            self.kaleidoscope_folds,
            KALEIDOSCOPE_FOLDS_RANGE.0,
            KALEIDOSCOPE_FOLDS_RANGE.1,
        ) {
            return false;
        }
        if !in_range_inclusive(self.cam_distance, CAM_DISTANCE_RANGE.0, CAM_DISTANCE_RANGE.1) {
            return false;
        }
        if !in_range_inclusive(self.orbit_speed, ORBIT_SPEED_RANGE.0, ORBIT_SPEED_RANGE.1) {
            return false;
        }
        if !in_range_inclusive(self.wobble_amount, WOBBLE_AMOUNT_RANGE.0, WOBBLE_AMOUNT_RANGE.1) {
            return false;
        }

        // Discrete audio targets: must be integer in [0, AUDIO_TARGET_COUNT)
        for &val in &[
            self.bass_target,
            self.mids_target,
            self.highs_target,
            self.energy_target,
            self.beat_target,
        ] {
            if val != val.floor() || val < 0.0 || val >= AUDIO_TARGET_COUNT {
                return false;
            }
        }

        // transition_type: integer in [0, 3)
        if self.transition_type != self.transition_type.floor()
            || self.transition_type < 0.0
            || self.transition_type >= 3.0
        {
            return false;
        }

        true
    }
}

// -- Helper functions --

fn in_range_inclusive(v: f32, min: f32, max: f32) -> bool {
    v >= min && v <= max
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    if t >= 1.0 {
        b
    } else if t <= 0.0 {
        a
    } else {
        a + (b - a) * t
    }
}

fn lerp_discrete(a: f32, b: f32, t: f32) -> f32 {
    if t < 0.5 { a } else { b }
}

/// Mutate a continuous parameter with Gaussian-like noise (Box-Muller approx).
fn mutate_continuous(value: f32, range: &Range, noise_scale: f32, rng: &mut impl Rng) -> f32 {
    // Use simple uniform noise centered on 0, scaled by noise_scale and range width
    let range_width = range.1 - range.0;
    let noise = (rng.random::<f32>() - 0.5) * 2.0 * noise_scale * range_width;
    range.clamp(value + noise)
}

/// Mutate a discrete parameter: with `prob` chance, pick a new random value.
fn mutate_discrete(value: f32, count: f32, prob: f32, rng: &mut impl Rng) -> f32 {
    if rng.random::<f32>() < prob {
        (rng.random::<u32>() % count as u32) as f32
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    #[test]
    fn random_genome_is_valid() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let g = Genome::random(&mut rng);
            assert!(g.is_valid(), "random genome should be valid: {:?}", g);
        }
    }

    #[test]
    fn mutation_produces_valid_genome() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let g = Genome::random(&mut rng);
            let m = g.mutate(&mut rng, 1.0);
            assert!(m.is_valid(), "mutated genome should be valid: {:?}", m);
        }
    }

    #[test]
    fn mutation_changes_something() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        // With rate 1.0 and 100 attempts, at least one mutation should differ
        let mut found_different = false;
        for _ in 0..100 {
            let m = g.mutate(&mut rng, 1.0);
            if m != g {
                found_different = true;
                break;
            }
        }
        assert!(found_different, "mutation at rate 1.0 should change something");
    }

    #[test]
    fn zero_rate_mutation_preserves_continuous_params() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        let m = g.mutate(&mut rng, 0.0);

        // With rate 0.0, noise_scale = 0 and discrete_prob = 0,
        // so all params should be preserved exactly.
        assert_eq!(g.shape_scales, m.shape_scales);
        assert_eq!(g.shape_offsets, m.shape_offsets);
        assert_eq!(g.shape_rot_speeds, m.shape_rot_speeds);
        assert_eq!(g.combinator_smoothness, m.combinator_smoothness);
        assert_eq!(g.fold_iterations, m.fold_iterations);
        assert_eq!(g.fold_scale, m.fold_scale);
        assert_eq!(g.fold_offset, m.fold_offset);
        assert_eq!(g.rep_z, m.rep_z);
        assert_eq!(g.kaleidoscope_folds, m.kaleidoscope_folds);
        assert_eq!(g.cam_distance, m.cam_distance);
        assert_eq!(g.orbit_speed, m.orbit_speed);
        assert_eq!(g.wobble_amount, m.wobble_amount);
        // Discrete params also preserved at rate 0
        assert_eq!(g.shape_types, m.shape_types);
        assert_eq!(g.combinator_types, m.combinator_types);
        assert_eq!(g.bass_target, m.bass_target);
        assert_eq!(g.transition_type, m.transition_type);
    }

    #[test]
    fn lerp_at_zero_returns_self() {
        let mut rng = test_rng();
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        let result = a.lerp(&b, 0.0);
        assert_eq!(result, a);
    }

    #[test]
    fn lerp_at_one_returns_other() {
        let mut rng = test_rng();
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        let result = a.lerp(&b, 1.0);
        assert_eq!(result, b);
    }

    #[test]
    fn lerp_midpoint_interpolates() {
        let mut rng = test_rng();
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        let mid = a.lerp(&b, 0.5);

        // Continuous params should be at midpoint
        for i in 0..4 {
            let expected = (a.shape_scales[i] + b.shape_scales[i]) / 2.0;
            assert!(
                (mid.shape_scales[i] - expected).abs() < 1e-5,
                "shape_scales[{i}] should be midpoint"
            );
        }

        // At t=0.5, discrete params snap to `other` (b)
        for i in 0..4 {
            assert_eq!(mid.shape_types[i], b.shape_types[i]);
        }
    }

    #[test]
    fn serde_round_trip() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        let json = serde_json::to_string(&g).expect("serialize");
        let restored: Genome = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(g, restored);
    }

    #[test]
    fn lerp_result_is_valid() {
        let mut rng = test_rng();
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let result = a.lerp(&b, t);
            assert!(result.is_valid(), "lerp at t={t} should produce valid genome");
        }
    }
}
