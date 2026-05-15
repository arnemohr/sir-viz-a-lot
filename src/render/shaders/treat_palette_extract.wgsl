// P1.3.5 — `palette_extract` treatment.
//
// **Implementation note:** the Phase 1 spec calls for k-means palette
// extraction on a downsampled source. v0.5 ships a simpler bit-depth
// posterization that hits the same operator-visible effect (a clip
// rendered with a reduced colour palette) without the CPU-side
// quantization infrastructure. The visible effect for natural images
// is comparable; true palette-extraction (with operator-named palette
// presets) lands in Phase 7 alongside collage's multi-input plumbing.
//
// Params (array<vec4<f32>, 2> = 32-byte uniform, PCleanup.8.3a expanded):
//   params[0].x  levels         (1..=8, default 4)  — quantization levels per channel
//   params[0].y  mix            (0..=1, default 0)   — crossfade between source and posterized
//   params[0].z  dither         (0..=1, default 0)   — ordered 4×4 Bayer dither amount
//   params[0].w  zone_mode      (0..=2, default 0)   — zone-aware mode:
//                                  0 = ignore_zone   (current behaviour; no zone awareness)
//                                  1 = strict_zone   (posterise on ZONE_WINDOW layers; others pass through)
//                                  2 = dual_quant    (ZONE_WINDOW layers use `levels`;
//                                                     non-ZONE_WINDOW layers use `outside_levels`)
//   params[1].x  outside_levels (1..=8, default 4)  — quantization levels for non-ZONE_WINDOW layers
//                                                      (only meaningful at zone_mode=2)
//   params[1].yzw _pad
//
// Identity at default (`mix=0`) — operator sees source unchanged until
// they reach for the mix slider.
//
// Zone mode identity: `zone_mode=0` reproduces the pre-8.3a output exactly
// for all existing projects (ignore_zone = same as before).
//
// build.rs prepends zone_tag_helper.wgsl via the ZONE_ONLY_CONSUMERS list
// (prefix "treat_palette_extract"). The ZoneTagUniform is at binding 6
// following the P3.3.2 slot contract for zone-aware treatments.

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(0) @binding(2) var<uniform> u_fit: vec4<f32>;
@group(0) @binding(3) var<uniform> u_params: array<vec4<f32>, 2>;
// bindings 4, 5 intentionally absent (reserved)
// P3.3.2 — slot 6: zone tag uniform (zone-aware treatments only).
@group(0) @binding(6) var<uniform> u_zone: ZoneTagUniform;

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

// 4×4 Bayer matrix, normalised to [0, 1).
fn bayer(p: vec2<i32>) -> f32 {
    let m = array<array<i32, 4>, 4>(
        array<i32, 4>( 0,  8,  2, 10),
        array<i32, 4>(12,  4, 14,  6),
        array<i32, 4>( 3, 11,  1,  9),
        array<i32, 4>(15,  7, 13,  5),
    );
    let x = (p.x % 4 + 4) % 4;
    let y = (p.y % 4 + 4) % 4;
    return f32(m[y][x]) / 16.0;
}

// Posterize `rgb` to `n_levels` quantization levels with Bayer dither
// of amount `dither_amt`. Returns clamped quantized RGB.
fn posterize(rgb: vec3<f32>, n_levels: f32, dither_amt: f32, frag: vec2<i32>) -> vec3<f32> {
    let bias = (bayer(frag) - 0.5) / n_levels * dither_amt;
    let q = vec3<f32>(
        floor((rgb.r + bias) * (n_levels - 1.0) + 0.5) / (n_levels - 1.0),
        floor((rgb.g + bias) * (n_levels - 1.0) + 0.5) / (n_levels - 1.0),
        floor((rgb.b + bias) * (n_levels - 1.0) + 0.5) / (n_levels - 1.0),
    );
    return clamp(q, vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let fit_mode = i32(u_fit.x + 0.5);
    let aspect   = max(u_fit.y, 1e-4);
    let focal    = vec2<f32>(u_fit.z, u_fit.w);
    var uv = in.uv;

    if (fit_mode == 1) {
        if (aspect > 1.0) {
            let scale = 1.0 / aspect;
            uv.x = (uv.x - 0.5) * scale + focal.x;
        } else {
            let scale = aspect;
            uv.y = (uv.y - 0.5) * scale + focal.y;
        }
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    } else if (fit_mode == 2) {
        if (aspect > 1.0) {
            let scale = aspect;
            uv.y = (uv.y - 0.5) * scale + 0.5;
        } else {
            let scale = 1.0 / aspect;
            uv.x = (uv.x - 0.5) * scale + 0.5;
        }
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }

    let src = textureSample(t_diffuse, s_diffuse, uv);

    let levels         = clamp(u_params[0].x, 1.0, 8.0);
    let mix_amt        = clamp(u_params[0].y, 0.0, 1.0);
    let dither         = clamp(u_params[0].z, 0.0, 1.0);
    let zone_mode      = i32(u_params[0].w + 0.5);
    let outside_levels = clamp(u_params[1].x, 1.0, 8.0);

    let dims = vec2<f32>(textureDimensions(t_diffuse, 0));
    let frag = vec2<i32>(in.uv * dims);

    // Determine effective quantisation levels for this layer based on
    // zone_mode and the layer's zone tag.
    var out_rgb: vec3<f32>;

    if (zone_mode == 0) {
        // ignore_zone (default): apply standard posterise regardless of zone.
        // Exactly reproduces pre-8.3a behaviour — no zone awareness.
        let q = posterize(src.rgb, levels, dither, frag);
        out_rgb = mix(src.rgb, q, mix_amt);
    } else if (zone_mode == 1) {
        // strict_zone: posterise only on ZONE_WINDOW layers; all others
        // pass the source through unchanged (mix bypassed).
        if u_zone.zone_tag == ZONE_WINDOW {
            let q = posterize(src.rgb, levels, dither, frag);
            out_rgb = mix(src.rgb, q, mix_amt);
        } else {
            out_rgb = src.rgb;
        }
    } else {
        // dual_quant (zone_mode == 2): ZONE_WINDOW layers use `levels`;
        // non-ZONE_WINDOW layers use `outside_levels`. This lets the
        // operator dial in a coarser or finer quantisation for the window
        // region vs. the surrounding layers when multiple layers carry the
        // palette_extract treatment with different zone roles.
        let active_levels = select(outside_levels, levels, u_zone.zone_tag == ZONE_WINDOW);
        let q = posterize(src.rgb, active_levels, dither, frag);
        out_rgb = mix(src.rgb, q, mix_amt);
    }

    return vec4<f32>(out_rgb, src.a);
}
