// Textured fullscreen quad. Samples @group(0) @binding(0) over the
// quad and writes RGBA. Used by both SVG layers (raster output of resvg)
// and Image layers (decoded JPG/PNG); the latter need fit-mode + focal
// awareness so a 16:9 photo doesn't stretch to a square (T-M8-04).
//
// Validated at build time by build.rs (naga). If this file fails to parse or
// validate, `cargo build` fails before any binary is produced.
//
// Uniform layout (16 bytes):
//   x = fit_mode    (0 = Stretch / SVG, 1 = Cover, 2 = Contain)
//   y = aspect_layer (texture_w / texture_h; >0)
//   z = focal_x      (normalized [0,1]; only meaningful for Cover)
//   w = focal_y      (normalized [0,1]; only meaningful for Cover)
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(0) @binding(2) var<uniform> u_fit: vec4<f32>;

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
    // Flip Y so the texture's top-left maps to the screen's top-left
    // (wgpu UV convention: 0,0 = top-left of texture; NDC y is up).
    out.uv  = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let mode    = i32(u_fit.x + 0.5);
    let aspect  = max(u_fit.y, 1e-4);
    let focal   = vec2<f32>(u_fit.z, u_fit.w);
    var uv = in.uv;

    // Quad aspect is 1:1 in pre-composite layer space (the layer's effect
    // chain renders into a unit square; warp later remaps to projector).
    // Cover crops along the long axis so the texture fills the quad;
    // Contain letterboxes; Stretch is the legacy SVG behavior.
    if (mode == 1) {
        // Cover: scale so the SHORT side fills, crop the long side around `focal`.
        if (aspect > 1.0) {
            // Layer is wider than tall — texture height fills, width crops.
            let scale = 1.0 / aspect;
            let center = focal.x;
            uv.x = (uv.x - 0.5) * scale + center;
        } else {
            let scale = aspect;
            let center = focal.y;
            uv.y = (uv.y - 0.5) * scale + center;
        }
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    } else if (mode == 2) {
        // Contain: scale so the LONG side fits, letterbox the short side.
        if (aspect > 1.0) {
            // Wider than tall — fit width, letterbox top/bottom.
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
    // mode == 0 (Stretch / SVG): pass-through.

    return textureSample(t_diffuse, s_diffuse, uv);
}
