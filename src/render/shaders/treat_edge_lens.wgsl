// PCleanup.2.2 — `edge_lens` treatment.
//
// SourceModifier sibling of the generative `mask_edge_wave_wash` FX
// preset: takes the same angular-position phase function (atan2 of the
// SDF normal × N crests) and uses it as a UV-displacement amplitude.
// The result: N traveling refraction bumps that orbit the mask
// boundary, distorting the underlying photo at each crest.
//
// Phase function shared with fx_edge_wave_wash.wgsl:
//   normal = SDF gradient (points outward from the polygon)
//   phi    = atan2(normal.y, normal.x)         ∈ (-π, π]
//   wave   = sin(phi * n_waves - clock * speed * 2π)
//
// The wave oscillates between [-1, 1]; multiplying by `amplitude`
// gives the per-fragment displacement magnitude. Direction is the SDF
// normal (so the displacement is always radial relative to the mask).
//
// Identity-default rule: amplitude = 0.0 → disp = vec2(0) everywhere
// → output equals textureSample(t_source, s_source, uv) exactly.
//
// Params (packed into a single vec4):
//   x = amplitude    (0..=0.1, default 0.0)
//   y = n_waves      (1..=8, default 4)
//   z = speed        (0..=5 cycles/sec, default 1.0)
//   w = clock_secs   (written by the dispatcher each frame, not an
//                     operator-facing param — there's no slider for it)
//
// build.rs prepends `sdf_helper.wgsl` because this file's basename
// starts with `treat_edge_lens` (added to SDF_CONSUMERS in build.rs).

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_params: vec4<f32>; // x=amp, y=n_waves, z=speed, w=clock_secs
@group(0) @binding(3) var<uniform> u_fit:    vec4<f32>; // fit_mode, aspect, focal_x, focal_y
@group(0) @binding(4) var t_sdf:    texture_2d<f32>;    // R32Float, NonFiltering

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
    let amplitude  = u_params.x;
    let n_waves    = max(1.0, round(u_params.y));
    let speed      = u_params.z;
    let clock_secs = u_params.w;

    // Apply fit transform to UV (same logic as treat_displacement_ripple.wgsl).
    let mode   = i32(u_fit.x + 0.5);
    let aspect = max(u_fit.y, 1e-4);
    let focal  = vec2<f32>(u_fit.z, u_fit.w);
    var uv = in.uv;
    if (mode == 1) {
        // Cover
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
        // Contain
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

    // Compute the angular position around the mask via SDF normal.
    // The crest pattern travels around the boundary as clock advances.
    let normal = sample_sdf_normal(t_sdf, in.uv);
    let phi = atan2(normal.y, normal.x);
    let wave = sin(phi * n_waves - clock_secs * speed * 6.28318);

    // Displacement vector: SDF normal × amplitude × wave.
    // When amplitude = 0 → disp = vec2(0) everywhere → passthrough.
    let disp = normal * amplitude * wave;
    return textureSample(t_source, s_source, uv + disp);
}
