# Render Pipeline

How frames get from the shader to the screen, including the feedback loop.

## Overview

The renderer uses a ping-pong feedback architecture: two offscreen textures alternate as "current frame" and "previous frame". The shader reads the previous frame to create melting trail effects, then the result is copied to the display surface.

```
Frame N:
┌──────────────┐     ┌─────────────┐     ┌──────────────┐
│ Texture B    │────▶│ Shader      │────▶│ Texture A    │──── Copy ──▶ Screen
│ (prev frame) │     │ (raymarch + │     │ (new frame)  │
│              │     │  feedback)  │     │              │
└──────────────┘     └─────────────┘     └──────────────┘

Frame N+1:
┌──────────────┐     ┌─────────────┐     ┌──────────────┐
│ Texture A    │────▶│ Shader      │────▶│ Texture B    │──── Copy ──▶ Screen
│ (prev frame) │     │ (raymarch + │     │ (new frame)  │
│              │     │  feedback)  │     │              │
└──────────────┘     └─────────────┘     └──────────────┘
```

## Components (`src/renderer.rs`)

### Uniform Buffer

Single `AudioUniforms` buffer (112 bytes) written every frame. See [Audio Pipeline](audio-pipeline.md) for field layout.

### Bind Groups

Two bind groups for ping-pong, each containing:
- `@binding(0)` — uniform buffer (same for both)
- `@binding(1)` — previous frame texture (alternates A/B)
- `@binding(2)` — linear sampler for smooth feedback

### Feedback Textures

Two textures at window resolution, recreated on resize. Format matches the surface (typically `Bgra8UnormSrgb`).

### Render Pass

Each frame:
1. Write updated uniforms to GPU
2. Render fullscreen triangle to current feedback texture (shader reads previous texture)
3. Copy current feedback texture to display surface via `copy_texture_to_texture`
4. Swap ping-pong index

### Seed System

`R` key calls `randomize_seed()` which sets `seed` from system clock nanoseconds. The shader uses `seed * 100/73/37` as camera position offset, jumping to a completely different visual region.

## Performance Notes

- Single raymarch per pixel (was 3x for chromatic aberration, moved to screen-space UV offset)
- 80 max raymarching steps, 0.8x understep for safety in folded space
- Feedback texture copy uses DMA (`copy_texture_to_texture`), not a blit pass

## Related

- [Shader Controls](shader-controls.md) — what the shader does per-pixel
- [Audio Pipeline](audio-pipeline.md) — how uniform values are derived
