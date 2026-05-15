// PCleanup.2.5a — `drift_pinholes` Treatment fragment shader.
//
// Sibling of `spotlights`: same particle SSBO, same compute pass — only the
// fragment math differs.  Where `spotlights` lifts source luminance under
// particles, `drift_pinholes` masks the source — pixels under a particle stay
// visible; everywhere else fades to black.  The effect looks like the source
// is glimpsed through drifting peepholes.
//
// At opacity == 0.0 the output is bit-exact to the source: the mix(src, masked,
// opacity) collapses to `src` regardless of particle positions.  The pinhole
// effect ramps in as opacity goes 0 → 1.
//
// build.rs prepends treatment_particles_helper.wgsl (for the Particle struct)
// before this file during naga validation; the runtime constructor concatenates
// the same source.
//
// Bind-group layout (fragment pass):
//   group 0, binding 0: t_source  (texture_2d<f32>, filterable)
//   group 0, binding 1: s_source  (sampler, filtering)
//   group 0, binding 2: u_params  (uniform, 32 bytes = DriftPinholesFragParams)
//   group 0, binding 7: particles (storage, read — array<Particle>)
//                         Slot 7 = particle SSBO for Treatment compute passes.

struct DriftPinholesFragParams {
    opacity:     f32,  // 0..=1.0, identity-default 0.0 → passthrough
    radius:      f32,  // 0.01..=0.3, normalised UV
    n_particles: f32,  // cast of u32 (1..=512)
    _pad0:       f32,
    _pad1:       f32,
    _pad2:       f32,
    _pad3:       f32,
    _pad4:       f32,
};

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_params: DriftPinholesFragParams;
// Slot 7: Treatment compute particle SSBO (read-only in fragment pass).
@group(0) @binding(7) var<storage, read> particles: array<Particle>;

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
    let src = textureSample(t_source, s_source, in.uv);

    let opacity = u_params.opacity;
    // opacity == 0 → mix(src, masked, 0.0) = src → bit-exact passthrough.
    // No early-exit needed; the branch is structural.

    let radius    = max(u_params.radius, 1e-4);
    let radius_sq = radius * radius;
    let n         = u32(u_params.n_particles);

    // Accumulate Gaussian weights from particles within `radius`.
    var weight_sum: f32 = 0.0;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let diff    = in.uv - particles[i].pos;
        let dist_sq = dot(diff, diff);
        if dist_sq < radius_sq {
            weight_sum = weight_sum + exp(-dist_sq / (2.0 * radius_sq));
        }
    }

    // Saturate to [0, 1] so dense particle clusters don't push the mask above
    // 1.0 and produce HDR-style ringing.
    let mask = clamp(weight_sum, 0.0, 1.0);

    // `masked`: source where particles are, black elsewhere.  Alpha is
    // preserved from the source so the layer's compositor can blend it.
    let masked = vec4<f32>(src.rgb * mask, src.a);

    // Crossfade between source (opacity=0) and masked (opacity=1).
    return mix(src, masked, opacity);
}
