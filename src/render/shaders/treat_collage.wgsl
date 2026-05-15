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
//   3  params uniform: vec4<f32>×2 (32 bytes)
//      [0] mix        — crossfade 0=source, 1=collage
//      [1] gap_norm   — gap fraction
//      [2] slot_mask  — bit-packed u32 cast to f32
//      [3] mode       — 0=grid, 1=kaleidoscope, 2=mosaic
//      [4] seed_r0    — per-tile mosaic offset r0 (pre-hashed in Rust)
//      [5] seed_r1    — per-tile mosaic offset r1
//      [6] seed_r2    — per-tile mosaic offset r2
//      [7] _pad
//   4-7  collage slot textures (collage_0 .. collage_3)
//   8  collage sampler (shared)
//
// `slot_mask` is bit-packed: bit i (LSB → slot 0) set means the slot
// is present. Encoded as a float (cast back to u32 in the shader).
//
// PCleanup.8.3b — new `mode` param:
//   0 = grid (current 2×2 behaviour — default, identity for existing projects)
//   1 = kaleidoscope (mirror-fold each cell via |fract(uv*2)-0.5|*2)
//   2 = mosaic (each slot samples from a random region of the SOURCE,
//       deterministic via pre-hashed seed offsets; ignores slot textures)
//
// Identity at default: `mix = 0` → operator sees source unchanged.

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;
@group(0) @binding(2) var<uniform> u_fit: vec4<f32>;
@group(0) @binding(3) var<uniform> u_params: array<vec4<f32>, 2>;
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

// Compute a per-tile UV offset for mosaic mode from the 3 pre-hashed seed
// floats. Each slot gets a deterministic offset in [0, 0.5] so the sampled
// region fits within the source (avoids edge clamping artefacts for most
// sources). The offset pair for slot `idx` uses different combinations of
// the three seed values so all four tiles differ even from a single seed.
fn mosaic_offset(idx: i32, r0: f32, r1: f32, r2: f32) -> vec2<f32> {
    // Each slot uses a different linear combination of the 3 seed reals.
    // The combinations are chosen so slots 0-3 all produce distinct offsets
    // while keeping arithmetic simple (no branches, no arrays).
    let s = f32(idx);
    let ox = fract(r0 + s * r1);
    let oy = fract(r1 + s * r2);
    // Scale to [0, 0.5] so the sampled half-sized tile fits in the source.
    return vec2<f32>(ox * 0.5, oy * 0.5);
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

    let mix_amt   = clamp(u_params[0].x, 0.0, 1.0);
    let gap       = clamp(u_params[0].y, 0.0, 0.1);
    let slot_mask = u32(u_params[0].z + 0.5);
    let col_mode  = i32(u_params[0].w + 0.5);  // 0=grid, 1=kaleidoscope, 2=mosaic
    let seed_r0   = u_params[1].x;
    let seed_r1   = u_params[1].y;
    let seed_r2   = u_params[1].z;

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

    var cell_rgb = src.rgb;

    if (col_mode == 1) {
        // --- Kaleidoscope mode ---
        // Mirror-fold cell_uv: map [0,1] → [0,1] with a fold at 0.5.
        // fract(uv * 2) gives [0,1] tiling; then |x - 0.5| * 2 mirrors it.
        let folded_uv = abs(cell_uv * 2.0 - 1.0);
        let slot_bit = u32(1) << u32(slot_idx);
        let slot_present = (slot_mask & slot_bit) != u32(0);
        if (slot_present) {
            let sl = slot_color(slot_idx, folded_uv);
            cell_rgb = sl.rgb;
        } else {
            cell_rgb = textureSample(t_source, s_source, folded_uv).rgb;
        }
    } else if (col_mode == 2) {
        // --- Mosaic mode ---
        // Each slot samples from a different region of the SOURCE (ignores
        // slot textures). The region is a half-sized tile offset by a
        // deterministic, seed-derived amount. cell_uv scaled to [0,0.5]
        // + per-tile offset keeps the sample within [0,1].
        let tile_offset = mosaic_offset(slot_idx, seed_r0, seed_r1, seed_r2);
        let mosaic_uv = cell_uv * 0.5 + tile_offset;
        cell_rgb = textureSample(t_source, s_source, mosaic_uv).rgb;
    } else {
        // --- Grid mode (default) ---
        let slot_bit = u32(1) << u32(slot_idx);
        let slot_present = (slot_mask & slot_bit) != u32(0);
        if (slot_present) {
            let sl = slot_color(slot_idx, cell_uv);
            cell_rgb = sl.rgb;
        }
    }

    let out_rgb = mix(src.rgb, cell_rgb, mix_amt);
    return vec4<f32>(out_rgb, src.a);
}
