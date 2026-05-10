// Test pattern: alignment cross with quarter / half / three-quarter
// reference markings, for two-projector physical alignment (P0.7.4).
//
// Builds on `test_crosshair.wgsl` (the v3 pattern) by adding three
// vertical and three horizontal tick lines at 25 / 50 / 75 % across
// the framebuffer. The 50% lines double as the centred cross; the
// 25% / 75% lines give operators reference fractions when nudging
// the second projector into place against the first.
//
// Coordinate space: UV (0..1). Same justification as
// test_crosshair.wgsl — no uniform binding required, the operator
// reads the markings visually and a small UV epsilon (~0.001) is
// "thin enough to look like 1 px" on any reasonable projector.

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

fn near(value: f32, at: f32, half_thickness: f32) -> bool {
    return abs(value - at) < half_thickness;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Centre lines (50% — full brightness, drawn thicker).
    let centre_thickness = 0.0008;
    let on_v_centre = near(uv.x, 0.5, centre_thickness);
    let on_h_centre = near(uv.y, 0.5, centre_thickness);

    // Reference marks at 25% and 75% — slightly thinner so the
    // centre still reads as primary.
    let mark_thickness = 0.0004;
    let on_v_quarter = near(uv.x, 0.25, mark_thickness)
                    || near(uv.x, 0.75, mark_thickness);
    let on_h_quarter = near(uv.y, 0.25, mark_thickness)
                    || near(uv.y, 0.75, mark_thickness);

    // Edge frame so the operator can confirm full-frame coverage.
    let edge_thickness = 0.001;
    let on_edge =
        uv.x < edge_thickness || uv.x > (1.0 - edge_thickness)
        || uv.y < edge_thickness || uv.y > (1.0 - edge_thickness);

    // Brighter centre + edge; dimmer reference quarters so both
    // read clearly without competing.
    let centre_lit = on_v_centre || on_h_centre || on_edge;
    let quarter_lit = on_v_quarter || on_h_quarter;
    let intensity = select(
        select(0.0, 0.6, quarter_lit),
        0.95,
        centre_lit,
    );
    return vec4<f32>(intensity, intensity, intensity, 1.0);
}
