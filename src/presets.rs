use rand::Rng;
use crate::genome::Genome;

pub const PRESET_COUNT: usize = 8;

/// Mutation rate applied to presets during evolution.
const PRESET_MUTATION_RATE: f32 = 0.1;

// Shape types: 1=sphere, 2=torus, 3=octahedron, 4=box (0=off)
// Combinator types: 0=union, 1=subtraction, 2=intersection
// Audio targets: 0=geometry, 1=camera, 2=color
// Transition types: 0=ParamInterpolation, 1=FeedbackMelt, 2=Both

pub static PRESETS: [Genome; PRESET_COUNT] = [
    // 0: Fractal Tunnel — torus + sphere, high Z-rep, kaleidoscope
    Genome {
        shape_types: [2.0, 1.0, 1.0, 0.0],
        shape_scales: [0.8, 0.5, 0.4, 0.5],
        shape_offsets: [0.0, 0.3, -0.3, 0.0],
        shape_rot_speeds: [0.6, 0.4, 0.3, 0.0],
        combinator_types: [0.0, 0.0, 0.0],
        combinator_smoothness: [0.8, 0.6, 0.5],
        fold_iterations: 2.0,
        fold_scale: 1.8,
        fold_offset: 0.3,
        rep_z: 6.5,
        kaleidoscope_folds: 5.0,
        cam_distance: 5.0,
        orbit_speed: 0.25,
        wobble_amount: 0.3,
        bass_target: 0.0,
        mids_target: 2.0,
        highs_target: 2.0,
        energy_target: 1.0,
        beat_target: 1.0,
        transition_type: 0.0,
    },
    // 1: Crystal Cave — octahedron + box, Mandelbox folding
    Genome {
        shape_types: [3.0, 4.0, 1.0, 0.0],
        shape_scales: [0.7, 0.6, 0.5, 0.5],
        shape_offsets: [0.0, 0.2, -0.2, 0.0],
        shape_rot_speeds: [0.3, 0.2, 0.4, 0.0],
        combinator_types: [0.0, 2.0, 0.0],
        combinator_smoothness: [0.6, 0.4, 0.5],
        fold_iterations: 3.0,
        fold_scale: 2.0,
        fold_offset: 0.4,
        rep_z: 4.5,
        kaleidoscope_folds: 4.0,
        cam_distance: 5.5,
        orbit_speed: 0.15,
        wobble_amount: 0.2,
        bass_target: 0.0,
        mids_target: 2.0,
        highs_target: 1.0,
        energy_target: 0.0,
        beat_target: 2.0,
        transition_type: 1.0,
    },
    // 2: Neon Bloom — dual spheres, smooth union, fast rotation
    Genome {
        shape_types: [1.0, 1.0, 2.0, 0.0],
        shape_scales: [1.0, 0.7, 0.5, 0.5],
        shape_offsets: [0.0, 0.5, -0.4, 0.0],
        shape_rot_speeds: [1.5, 1.2, 0.8, 0.0],
        combinator_types: [0.0, 0.0, 0.0],
        combinator_smoothness: [1.0, 0.8, 0.7],
        fold_iterations: 1.0,
        fold_scale: 1.5,
        fold_offset: 0.2,
        rep_z: 5.0,
        kaleidoscope_folds: 3.0,
        cam_distance: 4.5,
        orbit_speed: 0.35,
        wobble_amount: 0.5,
        bass_target: 0.0,
        mids_target: 0.0,
        highs_target: 2.0,
        energy_target: 0.0,
        beat_target: 2.0,
        transition_type: 2.0,
    },
    // 3: Kaleidoscope — all 4 shapes, high kaleidoscope folds
    Genome {
        shape_types: [1.0, 2.0, 3.0, 4.0],
        shape_scales: [0.5, 0.6, 0.4, 0.5],
        shape_offsets: [0.0, 0.3, -0.3, 0.1],
        shape_rot_speeds: [0.5, 0.7, 0.4, 0.6],
        combinator_types: [0.0, 0.0, 0.0],
        combinator_smoothness: [0.7, 0.6, 0.5],
        fold_iterations: 2.0,
        fold_scale: 1.6,
        fold_offset: 0.3,
        rep_z: 4.0,
        kaleidoscope_folds: 7.0,
        cam_distance: 3.5,
        orbit_speed: 0.2,
        wobble_amount: 0.4,
        bass_target: 0.0,
        mids_target: 2.0,
        highs_target: 2.0,
        energy_target: 1.0,
        beat_target: 0.0,
        transition_type: 0.0,
    },
    // 4: Void Pulse — torus + octahedron, subtraction, beat-reactive
    Genome {
        shape_types: [2.0, 3.0, 1.0, 0.0],
        shape_scales: [1.2, 0.8, 0.6, 0.5],
        shape_offsets: [0.0, 0.0, 0.2, 0.0],
        shape_rot_speeds: [0.4, 0.6, 0.3, 0.0],
        combinator_types: [1.0, 0.0, 0.0],
        combinator_smoothness: [0.5, 0.7, 0.6],
        fold_iterations: 2.0,
        fold_scale: 1.7,
        fold_offset: 0.5,
        rep_z: 5.5,
        kaleidoscope_folds: 4.0,
        cam_distance: 4.0,
        orbit_speed: 0.3,
        wobble_amount: 0.7,
        bass_target: 0.0,
        mids_target: 0.0,
        highs_target: 2.0,
        energy_target: 1.0,
        beat_target: 1.0,
        transition_type: 1.0,
    },
    // 5: Morphing Geometry — box + sphere intersection, slow
    Genome {
        shape_types: [4.0, 1.0, 3.0, 0.0],
        shape_scales: [0.9, 1.0, 0.5, 0.5],
        shape_offsets: [0.0, 0.0, 0.4, 0.0],
        shape_rot_speeds: [0.2, 0.3, 0.15, 0.0],
        combinator_types: [2.0, 0.0, 0.0],
        combinator_smoothness: [0.6, 0.5, 0.4],
        fold_iterations: 2.0,
        fold_scale: 1.4,
        fold_offset: 0.2,
        rep_z: 5.0,
        kaleidoscope_folds: 3.0,
        cam_distance: 4.5,
        orbit_speed: 0.15,
        wobble_amount: 0.3,
        bass_target: 0.0,
        mids_target: 2.0,
        highs_target: 2.0,
        energy_target: 0.0,
        beat_target: 2.0,
        transition_type: 0.0,
    },
    // 6: Infinite Corridor — high Z-rep, low cam distance
    Genome {
        shape_types: [1.0, 4.0, 2.0, 0.0],
        shape_scales: [0.6, 0.8, 0.4, 0.5],
        shape_offsets: [0.0, 0.0, 0.3, 0.0],
        shape_rot_speeds: [0.3, 0.2, 0.5, 0.0],
        combinator_types: [0.0, 0.0, 0.0],
        combinator_smoothness: [0.9, 0.7, 0.6],
        fold_iterations: 1.0,
        fold_scale: 1.3,
        fold_offset: 0.1,
        rep_z: 7.5,
        kaleidoscope_folds: 3.0,
        cam_distance: 3.0,
        orbit_speed: 0.2,
        wobble_amount: 0.2,
        bass_target: 0.0,
        mids_target: 1.0,
        highs_target: 2.0,
        energy_target: 0.0,
        beat_target: 1.0,
        transition_type: 2.0,
    },
    // 7: Cosmic Web — high fold iterations, all audio → color
    Genome {
        shape_types: [1.0, 3.0, 2.0, 4.0],
        shape_scales: [0.4, 0.5, 0.6, 0.3],
        shape_offsets: [0.0, 0.2, -0.2, 0.1],
        shape_rot_speeds: [0.4, 0.5, 0.3, 0.6],
        combinator_types: [0.0, 0.0, 0.0],
        combinator_smoothness: [1.2, 1.0, 0.8],
        fold_iterations: 4.0,
        fold_scale: 2.5,
        fold_offset: 0.6,
        rep_z: 3.5,
        kaleidoscope_folds: 5.0,
        cam_distance: 5.5,
        orbit_speed: 0.2,
        wobble_amount: 0.4,
        bass_target: 2.0,
        mids_target: 2.0,
        highs_target: 2.0,
        energy_target: 2.0,
        beat_target: 0.0,
        transition_type: 0.0,
    },
];

