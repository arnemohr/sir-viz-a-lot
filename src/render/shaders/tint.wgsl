// PCleanup.4.1 — Tint effect: three-mode colour mixing pass.
//
// Validated at build time by build.rs (naga). If this file fails to parse
// or validate, `cargo build` fails before any binary is produced.
//
// Reads the source texture, mixes it with a configured RGBA colour by
// `amount`, and writes the result. Three modes selected by the `mode` u32:
//   0 = Multiply  — src * mix(white, tint, amount). Darkens toward the
//                   tint colour; the canonical "proper" tint.
//   1 = Additive  — src + tint * amount * src.a. Lightens; classic "wash".
//   2 = Screen    — 1 - (1-src) * (1 - tint*amount). Soft additive that
//                   never blows past 1.0.
//
// The source alpha is preserved unchanged. The output is premultiplied-
// alpha-compatible because every mixing term scales by src.a where
// appropriate (the Color effect uses the same convention).
//
// Bind group layout (matches `color.wgsl` for cache symmetry):
//   @binding(0)  texture_2d<f32>  — source texture (sampled in fragment)
//   @binding(1)  sampler          — filtering sampler
//   @binding(2)  uniform buffer   — TintParams (rgba + amount + mode + 2× pad,
//                                   8 × f32 = 32 bytes; std140-friendly)

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;

struct TintParams {
    // rgba.rgb is the tint colour; rgba.a is a per-channel pre-scale
    // (an operator usually sets a=1.0; a<1 lets you weaken the colour
    // without touching the `amount` modulator). Both kept independent so
    // a modulator-driven `amount` can pulse without disturbing the colour.
    rgba: vec4<f32>,
    amount: f32,
    mode: u32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(2) var<uniform> params: TintParams;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0)       uv:  vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    // Six-vertex fullscreen quad; Y-flipped UVs to match texture
    // top-left. Identical to color.wgsl.
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
    let src = textureSample(t_source, s_source, in.uv);
    let src_alpha = src.a;

    // Effective tint colour after the rgba.a pre-scale. `amount` is
    // clamped so a modulator that briefly overshoots [0,1] doesn't blow
    // out the blend.
    let tint = params.rgba.rgb * params.rgba.a;
    let amount = clamp(params.amount, 0.0, 1.0);

    var out_rgb: vec3<f32>;
    if params.mode == 0u {
        // Multiply: mix toward (src * tint). At amount=0, passthrough.
        // At amount=1, src is fully multiplied by tint.
        out_rgb = src.rgb * mix(vec3<f32>(1.0), tint, amount);
    } else if params.mode == 1u {
        // Additive: lift each channel by amount*tint, scaled by src.a so
        // transparent pixels don't suddenly glow. Allowed to exceed 1.0
        // in HDR-style pipelines; the wgpu colour target will clamp on
        // store if the format is 8-bit.
        out_rgb = src.rgb + tint * amount * src_alpha;
    } else {
        // Screen: 1 - (1-src)(1 - tint*amount). Always preserves
        // [0,1] (assuming src and tint*amount each in [0,1]).
        let ta = tint * amount;
        out_rgb = src.rgb + ta * src_alpha - src.rgb * ta;
    }

    return vec4<f32>(out_rgb, src_alpha);
}
