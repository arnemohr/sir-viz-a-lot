// P2.5.2 — mask_constrained_drift compute shader.
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_".
//
// Particles drift slowly in a seed-derived random direction inside the mask.
// When a particle crosses the mask boundary (SDF >= 0: positive-outside
// convention used throughout this codebase), it respawns at a seeded
// interior location.
//
// Architecture: approach (ii) — separate constructor `new_constrained_drift`
// with its own bind-group layout that adds the SDF texture at binding 6.
// The identity preset layout (bindings 2, 3, 5) is unchanged.
//
// FxParamsUniform field aliasing:
//   wavelength → particle_count  (1..=2048, default 256)
//   speed      → drift_speed     (0.0..=0.05 UV/s, default 0.02)
//   falloff    → particle_size   (0.5..=4.0 px, default 2.0; read by vertex shader)
//
// Bind-group layout:
//   group 0, binding 2: FxParamsUniform (8 × f32)
//   group 0, binding 3: ClockUniform    (vec4<f32>)
//                         .x = clock_secs
//                         .y = t_layer_local_secs
//                         .z = seed_f32  (lower 23 bits of u64 seed)
//                         .w = n_particles
//   group 0, binding 5: output SSBO (array<Particle>, read_write)
//   group 0, binding 6: SDF texture (texture_2d<f32>, R32Float)

struct FxParams {
    wavelength: f32,  // particle_count
    speed:      f32,  // drift_speed (UV/s)
    falloff:    f32,  // particle_size (px)
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

// Simple deterministic hash: two u32 keys → f32 in [0, 1).
fn hash_f(a: u32, b: u32) -> f32 {
    var x: u32 = a ^ (b * 2654435761u);
    x = (x ^ (x >> 16u)) * 0x45d9f3bu;
    x = x ^ (x >> 16u);
    return f32(x & 0x7fffffu) / f32(0x800000u);
}

// Random unit direction from seed + particle index.
fn rand_dir(seed: u32, idx: u32) -> vec2<f32> {
    let angle = hash_f(seed, idx * 3u + 7u) * 6.28318530718;
    return vec2<f32>(cos(angle), sin(angle));
}

// Find a spawn position inside the mask (SDF < 0).
// Tries 16 hash-derived candidates; returns the first inside point.
// Falls back to (0.5, 0.5) if all candidates are outside.
fn find_interior_pos(seed: u32, idx: u32) -> vec2<f32> {
    for (var attempt: u32 = 0u; attempt < 16u; attempt = attempt + 1u) {
        let hx = hash_f(seed + attempt * 17u, idx * 5u + 1u);
        let hy = hash_f(seed + attempt * 19u, idx * 5u + 3u);
        let candidate = vec2<f32>(hx, hy);
        if sample_sdf_bilinear(t_sdf, candidate) < 0.0 {
            return candidate;
        }
    }
    return vec2<f32>(0.5, 0.5);
}

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let n   = min(u32(u_clock.w), 2048u);
    if idx >= n { return; }

    let t_local     = u_clock.y;
    let seed_bits   = u32(u_clock.z);
    let dt          = 1.0 / 60.0;         // fixed timestep ~16 ms
    let drift_speed = u_params.speed;     // UV/s

    var p = particles[idx];

    // First-frame spawn: age_secs == 0.0 means uninitialised buffer.
    if p.age_secs == 0.0 {
        p.pos      = find_interior_pos(seed_bits, idx);
        p.vel      = rand_dir(seed_bits, idx);
        p.age_secs = max(t_local, 0.001);
        p._pad     = 0.0;
        p._pad2    = 0.0;
        p._pad3    = 0.0;
        particles[idx] = p;
        return;
    }

    // Integrate: drift in seeded random direction.
    let dir      = rand_dir(seed_bits, idx);
    let new_pos  = p.pos + dir * drift_speed * dt;

    // Outside mask (SDF >= 0) → respawn inside using a time-varied seed.
    if sample_sdf_bilinear(t_sdf, new_pos) >= 0.0 {
        let respawn_seed = seed_bits ^ (u32(t_local * 1000.0) + idx * 31u);
        p.pos      = find_interior_pos(respawn_seed, idx);
        p.vel      = rand_dir(respawn_seed, idx);
        p.age_secs = t_local;
    } else {
        p.pos      = new_pos;
        p.age_secs = t_local;
    }

    particles[idx] = p;
}
