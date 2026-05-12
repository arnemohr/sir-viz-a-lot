// P2.5.1 — particle quad fragment shader.
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_".
// This shader does not read the SDF; the helper functions are present
// but unused.
//
// Returns a constant premultiplied white colour. Each particle appears
// as a solid white 2×2 px dot. Alpha is 1.0 so the dot is fully opaque
// when composited over the layer's fx_texture (which is cleared to
// transparent before the draw call).
//
// Phase 2 leaf presets (P2.5.2+) will replace this with coloured /
// faded fragments that read particle age and velocity from the SSBO.

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // Premultiplied white: (r*a, g*a, b*a, a) = (1, 1, 1, 1).
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
