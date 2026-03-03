# Silly Music Visualizer — Design

## Overview

A native Rust app that captures system audio or mic input and renders a continuously evolving psychedelic visualization using wgpu fragment shaders. Impossible geometry, melting fractals, pixel cloud effects — all driven by real-time audio analysis.

## Architecture

Three concurrent components:

1. **Audio Capture Thread** — `cpal` captures mic input; `screencapturekit-rs` captures macOS system audio. Continuously fills a ring buffer with raw PCM samples.
2. **Audio Analysis** — Runs FFT each frame to extract frequency bands (bass, mids, highs, sub-bass), beat detection, and overall energy. Packs into a uniform buffer (~64-128 floats).
3. **Render Loop** — `winit` event loop drives `wgpu`. Each frame: upload audio uniforms + time, draw a full-screen quad, fragment shader does all visual work.

```
┌─────────────┐     ring buffer     ┌──────────────┐     uniforms     ┌────────────────┐
│ Audio Capture├────────────────────>│ FFT/Analysis ├────────────────>│ Fragment Shader │
│   (cpal)    │                     │              │                  │   (wgpu)       │
└─────────────┘                     └──────────────┘                  └────────────────┘
```

## Approach: Fragment Shader Playground

Single full-screen fragment shader receives audio FFT data as uniforms. All visual generation happens in WGSL — fractal math, geometry warping, color cycling. Audio frequency bands drive shader parameters.

### Shader Techniques

- **Raymarching with SDFs** — impossible geometry via morphing shapes (torus, octahedron, gyroid). Twist, fold, and repeat space.
- **Domain repetition & folding** — kaleidoscopic infinite tunnels, mirrored geometry shifting with the beat.
- **Fractal-like iteration** — iterative space folding (Mandelbox/Menger sponge style). Audio energy controls iteration count and fold parameters.
- **Cosine gradient color palettes** — bass drives hue rotation, mids drive saturation, highs drive brightness. Beat hits cause palette shifts.
- **Feedback/trails** — sample previous frame as texture, blend with current frame offset/rotated for "melting" persistence effect.

### Audio-to-Visual Mapping

| Audio Band | Visual Parameter |
|------------|-----------------|
| Bass | Zoom/scale, camera distance, geometry size |
| Mids | Rotation speed, space folding angle |
| Highs | Detail level, edge sharpness, color sparkle |
| Beat | Sudden jumps — flash, palette swap, geometry morph |
| Energy | Overall intensity/chaos level |

## Input & Controls

Minimal — no GUI, keyboard only:

- **Space** — toggle system audio / mic input
- **F** — toggle fullscreen
- **1-9** — adjust audio sensitivity
- **Esc** — quit
- **R** — randomize shader seed

## macOS Audio Capture

- **Primary**: ScreenCaptureKit (macOS 13+) — zero-setup system audio capture
- **Fallback**: `cpal` default input device (covers mic and BlackHole if installed)

## Dependencies

- `winit` — windowing
- `wgpu` — GPU rendering
- `cpal` — audio capture (mic + BlackHole fallback)
- `rustfft` — FFT analysis
- `screencapturekit-rs` or `objc2` — macOS system audio capture
- `bytemuck` — struct-to-GPU byte casting

## Scope (v1)

One continuous generative/evolving visual. No effect switching, no presets. Time drives the base animation, audio modulates everything on top. The visual continuously evolves even with constant music.
