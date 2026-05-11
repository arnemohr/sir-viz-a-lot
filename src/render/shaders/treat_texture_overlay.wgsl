// P1.3.4 — `texture_overlay` treatment.
//
// Samples a secondary texture (from `Treatment.overlay_path`) and
// composites it over the layer's source with one of four blend modes
// selected by an integer-coded `blend_mode` param. The `mix` slider
// fades the effect (0 = pure source, 1 = full overlay), and
// `offset_x` / `offset_y` shift the overlay in normalised UV space so
// the operator can place the overlay where they want.
//
// Bind layout:
//   group 0 / binding 0   source texture (post-fit)
//   group 0 / binding 1   filtering sampler
//   group 0 / binding 2   fit_uniform (16 bytes)
//   group 0 / binding 3   params: vec4<f32> (mix, offset_x, offset_y, blend_mode_encoded)
//   group 0 / binding 4   overlay texture
//   group 0 / binding 5   filtering sampler for the overlay
//
// blend_mode encoding:
//   0.0 — Normal   (mix(src, overlay, mix_amt))
//   1.0 — Multiply (mix(src, src * overlay, mix_amt))
//   2.0 — Screen   (mix(src, 1 - (1-src)*(1-overlay), mix_amt))
//   3.0 — Add      (mix(src, src + overlay, mix_amt))
//
// Identity at default (`mix = 0`) — operator sees source unchanged
// until they reach for the mix slider.

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_fit: vec4<f32>;
@group(0) @binding(3) var<uniform> u_params: vec4<f32>;
@group(0) @binding(4) var t_overlay: texture_2d<f32>;
@group(0) @binding(5) var s_overlay: sampler;

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
    // Fit-mode crop for the SOURCE only — the overlay samples in its
    // own native UV space (so a tileable pattern overlay isn't
    // squashed to match the layer aspect).
    let mode   = i32(u_fit.x + 0.5);
    let aspect = max(u_fit.y, 1e-4);
    let focal  = vec2<f32>(u_fit.z, u_fit.w);
    var src_uv = in.uv;

    if (mode == 1) {
        if (aspect > 1.0) {
            let scale = 1.0 / aspect;
            src_uv.x = (src_uv.x - 0.5) * scale + focal.x;
        } else {
            let scale = aspect;
            src_uv.y = (src_uv.y - 0.5) * scale + focal.y;
        }
        if (src_uv.x < 0.0 || src_uv.x > 1.0 || src_uv.y < 0.0 || src_uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    } else if (mode == 2) {
        if (aspect > 1.0) {
            let scale = aspect;
            src_uv.y = (src_uv.y - 0.5) * scale + 0.5;
        } else {
            let scale = 1.0 / aspect;
            src_uv.x = (src_uv.x - 0.5) * scale + 0.5;
        }
        if (src_uv.x < 0.0 || src_uv.x > 1.0 || src_uv.y < 0.0 || src_uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }

    let src = textureSample(t_source, s_source, src_uv);

    let mix_amt   = clamp(u_params.x, 0.0, 1.0);
    let offset    = vec2<f32>(u_params.y, u_params.z);
    let blend_id  = i32(u_params.w + 0.5);

    // Overlay UV: wrap via fract() so the sampler's RepeatX/Y mode
    // tiles a small overlay across the full layer.
    let overlay_uv = fract(in.uv + offset);
    let ov = textureSample(t_overlay, s_overlay, overlay_uv);

    var blended: vec3<f32>;
    if (blend_id == 1) {
        blended = src.rgb * ov.rgb;
    } else if (blend_id == 2) {
        blended = vec3<f32>(1.0) - (vec3<f32>(1.0) - src.rgb) * (vec3<f32>(1.0) - ov.rgb);
    } else if (blend_id == 3) {
        blended = src.rgb + ov.rgb;
    } else {
        // 0 (Normal) — straight overlay
        blended = ov.rgb;
    }

    let out_rgb = mix(src.rgb, blended, mix_amt * ov.a);
    return vec4<f32>(out_rgb, src.a);
}
