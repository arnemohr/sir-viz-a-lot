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
// Three params:
//   levels  (1..=8, default 4) — quantization levels per channel
//   mix     (0..=1, default 0) — crossfade between source and posterized
//   dither  (0..=1, default 0) — ordered 4×4 Bayer dither amount
//
// Identity at default (`mix=0`) — operator sees source unchanged until
// they reach for the mix slider.

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(0) @binding(2) var<uniform> u_fit: vec4<f32>;
@group(0) @binding(3) var<uniform> u_params: vec4<f32>;

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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let mode   = i32(u_fit.x + 0.5);
    let aspect = max(u_fit.y, 1e-4);
    let focal  = vec2<f32>(u_fit.z, u_fit.w);
    var uv = in.uv;

    if (mode == 1) {
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
    } else if (mode == 2) {
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

    let levels = clamp(u_params.x, 1.0, 8.0);
    let mix_amt = clamp(u_params.y, 0.0, 1.0);
    let dither = clamp(u_params.z, 0.0, 1.0);

    // Per-channel quantize: scale into [0, levels-1], add Bayer-dithered
    // noise (so smooth gradients band less), round, divide back.
    let dims = vec2<f32>(textureDimensions(t_diffuse, 0));
    let frag = vec2<i32>(in.uv * dims);
    let bias = (bayer(frag) - 0.5) / levels * dither;

    let q = vec3<f32>(
        floor((src.r + bias) * (levels - 1.0) + 0.5) / (levels - 1.0),
        floor((src.g + bias) * (levels - 1.0) + 0.5) / (levels - 1.0),
        floor((src.b + bias) * (levels - 1.0) + 0.5) / (levels - 1.0),
    );

    let out_rgb = mix(src.rgb, clamp(q, vec3<f32>(0.0), vec3<f32>(1.0)), mix_amt);
    return vec4<f32>(out_rgb, src.a);
}
