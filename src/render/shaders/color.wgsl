// Color effect pass: hue shift, saturation, brightness, contrast.
// M4 effects pipeline. Spec §2 + plan §3.4 M4. T-M4-02.
//
// Validated at build time by build.rs (naga). If this file fails to parse or
// validate, `cargo build` fails before any binary is produced.
//
// HSV color model is used for hue/saturation manipulations (simpler
// than HSL for this purpose). rgb2hsv / hsv2rgb use the branchless
// Sam Hocevar form (mix + step), which avoids conditionals and compiles
// cleanly under naga.
//
// Bind group layout:
//   @binding(0)  texture_2d<f32>   – source texture (sampled in fragment)
//   @binding(1)  sampler           – filtering sampler
//   @binding(2)  uniform buffer    – ColorParams { hue_shift_deg, saturation_mul,
//                                    brightness_add, contrast_mul } (4 × f32, 16 bytes)

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;

struct ColorParams {
    hue_shift_deg: f32,
    saturation_mul: f32,
    brightness_add: f32,
    contrast_mul:   f32,
};

@group(0) @binding(2) var<uniform> params: ColorParams;

// ---------------------------------------------------------------------------
// Vertex stage: standard six-vertex fullscreen quad, Y-flipped UVs.
// Identical to textured_quad.wgsl.
// ---------------------------------------------------------------------------

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
    // Flip Y so the texture's top-left maps to the screen's top-left.
    out.uv  = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

// ---------------------------------------------------------------------------
// HSV <-> RGB (branchless Sam Hocevar form)
// Reference: https://stackoverflow.com/a/17897228
// ---------------------------------------------------------------------------

fn rgb2hsv(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = mix(vec4<f32>(c.bg, K.wz), vec4<f32>(c.gb, K.xy), step(c.b, c.g));
    let q = mix(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), step(p.x, c.r));
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

// ---------------------------------------------------------------------------
// Fragment stage
// ---------------------------------------------------------------------------

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // 1. Sample source texture, preserve alpha.
    let sample = textureSample(t_source, s_source, in.uv);
    let src_alpha = sample.a;

    // 2. Convert RGB -> HSV.
    var hsv = rgb2hsv(sample.rgb);

    // 3. Apply hue shift (degrees -> fraction, then wrap via fract).
    //    fract() in WGSL returns a value in [0, 1) for any input, so
    //    negative shifts wrap correctly without extra conditionals.
    hsv.x = fract(hsv.x + params.hue_shift_deg / 360.0);

    // 4. Apply saturation multiplier, clamp to [0, 1].
    hsv.y = clamp(hsv.y * params.saturation_mul, 0.0, 1.0);

    // 5. Convert HSV -> RGB.
    var rgb = hsv2rgb(hsv);

    // 6. Apply contrast around 0.5.
    rgb = (rgb - vec3<f32>(0.5)) * params.contrast_mul + vec3<f32>(0.5);

    // 7. Apply brightness as additive offset.
    rgb = rgb + vec3<f32>(params.brightness_add);

    // 8. Clamp and output with original source alpha.
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), src_alpha);
}
