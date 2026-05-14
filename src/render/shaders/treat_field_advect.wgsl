// PCleanup.2.7 — `field_advect_source` treatment.
//
// SourceModifier sibling of the generative `mask_field_flow` FX preset:
// instead of visualising particles driven by the SDF gradient field,
// this treatment uses the same gradient to advect the underlying photo
// directly. At each UV, the source is sampled at `uv - gradient(uv) *
// flow_speed * clock_secs`, making the image appear to drift along the
// mask's normal field over time.
//
// The gradient is taken from `sample_sdf_gradient` (finite-difference,
// not normalised) so the drift magnitude falls off naturally away from
// mask edges, matching the `mask_field_flow` compute shader's approach.
//
// SDF sign convention (per the rest of this codebase):
//   negative inside the polygon, positive outside, zero on edge.
//   Gradient points outward from the nearest edge (toward increasing SDF).
//
// Two params (packed into u_params):
//   x = flow_speed   (0.0..=2.0, default 0.0)
//     Drift rate in UV/s. Default 0.0 means identity passthrough.
//     Higher values produce faster apparent motion along mask normals.
//   w = clock_secs   (written by the dispatcher each frame, not an
//                     operator-facing param — no slider for it)
//     Accumulated time in seconds, multiplied with flow_speed to give
//     the total UV offset. Written into the params uniform's `w` slot
//     the same way `edge_lens` does.
//
// Identity-default rule: flow_speed = 0.0 → offset = vec2(0) everywhere
// → output equals textureSample(t_source, s_source, uv) exactly.
// Bit-identical passthrough so adding this treatment without configuring
// it is a guaranteed no-op (structurally: `uv - gradient * 0 * clock`
// collapses to `uv` regardless of gradient or clock value).
//
// build.rs prepends `sdf_helper.wgsl` because this file's basename
// starts with `treat_field_advect` (added to SDF_CONSUMERS in build.rs).

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_params: vec4<f32>; // x=flow_speed, y=_pad, z=_pad, w=clock_secs
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
    let flow_speed = u_params.x;
    let clock_secs = u_params.w;

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

    // SDF gradient: finite-difference, not normalised.
    // Mirrors fx_particles_field_flow.wgsl line 97: `gradient * flow_direction * flow_speed`.
    // Using the raw (non-normalised) gradient means drift magnitude tapers
    // naturally away from the mask interior toward flat regions.
    let gradient = sample_sdf_gradient(t_sdf, in.uv);

    // Sample source at `uv - gradient * flow_speed * clock`.
    // The minus sign means we look back along the gradient — the image
    // appears to flow *toward* the gradient direction (outward along mask
    // normals). When flow_speed = 0.0 the offset is vec2(0) regardless of
    // gradient or clock: identity passthrough.
    let offset = gradient * flow_speed * clock_secs;
    return textureSample(t_source, s_source, uv - offset);
}
