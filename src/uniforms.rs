/// WGSL uniform buffer alignment: arrays must have element stride that is a
/// multiple of 16 bytes. We use `array<vec4<f32>, 4>` in WGSL which maps to
/// `[f32; 16]` in Rust (same contiguous layout). The `debug_flags` and `_pad1`
/// fields align `bands` to a 16-byte boundary (offset 48).
///
/// Any changes to this struct MUST be mirrored in `shaders/visualizer.wgsl`.
const _: () = assert!(
    std::mem::size_of::<AudioUniforms>() == 112,
    "AudioUniforms size must be 112 bytes to match WGSL layout"
);
const _: () = assert!(
    std::mem::size_of::<AudioUniforms>().is_multiple_of(16),
    "AudioUniforms size must be a multiple of 16 for uniform buffer alignment"
);

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AudioUniforms {
    pub time: f32,
    pub bass: f32,
    pub mids: f32,
    pub highs: f32,
    pub energy: f32,
    pub beat: f32,
    pub seed: f32,
    pub palette_id: f32,
    pub resolution: [f32; 2],
    pub debug_flags: f32,
    pub _pad1: f32,
    pub bands: [f32; 16],
}

impl Default for AudioUniforms {
    fn default() -> Self {
        Self {
            time: 0.0,
            bass: 0.0,
            mids: 0.0,
            highs: 0.0,
            energy: 0.0,
            beat: 0.0,
            seed: 0.0,
            palette_id: 0.0,
            resolution: [0.0, 0.0],
            debug_flags: 0.0,
            _pad1: 0.0,
            bands: [0.0; 16],
        }
    }
}

/// GPU representation of genome parameters.
/// Layout matches the SceneUniforms WGSL struct.
/// Each shape slot is packed as vec4(type, scale, offset, rot_speed).
/// Combinators packed as vec4(type0, smooth0, type1, smooth1) + vec4(type2, smooth2, pad, pad).
/// Remaining params packed into vec4s for alignment.
const _: () = assert!(
    std::mem::size_of::<SceneUniforms>() == 160,
    "SceneUniforms size must be 160 bytes to match WGSL layout"
);
const _: () = assert!(
    std::mem::size_of::<SceneUniforms>().is_multiple_of(16),
    "SceneUniforms size must be 16-byte aligned"
);

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneUniforms {
    /// Shape slots packed as vec4(type, scale, offset, rot_speed) per shape
    pub shapes: [[f32; 4]; 4],           // 64 bytes (offset 0)
    /// Combinators: [type0, smooth0, type1, smooth1], [type2, smooth2, pad, pad]
    pub combinators: [[f32; 4]; 2],      // 32 bytes (offset 64)
    /// Folding: [iterations, scale, offset, rep_z]
    pub folding: [f32; 4],               // 16 bytes (offset 96)
    /// Camera + kal: [kaleidoscope_folds, cam_distance, orbit_speed, wobble_amount]
    pub camera: [f32; 4],                // 16 bytes (offset 112)
    /// Audio routing: [bass_target, mids_target, highs_target, energy_target]
    pub audio_routing: [f32; 4],         // 16 bytes (offset 128)
    /// Transition: [beat_target, transition_type, transition_boost, pad]
    pub transition: [f32; 4],            // 16 bytes (offset 144)
}

impl Default for SceneUniforms {
    fn default() -> Self {
        Self {
            shapes: [[0.0; 4]; 4],
            combinators: [[0.0; 4]; 2],
            folding: [1.0, 1.5, 0.0, 4.0],
            camera: [4.0, 5.0, 0.2, 0.0],
            audio_routing: [0.0; 4],
            transition: [0.0; 4],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_uniforms_size_is_112_bytes() {
        assert_eq!(std::mem::size_of::<AudioUniforms>(), 112);
    }

    #[test]
    fn audio_uniforms_size_is_16_byte_aligned() {
        assert_eq!(std::mem::size_of::<AudioUniforms>() % 16, 0);
    }

    #[test]
    fn audio_uniforms_field_offsets_match_wgsl() {
        use std::mem::offset_of;
        assert_eq!(offset_of!(AudioUniforms, time), 0);
        assert_eq!(offset_of!(AudioUniforms, bass), 4);
        assert_eq!(offset_of!(AudioUniforms, mids), 8);
        assert_eq!(offset_of!(AudioUniforms, highs), 12);
        assert_eq!(offset_of!(AudioUniforms, energy), 16);
        assert_eq!(offset_of!(AudioUniforms, beat), 20);
        assert_eq!(offset_of!(AudioUniforms, seed), 24);
        assert_eq!(offset_of!(AudioUniforms, palette_id), 28);
        assert_eq!(offset_of!(AudioUniforms, resolution), 32);
        assert_eq!(offset_of!(AudioUniforms, debug_flags), 40);
        assert_eq!(offset_of!(AudioUniforms, _pad1), 44);
        assert_eq!(offset_of!(AudioUniforms, bands), 48);
        assert_eq!(offset_of!(AudioUniforms, bands) % 16, 0);
    }

    #[test]
    fn audio_uniforms_is_pod_castable() {
        let u = AudioUniforms::default();
        let bytes: &[u8] = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), 112);
        let u2: &AudioUniforms = bytemuck::from_bytes(bytes);
        assert_eq!(u2.time, 0.0);
        assert_eq!(u2.bands, [0.0; 16]);
    }

    #[test]
    fn scene_uniforms_size_is_160_bytes() {
        assert_eq!(std::mem::size_of::<SceneUniforms>(), 160);
    }

    #[test]
    fn scene_uniforms_size_is_16_byte_aligned() {
        assert_eq!(std::mem::size_of::<SceneUniforms>() % 16, 0);
    }

    #[test]
    fn scene_uniforms_is_pod_castable() {
        let s = SceneUniforms::default();
        let bytes: &[u8] = bytemuck::bytes_of(&s);
        assert_eq!(bytes.len(), 160);
        let s2: &SceneUniforms = bytemuck::from_bytes(bytes);
        assert_eq!(s2.folding, [1.0, 1.5, 0.0, 4.0]);
    }

    #[test]
    fn scene_uniforms_field_offsets_match_wgsl() {
        use std::mem::offset_of;
        assert_eq!(offset_of!(SceneUniforms, shapes), 0);
        assert_eq!(offset_of!(SceneUniforms, combinators), 64);
        assert_eq!(offset_of!(SceneUniforms, folding), 96);
        assert_eq!(offset_of!(SceneUniforms, camera), 112);
        assert_eq!(offset_of!(SceneUniforms, audio_routing), 128);
        assert_eq!(offset_of!(SceneUniforms, transition), 144);
    }

    #[test]
    fn audio_uniforms_default_is_zeroed() {
        let u = AudioUniforms::default();
        assert_eq!(u.time, 0.0);
        assert_eq!(u.bass, 0.0);
        assert_eq!(u.mids, 0.0);
        assert_eq!(u.highs, 0.0);
        assert_eq!(u.energy, 0.0);
        assert_eq!(u.beat, 0.0);
        assert_eq!(u.seed, 0.0);
        assert_eq!(u.resolution, [0.0, 0.0]);
        assert_eq!(u.debug_flags, 0.0);
        assert_eq!(u.bands, [0.0; 16]);
    }

    #[test]
    fn audio_uniforms_debug_flags_offset() {
        use std::mem::offset_of;
        assert_eq!(offset_of!(AudioUniforms, debug_flags), 40);
    }
}
