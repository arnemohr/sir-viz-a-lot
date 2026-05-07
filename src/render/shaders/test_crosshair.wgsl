// Test pattern: centered crosshair + four corner L-brackets for
// projector calibration. M2. Validated at build time by build.rs
// (naga). Spec §6 Test patterns.
//
// Bright white crosshair (one vertical + one horizontal line through
// the framebuffer center) plus traditional camera-style L-shaped
// framing markers in each of the four corners. Background pure black.
//
// Vertex stage matches triangle.wgsl / test_grid.wgsl: a six-vertex
// fullscreen quad driven by @builtin(vertex_index). Only the fragment
// stage differs.
//
// Coordinate space: this shader works in UV space (0..1 across the
// framebuffer) rather than pixel space, because WGSL has no direct
// access to the framebuffer dimensions without a uniform binding,
// and dispatch + bind-group plumbing is owned by T-M2-08. For a
// calibration pattern an operator reads visually, a small UV epsilon
// (~0.001) is "thin enough to look like 1 px" on any reasonable
// projector resolution. Corner markers are sized in UV likewise.

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
    let uv = in.uv;

    // Crosshair: 1-pixel-thin vertical and horizontal lines through
    // the center. ~0.001 UV ≈ 1 px on a 1080p output; visibly thin
    // on any resolution typical for projection mapping.
    let crosshair_half_thickness = 0.0005;
    let on_vline = abs(uv.x - 0.5) < crosshair_half_thickness;
    let on_hline = abs(uv.y - 0.5) < crosshair_half_thickness;
    let on_crosshair = on_vline || on_hline;

    // Corner L-brackets. marker_size is the arm length (5% of the
    // smaller framebuffer dim ≈ 50 px on 1080p), marker_thickness is
    // the line width (~5 px on 1080p). Distance from the nearest
    // corner is min(uv, 1 - uv): zero at the corner, 0.5 at center.
    let marker_size = 0.05;
    let marker_thickness = 0.005;
    let corner_dist = min(uv, vec2<f32>(1.0, 1.0) - uv);

    // An L-bracket is "on" when one arm is short (within thickness of
    // the corner edge) and the other arm is within marker_size of the
    // corner. Mirroring via corner_dist handles all four corners with
    // a single test.
    let on_horiz_arm = corner_dist.y < marker_thickness && corner_dist.x < marker_size;
    let on_vert_arm  = corner_dist.x < marker_thickness && corner_dist.y < marker_size;
    let on_marker = on_horiz_arm || on_vert_arm;

    let lit = on_crosshair || on_marker;
    let intensity = select(0.0, 0.9, lit);
    return vec4<f32>(intensity, intensity, intensity, 1.0);
}
