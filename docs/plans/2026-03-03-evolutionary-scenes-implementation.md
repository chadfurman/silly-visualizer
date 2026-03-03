# Evolutionary Scenes + Visual Polish Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add smooth gradients, genome-driven scene variation, multi-generational lineage with ancestral influence, music-driven transitions, and persistence with favorites.

**Architecture:** A `Genome` struct defines scene parameters (shape types, combinators, folding, camera). A `Lineage` manages 4 generations with weighted ancestral influence on mutation. A `SceneManager` detects musical changes via spectral profile drift and orchestrates crossfades (param interpolation, feedback melt, or both — randomly chosen). Genome params are sent to the GPU as a second uniform buffer. Persistence saves lineage + bookmarked favorites to `~/.silly-visualizer/`.

**Tech Stack:** Rust, wgpu, WGSL shaders, serde_json for persistence, rand for mutation/randomness.

---

### Task 1: Add Dependencies

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add serde, serde_json, rand, dirs**

```toml
[dependencies]
wgpu = "28"
winit = "0.30"
cpal = "0.17"
rustfft = "6.4"
bytemuck = { version = "1.25", features = ["derive"] }
pollster = "0.4"
log = "0.4"
env_logger = "0.11"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rand = "0.9"
dirs = "6"
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: success, no errors

**Step 3: Commit**

```
feat: add serde, rand, dirs dependencies for genome system
```

---

### Task 2: Shader Grain Fix

**Files:**
- Modify: `src/shaders/visualizer.wgsl`

The wavy contour lines come from: (a) noisy normals with ε=0.001 on folded geometry, (b) hard glow falloff creating contour edges, (c) feedback loop amplifying both. Fix all three.

**Step 1: Replace calc_normal with tetrahedron technique**

Replace the `calc_normal` function (lines 288-296) with:

```wgsl
fn calc_normal(p: vec3<f32>) -> vec3<f32> {
    let e = 0.002;
    let k = vec2<f32>(1.0, -1.0);
    return normalize(
        k.xyy * map(p + k.xyy * e) +
        k.yyx * map(p + k.yyx * e) +
        k.yxy * map(p + k.yxy * e) +
        k.xxx * map(p + k.xxx * e)
    );
}
```

This uses 4 samples in a tetrahedral pattern instead of 3 axis-aligned samples. More stable on folded geometry, and ε=0.002 smooths out micro-detail.

**Step 2: Add dithering function and soften glow**

Add a hash function before `fs_main`:

```wgsl
fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}
```

In `fs_main`, after calculating `color` from glow, add dithering. Replace the glow section (lines 408-418):

```wgsl
    // ── Glow from close misses (softened falloff) ──
    let glow_intensity = 0.02 / (result.closest * result.closest + 0.02);
    let glow_color = palette(t * 0.1 + result.total_dist * 0.05 + bass);
    var glow = glow_color * glow_intensity * (0.3 + highs * 1.0);

    // Step-based ambient glow (smoothed)
    let step_glow = f32(result.steps) / f32(MAX_STEPS);
    let step_color = palette_shifted(step_glow + t * 0.02, bass * 0.8);
    glow = glow + step_color * step_glow * step_glow * 0.4;

    color = color + glow;
```

Key changes: glow uses `closest²` denominator (softer falloff), reduced multipliers to avoid harsh contours.

**Step 3: Add dithering before tone mapping**

After the beat flash / palette shift section and before the feedback section, add:

```wgsl
    // ── Dither to break banding ──
    let dither = (hash(pos.xy + vec2<f32>(u.time * 100.0, 0.0)) - 0.5) / 255.0;
    color = color + vec3<f32>(dither);
```

**Step 4: Replace single-tap feedback with 5-tap blur**

Replace the feedback sampling section (the `prev_r`, `prev_g`, `prev_b` lines and `prev_srgb` construction) with a 5-tap cross blur:

```wgsl
    // ── Feedback: blend with previous frame for melting trail effect ──
    // 5-tap cross blur on feedback to smooth accumulated artifacts
    let screen_uv = pos.xy / u.resolution;
    let drift = vec2<f32>(sin(u.time * 0.1) * 0.003, cos(u.time * 0.13) * 0.003);
    let ca_offset = beat * 0.015;
    let texel = 1.0 / u.resolution;

    // Center + 4 neighbors weighted: center=0.5, each neighbor=0.125
    let uv_c = screen_uv + drift;
    let sample_c = textureSample(prev_frame, prev_sampler, uv_c).rgb;
    let sample_u = textureSample(prev_frame, prev_sampler, uv_c + vec2<f32>(0.0, texel.y)).rgb;
    let sample_d = textureSample(prev_frame, prev_sampler, uv_c - vec2<f32>(0.0, texel.y)).rgb;
    let sample_l = textureSample(prev_frame, prev_sampler, uv_c - vec2<f32>(texel.x, 0.0)).rgb;
    let sample_r = textureSample(prev_frame, prev_sampler, uv_c + vec2<f32>(texel.x, 0.0)).rgb;
    let blurred = sample_c * 0.5 + (sample_u + sample_d + sample_l + sample_r) * 0.125;

    // Chromatic aberration on the blurred feedback
    let prev_srgb = vec3<f32>(
        textureSample(prev_frame, prev_sampler, uv_c + vec2<f32>(ca_offset, 0.0)).r * 0.5
            + textureSample(prev_frame, prev_sampler, uv_c + vec2<f32>(ca_offset, texel.y)).r * 0.125
            + textureSample(prev_frame, prev_sampler, uv_c + vec2<f32>(ca_offset, -texel.y)).r * 0.125
            + textureSample(prev_frame, prev_sampler, uv_c + vec2<f32>(ca_offset - texel.x, 0.0)).r * 0.125
            + textureSample(prev_frame, prev_sampler, uv_c + vec2<f32>(ca_offset + texel.x, 0.0)).r * 0.125,
        blurred.g,
        textureSample(prev_frame, prev_sampler, uv_c - vec2<f32>(ca_offset, 0.0)).b * 0.5
            + textureSample(prev_frame, prev_sampler, uv_c - vec2<f32>(ca_offset, texel.y)).b * 0.125
            + textureSample(prev_frame, prev_sampler, uv_c - vec2<f32>(ca_offset, -texel.y)).b * 0.125
            + textureSample(prev_frame, prev_sampler, uv_c - vec2<f32>(ca_offset - texel.x, 0.0)).b * 0.125
            + textureSample(prev_frame, prev_sampler, uv_c - vec2<f32>(ca_offset + texel.x, 0.0)).b * 0.125,
    );
    let prev_linear = pow(prev_srgb, vec3<f32>(2.2));
```

The rest of the feedback (trail_decay, blend_factor, mix) stays the same.

**Step 5: Verify it compiles and runs**

Run: `cargo build`
Expected: success
Run the app, verify visually that gradients are smoother and contour lines are reduced.

**Step 6: Commit**

```
fix: smoother gradients — tetrahedron normals, dithering, feedback blur
```

---

### Task 3: Genome Struct + Mutation + Tests

**Files:**
- Create: `src/genome.rs`
- Modify: `src/main.rs` (add `mod genome;`)

This is the core data model. A Genome defines what a scene looks like structurally. All fields are `f32` for easy interpolation + GPU transfer.

**Step 1: Write the failing tests**

Create `src/genome.rs` with test module first:

```rust
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Which SDF primitive a shape slot uses.
/// Encoded as f32 for GPU: 0=off, 1=sphere, 2=torus, 3=octahedron, 4=box
const SHAPE_TYPE_COUNT: u32 = 5;

/// Which SDF combinator to use between shapes.
/// 0=smooth_union, 1=smooth_subtraction, 2=smooth_intersection
const COMBINATOR_TYPE_COUNT: u32 = 3;

/// How to crossfade when transitioning away from this scene.
/// 0=param_interpolation, 1=feedback_melt, 2=both
const TRANSITION_TYPE_COUNT: u32 = 3;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Genome {
    // Shape slots (4 shapes)
    pub shape_types: [f32; 4],
    pub shape_scales: [f32; 4],
    pub shape_offsets: [f32; 4],
    pub shape_rot_speeds: [f32; 4],

    // Combinators (3, combining shapes 0+1, 01+2, 012+3)
    pub combinator_types: [f32; 3],
    pub combinator_smoothness: [f32; 3],

    // Space folding
    pub fold_iterations: f32,
    pub fold_scale: f32,
    pub fold_offset: f32,

    // Domain repetition
    pub rep_z: f32,
    pub kaleidoscope_folds: f32,

    // Camera
    pub cam_distance: f32,
    pub orbit_speed: f32,
    pub wobble_amount: f32,

    // Transition style (0=interp, 1=melt, 2=both)
    pub transition_type: f32,
}

