struct AudioUniforms {
    time: f32,
    bass: f32,
    mids: f32,
    highs: f32,
    energy: f32,
    beat: f32,
    seed: f32,
    palette_id: f32,
    resolution: vec2<f32>,
    debug_flags: f32,
    slow_energy: f32,
    bands: array<vec4<f32>, 4>,
    // extra: [beat_accumulator, beat_pulse, pad, pad]
    extra: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: AudioUniforms;
@group(0) @binding(1) var prev_frame: texture_2d<f32>;
@group(0) @binding(2) var prev_sampler: sampler;

struct SceneUniforms {
    shapes: array<vec4<f32>, 4>,
    combinators: array<vec4<f32>, 2>,
    folding: vec4<f32>,
    camera: vec4<f32>,
    audio_routing: vec4<f32>,
    // transition: [beat_target, transition_type, distortion_type, distortion_amount]
    transition: vec4<f32>,
}

@group(1) @binding(0) var<uniform> scene: SceneUniforms;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
}

// ─── Constants ───────────────────────────────────────────────────────────────

const PI: f32 = 3.14159265;
const TAU: f32 = 6.28318530;
const MAX_STEPS: i32 = 80;
const MAX_DIST: f32 = 50.0;
const SURF_DIST: f32 = 0.001;

// ─── SDF Primitives ─────────────────────────────────────────────────────────

fn sd_torus(p: vec3<f32>, t: vec2<f32>) -> f32 {
    let q = vec2<f32>(length(p.xz) - t.x, p.y);
    return length(q) - t.y;
}

fn sd_octahedron(p: vec3<f32>, s: f32) -> f32 {
    let ap = abs(p);
    return (ap.x + ap.y + ap.z - s) * 0.57735027;
}

fn sd_sphere(p: vec3<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// ─── SDF Combinators ────────────────────────────────────────────────────────

fn smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}

fn smooth_subtraction(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 - 0.5 * (d2 + d1) / k, 0.0, 1.0);
    return mix(d2, -d1, h) + k * h * (1.0 - h);
}

fn smooth_intersection(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 - 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) + k * h * (1.0 - h);
}

// ─── Color Palettes ─────────────────────────────────────────────────────────
// Each palette is defined by (a, b, c, d) vectors for: a + b * cos(TAU * (c*t + d))
// See https://iquilezles.org/articles/palettes/

const NUM_PALETTES: i32 = 6;

fn palette_params(id: i32) -> array<vec3<f32>, 4> {
    switch (id) {
        // 0: Electric Neon — magentas, cyans, hot pinks
        case 0: {
            return array<vec3<f32>, 4>(
                vec3<f32>(0.5, 0.5, 0.5),
                vec3<f32>(0.6, 0.6, 0.6),
                vec3<f32>(1.0, 1.0, 1.0),
                vec3<f32>(0.0, 0.33, 0.67),
            );
        }
        // 1: Inferno — deep reds, oranges, golds
        case 1: {
            return array<vec3<f32>, 4>(
                vec3<f32>(0.5, 0.5, 0.5),
                vec3<f32>(0.5, 0.5, 0.5),
                vec3<f32>(1.0, 1.0, 1.0),
                vec3<f32>(0.0, 0.10, 0.20),
            );
        }
        // 2: Deep Ocean — teals, blues, aquamarines
        case 2: {
            return array<vec3<f32>, 4>(
                vec3<f32>(0.5, 0.5, 0.5),
                vec3<f32>(0.5, 0.5, 0.5),
                vec3<f32>(1.0, 1.0, 1.0),
                vec3<f32>(0.30, 0.53, 0.67),
            );
        }
        // 3: Vaporwave — purples, pinks, warm cyans
        case 3: {
            return array<vec3<f32>, 4>(
                vec3<f32>(0.5, 0.5, 0.5),
                vec3<f32>(0.6, 0.55, 0.65),
                vec3<f32>(0.8, 0.8, 1.0),
                vec3<f32>(0.55, 0.40, 0.75),
            );
        }
        // 4: Acid — greens, yellows, toxic brights
        case 4: {
            return array<vec3<f32>, 4>(
                vec3<f32>(0.5, 0.5, 0.5),
                vec3<f32>(0.7, 0.65, 0.5),
                vec3<f32>(1.0, 1.0, 1.0),
                vec3<f32>(0.15, 0.25, 0.50),
            );
        }
        // 5: Monochrome — silver, white, cool grays
        default: {
            return array<vec3<f32>, 4>(
                vec3<f32>(0.5, 0.5, 0.5),
                vec3<f32>(0.4, 0.4, 0.45),
                vec3<f32>(1.0, 1.0, 1.0),
                vec3<f32>(0.0, 0.0, 0.05),
            );
        }
    }
}

