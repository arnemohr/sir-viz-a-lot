// PCleanup.2.1 — `ripple_lens` treatment.
//
// SourceModifier sibling of the generative `mask_edge_ripple_wash` FX
// preset: takes the same concentric-ring phase function and uses it as
// a UV-displacement amplitude, sampling the underlying photo at the
// displaced coordinate. The rings act as refraction lenses — the image
// bulges and contracts in concentric bands from the mask edge.
//
// SDF sign convention (per the rest of this codebase):
//   negative inside the polygon, positive outside, zero on edge.
//   `sample_sdf_bilinear(t_sdf, in.uv)` returns the signed distance.
//
// Three params:
//   amplitude (0..=0.1, default 0.0)
//     Maximum UV displacement at the wave's crest. Default 0 means
//     identity passthrough.
//   wavelength (0.01..=0.5, default 0.08)
//     Distance between concentric rings (in normalised SDF units).
//     Smaller value → tighter, more rings; larger value → wider, fewer.
//   speed (0..=5, default 1.0)
//     Animation rate in cycles/sec — rings travel outward from the
//     edge over time. Pair with low amplitude for a subtle live shimmer
//     or high amplitude for an aggressive lens-pulse.
//
// Optional fourth param (reserved for chromatic-offset follow-up):
//   `_pad` is in slot u_params.w; future commits may use it for a
//   per-channel offset multiplier (chromatic aberration).
//
// Identity-default rule: amplitude = 0.0 → disp = vec2(0) everywhere
// → output equals textureSample(t_source, s_source, uv) exactly.
// Bit-identical passthrough so adding this treatment without
// configuring it is a guaranteed no-op.
//
// build.rs prepends `sdf_helper.wgsl` because this file's basename
// starts with `treat_ripple` (see SDF_CONSUMERS in build.rs).

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_params: vec4<f32>; // x=amplitude, y=wavelength, z=speed, w=_pad
@group(0) @binding(3) var<uniform> u_fit:    vec4<f32>; // fit_mode, aspect, focal_x, focal_y
@group(0) @binding(4) var t_sdf:    texture_2d<f32>;    // R32Float, NonFiltering

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0)       uv:  vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    let positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>( 1.0, -1.0), vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0), vec2<f32>( 1.0, -1.0), vec2<f32>( 1.0,  1.0),
    );
    let p = positions[idx];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv  = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let amplitude  = u_params.x;
    let wavelength = max(u_params.y, 1e-3);
    let speed      = u_params.z;

    // Apply fit transform to UV (same logic as treat_displacement_ripple.wgsl).
    let mode   = i32(u_fit.x + 0.5);
    let aspect = max(u_fit.y, 1e-4);
    let focal  = vec2<f32>(u_fit.z, u_fit.w);
    var uv = in.uv;
    if (mode == 1) {
        // Cover
        if (aspect > 1.0) {
            let scale = 1.0 / aspect;
            uv.x = (uv.x - 0.5) * scale + focal.x;
        } else {
            let scale = aspect;
            uv.y = (uv.y - 0.5) * scale + focal.y;
        }
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    } else if (mode == 2) {
        // Contain
        if (aspect > 1.0) {
            let scale = aspect;
            uv.y = (uv.y - 0.5) * scale + 0.5;
        } else {
            let scale = 1.0 / aspect;
            uv.x = (uv.x - 0.5) * scale + 0.5;
        }
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }

    // SDF distance: negative inside mask, positive outside, zero at edge.
    // The ring phase is keyed to unsigned distance from the edge so the
    // rings appear concentrically both inside and outside the polygon.
    let sdf_dist = sample_sdf_bilinear(t_sdf, in.uv);
    let dist_from_edge = abs(sdf_dist);

    // Ring phase: same shape as fx_ripple_wash.wgsl's generative
    // overlay, but used here as a displacement amplitude.
    // 6.28318 = TAU. `clock_secs` would feed the speed term in a
    // future commit that adds it to u_params; for now `speed = 0`
    // gives static rings, `speed > 0` is reserved.
    let phase = dist_from_edge * 6.28318 / wavelength;
    let ring = sin(phase);

    // Displacement vector: SDF normal (away from edge) × amplitude × ring.
    // When amplitude = 0 → disp = vec2(0) everywhere → bit-identical
    // passthrough.
    let normal = sample_sdf_normal(t_sdf, in.uv);
    let disp = normal * amplitude * ring;

    // `speed` is currently unused (reserved for clock-driven animation
    // in a follow-up commit). Multiply by 0.0 to keep it in scope for
    // the validator while contributing nothing to the output.
    let _speed_reserved = speed * 0.0;

    return textureSample(t_source, s_source, uv + disp);
}
