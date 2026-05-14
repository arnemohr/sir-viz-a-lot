// PCleanup.1.4 — Feedback blit pass.
//
// Runs after the mix pass (feedback.wgsl). Copies the freshly-written
// dst texture into the per-layer history texture so the NEXT frame's
// mix pass samples this frame's output as history.
//
// Why a render pass and not `encoder.copy_texture_to_texture`: the
// ping-pong textures in `EffectPipeline` are allocated with usage
// `TEXTURE_BINDING | RENDER_ATTACHMENT` only (see `make_texture` in
// `src/render/pipeline.rs`). Adding `COPY_SRC` would require changing
// the ping-pong allocator; a textured-quad render pass uses the
// already-present `TEXTURE_BINDING` flag.
//
// Validated at build time by build.rs (naga). If this file fails to
// parse or validate, `cargo build` fails before any binary is produced.
//
// Bind group layout:
//   @binding(0)  texture_2d<f32>  — dst texture (source for the blit)
//   @binding(1)  sampler          — filtering sampler

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_linear: sampler;

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
    return textureSample(t_source, s_linear, in.uv);
}
