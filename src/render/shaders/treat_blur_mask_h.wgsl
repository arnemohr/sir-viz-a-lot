// P1.3.2 — `blur_mask` treatment, horizontal pass.
//
// Separable gaussian blur with SDF-gated per-fragment radius. The
// horizontal pass reads a fit-applied source (already written into the
// effect chain's `src_view` by the upstream fit pass) and writes its
// horizontally-blurred result into the layer's `intermediate_view`.
// The vertical pass then reads `intermediate_view` and writes the final
// result back to `src_view` so downstream effects/warp consume it
// unchanged.
//
// Radius derivation (mode 0 — edge-band, current default):
//   `r = max_radius * shape`, where `shape` is a smoothstepped
//   distance-from-edge curve.  The SDF returns negative inside the
//   polygon, positive outside, zero on edge — `abs(d)` gives "distance
//   from edge" in normalised units regardless of side. Blurry at edge,
//   sharp far away.
//
// Radius derivation (mode 1 — distance-driven, PCleanup.8.3c):
//   `r = base_radius * smoothstep(0, distance_falloff, abs(sdf))`.
//   Sharp at the mask edge (abs(sdf)=0 → r=0), blurry toward the interior
//   (abs(sdf)→distance_falloff → r=base_radius). Inverse of mode 0.
//
// PCleanup.8.3c adds `radius_mode` (u_params.w, default 0) and
// `distance_falloff` (u_params[1].x) to the 32-byte uniform.
//
// build.rs prepends `sdf_helper.wgsl` because this file's basename
// starts with `treat_blur` (see SDF_CONSUMERS in build.rs).

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(0) @binding(2) var<uniform> u_params: array<vec4<f32>, 2>;
@group(0) @binding(3) var t_sdf: texture_2d<f32>;

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
    let max_radius_px    = max(u_params[0].x, 0.0);
    let edge_band        = max(u_params[0].y, 1e-4);
    let falloff          = clamp(u_params[0].z, 0.0, 1.0);
    let radius_mode      = i32(u_params[0].w + 0.5);  // 0=edge-band (default), 1=distance-driven
    let distance_falloff = max(u_params[1].x, 1e-4);  // meaningful at radius_mode=1

    // Unsigned distance from mask edge.
    let d = abs(sample_sdf_bilinear(t_sdf, in.uv));

    var r: f32;
    if (radius_mode == 1) {
        // Distance-driven: sharp at edge, blurry toward the interior.
        // r = 0 at the edge (d=0), rises to max_radius_px at d=distance_falloff.
        r = max_radius_px * smoothstep(0.0, distance_falloff, d);
    } else {
        // Edge-band (mode 0, current default behaviour — preserved exactly):
        // 1.0 at edge → 0.0 deep inside / outside.
        let proximity = 1.0 - smoothstep(0.0, edge_band, d);
        // falloff = 0 → very steep (hard cutoff at edge_band)
        // falloff = 1 → smooth gradient
        let shape = pow(proximity, mix(8.0, 1.0, falloff));
        r = max_radius_px * shape;
    }

    let dims = textureDimensions(t_diffuse, 0);
    let texel_x = 1.0 / f32(dims.x);

    // Clamp the kernel half-width to 32 pixels (same bound as the
    // existing BlurPipeline) for a static loop.
    let radius = clamp(r, 0.0, 32.0);
    let r_int = i32(round(radius));
    let sigma = max(radius * 0.5, 0.5);
    let two_sigma_sq = 2.0 * sigma * sigma;

    var color = vec4<f32>(0.0);
    var weight_sum = 0.0;

    for (var i = -32; i <= 32; i = i + 1) {
        if (abs(i) > r_int) {
            continue;
        }
        let offset = vec2<f32>(f32(i) * texel_x, 0.0);
        let weight = exp(-f32(i * i) / two_sigma_sq);
        color = color + weight * textureSample(t_diffuse, s_diffuse, in.uv + offset);
        weight_sum = weight_sum + weight;
    }

    return color / max(weight_sum, 1e-6);
}
