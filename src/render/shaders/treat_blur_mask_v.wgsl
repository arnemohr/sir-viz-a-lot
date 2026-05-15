// P1.3.2 — `blur_mask` treatment, vertical pass. Reads the
// horizontally-blurred intermediate, writes the final result to dst.
// Per-fragment radius matches the H pass (same SDF math, same params).
//
// PCleanup.8.3c: uniform expanded to 32 bytes (array<vec4<f32>, 2>);
// adds `radius_mode` (w component of params[0]) and `distance_falloff`
// (x component of params[1]). Mirrors treat_blur_mask_h.wgsl exactly.
//
// build.rs prepends `sdf_helper.wgsl` because the basename starts with
// `treat_blur`.

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(0) @binding(2) var<uniform> u_params: array<vec4<f32>, 2>;
@group(0) @binding(3) var t_sdf: texture_2d<f32>;

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
    let max_radius_px    = max(u_params[0].x, 0.0);
    let edge_band        = max(u_params[0].y, 1e-4);
    let falloff          = clamp(u_params[0].z, 0.0, 1.0);
    let radius_mode      = i32(u_params[0].w + 0.5);  // 0=edge-band (default), 1=distance-driven
    let distance_falloff = max(u_params[1].x, 1e-4);  // meaningful at radius_mode=1

    let d = abs(sample_sdf_bilinear(t_sdf, in.uv));

    var r: f32;
    if (radius_mode == 1) {
        // Distance-driven: sharp at edge, blurry toward interior.
        r = max_radius_px * smoothstep(0.0, distance_falloff, d);
    } else {
        // Edge-band (mode 0, current default — preserved exactly).
        let proximity = 1.0 - smoothstep(0.0, edge_band, d);
        let shape = pow(proximity, mix(8.0, 1.0, falloff));
        r = max_radius_px * shape;
    }

    let dims = textureDimensions(t_diffuse, 0);
    let texel_y = 1.0 / f32(dims.y);

    let radius = clamp(r, 0.0, 32.0);
    let r_int = i32(round(radius));
    let sigma = max(radius * 0.5, 0.5);
    let two_sigma_sq = 2.0 * sigma * sigma;

    var color = vec4<f32>(0.0);
    var weight_sum = 0.0;

    for (var i = -32; i <= 32; i = i + 1) {
        if (abs(i) > r_int) {
            continue;
        }
        let offset = vec2<f32>(0.0, f32(i) * texel_y);
        let weight = exp(-f32(i * i) / two_sigma_sq);
        color = color + weight * textureSample(t_diffuse, s_diffuse, in.uv + offset);
        weight_sum = weight_sum + weight;
    }

    return color / max(weight_sum, 1e-6);
}