fn palette(t: f32) -> vec3<f32> {
    let id = i32(u.palette_id) % NUM_PALETTES;
    let p = palette_params(id);
    return p[0] + p[1] * cos(TAU * (p[2] * t + p[3]));
}

fn palette_shifted(t: f32, shift: f32) -> vec3<f32> {
    let id = i32(u.palette_id) % NUM_PALETTES;
    let p = palette_params(id);
    // Asymmetric frequency + shifted phase for psychedelic variety
    let c = p[2] * vec3<f32>(1.0, 0.7, 1.3);
    let d = p[3] + vec3<f32>(shift, shift * 0.6, shift * 0.3);
    let b = p[1] * vec3<f32>(1.1, 1.0, 1.15);
    return p[0] + b * cos(TAU * (c * t + d));
}

// ─── Rotation Matrices ──────────────────────────────────────────────────────

fn rot_x(a: f32) -> mat3x3<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat3x3<f32>(
        vec3<f32>(1.0, 0.0, 0.0),
        vec3<f32>(0.0, c, -s),
        vec3<f32>(0.0, s, c),
    );
}

fn rot_y(a: f32) -> mat3x3<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat3x3<f32>(
        vec3<f32>(c, 0.0, s),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(-s, 0.0, c),
    );
}

fn rot_z(a: f32) -> mat3x3<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat3x3<f32>(
        vec3<f32>(c, -s, 0.0),
        vec3<f32>(s, c, 0.0),
        vec3<f32>(0.0, 0.0, 1.0),
    );
}

// ─── WGSL-safe mod ──────────────────────────────────────────────────────────

fn wmod(x: f32, y: f32) -> f32 {
    return x - floor(x / y) * y;
}

fn wmod3(x: vec3<f32>, y: vec3<f32>) -> vec3<f32> {
    return x - floor(x / y) * y;
}

// ─── Hash for pseudo-noise ──────────────────────────────────────────────────

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.zyx + 31.32);
    return fract((q.x + q.y) * q.z);
}

// ─── Space Folding (Mandelbox-inspired) ─────────────────────────────────────

fn fold_space(p_in: vec3<f32>, iterations: i32, scale: f32) -> vec3<f32> {
    var p = p_in;
    for (var i = 0; i < iterations; i = i + 1) {
        // Box fold: reflect around -1..1 box
        p = clamp(p, vec3<f32>(-1.0), vec3<f32>(1.0)) * 2.0 - p;

        // Sphere fold: invert through sphere
        let r2 = dot(p, p);
        let min_r2 = 0.25;
        let fixed_r2 = 1.0;
        if (r2 < min_r2) {
            let temp = fixed_r2 / min_r2;
            p = p * temp;
        } else if (r2 < fixed_r2) {
            let temp = fixed_r2 / r2;
            p = p * temp;
        }

        p = p * scale + p_in;
    }
    return p;
}

// ─── Genome-driven dispatch ─────────────────────────────────────────────────

// Evaluate an SDF primitive by type: 0=off, 1=sphere, 2=torus, 3=octahedron, 4=box
fn eval_shape(p: vec3<f32>, shape_type: f32, scale: f32) -> f32 {
    let st = i32(shape_type);
    switch (st) {
        case 1: { return sd_sphere(p, scale); }
        case 2: { return sd_torus(p, vec2<f32>(scale, scale * 0.3)); }
        case 3: { return sd_octahedron(p, scale); }
        case 4: { return sd_box(p, vec3<f32>(scale)); }
        default: { return MAX_DIST; }
    }
}

// Dispatch combinator by type: 0=union, 1=subtraction, 2=intersection
fn combine(d1: f32, d2: f32, comb_type: f32, smoothness: f32) -> f32 {
    let ct = i32(comb_type);
    switch (ct) {
        case 1: { return smooth_subtraction(d1, d2, smoothness); }
        case 2: { return smooth_intersection(d1, d2, smoothness); }
        default: { return smooth_union(d1, d2, smoothness); }
    }
}

