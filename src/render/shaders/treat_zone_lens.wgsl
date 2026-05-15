// PCleanup.2.10 — `zone_lens` treatment.
//
// SourceModifier sibling of the generative `fx_zone_edge_ripple` FX preset:
// instead of generating cool-blue ripple pixels at the zone edge, this
// treatment reads the source image and displaces its UV coordinates in a band
// around the ZONE_WINDOW mask edge, creating a refraction / lens effect at the
// zone perimeter.
//
// Behaviour:
//   zone_tag == ZONE_WINDOW → displace UVs in a band around the mask edge
//     using the same exponential attenuation as `fx_zone_edge_ripple`. The
//     displacement direction is the SDF gradient normal (perpendicular to the
//     edge); the magnitude is `sin(clock * speed + dist_to_edge * frequency) *
//     amplitude * band_weight`. Outside the band the weight decays to zero so
//     the displacement vanishes naturally (no hard cutoff).
//   zone_tag != ZONE_WINDOW (including ZONE_NONE) → passthrough: return
//     source unchanged. No crash, no visible effect.
//   amplitude == 0.0 → disp = vec2(0) everywhere → bit-exact passthrough.
//
// Parameters (packed into u_params as a 32-byte uniform struct):
//   amplitude  (0.0..=0.05, default 0.0) — max UV displacement at the crest.
//     Default 0 satisfies the identity-default rule: no effect until
//     the operator configures the slider.
//   speed      (0.0..=3.0, default 1.0) — animation rate (cycles/sec).
//   band_width (0.0..=0.3, default 0.05) — exponential decay constant that
//     controls how far from the edge the lens band reaches.
//   frequency  (0.0..=40.0, default 10.0) — spatial frequency of the
//     sine ripple along the band (higher = tighter sub-ripples).
//   clock_secs — accumulated time written by dispatcher each frame (not
//                an operator-facing param; packed into bytes 16–19).
//
// SDF sign convention: negative inside the polygon, positive outside, ~0
// at the edge. `abs(sdf)` gives unsigned distance to the edge perimeter.
//
// build.rs prepends sdf_helper.wgsl + zone_tag_helper.wgsl because this
// file's basename starts with "treat_zone_" (SDF_CONSUMERS and ZONE_CONSUMERS
// in build.rs — PCleanup.2.9 added the "treat_zone_" prefix).
//
// Bind-group layout (see ZoneLensTreatmentPipeline in treatments.rs):
//   binding 0 — t_source (Texture2d<f32>, filterable)
//   binding 1 — s_source (Sampler, filtering)
//   binding 2 — u_params (uniform, 32 bytes)
//   binding 3 — t_sdf    (Texture2d<f32>, non-filterable, R32Float)
//   binding 6 — u_zone   (ZoneTagUniform, 16 bytes)

struct ZoneLensParams {
    amplitude:  f32,   // 0..0.05
    speed:      f32,   // 0..3.0 Hz
    band_width: f32,   // 0..0.3
    frequency:  f32,   // 0..40.0
    clock_secs: f32,   // written each frame; not operator-facing
    _pad0:      f32,
    _pad1:      f32,
    _pad2:      f32,
};

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_params: ZoneLensParams;
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
    // Sample source first — needed for all passthrough paths.
    let src_uv = in.uv;

    // Non-window zone or untagged: pass source through unchanged.
    if u_zone.zone_tag != ZONE_WINDOW {
        return textureSample(t_source, s_source, src_uv);
    }

    // SDF: negative inside polygon, positive outside, ~0 at edge.
    let d = sample_sdf_bilinear(t_sdf, in.uv);

    // Unsigned distance from the edge — lens band is symmetric around the
    // perimeter (both inside and outside the polygon), matching the spatial
    // shape of `fx_zone_edge_ripple`.
    let dist_to_edge = abs(d);

    // Exponential attenuation centred on the edge (dist_to_edge = 0).
    // Mirrors fx_zone_edge_ripple's attenuation formula so the band shape
    // is identical; only the effect type differs (UV warp vs colour overlay).
    let band_width = max(u_params.band_width, 1e-4);
    let band_weight = exp(-dist_to_edge / band_width);

    // Sine ripple phase: spatial frequency along the band, animated by clock.
    // 6.28318... = TAU.
    let frequency = max(u_params.frequency, 1e-3);
    let phase = dist_to_edge * 6.28318530718 / (1.0 / frequency)
              - u_params.clock_secs * u_params.speed;
    let ripple = sin(phase);

    // SDF normal: gradient direction pointing away from the nearest edge
    // point. Displacing along the normal creates a lens-like refraction at
    // the perimeter. When amplitude = 0 → disp = vec2(0) everywhere →
    // bit-exact passthrough (no early-out needed; multiplicative zero).
    let normal = sample_sdf_normal(t_sdf, in.uv);
    let disp = normal * ripple * u_params.amplitude * band_weight;

    // Sample source at the displaced UV. Clamp to edge to avoid sampling
    // outside the texture (same policy as treat_ripple_lens.wgsl).
    let displaced_uv = clamp(src_uv + disp, vec2<f32>(0.0), vec2<f32>(1.0));
    return textureSample(t_source, s_source, displaced_uv);
}
