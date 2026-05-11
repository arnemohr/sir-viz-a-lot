// P1.3.1 — `tone_map` treatment.
//
// S-curve tone mapping applied to an Image / Video source before the
// per-pixel effect chain. Three params drive the look:
//   exposure (stops, -2..=+2)  — pre-curve gain; `gain = exp2(exposure)`
//   contrast (0.5..=1.5)       — multiplier around the 0.5 grey pivot
//   shoulder (0..=1)           — highlight rolloff (linear ↔ Reinhard mix)
//
// At identity defaults (exposure=0, contrast=1, shoulder=0) the shader is
// a bit-exact passthrough of the underlying `textured_quad.wgsl`, so the
// preset is visually transparent until the operator tunes it.
//
// Vertex / fit-mode plumbing mirrors `textured_quad.wgsl` (same bind
// layout for groups 0-2). Tone-map params live at binding 3 so the
// existing fit-mode uniform stays untouched.

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(0) @binding(2) var<uniform> u_fit: vec4<f32>;
@group(0) @binding(3) var<uniform> u_params: vec4<f32>; // x=exposure, y=contrast, z=shoulder, w=reserved

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
    let mode   = i32(u_fit.x + 0.5);
    let aspect = max(u_fit.y, 1e-4);
    let focal  = vec2<f32>(u_fit.z, u_fit.w);
    var uv = in.uv;

    if (mode == 1) {
        // Cover
        if (aspect > 1.0) {
            let scale = 1.0 / aspect;
            uv.x = (uv.x - 0.5) * scale + focal.x;
        } else {
            let scale = aspect;
            uv.y = (uv.y - 0.5) * scale + focal.y;
        }
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    } else if (mode == 2) {
        // Contain
        if (aspect > 1.0) {
            let scale = aspect;
            uv.y = (uv.y - 0.5) * scale + 0.5;
        } else {
            let scale = 1.0 / aspect;
            uv.x = (uv.x - 0.5) * scale + 0.5;
        }
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }

    let src = textureSample(t_diffuse, s_diffuse, uv);

    let exposure = u_params.x;
    let contrast = u_params.y;
    let shoulder = u_params.z;

    // Pre-curve gain (linear scale).
    let gain = exp2(exposure);
    let c1 = src.rgb * gain;

    // Contrast around mid-grey 0.5 pivot.
    let c2 = (c1 - vec3<f32>(0.5)) * contrast + vec3<f32>(0.5);

    // Shoulder rolloff: linear ↔ Reinhard crossfade. Reinhard maps
    // [0, ∞) → [0, 1), so highlights compress smoothly when shoulder > 0.
    // At shoulder = 0 the linear path is preserved bit-exactly (identity).
    let toned = c2 / (vec3<f32>(1.0) + max(c2, vec3<f32>(0.0)));
    let c3 = mix(c2, toned, shoulder);

    return vec4<f32>(c3, src.a);
}