// Returns total audio contribution routed to a given target
// Targets: 0=geometry, 1=camera, 2=color
fn audio_for_target(tgt: f32) -> f32 {
    var total = 0.0;
    if (abs(scene.audio_routing.x - tgt) < 0.5) { total += u.bass; }
    if (abs(scene.audio_routing.y - tgt) < 0.5) { total += u.mids; }
    if (abs(scene.audio_routing.z - tgt) < 0.5) { total += u.highs; }
    if (abs(scene.audio_routing.w - tgt) < 0.5) { total += u.energy; }
    if (abs(scene.transition.x - tgt) < 0.5) { total += u.beat; }
    return total;
}

// ─── Surface Distortion (Task 4) ────────────────────────────────────────────
// Audio-reactive displacement applied in world space so the distortion pattern
// stays fixed while shapes rotate — like iron filings on a vibrating surface.

fn surface_distortion(p: vec3<f32>) -> f32 {
    let dist_amount = scene.transition.w; // distortion_amount from genome
    if (dist_amount < 0.001) {
        return 0.0;
    }

    let dist_type = i32(scene.transition.z); // distortion_type from genome
    let beat_pulse = u.extra.y;
    let bass = u.bass;
    let mids = u.mids;
    let highs = u.highs;

    // Bass — large-scale undulation
    let bass_wave = sin(p.x * 1.5 + u.time * 0.5) * sin(p.y * 1.5 + u.time * 0.3) * bass * 0.08;

    // Mids — medium undulation
    let mids_wave = sin(p.x * 5.0 + u.time * 0.8) * cos(p.z * 5.0 + u.time * 0.6) * mids * 0.04;

    // Highs — type-dependent character
    var highs_wave = 0.0;
    let r = length(p);
    switch (dist_type) {
        case 0: {
            // Ripple: concentric rings
            highs_wave = sin(r * 15.0 + u.time * 2.0) * highs * 0.03;
        }
        case 1: {
            // Spike: sharp peaks
            let s = sin(p.x * 10.0 + u.time) * sin(p.y * 10.0 - u.time * 0.7) * sin(p.z * 10.0 + u.time * 0.5);
            highs_wave = abs(s * s * s) * highs * 0.06;
        }
        case 2: {
            // Fuzz: hash-based pseudo-noise
            highs_wave = (hash3(p * 8.0 + u.time * 0.3) - 0.5) * highs * 0.05;
        }
        default: {
            // Mixed: ripple + spike combo
            let ripple = sin(r * 12.0 + u.time * 1.5) * 0.5;
            let spike = abs(sin(p.x * 8.0 + u.time) * sin(p.z * 8.0 - u.time * 0.6));
            highs_wave = (ripple + spike) * highs * 0.03;
        }
    }

    // Beat pulse — momentary burst
    let pulse_wave = beat_pulse * 0.05 * sin(r * 8.0 + u.time * 3.0);

    return (bass_wave + mids_wave + highs_wave + pulse_wave) * dist_amount;
}

// ─── Scene SDF ──────────────────────────────────────────────────────────────

fn map(p_in: vec3<f32>) -> f32 {
    let t = u.time;
    let beat_accum = u.extra.x;

    // Genome-driven structural params
    let rep_z = scene.folding.w;
    let kal_folds = max(scene.camera.x, 2.0);

    // Task 8: Fractal fold complexity drift — beat_accumulator drives fold iterations
    let fold_scale = scene.folding.y + beat_accum * 0.3;
    var iter_boost = 0.0;
    if (beat_accum > 0.5) { iter_boost = 1.0; }
    if (beat_accum > 0.8) { iter_boost = 2.0; }
    let fold_iters = i32(clamp(scene.folding.x + iter_boost, 1.0, 5.0));

    // Rotation: steady orbit from genome rot_speed values
    let avg_rot = (scene.shapes[0].w + scene.shapes[1].w) * 0.5;
    var p = rot_y(t * avg_rot * 0.4) * rot_x(t * avg_rot * 0.25) * p_in;

    // ── Space repetition along Z ──
    var rp = p;
    rp.z = wmod(p.z + t * 0.8, rep_z) - rep_z * 0.5;

    // ── Kaleidoscopic angular folding in XY ──
    if (i32(u.debug_flags) != 2) {
        let fold_angle = PI / kal_folds;
        let angle = atan2(rp.y, rp.x);
        let folded_angle = wmod(angle, fold_angle * 2.0) - fold_angle;
        let r_xy = length(rp.xy);
        rp = vec3<f32>(r_xy * cos(folded_angle), r_xy * sin(folded_angle), rp.z);
    }

    // ── Mandelbox-inspired folding ──
    var fp = rp;
    if (i32(u.debug_flags) != 2) {
        fp = fold_space(rp * 0.5, fold_iters, fold_scale) * 2.0;
    }

    // ── Evaluate 4 genome-defined shape slots ──
    // Shapes 0-1 in repeated space, shapes 2-3 in folded space
    let d0 = eval_shape(rp + vec3<f32>(0.0, scene.shapes[0].z, 0.0),
                        scene.shapes[0].x, scene.shapes[0].y);
    let d1 = eval_shape(rp + vec3<f32>(0.0, scene.shapes[1].z, 0.0),
                        scene.shapes[1].x, scene.shapes[1].y);
    let d2 = eval_shape(fp + vec3<f32>(0.0, scene.shapes[2].z, 0.0),
                        scene.shapes[2].x, scene.shapes[2].y);
    let d3 = eval_shape(fp + vec3<f32>(0.0, scene.shapes[3].z, 0.0),
                        scene.shapes[3].x, scene.shapes[3].y);

    // ── Combine shapes using genome combinators ──
    var d = combine(d0, d1, scene.combinators[0].x, scene.combinators[0].y);
    d = combine(d, d2, scene.combinators[0].z, scene.combinators[0].w);
    d = combine(d, d3, scene.combinators[1].x, scene.combinators[1].y);

    // Task 4: Surface distortion — applied in world space (p_in)
    d += surface_distortion(p_in);

    return d;
}

