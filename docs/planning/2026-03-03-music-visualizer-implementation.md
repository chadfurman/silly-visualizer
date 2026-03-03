# Silly Music Visualizer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a native Rust app that captures system audio or mic and renders a continuously evolving psychedelic visualization via wgpu fragment shaders.

**Architecture:** Audio capture thread (cpal) fills a ring buffer → FFT analysis extracts frequency bands each frame → uniforms uploaded to GPU → full-screen fragment shader generates all visuals. Previous frame fed back as texture for melting/trail effects.

**Tech Stack:** Rust, wgpu 28.0, winit 0.30, cpal 0.17, rustfft 6.4, bytemuck 1.25

---

### Task 1: Project Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `.gitignore`

**Step 1: Initialize cargo project**

Run: `cd ~/projects/silly-visualizer && cargo init`

**Step 2: Set up Cargo.toml with dependencies**

```toml
[package]
name = "silly-visualizer"
version = "0.1.0"
edition = "2024"

[dependencies]
wgpu = "28"
winit = "0.30"
cpal = "0.17"
rustfft = "6.4"
bytemuck = { version = "1.25", features = ["derive"] }
pollster = "0.4"
log = "0.4"
env_logger = "0.11"
```

**Step 3: Write minimal main.rs that opens a window**

```rust
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Window>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = Window::default_attributes()
                .with_title("silly visualizer");
            self.window = Some(event_loop.create_window(attrs).unwrap());
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = App { window: None };
    event_loop.run_app(&mut app).unwrap();
}
```

**Step 4: Verify it compiles and opens a window**

Run: `cd ~/projects/silly-visualizer && cargo run`
Expected: A blank window titled "silly visualizer" opens and closes cleanly with Cmd+W or the X button.

**Step 5: Add .gitignore and commit**

```gitignore
/target
```

Run:
```bash
cd ~/projects/silly-visualizer
git add Cargo.toml src/main.rs .gitignore
git commit -m "feat: project scaffold with winit window"
```

---

### Task 2: wgpu Rendering Pipeline

**Files:**
- Modify: `src/main.rs`
- Create: `src/renderer.rs`

**Step 1: Create renderer module with wgpu setup**

Create `src/renderer.rs` with a `Renderer` struct that holds:
- `wgpu::Device`
- `wgpu::Queue`
- `wgpu::Surface`
- `wgpu::SurfaceConfiguration`
- `wgpu::RenderPipeline`

The renderer should:
- Initialize wgpu with `pollster::block_on` inside `resumed`
- Create a surface from the winit window
- Set up a render pipeline with a vertex shader that draws a full-screen triangle (3 vertices, no vertex buffer — compute positions in shader)
- Set up a fragment shader that outputs a solid color for now

```wgsl
// Vertex shader - fullscreen triangle trick (no vertex buffer needed)
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(0.1, 0.0, 0.2, 1.0); // dark purple
}
```

**Step 2: Integrate renderer into App**

Modify `src/main.rs`:
- In `resumed`, after creating the window, create the `Renderer`
- In `window_event`, handle `RedrawRequested` to call `renderer.render()`
- Request redraw continuously (call `window.request_redraw()` after each frame)

**Step 3: Verify purple screen renders**

Run: `cargo run`
Expected: Window shows a solid dark purple fill.

**Step 4: Commit**

Run:
```bash
git add src/renderer.rs src/main.rs
git commit -m "feat: wgpu rendering pipeline with fullscreen quad"
```

---

### Task 3: Uniform Buffer for Shader Parameters

**Files:**
- Modify: `src/renderer.rs`
- Create: `src/shaders/visualizer.wgsl`

**Step 1: Define the uniform struct**

```rust
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AudioUniforms {
    pub time: f32,
    pub bass: f32,
    pub mids: f32,
    pub highs: f32,
    pub energy: f32,
    pub beat: f32,
    pub resolution: [f32; 2],  // width, height
    pub bands: [f32; 16],      // 16 frequency bands
}
```

Total: 96 bytes (24 floats × 4 bytes). Fits in a single uniform buffer.

**Step 2: Create uniform buffer and bind group in Renderer**

