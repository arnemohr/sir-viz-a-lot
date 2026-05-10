// Test pattern: horizontal 0→1 luminance ramp across the canvas
// (P0.7.4). Used to verify two-projector edge-blend overlap +
// falloff settings (P0.7.3) without media on the canvas.
//
// Operator workflow: project this pattern across both projectors,
// observe the brightness curve through the overlap region, tune
// `EdgeBlendConfig.falloff_curve` until the seam is invisible.
// Real edge-blend uses a gamma-corrected curve (typically
// ramp^2.2), so this shader emits a *linear* ramp and the
// edge-blend pass downstream (the per-output present-time
// brightness shaping) is what produces the visually-uniform seam.
//
// Coordinate space: UV (0..1). u=0 → black on the left edge,
// u=1 → full white on the right edge. Vertical (v) is ignored.

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
    out.uv  = p * 0.5 + vec2<f32>(0.5, 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Linear ramp 0..1 along U. Clamp defensively in case the
    // sampler ever returns out-of-unit UVs (edge sampling).
    let intensity = clamp(in.uv.x, 0.0, 1.0);
    return vec4<f32>(intensity, intensity, intensity, 1.0);
}
