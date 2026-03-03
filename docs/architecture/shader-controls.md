# Shader Controls

What each audio input controls in the fragment shader (`src/shaders/visualizer.wgsl`).

## Input Summary

The shader receives these audio-reactive values every frame:

| Input | Range | What drives it |
|-------|-------|----------------|
| `bass` | ~0.0–1.0+ | Low frequency FFT energy |
| `mids` | ~0.0–1.0+ | Mid frequency FFT energy |
| `highs` | ~0.0–1.0+ | High frequency FFT energy |
| `energy` | ~0.0–1.0+ | Overall loudness |
| `beat` | 0.0–1.0 | 1.0 on beat detection, decays by 0.15/frame |
| `time` | 0.0–inf | Wall clock seconds (always ticking) |

Note: bass/mids/highs/energy can exceed 1.0 due to auto-gain boosting quiet signals.

## Camera (lines 358-375)

| Parameter | Expression | Audio influence |
|-----------|-----------|-----------------|
| Motion gate | `clamp(energy * 3.0, 0.05, 1.0)` | Scales all time-based motion by energy |
| Camera distance | `5.0 - bass * 0.3` | Bass pushes camera closer |
| Orbit angle Y | `t * 0.2 * motion + mids * 0.1` | Energy-gated orbit + mids nudge |
| Orbit angle X | `sin(t * 0.15) * 0.3 * motion` | Energy-gated vertical angle |
| Camera Y offset | `sin(t * 0.3) * 0.3 * motion` | Energy-gated bob |
| Focal length | `1.5 - energy * 0.1` | Energy slightly widens FOV |

Camera motion is gated by `energy * 3.0` — in silence the scene is nearly still (5% residual drift), and as audio increases motion ramps up.

## Geometry / Scene SDF (lines 225-283)

### Rotation

| Parameter | Expression | Audio influence |
|-----------|-----------|-----------------|
| Rotation speed | `(0.1 + mids * 0.3) * motion_gate` | Energy-gated, near-still in silence |

### Domain Repetition

| Parameter | Expression | Audio influence |
|-----------|-----------|-----------------|
| Z repetition | `4.0 - bass * 0.3` | Bass compresses the tunnel |
| Kaleidoscope angle | `PI / (3.0 + mids * 0.8)` | Mids change fold symmetry |

### Space Folding (Mandelbox-like)

| Parameter | Expression | Audio influence |
|-----------|-----------|-----------------|
| Fold iterations | `clamp(1.0 + energy * 2.0, 1, 4)` | Energy adds complexity |
| Fold scale | `1.5 + energy * 0.3` | Energy increases fractal density |

### Primary Shape (torus/octahedron morph)

| Parameter | Expression | Audio influence |
|-----------|-----------|-----------------|
| Morph factor | `0.5 + 0.5 * sin(t * 0.6 * motion_gate + bass * 0.8)` | Energy-gated morph, bass shifts phase |
| Geometry scale | `1.0 + bass * 0.25` | Bass inflates shapes |
| Torus thickness | `0.3 + bass * 0.08` | Bass fattens torus ring |

### Secondary Shapes (folded space)

| Parameter | Expression | Audio influence |
|-----------|-----------|-----------------|
| Folded sphere radius | `0.8 + highs * 0.2` | Highs grow spheres |
| Folded box size | `0.5 + mids * 0.1` | Mids grow boxes |

### Carved Detail

| Parameter | Expression | Audio influence |
|-----------|-----------|-----------------|
| Carve rotation | `t * 1.2 + mids * 0.5` | Time-driven + mids offset |
| Carve size | `0.6 + highs * 0.2` | Highs enlarge carved holes |
| Carve smoothness | `0.3 + energy * 0.08` | Energy softens carved edges |

### Pulsing Spheres

| Parameter | Expression | Audio influence |
|-----------|-----------|-----------------|
| Pulse radius | `0.3 + 0.1 * sin(t * 2.0 * motion_gate + bass * PI)` | Energy-gated pulse, bass shifts phase |

## Color & Lighting (lines 388-426)

| Effect | What controls it |
|--------|-----------------|
| Surface palette | Distance + time + normals + bass/beat as palette shift |
| Specular sharpness | `16.0 + highs * 32.0` — highs make highlights sharper |
| Saturation boost | `1.2 + mids * 0.8` — mids increase color saturation |
| Brightness | `1.0 + highs * 0.5` — highs brighten the scene |
| Glow intensity | `0.03 / (closest + 0.01)` — near-miss distance (geometric) |
| Glow brightness | `0.4 + highs * 1.5` — highs amplify glow |
| Beat flash | `beat^2 * 0.2` — white additive flash on beat |
| Palette hue shift | `beat * 0.25` — beat rotates RGB channels |

## Feedback / Trail (lines 428-441)

| Parameter | Expression | Audio influence |
|-----------|-----------|-----------------|
| Screen drift | `sin/cos(t * 0.1/0.13) * 0.003` | Time-only — slow swirl |
| Chromatic aberration | `beat * 0.015` | Beat splits RGB channels |
| Trail decay | `0.85` (constant) | No audio influence |
| Blend factor | `0.45 + beat * 0.25` | Beat increases new frame dominance |

## Issues & Future Work

### Fixed: Constant Motion Without Audio

All time-driven motion is now gated by `motion_gate = clamp(energy * 3.0, 0.05, 1.0)`. In silence, only 5% residual drift remains. Camera orbit, bob, shape rotation, morph, and pulse all scale with audio energy.

### Problem: Beat Only Affects Color

Beat triggers white flash and hue rotation but has zero influence on geometry. Beats should deform shapes, trigger fold changes, or expand/contract geometry.

### Planned Fix: Genome System

The [evolutionary scenes design](../planning/2026-03-03-evolutionary-scenes-design.md) replaces all hardcoded values with genome-controlled parameters sent via a second uniform buffer (`SceneUniforms`). Each genome defines:

- Shape types, scales, offsets, rotation speeds (4 shape slots)
- Combinator types and smoothness (3 combinators)
- Fold iterations, scale, offset
- Domain repetition spacing, kaleidoscope folds
- Camera distance, orbit speed, wobble amount
- Transition style for crossfades

This means different "scenes" will have fundamentally different geometry and camera behavior, and music-driven transitions will evolve scenes over time.

## Related

- [Audio Pipeline](audio-pipeline.md) — how audio values are derived
- [Evolutionary Scenes Design](../planning/2026-03-03-evolutionary-scenes-design.md) — genome system that will replace hardcoded controls
