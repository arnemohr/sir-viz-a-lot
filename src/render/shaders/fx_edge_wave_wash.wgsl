// P2.4.3 — Mask-edge wave wash.
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_",
// so sample_sdf_bilinear / sample_sdf_gradient / sample_sdf_normal are
// available without further imports.
//
// Produces a wave that travels *along* the mask edge, as opposed to the
// concentric rings of fx_ripple_wash.wgsl. The angular position around the
// mask is approximated via atan2(normal.y, normal.x) on the SDF gradient
// direction. Emission is gated to a band of width `wave_width` around the
// edge (|sdf| < wave_width). The band itself is fully self-illuminated; no
// source texture (binding 4) is read.
//
// # FxParamsUniform field mapping
//
// The shader reads from the generic FxParamsUniform layout. Fields are
// aliased as documented here — the Rust constructor `FxParamsUniform::for_edge_wave_wash`
// fills them in:
//
// | FxParamsUniform field | Semantic for this preset               | Default |
// |-----------------------|----------------------------------------|---------|
// | speed                 | wave_speed (animation speed, 0..=5)    | 1.0     |
// | falloff               | wave_width (band half-width, 0..=0.3)  | 0.15    |
// | base_r                | colour (cold↔warm tint, 0..=1)         | 0.5     |
// | wavelength            | unused (always 0.0)                    | 0.0     |
// | base_g                | unused (always 0.0)                    | 0.0     |
// | base_b                | unused (always 0.0)                    | 0.0     |
// | _pad0                 | unused (always 0.0)                    | 0.0     |
// | _pad1                 | unused (always 0.0)                    | 0.0     |
//
// TODO: Add a gpu-tests golden image (needs a real wgpu adapter on CI;
// deferred per P2.4.3 spec — don't add golden test here).
//
// Coordinate space: fragment UV is in [0,1]², matching the SDF baker's
// output-normalised convention. Same as fx_ripple_wash.wgsl.

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct FxParams {
    wavelength: f32,  // PCleanup.3.2: n_waves — crest count (1..=8, default 4).
    speed: f32,       // wave_speed: animation speed (cycles/sec). Default 1.0.
    falloff: f32,     // wave_width: half-width of the edge emission band. Default 0.15.
    base_r: f32,      // colour: 0.0 = cold blue tint, 1.0 = warm orange tint. Default 0.5.
    base_g: f32,      // unused — see EDGE_WAVE_WASH_DESCRIPTORS doc for rationale.
    base_b: f32,      // unused — see EDGE_WAVE_WASH_DESCRIPTORS doc for rationale.
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var t_sdf: texture_2d<f32>;
// NOTE: s_sdf (binding 1) is included for bind-group layout symmetry with
// other presets. Not sampled at runtime (textureLoad is used via the helper).
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
    let clock_secs = u_clock.x;

    // Read wave_speed, wave_width, and colour from the aliased uniform fields.
    let wave_speed = u_params.speed;
    let wave_width = max(u_params.falloff, 1e-4);
    let colour_mix = clamp(u_params.base_r, 0.0, 1.0);

    // SDF: negative inside the polygon, positive outside, ~0 at the edge.
    let sdf_dist = sample_sdf_bilinear(t_sdf, in.uv);
    let unsigned_dist = abs(sdf_dist);

    // Gate emission to a band of width `wave_width` around the edge.
    let band = smoothstep(wave_width, 0.0, unsigned_dist);

    // Approximate angular position around the mask using the SDF normal
    // direction (normalised gradient). atan2 wraps in (-π, π].
    let normal = sample_sdf_normal(t_sdf, in.uv);
    let phi = atan2(normal.y, normal.x);

    // PCleanup.3.2 — `wavelength` (aliased from FxParamsUniform) now drives
    // the crest count. The shader rounds to the nearest integer because
    // non-integer crest counts produce a visible seam at the angle wrap
    // point (atan2 returns (-π, π]; a fractional N_WAVES makes
    // `sin(phi*N - clock)` discontinuous at ±π). max(1, round(...))
    // clamps below 1 → at least one crest always travels around the edge.
    let n_waves = max(1.0, round(u_params.wavelength));
    let wave = 0.5 + 0.5 * sin(phi * n_waves - clock_secs * wave_speed * 6.28318);

    // Interpolate between a cool blue and a warm amber for the tint.
    let cold = vec3<f32>(0.4, 0.6, 1.0);
    let warm = vec3<f32>(1.0, 0.8, 0.3);
    let tint = mix(cold, warm, colour_mix);

    // Premultiplied alpha output.
    let alpha = band * wave;
    let rgb = tint * alpha;
    return vec4<f32>(rgb, alpha);
}
