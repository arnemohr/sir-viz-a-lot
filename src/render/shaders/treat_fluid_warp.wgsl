// PCleanup.1.2 — `fluid_warp` treatment.
//
// Re-paths the abandoned `fx_fluid_warp.wgsl` (commit 2a30578) as a
// Treatment, per the source-modifier-placement decision (commit 920c8c2).
//
// Each frame the owning `FluidWarpTreatmentPipeline`:
//   1. Runs a bounded-fluid advect compute pass (via its owned
//      `FxFluidPipeline`) to update the 256×256 RGBA16Float velocity field.
//      The compute shader zeroes velocity outside the mask and reflects at
//      the boundary, so the warp is naturally constrained to the masked region.
//   2. Runs this fragment pass: samples `t_source` at
//      `uv - velocity(uv) * amplitude`, writing the warped result to `dst`.
//
// Identity rule: `amplitude = 0.0` → offset = vec2(0) regardless of velocity
// field content → output equals textureSample(t_source, s_source, uv) exactly.
// (Structurally: the velocity multiplication by zero is a compile-time collapse
// once the driver has the uniform value; even at runtime this is bit-identical
// passthrough because vel * 0.0 = vec2(0,0).)
//
// The compute side drives a swirl injector at inject_intensity=0.4, so the
// velocity field is non-zero at amplitude > 0 and the warp is visible.
//
// Bind-group layout (5 entries, Treatment convention):
//   @binding(0) texture_2d<f32>   — t_source:    layer source (filterable)
//   @binding(1) sampler           — s_source:    clamp-to-edge linear
//   @binding(2) uniform vec4<f32> — u_params:    .x = amplitude (0..=2),
//                                                 .w = clock_secs (frame write)
//   @binding(3) uniform vec4<f32> — u_fit:       fit_mode, aspect, focal_x, focal_y
//   @binding(4) texture_2d<f32>   — t_velocity:  RGBA16Float, filterable
//                                   (written each frame by the compute pre-pass;
//                                    uses the same filtering sampler as t_source)
//
// NOTE: this shader does NOT include sdf_helper.wgsl. The bounded-fluid
// compute already enforces zero velocity outside the mask, so the fragment
// pass needs no SDF sampling. Do NOT add "treat_fluid_warp" to SDF_CONSUMERS
// in build.rs.

@group(0) @binding(0) var t_source:   texture_2d<f32>;
@group(0) @binding(1) var s_source:   sampler;
@group(0) @binding(2) var<uniform>    u_params:   vec4<f32>; // x=amplitude, w=clock_secs
@group(0) @binding(3) var<uniform>    u_fit:      vec4<f32>; // mode, aspect, focal_x, focal_y
@group(0) @binding(4) var t_velocity: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0)       uv:  vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    // Two-triangle quad covering the full render target. Matches the
    // vertex layout used by all other Treatment shaders (field_advect, etc.).
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
    // clock_secs is in u_params.w (same convention as edge_lens / field_advect).
    // Not used in this fragment shader, but packed per convention so the
    // dispatcher can write it without a separate uniform buffer.

    // Apply fit transform to UV (same logic as treat_field_advect.wgsl).
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

    // Sample the velocity field at this UV. Velocity is in .rg of the
    // RGBA16Float texture; .ba are zero (unused).
    // The bounded-fluid compute has already zeroed velocity outside the mask,
    // so no explicit mask test is needed here.
    let vel = textureSample(t_velocity, s_source, in.uv).rg;

    // Displace the source UV by `vel * amplitude`.
    // When amplitude = 0.0 the offset is vec2(0,0) → identity passthrough.
    let warped_uv = clamp(uv - vel * amplitude, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(t_source, s_source, warped_uv);
}
