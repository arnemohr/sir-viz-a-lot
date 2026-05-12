// P2.6.1 — Fluid identity fragment shader.
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_".
//
// Reads the velocity texture at the fragment UV and renders it as colour:
//   R = Vx * 0.5 + 0.5
//   G = Vy * 0.5 + 0.5
//   B = 0.0
//   A = 0.5   (semi-transparent so the layer doesn't fully block layers below)
//
// This is a "proof-of-contract" preset demonstrating the compute → render
// pipeline without needing a visually polished output.
//
// Bind-group layout:
//   group 0, binding 0: velocity texture (texture_2d<f32>, RGBA16Float, filterable)
//   group 0, binding 1: sampler          (sampler, filtering)
//   group 0, binding 2: clock uniform    (vec4<f32>: .x = clock_secs)
//
// TODO(P2.9.2): golden baseline for fluid_identity render.

struct VsOut {
    @builtin(position) pos : vec4<f32>,
    @location(0)       uv  : vec2<f32>,
};

@group(0) @binding(0) var t_velocity : texture_2d<f32>;
@group(0) @binding(1) var s_linear   : sampler;
@group(0) @binding(2) var<uniform>    u_clock    : vec4<f32>;

// Fullscreen triangle (NDC corners outside [-1,1] get clipped).
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let p = positions[vi];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    // UV: map [-1,1] NDC to [0,1] UV, Y-flip for wgpu (NDC y-up, UV y-down).
    out.uv  = vec2<f32>(p.x * 0.5 + 0.5, 1.0 - (p.y * 0.5 + 0.5));
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let vel = textureSample(t_velocity, s_linear, in.uv).xy;
    let r = vel.x * 0.5 + 0.5;
    let g = vel.y * 0.5 + 0.5;
    return vec4<f32>(r, g, 0.0, 0.5);
}
