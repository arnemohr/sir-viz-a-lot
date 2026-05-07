// Final master: pow(rgb, 1/gamma) * contrast + brightness (per Project).

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

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;
// gamma, brightness, contrast, _
@group(0) @binding(2) var<uniform> u_tone: vec4<f32>;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var c = textureSample(t_src, s_src, in.uv);
    let g = max(u_tone.x, 0.01);
    let bri = u_tone.y;
    let con = u_tone.z;
    let rgb = pow(max(c.rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / g));
    c = vec4<f32>(rgb * con + vec3<f32>(bri), c.a);
    return c;
}
