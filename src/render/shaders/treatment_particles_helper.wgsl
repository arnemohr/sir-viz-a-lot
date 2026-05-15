// PCleanup.2.4 — Treatment-particle shared helper.
//
// This file is a function-only module (no entry points, no @binding
// declarations). Treatment compute shaders that need particle simulation
// prepend this source at runtime (via TREATMENT_PARTICLE_SIM_WGSL in
// treatment_particles.rs), mirroring the sdf_helper.wgsl / zone_tag_helper.wgsl
// pattern.
//
// # Particle struct (locked layout — do NOT change across W2.4–W2.6)
//
// std430 layout, 24-byte stride:
//   pos: vec2<f32>  — offset  0, size 8 (normalised [0,1]²)
//   vel: vec2<f32>  — offset  8, size 8 (UV/s; zero for spotlights)
//   age: f32        — offset 16, size 4 (used by edge_sparks fade; zero for spotlights)
//   _pad: f32       — offset 20, size 4 (alignment pad)
//
// Consumers that don't need vel/age (e.g. spotlights) keep them at 0.0.
// Future W2.5/W2.6 treatments will set vel for smear and age for fade.
//
// # Simple hash helper
//
// `hash_f(a, b)` — deterministic f32 in [0,1) from two u32 keys.  Same
// implementation as fx_particles_drift.wgsl; reproduced here so
// treatment_particles shaders are independent of the FX module.
//
// # Spawn helper
//
// `find_interior_pos_sdf(t_sdf, seed, idx)` — finds a spawn position inside
// the mask (SDF < 0).  Tries 16 hash-derived candidates; returns the first
// inside point. Falls back to (0.5, 0.5) if all outside.  Only callable
// from shaders that also prepend sdf_helper.wgsl (spotlights compute does).
//
// `random_unit_pos(seed, idx)` — random position in [0,1]² without SDF
// gating.  Used as fallback when no SDF is available.

// Particle struct — shared across all Treatment compute shaders (W2.4–W2.6).
// DO NOT change field order or add/remove fields; stride 24 bytes is locked.
struct Particle {
    pos: vec2<f32>,  // normalised [0, 1]²
    vel: vec2<f32>,  // UV/s; zero for spotlights (future: smear vel for W2.5)
    age: f32,        // seconds alive; zero for spotlights (future: W2.6 fade)
    _pad: f32,       // explicit alignment pad to reach 24-byte boundary
};

// Deterministic f32 in [0, 1) from two u32 keys (Wang/Knuth hash).
fn tp_hash_f(a: u32, b: u32) -> f32 {
    var x: u32 = a ^ (b * 2654435761u);
    x = (x ^ (x >> 16u)) * 0x45d9f3bu;
    x = x ^ (x >> 16u);
    return f32(x & 0x7fffffu) / f32(0x800000u);
}

// Random unit direction from seed + particle index.
fn tp_rand_dir(seed: u32, idx: u32) -> vec2<f32> {
    let angle = tp_hash_f(seed, idx * 3u + 7u) * 6.28318530718;
    return vec2<f32>(cos(angle), sin(angle));
}

// Unconstrained random position in [0, 1]².
fn tp_random_unit_pos(seed: u32, idx: u32) -> vec2<f32> {
    let hx = tp_hash_f(seed + 1u, idx * 5u + 1u);
    let hy = tp_hash_f(seed + 3u, idx * 5u + 3u);
    return vec2<f32>(hx, hy);
}
