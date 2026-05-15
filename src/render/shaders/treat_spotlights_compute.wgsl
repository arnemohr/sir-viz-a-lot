// PCleanup.2.4 — `spotlights` Treatment compute shader.
//
// Updates particle positions each frame. On first frame (age == 0.0,
// uninitialised buffer) the particle is seeded at a random position inside
// the mask (SDF < 0) or, if no mask is present, anywhere in [0,1]². On
// subsequent frames the particle drifts slowly in a seed-derived random
// direction; it respawns inside the mask when it exits the boundary.
//
// build.rs prepends sdf_helper.wgsl (for sample_sdf_bilinear) and
// treatment_particles_helper.wgsl (for Particle struct + tp_hash_f helpers)
// before this file during build-time naga validation. At runtime,
// SpotlightsTreatmentPipeline::new_spotlights() concatenates the same sources
// in the same order.
//
// Bind-group layout (compute pass):
//   group 0, binding 2: SpotlightsComputeParams (uniform, 32 bytes)
//   group 0, binding 3: SpotlightsClockUniform  (uniform, 16 bytes = vec4<f32>)
//                         .x = clock_secs
//                         .y = t_layer_local_secs
//                         .z = seed_f32 (lower 23 bits of LayerState seed)
//                         .w = n_particles (as f32)
//   group 0, binding 4: t_sdf (texture_2d<f32>, R32Float, non-filterable)
//                         Optional: when no mask is present the caller binds a
//                         1×1 dummy texture at (0.5) so particles spawn
//                         uniformly in [0,1]² instead of inside a polygon.
//   group 0, binding 7: particles SSBO (array<Particle>, read_write)
//                         Slot 7 = particle SSBO for Treatment compute passes.
//                         Slots 0-6: source, sampler, params, sdf, sdf_sampler,
//                         zone_tag, fit — reserved for fragment pass.

struct SpotlightsComputeParams {
    drift_speed: f32,  // UV/s, typically 0..=1.0
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
    _pad4: f32,
    _pad5: f32,
    _pad6: f32,
};

@group(0) @binding(2) var<uniform> u_params: SpotlightsComputeParams;
@group(0) @binding(3) var<uniform> u_clock:  vec4<f32>;
@group(0) @binding(4) var          t_sdf:    texture_2d<f32>;
// Slot 7: Treatment compute particle SSBO (locked — see TreatmentInputs comment block).
@group(0) @binding(7) var<storage, read_write> particles: array<Particle>;

// Attempts to find a spawn position inside the mask (SDF < 0).
// Falls back to a random position in [0,1]² if no interior point is found
// in 16 attempts (e.g. when the SDF covers the whole texture as positive).
fn find_spawn_pos(seed_bits: u32, idx: u32) -> vec2<f32> {
    for (var attempt: u32 = 0u; attempt < 16u; attempt = attempt + 1u) {
        let hx = tp_hash_f(seed_bits + attempt * 17u, idx * 5u + 1u);
        let hy = tp_hash_f(seed_bits + attempt * 19u, idx * 5u + 3u);
        let candidate = vec2<f32>(hx, hy);
        if sample_sdf_bilinear(t_sdf, candidate) < 0.0 {
            return candidate;
        }
    }
    // Fallback: unconstrained random position.
    return tp_random_unit_pos(seed_bits, idx);
}

// Fixed timestep: 1/60 s. Keeps drift independent of actual frame rate.
const DT: f32 = 1.0 / 60.0;

// Maximum particles for spotlights.  Hard cap in the SSBO; caller must
// allocate at least MAX_SPOTLIGHTS * 24 bytes.
const MAX_SPOTLIGHTS: u32 = 512u;

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let n   = min(u32(u_clock.w), MAX_SPOTLIGHTS);
    if idx >= n { return; }

    let t_local    = u_clock.y;
    let seed_bits  = u32(u_clock.z);
    let drift_speed = max(u_params.drift_speed, 0.0);

    var p = particles[idx];

    // First-frame spawn: age == 0.0 means the SSBO was just zero-initialised.
    if p.age == 0.0 {
        p.pos = find_spawn_pos(seed_bits, idx);
        p.vel = vec2<f32>(0.0, 0.0);  // spotlights don't use vel
        p.age = max(t_local, 0.001);  // avoid re-triggering spawn next frame
        p._pad = 0.0;
        particles[idx] = p;
        return;
    }

    // Drift in a deterministic random direction (seed + index → angle).
    let dir     = tp_rand_dir(seed_bits, idx);
    let new_pos = p.pos + dir * drift_speed * DT;

    // Outside mask → respawn inside using a time-varied seed to prevent
    // all ejected particles clustering at the same spot.
    if sample_sdf_bilinear(t_sdf, new_pos) >= 0.0 {
        let respawn_seed = seed_bits ^ (u32(t_local * 1000.0) + idx * 31u);
        p.pos = find_spawn_pos(respawn_seed, idx);
        p.age = t_local;
    } else {
        p.pos = new_pos;
        p.age = t_local;
    }
    // vel and _pad stay at 0 for spotlights.
    particles[idx] = p;
}
