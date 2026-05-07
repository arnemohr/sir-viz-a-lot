// Editor-overlay pass: per-layer bounding rectangles + per-warp mask
// polygon outlines, painted on top of the projector swapchain after the
// gamma pass. CPU expands every line segment into a thin triangle strip
// so it shows up at projector resolution; the GPU just rasterises the
// strip in clip space and writes pre-multiplied RGBA.

struct VsIn {
    @location(0) pos_clip: vec2<f32>,
    @location(1) color:    vec4<f32>,
}

struct VsOut {
    @builtin(position) pos:   vec4<f32>,
    @location(0)       color: vec4<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var o: VsOut;
    o.pos   = vec4<f32>(in.pos_clip, 0.0, 1.0);
    o.color = in.color;
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Premultiplied RGBA: keeps the SrcAlpha/OneMinusSrcAlpha blend in
    // the host pipeline correct even when the swapchain is opaque.
    return vec4<f32>(in.color.rgb * in.color.a, in.color.a);
}
