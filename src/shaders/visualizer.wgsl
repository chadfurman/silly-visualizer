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

// ─── Color Palette ──────────────────────────────────────────────────────────

fn palette(t: f32) -> vec3<f32> {
    let a = vec3<f32>(0.5, 0.5, 0.5);
    let b = vec3<f32>(0.5, 0.5, 0.5);
    let c = vec3<f32>(1.0, 1.0, 1.0);
    let d = vec3<f32>(0.263, 0.416, 0.557);
    return a + b * cos(TAU * (c * t + d));
}

fn palette_shifted(t: f32, shift: f32) -> vec3<f32> {
    let a = vec3<f32>(0.5, 0.5, 0.5);
    let b = vec3<f32>(0.5, 0.5, 0.5);
    let c = vec3<f32>(1.0, 1.0, 0.5);
    let d = vec3<f32>(0.263 + shift, 0.416 + shift * 0.7, 0.557 + shift * 0.3);
    return a + b * cos(TAU * (c * t + d));
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

// ─── Scene SDF ──────────────────────────────────────────────────────────────

fn map(p_in: vec3<f32>) -> f32 {
    let t = u.time;
    let bass = u.bass;
    let mids = u.mids;
    let highs = u.highs;
    let energy = u.energy;

    // Rotation driven by time and mids
    let rot_speed = 0.3 + mids * 1.5;
    var p = rot_y(t * rot_speed * 0.4) * rot_x(t * rot_speed * 0.25) * p_in;

    // ── Space repetition: infinite tunnel / kaleidoscopic ──
    // Domain repetition along Z for tunnel effect
    let rep_z = 4.0 - bass * 1.0;
    var rp = p;
    rp.z = wmod(p.z + t * 0.8, rep_z) - rep_z * 0.5;

    // Kaleidoscopic angular folding in XY
    let fold_angle = PI / (3.0 + mids * 3.0);
    let angle = atan2(rp.y, rp.x);
    let folded_angle = wmod(angle, fold_angle * 2.0) - fold_angle;
    let r_xy = length(rp.xy);
    rp = vec3<f32>(r_xy * cos(folded_angle), r_xy * sin(folded_angle), rp.z);

    // ── Iterative space folding (Mandelbox-like) ──
    let fold_iters = i32(clamp(1.0 + energy * 4.0, 1.0, 5.0));
    let fold_scale = 1.5 + energy * 0.8;
    let fp = fold_space(rp * 0.5, fold_iters, fold_scale) * 2.0;

    // ── Primary shape: morph between torus and octahedron ──
    let morph = 0.5 + 0.5 * sin(t * 0.6 + bass * PI);
    let geo_scale = 1.0 + bass * 0.8;

    let torus_r = vec2<f32>(1.2 * geo_scale, 0.3 + bass * 0.2);
    let d_torus = sd_torus(rp, torus_r);
    let d_octa = sd_octahedron(rp, 1.0 * geo_scale);
    let d_primary = mix(d_torus, d_octa, morph);

    // ── Secondary shapes from folded space ──
    let d_folded_sphere = sd_sphere(fp, 0.8 + highs * 0.5);
    let d_folded_box = sd_box(fp, vec3<f32>(0.5 + mids * 0.3));
    let d_secondary = smooth_union(d_folded_sphere, d_folded_box, 0.5);

    // ── Combine primary and secondary ──
    var d = smooth_union(d_primary, d_secondary * 0.6, 0.8 + bass * 0.4);

    // ── Carved detail: subtract rotating octahedra ──
    let carve_p = rot_z(t * 1.2 + mids * 2.0) * rp;
    let d_carve = sd_octahedron(carve_p, 0.6 + highs * 0.5);
    d = smooth_subtraction(d_carve, d, 0.3 + energy * 0.2);

    // ── Add pulsing spheres at tunnel repetitions ──
    let pulse = 0.3 + 0.2 * sin(t * 4.0 + bass * TAU);
    let sp = p;
    let srp_z = wmod(sp.z + t * 0.8, rep_z) - rep_z * 0.5;
    let d_pulse = sd_sphere(vec3<f32>(sp.x, sp.y, srp_z), pulse);
    d = smooth_union(d, d_pulse, 0.6);

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

    var t = 0.0;
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

    // ── Camera setup: orbiting camera ──
    let cam_dist = 5.0 - bass * 1.5;
    let cam_angle_y = t * 0.2 + mids * 0.5;
    let cam_angle_x = sin(t * 0.15) * 0.4;
    let ro = vec3<f32>(
        cam_dist * sin(cam_angle_y) * cos(cam_angle_x),
        cam_dist * sin(cam_angle_x) + sin(t * 0.3) * 0.5,
        cam_dist * cos(cam_angle_y) * cos(cam_angle_x),
    );
    let look_at = vec3<f32>(0.0, 0.0, 0.0);
    let forward = normalize(look_at - ro);
    let right = normalize(cross(forward, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, forward);
    let focal = 1.5 - energy * 0.3;
    let rd = normalize(forward * focal + right * uv.x + up * uv.y);

    // ── Chromatic aberration on beat ──
    let ca_strength = beat * 0.015;
    let rd_r = normalize(forward * focal + right * (uv.x + ca_strength) + up * uv.y);
    let rd_b = normalize(forward * focal + right * (uv.x - ca_strength) + up * uv.y);

    // ── Raymarch for each color channel ──
    let result_g = raymarch(ro, rd);
    let result_r = raymarch(ro, rd_r);
    let result_b = raymarch(ro, rd_b);

    // ── Coloring ──
    var color = vec3<f32>(0.0);
    var glow = vec3<f32>(0.0);

    // -- Green/main channel --
    if (result_g.dist < SURF_DIST * 2.0) {
        let hit_p = ro + rd * result_g.total_dist;
        let n = calc_normal(hit_p);

        // Cosine palette driven by distance and bass hue shift
        let base_color = palette_shifted(
            result_g.total_dist * 0.1 + t * 0.05 + dot(n, vec3<f32>(0.3, 0.6, 0.1)),
            bass * 0.5 + beat * 0.3
        );

        // Lighting: ambient + diffuse from orbiting light
        let light_dir = normalize(vec3<f32>(
            sin(t * 0.7) * 2.0,
            2.0 + cos(t * 0.5),
            cos(t * 0.7) * 2.0,
        ) - hit_p);
        let diff = max(dot(n, light_dir), 0.0);
        let spec = pow(max(dot(reflect(-light_dir, n), -rd), 0.0), 16.0 + highs * 32.0);

        // Saturation controlled by mids
        let luminance = dot(base_color, vec3<f32>(0.299, 0.587, 0.114));
        let saturated = mix(vec3<f32>(luminance), base_color, 0.7 + mids * 1.0);

        // Brightness controlled by highs
        let brightness = 0.8 + highs * 0.6;

        let lit = saturated * (0.15 + diff * 0.7) + spec * 0.4 * (1.0 + highs * 2.0);
        color.g = (lit.g * brightness);

        // Fog based on distance
        let fog = exp(-result_g.total_dist * 0.06);
        color.g = color.g * fog;
    }

    // -- Red channel (with chromatic offset) --
    if (result_r.dist < SURF_DIST * 2.0) {
        let hit_p = ro + rd_r * result_r.total_dist;
        let n = calc_normal(hit_p);
        let base_color = palette_shifted(
            result_r.total_dist * 0.1 + t * 0.05 + dot(n, vec3<f32>(0.3, 0.6, 0.1)),
            bass * 0.5 + beat * 0.3
        );
        let light_dir = normalize(vec3<f32>(
            sin(t * 0.7) * 2.0, 2.0 + cos(t * 0.5), cos(t * 0.7) * 2.0
        ) - hit_p);
        let diff = max(dot(n, light_dir), 0.0);
        let spec = pow(max(dot(reflect(-light_dir, n), -rd_r), 0.0), 16.0 + highs * 32.0);
        let luminance = dot(base_color, vec3<f32>(0.299, 0.587, 0.114));
        let saturated = mix(vec3<f32>(luminance), base_color, 0.7 + mids * 1.0);
        let brightness = 0.8 + highs * 0.6;
        let lit = saturated * (0.15 + diff * 0.7) + spec * 0.4 * (1.0 + highs * 2.0);
        color.r = lit.r * brightness * exp(-result_r.total_dist * 0.06);
    }

    // -- Blue channel (with chromatic offset) --
    if (result_b.dist < SURF_DIST * 2.0) {
        let hit_p = ro + rd_b * result_b.total_dist;
        let n = calc_normal(hit_p);
        let base_color = palette_shifted(
            result_b.total_dist * 0.1 + t * 0.05 + dot(n, vec3<f32>(0.3, 0.6, 0.1)),
            bass * 0.5 + beat * 0.3
        );
        let light_dir = normalize(vec3<f32>(
            sin(t * 0.7) * 2.0, 2.0 + cos(t * 0.5), cos(t * 0.7) * 2.0
        ) - hit_p);
        let diff = max(dot(n, light_dir), 0.0);
        let spec = pow(max(dot(reflect(-light_dir, n), -rd_b), 0.0), 16.0 + highs * 32.0);
        let luminance = dot(base_color, vec3<f32>(0.299, 0.587, 0.114));
        let saturated = mix(vec3<f32>(luminance), base_color, 0.7 + mids * 1.0);
        let brightness = 0.8 + highs * 0.6;
        let lit = saturated * (0.15 + diff * 0.7) + spec * 0.4 * (1.0 + highs * 2.0);
        color.b = lit.b * brightness * exp(-result_b.total_dist * 0.06);
    }

    // ── Glow from close misses ──
    let glow_intensity = 0.02 / (result_g.closest + 0.01);
    let glow_color = palette(t * 0.1 + result_g.total_dist * 0.05 + bass);
    glow = glow_color * glow_intensity * (0.3 + highs * 1.5);

    // Step-based ambient glow (more steps = ray was grazing surfaces)
    let step_glow = f32(result_g.steps) / f32(MAX_STEPS);
    let step_color = palette_shifted(step_glow + t * 0.02, bass * 0.8);
    glow = glow + step_color * step_glow * step_glow * 0.5;

    color = color + glow;

    // ── Beat flash: sudden bright pulse ──
    let flash = beat * beat * 0.15;
    color = color + vec3<f32>(flash);

    // ── Palette shift on beat: nudge hues ──
    let beat_shift = beat * 0.2;
    color = mix(color, color.gbr, beat_shift);

    // ── Vignette ──
    let vignette_uv = pos.xy / u.resolution - 0.5;
    let vignette = 1.0 - dot(vignette_uv, vignette_uv) * 0.8;
    color = color * vignette;

    // ── Tone mapping (simple Reinhard) ──
    color = color / (color + vec3<f32>(1.0));

    // ── Gamma correction ──
    color = pow(color, vec3<f32>(0.4545));

    return vec4<f32>(color, 1.0);
}
