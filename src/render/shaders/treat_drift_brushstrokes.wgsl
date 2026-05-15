// PCleanup.2.5b — `drift_brushstrokes` Treatment fragment shader.
//
// Sibling of `drift_pinholes` and `spotlights` — same particle SSBO, same
// compute pass (which writes `vel` per particle).  The fragment shader reads
// each particle's velocity and renders an *elongated* Gaussian: a brushstroke
// that trails behind the particle along its motion vector.  Source is visible
// inside each stroke and fades to black outside, modulated by opacity.
//
// At opacity == 0.0 the output is bit-exact passthrough: `mix(src, brush, 0)`
// = src regardless of velocity values.
//
// Bind-group layout (fragment pass):
//   group 0, binding 0: t_source  (texture_2d<f32>, filterable)
//   group 0, binding 1: s_source  (sampler, filtering)
//   group 0, binding 2: u_params  (uniform, 32 bytes = DriftBrushstrokesParams)
//   group 0, binding 7: particles (storage, read — array<Particle>)
//                         Slot 7 = particle SSBO for Treatment compute passes.

struct DriftBrushstrokesParams {
    opacity:         f32,  // 0..=1.0, identity-default 0.0 → passthrough
    radius:          f32,  // 0.01..=0.3, brush thickness (UV)
    n_particles:     f32,  // cast of u32 (1..=512)
    smear_duration:  f32,  // 0..=2.0, seconds of motion → trail length
    _pad0:           f32,
    _pad1:           f32,
    _pad2:           f32,
    _pad3:           f32,
};

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_params: DriftBrushstrokesParams;
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

    let opacity        = u_params.opacity;
    let radius         = max(u_params.radius, 1e-4);
    let radius_sq      = radius * radius;
    let n              = u32(u_params.n_particles);
    let smear_duration = max(u_params.smear_duration, 0.0);

    // Accumulate weights from each particle's elongated Gaussian.  For each
    // particle: compute the closest point on the brushstroke (a line segment
    // trailing the particle along its velocity vector), then use that
    // distance in the Gaussian falloff.  Particles that haven't moved
    // (vel ≈ 0, e.g. just respawned) degrade to a circular Gaussian.
    var weight_sum: f32 = 0.0;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let particle = particles[i];
        let speed = length(particle.vel);
        let smear_length = speed * smear_duration;

        var dist_sq: f32;
        if smear_length < 1e-4 {
            // No motion → circular Gaussian (like drift_pinholes).
            let diff = in.uv - particle.pos;
            dist_sq = dot(diff, diff);
        } else {
            let smear_dir = particle.vel / speed;
            let diff = in.uv - particle.pos;
            // Project diff onto motion direction (signed).  Trail extends
            // BEHIND the particle (negative along), so we clamp to [-L, 0].
            let along = dot(diff, smear_dir);
            let along_clamped = clamp(along, -smear_length, 0.0);
            let closest = smear_dir * along_clamped;
            let perp_vec = diff - closest;
            dist_sq = dot(perp_vec, perp_vec);
        }

        if dist_sq < radius_sq {
            weight_sum = weight_sum + exp(-dist_sq / (2.0 * radius_sq));
        }
    }

    let mask = clamp(weight_sum, 0.0, 1.0);
    let masked = vec4<f32>(src.rgb * mask, src.a);

    return mix(src, masked, opacity);
}
