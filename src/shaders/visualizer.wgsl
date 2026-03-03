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
