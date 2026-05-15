// PCleanup.2.8 — `collision_ripples` Treatment compute shader.
//
// Particles cycle through two states encoded in `Particle._pad`:
//   _pad <  0.5  → DRIFTING: behave like spotlights (move at vel, age increments).
//                  When the next step crosses the mask boundary, freeze at the
//                  collision point, zero the velocity, reset age, set
//                  `_pad = initial_amp` to record the ripple's starting strength.
//   _pad >= 0.5  → RIPPLING: position is frozen at the collision site, age
//                  increments each frame.  After `ripple_lifetime`, respawn
//                  into the mask (back to DRIFTING).
//
// This avoids needing a second SSBO + atomics for a ring buffer.  Each
// particle is its own ripple emitter; collisions and ripples are 1:1.
//
// Bind-group layout matches treat_spotlights_compute.wgsl so the shared
// `new_with_shaders` constructor reuses the compute BGL exactly:
//   group 0, binding 2: CollisionRipplesComputeParams (uniform, 32 bytes)
//                         .x = drift_speed         (UV/s, drift phase)
//                         .y = ripple_lifetime     (s, before respawn)
//                         .z = initial_amplitude   (>= 0.5 marker)
//                         rest reserved.
//   group 0, binding 3: ClockUniform (vec4<f32>)
//   group 0, binding 4: t_sdf (texture_2d<f32>, R32Float, non-filterable)
//   group 0, binding 7: particles SSBO (read_write)

struct CollisionRipplesComputeParams {
    drift_speed:       f32,
    ripple_lifetime:   f32,
    initial_amplitude: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
    _pad4: f32,
};

@group(0) @binding(2) var<uniform> u_params: CollisionRipplesComputeParams;
@group(0) @binding(3) var<uniform> u_clock:  vec4<f32>;
@group(0) @binding(4) var          t_sdf:    texture_2d<f32>;
@group(0) @binding(7) var<storage, read_write> particles: array<Particle>;

fn find_spawn_pos(seed_bits: u32, idx: u32) -> vec2<f32> {
    for (var attempt: u32 = 0u; attempt < 16u; attempt = attempt + 1u) {
        let hx = tp_hash_f(seed_bits + attempt * 17u, idx * 5u + 1u);
        let hy = tp_hash_f(seed_bits + attempt * 19u, idx * 5u + 3u);
        let candidate = vec2<f32>(hx, hy);
        if sample_sdf_bilinear(t_sdf, candidate) < 0.0 {
            return candidate;
        }
    }
    return tp_random_unit_pos(seed_bits, idx);
}

const DT: f32 = 1.0 / 60.0;
const MAX_SPOTLIGHTS: u32 = 512u;
const RIPPLE_STATE_MARKER: f32 = 0.5;

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let n   = min(u32(u_clock.w), MAX_SPOTLIGHTS);
    if idx >= n { return; }

    let t_local         = u_clock.y;
    let seed_bits       = u32(u_clock.z);
    let drift_speed     = max(u_params.drift_speed, 0.0);
    let ripple_lifetime = max(u_params.ripple_lifetime, 0.1);
    let initial_amp     = max(u_params.initial_amplitude, RIPPLE_STATE_MARKER + 0.001);

    var p = particles[idx];

    // First-frame initialisation: DRIFTING.
    if p.age == 0.0 && p._pad == 0.0 {
        p.pos = find_spawn_pos(seed_bits, idx);
        let dir = tp_rand_dir(seed_bits, idx);
        p.vel = dir * drift_speed;
        p.age = max(t_local, 0.001);
        p._pad = 0.0; // DRIFTING marker
        particles[idx] = p;
        return;
    }

    if p._pad < RIPPLE_STATE_MARKER {
        // DRIFTING: step forward, detect boundary crossing.
        let new_pos = p.pos + p.vel * DT;
        if sample_sdf_bilinear(t_sdf, new_pos) >= 0.0 {
            // Boundary crossed → spawn a ripple at the collision site.
            // Approximate the collision point as the midpoint (no binary
            // search; spec aims for visual fidelity, not physical accuracy).
            p.pos = (p.pos + new_pos) * 0.5;
            p.vel = vec2<f32>(0.0, 0.0);
            // Reset age so the ripple's local age starts at 0.
            p.age = max(t_local, 0.001);
            p._pad = initial_amp; // RIPPLING (>= 0.5)
        } else {
            p.pos = new_pos;
            // Keep age incrementing — fragment doesn't read it in DRIFT state.
        }
    } else {
        // RIPPLING: position frozen, age the ripple.
        let elapsed = max(t_local - p.age, 0.0);
        if elapsed > ripple_lifetime {
            // Ripple expired → respawn as DRIFTING with a new seed.
            let respawn_seed = seed_bits ^ (u32(t_local * 1000.0) + idx * 31u);
            p.pos = find_spawn_pos(respawn_seed, idx);
            let dir = tp_rand_dir(respawn_seed, idx);
            p.vel = dir * drift_speed;
            p.age = t_local;
            p._pad = 0.0; // DRIFTING
        }
        // else: stay frozen, fragment reads (clock - age) for ripple time.
    }

    particles[idx] = p;
}
