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

    #[allow(dead_code)] // used in tests via Genome::random
    fn random(&self, rng: &mut impl Rng) -> f32 {
        rng.random::<f32>() * (self.1 - self.0) + self.0
    }
}

// -- Continuous ranges --
const SHAPE_SCALE_RANGE: Range = Range(0.3, 2.0);
const SHAPE_OFFSET_RANGE: Range = Range(-1.0, 1.0);
const SHAPE_ROT_SPEED_RANGE: Range = Range(0.0, 2.0);
const COMBINATOR_SMOOTH_RANGE: Range = Range(0.1, 1.5);
const FOLD_ITERATIONS_RANGE: Range = Range(1.0, 5.0);
const FOLD_SCALE_RANGE: Range = Range(1.0, 3.0);
const FOLD_OFFSET_RANGE: Range = Range(0.0, 1.0);
const REP_Z_RANGE: Range = Range(2.0, 8.0);
const KALEIDOSCOPE_FOLDS_RANGE: Range = Range(2.0, 8.0);
const CAM_DISTANCE_RANGE: Range = Range(3.0, 8.0);
const ORBIT_SPEED_RANGE: Range = Range(0.1, 0.5);
const WOBBLE_AMOUNT_RANGE: Range = Range(0.0, 1.0);
const DISTORTION_AMOUNT_RANGE: Range = Range(0.0, 1.0);

