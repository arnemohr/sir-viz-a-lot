// P2.5.1 — particle quad vertex shader.
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_".
// This shader does not call SDF functions; the helpers are present but
// unused (same situation as fx_particles_identity_compute.wgsl).
//
// Reads particle positions from the SSBO produced by the compute pass,
// and emits two-triangle (6-vertex) screen-aligned quads. Each instance
// corresponds to one particle; each draw call uses 6 vertices.
//
// Quad size: 2×2 pixels in screen space, converted to NDC using the
// output resolution from the params uniform (aliased fields).
//
// Bind-group layout:
//   group 0, binding 3: ClockUniform (vec4<f32>)
//                         .x = clock_secs
//                         .y = t_layer_local_secs
//                         .z = seed_f32
//                         .w = n_particles
//   group 0, binding 4: ResUniform (vec4<f32>)
//                         .x = output_width  (f32)
//                         .y = output_height (f32)
//   group 0, binding 5: particle SSBO (read-only)

struct Particle {
    pos:      vec2<f32>,
    vel:      vec2<f32>,
    age_secs: f32,
    _pad:     f32,
    _pad2:    f32,
    _pad3:    f32,
};

@group(0) @binding(3) var<uniform>         u_clock    : vec4<f32>;
@group(0) @binding(4) var<uniform>         u_res      : vec4<f32>;
@group(0) @binding(5) var<storage, read>   particles  : array<Particle>;

struct VsOut {
    @builtin(position) pos : vec4<f32>,
};

// Two-triangle unit quad in [0, 1]² (before repositioning).
// Vertex index 0..6 maps to two triangles covering the unit square.
const QUAD_OFFSETS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
);

@vertex
fn vs_main(
    @builtin(vertex_index)   vi  : u32,
    @builtin(instance_index) inst: u32,
) -> VsOut {
    let n = u32(u_clock.w);
    // Clamp to SSBO capacity.
    let max_n = min(n, 2048u);

    var out: VsOut;
    if inst >= max_n {
        // Dead instance: clip to out-of-frustum (w=0 → NDC undefined →
        // rasteriser drops the primitive).
        out.pos = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    let p   = particles[inst];
    let res = vec2<f32>(u_res.x, u_res.y);

    // Particle centre in pixel space (flip Y: UV origin is top-left,
    // NDC origin is bottom-left).
    let px_centre = vec2<f32>(p.pos.x * res.x, (1.0 - p.pos.y) * res.y);

    // 2×2 px quad: offset the corner by ±1 px around the centre.
    // QUAD_OFFSETS are in [0,1]²; scale to 2×2 px, centre at particle.
    let corner_px = px_centre + (QUAD_OFFSETS[vi] - vec2<f32>(0.5)) * 2.0;

    // Convert pixel coords to NDC: x ∈ [-1, 1], y ∈ [-1, 1].
    let ndc_x = corner_px.x / res.x * 2.0 - 1.0;
    let ndc_y = corner_px.y / res.y * 2.0 - 1.0;

    out.pos = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    return out;
}
