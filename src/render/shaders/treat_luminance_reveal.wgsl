// P1.3.3 — `luminance_reveal` treatment.
//
// The layer's alpha becomes a smoothstep over its own luminance: pixels
// brighter than `threshold` show; pixels darker fade out. A `softness`
// band smooths the cut so the operator never sees a jagged 1-bit edge.
// `invert` (0/1) flips the relationship so darker pixels show instead.
//
// RGB is passed through unchanged — only alpha is modulated. The
// compositor blends the result normally so downstream warp + mask still
// apply.
//
// Luminance is computed with Rec. 601 weights (Y' = 0.299 R + 0.587 G +
// 0.114 B). Rec. 601 is the conventional "perceived brightness" formula
// for sRGB content; matches what After Effects / Resolve use for the
// equivalent operation, so live-show operators with that muscle memory
// see consistent results.
//
// Bind layout mirrors `treat_tone_map.wgsl`:
//   group 0 / binding 0   source texture
//   group 0 / binding 1   filtering sampler
//   group 0 / binding 2   fit_uniform (16 bytes)
//   group 0 / binding 3   params: vec4<f32> (x=threshold, y=softness, z=invert, w=reserved)

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(0) @binding(2) var<uniform> u_fit: vec4<f32>;
@group(0) @binding(3) var<uniform> u_params: vec4<f32>;

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
    let mode   = i32(u_fit.x + 0.5);
    let aspect = max(u_fit.y, 1e-4);
    let focal  = vec2<f32>(u_fit.z, u_fit.w);
    var uv = in.uv;

    if (mode == 1) {
        // Cover
        if (aspect > 1.0) {
            let scale = 1.0 / aspect;
            uv.x = (uv.x - 0.5) * scale + focal.x;
        } else {
            let scale = aspect;
            uv.y = (uv.y - 0.5) * scale + focal.y;
        }
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    } else if (mode == 2) {
        // Contain
        if (aspect > 1.0) {
            let scale = aspect;
            uv.y = (uv.y - 0.5) * scale + 0.5;
        } else {
            let scale = 1.0 / aspect;
            uv.x = (uv.x - 0.5) * scale + 0.5;
        }
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }

    let src = textureSample(t_diffuse, s_diffuse, uv);

    let threshold = u_params.x;
    let softness  = max(u_params.y, 1e-4); // avoid /0 in smoothstep degenerate
    let invert    = u_params.z;

    // Rec. 601 luma.
    let luma = dot(src.rgb, vec3<f32>(0.299, 0.587, 0.114));

    // smoothstep(low, high, x) is 0 below low, 1 above high; that gives
    // us bright-pixels-show by default.
    let mask = smoothstep(threshold - softness, threshold + softness, luma);

    // Invert is expressed as a 0/1 float so the HashMap-keyed param
    // shape stays uniform across presets. `mix(mask, 1 - mask, invert)`
    // flips cleanly when invert >= 0.5 — and degrades gracefully if the
    // operator drags it to a fractional value (cross-fade between modes).
    let alpha_mask = mix(mask, 1.0 - mask, clamp(invert, 0.0, 1.0));

    // Pre-multiply RGB by the alpha so the premultiplied-alpha blend
    // downstream gives the expected look. Source alpha is also
    // multiplied so transparent pixels in the input stay transparent.
    let out_a = src.a * alpha_mask;
    return vec4<f32>(src.rgb * out_a, out_a);
}