enum FieldKind {
    ContinuousArray(&'static Range, #[allow(dead_code)] usize),
    DiscreteArray(f32, #[allow(dead_code)] usize),
    Continuous(&'static Range),
    Discrete(f32),
}

struct FieldDescriptor {
    kind: FieldKind,
    get: fn(&Genome) -> Vec<f32>,
    set: fn(&mut Genome, &[f32]),
}

macro_rules! arr_field {
    (cont, $range:expr, $len:expr, $field:ident) => {
        FieldDescriptor {
            kind: FieldKind::ContinuousArray(&$range, $len),
            get: |g| g.$field.to_vec(),
            set: |g, v| g.$field.copy_from_slice(v),
        }
    };
    (disc, $count:expr, $len:expr, $field:ident) => {
        FieldDescriptor {
            kind: FieldKind::DiscreteArray($count, $len),
            get: |g| g.$field.to_vec(),
            set: |g, v| g.$field.copy_from_slice(v),
        }
    };
}

macro_rules! scalar_field {
    (cont, $range:expr, $field:ident) => {
        FieldDescriptor {
            kind: FieldKind::Continuous(&$range),
            get: |g| vec![g.$field],
            set: |g, v| g.$field = v[0],
        }
    };
    (disc, $count:expr, $field:ident) => {
        FieldDescriptor {
            kind: FieldKind::Discrete($count),
            get: |g| vec![g.$field],
            set: |g, v| g.$field = v[0],
        }
    };
}

#[allow(clippy::too_many_lines)] // data table, not logic
fn field_descriptors() -> Vec<FieldDescriptor> {
    vec![
        arr_field!(disc, 5.0, 4, shape_types),
        arr_field!(cont, SHAPE_SCALE_RANGE, 4, shape_scales),
        arr_field!(cont, SHAPE_OFFSET_RANGE, 4, shape_offsets),
        arr_field!(cont, SHAPE_ROT_SPEED_RANGE, 4, shape_rot_speeds),
        arr_field!(disc, 3.0, 3, combinator_types),
        arr_field!(cont, COMBINATOR_SMOOTH_RANGE, 3, combinator_smoothness),
        scalar_field!(cont, FOLD_ITERATIONS_RANGE, fold_iterations),
        scalar_field!(cont, FOLD_SCALE_RANGE, fold_scale),
        scalar_field!(cont, FOLD_OFFSET_RANGE, fold_offset),
        scalar_field!(cont, REP_Z_RANGE, rep_z),
        scalar_field!(cont, KALEIDOSCOPE_FOLDS_RANGE, kaleidoscope_folds),
        scalar_field!(cont, CAM_DISTANCE_RANGE, cam_distance),
        scalar_field!(cont, ORBIT_SPEED_RANGE, orbit_speed),
        scalar_field!(cont, WOBBLE_AMOUNT_RANGE, wobble_amount),
        scalar_field!(disc, AUDIO_TARGET_COUNT, bass_target),
        scalar_field!(disc, AUDIO_TARGET_COUNT, mids_target),
        scalar_field!(disc, AUDIO_TARGET_COUNT, highs_target),
        scalar_field!(disc, AUDIO_TARGET_COUNT, energy_target),
        scalar_field!(disc, AUDIO_TARGET_COUNT, beat_target),
        scalar_field!(disc, 3.0, transition_type),
        scalar_field!(disc, 4.0, distortion_type),
        scalar_field!(cont, DISTORTION_AMOUNT_RANGE, distortion_amount),
    ]
}

/// A Genome defines the structural appearance of a scene.
///
/// All fields are f32 for easy interpolation and GPU transfer.
/// Discrete parameters (shape types, combinator types, audio routing targets,
/// transition type) are stored as f32 but take integer values.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Genome {
    pub shape_types: [f32; 4],
    pub shape_scales: [f32; 4],
    pub shape_offsets: [f32; 4],
    pub shape_rot_speeds: [f32; 4],
    pub combinator_types: [f32; 3],
    pub combinator_smoothness: [f32; 3],
    pub fold_iterations: f32,
    pub fold_scale: f32,
    pub fold_offset: f32,
    pub rep_z: f32,
    pub kaleidoscope_folds: f32,
    pub cam_distance: f32,
    pub orbit_speed: f32,
    pub wobble_amount: f32,
    pub bass_target: f32,
    pub mids_target: f32,
    pub highs_target: f32,
    pub energy_target: f32,
    pub beat_target: f32,
    pub transition_type: f32,
    pub distortion_type: f32,
    pub distortion_amount: f32,
}

impl Genome {
    pub fn to_uniforms(&self) -> crate::uniforms::SceneUniforms {
        crate::uniforms::SceneUniforms {
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
            transition: [self.beat_target, self.transition_type, self.distortion_type, self.distortion_amount],
        }
    }

    #[allow(dead_code)] // used in tests
    pub fn random(rng: &mut impl Rng) -> Self {
        let mut genome = Self::zeroed();
        for desc in &field_descriptors() {
            let values = random_field(&desc.kind, rng);
            (desc.set)(&mut genome, &values);
        }
        genome
    }

    pub fn mutate(&self, rng: &mut impl Rng, rate: f32) -> Self {
        let mut result = self.clone();
        let discrete_prob = 0.15 * rate;
        let noise_scale = 0.3 * rate;
        for desc in &field_descriptors() {
            let old = (desc.get)(self);
            let new = mutate_field(&desc.kind, &old, discrete_prob, noise_scale, rng);
            (desc.set)(&mut result, &new);
        }
        result
    }

    pub fn lerp(&self, other: &Genome, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mut result = self.clone();
        for desc in &field_descriptors() {
            let a = (desc.get)(self);
            let b = (desc.get)(other);
            let interp = lerp_field(&desc.kind, &a, &b, t);
            (desc.set)(&mut result, &interp);
        }
        result
    }

    #[allow(dead_code)]
    pub fn is_valid(&self) -> bool {
        for desc in &field_descriptors() {
            let values = (desc.get)(self);
            if !validate_field(&desc.kind, &values) {
                return false;
            }
        }
        true
    }

    #[allow(dead_code)] // used in tests
    fn zeroed() -> Self {
        Self::default()
    }
}

// -- Field-level operations --

#[allow(dead_code)] // used in tests via Genome::random
fn random_field(kind: &FieldKind, rng: &mut impl Rng) -> Vec<f32> {
    match kind {
        FieldKind::ContinuousArray(range, len) => {
            (0..*len).map(|_| range.random(rng)).collect()
        }
        FieldKind::DiscreteArray(count, len) => {
            (0..*len).map(|_| (rng.random::<u32>() % *count as u32) as f32).collect()
        }
        FieldKind::Continuous(range) => vec![range.random(rng)],
        FieldKind::Discrete(count) => {
            vec![(rng.random::<u32>() % *count as u32) as f32]
        }
    }
}

fn mutate_field(
    kind: &FieldKind,
    old: &[f32],
    discrete_prob: f32,
    noise_scale: f32,
    rng: &mut impl Rng,
) -> Vec<f32> {
    match kind {
        FieldKind::ContinuousArray(range, _) => {
            old.iter().map(|&v| mutate_continuous(v, range, noise_scale, rng)).collect()
        }
        FieldKind::DiscreteArray(count, _) => {
            old.iter().map(|&v| mutate_discrete(v, *count, discrete_prob, rng)).collect()
        }
        FieldKind::Continuous(range) => {
            vec![mutate_continuous(old[0], range, noise_scale, rng)]
        }
        FieldKind::Discrete(count) => {
            vec![mutate_discrete(old[0], *count, discrete_prob, rng)]
        }
    }
}

fn lerp_field(kind: &FieldKind, a: &[f32], b: &[f32], t: f32) -> Vec<f32> {
    match kind {
        FieldKind::ContinuousArray(..) | FieldKind::Continuous(_) => {
            a.iter().zip(b).map(|(&x, &y)| lerp_f32(x, y, t)).collect()
        }
        FieldKind::DiscreteArray(..) | FieldKind::Discrete(_) => {
            a.iter().zip(b).map(|(&x, &y)| lerp_discrete(x, y, t)).collect()
        }
    }
}

#[allow(dead_code)]
fn validate_field(kind: &FieldKind, values: &[f32]) -> bool {
    match kind {
        FieldKind::ContinuousArray(range, _) => {
            values.iter().all(|&v| in_range_inclusive(v, range.0, range.1))
        }
        FieldKind::DiscreteArray(count, _) => {
            values.iter().all(|&v| is_valid_discrete(v, *count))
        }
        FieldKind::Continuous(range) => {
            in_range_inclusive(values[0], range.0, range.1)
        }
        FieldKind::Discrete(count) => is_valid_discrete(values[0], *count),
    }
}

// -- Helper functions --

#[allow(dead_code)]
fn in_range_inclusive(v: f32, min: f32, max: f32) -> bool {
    v >= min && v <= max
}

#[allow(dead_code)]
fn is_valid_discrete(v: f32, count: f32) -> bool {
    v == v.floor() && v >= 0.0 && v < count
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

fn mutate_continuous(value: f32, range: &Range, noise_scale: f32, rng: &mut impl Rng) -> f32 {
    let range_width = range.1 - range.0;
    let noise = (rng.random::<f32>() - 0.5) * 2.0 * noise_scale * range_width;
    range.clamp(value + noise)
}

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
    fn zero_rate_mutation_preserves_params() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        let m = g.mutate(&mut rng, 0.0);
        assert_eq!(g, m, "zero-rate mutation should preserve all params");
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

    #[test]
    fn is_valid_rejects_out_of_range() {
        let mut rng = test_rng();
        let mut g = Genome::random(&mut rng);
        g.shape_scales[0] = 999.0;
        assert!(!g.is_valid());
    }

    #[test]
    fn is_valid_rejects_non_integer_discrete() {
        let mut rng = test_rng();
        let mut g = Genome::random(&mut rng);
        g.shape_types[0] = 1.5;
        assert!(!g.is_valid());
    }

    #[test]
    fn is_valid_rejects_negative_discrete() {
        let mut rng = test_rng();
        let mut g = Genome::random(&mut rng);
        g.bass_target = -1.0;
        assert!(!g.is_valid());
    }

    #[test]
    fn is_valid_rejects_discrete_at_count() {
        let mut rng = test_rng();
        let mut g = Genome::random(&mut rng);
        g.transition_type = 3.0; // count is 3, so valid range is [0, 3)
        assert!(!g.is_valid());
    }

    #[test]
    fn mutate_continuous_clamps_to_range() {
        let mut rng = test_rng();
        let range = Range(0.0, 1.0);
        for _ in 0..100 {
            let v = mutate_continuous(0.5, &range, 10.0, &mut rng);
            assert!(v >= 0.0 && v <= 1.0);
        }
    }

    #[test]
    fn mutate_discrete_preserves_at_zero_prob() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let v = mutate_discrete(2.0, 5.0, 0.0, &mut rng);
            assert_eq!(v, 2.0);
        }
    }

