// PCleanup.2.4 — `spotlights` Treatment fragment shader.
//
// Reads the particle SSBO written by treat_spotlights_compute.wgsl and
// accumulates a Gaussian luminance boost over source pixels that lie within
// `radius` of any particle.  Source is visible everywhere; particles only
// brighten locally.
//
// At brightness_gain == 0.0 the output is bit-exact to the source (the weight
// sum contributes 0.0 × anything = 0.0 lift, so multiplier = 1.0 everywhere).
//
// build.rs prepends treatment_particles_helper.wgsl (for the Particle struct)
// before this file during naga validation. At runtime, the pipeline constructor
// concatenates the same source.
//
// Bind-group layout (fragment pass):
//   group 0, binding 0: t_source      (texture_2d<f32>, filterable)
//   group 0, binding 1: s_source      (sampler, filtering)
//   group 0, binding 2: u_params      (uniform, 32 bytes = SpotlightsFragParams)
//   group 0, binding 7: particles     (storage, read — array<Particle>)
//                         Slot 7 = particle SSBO for Treatment compute passes.
//                         See TreatmentInputs doc-comment block in treatments.rs.

struct SpotlightsFragParams {
    brightness_gain: f32,  // 0..=2.0
    radius:          f32,  // 0.01..=0.3, normalised UV
    n_particles:     f32,  // cast of u32 particle count (1..=512)
    _pad0:           f32,
    _pad1:           f32,
    _pad2:           f32,
    _pad3:           f32,
    _pad4:           f32,
};

@group(0) @binding(0) var t_source:   texture_2d<f32>;
@group(0) @binding(1) var s_source:   sampler;
@group(0) @binding(2) var<uniform>  u_params:  SpotlightsFragParams;
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

    let brightness_gain = u_params.brightness_gain;
    // brightness_gain == 0 → weight_sum contributes 0 → multiplier = 1.0
    // → bit-exact passthrough. No early-exit needed; the branch is structural.

    let radius    = max(u_params.radius, 1e-4);
    let radius_sq = radius * radius;
    let n         = u32(u_params.n_particles);

    var weight_sum: f32 = 0.0;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let diff    = in.uv - particles[i].pos;
        let dist_sq = dot(diff, diff);
        if dist_sq < radius_sq {
            // Gaussian: exp(-dist² / (2 × radius²))
            weight_sum = weight_sum + exp(-dist_sq / (2.0 * radius_sq));
        }
    }

    // Clamp weight_sum so an operator cranking brightness_gain very high
    // doesn't blow out HDR-capable targets.  Values up to 1.0 per particle
    // can pile up, so clamp at n to avoid run-away.
    weight_sum = min(weight_sum, f32(n));

    // Luminance multiplier: 1.0 at gain=0 (passthrough), up to 1+2×N at max.
    let multiplier = 1.0 + brightness_gain * weight_sum;

    return vec4<f32>(src.rgb * multiplier, src.a);
}
