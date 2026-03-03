# Visual Debugging Guide

## How to Describe What You See

| What you see | How to describe it |
|---|---|
| Too dark/muddy | "needs more brightness/contrast" |
| All one color | "needs more color variety" |
| Blurry mess | "too much feedback/trails" |
| Jittery/noisy | "too reactive / not smooth enough" |
| Nothing happens with music | "not reactive enough to audio" |
| Too zoomed in | "camera too close" |
| Too zoomed out | "camera too far" |
| Can't see shapes | "shapes not defined enough" |
| Too busy/chaotic | "too much folding/repetition" |
| Too simple/boring | "needs more complexity" |
| Colors ugly | "try a different palette" (press C) |
| Moves too fast | "slow down rotation/orbit" |
| Moves too slow | "speed up rotation/orbit" |

## Debug Modes (press V to cycle)

| Mode | Name | What it shows | Use when |
|---|---|---|---|
| 0 | Normal | Full rendering | Default |
| 1 | No Feedback | Raw shapes, no trails | Trails are smearing everything |
| 2 | No Folding | Shapes without kaleidoscope/fractal | Folding makes it too chaotic |
| 3 | Normals | Surface angles as rainbow | Checking if geometry is 3D |
| 4 | Audio Bars | 16 frequency bands | Checking if audio works |
| 5 | Depth | White=close, black=far | Finding where shapes are |

## Other Keyboard Shortcuts

| Key | What it does |
|---|---|
| D | Toggle debug info (FPS + audio readout in terminal) |
| C | Cycle color palette |
| 1-9 | Audio sensitivity (5=default, 9=max) |
| N | Force evolve to new scene |
| R | Randomize seed (jump to new visual region) |
| Space | Toggle mic / system audio |
| F | Fullscreen |
| B | Bookmark current scene |
| L | Load random bookmarked scene |
| V | Cycle visual debug modes |
| Shift+R | Start/stop audio recording |

## Debugging Workflow

1. Run with `RUST_LOG=info cargo run`
2. Press D to see audio values in terminal
3. Press V to try each debug mode
4. Use the vocabulary table above to describe issues
5. Press C to try different color palettes
6. Press 1-9 to adjust audio sensitivity
