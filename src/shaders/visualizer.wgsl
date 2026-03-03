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
    _pad2: vec2<f32>,
    bands: array<vec4<f32>, 4>,
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
fn audio_for_target(target: f32) -> f32 {
    var total = 0.0;
    if (abs(scene.audio_routing.x - target) < 0.5) { total += u.bass; }
    if (abs(scene.audio_routing.y - target) < 0.5) { total += u.mids; }
    if (abs(scene.audio_routing.z - target) < 0.5) { total += u.highs; }
    if (abs(scene.audio_routing.w - target) < 0.5) { total += u.energy; }
    if (abs(scene.transition.x - target) < 0.5) { total += u.beat; }
    return total;
}

// ─── Scene SDF ──────────────────────────────────────────────────────────────

fn map(p_in: vec3<f32>) -> f32 {
    let t = u.time;

    // Global audio level for fundamental behaviors (motion gate, presence)
    let audio_level = max(max(u.bass, u.mids), max(u.highs, u.energy));
    let motion_gate = clamp(audio_level * 8.0, 0.05, 1.0);

    // Audio routed to geometry modulation via genome
    let geo_audio = audio_for_target(0.0);

    // Genome-driven structural params
    let rep_z = scene.folding.w - geo_audio * 0.5;
    let kal_folds = max(scene.camera.x, 2.0);
    let fold_iters = i32(clamp(scene.folding.x, 1.0, 5.0));
    let fold_scale = scene.folding.y + geo_audio * 0.3;

    // Rotation: average rot_speed from active shapes, gated by audio
    let avg_rot = (scene.shapes[0].w + scene.shapes[1].w) * 0.5;
    let rot_speed = (avg_rot + geo_audio * 0.3) * motion_gate;
    var p = rot_y(t * rot_speed * 0.4) * rot_x(t * rot_speed * 0.25) * p_in;

    // ── Space repetition along Z ──
    var rp = p;
    rp.z = wmod(p.z + t * 0.8 * motion_gate, rep_z) - rep_z * 0.5;

    // ── Kaleidoscopic angular folding in XY ──
    let fold_angle = PI / kal_folds;
    let angle = atan2(rp.y, rp.x);
    let folded_angle = wmod(angle, fold_angle * 2.0) - fold_angle;
    let r_xy = length(rp.xy);
    rp = vec3<f32>(r_xy * cos(folded_angle), r_xy * sin(folded_angle), rp.z);

    // ── Mandelbox-inspired folding ──
    let fp = fold_space(rp * 0.5, fold_iters, fold_scale) * 2.0;

    // Audio-driven size modulation
    let geo_mod = 1.0 + geo_audio * 0.4;

    // ── Evaluate 4 genome-defined shape slots ──
    // Shapes 0-1 in repeated space, shapes 2-3 in folded space
    let d0 = eval_shape(rp + vec3<f32>(0.0, scene.shapes[0].z, 0.0),
                        scene.shapes[0].x, scene.shapes[0].y * geo_mod);
    let d1 = eval_shape(rp + vec3<f32>(0.0, scene.shapes[1].z, 0.0),
                        scene.shapes[1].x, scene.shapes[1].y * geo_mod);
    let d2 = eval_shape(fp + vec3<f32>(0.0, scene.shapes[2].z, 0.0),
                        scene.shapes[2].x, scene.shapes[2].y * geo_mod);
    let d3 = eval_shape(fp + vec3<f32>(0.0, scene.shapes[3].z, 0.0),
                        scene.shapes[3].x, scene.shapes[3].y * geo_mod);

    // ── Combine shapes using genome combinators ──
    var d = combine(d0, d1, scene.combinators[0].x,
                    scene.combinators[0].y + geo_audio * 0.1);
    d = combine(d, d2, scene.combinators[0].z,
                scene.combinators[0].w + geo_audio * 0.1);
    d = combine(d, d3, scene.combinators[1].x,
                scene.combinators[1].y + geo_audio * 0.1);

    // Fade geometry to void when silent
    let presence = clamp(audio_level * 15.0, 0.0, 1.0);
    return d + (1.0 - presence) * 15.0;
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

// ─── Fragment Shader ────────────────────────────────────────────────────────

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let aspect = u.resolution.x / u.resolution.y;
    let uv = (pos.xy / u.resolution - 0.5) * vec2<f32>(aspect, 1.0);

    let t = u.time;
    let bass = u.bass;
    let mids = u.mids;
    let highs = u.highs;
    let energy = u.energy;
    let beat = u.beat;

    // ── Camera setup: genome-driven orbiting camera ──
    let audio_peak = max(max(bass, mids), max(highs, energy));
    let motion = clamp(audio_peak * 8.0, 0.05, 1.0);
    let cam_audio = audio_for_target(1.0);
    let seed = u.seed;
    let cam_dist = scene.camera.y - cam_audio * 0.3;
    let cam_angle_y = t * scene.camera.z * motion + cam_audio * 0.15;
    let cam_angle_x = sin(t * 0.15) * scene.camera.w * motion;
    var ro = vec3<f32>(
        cam_dist * sin(cam_angle_y) * cos(cam_angle_x),
        cam_dist * sin(cam_angle_x) + sin(t * 0.3) * 0.3 * motion,
        cam_dist * cos(cam_angle_y) * cos(cam_angle_x),
    );
    // Seed offset: pressing R jumps to a completely different visual region
    ro += vec3<f32>(seed * 100.0, seed * 73.0, seed * 37.0);
    let look_at = vec3<f32>(0.0, 0.0, 0.0);
    let forward = normalize(look_at - ro);
    let right = normalize(cross(forward, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, forward);
    let focal = 1.5 - energy * 0.1;
    let rd = normalize(forward * focal + right * uv.x + up * uv.y);

    // ── Single raymarch (was 3 per pixel for chromatic aberration) ──
    let result = raymarch(ro, rd);

    // ── Lighting setup ──
    let light_pos = vec3<f32>(
        sin(t * 0.7) * 2.0,
        2.0 + cos(t * 0.5),
        cos(t * 0.7) * 2.0,
    );

    // ── Coloring: genome-routed audio drives color modulation ──
    let color_audio = audio_for_target(2.0);
    var color = vec3<f32>(0.0);

    if (result.dist < SURF_DIST * 2.0) {
        let hit_p = ro + rd * result.total_dist;
        let n = calc_normal(hit_p);
        let base_color = palette_shifted(
            result.total_dist * 0.15 + t * 0.08 + dot(n, vec3<f32>(0.3, 0.6, 0.1)),
            color_audio * 0.6 + beat * 0.4
        );
        let light_dir = normalize(light_pos - hit_p);
        let diff = max(dot(n, light_dir), 0.0);
        let spec = pow(max(dot(reflect(-light_dir, n), -rd), 0.0), 16.0 + color_audio * 32.0);
        let luminance = dot(base_color, vec3<f32>(0.299, 0.587, 0.114));
        let saturated = mix(vec3<f32>(luminance), base_color, 1.2 + color_audio * 0.8);
        let brightness = 1.0 + color_audio * 0.5;
        let lit = saturated * (0.25 + diff * 0.75) + spec * 0.5 * (1.0 + color_audio * 2.0);
        let fog = exp(-result.total_dist * 0.05);
        color = lit * brightness * fog;
    }

    // ── Glow from close misses ──
    let glow_intensity = 0.03 / (result.closest + 0.01);
    let glow_color = palette(t * 0.1 + result.total_dist * 0.05 + color_audio);
    var glow = glow_color * glow_intensity * (0.4 + color_audio * 1.5);

    // Step-based ambient glow (more steps = ray was grazing surfaces)
    let step_glow = f32(result.steps) / f32(MAX_STEPS);
    let step_color = palette_shifted(step_glow + t * 0.02, color_audio * 0.8);
    glow = glow + step_color * step_glow * step_glow * 0.6;

    color = color + glow;

    // ── Beat flash: sudden bright pulse ──
    let flash = beat * beat * 0.2;
    color = color + vec3<f32>(flash);

    // ── Palette shift on beat: nudge hues ──
    let beat_shift = beat * 0.25;
    color = mix(color, color.gbr, beat_shift);

    // ── Feedback: blend with previous frame for melting trail effect ──
    // Chromatic aberration applied as screen-space UV offset on feedback sampling
    let screen_uv = pos.xy / u.resolution;
    let drift = vec2<f32>(sin(u.time * 0.1) * 0.003, cos(u.time * 0.13) * 0.003);
    let ca_offset = beat * 0.015;
    let prev_r = textureSample(prev_frame, prev_sampler, screen_uv + drift + vec2<f32>(ca_offset, 0.0)).r;
    let prev_g = textureSample(prev_frame, prev_sampler, screen_uv + drift).g;
    let prev_b = textureSample(prev_frame, prev_sampler, screen_uv + drift - vec2<f32>(ca_offset, 0.0)).b;
    let prev_srgb = vec3<f32>(prev_r, prev_g, prev_b);
    let prev_linear = pow(prev_srgb, vec3<f32>(2.2));
    // Fade the previous frame down so trails decay instead of accumulating
    let trail_decay = 0.85;
    let blend_factor = 0.45 + beat * 0.25;
    color = mix(prev_linear * trail_decay, color, blend_factor);

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
