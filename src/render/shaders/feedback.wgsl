// PCleanup.1.4 — Feedback / trails fragment shader (mix pass).
//
// Validated at build time by build.rs (naga). If this file fails to parse
// or validate, `cargo build` fails before any binary is produced.
//
// Reads two textures:
//   * `t_source` — the current-frame input to this effect (i.e. the output
//     of the previous effect in the chain).
//   * `t_history` — the previous frame's output of this Feedback pass
//     (kept in a per-layer history texture by the caller, refreshed via
//     the second feedback_blit.wgsl pass after each frame).
//
// Writes `mix(source, history(uv - offset), decay)`:
//   * decay = 0.0 → pure source (no trail).
//   * decay = 1.0 → pure history (infinite hold).
//   * decay = 0.95 → long trail (95% of each frame's pixel inherits from
//     the prior frame's pixel at the offset location).
//
// `offset` shifts the history sample by `(offset_x, offset_y)` in UV
// space. Positive `offset_x` makes trails appear to drift to the LEFT
// (history sampled at uv - offset). Values clamped to layer bounds in
// the shader.
//
// Bind group layout (mix pass):
//   @binding(0)  texture_2d<f32>  — source texture (this frame's input)
//   @binding(1)  texture_2d<f32>  — history texture (previous frame's output)
//   @binding(2)  sampler          — filtering sampler (shared)
//   @binding(3)  uniform buffer   — FeedbackParams (16 bytes: decay,
//                                   offset_x, offset_y, _pad)

@group(0) @binding(0) var t_source:  texture_2d<f32>;
@group(0) @binding(1) var t_history: texture_2d<f32>;
@group(0) @binding(2) var s_linear:  sampler;

struct FeedbackParams {
    decay:    f32,
    offset_x: f32,
    offset_y: f32,
    _pad:     f32,
};

@group(0) @binding(3) var<uniform> params: FeedbackParams;

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
    let src = textureSample(t_source, s_linear, in.uv);
    let decay = clamp(params.decay, 0.0, 1.0);
    let hist_uv = clamp(
        in.uv - vec2<f32>(params.offset_x, params.offset_y),
        vec2<f32>(0.0),
        vec2<f32>(1.0),
    );
    let hist = textureSample(t_history, s_linear, hist_uv);
    return mix(src, hist, decay);
}