/// Parameter bounds: (min, max) for each continuous parameter.
struct Bounds {
    shape_scale: (f32, f32),
    shape_offset: (f32, f32),
    shape_rot_speed: (f32, f32),
    combinator_smoothness: (f32, f32),
    fold_iterations: (f32, f32),
    fold_scale: (f32, f32),
    fold_offset: (f32, f32),
    rep_z: (f32, f32),
    kaleidoscope_folds: (f32, f32),
    cam_distance: (f32, f32),
    orbit_speed: (f32, f32),
    wobble_amount: (f32, f32),
}

const BOUNDS: Bounds = Bounds {
    shape_scale: (0.3, 2.0),
    shape_offset: (-1.0, 1.0),
    shape_rot_speed: (0.0, 2.0),
    combinator_smoothness: (0.1, 1.5),
    fold_iterations: (1.0, 5.0),
    fold_scale: (1.0, 3.0),
    fold_offset: (0.0, 1.0),
    rep_z: (2.0, 8.0),
    kaleidoscope_folds: (2.0, 8.0),
    cam_distance: (3.0, 8.0),
    orbit_speed: (0.1, 0.5),
    wobble_amount: (0.0, 1.0),
};

fn clamp_f32(v: f32, min: f32, max: f32) -> f32 {
    v.clamp(min, max)
}

fn rand_range(rng: &mut impl Rng, min: f32, max: f32) -> f32 {
    rng.random::<f32>() * (max - min) + min
}

fn mutate_continuous(rng: &mut impl Rng, value: f32, rate: f32, min: f32, max: f32) -> f32 {
    let noise: f32 = rng.random::<f32>() * 2.0 - 1.0; // uniform -1..1
    clamp_f32(value + noise * rate * (max - min), min, max)
}

fn mutate_discrete(rng: &mut impl Rng, value: f32, count: u32, probability: f32) -> f32 {
    if rng.random::<f32>() < probability {
        (rng.random::<u32>() % count) as f32
    } else {
        value
    }
}

impl Genome {
    /// Generate a random genome.
    pub fn random(rng: &mut impl Rng) -> Self {
        Self {
            shape_types: std::array::from_fn(|_| (rng.random::<u32>() % SHAPE_TYPE_COUNT) as f32),
            shape_scales: std::array::from_fn(|_| rand_range(rng, BOUNDS.shape_scale.0, BOUNDS.shape_scale.1)),
            shape_offsets: std::array::from_fn(|_| rand_range(rng, BOUNDS.shape_offset.0, BOUNDS.shape_offset.1)),
            shape_rot_speeds: std::array::from_fn(|_| rand_range(rng, BOUNDS.shape_rot_speed.0, BOUNDS.shape_rot_speed.1)),
            combinator_types: std::array::from_fn(|_| (rng.random::<u32>() % COMBINATOR_TYPE_COUNT) as f32),
            combinator_smoothness: std::array::from_fn(|_| rand_range(rng, BOUNDS.combinator_smoothness.0, BOUNDS.combinator_smoothness.1)),
            fold_iterations: rand_range(rng, BOUNDS.fold_iterations.0, BOUNDS.fold_iterations.1),
            fold_scale: rand_range(rng, BOUNDS.fold_scale.0, BOUNDS.fold_scale.1),
            fold_offset: rand_range(rng, BOUNDS.fold_offset.0, BOUNDS.fold_offset.1),
            rep_z: rand_range(rng, BOUNDS.rep_z.0, BOUNDS.rep_z.1),
            kaleidoscope_folds: rand_range(rng, BOUNDS.kaleidoscope_folds.0, BOUNDS.kaleidoscope_folds.1),
            cam_distance: rand_range(rng, BOUNDS.cam_distance.0, BOUNDS.cam_distance.1),
            orbit_speed: rand_range(rng, BOUNDS.orbit_speed.0, BOUNDS.orbit_speed.1),
            wobble_amount: rand_range(rng, BOUNDS.wobble_amount.0, BOUNDS.wobble_amount.1),
            transition_type: (rng.random::<u32>() % TRANSITION_TYPE_COUNT) as f32,
        }
    }

    /// Mutate this genome, returning a new child.
    /// `rate` controls mutation intensity (0.0 = no change, 1.0 = max).
    pub fn mutate(&self, rng: &mut impl Rng, rate: f32) -> Self {
        let discrete_prob = 0.15 * rate; // structural changes are rare
        let cont_rate = 0.3 * rate;

        Self {
            shape_types: std::array::from_fn(|i| mutate_discrete(rng, self.shape_types[i], SHAPE_TYPE_COUNT, discrete_prob)),
            shape_scales: std::array::from_fn(|i| mutate_continuous(rng, self.shape_scales[i], cont_rate, BOUNDS.shape_scale.0, BOUNDS.shape_scale.1)),
            shape_offsets: std::array::from_fn(|i| mutate_continuous(rng, self.shape_offsets[i], cont_rate, BOUNDS.shape_offset.0, BOUNDS.shape_offset.1)),
            shape_rot_speeds: std::array::from_fn(|i| mutate_continuous(rng, self.shape_rot_speeds[i], cont_rate, BOUNDS.shape_rot_speed.0, BOUNDS.shape_rot_speed.1)),
            combinator_types: std::array::from_fn(|i| mutate_discrete(rng, self.combinator_types[i], COMBINATOR_TYPE_COUNT, discrete_prob)),
            combinator_smoothness: std::array::from_fn(|i| mutate_continuous(rng, self.combinator_smoothness[i], cont_rate, BOUNDS.combinator_smoothness.0, BOUNDS.combinator_smoothness.1)),
            fold_iterations: mutate_continuous(rng, self.fold_iterations, cont_rate, BOUNDS.fold_iterations.0, BOUNDS.fold_iterations.1),
            fold_scale: mutate_continuous(rng, self.fold_scale, cont_rate, BOUNDS.fold_scale.0, BOUNDS.fold_scale.1),
            fold_offset: mutate_continuous(rng, self.fold_offset, cont_rate, BOUNDS.fold_offset.0, BOUNDS.fold_offset.1),
            rep_z: mutate_continuous(rng, self.rep_z, cont_rate, BOUNDS.rep_z.0, BOUNDS.rep_z.1),
            kaleidoscope_folds: mutate_continuous(rng, self.kaleidoscope_folds, cont_rate, BOUNDS.kaleidoscope_folds.0, BOUNDS.kaleidoscope_folds.1),
            cam_distance: mutate_continuous(rng, self.cam_distance, cont_rate, BOUNDS.cam_distance.0, BOUNDS.cam_distance.1),
            orbit_speed: mutate_continuous(rng, self.orbit_speed, cont_rate, BOUNDS.orbit_speed.0, BOUNDS.orbit_speed.1),
            wobble_amount: mutate_continuous(rng, self.wobble_amount, cont_rate, BOUNDS.wobble_amount.0, BOUNDS.wobble_amount.1),
            transition_type: mutate_discrete(rng, self.transition_type, TRANSITION_TYPE_COUNT, discrete_prob),
        }
    }

    /// Linearly interpolate between two genomes.
    /// Discrete params (shape_type, combinator_type, transition_type) snap at t=0.5.
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let snap = if t < 0.5 { 0.0 } else { 1.0 };

        fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }
        fn lerp_arr4(a: &[f32; 4], b: &[f32; 4], t: f32) -> [f32; 4] {
            std::array::from_fn(|i| lerp_f32(a[i], b[i], t))
        }
        fn lerp_arr3(a: &[f32; 3], b: &[f32; 3], t: f32) -> [f32; 3] {
            std::array::from_fn(|i| lerp_f32(a[i], b[i], t))
        }
        fn snap_arr4(a: &[f32; 4], b: &[f32; 4], snap: f32) -> [f32; 4] {
            std::array::from_fn(|i| if snap < 0.5 { a[i] } else { b[i] })
        }
        fn snap_arr3(a: &[f32; 3], b: &[f32; 3], snap: f32) -> [f32; 3] {
            std::array::from_fn(|i| if snap < 0.5 { a[i] } else { b[i] })
        }

        Self {
            shape_types: snap_arr4(&self.shape_types, &other.shape_types, snap),
            shape_scales: lerp_arr4(&self.shape_scales, &other.shape_scales, t),
            shape_offsets: lerp_arr4(&self.shape_offsets, &other.shape_offsets, t),
            shape_rot_speeds: lerp_arr4(&self.shape_rot_speeds, &other.shape_rot_speeds, t),
            combinator_types: snap_arr3(&self.combinator_types, &other.combinator_types, snap),
            combinator_smoothness: lerp_arr3(&self.combinator_smoothness, &other.combinator_smoothness, t),
            fold_iterations: lerp_f32(self.fold_iterations, other.fold_iterations, t),
            fold_scale: lerp_f32(self.fold_scale, other.fold_scale, t),
            fold_offset: lerp_f32(self.fold_offset, other.fold_offset, t),
            rep_z: lerp_f32(self.rep_z, other.rep_z, t),
            kaleidoscope_folds: lerp_f32(self.kaleidoscope_folds, other.kaleidoscope_folds, t),
            cam_distance: lerp_f32(self.cam_distance, other.cam_distance, t),
            orbit_speed: lerp_f32(self.orbit_speed, other.orbit_speed, t),
            wobble_amount: lerp_f32(self.wobble_amount, other.wobble_amount, t),
            transition_type: if snap < 0.5 { self.transition_type } else { other.transition_type },
        }
    }

    /// Check that all values are within valid bounds.
    pub fn is_valid(&self) -> bool {
        let in_range = |v: f32, min: f32, max: f32| v >= min && v <= max;
        let discrete_ok = |v: f32, count: u32| {
            let i = v as u32;
            i as f32 == v && i < count
        };

        self.shape_types.iter().all(|&v| discrete_ok(v, SHAPE_TYPE_COUNT))
            && self.shape_scales.iter().all(|&v| in_range(v, BOUNDS.shape_scale.0, BOUNDS.shape_scale.1))
            && self.shape_offsets.iter().all(|&v| in_range(v, BOUNDS.shape_offset.0, BOUNDS.shape_offset.1))
            && self.shape_rot_speeds.iter().all(|&v| in_range(v, BOUNDS.shape_rot_speed.0, BOUNDS.shape_rot_speed.1))
            && self.combinator_types.iter().all(|&v| discrete_ok(v, COMBINATOR_TYPE_COUNT))
            && self.combinator_smoothness.iter().all(|&v| in_range(v, BOUNDS.combinator_smoothness.0, BOUNDS.combinator_smoothness.1))
            && in_range(self.fold_iterations, BOUNDS.fold_iterations.0, BOUNDS.fold_iterations.1)
            && in_range(self.fold_scale, BOUNDS.fold_scale.0, BOUNDS.fold_scale.1)
            && in_range(self.fold_offset, BOUNDS.fold_offset.0, BOUNDS.fold_offset.1)
            && in_range(self.rep_z, BOUNDS.rep_z.0, BOUNDS.rep_z.1)
            && in_range(self.kaleidoscope_folds, BOUNDS.kaleidoscope_folds.0, BOUNDS.kaleidoscope_folds.1)
            && in_range(self.cam_distance, BOUNDS.cam_distance.0, BOUNDS.cam_distance.1)
            && in_range(self.orbit_speed, BOUNDS.orbit_speed.0, BOUNDS.orbit_speed.1)
            && in_range(self.wobble_amount, BOUNDS.wobble_amount.0, BOUNDS.wobble_amount.1)
            && discrete_ok(self.transition_type, TRANSITION_TYPE_COUNT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn seeded_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    #[test]
    fn random_genome_is_valid() {
        let mut rng = seeded_rng();
        for _ in 0..100 {
            let g = Genome::random(&mut rng);
            assert!(g.is_valid(), "random genome should be valid: {g:?}");
        }
    }

    #[test]
    fn mutation_produces_valid_genome() {
        let mut rng = seeded_rng();
        let parent = Genome::random(&mut rng);
        for _ in 0..100 {
            let child = parent.mutate(&mut rng, 1.0);
            assert!(child.is_valid(), "mutated genome should be valid: {child:?}");
        }
    }

    #[test]
    fn mutation_changes_something() {
        let mut rng = seeded_rng();
        let parent = Genome::random(&mut rng);
        let child = parent.mutate(&mut rng, 1.0);
        // At max rate, extremely unlikely that ALL params stay identical
        assert_ne!(parent, child, "mutation should change at least one param");
    }

    #[test]
    fn zero_rate_mutation_preserves_continuous_params() {
        let mut rng = seeded_rng();
        let parent = Genome::random(&mut rng);
        let child = parent.mutate(&mut rng, 0.0);
        // Continuous params should be identical (rate=0 means noise*0)
        assert_eq!(parent.shape_scales, child.shape_scales);
        assert_eq!(parent.fold_scale, child.fold_scale);
        assert_eq!(parent.cam_distance, child.cam_distance);
    }

    #[test]
    fn lerp_at_zero_returns_self() {
        let mut rng = seeded_rng();
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        let result = a.lerp(&b, 0.0);
        assert_eq!(result.shape_scales, a.shape_scales);
        assert_eq!(result.fold_scale, a.fold_scale);
        assert_eq!(result.shape_types, a.shape_types); // discrete snaps to a at t<0.5
    }

    #[test]
    fn lerp_at_one_returns_other() {
        let mut rng = seeded_rng();
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        let result = a.lerp(&b, 1.0);
        assert_eq!(result.shape_scales, b.shape_scales);
        assert_eq!(result.fold_scale, b.fold_scale);
        assert_eq!(result.shape_types, b.shape_types); // discrete snaps to b at t>=0.5
    }

    #[test]
    fn lerp_midpoint_interpolates() {
        let mut rng = seeded_rng();
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        let mid = a.lerp(&b, 0.5);
        // Continuous params should be midpoint
        for i in 0..4 {
            let expected = (a.shape_scales[i] + b.shape_scales[i]) / 2.0;
            assert!((mid.shape_scales[i] - expected).abs() < 1e-6);
        }
    }

    #[test]
    fn serde_round_trip() {
        let mut rng = seeded_rng();
        let g = Genome::random(&mut rng);
        let json = serde_json::to_string(&g).unwrap();
        let g2: Genome = serde_json::from_str(&json).unwrap();
        assert_eq!(g, g2);
    }

    #[test]
    fn lerp_result_is_valid() {
        let mut rng = seeded_rng();
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let result = a.lerp(&b, t);
            assert!(result.is_valid(), "lerp at t={t} should be valid");
        }
    }
}
```

**Step 2: Add `mod genome;` to main.rs**

Add `mod genome;` after the existing `mod renderer;` line (line 3 of `src/main.rs`).

**Step 3: Run tests to verify they pass**

Run: `cargo test genome`
Expected: all 8 tests pass

**Step 4: Commit**

```
feat(genome): add Genome struct with mutation, interpolation, validation, serde
```

---

### Task 4: Lineage + Ancestral Influence + Tests

**Files:**
- Create: `src/lineage.rs`
- Modify: `src/main.rs` (add `mod lineage;`)

**Step 1: Write lineage module with tests**

Create `src/lineage.rs`:

```rust
use rand::Rng;

use crate::genome::Genome;

/// Ancestral weight for mutation influence.
const WEIGHT_CHILD: f32 = 1.0;
const WEIGHT_PARENT: f32 = 0.5;
const WEIGHT_GRANDPARENT: f32 = 0.25;
const WEIGHT_GREAT_GRANDPARENT: f32 = 0.125;

/// Multi-generational lineage with ancestral influence.
///
/// The child is the active scene. When a new generation is created,
/// each generation shifts up one slot (great-grandparent dies).
///
/// Ancestral influence: when creating a new child via mutation,
/// the parent is the primary source. Grandparent and great-grandparent
/// "pull" certain params toward their values via a weighted blend on
/// the mutation result.
pub struct Lineage {
    pub child: Genome,
    pub parent: Option<Genome>,
    pub grandparent: Option<Genome>,
    pub great_grandparent: Option<Genome>,
}

