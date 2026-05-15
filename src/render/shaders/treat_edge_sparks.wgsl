// PCleanup.2.6 — `edge_sparks` Treatment fragment shader.
//
// Sibling of `spotlights`: same additive Gaussian luminance lift, but each
// spark's contribution fades over its lifetime so the visual is closer to
// glowing embers along the mask edge.  The compute shader writes the spawn
// timestamp into `Particle._pad`; this fragment computes the spark's age
// from `(clock - spawn)` and modulates intensity by `1 - age / lifetime`.
//
// At brightness_gain == 0.0 the output is bit-exact passthrough.
//
// Bind-group layout (fragment pass):
//   group 0, binding 0: t_source  (texture_2d<f32>, filterable)
//   group 0, binding 1: s_source  (sampler, filtering)
//   group 0, binding 2: u_params  (uniform, 32 bytes = EdgeSparksFragParams)
//   group 0, binding 7: particles (storage, read — array<Particle>)

struct EdgeSparksFragParams {
    brightness_gain: f32,  // 0..=2.0, identity 0.0 → passthrough
    radius:          f32,  // 0.01..=0.3, normalised UV
    n_particles:     f32,  // u32 cast (1..=512)
    clock_secs:      f32,  // current frame time, used for age = clock - spawn
    lifetime_s:      f32,  // particle lifetime (matches compute pass)
    _pad0:           f32,
    _pad1:           f32,
    _pad2:           f32,
};

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_params: EdgeSparksFragParams;
@group(0) @binding(7) var<storage, read> particles: array<Particle>;

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
    let src = textureSample(t_source, s_source, in.uv);

    let brightness_gain = u_params.brightness_gain;
    let radius          = max(u_params.radius, 1e-4);
    let radius_sq       = radius * radius;
    let n               = u32(u_params.n_particles);
    let clock_secs      = u_params.clock_secs;
    let lifetime_s      = max(u_params.lifetime_s, 0.05);

    // Accumulate Gaussian weights, modulated by each spark's remaining
    // lifetime fraction.  Sparks past their lifetime contribute 0.
    var weight_sum: f32 = 0.0;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let particle = particles[i];
        let spawn_time = particle._pad;
        let elapsed = max(clock_secs - spawn_time, 0.0);
        let life_frac = clamp(1.0 - elapsed / lifetime_s, 0.0, 1.0);
        if life_frac <= 0.0 { continue; }

        let diff    = in.uv - particle.pos;
        let dist_sq = dot(diff, diff);
        if dist_sq < radius_sq {
            weight_sum = weight_sum + life_frac * exp(-dist_sq / (2.0 * radius_sq));
        }
    }

    // Clamp the additive weight so dense spark clusters don't blow out the
    // image, matching spotlights' upper bound.
    weight_sum = min(weight_sum, f32(n));

    let multiplier = 1.0 + brightness_gain * weight_sum;
    return vec4<f32>(src.rgb * multiplier, src.a);
}
