// Warped fullscreen mesh: VS passes dst UV (0–1 output) and source UV for sampling.
// FS samples scene (linear) + optional R32Float SDF mask (textureLoad bilinear; format is unfilterable).

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) src_uv: vec2<f32>,
    @location(1) mask_uv: vec2<f32>,
}

struct VsIn {
    @location(0) pos_clip: vec2<f32>,
    @location(1) src_uv: vec2<f32>,
}

@group(0) @binding(0) var t_scene: texture_2d<f32>;
@group(0) @binding(1) var s_scene: sampler;
@group(0) @binding(2) var t_sdf: texture_2d<f32>;
// x = use_mask (1 / 0), y = feather distance (same units as CPU SDF baker), zw unused
@group(0) @binding(3) var<uniform> u_mask: vec4<f32>;

fn sample_sdf_bilinear(uv: vec2<f32>) -> f32 {
    let dims_u = textureDimensions(t_sdf);
    let dims = vec2<f32>(dims_u);
    let max_x = i32(dims_u.x) - 1;
    let max_y = i32(dims_u.y) - 1;
    let p = uv * dims - vec2<f32>(0.5);
    let i0 = vec2<i32>(i32(floor(p.x)), i32(floor(p.y)));
    let f = p - vec2<f32>(f32(i0.x), f32(i0.y));
    let ix0 = clamp(i0.x, 0, max_x);
    let iy0 = clamp(i0.y, 0, max_y);
    let ix1 = clamp(i0.x + 1, 0, max_x);
    let iy1 = clamp(i0.y + 1, 0, max_y);
    let v00 = textureLoad(t_sdf, vec2<i32>(ix0, iy0), 0).r;
    let v10 = textureLoad(t_sdf, vec2<i32>(ix1, iy0), 0).r;
    let v01 = textureLoad(t_sdf, vec2<i32>(ix0, iy1), 0).r;
    let v11 = textureLoad(t_sdf, vec2<i32>(ix1, iy1), 0).r;
    return mix(mix(v00, v10, f.x), mix(v01, v11, f.x), f.y);
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var o: VsOut;
    o.pos = vec4<f32>(in.pos_clip, 0.0, 1.0);
    o.src_uv = in.src_uv;
    // mask uses output-normalized coords — matches baker polygon space
    o.mask_uv = vec2<f32>(in.pos_clip.x * 0.5 + 0.5, 0.5 - in.pos_clip.y * 0.5);
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var c = textureSample(t_scene, s_scene, in.src_uv);
    let use_mask = u_mask.x > 0.5;
    if (use_mask) {
        let feather = max(u_mask.y, 1e-4);
        let d = sample_sdf_bilinear(in.mask_uv);
        let a = 1.0 - smoothstep(-feather, feather, d);
        // Premultiply: the projector swapchain ignores alpha, so attenuating
        // only `c.a` left full-brightness RGB hitting the lamp. Multiplying
        // RGB by the mask alpha forces black (no light) outside the polygon
        // and a smooth ramp through the feather band — that's the actual
        // physical cut-off we want.
        c = vec4<f32>(c.rgb * a, c.a * a);
    }
    return c;
}
