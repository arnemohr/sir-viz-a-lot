// P0.5.3 — Mask-edge ripple wash.
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_",
// so sample_sdf_bilinear / sample_sdf_gradient / sample_sdf are
// available without further imports.
//
// The ripple emanates from the polygon edge. `sdf_distance` is negative
// inside the polygon (per `src/render/sdf.rs`); we take its absolute
// value for the phase so the wave radiates from edge=0 inward and
// outward symmetrically. The exponential falloff darkens far-from-edge
// regions so the effect concentrates near the edge.
//
// Coordinate space: fragment UV is in [0,1]², which maps directly to
// the output-normalised space the SDF baker uses (same convention as
// warp.wgsl:46). No bounding-box gymnastics needed — documented as a
// v0.4 simplification; Phase 2 can refine to per-layer bounding-box
// rendering if needed.

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct FxParams {
    wavelength: f32,
    speed: f32,
    falloff: f32,
    base_r: f32,
    base_g: f32,
    base_b: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var t_sdf: texture_2d<f32>;
// NOTE: s_sdf (binding 1) is included for bind-group layout symmetry with
// future presets that may use filtered sampling. The helper functions use
// textureLoad internally (R32Float is unfilterable), so this sampler is
// unused at runtime. Dropped if naga validation rejects unused bindings.
@group(0) @binding(1) var s_sdf: sampler;
@group(0) @binding(2) var<uniform> u_params: FxParams;
@group(0) @binding(3) var<uniform> u_clock: vec4<f32>;  // .x = clock_secs

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Fullscreen triangle covering [-1,1]² clip space.
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vi & 2u) * 2.0 - 1.0;
    var o: VsOut;
    o.pos = vec4<f32>(x, y, 0.0, 1.0);
    // Flip Y to convert from NDC (Y-up) to UV (Y-down).
    o.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // SDF: negative inside the polygon, positive outside, ~0 at the edge.
    let d = sample_sdf_bilinear(t_sdf, in.uv);
    let t = u_clock.x;

    // Use absolute distance so the ripple radiates symmetrically from
    // the edge in both directions.
    let dist = abs(d);

    // Phase: distance drives the spatial frequency; time drives animation.
    let phase = dist * 6.28318530718 / max(u_params.wavelength, 1.0) - t * u_params.speed;
    let ripple = sin(phase);

    // Exponential falloff: pixels closer to the edge are brighter.
    let attenuation = exp(-dist / max(u_params.falloff, 1e-3));

    let intensity = 0.5 + 0.5 * ripple;
    let colour = vec3<f32>(u_params.base_r, u_params.base_g, u_params.base_b)
        * intensity
        * attenuation;

    // Alpha: full inside the polygon (d <= 0), fade to zero outside.
    // Smoothstep across one wavelength softens the outer boundary.
    let inside = 1.0 - smoothstep(0.0, max(u_params.wavelength, 1.0), d);
    return vec4<f32>(colour, inside);
}
