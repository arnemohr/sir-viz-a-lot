// P0.7.3 — edge-blend overlap region rendering.
//
// Fullscreen multiply pass: outputs a grayscale factor that ramps from 1.0
// (outside the overlap region) down to 0.0 (at the projector's inner edge).
// Applied with multiplicative blend (`src_factor: Dst, dst_factor: Zero`) so
// the result equals the existing surface colour multiplied by the factor.
//
// Topology (v0.4, hardcoded):
//   output 0 (left projector)  → edge_side = 0.0 → right-edge falloff
//   output 1 (right projector) → edge_side = 1.0 → left-edge falloff
//
// The linear ramp satisfies sum-to-1.0 when both outputs use the same
// overlap_px; the cosine option is also complementary (cos + (1 - cos) = 1).

struct EdgeBlendUniform {
    /// Width of the overlap region in pixels on the projector surface.
    overlap_px: f32,
    /// Width of the projector surface in pixels (output.config.width).
    surface_width: f32,
    /// 0.0 = right-edge falloff (output 0 / left projector).
    /// 1.0 = left-edge falloff  (output 1 / right projector).
    edge_side: f32,
    /// 0.0 = linear ramp, 1.0 = cosine S-curve (mix between the two).
    falloff_curve: f32,
};

@group(0) @binding(0) var<uniform> u: EdgeBlendUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Fullscreen triangle covering the viewport without a vertex buffer.
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vi & 2u) * 2.0 - 1.0;
    return VsOut(vec4<f32>(x, y, 0.0, 1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Fragment position x is in pixels (wgpu gl_FragCoord convention).
    let x = in.pos.x;
    let w = max(u.surface_width, 1.0);
    let overlap = max(u.overlap_px, 1.0);

    // Distance from the relevant edge (in pixels):
    //   edge_side == 0 → right-edge falloff: dist = w - x  (0 at right edge)
    //   edge_side == 1 → left-edge falloff:  dist = x      (0 at left edge)
    let dist = mix(w - x, x, u.edge_side);

    var factor = 1.0;
    if dist < overlap {
        let t = dist / overlap;                          // 0..1 across the overlap region
        let linear_val = t;
        let cosine_val = 0.5 - 0.5 * cos(t * 3.14159265);
        factor = mix(linear_val, cosine_val, u.falloff_curve);
    }
    // Clamp for safety (guards against precision issues when overlap_px → 0).
    factor = clamp(factor, 0.0, 1.0);
    return vec4<f32>(factor, factor, factor, 1.0);
}