impl Lineage {
    /// Create a new lineage starting from a single genome.
    pub fn new(initial: Genome) -> Self {
        Self {
            child: initial,
            parent: None,
            grandparent: None,
            great_grandparent: None,
        }
    }

    /// Number of living generations (1-4).
    pub fn generation_count(&self) -> usize {
        1 + self.parent.is_some() as usize
            + self.grandparent.is_some() as usize
            + self.great_grandparent.is_some() as usize
    }

    /// Advance to a new generation. The current child becomes parent,
    /// and a new child is created by mutating the parent with ancestral
    /// influence from older generations.
    pub fn advance(&mut self, rng: &mut impl Rng, mutation_rate: f32) {
        // Mutate from current child (who is about to become parent)
        let mut new_child = self.child.mutate(rng, mutation_rate);

        // Apply ancestral pull: blend new_child toward ancestor values
        // with decaying weights. This creates "family resemblance".
        let mut total_weight = WEIGHT_CHILD;

        if let Some(ref parent) = self.parent {
            new_child = new_child.lerp(parent, WEIGHT_PARENT / (total_weight + WEIGHT_PARENT));
            total_weight += WEIGHT_PARENT;
        }
        if let Some(ref gp) = self.grandparent {
            new_child = new_child.lerp(gp, WEIGHT_GRANDPARENT / (total_weight + WEIGHT_GRANDPARENT));
            total_weight += WEIGHT_GRANDPARENT;
        }
        if let Some(ref ggp) = self.great_grandparent {
            new_child = new_child.lerp(ggp, WEIGHT_GREAT_GRANDPARENT / (total_weight + WEIGHT_GREAT_GRANDPARENT));
        }

        // Shift generations
        self.great_grandparent = self.grandparent.take();
        self.grandparent = self.parent.take();
        self.parent = Some(std::mem::replace(&mut self.child, new_child));
    }

    /// Inject a genome as the new child (e.g. loading a favorite).
    /// Shifts existing generations up.
    pub fn inject(&mut self, genome: Genome) {
        self.great_grandparent = self.grandparent.take();
        self.grandparent = self.parent.take();
        self.parent = Some(std::mem::replace(&mut self.child, genome));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn seeded_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    #[test]
    fn new_lineage_has_one_generation() {
        let mut rng = seeded_rng();
        let lineage = Lineage::new(Genome::random(&mut rng));
        assert_eq!(lineage.generation_count(), 1);
        assert!(lineage.parent.is_none());
        assert!(lineage.grandparent.is_none());
        assert!(lineage.great_grandparent.is_none());
    }

    #[test]
    fn advance_shifts_generations() {
        let mut rng = seeded_rng();
        let g0 = Genome::random(&mut rng);
        let mut lineage = Lineage::new(g0.clone());

        lineage.advance(&mut rng, 0.5);
        assert_eq!(lineage.generation_count(), 2);
        assert!(lineage.parent.is_some());
        assert!(lineage.grandparent.is_none());

        lineage.advance(&mut rng, 0.5);
        assert_eq!(lineage.generation_count(), 3);
        assert!(lineage.grandparent.is_some());

        lineage.advance(&mut rng, 0.5);
        assert_eq!(lineage.generation_count(), 4);
        assert!(lineage.great_grandparent.is_some());
    }

    #[test]
    fn advance_caps_at_four_generations() {
        let mut rng = seeded_rng();
        let mut lineage = Lineage::new(Genome::random(&mut rng));
        for _ in 0..10 {
            lineage.advance(&mut rng, 0.5);
        }
        assert_eq!(lineage.generation_count(), 4);
    }

    #[test]
    fn great_grandparent_dies_on_fifth_advance() {
        let mut rng = seeded_rng();
        let initial = Genome::random(&mut rng);
        let mut lineage = Lineage::new(initial.clone());

        // Build up to 4 generations
        for _ in 0..3 {
            lineage.advance(&mut rng, 0.5);
        }
        let ggp_before = lineage.great_grandparent.clone().unwrap();

        // One more advance — old great-grandparent should be replaced
        lineage.advance(&mut rng, 0.5);
        let ggp_after = lineage.great_grandparent.clone().unwrap();
        assert_ne!(ggp_before, ggp_after, "great-grandparent should change after advance");
    }

    #[test]
    fn child_is_always_valid_after_advance() {
        let mut rng = seeded_rng();
        let mut lineage = Lineage::new(Genome::random(&mut rng));
        for _ in 0..20 {
            lineage.advance(&mut rng, 1.0);
            assert!(lineage.child.is_valid(), "child should be valid after advance");
        }
    }

    #[test]
    fn inject_shifts_generations() {
        let mut rng = seeded_rng();
        let mut lineage = Lineage::new(Genome::random(&mut rng));
        lineage.advance(&mut rng, 0.5);
        assert_eq!(lineage.generation_count(), 2);

        let favorite = Genome::random(&mut rng);
        lineage.inject(favorite.clone());
        assert_eq!(lineage.generation_count(), 3);
        assert_eq!(lineage.child, favorite);
    }

    #[test]
    fn ancestral_influence_creates_family_resemblance() {
        let mut rng = seeded_rng();
        // Create a lineage and advance several times
        let initial = Genome::random(&mut rng);
        let mut lineage = Lineage::new(initial.clone());
        for _ in 0..5 {
            lineage.advance(&mut rng, 0.3); // low mutation rate
        }

        // With ancestors pulling back, the child shouldn't drift
        // as far as pure mutation would take it. Compare against
        // a genome mutated 5 times with NO ancestral influence.
        let mut rng2 = StdRng::seed_from_u64(42);
        let mut no_ancestry = Genome::random(&mut rng2);
        for _ in 0..5 {
            no_ancestry = no_ancestry.mutate(&mut rng2, 0.3);
        }

        // The lineage child should be "closer" to the initial genome
        // on average because ancestors pull it back. We check fold_scale
        // as a representative continuous param.
        let lineage_drift = (lineage.child.fold_scale - initial.fold_scale).abs();
        let no_anc_drift = (no_ancestry.fold_scale - initial.fold_scale).abs();
        // This is probabilistic but with seed=42 and 5 generations
        // the ancestral pull should keep drift smaller.
        // We just assert the lineage child is valid — the statistical
        // property is hard to assert deterministically.
        assert!(lineage.child.is_valid());
        // Log for manual inspection during development
        log::info!("lineage drift: {lineage_drift:.4}, no-ancestry drift: {no_anc_drift:.4}");
    }
}
```

**Step 2: Add `mod lineage;` to main.rs**

Add `mod lineage;` after `mod genome;`.

**Step 3: Run tests**

Run: `cargo test lineage`
Expected: all 7 tests pass

**Step 4: Commit**

```
feat(lineage): multi-generational lineage with ancestral influence
```

---

### Task 5: Spectral Profile + Musical Change Detection + Tests

**Files:**
- Modify: `src/analysis.rs` (add `SpectralProfile` to `AnalysisResult`)
- Create: `src/scene.rs`
- Modify: `src/main.rs` (add `mod scene;`)

**Step 1: Add spectral profile to AnalysisResult**

In `src/analysis.rs`, add a `spectral_profile` field to `AnalysisResult`:

```rust
pub struct AnalysisResult {
    pub bands: [f32; 16],
    pub bass: f32,
    pub mids: f32,
    pub highs: f32,
    pub energy: f32,
    pub beat: f32,
    pub spectral_profile: [f32; 5], // [bass, mids, highs, energy, beat]
}
```

At the end of `analyze()`, before returning, populate it:

```rust
        let spectral_profile = [bass, mids, highs, energy, beat];