// ─── Normal Estimation ──────────────────────────────────────────────────────

fn calc_normal(p: vec3<f32>) -> vec3<f32> {
    let e = 0.001;
    let d = map(p);
    return normalize(vec3<f32>(
        map(p + vec3<f32>(e, 0.0, 0.0)) - d,
        map(p + vec3<f32>(0.0, e, 0.0)) - d,
        map(p + vec3<f32>(0.0, 0.0, e)) - d,
    ));
}

// ─── Raymarching ────────────────────────────────────────────────────────────

struct RayResult {
    dist: f32,
    total_dist: f32,
    steps: i32,
    closest: f32,
}

fn raymarch(ro: vec3<f32>, rd: vec3<f32>) -> RayResult {
    var result: RayResult;
    result.dist = 0.0;
    result.total_dist = 0.0;
    result.steps = 0;
    result.closest = MAX_DIST;

    // Start rays slightly in front of camera to avoid geometry at the camera
    // origin filling the viewport
    var t = 0.5;
    for (var i = 0; i < MAX_STEPS; i = i + 1) {
        let p = ro + rd * t;
        let d = map(p);

        // Track closest approach for glow
        if (d < result.closest) {
            result.closest = d;
        }

        if (d < SURF_DIST) {
            result.dist = d;
            result.total_dist = t;
            result.steps = i;
            return result;
        }

        if (t > MAX_DIST) {
            break;
        }

        t = t + d * 0.8; // slight understep for safety in folded space
        result.steps = i;
    }

    result.dist = MAX_DIST;
    result.total_dist = t;
    return result;
}

// ─── Audio Bars (debug mode 4) ──────────────────────────────────────────────

fn render_audio_bars(uv: vec2<f32>) -> vec3<f32> {
    let num_bands = 16.0;
    let bar_width = 1.0 / num_bands;
    let band_idx = i32(floor((uv.x + 0.5) / bar_width));
    if (band_idx < 0 || band_idx >= 16) {
        return vec3<f32>(0.0);
    }
    let vec_idx = band_idx / 4;
    let comp_idx = band_idx % 4;
    var band_val = 0.0;
    switch (vec_idx) {
        case 0: {
            switch (comp_idx) { case 0: { band_val = u.bands[0].x; } case 1: { band_val = u.bands[0].y; } case 2: { band_val = u.bands[0].z; } default: { band_val = u.bands[0].w; } }
        }
        case 1: {
            switch (comp_idx) { case 0: { band_val = u.bands[1].x; } case 1: { band_val = u.bands[1].y; } case 2: { band_val = u.bands[1].z; } default: { band_val = u.bands[1].w; } }
        }
        case 2: {
            switch (comp_idx) { case 0: { band_val = u.bands[2].x; } case 1: { band_val = u.bands[2].y; } case 2: { band_val = u.bands[2].z; } default: { band_val = u.bands[2].w; } }
        }
        default: {
            switch (comp_idx) { case 0: { band_val = u.bands[3].x; } case 1: { band_val = u.bands[3].y; } case 2: { band_val = u.bands[3].z; } default: { band_val = u.bands[3].w; } }
        }
    }
    let bar_height = band_val * 5.0;
    let y_norm = uv.y + 0.5;
    if (y_norm < bar_height) {
        let hue = f32(band_idx) / 16.0;
        return palette(hue);
    }
    return vec3<f32>(0.05);
}

