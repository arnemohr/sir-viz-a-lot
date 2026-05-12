// P2.5.5 — mask_collision_reflection compute shader.
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_".
//
// Particles bounce elastically inside the mask. When a particle reaches the
// boundary (SDF >= 0), it reflects using the SDF normal as the surface
// normal and is pushed back inside. A `restitution` factor scales the speed
// after each bounce (1.0 = perfectly elastic; 0.5 = half energy retained).
//
// Architecture: approach (ii) — separate constructor `new_collision_reflection`
// with SDF texture at binding 6.
//
// FxParamsUniform field aliasing:
//   wavelength → particle_count  (1..=512, default 64)
//   speed      → speed           (0.01..=0.2 UV/s, default 0.08)
//   falloff    → restitution     (0.5..=1.0, default 0.95)
//
// Bind-group layout:
//   group 0, binding 2: FxParamsUniform (8 × f32)
//   group 0, binding 3: ClockUniform    (vec4<f32>)
//   group 0, binding 5: output SSBO (array<Particle>, read_write)
//   group 0, binding 6: SDF texture (texture_2d<f32>, R32Float)

struct FxParams {
    wavelength: f32,  // particle_count
    speed:      f32,  // initial speed (UV/s)
    falloff:    f32,  // restitution (0.5..=1.0)
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

// Random unit direction from two hash keys.
fn rand_dir(seed: u32, idx: u32) -> vec2<f32> {
    let angle = hash_f(seed, idx * 3u + 7u) * 6.28318530718;
    return vec2<f32>(cos(angle), sin(angle));
}

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
    let n   = min(u32(u_clock.w), 512u);
    if idx >= n { return; }

    let t_local     = u_clock.y;
    let seed_bits   = u32(u_clock.z);
    let dt          = 1.0 / 60.0;
    let spd         = u_params.speed;                          // UV/s
    let restitution = clamp(u_params.falloff, 0.5, 1.0);

    var p = particles[idx];

    // First-frame spawn: assign seeded random velocity at the given speed.
    if p.age_secs == 0.0 {
        p.pos      = find_interior_pos(seed_bits, idx);
        p.vel      = rand_dir(seed_bits, idx) * spd;
        p.age_secs = max(t_local, 0.001);
        p._pad     = 0.0;
        p._pad2    = 0.0;
        p._pad3    = 0.0;
        particles[idx] = p;
        return;
    }

    // Integrate.
    let new_pos = p.pos + p.vel * dt;
    let new_sdf = sample_sdf_bilinear(t_sdf, new_pos);

    if new_sdf >= 0.0 {
        // Collision: reflect velocity off the SDF normal at the boundary.
        let n_vec = sample_sdf_normal(t_sdf, p.pos);
        // v_reflected = v - 2 * dot(v, n) * n
        let vdotn = dot(p.vel, n_vec);
        p.vel = (p.vel - 2.0 * vdotn * n_vec) * restitution;

        // Push back to a safe interior position (step back along the normal).
        // Use the old position (which was inside) to stay valid.
        p.pos = p.pos - n_vec * 0.002;  // small backstep away from boundary

        // If still outside after backstep (degenerate geometry), respawn.
        if sample_sdf_bilinear(t_sdf, p.pos) >= 0.0 {
            p.pos = find_interior_pos(seed_bits ^ (idx * 1013904223u), idx);
            p.vel = rand_dir(seed_bits ^ (u32(t_local * 100.0) + idx), idx) * spd;
        }
    } else {
        p.pos = new_pos;
    }

    p.age_secs = t_local;
    particles[idx] = p;
}
