// P2.6.1 — Fluid advection compute shader (semi-Lagrangian + dissipation).
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_".
//
// Implements a simple semi-Lagrangian advection step on a 256×256 velocity
// texture (RGBA16Float, ping-pong between two textures).  Each invocation:
//   1. Determines the grid cell UV for the current thread.
//   2. Reads the current velocity at that UV.
//   3. Back-traces: `back_uv = current_uv - velocity * dt` (semi-Lagrangian).
//   4. Bilinear-samples the current velocity texture at back_uv.
//   5. Applies dissipation: `result = back_velocity * (1.0 - dissipation * dt)`.
//   6. Writes result to the destination storage texture.
//
// Bind-group layout:
//   group 0, binding 0: source velocity  (texture_2d<f32>,      sampled, filterable, RGBA16Float)
//   group 0, binding 1: sampler          (sampler, filtering)
//   group 0, binding 2: dest velocity    (texture_storage_2d<rgba16float, write>)
//   group 0, binding 3: uniforms         (vec4<f32>: .x = dt, .y = dissipation, .z = clock_secs, .w = unused)
//
// Workgroup: 16×16 threads → dispatch ceil(256/16)=16 × ceil(256/16)=16 groups.
//
// NOTE: sdf_helper.wgsl functions (sample_sdf_bilinear etc.) are available
// because build.rs prepends the helper.  They are not used in this shader.
//
// TODO(P2.9.2): golden test for circular velocity blob after 10 advection ticks.

@group(0) @binding(0) var t_velocity_src : texture_2d<f32>;
@group(0) @binding(1) var s_linear       : sampler;
@group(0) @binding(2) var t_velocity_dst : texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform>        u_advect   : vec4<f32>;
// u_advect.x = dt_secs
// u_advect.y = dissipation_rate  (fraction/sec, e.g. 0.1)
// u_advect.z = clock_secs
// u_advect.w = inject_intensity  (0 = none; ~0.5 = visible swirl for fluid_identity)

@compute @workgroup_size(16, 16, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(t_velocity_src);
    let x = gid.x;
    let y = gid.y;
    if x >= dims.x || y >= dims.y { return; }

    let dt          = u_advect.x;
    let dissipation = u_advect.y;
    let clock_secs  = u_advect.z;
    let inject      = u_advect.w;

    // Current cell centre in UV [0, 1) space.
    let uv = (vec2<f32>(f32(x), f32(y)) + vec2<f32>(0.5)) / vec2<f32>(f32(dims.x), f32(dims.y));

    // Read current velocity at this cell.
    let cur_vel = textureSampleLevel(t_velocity_src, s_linear, uv, 0.0).xy;

    // Semi-Lagrangian back-trace: step backward along the velocity field.
    let back_uv = uv - cur_vel * dt;

    // Bilinear-sample velocity at the back-traced position (clamp to border).
    let clamped_uv = clamp(back_uv, vec2<f32>(0.0), vec2<f32>(1.0));
    let back_vel = textureSampleLevel(t_velocity_src, s_linear, clamped_uv, 0.0).xy;

    // Dissipation: decay velocity toward zero.
    var result_vel = back_vel * (1.0 - dissipation * dt);

    // Optional velocity injection: a swirl source centred at (0.5, 0.5) with
    // gaussian falloff. Without this the field stays at zero forever (a fluid
    // sim that nothing is stirring just sits there). The fluid_identity preset
    // sets `inject > 0` so the operator sees a steady swirl. bounded_fluid
    // can keep `inject = 0` if it has another source of motion.
    if inject > 0.0 {
        let d = uv - vec2<f32>(0.5, 0.5);
        let r = length(d);
        let falloff = exp(-r * r * 32.0); // gaussian, ~0.1 UV radius
        // Perpendicular to the radial vector → rotational flow. Slowly
        // wobble the rotation direction with the clock so it's not a
        // static-looking pattern.
        let phase = clock_secs * 0.4;
        let swirl = vec2<f32>(-d.y, d.x) + vec2<f32>(cos(phase), sin(phase)) * 0.15;
        result_vel = result_vel + swirl * (inject * falloff * dt * 4.0);
    }

    // Write result (store as RGBA; BA = 0).
    textureStore(t_velocity_dst, vec2<i32>(i32(x), i32(y)), vec4<f32>(result_vel.x, result_vel.y, 0.0, 1.0));
}
