// PCleanup.1.2 — fluid_warp draw shader.
//
// First SourceModifier preset (per FxFamily::SourceModifier, PCleanup.1.1):
// instead of rendering the velocity field as colour like
// fx_fluid_identity.wgsl does, this shader **reads the underlying layer
// source texture** and samples it at `uv - velocity * amplitude`.
// Result: the photo flows according to the fluid sim.
//
// Compute side: reuses fx_fluid_bounded.wgsl unchanged — the velocity
// field already enforces no-slip at the mask edge, so the warp stays
// inside the masked region naturally.
//
// Bind group layout (separate from fluid_identity's 3-binding layout):
//   @binding(0) texture_2d<f32>  — velocity field (RGBA16Float, filterable)
//   @binding(1) sampler          — filtering sampler
//   @binding(2) uniform<vec4>    — .x = clock_secs (unused in fs), .y = amplitude
//   @binding(3) texture_2d<f32>  — layer source texture (the photo to warp)
//
// Validated at build time by build.rs (naga).

@group(0) @binding(0) var t_velocity : texture_2d<f32>;
@group(0) @binding(1) var s_linear   : sampler;
@group(0) @binding(2) var<uniform>    u_clock    : vec4<f32>;
@group(0) @binding(3) var t_source   : texture_2d<f32>;

struct VsOut {
    @builtin(position) pos : vec4<f32>,
    @location(0)       uv  : vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Fullscreen triangle (NDC corners outside [-1,1] get clipped). Same
    // pattern as fx_fluid_identity.wgsl.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let p = positions[vi];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    // Y-flip: NDC y-up → UV y-down.
    out.uv  = vec2<f32>(p.x * 0.5 + 0.5, 1.0 - (p.y * 0.5 + 0.5));
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let amplitude = u_clock.y;
    // Velocity at this fragment's UV. The bounded-fluid compute already
    // zeroed velocity outside the mask, so the warp self-limits.
    let vel = textureSample(t_velocity, s_linear, in.uv).xy;
    // Sample the photo at the warped coordinate. Clamp prevents sampling
    // off-edge when the velocity field points outward at the boundary
    // (gives a mild "freeze on the rim" look rather than a black halo).
    let warped_uv = clamp(in.uv - vel * amplitude, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(t_source, s_linear, warped_uv);
}
