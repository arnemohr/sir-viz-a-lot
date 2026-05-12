// P2.5.3 — mask_edge_emission compute shader.
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_".
//
// Particles spawn along the mask edge (SDF ≈ 0) and travel outward in the
// direction of the SDF normal. They age out after `lifetime_secs` and
// respawn at a new edge position.
//
// Architecture: approach (ii) — separate constructor `new_edge_emission`
// with SDF texture at binding 6.
//
// FxParamsUniform field aliasing:
//   wavelength → particle_count   (1..=1024, default 128)
//   speed      → emission_speed   (0.01..=0.15 UV/s, default 0.05)
//   falloff    → lifetime_secs    (0.5..=5.0, default 2.0)
//
// Bind-group layout:
//   group 0, binding 2: FxParamsUniform (8 × f32)
//   group 0, binding 3: ClockUniform    (vec4<f32>)
//   group 0, binding 5: output SSBO (array<Particle>, read_write)
//   group 0, binding 6: SDF texture (texture_2d<f32>, R32Float)

struct FxParams {
    wavelength: f32,  // particle_count
    speed:      f32,  // emission_speed (UV/s)
    falloff:    f32,  // lifetime_secs
    base_r:     f32,
    base_g:     f32,
    base_b:     f32,
    _pad0:      f32,
    _pad1:      f32,
};

@group(0) @binding(2) var<uniform>             u_params  : FxParams;
@group(0) @binding(3) var<uniform>             u_clock   : vec4<f32>;

struct Particle {
    pos:      vec2<f32>,
    vel:      vec2<f32>,
    age_secs: f32,
    _pad:     f32,
    _pad2:    f32,
    _pad3:    f32,
};

@group(0) @binding(5) var<storage, read_write> particles : array<Particle>;
@group(0) @binding(6) var                      t_sdf     : texture_2d<f32>;

fn hash_f(a: u32, b: u32) -> f32 {
    var x: u32 = a ^ (b * 2654435761u);
    x = (x ^ (x >> 16u)) * 0x45d9f3bu;
    x = x ^ (x >> 16u);
    return f32(x & 0x7fffffu) / f32(0x800000u);
}

// Find a spawn position near the mask edge (|SDF| close to 0).
// Strategy: sample a grid of candidates, pick the one with SDF closest to 0
// that is still inside (SDF < 0). Falls back to (0.5, 0.5) if none found.
fn find_edge_pos(seed: u32, idx: u32) -> vec2<f32> {
    var best_pos   = vec2<f32>(0.5, 0.5);
    var best_dist  = 1e9;

    for (var attempt: u32 = 0u; attempt < 24u; attempt = attempt + 1u) {
        let hx = hash_f(seed + attempt * 13u, idx * 7u + 2u);
        let hy = hash_f(seed + attempt * 17u, idx * 7u + 4u);
        let candidate = vec2<f32>(hx, hy);
        let d = sample_sdf_bilinear(t_sdf, candidate);
        // Prefer positions just inside the edge (SDF in [-0.05, 0))
        if d < 0.0 && abs(d) < best_dist {
            best_dist = abs(d);
            best_pos  = candidate;
        }
    }
    return best_pos;
}

// Spawn a particle at the edge with outward velocity.
fn spawn_at_edge(seed: u32, idx: u32, t_local: f32, emission_speed: f32) -> Particle {
    var p: Particle;
    p.pos = find_edge_pos(seed, idx);
    // SDF normal points away from edge (outward from mask interior).
    let normal = sample_sdf_normal(t_sdf, p.pos);
    // Emit outward along the normal.
    p.vel      = normal * emission_speed;
    p.age_secs = t_local;
    p._pad     = 0.0;
    p._pad2    = 0.0;
    p._pad3    = 0.0;
    return p;
}

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let n   = min(u32(u_clock.w), 1024u);
    if idx >= n { return; }

    let t_local       = u_clock.y;
    let seed_bits     = u32(u_clock.z);
    let dt            = 1.0 / 60.0;
    let emission_speed = u_params.speed;   // UV/s
    let lifetime      = max(u_params.falloff, 0.5);  // secs

    var p = particles[idx];

    // First-frame spawn (uninitialised).
    if p.age_secs == 0.0 {
        // Stagger birth times so particles don't all die together.
        let stagger = hash_f(seed_bits, idx * 11u) * lifetime;
        let seed_i  = seed_bits ^ (idx * 97u);
        p = spawn_at_edge(seed_i, idx, t_local - stagger, emission_speed);
        p.age_secs = max(p.age_secs, 0.001);
        particles[idx] = p;
        return;
    }

    // Age out and respawn.
    let particle_age = t_local - p.age_secs;
    if particle_age >= lifetime {
        let respawn_seed = seed_bits ^ (u32(t_local * 500.0) * 1013904223u + idx * 1664525u);
        p = spawn_at_edge(respawn_seed, idx, t_local, emission_speed);
        particles[idx] = p;
        return;
    }

    // Integrate: move outward; slight deceleration over lifetime.
    let frac    = clamp(particle_age / lifetime, 0.0, 1.0);
    let vel_scale = 1.0 - frac * 0.5;   // decelerate to 50% by end of life
    p.pos = p.pos + p.vel * vel_scale * dt;

    particles[idx] = p;
}
