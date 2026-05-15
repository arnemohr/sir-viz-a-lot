// PCleanup.2.11 — `portal_warp` Treatment fragment shader.
//
// Sibling of `fx_zone_portal_drift`.  Particles drift through the mask
// (compute pass is the shared spotlights compute shader); the fragment
// pass displaces source UVs toward each nearby particle by a Gaussian
// magnitude.  The visual is a "ghost through the room" — source pixels
// near a particle smear toward it, producing a soft warp that travels
// with the drifting particles.
//
// At `amplitude = 0.0` the output is bit-exact passthrough: the
// accumulated displacement is zero so the fragment samples the source
// at the unmodified UV.
//
// Bind-group layout (fragment pass) — matches the shared particle render BGL:
//   group 0, binding 0: t_source  (texture_2d<f32>, filterable)
//   group 0, binding 1: s_source  (sampler, filtering)
//   group 0, binding 2: u_params  (uniform, 32 bytes = PortalWarpFragParams)
//   group 0, binding 7: particles (storage, read — array<Particle>)

struct PortalWarpFragParams {
    amplitude:   f32,  // 0..=0.05, peak UV displacement at the particle
    radius:      f32,  // 0.01..=0.3, falloff radius (UV)
    n_particles: f32,  // u32 cast (1..=512)
    pull:        f32,  // -1..=+1 — sign selects pull (toward) vs push (away)
    _pad0:       f32,
    _pad1:       f32,
    _pad2:       f32,
    _pad3:       f32,
};

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_params: PortalWarpFragParams;
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
    let amplitude = u_params.amplitude;
    let radius    = max(u_params.radius, 1e-4);
    let radius_sq = radius * radius;
    let n         = u32(u_params.n_particles);
    // Positive `pull` smears toward the particle, negative away.  Clamp to
    // [-1, 1] so the operator can't accidentally amplify beyond the radius.
    let direction_sign = clamp(u_params.pull, -1.0, 1.0);

    var displacement = vec2<f32>(0.0, 0.0);

    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let p = particles[i];
        let to_particle = p.pos - in.uv;
        let dist_sq = dot(to_particle, to_particle);
        if dist_sq >= radius_sq { continue; }
        let dist = sqrt(dist_sq);
        if dist < 1e-5 { continue; }
        let dir = to_particle / dist;
        // Gaussian falloff: 1.0 at particle, → 0 at radius.
        let weight = exp(-dist_sq / (2.0 * radius_sq));
        displacement = displacement + dir * weight * amplitude * direction_sign;
    }

    let warped_uv = in.uv + displacement;
    return textureSample(t_source, s_source, warped_uv);
}