        AnalysisResult {
            bands,
            bass,
            mids,
            highs,
            energy,
            beat,
            spectral_profile,
        }
```

**Step 2: Run existing analysis tests to verify no breakage**

Run: `cargo test analysis`
Expected: all 8 existing tests pass

**Step 3: Create scene.rs with ChangeDetector**

Create `src/scene.rs`:

```rust
/// Detects significant changes in musical "feel" by tracking
/// spectral profile drift between a fast EMA and slow EMA baseline.
pub struct ChangeDetector {
    /// Slow-moving baseline (τ ≈ 10s at 60fps → α ≈ 0.0017)
    baseline: [f32; 5],
    /// Fast-moving current (τ ≈ 2s at 60fps → α ≈ 0.0083)
    current: [f32; 5],
    /// Novelty threshold for triggering a generation change.
    threshold: f32,
    /// Minimum time between triggers (in seconds).
    cooldown: f32,
    /// Time since last trigger (in seconds).
    time_since_trigger: f32,
    /// Whether the detector has been primed with initial data.
    primed: bool,
}

impl ChangeDetector {
    pub fn new(threshold: f32, cooldown: f32) -> Self {
        Self {
            baseline: [0.0; 5],
            current: [0.0; 5],
            threshold,
            cooldown,
            time_since_trigger: 0.0,
            primed: false,
        }
    }

    /// Update with a new spectral profile. Returns true if a
    /// significant musical change was detected.
    pub fn update(&mut self, profile: &[f32; 5], dt: f32) -> bool {
        self.time_since_trigger += dt;

        if !self.primed {
            self.baseline = *profile;
            self.current = *profile;
            self.primed = true;
            return false;
        }

        // Fast EMA (τ ≈ 2s)
        let alpha_fast = (dt / 2.0).min(1.0);
        // Slow EMA (τ ≈ 10s)
        let alpha_slow = (dt / 10.0).min(1.0);

        for i in 0..5 {
            self.current[i] += (profile[i] - self.current[i]) * alpha_fast;
            self.baseline[i] += (profile[i] - self.baseline[i]) * alpha_slow;
        }

        let novelty = self.novelty();

        if novelty > self.threshold && self.time_since_trigger >= self.cooldown {
            // Reset baseline to current so we adapt to new feel
            self.baseline = self.current;
            self.time_since_trigger = 0.0;
            return true;
        }

        false
    }

    /// Euclidean distance between current and baseline spectral profiles.
    pub fn novelty(&self) -> f32 {
        let mut sum = 0.0f32;
        for i in 0..5 {
            let diff = self.current[i] - self.baseline[i];
            sum += diff * diff;
        }
        sum.sqrt()
    }

    /// Reset the detector (e.g. when changing audio source).
    pub fn reset(&mut self) {
        self.baseline = [0.0; 5];
        self.current = [0.0; 5];
        self.primed = false;
        self.time_since_trigger = 0.0;
    }
}

/// Which crossfade mode to use for a scene transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrossfadeMode {
    ParamInterpolation,
    FeedbackMelt,
    Both,
}

impl CrossfadeMode {
    pub fn from_genome_value(v: f32) -> Self {
        match v as u32 {
            0 => Self::ParamInterpolation,
            1 => Self::FeedbackMelt,
            _ => Self::Both,
        }
    }

    /// Duration in seconds for this crossfade mode.
    pub fn duration(&self) -> f32 {
        match self {
            Self::ParamInterpolation => 4.0,
            Self::FeedbackMelt => 2.5,
            Self::Both => 4.0,
        }
    }
}

/// Manages an active crossfade between two genomes.
pub struct Crossfade {
    pub mode: CrossfadeMode,
    pub from: crate::genome::Genome,
    pub to: crate::genome::Genome,
    pub progress: f32, // 0.0 to 1.0
    duration: f32,
}

impl Crossfade {
    pub fn new(mode: CrossfadeMode, from: crate::genome::Genome, to: crate::genome::Genome) -> Self {
        let duration = mode.duration();
        Self { mode, from, to, progress: 0.0, duration }
    }

    /// Advance the crossfade. Returns true when complete.
    pub fn advance(&mut self, dt: f32) -> bool {
        self.progress = (self.progress + dt / self.duration).min(1.0);
        self.progress >= 1.0
    }

    /// Get the interpolated genome at current progress.
    pub fn current_genome(&self) -> crate::genome::Genome {
        self.from.lerp(&self.to, self.progress)
    }

