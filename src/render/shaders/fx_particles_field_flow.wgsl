// P2.5.4 — mask_field_flow compute shader.
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_".
//
// Particles move according to the SDF gradient field. The gradient points
// outward from the mask interior (away from the nearest edge). `flow_direction`
// controls whether particles move toward the boundary (+1) or away from it (-1).
// Particles that leave the valid SDF domain are respawned inside the mask.
//
// Architecture: approach (ii) — separate constructor `new_field_flow`
// with SDF texture at binding 6.
//
// FxParamsUniform field aliasing:
//   wavelength → particle_count   (1..=2048, default 256)
//   speed      → flow_speed       (0.0..=0.1 UV/s, default 0.03)
//   falloff    → flow_direction   (-1.0=inward, +1.0=outward, default 1.0)
//
// Bind-group layout:
//   group 0, binding 2: FxParamsUniform (8 × f32)
//   group 0, binding 3: ClockUniform    (vec4<f32>)
//   group 0, binding 5: output SSBO (array<Particle>, read_write)
//   group 0, binding 6: SDF texture (texture_2d<f32>, R32Float)

struct FxParams {
    wavelength: f32,  // particle_count
    speed:      f32,  // flow_speed (UV/s)
    falloff:    f32,  // flow_direction (-1..=+1)
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

    let t_local        = u_clock.y;
    let seed_bits      = u32(u_clock.z);
    let dt             = 1.0 / 60.0;
    let flow_speed     = u_params.speed;                          // UV/s
    let flow_direction = clamp(u_params.falloff, -1.0, 1.0);     // -1=inward, +1=outward

    var p = particles[idx];

    // First-frame spawn.
    if p.age_secs == 0.0 {
        p.pos      = find_interior_pos(seed_bits, idx);
        p.vel      = vec2<f32>(0.0, 0.0);
        p.age_secs = max(t_local, 0.001);
        p._pad     = 0.0;
        p._pad2    = 0.0;
        p._pad3    = 0.0;
        particles[idx] = p;
        return;
    }

    // Velocity = gradient * flow_direction * flow_speed.
    // Gradient points outward from the nearest edge.
    let gradient = sample_sdf_gradient(t_sdf, p.pos);
    let velocity = gradient * flow_direction * flow_speed;

    let new_pos = p.pos + velocity * dt;

    // Respawn if outside mask (SDF >= 0) or out of [0,1]² domain.
    let new_sdf = sample_sdf_bilinear(t_sdf, new_pos);
    if new_sdf >= 0.0 || new_pos.x < 0.0 || new_pos.x > 1.0 || new_pos.y < 0.0 || new_pos.y > 1.0 {
        let respawn_seed = seed_bits ^ (u32(t_local * 1000.0) + idx * 31u);
        p.pos      = find_interior_pos(respawn_seed, idx);
        p.vel      = vec2<f32>(0.0, 0.0);
        p.age_secs = t_local;
    } else {
        p.pos      = new_pos;
        p.vel      = velocity;
        p.age_secs = t_local;
    }

    particles[idx] = p;
}
