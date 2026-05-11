// Warped fullscreen mesh: VS passes dst UV (0–1 output) and source UV for sampling.
// FS samples scene (linear) + optional R32Float SDF mask (textureLoad bilinear; format is unfilterable).
// sdf_helper.wgsl is prepended at pipeline build time (see warp.rs) — sample_sdf_bilinear
// is declared there and takes t_sdf as an explicit parameter.

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
        let d = sample_sdf_bilinear(t_sdf, in.mask_uv);
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
