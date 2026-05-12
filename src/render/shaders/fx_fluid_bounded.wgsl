// P2.6.2 — Mask-bounded fluid advection compute shader.
//
// build.rs prepends sdf_helper.wgsl for files starting with "fx_".
//
// Extends the semi-Lagrangian advection from fx_fluid_advect.wgsl with
// mask-constrained no-slip boundary conditions:
//
//   1. Standard semi-Lagrangian advect step (same as fx_fluid_advect.wgsl).
//   2. Sample the SDF at the current cell.
//      - If SDF > 0 (outside mask, positive-outside convention): zero velocity.
//      - If |SDF| < boundary_epsilon (near boundary): reflect velocity using
//        the SDF normal (from `sample_sdf_normal` in sdf_helper.wgsl).
//   3. Apply dissipation.
//
// SDF convention: negative inside the mask, positive outside.
// `sample_sdf_normal` returns the gradient direction (pointing away from mask).
//
// Bind-group layout:
//   group 0, binding 0: source velocity  (texture_2d<f32>, RGBA16Float, filterable)
//   group 0, binding 1: sampler          (filtering)
//   group 0, binding 2: dest velocity    (texture_storage_2d<rgba16float, write>)
//   group 0, binding 3: uniforms         (vec4<f32>: .x = dt, .y = dissipation, .z = clock, .w = unused)
//   group 0, binding 4: SDF texture      (texture_2d<f32>, R32Float, unfilterable)
//
// Workgroup: 16×16 threads → dispatch ceil(256/16)=16 × 16 groups.
//
// Simplification note: particle visualisation was skipped in this commit.
// The `particle_count` descriptor exists for the spec test contract but the
// shader operates as a pure velocity-field simulation.  Particle SSBO
// visualisation is deferred to a follow-up.
//
// TODO(P2.9.2): golden test for vortex in circular mask at clock=5.

@group(0) @binding(0) var t_velocity_src : texture_2d<f32>;
@group(0) @binding(1) var s_linear       : sampler;
@group(0) @binding(2) var t_velocity_dst : texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform>        u_advect   : vec4<f32>;
// u_advect.x = dt_secs
// u_advect.y = dissipation_rate
// u_advect.z = clock_secs
// u_advect.w = inject_intensity  (centred swirl source; 0 = no injection)
@group(0) @binding(4) var                 t_sdf      : texture_2d<f32>;

// Distance threshold for boundary cell detection (in normalised SDF units).
const BOUNDARY_EPSILON: f32 = 0.01;

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

    // --- Standard semi-Lagrangian advect ---
    let cur_vel = textureSampleLevel(t_velocity_src, s_linear, uv, 0.0).xy;
    let back_uv = uv - cur_vel * dt;
    let clamped_uv = clamp(back_uv, vec2<f32>(0.0), vec2<f32>(1.0));
    var advected_vel = textureSampleLevel(t_velocity_src, s_linear, clamped_uv, 0.0).xy;

    // --- Mask boundary enforcement ---
    // Note: sample_sdf_bilinear is provided by sdf_helper.wgsl (prepended by build.rs).
    // Note: sample_sdf_normal is provided by sdf_helper.wgsl.
    let sdf_val = sample_sdf_bilinear(t_sdf, uv);

    if sdf_val > 0.0 {
        // Outside mask: no-slip — zero velocity.
        advected_vel = vec2<f32>(0.0, 0.0);
    } else if abs(sdf_val) < BOUNDARY_EPSILON {
        // Near boundary: reflect velocity along the SDF normal.
        // The normal points outward (away from mask interior).
        let normal = sample_sdf_normal(t_sdf, uv);
        // Reflect: v_reflect = v - 2 * dot(v, n) * n
        let dot_vn = dot(advected_vel, normal);
        // Only reflect if velocity is pointing outward (toward boundary).
        if dot_vn > 0.0 {
            advected_vel = advected_vel - 2.0 * dot_vn * normal;
        }
    }

    // --- Dissipation ---
    var result_vel = advected_vel * (1.0 - dissipation * dt);

    // --- Velocity injection (swirl source at mask centre) ---
    // Inject only when fully inside the mask, so the source point isn't
    // killed by the no-slip boundary on the same tick.
    if inject > 0.0 && sdf_val < -BOUNDARY_EPSILON {
        let d = uv - vec2<f32>(0.5, 0.5);
        let r = length(d);
        let falloff = exp(-r * r * 32.0);
        let phase = clock_secs * 0.4;
        let swirl = vec2<f32>(-d.y, d.x) + vec2<f32>(cos(phase), sin(phase)) * 0.15;
        result_vel = result_vel + swirl * (inject * falloff * dt * 4.0);
    }

    textureStore(t_velocity_dst, vec2<i32>(i32(x), i32(y)), vec4<f32>(result_vel.x, result_vel.y, 0.0, 1.0));
}
