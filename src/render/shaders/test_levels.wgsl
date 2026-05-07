// Test pattern: white levels + SMPTE 7-bar color bars. M2.
// Validated at build time by build.rs (naga). Spec §6 Test patterns.
//
// One shader, four variants selected by a `mode: u32` uniform. T-M2-08
// owns dispatch + bind group construction; this file is just the GPU
// program.
//
//   mode == 0u  →  White100   full-frame white at intensity 1.00
//   mode == 1u  →  White50    full-frame white at intensity 0.50
//   mode == 2u  →  White25    full-frame white at intensity 0.25
//   mode == 3u  →  ColorBars  SMPTE 7-bar split at 75% saturation
//
// SMPTE 7-bar order, left to right: white, yellow, cyan, green,
// magenta, red, blue. Each band is 1/7 of the framebuffer width.
// 75% saturation means each component is either 0.75 or 0.0.
//
// The mode uniform is wrapped in a single-field struct because WGSL's
// uniform layout rules don't always allow naked scalars in uniform
// blocks portably.
//
// Vertex stage matches triangle.wgsl / test_grid.wgsl: a six-vertex
// fullscreen quad driven by @builtin(vertex_index). Only the fragment
// stage differs.

struct Mode {
    value: u32,
};

@group(0) @binding(0) var<uniform> mode: Mode;

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
    out.uv  = p * 0.5 + vec2<f32>(0.5, 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (mode.value == 0u) {
        // White100
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    } else if (mode.value == 1u) {
        // White50
        return vec4<f32>(0.5, 0.5, 0.5, 1.0);
    } else if (mode.value == 2u) {
        // White25
        return vec4<f32>(0.25, 0.25, 0.25, 1.0);
    } else {
        // ColorBars — SMPTE 7-bar at 75% saturation. Index a fixed
        // array<vec3<f32>, 7> by floor(uv.x * 7). Clamp the band index
        // to [0, 6] so the right-edge pixel (uv.x == 1.0) doesn't fall
        // off the end of the array.
        let bars = array<vec3<f32>, 7>(
            vec3<f32>(0.75, 0.75, 0.75), // white
            vec3<f32>(0.75, 0.75, 0.0 ), // yellow
            vec3<f32>(0.0 , 0.75, 0.75), // cyan
            vec3<f32>(0.0 , 0.75, 0.0 ), // green
            vec3<f32>(0.75, 0.0 , 0.75), // magenta
            vec3<f32>(0.75, 0.0 , 0.0 ), // red
            vec3<f32>(0.0 , 0.0 , 0.75), // blue
        );
        let band = min(u32(in.uv.x * 7.0), 6u);
        let c = bars[band];
        return vec4<f32>(c, 1.0);
    }
}
