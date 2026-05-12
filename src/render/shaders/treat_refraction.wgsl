// P2.4.2 — `refraction` treatment.
//
// Bends pixel-rays at the mask boundary using a Snell-like UV offset along
// the SDF normal. Reads as light refracting through glass at the mask edge.
// Unlike displacement_ripple (P2.4.1), there is no sinusoidal oscillation —
// just a smooth steady bend whose magnitude is controlled by `ior`.
//
// SDF sign convention (per the rest of this codebase):
//   negative inside the polygon, positive outside, zero on edge.
//   `abs(sdf_dist)` gives unsigned distance from the edge on either side.
//
// Refraction only fires inside the mask (sdf_dist <= 0). Pixels outside
// are passed through unchanged. `edge_width` controls the band around the
// mask edge where refraction is active; small edge_width → thin fringe,
// large → wide affected band.
//
// Identity-default rule: ior = 1.0 → bend = vec2(0) everywhere
// → output equals textureSample(t_source, s, uv) exactly.
//
// build.rs prepends `sdf_helper.wgsl` because this file's basename
// starts with `treat_refraction` (see SDF_CONSUMERS in build.rs).
//
// TODO(P2.4.2-golden): add a tests/golden baseline once the headless
// treatment golden harness lands.

@group(0) @binding(0) var t_source:  texture_2d<f32>;
@group(0) @binding(1) var s_source:  sampler;
@group(0) @binding(2) var<uniform>   u_params: vec4<f32>; // x=ior, y=edge_width, z=_pad, w=_pad
@group(0) @binding(3) var<uniform>   u_fit:    vec4<f32>; // fit_mode, aspect, focal_x, focal_y
@group(0) @binding(4) var t_sdf:     texture_2d<f32>;    // R32Float, NonFiltering

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
    let ior        = u_params.x;
    let edge_width = u_params.y;

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
    let sdf_dist = sample_sdf_bilinear(t_sdf, in.uv);

    // Pixels outside the mask pass through unchanged.
    if (sdf_dist > 0.0) {
        return textureSample(t_source, s_source, uv);
    }

    // Unsigned distance from the mask edge.
    let dist_from_edge = abs(sdf_dist);

    // SDF normal (points away from mask edge, toward interior).
    let normal = sample_sdf_normal(t_sdf, in.uv);

    // Refraction bend: magnitude proportional to (ior - 1) × smoothstep envelope.
    // When ior = 1.0, bend = vec2(0) → refracted_uv = uv (identity).
    // Guard edge_width against zero with 1e-4 to avoid smoothstep(0,0,x) UB.
    let bend = normal * (ior - 1.0) * smoothstep(0.0, max(edge_width, 1e-4), dist_from_edge);
    let refracted_uv = clamp(uv + bend, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(t_source, s_source, refracted_uv);
}
