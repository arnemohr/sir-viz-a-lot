// PCleanup.2.6 — `edge_sparks` Treatment compute shader.
//
// Particles spawn near the mask edge (SDF ≈ 0, sampled from the interior
// side) and live for a configurable lifetime before respawning at a new
// edge point.  Velocity is set outward along the SDF gradient so each spark
// drifts away from the mask as it ages.  The fragment shader reads `age`
// (seconds since spawn) and the configured `lifetime_s` to fade brightness
// over time — fresh sparks at full intensity, old sparks invisible.
//
// build.rs prepends sdf_helper.wgsl + treatment_particles_helper.wgsl
// (SDF first, then particle helpers) before this file at compile time.
//
// Bind-group layout matches treat_spotlights_compute.wgsl exactly so the
// shared `new_with_shaders` constructor reuses the same compute BGL:
//   group 0, binding 2: EdgeSparksComputeParams (uniform, 32 bytes)
//                         .x = drift_speed (UV/s along outward normal)
//                         .y = lifetime_s   (seconds, 0..=4)
//                         rest reserved.
//   group 0, binding 3: ClockUniform (vec4<f32>: clock, t_local, seed_f, n)
//   group 0, binding 4: t_sdf (texture_2d<f32>, R32Float, non-filterable)
//   group 0, binding 7: particles SSBO (read_write).

struct EdgeSparksComputeParams {
    drift_speed:  f32,
    lifetime_s:   f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
    _pad4: f32,
    _pad5: f32,
};

@group(0) @binding(2) var<uniform> u_params: EdgeSparksComputeParams;
@group(0) @binding(3) var<uniform> u_clock:  vec4<f32>;
@group(0) @binding(4) var          t_sdf:    texture_2d<f32>;
@group(0) @binding(7) var<storage, read_write> particles: array<Particle>;

// SDF gradient via central differences; used to push sparks outward along
// the mask normal.  Returns a unit vector when the SDF is non-degenerate;
// otherwise the zero vector (handled by callers).
fn sdf_gradient(uv: vec2<f32>) -> vec2<f32> {
    let eps = 1.0 / 256.0;
    let gx = sample_sdf_bilinear(t_sdf, uv + vec2<f32>(eps, 0.0))
           - sample_sdf_bilinear(t_sdf, uv - vec2<f32>(eps, 0.0));
    let gy = sample_sdf_bilinear(t_sdf, uv + vec2<f32>(0.0, eps))
           - sample_sdf_bilinear(t_sdf, uv - vec2<f32>(0.0, eps));
    let g = vec2<f32>(gx, gy);
    let m = length(g);
    if m < 1e-6 { return vec2<f32>(0.0, 0.0); }
    return g / m;
}

// Sample 16 candidate positions; return the first that sits inside the
// mask AND close to the boundary (|sdf| < band).  Falls back to a fully
// random unit position if no edge-adjacent point is found.
fn find_edge_pos(seed_bits: u32, idx: u32) -> vec2<f32> {
    let band = 0.05;
    for (var attempt: u32 = 0u; attempt < 16u; attempt = attempt + 1u) {
        let hx = tp_hash_f(seed_bits + attempt * 17u, idx * 5u + 1u);
        let hy = tp_hash_f(seed_bits + attempt * 19u, idx * 5u + 3u);
        let candidate = vec2<f32>(hx, hy);
        let s = sample_sdf_bilinear(t_sdf, candidate);
        // Inside the mask, near the edge.
        if s < 0.0 && s > -band { return candidate; }
    }
    return tp_random_unit_pos(seed_bits, idx);
}

const DT: f32 = 1.0 / 60.0;
const MAX_SPOTLIGHTS: u32 = 512u;

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let n   = min(u32(u_clock.w), MAX_SPOTLIGHTS);
    if idx >= n { return; }

    let t_local    = u_clock.y;
    let seed_bits  = u32(u_clock.z);
    let drift_speed = max(u_params.drift_speed, 0.0);
    // Lifetime defaults to 1.5 s when the operator hasn't set it.
    let lifetime_s = max(u_params.lifetime_s, 0.05);

    var p = particles[idx];

    // First-frame spawn: age == 0 means the SSBO was just zero-initialised.
    if p.age == 0.0 {
        p.pos = find_edge_pos(seed_bits, idx);
        // Velocity outward along the mask normal (gradient points outward
        // from the mask interior since SDF > 0 outside, < 0 inside).
        let grad = sdf_gradient(p.pos);
        p.vel = grad * drift_speed;
        p.age = max(t_local, 0.001);
        p._pad = max(t_local, 0.001); // store spawn time in _pad
        particles[idx] = p;
        return;
    }

    let elapsed = max(t_local - p._pad, 0.0);
    if elapsed > lifetime_s {
        // Faded out — respawn at the edge with a time-varied seed.
        let respawn_seed = seed_bits ^ (u32(t_local * 1000.0) + idx * 31u);
        p.pos = find_edge_pos(respawn_seed, idx);
        let grad = sdf_gradient(p.pos);
        p.vel = grad * drift_speed;
        p.age = t_local;
        p._pad = t_local; // new spawn time
        particles[idx] = p;
        return;
    }

    p.pos = p.pos + p.vel * DT;
    p.age = t_local;
    // p._pad keeps the spawn timestamp; p.vel persists.
    particles[idx] = p;
}