/// Pick a random preset (no mutation).
pub fn random_preset() -> Genome {
    let idx = rand::rng().random_range(0..PRESET_COUNT);
    PRESETS[idx].clone()
}

/// Pick a random preset and apply light mutation.
pub fn random_preset_mutated(rng: &mut impl Rng) -> Genome {
    let idx = rng.random_range(0..PRESET_COUNT);
    PRESETS[idx].mutate(rng, PRESET_MUTATION_RATE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_are_valid() {
        for (i, preset) in PRESETS.iter().enumerate() {
            assert!(preset.is_valid(), "preset {i} should be valid");
        }
    }

    #[test]
    fn all_presets_have_at_least_two_active_shapes() {
        for (i, preset) in PRESETS.iter().enumerate() {
            let active = preset.shape_types.iter().filter(|&&t| t >= 1.0).count();
            assert!(active >= 2, "preset {i} has only {active} active shapes");
        }
    }

    #[test]
    fn preset_count_matches_constant() {
        assert_eq!(PRESETS.len(), PRESET_COUNT);
    }

    #[test]
    fn random_preset_returns_valid_genome() {
        let g = random_preset();
        assert!(g.is_valid());
    }

    #[test]
    fn random_preset_mutated_returns_valid_genome() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        for _ in 0..50 {
            let g = random_preset_mutated(&mut rng);
            assert!(g.is_valid(), "mutated preset should be valid: {:?}", g);
        }
    }
}
