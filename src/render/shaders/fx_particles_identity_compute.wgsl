// P2.5.1 — particles_identity compute shader.
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_".
// This shader does NOT read the SDF; the helper functions are present
// but unused. Future particle presets (P2.5.2+) may read the SDF to
// confine particles inside the mask.
//
// Each invocation writes one particle into the output SSBO.  Positions
// form a regular sqrt(n) × sqrt(n) grid centred at (0.5, 0.5) in
// normalised [0, 1]² space, covering a 0.4-wide square.  Velocity is
// always zero (identity preset).  age_secs is set to t_layer_local_secs
// so every particle appears "alive" from the moment the layer is added.
//
// The `seed` value offsets the anchor positions deterministically:
// changing `seed` shuffles which column/row offset is added, giving
// the operator a knob to produce different grid arrangements without
// physics.
//
// FxParamsUniform field aliasing for particles_identity:
//   wavelength → particle_count (1..=16, default 16)
//   All other fields unused (kept at 0).
//
// Bind-group layout (canonical P2.5.1 slots):
//   group 0, binding 2: FxParamsUniform (8 × f32)
//   group 0, binding 3: ClockUniform    (vec4<f32>, .x = clock_secs,
//                                        .y = t_layer_local_secs,
//                                        .z = seed_f32 (lower 23 bits),
//                                        .w = n_particles)
//   group 0, binding 5: output SSBO     (array<Particle>)

struct FxParams {
    wavelength: f32,  // aliased: particle_count (clamped to 1..=16)
    speed:      f32,
    falloff:    f32,
    base_r:     f32,
    base_g:     f32,
    base_b:     f32,
    _pad0:      f32,
    _pad1:      f32,
};

// ClockUniform layout:
//   .x = clock_secs         (absolute project clock)
//   .y = t_layer_local_secs (clock_secs - t_layer_added_secs)
//   .z = seed_f32           (lower 23 bits of seed cast to f32)
//   .w = n_particles        (f32 of u32 n_particles)
@group(0) @binding(2) var<uniform>          u_params : FxParams;
@group(0) @binding(3) var<uniform>          u_clock  : vec4<f32>;

struct Particle {
    pos:      vec2<f32>,   // normalised [0, 1]²
    vel:      vec2<f32>,   // always zero for identity
    age_secs: f32,
    _pad:     f32,         // 24 bytes; stride rounds to 32 for std430
    // Two additional f32 padding words to reach 32-byte stride
    _pad2:    f32,
    _pad3:    f32,
};

@group(0) @binding(5) var<storage, read_write> particles: array<Particle>;

// Compute a deterministic float in [0, 1) from two u32 keys (simple
// hash — not cryptographic, sufficient for deterministic layout).
fn hash_f(a: u32, b: u32) -> f32 {
    var x: u32 = a ^ (b * 2654435761u);
    x = (x ^ (x >> 16u)) * 0x45d9f3bu;
    x = (x ^ (x >> 16u));
    return f32(x & 0x7fffffu) / f32(0x800000u);
}

// Returns the grid position for particle index `i` out of `n` total
// particles, with an optional seed-based offset applied.
fn grid_xy(i: u32, n: u32, seed: u32) -> vec2<f32> {
    let sqrt_n = u32(ceil(sqrt(f32(n))));
    let cols   = sqrt_n;
    let rows   = (n + cols - 1u) / cols;
    let col    = i % cols;
    let row    = i / cols;

    // Cell size in normalised space: cover a 0.4-wide square centred at 0.5.
    let cell   = 0.4 / f32(max(sqrt_n, 1u));
    let origin = 0.5 - 0.2;           // left/top edge of the 0.4 × 0.4 grid

    // Seed-based micro-offset (up to ±0.1 cell) keeps the grid deterministic
    // but lets the operator vary arrangement by changing `seed`.
    let ox = (hash_f(seed, i * 2u + 0u) - 0.5) * cell * 0.2;
    let oy = (hash_f(seed, i * 2u + 1u) - 0.5) * cell * 0.2;

    let x = origin + (f32(col) + 0.5) * cell + ox;
    let y = origin + (f32(row) + 0.5) * cell * (f32(n) / f32(cols * rows)) + oy;
    return vec2<f32>(x, clamp(y, 0.0, 1.0));
}

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    // n_particles is packed as f32 in the .w component of u_clock.
    let n = u32(u_clock.w);
    // Clamp to the SSBO capacity (MAX_PARTICLES = 2048).
    let max_n = min(n, 2048u);
    if idx >= max_n {
        return;
    }

    let t_local   = u_clock.y;
    let seed_bits = u32(u_clock.z);

    var p: Particle;
    p.pos      = grid_xy(idx, max_n, seed_bits);
    p.vel      = vec2<f32>(0.0, 0.0);
    p.age_secs = t_local;
    p._pad     = 0.0;
    p._pad2    = 0.0;
    p._pad3    = 0.0;

    particles[idx] = p;
}
