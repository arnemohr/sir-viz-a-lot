// Test pattern: 50-pixel grid for projection-mapping calibration. M2.
// Validated at build time by build.rs (naga). Spec §6 Test patterns.
//
// Dim grey 1-pixel lines every 50 px; brighter 1-pixel lines every
// 250 px (every 5th). Background pure black so the projection surface
// remains visible between lines.
//
// Vertex stage matches triangle.wgsl: a six-vertex fullscreen quad
// driven by @builtin(vertex_index). Only the fragment stage differs.

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
    // @builtin(position) in the fragment stage is the pixel coordinate
    // (framebuffer space): x in [0, width), y in [0, height).
    let px = in.pos.xy;

    // A line is "on" when the pixel falls within the first 1 px of a
    // period. fract(px / period) < 1/period selects exactly one column
    // (or row) of pixels per period. step(edge, x) returns 1.0 when
    // x < edge, else 0.0 — but WGSL's step is (edge, x) with the
    // convention step(edge, x) = x >= edge ? 1 : 0, so we invert by
    // using 1.0 - step(...).
    let minor_period = 50.0;
    let major_period = 250.0;

    let minor_f = fract(px / minor_period);
    let major_f = fract(px / major_period);

    // 1 - step(threshold, frac): 1 when frac < threshold, else 0.
    let minor = vec2<f32>(1.0, 1.0) - step(
        vec2<f32>(1.0 / minor_period, 1.0 / minor_period),
        minor_f,
    );
    let major = vec2<f32>(1.0, 1.0) - step(
        vec2<f32>(1.0 / major_period, 1.0 / major_period),
        major_f,
    );

    let on_minor = max(minor.x, minor.y);
    let on_major = max(major.x, major.y);

    // Major lines override minor lines where they coincide.
    let intensity = max(on_minor * 0.25, on_major * 0.7);
    return vec4<f32>(intensity, intensity, intensity, 1.0);
}