- Create a `wgpu::Buffer` with `BufferUsages::UNIFORM | BufferUsages::COPY_DST`
- Create a `wgpu::BindGroupLayout` with one uniform buffer binding at group 0, binding 0
- Create a `wgpu::BindGroup` referencing the buffer
- Update the render pipeline layout to use this bind group layout

**Step 3: Update shader to read uniforms**

Move shader to `src/shaders/visualizer.wgsl`:

```wgsl
struct AudioUniforms {
    time: f32,
    bass: f32,
    mids: f32,
    highs: f32,
    energy: f32,
    beat: f32,
    resolution: vec2<f32>,
    bands: array<f32, 16>,
}

@group(0) @binding(0) var<uniform> u: AudioUniforms;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = pos.xy / u.resolution;
    let r = 0.5 + 0.5 * sin(u.time + uv.x * 6.28);
    let g = 0.5 + 0.5 * sin(u.time * 1.3 + uv.y * 6.28);
    let b = 0.5 + 0.5 * sin(u.time * 0.7 + (uv.x + uv.y) * 3.14);
    return vec4<f32>(r, g, b, 1.0);
}
```

**Step 4: Write uniform data each frame**

In the render loop, before the render pass:
- Update `time` field using `std::time::Instant` elapsed since start
- Write the uniform buffer with `queue.write_buffer()`
- Set the bind group in the render pass

**Step 5: Verify animated colors**

Run: `cargo run`
Expected: Window shows smoothly animating rainbow colors that shift over time.

**Step 6: Commit**

Run:
```bash
git add src/renderer.rs src/shaders/visualizer.wgsl
git commit -m "feat: uniform buffer with time-based animated shader"
```

---

### Task 4: Audio Capture with cpal

**Files:**
- Create: `src/audio.rs`
- Modify: `src/main.rs`

**Step 1: Create audio module**

Create `src/audio.rs` with an `AudioCapture` struct that:
- Holds a `cpal::Stream` (kept alive to prevent dropping)
- Shares a ring buffer with the main thread via `Arc<Mutex<Vec<f32>>>`
- Has methods: `new_mic()`, `new_loopback()`, `get_samples() -> Vec<f32>`

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

pub struct AudioCapture {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
}

impl AudioCapture {
    pub fn new_default_input() -> Self {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .expect("no input device available");
        let config = device.default_input_config()
            .expect("no default input config");

        let buffer = Arc::new(Mutex::new(Vec::with_capacity(4096)));
        let buffer_clone = buffer.clone();

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut buf = buffer_clone.lock().unwrap();
                // Keep last 4096 samples
                buf.extend_from_slice(data);
                if buf.len() > 4096 {
                    let drain = buf.len() - 4096;
                    buf.drain(..drain);
                }
            },
            |err| eprintln!("audio error: {}", err),
            None,
        ).unwrap();

        stream.play().unwrap();

        Self { _stream: stream, buffer }
    }

    pub fn get_samples(&self) -> Vec<f32> {
        self.buffer.lock().unwrap().clone()
    }
}
```

**Step 2: Test mic capture works**

Add a temporary `println!` in main to verify samples are coming in:

```rust
// In the render loop temporarily:
let samples = audio.get_samples();
if !samples.is_empty() {
    println!("got {} samples, max amplitude: {:.3}", samples.len(),
             samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max));
}
```

Run: `cargo run`
Expected: Console prints sample counts and amplitudes. May need to allow mic permission in macOS System Settings.

**Step 3: Add loopback capture (system audio)**

cpal 0.17+ has CoreAudio loopback via `AudioHardwareCreateProcessTap` (macOS 14.2+). Check if `host.input_devices()` lists a loopback device, or use the host-specific loopback API if available. If not directly available in cpal 0.17.3, fall back to screencapturekit crate.

Add a method `new_loopback()` that attempts system audio capture.

**Step 4: Wire audio into App struct**

- Create `AudioCapture` before the event loop starts (audio doesn't need a window)
- Store it in the `App` struct
- Pass samples to the renderer each frame

**Step 5: Remove debug println, commit**

Run:
```bash
git add src/audio.rs src/main.rs
git commit -m "feat: audio capture via cpal with mic input"
```

---

### Task 5: FFT Analysis

**Files:**
- Create: `src/analysis.rs`
- Modify: `src/main.rs`

**Step 1: Create analysis module**

Create `src/analysis.rs` with an `AudioAnalyzer` struct:

```rust
use rustfft::{FftPlanner, num_complex::Complex};

