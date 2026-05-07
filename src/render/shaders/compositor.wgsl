// N-layer compositing one layer at a time: sample previous composite + layer,
// emit blended RGBA. Blend mode via uniform (0 Normal, 1 Add, 2 Multiply, 3 Screen).

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );
    let p = positions[vi];
    var o: VsOut;
    o.pos = vec4<f32>(p, 0.0, 1.0);
    o.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return o;
}

@group(0) @binding(0) var t_base: texture_2d<f32>;
@group(0) @binding(1) var t_layer: texture_2d<f32>;
@group(0) @binding(2) var s_linear: sampler;
// x = opacity, y = blend_mode as f32 (0..3)
@group(0) @binding(3) var<uniform> u_params: vec4<f32>;

fn blend_normal(dst: vec4<f32>, src: vec4<f32>, opacity: f32) -> vec4<f32> {
    let sa = clamp(src.a * opacity, 0.0, 1.0);
    let sr = src.rgb;
    let da = clamp(dst.a, 0.0, 1.0);
    let dr = dst.rgb;
    let out_a = sa + da * (1.0 - sa);
    let out_rgb = (sr * sa + dr * da * (1.0 - sa)) / max(out_a, 1e-5);
    return vec4<f32>(out_rgb, out_a);
}

fn blend_add(dst: vec4<f32>, src: vec4<f32>, opacity: f32) -> vec4<f32> {
    let k = opacity * src.a;
    let rgb = clamp(dst.rgb + src.rgb * k, vec3<f32>(0.0), vec3<f32>(1.0));
    let a = clamp(dst.a + k * (1.0 - dst.a), 0.0, 1.0);
    return vec4<f32>(rgb, a);
}

fn blend_multiply(dst: vec4<f32>, src: vec4<f32>, opacity: f32) -> vec4<f32> {
    let k = clamp(opacity * src.a, 0.0, 1.0);
    let mixed = dst.rgb * mix(vec3<f32>(1.0), src.rgb, k);
    let out_a = blend_normal(dst, src, opacity).a;
    return vec4<f32>(mixed, out_a);
}

fn blend_screen(dst: vec4<f32>, src: vec4<f32>, opacity: f32) -> vec4<f32> {
    let k = clamp(opacity * src.a, 0.0, 1.0);
    let one = vec3<f32>(1.0);
    let srgb = one - (one - dst.rgb) * (one - src.rgb * k);
    let rgb = clamp(srgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let out_a = blend_normal(dst, src, opacity).a;
    return vec4<f32>(rgb, out_a);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dst = textureSample(t_base, s_linear, in.uv);
    let src = textureSample(t_layer, s_linear, in.uv);
    let opacity = clamp(u_params.x, 0.0, 1.0);
    let mode = i32(u_params.y);

    if mode == 1 {
        return blend_add(dst, src, opacity);
    }
    if mode == 2 {
        return blend_multiply(dst, src, opacity);
    }
    if mode == 3 {
        return blend_screen(dst, src, opacity);
    }
    return blend_normal(dst, src, opacity);
}
