// Light trail SDF shader — 005-T3.2 skeleton.
// Bind group contract (from 02-render-approach-decision.md):
//   @binding(0) source texture
//   @binding(1) source sampler
//   @binding(2) uniform LightTrailParams (192 bytes)
//   @binding(3) storage array<f32> polyline [px, py, arclen] triples
//
// T3.2: source passthrough only — comet SDF rendering lands in T3.3.

// ---------------------------------------------------------------------------
// Uniform struct — mirror of Rust LightTrailParams (192 bytes, std140)
// ---------------------------------------------------------------------------

struct LightTrailParams {
    progress:         f32,
    trail_length:     f32,
    head_size:        f32,
    stroke_width:     f32,
    glow_blur:        f32,
    opacity_fade:     f32,
    gradient_spread:  f32,
    start:            f32,
    end:              f32,
    align:            u32,
    path_index:       u32,
    sample_resolution: u32,
    palette_mode:     u32,
    hue_shift_speed:  f32,
    palette_len:      u32,
    _pad0:            u32,
    palette_colors:   array<vec4<f32>, 8>,
};

// ---------------------------------------------------------------------------
// Bindings
// ---------------------------------------------------------------------------

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> params: LightTrailParams;
@group(0) @binding(3) var<storage, read> polyline: array<f32>;

// ---------------------------------------------------------------------------
// Vertex shader — fullscreen triangle pair (copied from blur_h.wgsl pattern)
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
    out.uv  = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

// ---------------------------------------------------------------------------
// Fragment shader — source passthrough (T3.3 adds SDF comet rendering)
// ---------------------------------------------------------------------------

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // T3.2: passthrough — just sample the source texture.
    // T3.3 will use params + polyline to compute the SDF comet and blend over this.
    _ = params.progress;      // suppress unused-variable naga warnings
    _ = arrayLength(&polyline);
    return textureSample(t_source, s_source, in.uv);
}