pub struct AudioAnalyzer {
    planner: FftPlanner<f32>,
    fft_size: usize,
    window: Vec<f32>,  // Hann window
    prev_energy: f32,
}

pub struct AnalysisResult {
    pub bands: [f32; 16],
    pub bass: f32,
    pub mids: f32,
    pub highs: f32,
    pub energy: f32,
    pub beat: f32,
}

impl AudioAnalyzer {
    pub fn new(fft_size: usize) -> Self {
        // Pre-compute Hann window
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / fft_size as f32).cos())
            })
            .collect();

        Self {
            planner: FftPlanner::new(),
            fft_size,
            window,
            prev_energy: 0.0,
        }
    }

    pub fn analyze(&mut self, samples: &[f32]) -> AnalysisResult {
        // Apply window and FFT
        let fft = self.planner.plan_fft_forward(self.fft_size);
        let mut buffer: Vec<Complex<f32>> = samples.iter()
            .take(self.fft_size)
            .zip(self.window.iter())
            .map(|(s, w)| Complex::new(s * w, 0.0))
            .collect();

        // Pad if not enough samples
        buffer.resize(self.fft_size, Complex::new(0.0, 0.0));

        fft.process(&mut buffer);

        // Convert to magnitudes (only first half — Nyquist)
        let magnitudes: Vec<f32> = buffer[..self.fft_size / 2]
            .iter()
            .map(|c| c.norm() / self.fft_size as f32)
            .collect();

        // Split into 16 bands (logarithmic distribution)
        let mut bands = [0.0f32; 16];
        let half = magnitudes.len();
        for i in 0..16 {
            let start = (half as f32 * (2.0f32.powf(i as f32 / 16.0 * 10.0) - 1.0) / 1023.0) as usize;
            let end = (half as f32 * (2.0f32.powf((i + 1) as f32 / 16.0 * 10.0) - 1.0) / 1023.0) as usize;
            let start = start.min(half - 1);
            let end = end.min(half).max(start + 1);
            bands[i] = magnitudes[start..end].iter().sum::<f32>() / (end - start) as f32;
        }

        // Aggregate bands
        let bass = (bands[0] + bands[1] + bands[2]) / 3.0;
        let mids = (bands[4] + bands[5] + bands[6] + bands[7]) / 4.0;
        let highs = (bands[10] + bands[11] + bands[12] + bands[13]) / 4.0;
        let energy = bands.iter().sum::<f32>() / 16.0;

        // Simple beat detection: energy spike compared to recent average
        let beat = if energy > self.prev_energy * 1.5 && energy > 0.01 {
            1.0
        } else {
            0.0
        };
        self.prev_energy = self.prev_energy * 0.9 + energy * 0.1; // smooth

        AnalysisResult { bands, bass, mids, highs, energy, beat }
    }
}
```

**Step 2: Integrate analyzer into render loop**

In `main.rs`:
- Create `AudioAnalyzer::new(2048)` at startup
- Each frame: get samples from `AudioCapture`, run `analyzer.analyze(&samples)`
- Pack result into `AudioUniforms` and upload to GPU

**Step 3: Update shader to react to audio**

Modify `visualizer.wgsl` to use `u.bass`, `u.mids`, `u.highs` to modulate the colors:

```wgsl
@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = pos.xy / u.resolution;
    let r = 0.5 + 0.5 * sin(u.time + uv.x * 6.28 + u.bass * 10.0);
    let g = 0.5 + 0.5 * sin(u.time * 1.3 + uv.y * 6.28 + u.mids * 10.0);
    let b = 0.5 + 0.5 * sin(u.time * 0.7 + (uv.x + uv.y) * 3.14 + u.highs * 10.0);
    let intensity = 0.7 + u.energy * 3.0;
    return vec4<f32>(r * intensity, g * intensity, b * intensity, 1.0);
}
```

**Step 4: Verify audio-reactive colors**

Run: `cargo run`
Expected: Colors shift and brighten when you play music or speak into the mic.

**Step 5: Commit**

Run:
```bash
git add src/analysis.rs src/main.rs src/shaders/visualizer.wgsl
git commit -m "feat: FFT analysis with audio-reactive shader"
```

---

### Task 6: Psychedelic Raymarching Shader

**Files:**
- Modify: `src/shaders/visualizer.wgsl`

This is the big creative task. Replace the simple color shader with a full raymarching shader.

**Step 1: Write the raymarching shader**

Replace `visualizer.wgsl` with a shader that implements:

1. **Camera setup**: Ray origin + direction from UV coordinates
2. **SDF scene**: Combine multiple shapes with smooth union/subtraction
   - Morphing torus/octahedron based on `u.time` and `u.bass`
   - Space repetition (infinite tunnel) with `fract()` and domain folding
   - Iterative folding (Mandelbox-like) controlled by `u.energy`
3. **Raymarching loop**: March rays through the SDF, max ~80 steps
4. **Coloring**: Cosine palettes driven by distance, normals, and audio bands
5. **Post-processing**: Glow on close misses, chromatic aberration on `u.beat`

Key SDF primitives needed:
```wgsl
fn sd_torus(p: vec3<f32>, t: vec2<f32>) -> f32 {
    let q = vec2<f32>(length(p.xz) - t.x, p.y);
    return length(q) - t.y;
}