// ─── Fragment Shader ────────────────────────────────────────────────────────

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let aspect = u.resolution.x / u.resolution.y;
    let uv = (pos.xy / u.resolution - 0.5) * vec2<f32>(aspect, 1.0);

    let debug_mode = i32(u.debug_flags);

    // Mode 4: Audio bars — pure 2D, skip all raymarching
    if (debug_mode == 4) {
        let bars = render_audio_bars(uv);
        return vec4<f32>(bars, 1.0);
    }

    let t = u.time;
    let bass = u.bass;
    let mids = u.mids;
    let highs = u.highs;
    let energy = u.energy;
    let beat = u.beat;
    let beat_pulse = u.extra.y;
    let beat_accum = u.extra.x;
    let mood = clamp(u.slow_energy * 20.0, 0.0, 1.0);

    // ── Camera setup: smooth orbiting camera (no audio jitter) ──
    let seed = u.seed;
    let cam_dist = scene.camera.y;
    let cam_angle_y = t * scene.camera.z;
    let cam_angle_x = sin(t * 0.15) * scene.camera.w;
    var ro = vec3<f32>(
        cam_dist * sin(cam_angle_y) * cos(cam_angle_x),
        cam_dist * sin(cam_angle_x) + sin(t * 0.3) * 0.3,
        cam_dist * cos(cam_angle_y) * cos(cam_angle_x),
    );
    // Seed offset: pressing R jumps to a completely different visual region
    ro += vec3<f32>(seed * 100.0, seed * 73.0, seed * 37.0);
    let look_at = vec3<f32>(0.0, 0.0, 0.0);
    let forward = normalize(look_at - ro);
    let right = normalize(cross(forward, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, forward);
    let focal = 1.5;
    let rd = normalize(forward * focal + right * uv.x + up * uv.y);

    // ── Single raymarch ──
    let result = raymarch(ro, rd);

    // Mode 3: Normals visualization
    if (debug_mode == 3) {
        if (result.dist < SURF_DIST * 2.0) {
            let hit_p = ro + rd * result.total_dist;
            let n = calc_normal(hit_p);
            let normal_color = n * 0.5 + 0.5;
            return vec4<f32>(normal_color, 1.0);
        }
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // Mode 5: Depth visualization
    if (debug_mode == 5) {
        let depth = 1.0 - clamp(result.total_dist / MAX_DIST, 0.0, 1.0);
        let depth_color = vec3<f32>(depth * depth);
        return vec4<f32>(depth_color, 1.0);
    }

    // ── Lighting setup ──
    let light_pos = vec3<f32>(
        sin(t * 0.7) * 2.0,
        2.0 + cos(t * 0.5),
        cos(t * 0.7) * 2.0,
    );

    // ── Task 5: Color system overhaul — mood-driven coloring ──
    let color_audio = audio_for_target(2.0);
    var color = vec3<f32>(0.0);

    if (result.dist < SURF_DIST * 2.0) {
        let hit_p = ro + rd * result.total_dist;
        let n = calc_normal(hit_p);

        // Mood blends two palette evaluations: warm + cool
        let palette_t = result.total_dist * 0.15 + t * 0.08 + dot(n, vec3<f32>(0.3, 0.6, 0.1));
        let warm_color = palette_shifted(palette_t, color_audio * 0.4);
        let cool_color = palette_shifted(palette_t + 0.33, color_audio * 0.4);
        let gradient_blend = mix(0.4, 0.15, mood); // calm: more blended, energetic: more distinct
        let base_color = mix(warm_color, cool_color, gradient_blend);

        // Lighting with mood-driven contrast and specular
        let light_dir = normalize(light_pos - hit_p);
        let diff = max(dot(n, light_dir), 0.0);
        let spec_power = mix(16.0, 64.0, mood);
        let spec = pow(max(dot(reflect(-light_dir, n), -rd), 0.0), spec_power);

        // Contrast scales with mood
        let contrast = mix(1.0, 1.6, mood);
        let luminance = dot(base_color, vec3<f32>(0.299, 0.587, 0.114));
        let saturated = mix(vec3<f32>(luminance), base_color, contrast);

        // Electric accent at high energy — cyan/magenta mixed in
        let accent = mix(vec3<f32>(0.0, 1.0, 1.0), vec3<f32>(1.0, 0.0, 1.0), sin(palette_t * 3.0) * 0.5 + 0.5);
        let accent_mix = mood * 0.15;

        let brightness = 1.0 + color_audio * 0.3;
        let lit = mix(saturated, accent, accent_mix) * (0.40 + diff * 0.60)
                  + spec * 0.5 * (1.0 + mood * 2.0);
        let fog = exp(-result.total_dist * 0.05);
        color = lit * brightness * fog;
    }

    // ── Task 6: Glow surge with ring pulses ──
    // Inner glow — proximity-based, amplified by beat_pulse and bass
    let inner_glow_intensity = 0.03 / (result.closest + 0.01);
    let inner_glow_color = palette(t * 0.1 + result.total_dist * 0.05 + color_audio);
    let inner_glow_amp = 0.3 + beat_pulse * 2.0 + bass * 0.8;
    var glow = inner_glow_color * inner_glow_intensity * inner_glow_amp;

    // Outer glow — step-count-based, scales with mood
    let step_glow = f32(result.steps) / f32(MAX_STEPS);
    let outer_glow_color = palette_shifted(step_glow + t * 0.02, color_audio * 0.5);
    let outer_glow_strength = mix(0.4, 0.8, mood);
    glow += outer_glow_color * step_glow * step_glow * outer_glow_strength;

    // Bass surge — multiplies total glow
    glow *= 1.0 + bass * 0.8;

    color += glow;

    // ── Ring pulses on beats ──
    let ring_center = length(uv);
    // Ring 1: expands from center as beat_pulse decays
    let ring1_radius = (1.0 - beat_pulse) * 1.5;
    let ring1_dist = abs(ring_center - ring1_radius);
    let ring1 = smoothstep(0.03, 0.0, ring1_dist) * beat_pulse;
    // Ring 2: second concentric ring at 70% phase, 40% intensity
    let ring2_radius = (1.0 - beat_pulse * 0.7) * 1.5;
    let ring2_dist = abs(ring_center - ring2_radius);
    let ring2 = smoothstep(0.03, 0.0, ring2_dist) * beat_pulse * 0.4;
    let ring_color = palette(t * 0.15 + ring_center);
    color += ring_color * (ring1 + ring2);

    // ── Task 7: Softened beat effects ──
    // Palette shift on beat: gentle hue nudge using beat_pulse
    let beat_shift = beat_pulse * 0.10;
    color = mix(color, color.gbr, beat_shift);

    // ── Task 7: Mood-driven adaptive feedback ──
    if (debug_mode != 1) {
        let screen_uv = pos.xy / u.resolution;
        let drift = vec2<f32>(sin(u.time * 0.1) * 0.003, cos(u.time * 0.13) * 0.003);
        // Softened chromatic aberration using beat_pulse
        let ca_offset = beat_pulse * 0.008;
        let prev_r = textureSample(prev_frame, prev_sampler, screen_uv + drift + vec2<f32>(ca_offset, 0.0)).r;
        let prev_g = textureSample(prev_frame, prev_sampler, screen_uv + drift).g;
        let prev_b = textureSample(prev_frame, prev_sampler, screen_uv + drift - vec2<f32>(ca_offset, 0.0)).b;
        let prev_srgb = vec3<f32>(prev_r, prev_g, prev_b);
        let prev_linear = pow(prev_srgb, vec3<f32>(2.2));
        // Calm: more smearing. Energetic: crisper (glow/rings compensate).
        let trail_decay = mix(0.65, 0.50, mood);
        let blend_factor = mix(0.65, 0.78, mood) + beat_pulse * 0.10;
        color = mix(prev_linear * trail_decay, color, blend_factor);
    }

    // ── Vignette (softer) ──
    let vignette_uv = pos.xy / u.resolution - 0.5;
    let vignette = 1.0 - dot(vignette_uv, vignette_uv) * 0.5;
    color = color * vignette;

    // ── Tone mapping: ACES filmic (preserves saturation better than Reinhard) ──
    let a_aces = 2.51;
    let b_aces = 0.03;
    let c_aces = 2.43;
    let d_aces = 0.59;
    let e_aces = 0.14;
    color = clamp((color * (a_aces * color + b_aces)) / (color * (c_aces * color + d_aces) + e_aces), vec3<f32>(0.0), vec3<f32>(1.0));

    // ── Gamma correction ──
    color = pow(color, vec3<f32>(0.4545));

    return vec4<f32>(color, 1.0);
}
