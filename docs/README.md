# Silly Visualizer

A real-time audio visualizer built with Rust, wgpu, and WGSL shaders. Captures audio from microphone or system loopback, runs FFT analysis, and renders raymarched SDF geometry that reacts to the music.

## Documentation Navigation

| Section | Description |
|---------|-------------|
| [Architecture](architecture/README.md) | How the system works end-to-end |
| [Reference](reference/README.md) | Keyboard shortcuts, shader controls, uniform layouts |
| [Planning](planning/README.md) | Design docs and implementation plans |

## Quick Start

```bash
cargo run
```

Press `D` for FPS counter, `F` for fullscreen, `Space` to toggle mic/loopback.

## Tech Stack

- **Rust** — application logic, audio capture, FFT analysis
- **wgpu** — GPU rendering pipeline
- **WGSL** — fragment shader with raymarched SDFs
- **cpal** — cross-platform audio capture
- **rustfft** — FFT for frequency analysis
