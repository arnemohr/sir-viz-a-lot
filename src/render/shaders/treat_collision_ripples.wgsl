// PCleanup.2.8 — `collision_ripples` Treatment fragment shader.
//
// Each ACTIVE particle (state marker `_pad >= 0.5` from the compute pass,
// set when the particle crossed the mask boundary) emits a circular wave
// originating at its frozen position.  The fragment samples the source at
// `uv + total_displacement`, where the displacement is the sum of radial
// pulses from all active ripples.
//
// At `amplitude = 0.0` the output is bit-exact passthrough: total
// displacement is always zero so `textureSample(t_source, ...)` lands at
// the unmodified UV.
//
// Bind-group layout (fragment pass) — matches the shared particle render BGL:
//   group 0, binding 0: t_source  (texture_2d<f32>, filterable)
//   group 0, binding 1: s_source  (sampler, filtering)
//   group 0, binding 2: u_params  (uniform, 32 bytes = CollisionRipplesFragParams)
//   group 0, binding 7: particles (storage, read — array<Particle>)

struct CollisionRipplesFragParams {
    amplitude:   f32,  // 0..=0.1, peak UV displacement at the ripple front
    frequency:   f32,  // 1..=80, wavelength (higher = tighter rings)
    speed:       f32,  // 0..=2.0, ring expansion rate (UV/s)
    decay:       f32,  // 0..=5.0, exponential damping per second
    n_particles: f32,  // u32 cast
    clock_secs:  f32,  // current time, for `age = clock - p.age`
    _pad0:       f32,
    _pad1:       f32,
};

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_params: CollisionRipplesFragParams;
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

const RIPPLE_STATE_MARKER: f32 = 0.5;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let amplitude  = u_params.amplitude;
    let frequency  = max(u_params.frequency, 1.0);
    let speed      = max(u_params.speed, 0.0);
    let decay      = max(u_params.decay, 0.0);
    let n          = u32(u_params.n_particles);
    let clock_secs = u_params.clock_secs;

    var displacement = vec2<f32>(0.0, 0.0);

    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let p = particles[i];
        // Only RIPPLING particles contribute (state marker in _pad).
        if p._pad < RIPPLE_STATE_MARKER { continue; }

        let local_age = max(clock_secs - p.age, 0.0);
        let to_frag = in.uv - p.pos;
        let dist = length(to_frag);
        if dist < 1e-5 { continue; }
        let dir = to_frag / dist;

        // Gaussian envelope around the expanding ring at radius = age*speed.
        let ring_r = local_age * speed;
        let band   = max(0.5 / frequency, 0.01);
        let offset = dist - ring_r;
        let env    = exp(-(offset * offset) / (2.0 * band * band));

        // Carrier wave + temporal decay.
        let phase = (dist - ring_r) * frequency * 6.2831853;
        let wave  = sin(phase) * exp(-local_age * decay);

        // Modulate by the recorded initial amplitude (stored in _pad).
        let strength = p._pad;
        displacement = displacement + dir * amplitude * wave * env * strength;
    }

    let warped_uv = in.uv + displacement;
    return textureSample(t_source, s_source, warped_uv);
}