    #[test]
    fn mutate_discrete_changes_at_full_prob() {
        let mut rng = test_rng();
        let mut changed = false;
        for _ in 0..100 {
            let v = mutate_discrete(2.0, 5.0, 1.0, &mut rng);
            if v != 2.0 {
                changed = true;
                break;
            }
        }
        assert!(changed, "mutate_discrete at prob 1.0 should change value");
    }

    #[test]
    fn lerp_f32_boundary_values() {
        assert_eq!(lerp_f32(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp_f32(0.0, 10.0, 1.0), 10.0);
        assert!((lerp_f32(0.0, 10.0, 0.5) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn lerp_discrete_snaps_at_half() {
        assert_eq!(lerp_discrete(1.0, 2.0, 0.0), 1.0);
        assert_eq!(lerp_discrete(1.0, 2.0, 0.49), 1.0);
        assert_eq!(lerp_discrete(1.0, 2.0, 0.5), 2.0);
        assert_eq!(lerp_discrete(1.0, 2.0, 1.0), 2.0);
    }

    #[test]
    fn in_range_inclusive_works() {
        assert!(in_range_inclusive(0.5, 0.0, 1.0));
        assert!(in_range_inclusive(0.0, 0.0, 1.0));
        assert!(in_range_inclusive(1.0, 0.0, 1.0));
        assert!(!in_range_inclusive(-0.1, 0.0, 1.0));
        assert!(!in_range_inclusive(1.1, 0.0, 1.0));
    }

    #[test]
    fn is_valid_discrete_works() {
        assert!(is_valid_discrete(0.0, 3.0));
        assert!(is_valid_discrete(2.0, 3.0));
        assert!(!is_valid_discrete(3.0, 3.0));
        assert!(!is_valid_discrete(-1.0, 3.0));
        assert!(!is_valid_discrete(1.5, 3.0));
    }

    #[test]
    fn to_uniforms_packs_correctly() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        let u = g.to_uniforms();
        assert_eq!(u.shapes[0][0], g.shape_types[0]);
        assert_eq!(u.shapes[0][1], g.shape_scales[0]);
        assert_eq!(u.folding[0], g.fold_iterations);
        assert_eq!(u.camera[1], g.cam_distance);
        assert_eq!(u.audio_routing[0], g.bass_target);
        assert_eq!(u.transition[1], g.transition_type);
    }

    #[test]
    fn zeroed_genome_has_all_zeros() {
        let g = Genome::zeroed();
        assert_eq!(g.shape_types, [0.0; 4]);
        assert_eq!(g.fold_iterations, 0.0);
        assert_eq!(g.transition_type, 0.0);
    }

    #[test]
    fn range_clamp_works() {
        let r = Range(0.0, 1.0);
        assert_eq!(r.clamp(-0.5), 0.0);
        assert_eq!(r.clamp(0.5), 0.5);
        assert_eq!(r.clamp(1.5), 1.0);
    }

    #[test]
    fn range_random_in_bounds() {
        let mut rng = test_rng();
        let r = Range(2.0, 5.0);
        for _ in 0..100 {
            let v = r.random(&mut rng);
            assert!(v >= 2.0 && v <= 5.0, "got {v}");
        }
    }
}
