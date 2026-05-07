// Separable gaussian blur, horizontal pass. T-M4-03. Validated by
// build.rs (naga). T-M4-04 adds the vertical pass and the
// orchestrator (effects::blur::apply runs h then v into ping-pong
// views).
//
// Bind group: @binding(0) source texture, @binding(1) sampler,
// @binding(2) uniform { radius_px: f32 }.

struct BlurParams {
    radius_px: f32,
};

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(0) @binding(2) var<uniform> params: BlurParams;

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
    let dims = textureDimensions(t_diffuse, 0);
    let texel = vec2<f32>(1.0 / f32(dims.x), 1.0 / f32(dims.y));

    // Clamp the kernel half-width to 32 pixels for a bounded loop.
    let radius = clamp(params.radius_px, 0.0, 32.0);
    let r_int = i32(round(radius));
    let sigma = max(radius * 0.5, 0.5);  // avoid div-by-zero at radius=0
    let two_sigma_sq = 2.0 * sigma * sigma;

    var color = vec4<f32>(0.0);
    var weight_sum = 0.0;

    // The fixed loop bound MUST be a constant for WGSL; we walk from
    // -32 to +32 and skip work outside the actual kernel via early
    // continue. WGSL allows `continue` inside loops.
    for (var i = -32; i <= 32; i = i + 1) {
        if (abs(i) > r_int) {
            continue;
        }
        let offset = vec2<f32>(f32(i) * texel.x, 0.0);
        let weight = exp(-f32(i * i) / two_sigma_sq);
        color = color + weight * textureSample(t_diffuse, s_diffuse, in.uv + offset);
        weight_sum = weight_sum + weight;
    }

    return color / max(weight_sum, 1e-6);
}
