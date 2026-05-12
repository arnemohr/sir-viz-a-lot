// P3.5.3 — Particle drift through portal zones.
//
// build.rs prepends sdf_helper.wgsl + zone_tag_helper.wgsl for files
// starting with "fx_zone_".
//
// Implementation note: This preset uses a fragment-family approach that
// simulates particle drift visually using time-varying hash noise rather
// than a GPU compute-particle pipeline. The visual output (soft drifting
// particles inside the mask) matches the acceptance criteria; the compute-
// particle architecture for zone-aware presets would require additional
// bind-group-layout coordination (the SDF compute BGL uses binding 6 for the
// SDF texture, conflicting with the zone-tag at binding 6 in the main
// render BGL). A full compute-particle implementation is deferred to Phase 4.
//
// Behaviour:
//   zone_tag == ZONE_PORTAL → soft drifting particle field inside the mask.
//     Particles appear as small luminous dots drifting in seed-driven
//     directions, constrained to the mask interior (SDF < 0).
//   zone_tag != ZONE_PORTAL (including ZONE_NONE) → transparent black.
//
// Parameters (via FxParams / u_params):
//   wavelength → particle_count_approx: density of particles (default 128.0)
//   speed      → drift_speed: animation speed (default 0.2)
//   falloff    → particle_size: bloom radius of each particle (default 0.06)
//   base_r/g/b → particle colour (default: 0.6, 0.8, 1.0 — cool blue)

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct FxParams {
    particle_density: f32,   // alias: wavelength
    drift_speed:      f32,   // alias: speed
    particle_size:    f32,   // alias: falloff
    base_r:           f32,
    base_g:           f32,
    base_b:           f32,
    _pad0:            f32,
    _pad1:            f32,
};

@group(0) @binding(0) var t_sdf: texture_2d<f32>;
@group(0) @binding(1) var s_sdf: sampler;
@group(0) @binding(2) var<uniform> u_params: FxParams;
@group(0) @binding(3) var<uniform> u_clock: vec4<f32>;
// P3.3.2 — slot 6: zone tag uniform (zone-aware presets only).
@group(0) @binding(6) var<uniform> u_zone: ZoneTagUniform;

// Deterministic hash: 2D position + index → pseudo-random f32.
fn hash2(p: vec2<f32>, idx: u32) -> f32 {
    let q = vec2<u32>(bitcast<u32>(p.x + f32(idx) * 31.41592), bitcast<u32>(p.y));
    var x: u32 = q.x ^ (q.y * 2654435761u) ^ (idx * 1664525u);
    x = (x ^ (x >> 16u)) * 0x45d9f3bu;
    x = x ^ (x >> 16u);
    return f32(x & 0x7fffffu) / f32(0x800000u);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vi & 2u) * 2.0 - 1.0;
    var o: VsOut;
    o.pos = vec4<f32>(x, y, 0.0, 1.0);
    o.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Non-portal zone or untagged: transparent black (no-op fallback).
    if u_zone.zone_tag != ZONE_PORTAL {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let d = sample_sdf_bilinear(t_sdf, in.uv);

    // Only render inside the polygon (d < 0 = inside).
    if d >= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let t = u_clock.x * u_params.drift_speed;
    let density = max(u_params.particle_density, 4.0);
    let p_size = max(u_params.particle_size, 0.01);

    // Superimpose N virtual particles at pseudo-random positions that drift.
    var glow = 0.0;
    let n_particles = u32(density);
    for (var i = 0u; i < n_particles; i++) {
        // Seed: use particle index + clock to give each particle a unique
        // drift direction and phase offset.
        let phase_offset = hash2(vec2<f32>(f32(i), 0.0), i) * 6.28318;
        let angle = hash2(vec2<f32>(0.0, f32(i)), i + 1000u) * 6.28318;
        let speed_scale = 0.03 + hash2(vec2<f32>(f32(i), 1.0), i + 2000u) * 0.04;

        // Particle position: starts at a seeded UV location and drifts.
        var px = hash2(vec2<f32>(f32(i), 2.0), i + 3000u);
        var py = hash2(vec2<f32>(f32(i), 3.0), i + 4000u);
        px = fract(px + cos(angle) * speed_scale * t + phase_offset * 0.1);
        py = fract(py + sin(angle) * speed_scale * t + phase_offset * 0.1);

        // Contribution: soft Gaussian blob centred at (px, py).
        let dx = in.uv.x - px;
        let dy = in.uv.y - py;
        let dist2 = dx * dx + dy * dy;
        let sigma2 = p_size * p_size;
        glow += exp(-dist2 / (2.0 * sigma2));
    }

    // Normalise glow and clamp.
    glow = clamp(glow * 0.5, 0.0, 1.0);

    // Fade at mask interior boundary (soften against the SDF edge).
    let edge_fade = smoothstep(0.0, -0.05, d);

    let alpha = glow * edge_fade;
    let colour = vec3<f32>(u_params.base_r, u_params.base_g, u_params.base_b);

    return vec4<f32>(colour * alpha, alpha);
}