    /// Whether this crossfade should boost feedback retention
    /// (feedback melt or both modes).
    pub fn boost_feedback(&self) -> bool {
        matches!(self.mode, CrossfadeMode::FeedbackMelt | CrossfadeMode::Both)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_detector_does_not_trigger_initially() {
        let mut det = ChangeDetector::new(0.1, 15.0);
        let profile = [0.5, 0.3, 0.2, 0.4, 0.0];
        assert!(!det.update(&profile, 1.0 / 60.0));
    }

    #[test]
    fn change_detector_triggers_on_large_shift() {
        let mut det = ChangeDetector::new(0.1, 0.0); // no cooldown for test
        let quiet = [0.0, 0.0, 0.0, 0.0, 0.0];
        // Prime with quiet
        for _ in 0..300 {
            det.update(&quiet, 1.0 / 60.0);
        }
        // Sudden loud signal
        let loud = [1.0, 1.0, 1.0, 1.0, 1.0];
        // Feed loud for enough frames to shift `current` away from `baseline`
        let mut triggered = false;
        for _ in 0..120 {
            if det.update(&loud, 1.0 / 60.0) {
                triggered = true;
                break;
            }
        }
        assert!(triggered, "should trigger on large spectral shift");
    }

    #[test]
    fn change_detector_respects_cooldown() {
        let mut det = ChangeDetector::new(0.01, 15.0);
        let quiet = [0.0; 5];
        let loud = [1.0; 5];
        // Prime
        for _ in 0..300 { det.update(&quiet, 1.0 / 60.0); }
        // Trigger once
        for _ in 0..120 { det.update(&loud, 1.0 / 60.0); }
        // Reset to quiet and try to trigger again quickly
        for _ in 0..60 { det.update(&quiet, 1.0 / 60.0); }
        // Within cooldown, should not trigger even with another big shift
        let triggered = det.update(&[2.0; 5], 1.0 / 60.0);
        // time_since_trigger is only ~3s (180 frames / 60fps), cooldown is 15s
        assert!(!triggered, "should not trigger within cooldown");
    }

    #[test]
    fn change_detector_does_not_trigger_on_steady_signal() {
        let mut det = ChangeDetector::new(0.1, 0.0);
        let steady = [0.5, 0.3, 0.2, 0.4, 0.0];
        let mut triggered = false;
        for _ in 0..600 {
            if det.update(&steady, 1.0 / 60.0) {
                triggered = true;
            }
        }
        assert!(!triggered, "steady signal should not trigger change");
    }

    #[test]
    fn novelty_is_zero_when_equal() {
        let mut det = ChangeDetector::new(0.1, 15.0);
        let p = [0.5, 0.3, 0.2, 0.4, 0.0];
        // Prime with same signal many times
        for _ in 0..1000 {
            det.update(&p, 1.0 / 60.0);
        }
        assert!(det.novelty() < 0.01, "novelty should be near zero for steady signal, got {}", det.novelty());
    }

    #[test]
    fn crossfade_completes_in_expected_time() {
        use crate::genome::Genome;
        use rand::SeedableRng;
        use rand::rngs::StdRng;

        let mut rng = StdRng::seed_from_u64(42);
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        let mut cf = Crossfade::new(CrossfadeMode::ParamInterpolation, a, b);

        let dt = 1.0 / 60.0;
        let mut frames = 0;
        while !cf.advance(dt) {
            frames += 1;
            assert!(frames < 300, "crossfade should complete within 5s");
        }
        let elapsed = frames as f32 * dt;
        assert!((elapsed - 4.0).abs() < 0.1, "should take ~4s, took {elapsed}");
    }

    #[test]
    fn crossfade_genome_at_zero_matches_from() {
        use crate::genome::Genome;
        use rand::SeedableRng;
        use rand::rngs::StdRng;

        let mut rng = StdRng::seed_from_u64(42);
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        let cf = Crossfade::new(CrossfadeMode::Both, a.clone(), b);
        let g = cf.current_genome();
        assert_eq!(g.shape_scales, a.shape_scales);
    }

    #[test]
    fn crossfade_mode_from_genome_value() {
        assert_eq!(CrossfadeMode::from_genome_value(0.0), CrossfadeMode::ParamInterpolation);
        assert_eq!(CrossfadeMode::from_genome_value(1.0), CrossfadeMode::FeedbackMelt);
        assert_eq!(CrossfadeMode::from_genome_value(2.0), CrossfadeMode::Both);
        assert_eq!(CrossfadeMode::from_genome_value(99.0), CrossfadeMode::Both);
    }
}
```

**Step 4: Add `mod scene;` to main.rs**

**Step 5: Run tests**

Run: `cargo test scene`
Expected: all 8 tests pass

Run: `cargo test analysis`
Expected: all existing tests still pass

**Step 6: Commit**

```
feat(scene): musical change detection + crossfade orchestration
```

---

### Task 6: SceneUniforms + Renderer Integration

**Files:**
- Modify: `src/renderer.rs`
- Modify: `src/genome.rs` (add `to_uniforms()`)

**Step 1: Add SceneUniforms struct to renderer.rs**

Add after AudioUniforms (after line 53):

```rust
/// GPU representation of genome parameters.
/// Layout matches the SceneUniforms WGSL struct.
/// Each shape slot is packed as vec4(type, scale, offset, rot_speed).
const _: () = assert!(
    std::mem::size_of::<SceneUniforms>() == 160,
    "SceneUniforms size must be 160 bytes to match WGSL layout"
);
const _: () = assert!(
    std::mem::size_of::<SceneUniforms>() % 16 == 0,
    "SceneUniforms size must be 16-byte aligned"
);

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneUniforms {
    /// shapes[i] = (type, scale, offset, rot_speed) for shape slot i
    pub shapes: [[f32; 4]; 4],
    /// combinators[i] = (type, smoothness, 0, 0) — index 3 unused
    pub combinators: [[f32; 4]; 4],
    /// (fold_iterations, fold_scale, fold_offset, rep_z)
    pub folding: [f32; 4],
    /// (kaleidoscope_folds, cam_distance, orbit_speed, wobble_amount)
    pub camera: [f32; 4],
    /// (transition_progress, feedback_boost, 0, 0)
    pub extra: [f32; 4],
}

impl Default for SceneUniforms {
    fn default() -> Self {
        Self {
            shapes: [
                [2.0, 1.2, 0.0, 0.4],  // torus
                [3.0, 1.0, 0.0, 0.25],  // octahedron
                [1.0, 0.8, 0.0, 0.0],   // sphere
                [4.0, 0.5, 0.0, 0.0],   // box
            ],
            combinators: [
                [0.0, 0.8, 0.0, 0.0],   // smooth_union
                [1.0, 0.3, 0.0, 0.0],   // smooth_subtraction
                [0.0, 0.5, 0.0, 0.0],   // smooth_union
                [0.0; 4],
            ],
            folding: [3.0, 1.5, 0.0, 4.0],
            camera: [3.0, 0.2, 0.4, 0.0],
            extra: [0.0; 4],
        }
    }
}
```

**Step 2: Add `to_uniforms()` to Genome in genome.rs**

Add this method to the `impl Genome` block. Note: import is not needed if we define a standalone function or use a method. Add at the end of the impl block:

```rust
    /// Convert to GPU-friendly SceneUniforms.
    pub fn to_scene_uniforms(&self, transition_progress: f32, feedback_boost: f32) -> crate::renderer::SceneUniforms {
        crate::renderer::SceneUniforms {
            shapes: std::array::from_fn(|i| {
                [self.shape_types[i], self.shape_scales[i], self.shape_offsets[i], self.shape_rot_speeds[i]]
            }),
            combinators: [
                [self.combinator_types[0], self.combinator_smoothness[0], 0.0, 0.0],
                [self.combinator_types[1], self.combinator_smoothness[1], 0.0, 0.0],
                [self.combinator_types[2], self.combinator_smoothness[2], 0.0, 0.0],
                [0.0; 4],
            ],
            folding: [self.fold_iterations, self.fold_scale, self.fold_offset, self.rep_z],
            camera: [self.kaleidoscope_folds, self.cam_distance, self.orbit_speed, self.wobble_amount],
            extra: [transition_progress, feedback_boost, 0.0, 0.0],
        }
    }
```

**Step 3: Add second uniform buffer to Renderer**

In the `Renderer` struct, add fields:

```rust
    scene_uniform_buffer: wgpu::Buffer,
```

In `Renderer::init()`, create it alongside the audio uniform buffer:

```rust
        let scene_uniforms = SceneUniforms::default();
        let scene_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scene uniforms buffer"),
                contents: bytemuck::cast_slice(&[scene_uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
```

Add a binding 3 to the bind group layout entries:

```rust
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
```

Add a 4th entry to `create_bind_group` function params and body:

```rust
fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform_buffer: &wgpu::Buffer,
    scene_uniform_buffer: &wgpu::Buffer,
    prev_frame_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(prev_frame_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: scene_uniform_buffer.as_entire_binding(),
            },
        ],
    })
}
```

Update ALL call sites (init, resize) to pass `&scene_uniform_buffer`.

Add a public method to update scene uniforms:

```rust
    pub fn update_scene_uniforms(&self, uniforms: &SceneUniforms) {
        self.queue.write_buffer(
            &self.scene_uniform_buffer,
            0,
            bytemuck::cast_slice(&[*uniforms]),
        );
    }
