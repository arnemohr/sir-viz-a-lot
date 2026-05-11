// P1.3.6 — `collage` treatment.
//
// Fixed 2×2 grid of up to four `collage_paths` textures composited over
// the layer's source. Each grid cell samples its slot texture; empty
// slots (signalled by params) fall back to source so the operator can
// fill 1/2/3 cells without leaving black holes.
//
// **Slot layout:**  cell (0,0) = slot 0, (1,0) = slot 1,
//                   (0,1) = slot 2, (1,1) = slot 3.
//
// Bind group (10 entries):
//   0  source texture
//   1  source sampler
//   2  fit_uniform (16 bytes)
//   3  params uniform: vec4<f32> (mix, gap_norm, slot_mask, _pad)
//   4-7  collage slot textures (collage_0 .. collage_3)
//   8  collage sampler (shared)
//   9  unused / future
//
// `slot_mask` is bit-packed: bit i (LSB → slot 0) set means the slot
// is present. Encoded as a float (cast back to u32 in the shader).
//
// Identity at default: `mix = 0` → operator sees source unchanged.

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_fit: vec4<f32>;
@group(0) @binding(3) var<uniform> u_params: vec4<f32>;
@group(0) @binding(4) var t_slot0: texture_2d<f32>;
@group(0) @binding(5) var t_slot1: texture_2d<f32>;
@group(0) @binding(6) var t_slot2: texture_2d<f32>;
@group(0) @binding(7) var t_slot3: texture_2d<f32>;
@group(0) @binding(8) var s_slot: sampler;

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

fn slot_color(idx: i32, uv: vec2<f32>) -> vec4<f32> {
    if (idx == 0) { return textureSample(t_slot0, s_slot, uv); }
    if (idx == 1) { return textureSample(t_slot1, s_slot, uv); }
    if (idx == 2) { return textureSample(t_slot2, s_slot, uv); }
    return textureSample(t_slot3, s_slot, uv);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let mode   = i32(u_fit.x + 0.5);
    let aspect = max(u_fit.y, 1e-4);
    let focal  = vec2<f32>(u_fit.z, u_fit.w);
    var src_uv = in.uv;

    if (mode == 1) {
        if (aspect > 1.0) {
            let scale = 1.0 / aspect;
            src_uv.x = (src_uv.x - 0.5) * scale + focal.x;
        } else {
            let scale = aspect;
            src_uv.y = (src_uv.y - 0.5) * scale + focal.y;
        }
        if (src_uv.x < 0.0 || src_uv.x > 1.0 || src_uv.y < 0.0 || src_uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    } else if (mode == 2) {
        if (aspect > 1.0) {
            let scale = aspect;
            src_uv.y = (src_uv.y - 0.5) * scale + 0.5;
        } else {
            let scale = 1.0 / aspect;
            src_uv.x = (src_uv.x - 0.5) * scale + 0.5;
        }
        if (src_uv.x < 0.0 || src_uv.x > 1.0 || src_uv.y < 0.0 || src_uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }
    let src = textureSample(t_source, s_source, src_uv);

    let mix_amt = clamp(u_params.x, 0.0, 1.0);
    let gap     = clamp(u_params.y, 0.0, 0.1);
    let slot_mask = u32(u_params.z + 0.5);

    // Pick the slot for this fragment based on (column, row) ∈ {0,1}².
    let col = select(0, 1, in.uv.x >= 0.5);
    let row = select(0, 1, in.uv.y >= 0.5);
    let slot_idx = row * 2 + col;

    // Cell-local UV (in [0, 1] per cell, with an optional gap inset).
    var cell_uv = vec2<f32>(
        fract(in.uv.x * 2.0),
        fract(in.uv.y * 2.0),
    );
    // Gap: shrink the visible cell content inward, leaving the
    // background source visible at the seam.
    let half_gap = gap;
    if (cell_uv.x < half_gap || cell_uv.x > 1.0 - half_gap
        || cell_uv.y < half_gap || cell_uv.y > 1.0 - half_gap) {
        return vec4<f32>(mix(src.rgb, vec3<f32>(0.0), 0.0), src.a);
    }
    // Re-normalise after the gap inset.
    cell_uv = (cell_uv - vec2<f32>(half_gap)) / max(1.0 - 2.0 * half_gap, 1e-4);

    let slot_bit = u32(1) << u32(slot_idx);
    let slot_present = (slot_mask & slot_bit) != u32(0);
    var cell_rgb = src.rgb;
    if (slot_present) {
        let sl = slot_color(slot_idx, cell_uv);
        cell_rgb = sl.rgb;
    }

    let out_rgb = mix(src.rgb, cell_rgb, mix_amt);
    return vec4<f32>(out_rgb, src.a);
}