fn sd_octahedron(p: vec3<f32>, s: f32) -> f32 {
    let ap = abs(p);
    return (ap.x + ap.y + ap.z - s) * 0.57735027;
}

fn smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}
```

Cosine palette function:
```wgsl
fn palette(t: f32) -> vec3<f32> {
    let a = vec3<f32>(0.5, 0.5, 0.5);
    let b = vec3<f32>(0.5, 0.5, 0.5);
    let c = vec3<f32>(1.0, 1.0, 1.0);
    let d = vec3<f32>(0.263, 0.416, 0.557);
    return a + b * cos(6.28318 * (c * t + d));
}
```

**Step 2: Verify the psychedelic visuals**

Run: `cargo run`
Expected: Trippy, evolving geometry that reacts to audio. Should hit 60fps on a modern Mac GPU.

**Step 3: Iterate on the shader**

Tweak parameters until it looks good:
- Adjust how much each audio band affects each visual parameter
- Tune the color palette
- Adjust raymarching step count vs performance
- Add/remove space folding iterations

**Step 4: Commit**

Run:
```bash
git add src/shaders/visualizer.wgsl
git commit -m "feat: psychedelic raymarching shader with SDF geometry"
```

---

### Task 7: Feedback/Trail Effect (Previous Frame Texture)

**Files:**
- Modify: `src/renderer.rs`
- Modify: `src/shaders/visualizer.wgsl`

**Step 1: Create two textures for ping-pong rendering**

In `Renderer`, create two render textures (same size as surface):
- `texture_a` and `texture_b`
- Each frame, render to one and read from the other
- After rendering, copy the result to the "previous frame" texture

**Step 2: Add texture sampler to bind group**

Add a second binding in the bind group layout:
- `@group(0) @binding(1)` — texture_2d
- `@group(0) @binding(2)` — sampler

**Step 3: Update shader to blend with previous frame**

```wgsl
@group(0) @binding(1) var prev_frame: texture_2d<f32>;
@group(0) @binding(2) var prev_sampler: sampler;

