# Evolutionary Scenes + Visual Polish Design

## Context

The visualizer runs smoothly after performance optimizations but has two issues:
1. Grainy/wavy contour lines from raymarcher artifacts amplified by feedback
2. Lack of visual variety — same shapes, same structure every session

User wants an evolutionary scene system where scenes have structural genomes that mutate, with multi-generational lineage and music-driven transitions.

## Part 1: Gradient / Grain Fix

Sources of graininess:
- Raymarcher stepping artifacts (0.8x understep in folded space creates visible bands)
- Sharp normal estimation (fixed ε=0.001 noisy on folded geometry)
- Feedback amplification (trail system re-blends artifacts frame-over-frame)

Fixes in `visualizer.wgsl`:
- Tetrahedron-based normal estimation with ε=0.002 (more stable)
- Blue-noise-approximation dithering from fragment position to break banding
- Softer glow falloff curve
- Feedback sampling with small cross-pattern blur (average 5 taps) to smooth accumulated artifacts

## Part 2: Scene Genome

A genome is a flat struct of ~20 `f32` parameters:

### Shape Slots (4 slots × 4 params = 16 params)
- `shape_type`: 0=off, 1=sphere, 2=torus, 3=octahedron, 4=box
- `scale`: 0.3–2.0
- `position_offset`: -1.0–1.0
- `rotation_speed`: 0.0–2.0

### Combinator Slots (3 × 2 params = 6 params)
- `combinator_type`: 0=smooth_union, 1=smooth_subtraction, 2=smooth_intersection
- `smoothness`: 0.1–1.5

### Space Folding (3 params)
- `fold_iterations`: 1–5
- `fold_scale`: 1.0–3.0
- `fold_offset`: 0.0–1.0

### Domain Repetition (2 params)
- `rep_z`: 2.0–8.0
- `kaleidoscope_folds`: 2–8

### Camera (3 params)
- `cam_distance`: 3.0–8.0
- `orbit_speed`: 0.1–0.5
- `wobble_amount`: 0.0–1.0

### Audio Routing (5 params)

Each scene can have a different "personality" — which audio inputs drive which visual parameters. These weights control how much each audio band influences geometry vs camera vs color:

- `bass_target`: 0=geometry, 1=camera, 2=color (discrete, mutates rarely)
- `mids_target`: 0=geometry, 1=camera, 2=color
- `highs_target`: 0=geometry, 1=camera, 2=color
- `energy_target`: 0=geometry, 1=camera, 2=color
- `beat_target`: 0=geometry, 1=camera, 2=color

In the shader, the routing multiplies the audio value into the targeted subsystem. For example, a scene with `bass_target=0` (geometry) would have bass inflate/morph shapes, while `bass_target=1` (camera) would have bass push the camera around instead. This creates fundamentally different scene personalities without changing the underlying SDF.

### Transition (1 param)
- `transition_type`: 0=param_interpolation, 1=feedback_melt, 2=both

Total: ~36 f32 parameters. Passed to shader as second uniform buffer.

## Part 3: Multi-Generational Lineage

```
Lineage {
    great_grandparent: Option<Genome>,  // weight 0.125
    grandparent: Option<Genome>,         // weight 0.25
    parent: Option<Genome>,              // weight 0.5
    child: Genome,                       // weight 1.0 (active)
}
```

Generation advancement:
1. Great-grandparent dropped
2. Grandparent → great-grandparent
3. Parent → grandparent
4. Child → parent
5. New child mutated from parent with ancestral influence

Mutation rules:
- Structural params (shape_type, combinator_type) mutate rarely, discretely
- Continuous params get Gaussian noise scaled by per-param mutation rate
- Ancestral influence: weighted blend on mutation vector pulls toward ancestor values
- Family resemblance emerges — scenes drift but maintain continuity

## Part 4: Musical Change Detection

Track spectral profile vector [bass, mids, highs, energy, beat_density]:
- `baseline`: slow EMA (τ ≈ 10s)
- `current`: fast EMA (τ ≈ 2s)
- `novelty = euclidean_distance(current, baseline)`
- Trigger new generation when novelty > threshold
- Reset baseline after trigger
- Cooldown: minimum 15s between generations

## Part 5: Crossfade

Three modes, randomly chosen per transition:
1. **Parameter interpolation** (3–5s): lerp genome params. Scene morphs continuously.
2. **Feedback melt** (2–3s): snap to new genome, boost trail_decay/blend_factor. Old scene melts away.
3. **Both** (4s): interpolate + boosted feedback. Smoothest.

## Part 6: Persistence + Favorites

- Auto-save lineage to `~/.silly-visualizer/lineage.json` every generation
- `B` key: bookmark current genome to `~/.silly-visualizer/favorites/`
- `N` key: load random favorite as new child (injected into lineage)
- On startup: load last lineage from disk if exists, else random genome

## Part 7: Testing Strategy

All Rust-side logic testable without GPU:
- **Genome**: mutation stays within bounds, serde round-trips, interpolation produces valid intermediates
- **Lineage**: generation shift correctness, ancestral weights, great-grandparent drops
- **Musical change detection**: synthetic profiles trigger/don't trigger, cooldown enforced
- **Crossfade**: interpolation produces valid genomes at t=0, t=0.5, t=1.0
- **Persistence**: save/load round-trips, favorites directory management

Shader changes verified visually + cargo test for all Rust modules.

## Files Affected

- `src/shaders/visualizer.wgsl` — grain fix + genome-driven scene rendering
- `src/genome.rs` — new: Genome struct, mutation, interpolation, serde
- `src/lineage.rs` — new: multi-generational lineage management
- `src/scene.rs` — new: musical change detection, crossfade orchestration
- `src/persistence.rs` — new: save/load lineage and favorites
- `src/renderer.rs` — second uniform buffer for genome params
- `src/main.rs` — wire up lineage, scene transitions, B/N keys
- `src/analysis.rs` — expose spectral profile for change detection
