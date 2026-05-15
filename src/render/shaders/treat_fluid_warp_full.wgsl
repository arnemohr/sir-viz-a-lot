// PCleanup.2.3 — `fluid_warp_full` treatment.
//
// Unbounded sibling of `treat_fluid_warp.wgsl` (PCleanup.1.2).
// Same formula (textureSample(source, uv - velocity * amplitude)), same
// fragment-shader shape, but uses the `fluid_identity` compute pass (no
// SDF mask boundary) instead of `fluid_bounded`.  The result is full-layer
// fluid distortion — the warp covers the whole layer rect, not just inside
// the mask.  Works on any layer source (Image / Video / SVG / FxLayer)
// because no SDF is required.
//
// Each frame the owning `FluidWarpFullTreatmentPipeline`:
//   1. Runs a fluid_identity advect compute pass (via its owned
//      `FxFluidPipeline`) to update the 256×256 RGBA16Float velocity field.
//      Unlike the bounded variant there is no SDF constraint — velocity is
//      non-zero across the full quad and the warp covers the entire layer.
//   2. Runs this fragment pass: samples `t_source` at
//      `uv - velocity(uv) * amplitude`, writing the warped result to `dst`.
//
// Identity rule: `amplitude = 0.0` → offset = vec2(0) regardless of velocity
// field content → output equals textureSample(t_source, s_source, uv) exactly.
// (Structurally: the velocity multiplication by zero is a compile-time collapse
// once the driver has the uniform value; even at runtime this is bit-identical
// passthrough because vel * 0.0 = vec2(0,0).)
//
// The clamp on `warped_uv` is load-bearing here (not just defensive as in
// fluid_warp): since velocity is non-zero everywhere, without a clamp the
// displaced UV can easily wander off [0,1] and produce sampling halos at the
// layer edges.  ClampToEdge on the sampler is also set, but the explicit
// clamp gives a hard black-transparent edge when the warp pushes past the
// layer boundary.
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
// NOTE: this shader does NOT include sdf_helper.wgsl.  The fluid_identity
// compute pass has no mask dependency.  Do NOT add "treat_fluid_warp_full"
// to SDF_CONSUMERS in build.rs.

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

    // Apply fit transform to UV (same logic as treat_fluid_warp.wgsl).
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
    // Unlike fluid_warp (bounded variant), velocity is non-zero across the
    // entire quad — no implicit mask zeroing — so the warp affects the
    // whole layer surface.
    let vel = textureSample(t_velocity, s_source, in.uv).rg;

    // Displace the source UV by `vel * amplitude`.
    // When amplitude = 0.0 the offset is vec2(0,0) → identity passthrough.
    // clamp is load-bearing here: with full-layer velocity, unguarded
    // warped_uv easily escapes [0,1] and causes edge sampling halos.
    let warped_uv = clamp(uv - vel * amplitude, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(t_source, s_source, warped_uv);
}
