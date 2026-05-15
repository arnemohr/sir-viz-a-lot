// PCleanup.2.9 — `zone_brighten` treatment.
//
// SourceModifier sibling of the generative `fx_zone_light_spill` FX preset:
// instead of adding a warm-glow colour overlay on top of the layer, this
// treatment reads the source image inside the ZONE_WINDOW-tagged area and
// multiplicatively boosts its luminance, leaving hue and saturation intact.
//
// Behaviour:
//   zone_tag == ZONE_WINDOW → sample source, compute brightness multiplier
//     `1.0 + intensity * exp(-dist_in / (spill_radius * falloff)) * smooth_weight`,
//     multiply source RGB by it, keep alpha unchanged.
//   zone_tag != ZONE_WINDOW (including ZONE_NONE) → passthrough: return
//     source unchanged. No crash, no visible effect.
//   intensity == 0.0 → multiplier = 1.0 everywhere → bit-exact passthrough.
//
// Parameters (packed into u_params as a 32-byte uniform struct):
//   intensity    (0.0..=2.0, default 0.0) — boost magnitude at the edge.
//   falloff      (0.0..=20.0, default 8.0) — exponent sharpness.
//   spill_radius (0.0..=1.0, default 0.3) — normalised reach inside polygon.
//   speed        (0.0..=2.0, default 0.0) — breathing pulse rate (cycles/sec).
//   clock_secs   — accumulated time written by dispatcher each frame (not
//                  an operator-facing param; packed into byte offsets 16-19).
//
// SDF sign convention: negative inside the polygon, positive outside, ~0
// at the edge. We only brighten where d < 0 (inside) and within spill_radius.
//
// build.rs prepends sdf_helper.wgsl + zone_tag_helper.wgsl because this
// file's basename starts with "treat_zone_" (added to SDF_CONSUMERS and
// ZONE_CONSUMERS in build.rs — PCleanup.2.9).
//
// Bind-group layout (see ZoneBrightenTreatmentPipeline in treatments.rs):
//   binding 0 — t_source (Texture2d<f32>, filterable)
//   binding 1 — s_source (Sampler, filtering)
//   binding 2 — u_params (uniform, 32 bytes)
//   binding 3 — t_sdf    (Texture2d<f32>, non-filterable, R32Float)
//   binding 6 — u_zone   (ZoneTagUniform, 16 bytes)

struct ZoneBrightenParams {
    intensity:    f32,   // 0..2.0
    falloff:      f32,   // 0..20.0
    spill_radius: f32,   // 0..1.0
    speed:        f32,   // 0..2.0 Hz
    clock_secs:   f32,   // written each frame; not operator-facing
    _pad0:        f32,
    _pad1:        f32,
    _pad2:        f32,
};

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_params: ZoneBrightenParams;
@group(0) @binding(3) var t_sdf:   texture_2d<f32>;  // R32Float; textureLoad, no sampler needed
// P3.3.2 — slot 6: zone tag uniform (zone-aware treatments only).
@group(0) @binding(6) var<uniform> u_zone: ZoneTagUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0)       uv:  vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    let positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>( 1.0, -1.0), vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0), vec2<f32>( 1.0, -1.0), vec2<f32>( 1.0,  1.0),
    );
    let p = positions[idx];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv  = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Sample source first — needed for the passthrough paths.
    let src = textureSample(t_source, s_source, in.uv);

    // Non-window zone or untagged: pass source through unchanged.
    if u_zone.zone_tag != ZONE_WINDOW {
        return src;
    }

    // intensity == 0.0 → boost = 0 → multiplier = 1.0 → bit-exact passthrough.
    let intensity = u_params.intensity;

    // SDF: negative inside polygon, positive outside, ~0 at edge.
    // sample_sdf_bilinear uses textureLoad internally (R32Float is unfilterable).
    let d = sample_sdf_bilinear(t_sdf, in.uv);

    // Only brighten inside the polygon (dist_in > 0) and within spill_radius.
    let spill_radius = max(u_params.spill_radius, 0.01);
    let dist_in = -d; // positive inside the polygon
    if dist_in <= 0.0 {
        return src;
    }

    // Brightness multiplier: peaks at the edge (dist_in ≈ 0), falls off
    // exponentially with depth. Mirrors fx_zone_light_spill's falloff formula
    // so the spatial shape is identical; only the effect type differs.
    let falloff = max(u_params.falloff, 1e-3);
    let boost = intensity * exp(-dist_in / (spill_radius * falloff));

    // Smooth the boost to zero at spill_radius (avoids a hard cutoff ring).
    let smooth_weight = smoothstep(spill_radius, 0.0, dist_in);

    // Optional breathing pulse — mirrors fx_zone_light_spill exactly.
    // speed > 0 modulates ±15% around a 0.85 DC offset. speed == 0.0
    // gives a constant 1.0 multiplier (no pulse), matching static glow.
    let speed = max(u_params.speed, 0.0);
    let pulse = select(
        1.0,
        0.85 + 0.15 * sin(6.28318530718 * u_params.clock_secs * speed),
        speed > 1e-6,
    );

    let multiplier = 1.0 + boost * smooth_weight * pulse;

    // Multiply RGB by the brightness multiplier; alpha is preserved.
    // Hue and saturation stay intact — only luminance is affected.
    return vec4<f32>(src.rgb * multiplier, src.a);
}
