// P2.4.1 — `displacement_ripple` treatment.
//
// Distorts the underlying Image / Video texture along the mask boundary
// by displacing the sample UV in the direction of the SDF normal. The
// displacement is modulated by a sinusoid keyed to SDF distance and
// decays to zero away from the edge, giving a "glass lens at the window
// edge" look.
//
// SDF sign convention (per the rest of this codebase):
//   negative inside the polygon, positive outside, zero on edge.
//   `abs(sdf_dist)` gives unsigned distance from edge on either side.
//
// Displacement only fires inside the mask (sdf_dist <= 0). Pixels
// outside are passed through unchanged. The decay param controls how
// quickly the effect falls off as you move away from the edge; small
// decay → thin fringe, large decay → wide affected band.
//
// Identity-default rule: amplitude = 0.0 → disp = vec2(0) everywhere
// → output equals textureSample(t_source, s_source, uv) exactly.
//
// build.rs prepends `sdf_helper.wgsl` because this file's basename
// starts with `treat_displacement` (see SDF_CONSUMERS in build.rs).
//
// TODO(P2.4.1-golden): add a tests/golden baseline once the headless
// treatment golden harness lands.

@group(0) @binding(0) var t_source:  texture_2d<f32>;
@group(0) @binding(1) var s_source:  sampler;
@group(0) @binding(2) var<uniform>   u_params: vec4<f32>; // x=amplitude, y=frequency, z=decay, w=_pad
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
    let amplitude = u_params.x;
    let frequency = u_params.y;
    let decay     = max(u_params.z, 0.001);

    // Apply fit transform to UV (same logic as treat_tone_map.wgsl).
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

    // Unsigned distance from the mask edge (used for smoothstep envelope).
    let dist_from_edge = abs(sdf_dist);

    // Decay envelope: 1.0 at edge → 0.0 at dist_from_edge = decay.
    let envelope = smoothstep(0.0, decay, dist_from_edge);

    // Displacement vector: SDF normal (points away from edge) modulated
    // by a ripple sinusoid. When amplitude = 0, disp = vec2(0) exactly.
    let normal = sample_sdf_normal(t_sdf, in.uv);
    let wave   = sin(dist_from_edge * frequency * 6.28318); // 6.28318 = TAU
    let disp   = normal * amplitude * wave * envelope;

    return textureSample(t_source, s_source, uv + disp);
}
