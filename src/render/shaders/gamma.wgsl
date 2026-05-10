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

// 64-byte uniform: tone (gamma, brightness, contrast, _) + 3 RGB
// matrix rows. Each row is `vec4<f32>` because uniform layout
// requires 16-byte alignment for vec3-shaped data; only the .xyz
// is used. Identity matrix = no-op for bit-exact equivalence to
// pre-P0.8.2 builds.
struct GammaUniforms {
    tone: vec4<f32>,    // x = gamma, y = brightness, z = contrast, w = unused
    row_r: vec4<f32>,   // .xyz = matrix row for output R
    row_g: vec4<f32>,   // .xyz = matrix row for output G
    row_b: vec4<f32>,   // .xyz = matrix row for output B
};
@group(0) @binding(2) var<uniform> u: GammaUniforms;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var c = textureSample(t_src, s_src, in.uv);
    let g = max(u.tone.x, 0.01);
    let bri = u.tone.y;
    let con = u.tone.z;
    let rgb = pow(max(c.rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / g));
    let toned = rgb * con + vec3<f32>(bri);
    // P0.8.2 — per-projector RGB matrix. Identity by default
    // (`OutputTarget::default().rgb_matrix == identity`), so
    // operators with no calibration in their project see byte-
    // identical output to pre-P0.8.2 builds.
    let out_r = dot(u.row_r.xyz, toned);
    let out_g = dot(u.row_g.xyz, toned);
    let out_b = dot(u.row_b.xyz, toned);
    return vec4<f32>(out_r, out_g, out_b, c.a);
}
