# Audio Pipeline

How audio flows from capture device to GPU uniform buffer.

## Capture (`src/audio.rs`)

Audio is captured via [cpal](https://docs.rs/cpal) from one of two sources:

| Source | How it works | Key |
|--------|-------------|-----|
| **Mic** | Default input device | Default on startup |
| **Loopback** | Default output device via CoreAudio ProcessTap (macOS 14.6+) | `Space` to toggle |

Samples are pushed into a ring buffer (4096 samples max). Each frame, `get_samples_into()` copies the buffer contents without allocating.

## FFT Analysis (`src/analysis.rs`)

The analyzer runs a 2048-point FFT each frame:

1. **Window** — Hann window applied to raw samples
2. **FFT** — Forward FFT via cached `rustfft` plan (no per-frame allocation)
3. **Magnitudes** — Complex output converted to magnitudes, normalized by FFT size
4. **16 bands** — Logarithmically spaced from low to high frequency

From the 16 bands, summary values are derived:

| Output | Bands | Description |
|--------|-------|-------------|
| `bass` | 0-2 | Low frequency energy (kick drums, bass notes) |
| `mids` | 4-7 | Mid frequency energy (vocals, guitars, snare) |
| `highs` | 10-13 | High frequency energy (cymbals, hi-hats, sibilance) |
| `energy` | 0-15 | Average across all bands (overall loudness) |
| `beat` | — | 1.0 when energy spikes 1.5x above recent average, else 0.0 |

## Smoothing (`src/main.rs`)

Raw FFT values are jittery frame-to-frame. Exponential moving average smooths them:

```
smoothed = smoothed * 0.95 + raw * 0.05
```

Higher retention (0.95) means smoother but more laggy response. This was tuned from 0.80 → 0.92 → 0.95 to reduce scene shaking and soften mic reactivity.

## Auto-Gain (`src/main.rs`)

Quiet audio (vocals, speech) barely registers on the FFT. Auto-gain normalizes the signal:

```
if energy > peak_energy:
    peak_energy += (energy - peak_energy) * 0.04    # fast attack
else:
    peak_energy += (energy - peak_energy) * 0.005   # slow release

target_gain = clamp(0.05 / peak_energy, 1.0, 6.0)
auto_gain += (target_gain - auto_gain) * 0.02
```

- **Fast attack (0.04)** — responds quickly to loud signals, prevents clipping
- **Slow release (0.005)** — holds gain during quiet passages, releases faster than before to avoid over-boosting ambient noise
- **Max gain: 6x** — moderate boost for quiet signals without over-amplifying mic noise

The final gain applied to all audio values is `auto_gain * sensitivity` (sensitivity adjustable via 1-9 keys).

## Uniform Buffer

Smoothed + gained values are packed into `AudioUniforms` (112 bytes, 16-byte aligned) and written to the GPU each frame:

| Field | Offset | Size | Source |
|-------|--------|------|--------|
| `time` | 0 | 4 | Wall clock (seconds since start) |
| `bass` | 4 | 4 | Smoothed + gained bass |
| `mids` | 8 | 4 | Smoothed + gained mids |
| `highs` | 12 | 4 | Smoothed + gained highs |
| `energy` | 16 | 4 | Smoothed + gained energy |
| `beat` | 20 | 4 | Beat indicator (1.0 on beat, decays by 0.15/frame) |
| `seed` | 24 | 4 | Random seed (R key randomizes) |
| `palette_id` | 28 | 4 | Color palette index (C key cycles) |
| `resolution` | 32 | 8 | Window width, height in pixels |
| `_pad2` | 40 | 8 | Alignment padding |
| `bands` | 48 | 64 | 16 raw FFT bands as `array<vec4<f32>, 4>` |

## Related

- [Shader Controls](shader-controls.md) — what the shader does with these values
- [Render Pipeline](render-pipeline.md) — how the uniform buffer reaches the GPU