// In fs_main, after computing current color:
let prev_uv = uv + vec2<f32>(sin(u.time * 0.1) * 0.002, cos(u.time * 0.13) * 0.002);
let prev_color = textureSample(prev_frame, prev_sampler, prev_uv);
let final_color = mix(prev_color.rgb, current_color, 0.15 + u.beat * 0.3);
```

The `0.15` blend factor means 85% of the previous frame persists — this creates heavy trails. The slight UV offset creates the "melting" drift. Beat hits increase the blend to current, making beats "punch through" the trails.

**Step 4: Verify melting trail effect**

Run: `cargo run`
Expected: Visuals leave ghostly trails that slowly drift and melt. Beat hits cause sharp new geometry to appear through the trails.

**Step 5: Commit**

Run:
```bash
git add src/renderer.rs src/shaders/visualizer.wgsl
git commit -m "feat: feedback loop with melting trail effect"
```

---

### Task 8: Keyboard Controls

**Files:**
- Modify: `src/main.rs`

**Step 1: Add key handling in window_event**

In the `WindowEvent::KeyboardInput` handler:

```rust
WindowEvent::KeyboardInput { event, .. } => {
    if event.state == ElementState::Pressed {
        match event.logical_key.as_ref() {
            Key::Named(NamedKey::Escape) => event_loop.exit(),
            Key::Named(NamedKey::Space) => {
                // Toggle between mic and system audio
                self.toggle_audio_source();
            }
            Key::Character("f") => {
                // Toggle fullscreen
                let window = self.window.as_ref().unwrap();
                if window.fullscreen().is_some() {
                    window.set_fullscreen(None);
                } else {
                    window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                }
            }
            Key::Character("r") => {
                // Randomize shader seed
                self.renderer.as_mut().unwrap().randomize_seed();
            }
            Key::Character(c) if c.len() == 1 => {
                if let Some(digit) = c.chars().next().and_then(|c| c.to_digit(10)) {
                    if digit >= 1 && digit <= 9 {
                        self.sensitivity = digit as f32 / 5.0;
                    }
                }
            }
            _ => {}
        }
    }
}
```

**Step 2: Add seed uniform to shader**

Add a `seed: f32` field to `AudioUniforms`. Use it in the shader to offset the starting position/rotation of the scene, so pressing R jumps to a different visual.

**Step 3: Verify controls work**

Run: `cargo run`
Expected: Esc quits, F toggles fullscreen, R changes the visual, 1-9 adjusts sensitivity, Space toggles audio source.

**Step 4: Commit**

Run:
```bash
git add src/main.rs src/renderer.rs src/shaders/visualizer.wgsl
git commit -m "feat: keyboard controls for fullscreen, sensitivity, seed"
```

---

### Task 9: System Audio Loopback

**Files:**
- Modify: `src/audio.rs`
- Modify: `src/main.rs`

**Step 1: Investigate cpal loopback support**

Check if `cpal 0.17.3` exposes loopback/process tap on macOS. Look for:
- A loopback device in `host.input_devices()`
- Host-specific extension traits for CoreAudio process tap
- If not available, add `screencapturekit = "1.5"` to Cargo.toml and use its audio stream API

**Step 2: Implement loopback capture**

Add `AudioCapture::new_loopback()`:
- If cpal has native loopback: use it directly
- If not: use screencapturekit to create an audio-only capture stream, feed samples into the same ring buffer

**Step 3: Wire toggle into Space key**

When Space is pressed:
- Drop the current `AudioCapture`
- Create a new one with the opposite source (mic ↔ loopback)
- Print which source is active to the console

**Step 4: Verify system audio capture**

Run: `cargo run`, play music, press Space to switch to system audio.
Expected: Visualizer reacts to system audio playback.

**Step 5: Commit**

Run:
```bash
git add src/audio.rs src/main.rs Cargo.toml
git commit -m "feat: system audio loopback capture with Space to toggle"
```

---

### Task 10: Polish and Cleanup

**Files:**
- Modify: various

**Step 1: Handle window resize**

In `WindowEvent::Resized`, reconfigure the wgpu surface and recreate feedback textures at the new size. Update `resolution` uniform.

**Step 2: Smooth audio values**

Add exponential smoothing to the audio analysis values so they don't jump harshly between frames:

```rust
// In the render loop:
smoothed_bass = smoothed_bass * 0.8 + analysis.bass * 0.2;
smoothed_mids = smoothed_mids * 0.8 + analysis.mids * 0.2;
// etc.
```

**Step 3: Handle edge cases**

- No audio device available → print error and run with zero audio (still animates on time)
- Window minimized → skip rendering
- macOS permission dialogs for mic access

**Step 4: Final test**

Run: `cargo run`
Expected: Smooth psychedelic visuals reacting to audio, no crashes on resize/fullscreen toggle, clean shutdown.

**Step 5: Commit**

Run:
```bash
git add -A
git commit -m "feat: polish - resize handling, audio smoothing, edge cases"
```
