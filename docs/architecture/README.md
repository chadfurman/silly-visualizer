# Architecture

How audio gets from your speakers to animated geometry on screen.

## Documents

| Document | Description |
|----------|-------------|
| [Audio Pipeline](audio-pipeline.md) | Mic/loopback capture, FFT analysis, smoothing, auto-gain |
| [Shader Controls](shader-controls.md) | What each audio input controls in the shader |
| [Render Pipeline](render-pipeline.md) | wgpu setup, feedback textures, ping-pong rendering |

## High-Level Flow

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐     ┌─────────┐
│ Audio Device │────▶│ FFT Analysis │────▶│  Smoothing + │────▶│  GPU    │
│ (mic/loop)   │     │ (rustfft)    │     │  Auto-Gain   │     │ Shader  │
└─────────────┘     └──────────────┘     └──────────────┘     └─────────┘
       cpal               analysis.rs          main.rs          renderer.rs
                                                                visualizer.wgsl
```

Audio is captured via cpal, windowed and FFT'd into 16 frequency bands, smoothed with exponential moving averages, auto-gained to normalize quiet/loud signals, then packed into a uniform buffer and sent to the GPU every frame. The fragment shader raymarches SDF geometry whose parameters are modulated by these audio values.
