// P3.5.2 — Ripple at edge zones.
//
// build.rs prepends sdf_helper.wgsl + zone_tag_helper.wgsl for files
// starting with "fx_zone_".
//
// Behaviour:
//   zone_tag == ZONE_EDGE → tighter, higher-frequency ripple originating
//     from the mask edge. Amplifies the ripple_wash behaviour but with
//     ZONE_EDGE semantics: the effect is tighter to the boundary.
//   zone_tag != ZONE_EDGE (including ZONE_NONE) → transparent black.
//
// Parameters (via FxParams / u_params):
//   wavelength → wave frequency (higher = tighter ripple, default 20.0)
//   speed      → animation speed (default 3.0)
//   falloff    → exp-falloff from edge (default 0.04, tighter than ripple_wash)
//   base_r/g/b → ripple colour (default cool: 0.3, 0.7, 1.0)

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct FxParams {
    wavelength: f32,
    speed:      f32,
    falloff:    f32,
    base_r:     f32,
    base_g:     f32,
    base_b:     f32,
    _pad0:      f32,
    _pad1:      f32,
};

@group(0) @binding(0) var t_sdf: texture_2d<f32>;
@group(0) @binding(1) var s_sdf: sampler;
@group(0) @binding(2) var<uniform> u_params: FxParams;
@group(0) @binding(3) var<uniform> u_clock: vec4<f32>;
// P3.3.2 — slot 6: zone tag uniform (zone-aware presets only).
@group(0) @binding(6) var<uniform> u_zone: ZoneTagUniform;

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vi & 2u) * 2.0 - 1.0;
    var o: VsOut;
    o.pos = vec4<f32>(x, y, 0.0, 1.0);
    o.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Non-edge zone or untagged: transparent black (no-op fallback).
    if u_zone.zone_tag != ZONE_EDGE {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let d = sample_sdf_bilinear(t_sdf, in.uv);
    let t = u_clock.x;

    // Absolute distance from the edge — ripple radiates symmetrically.
    let dist = abs(d);

    // Higher spatial frequency (tighter ripple) than ripple_wash.
    let phase = dist * 6.28318530718 / max(u_params.wavelength, 0.5)
              - t * u_params.speed;
    let ripple = sin(phase);

    // Tighter exponential falloff (concentrates effect near the edge).
    let attenuation = exp(-dist / max(u_params.falloff, 1e-4));

    let intensity = 0.5 + 0.5 * ripple;
    let colour = vec3<f32>(u_params.base_r, u_params.base_g, u_params.base_b)
              * intensity * attenuation;

    // Alpha: fade to zero outside the polygon.
    let inside = 1.0 - smoothstep(0.0, max(u_params.wavelength, 0.5), d);
    return vec4<f32>(colour, inside);
}
