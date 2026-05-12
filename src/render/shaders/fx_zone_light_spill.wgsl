// P3.5.1 — Light spill from window zones.
//
// build.rs prepends sdf_helper.wgsl + zone_tag_helper.wgsl for files
// starting with "fx_zone_", so sample_sdf_bilinear, the ZONE_* constants,
// and ZoneTagUniform are available without further imports.
//
// Behaviour:
//   zone_tag == ZONE_WINDOW → warm-glow spill gradient inward from the
//     mask edge. Intensity peaks at the edge (sdf_distance ≈ 0) and falls
//     off exponentially as distance from the edge grows.
//   zone_tag != ZONE_WINDOW (including ZONE_NONE) → transparent black
//     (no-op fallback). The operator sees no effect; no crash.
//
// Parameters (via FxParams / u_params):
//   wavelength  → spill_radius: normalised distance the spill reaches (0..1)
//   speed       → unused (kept at 0.0)
//   falloff     → falloff sharpness (higher = narrower glow band)
//   base_r/g/b  → spill colour (default warm: 1.0, 0.85, 0.55)

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct FxParams {
    spill_radius: f32,   // alias: wavelength
    _unused:      f32,   // alias: speed
    falloff:      f32,
    base_r:       f32,
    base_g:       f32,
    base_b:       f32,
    _pad0:        f32,
    _pad1:        f32,
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
    // Non-window zone or untagged: transparent black (no-op fallback).
    if u_zone.zone_tag != ZONE_WINDOW {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // SDF: negative inside the polygon, positive outside, ~0 at the edge.
    let d = sample_sdf_bilinear(t_sdf, in.uv);

    // Only render inside the polygon (d <= 0) and within spill_radius.
    let radius = max(u_params.spill_radius, 0.01);
    // `dist_in` is how far inside the polygon we are (positive inside).
    let dist_in = -d;
    if dist_in <= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Warm glow: intensity peaks at the edge (dist_in ≈ 0) and falls off.
    let falloff = max(u_params.falloff, 1e-3);
    let intensity = exp(-dist_in / (radius * falloff));

    // Alpha: smooth from 1 at the edge to 0 at spill_radius.
    let alpha = smoothstep(radius, 0.0, dist_in) * intensity;

    let colour = vec3<f32>(u_params.base_r, u_params.base_g, u_params.base_b) * intensity;

    // Premultiplied alpha output.
    return vec4<f32>(colour * alpha, alpha);
}