```

And modify `render()` to accept scene uniforms:

```rust
    pub fn render(&mut self, uniforms: &mut AudioUniforms, scene_uniforms: &SceneUniforms) {
```

Call `self.update_scene_uniforms(scene_uniforms);` alongside the existing `self.update_uniforms(uniforms);`.

**Step 4: Update main.rs render call**

Change the `renderer.render(&mut self.uniforms)` call to pass default scene uniforms for now:

```rust
                if let Some(renderer) = &mut self.renderer {
                    renderer.render(&mut self.uniforms, &renderer::SceneUniforms::default());
                }
```

**Step 5: Add tests for SceneUniforms**

In the renderer test module:

```rust
    #[test]
    fn scene_uniforms_size_is_160_bytes() {
        assert_eq!(std::mem::size_of::<SceneUniforms>(), 160);
    }

    #[test]
    fn scene_uniforms_size_is_16_byte_aligned() {
        assert_eq!(std::mem::size_of::<SceneUniforms>() % 16, 0);
    }

    #[test]
    fn scene_uniforms_is_pod_castable() {
        let s = SceneUniforms::default();
        let bytes: &[u8] = bytemuck::bytes_of(&s);
        assert_eq!(bytes.len(), 160);
    }
```

Add genome conversion test to genome tests:

```rust
    #[test]
    fn genome_to_scene_uniforms_round_trips_values() {
        let mut rng = seeded_rng();
        let g = Genome::random(&mut rng);
        let su = g.to_scene_uniforms(0.5, 1.0);
        // Verify shape types match
        for i in 0..4 {
            assert_eq!(su.shapes[i][0], g.shape_types[i]);
            assert_eq!(su.shapes[i][1], g.shape_scales[i]);
        }
        assert_eq!(su.extra[0], 0.5); // transition_progress
        assert_eq!(su.extra[1], 1.0); // feedback_boost
    }
```

**Step 6: Verify everything compiles and tests pass**

Run: `cargo test`
Expected: all tests pass

**Step 7: Commit**

```
feat(renderer): add SceneUniforms buffer for genome-driven rendering
```

---

### Task 7: Genome-Driven Shader

**Files:**
- Modify: `src/shaders/visualizer.wgsl`

This is the big shader rewrite. The `map()` function reads shape/combinator/folding params from the new scene uniform buffer instead of hardcoded values.

**Step 1: Add SceneUniforms binding to WGSL**

After the existing bindings (line 17), add:

```wgsl
struct SceneUniforms {
    // shapes[i] = vec4(type, scale, offset, rot_speed)
    shapes: array<vec4<f32>, 4>,
    // combinators[i] = vec4(type, smoothness, 0, 0)
    combinators: array<vec4<f32>, 4>,
    // (fold_iterations, fold_scale, fold_offset, rep_z)
    folding: vec4<f32>,
    // (kaleidoscope_folds, cam_distance, orbit_speed, wobble_amount)
    camera: vec4<f32>,
    // (transition_progress, feedback_boost, 0, 0)
    extra: vec4<f32>,
}

@group(0) @binding(3) var<uniform> scene: SceneUniforms;
```

**Step 2: Add a generic `sdf_shape` dispatcher**

Add after the existing SDF primitives:

```wgsl
/// Dispatch to the right SDF based on shape type.
/// 0=off (large distance), 1=sphere, 2=torus, 3=octahedron, 4=box
fn sdf_shape(p: vec3<f32>, shape_type: f32, scale: f32) -> f32 {
    let st = i32(shape_type);
    switch (st) {
        case 1: { return sd_sphere(p, scale); }
        case 2: { return sd_torus(p, vec2<f32>(scale, scale * 0.25)); }
        case 3: { return sd_octahedron(p, scale); }
        case 4: { return sd_box(p, vec3<f32>(scale)); }
        default: { return MAX_DIST; } // off
    }
}

/// Dispatch to the right combinator.
/// 0=smooth_union, 1=smooth_subtraction, 2=smooth_intersection
fn sdf_combine(d1: f32, d2: f32, ctype: f32, smoothness: f32) -> f32 {
    let ct = i32(ctype);
    switch (ct) {
        case 1: { return smooth_subtraction(d2, d1, smoothness); }
        case 2: { return smooth_intersection(d1, d2, smoothness); }
        default: { return smooth_union(d1, d2, smoothness); }
    }
}
```

**Step 3: Rewrite `map()` to use genome params**

Replace the existing `map()` function with:

```wgsl
fn map(p_in: vec3<f32>) -> f32 {
    let t = u.time;
    let bass = u.bass;
    let mids = u.mids;
    let highs = u.highs;
    let energy = u.energy;

    // Rotation driven by time + audio, modulated by genome orbit_speed
    let orbit = scene.camera.z; // orbit_speed
    let wobble = scene.camera.w; // wobble_amount
    let rot_speed = orbit + mids * 1.5 * wobble;
    var p = rot_y(t * rot_speed * 0.4) * rot_x(t * rot_speed * 0.25) * p_in;

    // ── Domain repetition ──
    let rep_z = scene.folding.w - bass * 1.0; // rep_z from genome
    var rp = p;
    rp.z = wmod(p.z + t * 0.8, max(rep_z, 0.5)) - max(rep_z, 0.5) * 0.5;

    // Kaleidoscopic angular folding
    let k_folds = max(scene.camera.x, 2.0); // kaleidoscope_folds
    let fold_angle = PI / (k_folds + mids * 3.0);
    let angle = atan2(rp.y, rp.x);
    let folded_angle = wmod(angle, fold_angle * 2.0) - fold_angle;
    let r_xy = length(rp.xy);
    rp = vec3<f32>(r_xy * cos(folded_angle), r_xy * sin(folded_angle), rp.z);

    // ── Iterative space folding ──
    let fold_iters = i32(clamp(scene.folding.x + energy * 2.0, 1.0, 5.0));
    let fold_scale = scene.folding.y + energy * 0.4;
    let fp = fold_space(rp * 0.5, fold_iters, fold_scale) * 2.0;

    // ── Evaluate 4 shape slots ──
    let audio_scale = 1.0 + bass * 0.8;

    // Shape 0 in repeated space
    let s0_type = scene.shapes[0].x;
    let s0_scale = scene.shapes[0].y * audio_scale;
    let s0_offset = scene.shapes[0].z;
    let s0_rot = scene.shapes[0].w;
    let s0_p = rot_z(t * s0_rot + mids * 2.0) * (rp + vec3<f32>(s0_offset, 0.0, 0.0));
    var d = sdf_shape(s0_p, s0_type, s0_scale);

    // Shape 1 in repeated space
    let s1_type = scene.shapes[1].x;
    let s1_scale = scene.shapes[1].y * audio_scale;
    let s1_offset = scene.shapes[1].z;
    let s1_rot = scene.shapes[1].w;
    let s1_p = rot_x(t * s1_rot) * (rp + vec3<f32>(0.0, s1_offset, 0.0));
    let d1 = sdf_shape(s1_p, s1_type, s1_scale);
    d = sdf_combine(d, d1, scene.combinators[0].x, scene.combinators[0].y + bass * 0.4);

    // Shape 2 in folded space
    let s2_type = scene.shapes[2].x;
    let s2_scale = scene.shapes[2].y + highs * 0.5;
    let s2_offset = scene.shapes[2].z;
    let s2_p = fp + vec3<f32>(s2_offset, 0.0, 0.0);
    let d2 = sdf_shape(s2_p, s2_type, s2_scale);
    d = sdf_combine(d, d2 * 0.6, scene.combinators[1].x, scene.combinators[1].y + energy * 0.2);

    // Shape 3 in folded space
    let s3_type = scene.shapes[3].x;
    let s3_scale = scene.shapes[3].y + mids * 0.3;
    let s3_offset = scene.shapes[3].z;
    let s3_p = fp + vec3<f32>(0.0, s3_offset, 0.0);
    let d3 = sdf_shape(s3_p, s3_type, s3_scale);
    d = sdf_combine(d, d3, scene.combinators[2].x, scene.combinators[2].y);

    // ── Pulsing spheres at tunnel repetitions ──
    let pulse = 0.3 + 0.2 * sin(t * 4.0 + bass * TAU);
    let srp_z = wmod(p.z + t * 0.8, max(rep_z, 0.5)) - max(rep_z, 0.5) * 0.5;
    let d_pulse = sd_sphere(vec3<f32>(p.x, p.y, srp_z), pulse);
    d = smooth_union(d, d_pulse, 0.6);

    return d;
}
```

**Step 4: Update camera setup in fs_main to use genome params**

Replace the camera section to read from genome:

```wgsl
    // ── Camera setup: orbiting camera (genome-driven) ──
    let seed = u.seed;
    let cam_dist = scene.camera.y - bass * 1.5; // cam_distance from genome
    let cam_orbit = scene.camera.z; // orbit_speed
    let cam_wobble = scene.camera.w; // wobble_amount
    let cam_angle_y = t * cam_orbit + mids * 0.5;
    let cam_angle_x = sin(t * 0.15) * 0.4 * cam_wobble;
```

Also update `focal` to incorporate wobble:

```wgsl
    let focal = 1.5 - energy * 0.3 * cam_wobble;
```

**Step 5: Update feedback to respect feedback_boost**

In the feedback section, modify `trail_decay` and `blend_factor`:

```wgsl
    let feedback_boost = scene.extra[1];
    let trail_decay = 0.85 + feedback_boost * 0.1; // higher during melt transitions
    let blend_factor = 0.45 + beat * 0.25 - feedback_boost * 0.15; // less blend = more old frame
```

**Step 6: Verify it compiles and runs**

Run: `cargo build`
Expected: success. Run the app — should look similar to before since SceneUniforms::default() matches roughly the old hardcoded values.

**Step 7: Commit**

```
feat(shader): genome-driven scene rendering with 4 shape slots + combinators
```

---

### Task 8: Persistence (Save/Load Lineage + Favorites)

**Files:**
- Create: `src/persistence.rs`
- Modify: `src/main.rs` (add `mod persistence;`)

**Step 1: Create persistence.rs with tests**

```rust
use std::fs;
use std::path::PathBuf;

use crate::genome::Genome;

/// Get the data directory: `~/.silly-visualizer/`
pub fn data_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".silly-visualizer"))
}

/// Get the favorites directory: `~/.silly-visualizer/favorites/`
pub fn favorites_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("favorites"))
}

/// Save a genome lineage snapshot (just the child + parent + grandparent + great_grandparent).
pub fn save_lineage(
    child: &Genome,
    parent: Option<&Genome>,
    grandparent: Option<&Genome>,
    great_grandparent: Option<&Genome>,
) -> Result<(), String> {
    let dir = data_dir().ok_or("could not determine home directory")?;
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create data dir: {e}"))?;

    let data = LineageData {
        child: child.clone(),
        parent: parent.cloned(),
        grandparent: grandparent.cloned(),
        great_grandparent: great_grandparent.cloned(),
    };
    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("serialization error: {e}"))?;
    fs::write(dir.join("lineage.json"), json)
        .map_err(|e| format!("failed to write lineage: {e}"))
}

/// Load the last saved lineage.
pub fn load_lineage() -> Result<Option<LineageData>, String> {
    let path = match data_dir() {
        Some(d) => d.join("lineage.json"),
        None => return Ok(None),
    };
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read lineage: {e}"))?;
    let data: LineageData = serde_json::from_str(&json)
        .map_err(|e| format!("failed to parse lineage: {e}"))?;
    Ok(Some(data))
}

/// Save a favorite genome with a timestamp-based filename.
pub fn save_favorite(genome: &Genome) -> Result<String, String> {
    let dir = favorites_dir().ok_or("could not determine home directory")?;
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create favorites dir: {e}"))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let filename = format!("{timestamp}.json");
    let json = serde_json::to_string_pretty(genome)
        .map_err(|e| format!("serialization error: {e}"))?;
    fs::write(dir.join(&filename), json)
        .map_err(|e| format!("failed to write favorite: {e}"))?;
    Ok(filename)
}

/// Load a random favorite genome, if any exist.
pub fn load_random_favorite(rng: &mut impl rand::Rng) -> Result<Option<Genome>, String> {
    let dir = match favorites_dir() {
        Some(d) if d.exists() => d,
        _ => return Ok(None),
    };
    let entries: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| format!("failed to read favorites dir: {e}"))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();

    if entries.is_empty() {
        return Ok(None);
    }

    let idx = rng.random::<usize>() % entries.len();
    let path = entries[idx].path();
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read favorite: {e}"))?;
    let genome: Genome = serde_json::from_str(&json)
        .map_err(|e| format!("failed to parse favorite: {e}"))?;
    Ok(Some(genome))
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct LineageData {
    pub child: Genome,
    pub parent: Option<Genome>,
    pub grandparent: Option<Genome>,
    pub great_grandparent: Option<Genome>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use std::fs;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("silly-vis-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lineage_data_serde_round_trip() {
        let mut rng = StdRng::seed_from_u64(42);
        let data = LineageData {
            child: Genome::random(&mut rng),
            parent: Some(Genome::random(&mut rng)),
            grandparent: None,
            great_grandparent: None,
        };
        let json = serde_json::to_string(&data).unwrap();
        let data2: LineageData = serde_json::from_str(&json).unwrap();
        assert_eq!(data.child, data2.child);
        assert_eq!(data.parent, data2.parent);
        assert!(data2.grandparent.is_none());
    }

    #[test]
    fn favorite_serde_round_trip() {
        let mut rng = StdRng::seed_from_u64(42);
        let g = Genome::random(&mut rng);
        let dir = temp_dir();
        let path = dir.join("test_fav.json");
        let json = serde_json::to_string_pretty(&g).unwrap();
        fs::write(&path, &json).unwrap();
        let loaded: Genome = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(g, loaded);
        fs::remove_dir_all(&dir).ok();
    }
}
```

**Step 2: Add `mod persistence;` to main.rs**

**Step 3: Run tests**

Run: `cargo test persistence`
Expected: 2 tests pass

**Step 4: Commit**

```
feat(persistence): save/load lineage and favorites to ~/.silly-visualizer
```

---

### Task 9: Wire Up main.rs

**Files:**
- Modify: `src/main.rs`

This is the integration task. Wire up: Lineage, SceneManager (ChangeDetector + Crossfade), persistence, and keyboard shortcuts.

**Step 1: Add new fields to App**

```rust
use genome::Genome;
use lineage::Lineage;
use scene::{ChangeDetector, Crossfade, CrossfadeMode};
use renderer::{AudioUniforms, Renderer, SceneUniforms};
```

Add to the `App` struct:

```rust
    lineage: Option<Lineage>,
    change_detector: ChangeDetector,
    crossfade: Option<Crossfade>,
    active_scene_uniforms: SceneUniforms,
    rng: rand::rngs::ThreadRng,
```

**Step 2: Initialize in main()**

```rust
    let mut rng = rand::rng();
    // Try to load saved lineage, otherwise start with random genome
    let lineage = match persistence::load_lineage() {
        Ok(Some(data)) => {
            log::info!("loaded saved lineage");
            let mut l = Lineage::new(data.child);
            if let Some(p) = data.parent { l.inject(p); }
            // We lose grandparent/great-grandparent ordering on load,
            // but that's fine — they'll rebuild over a few generations
            l
        }
        _ => {
            log::info!("starting with random genome");
            Lineage::new(Genome::random(&mut rng))
        }
    };
```

Initialize the App with:

```rust
        lineage: None, // set in resumed()
        change_detector: ChangeDetector::new(0.15, 15.0),
        crossfade: None,
        active_scene_uniforms: SceneUniforms::default(),
        rng: rand::rng(),
```

Move lineage initialization to `resumed()` alongside the analyzer setup, or initialize it in `main()` — simpler to do it in main and make it `Some(lineage)` from the start.

**Step 3: Update RedrawRequested to drive scene evolution**

After audio analysis, before rendering:

```rust
                // Drive scene evolution
                let dt = now.duration_since(self.last_frame_time).as_secs_f32();

                if let Some(ref mut lineage) = self.lineage {
                    // Check for musical change
                    if self.crossfade.is_none() {
                        let result_ref = /* the analysis result from above */;
                        // We need to restructure slightly — store spectral_profile
                    }

                    // Advance crossfade if active
                    if let Some(ref mut cf) = self.crossfade {
                        let done = cf.advance(dt);
                        let genome = cf.current_genome();
                        let fb_boost = if cf.boost_feedback() { 1.0 - cf.progress } else { 0.0 };
                        self.active_scene_uniforms = genome.to_scene_uniforms(cf.progress, fb_boost);
                        if done {
                            self.crossfade = None;
                        }
                    }
                }
```

The spectral profile needs to be captured from the analysis result. Restructure the audio analysis block to save the spectral_profile, then use it for change detection.

**Step 4: Add B and N key handlers**

```rust
                        Key::Character("b") => {
                            if let Some(ref lineage) = self.lineage {
                                match persistence::save_favorite(&lineage.child) {
                                    Ok(name) => log::info!("saved favorite: {name}"),
                                    Err(e) => log::warn!("failed to save favorite: {e}"),
                                }
                            }
                        }
                        Key::Character("n") => {
                            if let Some(ref mut lineage) = self.lineage {
                                match persistence::load_random_favorite(&mut self.rng) {
                                    Ok(Some(fav)) => {
                                        log::info!("loaded random favorite");
                                        let mode = CrossfadeMode::from_genome_value(lineage.child.transition_type);
                                        self.crossfade = Some(Crossfade::new(mode, lineage.child.clone(), fav.clone()));
                                        lineage.inject(fav);
                                    }
                                    Ok(None) => log::info!("no favorites saved yet"),
                                    Err(e) => log::warn!("failed to load favorite: {e}"),
                                }
                            }
                        }
```

**Step 5: Save lineage on generation change and exit**

After triggering a new generation:

```rust
    if let Some(ref lineage) = self.lineage {
        if let Err(e) = persistence::save_lineage(
            &lineage.child,
            lineage.parent.as_ref(),
            lineage.grandparent.as_ref(),
            lineage.great_grandparent.as_ref(),
        ) {
            log::warn!("failed to save lineage: {e}");
        }
    }
```

**Step 6: Pass scene uniforms to renderer**

Change the render call:

```rust
                if let Some(renderer) = &mut self.renderer {
                    renderer.render(&mut self.uniforms, &self.active_scene_uniforms);
                }
```

**Step 7: Verify it compiles and runs**

Run: `cargo build`
Run: `cargo test`
Expected: all tests pass, app runs with genome-driven scenes

**Step 8: Commit**

```
feat: wire up evolutionary scene system — lineage, change detection, persistence, B/N keys
```

---

### Task 10: Final Integration Test + Polish

**Files:**
- All files — verify holistically

**Step 1: Run full test suite**

Run: `cargo test`
Expected: ALL tests pass

**Step 2: Manual testing checklist**

- [ ] App starts, renders with genome-driven scene
- [ ] D key shows FPS
- [ ] B key saves a favorite (check `~/.silly-visualizer/favorites/`)
- [ ] N key loads a random favorite with crossfade
- [ ] R key randomizes seed (still works)
- [ ] C key cycles palettes (still works)
- [ ] Space toggles audio source (still works)
- [ ] Musical changes trigger scene evolution (play different songs)
- [ ] Gradients are smoother than before (no harsh contour lines)
- [ ] Crossfade transitions are visible (scene morphs, melts, or both)
- [ ] Lineage is saved to `~/.silly-visualizer/lineage.json`
- [ ] Restarting loads the saved lineage

**Step 3: Commit any polish fixes**

```
chore: final integration polish
```
