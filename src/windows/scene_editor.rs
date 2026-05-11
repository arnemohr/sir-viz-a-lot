//! Direct-manipulation scene editor (M10–M11) — the live preview's
//! click/drag handling.
//!
//! v2's headline operator move: click a layer in the Scene tab to select
//! it, drag to translate, modifier-drag to scale / rotate. The preview
//! itself is the input surface; the underlying `warp_rt` egui texture
//! is the visual.
//!
//! Hit-test priority (M11 will fill in the higher-priority entries):
//! 1. Warp corners   (small, precise — pick first)
//! 2. Mask vertices
//! 3. Source-rect corners
//! 4. Layer body     (largest hit area, picked last)
//!
//! Drag math uses the snapshot pattern: `DragSession.start_value` records
//! the project's state at drag-start, every frame computes "start +
//! cumulative delta" rather than accumulating per-frame deltas. Avoids the
//! float-drift bugs that haunt interactive editors.

use egui::Pos2;

use crate::effects::Effect;
use crate::modulators::Modulator;
#[cfg_attr(not(feature = "v3"), allow(unused_imports))]
use crate::project::schema::{LayerConfig, Project, WarpMesh};
#[cfg_attr(not(feature = "v3"), allow(unused_imports))]
use crate::windows::theme::{
    ACCENT, ACCENT_DIM, HANDLE_ACTIVE, HANDLE_DEFAULT, HANDLE_OUTLINE, LAYER_PALETTE, MASK_EDGE,
    MESH_LINE, TEXT_SECONDARY,
};

/// Pixel radius for mask-vertex hit-testing in preview space (M11).
const MASK_HANDLE_HIT_PX: f32 = 9.0;
/// Pixel radius for the painted mask handle.
const MASK_HANDLE_DRAW_PX: f32 = 5.5;
/// Hit-test radius for warp grid corners. Larger than the mask radius
/// (Fitts's law — warp corners are the canvas's primary direct-
/// manipulation surface; an 9 px target on a HiDPI display is too
/// tight to grab reliably). Stays well under WARP_SNAP_RADIUS_PX so
/// the magnetic-corner snap behaviour at drag-end is unaffected.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
const WARP_HANDLE_HIT_PX: f32 = 16.0;
/// Painted radius for warp grid handles. Slightly larger than the
/// mask-handle radius so the grippable target reads as a handle, not
/// a decoration.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
const WARP_HANDLE_DRAW_PX: f32 = 7.0;
/// Distance to a mask edge that counts as "double-click on this edge"
/// for the insert-vertex gesture (M11).
const MASK_EDGE_HIT_PX: f32 = 7.0;
/// 003-T3.10 — pixel radius (in canvas-screen space) for the warp-
/// corner snap to one of the four framebuffer corners. Holding Shift
/// at drag-end bypasses the snap.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
const WARP_SNAP_RADIUS_PX: f32 = 10.0;
/// The four framebuffer corners a warp grid vertex can snap to.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
const FRAMEBUFFER_CORNERS: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

/// 003-T3.10 — return the closest framebuffer corner if `pos` is
/// within `WARP_SNAP_RADIUS_PX` (measured in screen pixels). `None`
/// otherwise. Used by the drag-end branch to decide whether to snap
/// the corner to integer coordinates.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
pub fn snap_corner_target(pos: [f32; 2], preview_rect: egui::Rect) -> Option<[f32; 2]> {
    let r2 = WARP_SNAP_RADIUS_PX * WARP_SNAP_RADIUS_PX;
    let mut best: Option<(f32, [f32; 2])> = None;
    for corner in FRAMEBUFFER_CORNERS.iter() {
        let dx = (pos[0] - corner[0]) * preview_rect.width();
        let dy = (pos[1] - corner[1]) * preview_rect.height();
        let d2 = dx * dx + dy * dy;
        if d2 <= r2 && best.as_ref().map(|(b, _)| d2 < *b).unwrap_or(true) {
            best = Some((d2, *corner));
        }
    }
    best.map(|(_, c)| c)
}

/// What the operator currently has selected in the Scene preview. Single-
/// select for v2; multi-select is deferred. The non-Layer variants land
/// at M11 — declared now so the dispatch shape doesn't change later.
#[allow(dead_code)] // M11 fills in the WarpCorner/MaskVertex/SourceRect arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Layer(usize),
    /// (M11) Single warp control point at `(grid[r][c])`.
    WarpCorner {
        warp: usize,
        r: usize,
        c: usize,
    },
    /// (M11) One mask polygon vertex.
    MaskVertex {
        warp: usize,
        idx: usize,
    },
    /// (M11) One of the four `source_rect` corners.
    SourceRect {
        warp: usize,
        corner: SourceRectCorner,
    },
}

#[allow(dead_code)] // M11 wires source-rect corner editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRectCorner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

/// Drag-time snapshot. Different selection types snapshot different
/// fields; one struct + one enum covers them so `handle_scene_input`
/// stays a single match.
#[derive(Debug, Clone)]
pub struct DragSession {
    pub start_screen: Pos2,
    pub kind: DragKind,
}

#[derive(Debug, Clone)]
pub enum DragKind {
    LayerTransform {
        start_translate: [f32; 2],
        start_scale: [f32; 2],
        start_rotate_deg: f32,
        mode: DragMode,
        /// 003-T3.29 — pre-drag warp snapshot, captured at drag_started
        /// for v3's `ResetLayerWarpMesh` Reverse storage (rule 3 — full
        /// `WarpMesh` snapshot). Under v5 the warp IS the layer's
        /// placement, so Layer-mode Translate / Scale / Rotate drags
        /// transform the warp grid (and emit a warp mutation at
        /// drag-stop) instead of writing to `Effect::Transform`.
        ///
        /// Replaces the v3-original `effects_snapshot` field — Layer-
        /// mode drag no longer touches `Effect::Transform`. Modulator
        /// pickers and the Effects panel continue to mutate the
        /// effects chain through their own paths.
        #[cfg(feature = "v3")]
        start_warp: WarpMesh,
    },
    /// Mask polygon vertex move (M11). Captures the original normalized
    /// position so live drag is `start + delta_normalized`.
    MaskVertex {
        warp: usize,
        idx: usize,
        start_pos: [f32; 2],
    },
    /// 003-T3.5 — warp corner move. `layer_idx` indexes
    /// `Project.layers`; `r`/`c` index `LayerConfig.warp.grid`.
    /// `start_pos` is the pre-drag normalized output-space position so
    /// the live drag is `start + delta_normalized`.
    WarpCorner {
        layer_idx: usize,
        r: usize,
        c: usize,
        start_pos: [f32; 2],
    },
}

/// 003-T3.7: canvas interaction mode. Each non-`Inspect` mode is
/// implicitly scoped to the selected layer — there is no global
/// warp or mask under v4 (T3.0a).
#[allow(dead_code)] // T3.4 wires Warp button; T3.5 reads Warp in corner-drag; T3.8 reads all for banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditMode {
    /// Default: drag/scale/rotate the selected layer's body.
    #[default]
    Layer,
    /// Edit the selected layer's warp grid corners. T3.5 wires the
    /// drag handler.
    Warp,
    /// Edit the selected layer's mask polygon. The existing mask-
    /// vertex drag in `handle_scene_input` already targets the
    /// selected layer's mask; T3.7 just gates the visual on this
    /// mode.
    Mask,
    /// Selection only, no drag.
    Inspect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragMode {
    Translate,
    Scale,
    Rotate,
}

#[derive(Default)]
pub struct SceneEditorState {
    pub selected: Option<Selection>,
    pub drag: Option<DragSession>,
    pub mode: EditMode,
    /// Last mode rendered — used by [`mode_banner`] to drive the cross-fade
    /// animation when the mode changes. Read by the v3 banner-render path
    /// (T4.15 follow-up); the field stays in the struct because
    /// `SceneEditorState` is constructed from the v3 launcher and the
    /// v3 banner reads it transitively. Allow dead_code under
    /// `--all-features` until the read site is wired.
    #[allow(dead_code)]
    pub previous_mode: Option<EditMode>,
}

/// Read the layer's effective static `(translate, scale, rotate_deg)` from
/// the first `Effect::Transform` in its effect chain. Modulator-driven
/// values are *not* sampled — selection is operator-edit-time, not
/// animation-time, so we use the `Modulator::Static(v)` arms and fall
/// back to identity when other variants are present (a sine-modulated
/// scale uses 1.0 here so the hit-rect doesn't pulse with the audio).
///
/// Returns identity (translate 0, scale 1, rotate 0) when the chain has
/// no Transform effect — matches what the renderer would do.
pub fn effective_static_transform(layer: &LayerConfig) -> ([f32; 2], [f32; 2], f32) {
    for e in layer.effects.iter() {
        if let Effect::Transform {
            translate,
            scale_x,
            scale_y,
            rotate_deg,
        } = e
        {
            let s_x = match scale_x {
                Modulator::Static(v) => *v,
                _ => 1.0,
            };
            let s_y = match scale_y {
                Modulator::Static(v) => *v,
                _ => 1.0,
            };
            let rot = match rotate_deg {
                Modulator::Static(v) => *v,
                _ => 0.0,
            };
            return (*translate, [s_x, s_y], rot);
        }
    }
    ([0.0, 0.0], [1.0, 1.0], 0.0)
}

/// Mutate the layer's first `Effect::Transform` via the given closure.
/// If the chain doesn't yet contain a Transform effect, append one with
/// identity defaults first — so `mutate_transform_effect` is always safe
/// to call from a drag handler.
pub fn mutate_transform_effect<F>(layer: &mut LayerConfig, mutate: F)
where
    F: FnOnce(&mut [f32; 2], &mut Modulator, &mut Modulator, &mut Modulator),
{
    if !layer
        .effects
        .iter()
        .any(|e| matches!(e, Effect::Transform { .. }))
    {
        layer.effects.push(Effect::Transform {
            translate: [0.0, 0.0],
            rotate_deg: Modulator::Static(0.0),
            scale_x: Modulator::Static(1.0),
            scale_y: Modulator::Static(1.0),
        });
    }
    for e in layer.effects.iter_mut() {
        if let Effect::Transform {
            translate,
            rotate_deg,
            scale_x,
            scale_y,
        } = e
        {
            mutate(translate, rotate_deg, scale_x, scale_y);
            return;
        }
    }
}

/// 003-T3.29 — centroid (mean position) of all warp grid points.
/// Used as the pivot for Scale / Rotate drags so the quad scales and
/// rotates "in place" rather than relative to the canvas origin.
#[cfg(feature = "v3")]
fn warp_grid_centroid(grid: &[Vec<[f32; 2]>]) -> [f32; 2] {
    let mut n = 0u32;
    let mut sx = 0.0f32;
    let mut sy = 0.0f32;
    for row in grid {
        for p in row {
            sx += p[0];
            sy += p[1];
            n += 1;
        }
    }
    if n == 0 {
        [0.5, 0.5]
    } else {
        [sx / n as f32, sy / n as f32]
    }
}

/// 003-T3.29 — bit-exact grid comparison used by the drag-stop path
/// to skip the mutation entirely on a zero-delta drag (operator
/// clicked without moving — no work to undo).
#[cfg(feature = "v3")]
fn grids_byte_equal(a: &[Vec<[f32; 2]>], b: &[Vec<[f32; 2]>]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (ar, br) in a.iter().zip(b.iter()) {
        if ar.len() != br.len() {
            return false;
        }
        for (ap, bp) in ar.iter().zip(br.iter()) {
            if ap[0].to_bits() != bp[0].to_bits() || ap[1].to_bits() != bp[1].to_bits() {
                return false;
            }
        }
    }
    true
}

/// 003-T3.29 — return a translated copy of `grid`. Each point shifts
/// by `(dx, dy)` in projector [0, 1]² space.
#[cfg(feature = "v3")]
fn translated_grid(grid: &[Vec<[f32; 2]>], dx: f32, dy: f32) -> Vec<Vec<[f32; 2]>> {
    grid.iter()
        .map(|row| row.iter().map(|p| [p[0] + dx, p[1] + dy]).collect())
        .collect()
}

/// 003-T3.29 — return a copy of `grid` scaled about its centroid by
/// `factor`. `factor < 1` shrinks; `factor > 1` enlarges. The centroid
/// is unchanged.
#[cfg(feature = "v3")]
fn scaled_grid(grid: &[Vec<[f32; 2]>], factor: f32) -> Vec<Vec<[f32; 2]>> {
    let [cx, cy] = warp_grid_centroid(grid);
    grid.iter()
        .map(|row| {
            row.iter()
                .map(|p| [cx + (p[0] - cx) * factor, cy + (p[1] - cy) * factor])
                .collect()
        })
        .collect()
}

/// 003-T3.29 — return a copy of `grid` rotated about its centroid by
/// `theta_rad`. Positive theta rotates clockwise in the canvas
/// y-down coordinate system (matches egui's mouse delta convention).
#[cfg(feature = "v3")]
fn rotated_grid(grid: &[Vec<[f32; 2]>], theta_rad: f32) -> Vec<Vec<[f32; 2]>> {
    let [cx, cy] = warp_grid_centroid(grid);
    let (s, c) = theta_rad.sin_cos();
    grid.iter()
        .map(|row| {
            row.iter()
                .map(|p| {
                    let dx = p[0] - cx;
                    let dy = p[1] - cy;
                    [cx + dx * c - dy * s, cy + dx * s + dy * c]
                })
                .collect()
        })
        .collect()
}

/// Hit-test screen-space `pos` against every layer in `project`, walking
/// reverse draw order so the topmost layer wins. Returns the layer index
/// of the topmost match.
///
/// Each layer's hit rect is the unit-quad shifted by its static
/// `Effect::Transform.translate` and shrunk by `scale_x / scale_y`. The
/// renderer's textured-quad pipeline always covers the full layer rect
/// in normalized output space; `Effect::Transform` shifts and scales
/// the *content* within that rect, so for picking purposes we treat the
/// transform-shifted box as the "where the layer is on-screen" region.
/// Modulator-animated Transform fields fall back to identity (see
/// `effective_static_transform`) so drag-pick doesn't drift mid-music.
pub fn hit_layer(project: &Project, pos_screen: Pos2, preview_rect: egui::Rect) -> Option<usize> {
    let pos_norm = screen_to_normalized(pos_screen, preview_rect)?;
    for (idx, layer) in project.layers.iter().enumerate().rev() {
        if !layer.enabled {
            continue;
        }
        let (translate, scale, _rot) = effective_static_transform(layer);
        let half = [scale[0].abs() * 0.5, scale[1].abs() * 0.5];
        let center = [0.5 + translate[0], 0.5 + translate[1]];
        if pos_norm[0] >= center[0] - half[0]
            && pos_norm[0] <= center[0] + half[0]
            && pos_norm[1] >= center[1] - half[1]
            && pos_norm[1] <= center[1] + half[1]
        {
            return Some(idx);
        }
    }
    None
}

/// Hit-test screen-space `pos` against every mask-polygon vertex of every
/// warp. Returns the first match within `MASK_HANDLE_HIT_PX`. Walked in
/// (warp, idx) lexicographic order; ties at the same screen point pick
/// the first warp's earliest vertex (deterministic, predictable for
/// undo-style tooling).
pub fn hit_mask_vertex(
    project: &Project,
    pos_screen: Pos2,
    preview_rect: egui::Rect,
) -> Option<(usize, usize)> {
    if !preview_rect.contains(pos_screen) {
        return None;
    }
    let to_screen = |n: [f32; 2]| -> Pos2 {
        egui::pos2(
            preview_rect.left() + n[0] * preview_rect.width(),
            preview_rect.top() + n[1] * preview_rect.height(),
        )
    };
    let r2 = MASK_HANDLE_HIT_PX * MASK_HANDLE_HIT_PX;
    for (w_idx, layer) in project.layers.iter().enumerate() {
        let warp = &layer.warp;
        for (v_idx, p) in warp.mask_polygon.iter().enumerate() {
            let s = to_screen(*p);
            let dx = pos_screen.x - s.x;
            let dy = pos_screen.y - s.y;
            if dx * dx + dy * dy <= r2 {
                return Some((w_idx, v_idx));
            }
        }
    }
    None
}

/// Distance from `p` to segment `(a, b)` in 2D.
fn point_segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let apx = p.x - a.x;
    let apy = p.y - a.y;
    let ab2 = abx * abx + aby * aby;
    if ab2 < 1e-6 {
        return (apx * apx + apy * apy).sqrt();
    }
    let t = ((apx * abx + apy * aby) / ab2).clamp(0.0, 1.0);
    let cx = a.x + t * abx;
    let cy = a.y + t * aby;
    let dx = p.x - cx;
    let dy = p.y - cy;
    (dx * dx + dy * dy).sqrt()
}

/// Hit-test screen-space `pos` against every mask polygon edge. Returns
/// `(warp, vertex_idx_after_which_to_insert, normalized_xy)` for the
/// closest edge within `MASK_EDGE_HIT_PX`. The returned index is the
/// *after* index — the insert call should do
/// `mask_polygon.insert(idx + 1, pt)` so the new vertex sits between
/// the old endpoints in list order. Used by the insert-vertex
/// double-click gesture (M11).
pub fn hit_mask_edge(
    project: &Project,
    pos_screen: Pos2,
    preview_rect: egui::Rect,
) -> Option<(usize, usize, [f32; 2])> {
    if !preview_rect.contains(pos_screen) {
        return None;
    }
    let to_screen = |n: [f32; 2]| -> Pos2 {
        egui::pos2(
            preview_rect.left() + n[0] * preview_rect.width(),
            preview_rect.top() + n[1] * preview_rect.height(),
        )
    };
    let to_norm = |s: Pos2| -> [f32; 2] {
        [
            (s.x - preview_rect.left()) / preview_rect.width().max(1.0),
            (s.y - preview_rect.top()) / preview_rect.height().max(1.0),
        ]
    };
    let mut best: Option<(f32, usize, usize)> = None;
    for (w_idx, layer) in project.layers.iter().enumerate() {
        let warp = &layer.warp;
        let n = warp.mask_polygon.len();
        if n < 2 {
            continue;
        }
        for i in 0..n {
            let a = to_screen(warp.mask_polygon[i]);
            let b = to_screen(warp.mask_polygon[(i + 1) % n]);
            let d = point_segment_distance(pos_screen, a, b);
            if d <= MASK_EDGE_HIT_PX && best.as_ref().map(|(bd, ..)| d < *bd).unwrap_or(true) {
                best = Some((d, w_idx, i));
            }
        }
    }
    let (_, w_idx, after) = best?;
    Some((w_idx, after, to_norm(pos_screen)))
}

/// 003-T3.5 — hit-test screen-space `pos` against the warp grid
/// vertices of `layer_idx`'s warp. Returns `(r, c)` of the closest
/// vertex within `WARP_HANDLE_HIT_PX`. Caller should only invoke when
/// `EditMode::Warp` is active and a layer is selected — this is the
/// per-layer-clarity contract from T3.5's spec.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
pub fn hit_warp_corner(
    project: &Project,
    layer_idx: usize,
    pos_screen: Pos2,
    preview_rect: egui::Rect,
) -> Option<(usize, usize)> {
    if !preview_rect.contains(pos_screen) {
        return None;
    }
    let layer = project.layers.get(layer_idx)?;
    let warp = &layer.warp;
    let to_screen = |n: [f32; 2]| -> Pos2 {
        egui::pos2(
            preview_rect.left() + n[0] * preview_rect.width(),
            preview_rect.top() + n[1] * preview_rect.height(),
        )
    };
    let r2 = WARP_HANDLE_HIT_PX * WARP_HANDLE_HIT_PX;
    let mut best: Option<(f32, usize, usize)> = None;
    for (r, row) in warp.grid.iter().enumerate() {
        for (c, p) in row.iter().enumerate() {
            let s = to_screen(*p);
            let dx = pos_screen.x - s.x;
            let dy = pos_screen.y - s.y;
            let d = dx * dx + dy * dy;
            if d <= r2 && best.as_ref().map(|(bd, ..)| d < *bd).unwrap_or(true) {
                best = Some((d, r, c));
            }
        }
    }
    best.map(|(_, r, c)| (r, c))
}

/// Convert a screen pos inside `preview_rect` to normalized [0, 1]
/// output-space. Returns `None` when `pos` is outside the rect.
pub fn screen_to_normalized(pos: Pos2, preview_rect: egui::Rect) -> Option<[f32; 2]> {
    if !preview_rect.contains(pos) {
        return None;
    }
    let w = preview_rect.width().max(1.0);
    let h = preview_rect.height().max(1.0);
    Some([
        (pos.x - preview_rect.left()) / w,
        (pos.y - preview_rect.top()) / h,
    ])
}

/// Apply one frame of click / drag input from the scene preview to the
/// project. Called from `show_scene_tab` after egui's response is in
/// hand. `preview_rect` is the actual on-screen rect the texture was
/// painted into (which may be smaller than the allocated panel due to
/// aspect letterboxing).
///
/// Behavior:
/// - Mouse-down inside the preview: hit-test layers; set selection +
///   start a `DragSession` snapshot.
/// - Drag while a layer is selected: set `transform.translate` =
///   `start_translate + (cursor_delta_normalized)`.
/// - Mouse-up: clear `drag` (selection persists). Under v3, emits a
///   `Mutation::SetLayerEffects` for Translate drags covering the full
///   cumulative delta (T-003-T1.24).
/// - Click outside any layer: clear selection.
///
/// Returns `Some(Mutation)` under v3 when a translate drag ends and a
/// mutation should be pushed to the undo stack. Returns `None` in all
/// other cases; in v2 builds (no undo machinery) always returns `None`.
#[cfg(feature = "v3")]
pub fn handle_scene_input(
    response: &egui::Response,
    project: &mut Project,
    scene: &mut SceneEditorState,
    preview_rect: egui::Rect,
    pointer: Option<Pos2>,
    modifiers: egui::Modifiers,
) -> Option<crate::project::command::Mutation> {
    let mut emitted: Option<crate::project::command::Mutation> = None;

    if response.drag_started() {
        if let Some(pos) = pointer {
            scene.drag = None;
            // 003-T3.5 — Warp mode: only the *selected layer's* corners
            // are hit-testable. The N-other-layers' grids are not
            // painted and not interactive while in Warp mode (per-layer-
            // clarity goal). Layer-body / mask hit tests still fire as
            // a fallback when no corner is hit, so the operator can
            // re-select another layer mid-Warp.
            let warp_layer_idx = if scene.mode == EditMode::Warp {
                if let Some(Selection::Layer(idx)) = scene.selected {
                    Some(idx)
                } else if let Some(Selection::WarpCorner { warp, .. }) = scene.selected {
                    Some(warp)
                } else {
                    None
                }
            } else {
                None
            };
            let warp_hit = warp_layer_idx.and_then(|idx| {
                hit_warp_corner(project, idx, pos, preview_rect).map(|rc| (idx, rc))
            });

            // Hit-test priority: warp corners (Warp mode + selected
            // layer) → mask vertices → layer body.
            if let Some((layer_idx, (r, c))) = warp_hit {
                let start_pos = project.layers[layer_idx].warp.grid[r][c];
                scene.selected = Some(Selection::WarpCorner {
                    warp: layer_idx,
                    r,
                    c,
                });
                scene.drag = Some(DragSession {
                    start_screen: pos,
                    kind: DragKind::WarpCorner {
                        layer_idx,
                        r,
                        c,
                        start_pos,
                    },
                });
            } else if let Some((w_idx, v_idx)) = hit_mask_vertex(project, pos, preview_rect) {
                let start_pos = project.layers[w_idx].warp.mask_polygon[v_idx];
                scene.mode = EditMode::Mask;
                scene.selected = Some(Selection::MaskVertex {
                    warp: w_idx,
                    idx: v_idx,
                });
                scene.drag = Some(DragSession {
                    start_screen: pos,
                    kind: DragKind::MaskVertex {
                        warp: w_idx,
                        idx: v_idx,
                        start_pos,
                    },
                });
            } else if let Some(idx) = hit_layer(project, pos, preview_rect) {
                // 003-T3.5 follow-up — in Warp mode a layer-body click selects
                // the layer (so its grid becomes visible and its corners are
                // hit-testable) but does NOT start a translate/scale/rotate
                // drag. Otherwise an operator who clicks slightly off a small
                // corner handle accidentally moves the whole layer instead of
                // missing the corner and leaving state untouched.
                if scene.mode == EditMode::Warp {
                    scene.selected = Some(Selection::Layer(idx));
                } else {
                    let (translate, scale, rotate) =
                        effective_static_transform(&project.layers[idx]);
                    let mode = if modifiers.shift {
                        DragMode::Scale
                    } else if modifiers.alt {
                        DragMode::Rotate
                    } else {
                        DragMode::Translate
                    };
                    #[cfg(feature = "v3")]
                    let start_warp = project.layers[idx].warp.clone();
                    scene.selected = Some(Selection::Layer(idx));
                    scene.drag = Some(DragSession {
                        start_screen: pos,
                        kind: DragKind::LayerTransform {
                            start_translate: translate,
                            start_scale: scale,
                            start_rotate_deg: rotate,
                            mode,
                            #[cfg(feature = "v3")]
                            start_warp,
                        },
                    });
                }
            } else {
                scene.selected = None;
            }
        }
    }

    if response.dragged() {
        if let (Some(pos), Some(drag)) = (pointer, scene.drag.as_ref()) {
            let dx = (pos.x - drag.start_screen.x) / preview_rect.width().max(1.0);
            let dy = (pos.y - drag.start_screen.y) / preview_rect.height().max(1.0);
            match &drag.kind {
                DragKind::LayerTransform {
                    start_translate,
                    start_scale,
                    start_rotate_deg,
                    mode,
                    #[cfg(feature = "v3")]
                    start_warp,
                    ..
                } => {
                    if let Some(Selection::Layer(idx)) = scene.selected {
                        if let Some(layer) = project.layers.get_mut(idx) {
                            // 003-T3.29 — under v3 the warp IS the layer's
                            // placement; Layer-mode drags transform the warp
                            // grid (about the quad's centroid for Scale /
                            // Rotate) rather than mutating Effect::Transform.
                            // The drag math reads `start_warp.grid` (frozen
                            // at drag-start) so the cumulative transform is
                            // re-applied each frame from a stable origin —
                            // not delta-on-current, which would compound.
                            #[cfg(feature = "v3")]
                            {
                                let _ = (start_translate, start_scale, start_rotate_deg);
                                let new_grid = match mode {
                                    DragMode::Translate => {
                                        translated_grid(&start_warp.grid, dx, dy)
                                    }
                                    DragMode::Scale => {
                                        // Same gesture math as the v4 Effect-
                                        // Transform path: cumulative drag
                                        // delta along x+y, floored at 0.05
                                        // so the quad can't collapse.
                                        let factor = (1.0 + (dx + dy)).max(0.05);
                                        scaled_grid(&start_warp.grid, factor)
                                    }
                                    DragMode::Rotate => {
                                        // Cumulative drag along x → 360° spin.
                                        let theta_rad = (dx * 360.0).to_radians();
                                        rotated_grid(&start_warp.grid, theta_rad)
                                    }
                                };
                                layer.warp.grid = new_grid;
                            }
                            // v2 path preserved: writes Effect::Transform
                            // through the legacy helper. v2 has no undo.
                            #[cfg(not(feature = "v3"))]
                            match mode {
                                DragMode::Translate => {
                                    let new_t = [start_translate[0] + dx, start_translate[1] + dy];
                                    mutate_transform_effect(layer, |t, _r, _sx, _sy| {
                                        *t = new_t;
                                    });
                                }
                                DragMode::Scale => {
                                    let factor = (1.0 + (dx + dy)).max(0.05);
                                    let new_sx = start_scale[0] * factor;
                                    let new_sy = start_scale[1] * factor;
                                    mutate_transform_effect(layer, |_t, _r, sx, sy| {
                                        *sx = Modulator::Static(new_sx);
                                        *sy = Modulator::Static(new_sy);
                                    });
                                }
                                DragMode::Rotate => {
                                    let new_rot = start_rotate_deg + dx * 360.0;
                                    mutate_transform_effect(layer, |_t, r, _sx, _sy| {
                                        *r = Modulator::Static(new_rot);
                                    });
                                }
                            }
                        }
                    }
                }
                DragKind::MaskVertex {
                    warp,
                    idx,
                    start_pos,
                } => {
                    if let Some(layer) = project.layers.get_mut(*warp) {
                        if let Some(p) = layer.warp.mask_polygon.get_mut(*idx) {
                            p[0] = (start_pos[0] + dx).clamp(0.0, 1.0);
                            p[1] = (start_pos[1] + dy).clamp(0.0, 1.0);
                        }
                    }
                }
                DragKind::WarpCorner {
                    layer_idx,
                    r,
                    c,
                    start_pos,
                } => {
                    if let Some(layer) = project.layers.get_mut(*layer_idx) {
                        if let Some(row) = layer.warp.grid.get_mut(*r) {
                            if let Some(p) = row.get_mut(*c) {
                                // Warp corners may sit outside [0, 1]
                                // intentionally (oversized projection
                                // surfaces); leave the drag delta
                                // unclamped.
                                p[0] = start_pos[0] + dx;
                                p[1] = start_pos[1] + dy;
                            }
                        }
                    }
                }
            }
        }
    }

    if response.drag_stopped() {
        // 003-T3.29 — Layer-mode Translate / Scale / Rotate drags emit
        // a single `ResetLayerWarpMesh` covering the full cumulative
        // delta as a snapshot Reverse (rule 3 — full WarpMesh snap).
        // The live drag has mutated `layer.warp.grid`; we revert before
        // emit so `apply` sees project state == old at apply time.
        // The drain re-applies `new` in the same frame — no flash.
        //
        // Pre-T3.29 this branch emitted `SetLayerEffects` against
        // `effects_snapshot` (T1.24/25/26's effects-Vec Reverse pattern
        // for Effect::Transform mutations). Under v5 Layer-mode no
        // longer touches Effect::Transform; modulator pickers and the
        // Effects panel still emit `SetLayerEffects` through their own
        // paths, so the canonical effects-Vec Reverse test in
        // command.rs (`effects_vec_reverse_no_stray_transform`) stays
        // load-bearing — just exercises a different code path.
        #[cfg(feature = "v3")]
        if let Some(drag) = scene.drag.as_ref() {
            match &drag.kind {
                DragKind::LayerTransform {
                    mode, start_warp, ..
                } => {
                    if matches!(
                        mode,
                        DragMode::Translate | DragMode::Scale | DragMode::Rotate
                    ) {
                        if let Some(Selection::Layer(layer_idx)) = scene.selected {
                            if let Some(layer) = project.layers.get_mut(layer_idx) {
                                let old = start_warp.clone();
                                let new = layer.warp.clone();
                                // Skip the mutation entirely on a zero-delta
                                // drag (operator clicked without moving) —
                                // ResetLayerWarpMesh's debug_assert only
                                // permits same-state apply when new != old
                                // would also be a no-op. Cleaner to drop it.
                                let same = grids_byte_equal(&old.grid, &new.grid);
                                // Revert live-drag mutation so `apply` sees
                                // project state == old (Reverse-storage rule 3).
                                layer.warp = old.clone();
                                if !same {
                                    emitted = Some(
                                        crate::project::command::Mutation::ResetLayerWarpMesh(
                                            crate::project::command::ResetLayerWarpMesh {
                                                layer_idx,
                                                new,
                                                old,
                                            },
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
                // 003-T1.27 — emit SetMaskVertex for the cumulative drag delta.
                // Revert the live mutation before emission so `apply` sees
                // project state == old (same Reverse-storage pattern as T1.24).
                DragKind::MaskVertex {
                    warp,
                    idx,
                    start_pos,
                } => {
                    if let Some(layer) = project.layers.get_mut(*warp) {
                        if let Some(p) = layer.warp.mask_polygon.get_mut(*idx) {
                            let new = *p;
                            let old = *start_pos;
                            if (new[0] - old[0]).abs() > 1e-6 || (new[1] - old[1]).abs() > 1e-6 {
                                // Revert live mutation; the drain re-applies `new`.
                                *p = old;
                                emitted =
                                    Some(crate::project::command::Mutation::SetLayerMaskVertex(
                                        crate::project::command::SetLayerMaskVertex {
                                            layer_idx: *warp,
                                            idx: *idx,
                                            new,
                                            old,
                                        },
                                    ));
                            }
                        }
                    }
                }
                // 003-T3.5 — emit SetLayerWarpCorner for the cumulative
                // corner-drag delta. Same Reverse-storage pattern as
                // MaskVertex above (rule-3 snapshot Reverse): revert
                // the live mutation, then return the Mutation so the
                // app's drain re-applies it via undo_stack.push.
                //
                // 003-T3.10 — when the released position is within
                // WARP_SNAP_RADIUS_PX of a framebuffer corner, snap to
                // integer coords. Holding Shift bypasses the snap (the
                // operator wants pixel-precise placement).
                DragKind::WarpCorner {
                    layer_idx,
                    r,
                    c,
                    start_pos,
                } => {
                    if let Some(layer) = project.layers.get_mut(*layer_idx) {
                        if let Some(row) = layer.warp.grid.get_mut(*r) {
                            if let Some(p) = row.get_mut(*c) {
                                let mut new = *p;
                                if !modifiers.shift {
                                    if let Some(snap) = snap_corner_target(new, preview_rect) {
                                        new = snap;
                                    }
                                }
                                let old = *start_pos;
                                if (new[0] - old[0]).abs() > 1e-6 || (new[1] - old[1]).abs() > 1e-6
                                {
                                    *p = old;
                                    emitted = Some(
                                        crate::project::command::Mutation::SetLayerWarpCorner(
                                            crate::project::command::SetLayerWarpCorner {
                                                layer_idx: *layer_idx,
                                                r: *r,
                                                c: *c,
                                                new,
                                                old,
                                            },
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        scene.drag = None;
    }

    emitted
}

/// v2 version of `handle_scene_input` — same drag mechanics, no Mutation
/// emission (no undo machinery in v2). Returns `()`.
#[cfg(not(feature = "v3"))]
pub fn handle_scene_input(
    response: &egui::Response,
    project: &mut Project,
    scene: &mut SceneEditorState,
    preview_rect: egui::Rect,
    pointer: Option<Pos2>,
    modifiers: egui::Modifiers,
) {
    if response.drag_started() {
        if let Some(pos) = pointer {
            scene.drag = None;
            if let Some((w_idx, v_idx)) = hit_mask_vertex(project, pos, preview_rect) {
                let start_pos = project.layers[w_idx].warp.mask_polygon[v_idx];
                scene.mode = EditMode::Mask;
                scene.selected = Some(Selection::MaskVertex {
                    warp: w_idx,
                    idx: v_idx,
                });
                scene.drag = Some(DragSession {
                    start_screen: pos,
                    kind: DragKind::MaskVertex {
                        warp: w_idx,
                        idx: v_idx,
                        start_pos,
                    },
                });
            } else if let Some(idx) = hit_layer(project, pos, preview_rect) {
                let (translate, scale, rotate) = effective_static_transform(&project.layers[idx]);
                let mode = if modifiers.shift {
                    DragMode::Scale
                } else if modifiers.alt {
                    DragMode::Rotate
                } else {
                    DragMode::Translate
                };
                scene.selected = Some(Selection::Layer(idx));
                scene.drag = Some(DragSession {
                    start_screen: pos,
                    kind: DragKind::LayerTransform {
                        start_translate: translate,
                        start_scale: scale,
                        start_rotate_deg: rotate,
                        mode,
                    },
                });
            } else {
                scene.selected = None;
            }
        }
    }

    if response.dragged() {
        if let (Some(pos), Some(drag)) = (pointer, scene.drag.as_ref()) {
            let dx = (pos.x - drag.start_screen.x) / preview_rect.width().max(1.0);
            let dy = (pos.y - drag.start_screen.y) / preview_rect.height().max(1.0);
            match &drag.kind {
                DragKind::LayerTransform {
                    start_translate,
                    start_scale,
                    start_rotate_deg,
                    mode,
                } => {
                    if let Some(Selection::Layer(idx)) = scene.selected {
                        if let Some(layer) = project.layers.get_mut(idx) {
                            match mode {
                                DragMode::Translate => {
                                    let new_t = [start_translate[0] + dx, start_translate[1] + dy];
                                    mutate_transform_effect(layer, |t, _r, _sx, _sy| {
                                        *t = new_t;
                                    });
                                }
                                DragMode::Scale => {
                                    let factor = (1.0 + (dx + dy)).max(0.05);
                                    let new_sx = start_scale[0] * factor;
                                    let new_sy = start_scale[1] * factor;
                                    mutate_transform_effect(layer, |_t, _r, sx, sy| {
                                        *sx = Modulator::Static(new_sx);
                                        *sy = Modulator::Static(new_sy);
                                    });
                                }
                                DragMode::Rotate => {
                                    let new_rot = start_rotate_deg + dx * 360.0;
                                    mutate_transform_effect(layer, |_t, r, _sx, _sy| {
                                        *r = Modulator::Static(new_rot);
                                    });
                                }
                            }
                        }
                    }
                }
                DragKind::MaskVertex {
                    warp,
                    idx,
                    start_pos,
                } => {
                    if let Some(layer) = project.layers.get_mut(*warp) {
                        if let Some(p) = layer.warp.mask_polygon.get_mut(*idx) {
                            p[0] = (start_pos[0] + dx).clamp(0.0, 1.0);
                            p[1] = (start_pos[1] + dy).clamp(0.0, 1.0);
                        }
                    }
                }
                // 003-T3.5 — corner drag is v3-only (gated by EditMode);
                // v2 has no entry path so the variant is unreachable
                // here, but the match must stay exhaustive.
                DragKind::WarpCorner { .. } => {}
            }
        }
    }

    if response.drag_stopped() {
        scene.drag = None;
    }
}

// ── T3.8 / T3.9 ─────────────────────────────────────────────────────────────

/// 003-T3.8 — pure text helper for the mode banner. Extracted so it is
/// unit-testable without an egui context.
///
/// `has_layer_selected` is `true` when any `Selection` variant is set
/// (every variant carries a layer index, so `is_some()` is the correct
/// test — see `paint_warp_grid_overlay`'s gate logic).
///
/// v3-only: v2 builds have no `EditMode`, so this helper is gated below
/// the `mode_banner` fn itself.
#[cfg(feature = "v3")]
pub fn mode_banner_copy(mode: EditMode, has_layer_selected: bool) -> &'static str {
    match mode {
        EditMode::Layer => "Drag to move. Shift-drag to scale. Alt-drag to rotate.",
        EditMode::Warp => {
            // 003-T3.29 — copy reflects the warp-as-placement model:
            // each corner is a point in projector space, and dragging
            // moves the layer on the wall directly (not a fine-tune
            // layered on top of a separate transform).
            if has_layer_selected {
                "Drag the corners to position the layer on the wall."
            } else {
                "Select a layer first."
            }
        }
        EditMode::Mask => {
            if has_layer_selected {
                "Drag a vertex. Double-click an edge to insert. Shift-click to delete."
            } else {
                "Select a layer first."
            }
        }
        EditMode::Inspect => "Click anything to inspect.",
    }
}

/// 003-T3.8 — thin instruction strip at the top of the canvas.
///
/// Renders a single low-contrast label with guidance text that varies by
/// `scene.mode`. T4.15: fades the text in over `TRANSITION_MS` when the mode
/// changes. No border; small font; grey text so it doesn't compete with the
/// canvas content. v3-only — v2 has its own static instruction label in
/// `show_scene_tab`.
#[cfg(feature = "v3")]
pub fn mode_banner(ui: &mut egui::Ui, scene: &mut SceneEditorState) {
    use crate::windows::anim::{TRANSITION_MS, animate_bool_to};

    let has_selection = scene.selected.is_some();
    let copy = mode_banner_copy(scene.mode, has_selection);

    // Each mode gets its own animation id so switching modes starts a fresh
    // 0→1 fade automatically — no manual reset needed.
    let anim_id = ui.id().with(("mode_banner_alpha", scene.mode as u8));
    let alpha = animate_bool_to(ui, anim_id, true, TRANSITION_MS);

    let color = TEXT_SECONDARY.linear_multiply(alpha);
    ui.label(egui::RichText::new(copy).small().color(color));
}

/// 003-T3.9 — map the current `EditMode` to the cursor icon that reflects
/// the user's pending action. Pure helper — easy to test without a GPU.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
pub fn cursor_for_mode(mode: EditMode) -> egui::CursorIcon {
    match mode {
        EditMode::Layer => egui::CursorIcon::Default,
        EditMode::Warp => egui::CursorIcon::Crosshair,
        EditMode::Mask => egui::CursorIcon::Cell,
        EditMode::Inspect => egui::CursorIcon::Default,
    }
}

/// Distinct, deterministic per-layer outline color. Cycles through an
/// 8-entry palette by layer index. The palette is hand-picked for high
/// contrast against typical projection content (mid-grey to dark
/// backgrounds) and against each other on screen.
pub fn layer_color(idx: usize) -> egui::Color32 {
    LAYER_PALETTE[idx % LAYER_PALETTE.len()]
}

/// Paint a colored, rotation-aware outline for every enabled layer.
/// Each layer gets a deterministic per-index color (see [`layer_color`])
/// so the operator can tell which preview rectangle belongs to which
/// layer in the sidebar list. The currently selected layer is drawn
/// thicker so it pops without changing its color.
///
/// The rect math mirrors what the shader does: the unit quad in NDC
/// spans (-1, 1), is scaled by `(scale_x, scale_y)`, rotated around the
/// origin (anchor=0), then translated. Mapped into normalized output-
/// space ([0, 1] with y-down to match egui), the visible center is
/// `0.5 + translate` and each half-extent is `scale/2`.
pub fn paint_layer_outlines(
    project: &Project,
    scene: &SceneEditorState,
    painter: &egui::Painter,
    inner: egui::Rect,
) {
    let to_screen = |n: [f32; 2]| {
        egui::pos2(
            inner.left() + n[0] * inner.width(),
            inner.top() + n[1] * inner.height(),
        )
    };
    for (idx, layer) in project.layers.iter().enumerate() {
        if !layer.enabled {
            continue;
        }
        let (translate, scale, rotate_deg) = effective_static_transform(layer);
        let center = [0.5 + translate[0], 0.5 + translate[1]];
        let half = [scale[0].abs() * 0.5, scale[1].abs() * 0.5];
        // Renderer rotates math-positive (CCW in NDC, y-up). egui screen-y
        // is down, so to draw the same on-screen rotation we flip the
        // sin-axis term — visually a CCW shader rotation appears CCW in
        // the preview as well.
        let rad = rotate_deg.to_radians();
        let cos_r = rad.cos();
        let sin_r = -rad.sin();
        let signs = [(-1.0_f32, -1.0_f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        let corners: [egui::Pos2; 4] = std::array::from_fn(|i| {
            let (sx, sy) = signs[i];
            let lx = sx * half[0];
            let ly = sy * half[1];
            let rx = lx * cos_r - ly * sin_r;
            let ry = lx * sin_r + ly * cos_r;
            to_screen([center[0] + rx, center[1] + ry])
        });
        let selected = matches!(scene.selected, Some(Selection::Layer(i)) if i == idx);
        let color = layer_color(idx);
        let stroke_w = if selected { 2.5 } else { 1.25 };
        let stroke = egui::Stroke::new(stroke_w, color);
        for i in 0..4 {
            painter.line_segment([corners[i], corners[(i + 1) % 4]], stroke);
        }
        // Label sits just above the top-left corner so it never overlaps
        // the rect interior.
        let label_pos = corners[0] - egui::vec2(0.0, 2.0);
        painter.text(
            label_pos,
            egui::Align2::LEFT_BOTTOM,
            &layer.id,
            egui::FontId::proportional(11.0),
            color,
        );
    }
}

/// 003-T3.5 — paint the *selected layer's* warp grid as a faint mesh
/// with a handle dot at every grid intersection. Other layers'
/// grids are intentionally not drawn so the canvas stays scoped to
/// the layer the operator is editing.
///
/// Caller wires this **only** when `scene.mode == EditMode::Warp`
/// and `scene.selected` is `Selection::Layer(idx)` or
/// `Selection::WarpCorner { warp: idx, .. }`.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
pub fn paint_warp_grid_overlay(
    project: &Project,
    layer_idx: usize,
    scene: &SceneEditorState,
    painter: &egui::Painter,
    inner: egui::Rect,
) {
    let Some(layer) = project.layers.get(layer_idx) else {
        return;
    };
    let warp = &layer.warp;
    let to_screen = |n: [f32; 2]| {
        egui::pos2(
            inner.left() + n[0] * inner.width(),
            inner.top() + n[1] * inner.height(),
        )
    };
    let mesh_stroke = egui::Stroke::new(1.0, MESH_LINE);
    // Horizontal grid lines: one per vertex row.
    for row in warp.grid.iter() {
        for pair in row.windows(2) {
            let a = to_screen(pair[0]);
            let b = to_screen(pair[1]);
            painter.line_segment([a, b], mesh_stroke);
        }
    }
    // Vertical grid lines: walk columns.
    let cols = warp.grid.first().map(|r| r.len()).unwrap_or(0);
    for c in 0..cols {
        for r in 0..warp.grid.len().saturating_sub(1) {
            if warp.grid[r].len() <= c || warp.grid[r + 1].len() <= c {
                continue;
            }
            let a = to_screen(warp.grid[r][c]);
            let b = to_screen(warp.grid[r + 1][c]);
            painter.line_segment([a, b], mesh_stroke);
        }
    }
    // Handle dots at every intersection.
    for (r, row) in warp.grid.iter().enumerate() {
        for (c, p) in row.iter().enumerate() {
            let center = to_screen(*p);
            let is_selected = matches!(
                scene.selected,
                Some(Selection::WarpCorner { warp, r: sr, c: sc })
                    if warp == layer_idx && sr == r && sc == c
            );
            let (fill, stroke) = if is_selected {
                (HANDLE_ACTIVE, egui::Stroke::new(2.0, egui::Color32::WHITE))
            } else {
                (HANDLE_DEFAULT, egui::Stroke::new(1.0, HANDLE_OUTLINE))
            };
            painter.circle(center, WARP_HANDLE_DRAW_PX, fill, stroke);
        }
    }
}

/// 003-T3.10 — paint a faint magnetic-zone indicator at each
/// framebuffer corner while a warp-corner drag is live and the
/// dragged corner is within `WARP_SNAP_RADIUS_PX` of one of them. A
/// no-op when the drag isn't a `WarpCorner` or no corner is in range.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
pub fn paint_warp_snap_indicator(
    project: &Project,
    scene: &SceneEditorState,
    painter: &egui::Painter,
    inner: egui::Rect,
) {
    let Some(drag) = scene.drag.as_ref() else {
        return;
    };
    let DragKind::WarpCorner {
        layer_idx, r, c, ..
    } = &drag.kind
    else {
        return;
    };
    let Some(layer) = project.layers.get(*layer_idx) else {
        return;
    };
    let Some(row) = layer.warp.grid.get(*r) else {
        return;
    };
    let Some(p) = row.get(*c) else {
        return;
    };
    let Some(target) = snap_corner_target(*p, inner) else {
        return;
    };
    let to_screen = |n: [f32; 2]| {
        egui::pos2(
            inner.left() + n[0] * inner.width(),
            inner.top() + n[1] * inner.height(),
        )
    };
    let center = to_screen(target);
    painter.circle(
        center,
        WARP_SNAP_RADIUS_PX,
        ACCENT_DIM.linear_multiply(0.3),
        egui::Stroke::new(1.5, ACCENT),
    );
}

/// Paint mask polygon overlays for every warp inside `inner` (the
/// preview rect). Edges + vertex handles are drawn after the texture
/// so they sit on top of the live image.
pub fn paint_mask_overlays(
    project: &Project,
    scene: &SceneEditorState,
    painter: &egui::Painter,
    inner: egui::Rect,
) {
    let to_screen = |n: [f32; 2]| {
        egui::pos2(
            inner.left() + n[0] * inner.width(),
            inner.top() + n[1] * inner.height(),
        )
    };
    let edge_stroke = egui::Stroke::new(1.5, MASK_EDGE);
    for (w_idx, layer) in project.layers.iter().enumerate() {
        let warp = &layer.warp;
        let n = warp.mask_polygon.len();
        if n < 2 {
            continue;
        }
        for i in 0..n {
            let a = to_screen(warp.mask_polygon[i]);
            let b = to_screen(warp.mask_polygon[(i + 1) % n]);
            painter.line_segment([a, b], edge_stroke);
        }
        for (v_idx, p) in warp.mask_polygon.iter().enumerate() {
            let center = to_screen(*p);
            let is_selected = matches!(
                scene.selected,
                Some(Selection::MaskVertex { warp, idx })
                    if warp == w_idx && idx == v_idx
            );
            let (fill, stroke) = if is_selected {
                (HANDLE_ACTIVE, egui::Stroke::new(2.0, egui::Color32::WHITE))
            } else {
                (MASK_EDGE, egui::Stroke::new(1.0, HANDLE_OUTLINE))
            };
            painter.circle(center, MASK_HANDLE_DRAW_PX, fill, stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{BlendMode, LayerConfig, LayerKind, Transform2D, WarpMesh};
    use std::path::PathBuf;

    /// Build a layer whose Effect::Transform produces the given static
    /// translate / scale. Hit-testing reads the chain, so the static
    /// `LayerConfig.transform` field is left at its (unused) default.
    fn dummy_layer(id: &str, translate: [f32; 2], scale: [f32; 2]) -> LayerConfig {
        LayerConfig {
            id: id.into(),
            kind: LayerKind::Svg {
                svg_path: PathBuf::from("/tmp/x.svg"),
            },
            enabled: true,
            transform: Transform2D::default(),
            effects: vec![Effect::Transform {
                translate,
                rotate_deg: Modulator::Static(0.0),
                scale_x: Modulator::Static(scale[0]),
                scale_y: Modulator::Static(scale[1]),
            }],
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
            warp: WarpMesh::identity(),
            muted: false,
            treatment: None,
        }
    }

    /// Click at the center of the preview hits a layer that covers the
    /// whole frame (translate=0, scale=1).
    #[test]
    fn hit_layer_center_full_frame() {
        let mut project = Project::default();
        project
            .layers
            .push(dummy_layer("a", [0.0, 0.0], [1.0, 1.0]));
        let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0));
        let pos = egui::pos2(50.0, 50.0);
        assert_eq!(hit_layer(&project, pos, rect), Some(0));
    }

    /// Topmost (last) of two stacked layers wins.
    #[test]
    fn hit_layer_top_of_stack_wins() {
        let mut project = Project::default();
        project
            .layers
            .push(dummy_layer("bottom", [0.0, 0.0], [1.0, 1.0]));
        project
            .layers
            .push(dummy_layer("top", [0.0, 0.0], [0.5, 0.5]));
        let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0));
        // Center hits both; we should select the top layer.
        let pos = egui::pos2(50.0, 50.0);
        assert_eq!(hit_layer(&project, pos, rect), Some(1));
        // Edge hits only bottom (top is half-size).
        let pos_edge = egui::pos2(10.0, 50.0);
        assert_eq!(hit_layer(&project, pos_edge, rect), Some(0));
    }

    #[test]
    fn hit_layer_skips_disabled() {
        let mut project = Project::default();
        let mut top = dummy_layer("top", [0.0, 0.0], [1.0, 1.0]);
        top.enabled = false;
        project
            .layers
            .push(dummy_layer("bottom", [0.0, 0.0], [1.0, 1.0]));
        project.layers.push(top);
        let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0));
        assert_eq!(hit_layer(&project, egui::pos2(50.0, 50.0), rect), Some(0));
    }

    /// `mutate_transform_effect` appends a default Transform effect when the
    /// chain has none, so a fresh hand-edited project where the operator
    /// removed the default Transform still works for drag.
    #[test]
    fn mutate_transform_effect_appends_when_missing() {
        let mut layer = LayerConfig {
            id: "a".into(),
            kind: LayerKind::Svg {
                svg_path: PathBuf::from("/tmp/x.svg"),
            },
            enabled: true,
            transform: Transform2D::default(),
            effects: Vec::new(), // No Transform effect.
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
            warp: WarpMesh::identity(),
            muted: false,
            treatment: None,
        };
        mutate_transform_effect(&mut layer, |t, _r, _sx, _sy| {
            *t = [0.25, 0.0];
        });
        assert_eq!(layer.effects.len(), 1);
        match &layer.effects[0] {
            Effect::Transform { translate, .. } => assert_eq!(*translate, [0.25, 0.0]),
            other => panic!("expected Transform, got {other:?}"),
        }
    }

    #[test]
    fn effective_static_transform_reads_chain() {
        let layer = dummy_layer("a", [0.1, 0.2], [0.5, 0.5]);
        let (t, s, r) = effective_static_transform(&layer);
        assert_eq!(t, [0.1, 0.2]);
        assert_eq!(s, [0.5, 0.5]);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn screen_to_normalized_round_trip() {
        let rect = egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(110.0, 220.0));
        let n = screen_to_normalized(egui::pos2(60.0, 120.0), rect).expect("inside");
        assert!((n[0] - 0.5).abs() < 1e-4);
        assert!((n[1] - 0.5).abs() < 1e-4);
        // Outside the rect returns None.
        assert!(screen_to_normalized(egui::pos2(0.0, 0.0), rect).is_none());
    }

    // --- 003-T3.8: mode_banner_copy tests ---

    /// Warp mode with no layer selected must return "Select a layer first."
    #[cfg(feature = "v3")]
    #[test]
    fn mode_banner_copy_warp_no_layer_returns_select_first() {
        assert_eq!(
            super::mode_banner_copy(EditMode::Warp, false),
            "Select a layer first."
        );
    }

    /// Warp mode with a layer selected must return the drag instruction.
    /// 003-T3.29 — copy updated for the warp-as-placement model.
    #[cfg(feature = "v3")]
    #[test]
    fn mode_banner_copy_warp_with_layer_returns_drag_instruction() {
        assert_eq!(
            super::mode_banner_copy(EditMode::Warp, true),
            "Drag the corners to position the layer on the wall."
        );
    }

    /// Mask mode with no layer selected must also return "Select a layer first."
    #[cfg(feature = "v3")]
    #[test]
    fn mode_banner_copy_mask_no_layer_returns_select_first() {
        assert_eq!(
            super::mode_banner_copy(EditMode::Mask, false),
            "Select a layer first."
        );
    }

    // --- 003-T3.9: cursor_for_mode tests ---

    /// 003-T3.10 — releasing a corner at one of the four framebuffer
    /// corners snaps; releasing far away does not. The snap radius is
    /// in screen pixels, so the same normalized delta snaps on a small
    /// preview but not on a large one (or vice versa).
    #[test]
    fn snap_corner_target_picks_nearest_framebuffer_corner() {
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 1000.0));
        // 0.005 normalized × 1000 px = 5 px, well within 10 px radius.
        assert_eq!(
            super::snap_corner_target([0.005, 0.005], rect),
            Some([0.0, 0.0]),
            "near top-left should snap to (0, 0)"
        );
        assert_eq!(
            super::snap_corner_target([0.995, 0.005], rect),
            Some([1.0, 0.0]),
            "near top-right should snap to (1, 0)"
        );
        assert_eq!(
            super::snap_corner_target([0.5, 0.5], rect),
            None,
            "centre is far from every corner"
        );
        // 0.05 normalized × 1000 px = 50 px, outside the 10 px radius.
        assert_eq!(
            super::snap_corner_target([0.05, 0.05], rect),
            None,
            "50 px from top-left is outside the snap radius"
        );
    }

    /// Exhaustive 4-arm check of the EditMode → CursorIcon mapping.
    #[test]
    fn cursor_for_mode_maps_correctly() {
        use egui::CursorIcon;
        let cases = [
            (EditMode::Layer, CursorIcon::Default),
            (EditMode::Warp, CursorIcon::Crosshair),
            (EditMode::Mask, CursorIcon::Cell),
            (EditMode::Inspect, CursorIcon::Default),
        ];
        for (mode, expected) in cases {
            assert_eq!(
                super::cursor_for_mode(mode),
                expected,
                "cursor_for_mode({mode:?}) should be {expected:?}"
            );
        }
    }

    // --- 003-T3.7: EditMode tests ---

    /// `EditMode::default()` must be `Layer` (the operator starts in layer-
    /// drag mode and only enters Mask/Warp/Inspect explicitly).
    #[test]
    fn edit_mode_default_is_layer() {
        assert_eq!(EditMode::default(), EditMode::Layer);
    }

    /// All four variants must be pairwise distinct so downstream code that
    /// matches on mode cannot silently conflate two states.
    #[test]
    fn edit_mode_variants_are_distinct() {
        let all = [
            EditMode::Layer,
            EditMode::Warp,
            EditMode::Mask,
            EditMode::Inspect,
        ];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b, "same variant must equal itself");
                } else {
                    assert_ne!(a, b, "distinct variants must not be equal");
                }
            }
        }
    }

    /// Touching a mask vertex in `hit_mask_vertex` must auto-switch
    /// `SceneEditorState::mode` to `EditMode::Mask`.
    ///
    /// We drive `hit_mask_vertex` + the mode-switch logic directly rather
    /// than synthesising a full `egui::Response` (which requires a live
    /// egui context). The integration-level smoke test that calls
    /// `handle_scene_input` end-to-end is tracked as T3.26.
    #[test]
    fn selecting_mask_vertex_switches_to_mask_mode() {
        // Build a project with one layer that has a 4-vertex mask polygon
        // in the top-left quadrant of normalized space.
        let mut layer = dummy_layer("a", [0.0, 0.0], [1.0, 1.0]);
        layer.warp.mask_polygon = vec![[0.1, 0.1], [0.4, 0.1], [0.4, 0.4], [0.1, 0.4]];
        let mut project = Project::default();
        project.layers.push(layer);

        let preview_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 200.0));

        // Screen position of the first vertex ([0.1, 0.1] → (20, 20) in the
        // 200×200 preview rect) — well within MASK_HANDLE_HIT_PX.
        let pos = egui::pos2(20.0, 20.0);

        // Confirm hit_mask_vertex finds it.
        let hit = hit_mask_vertex(&project, pos, preview_rect);
        assert_eq!(hit, Some((0, 0)), "expected vertex hit at (0, 0)");

        // Simulate the drag_started branch that sets mode + selected.
        let (w_idx, v_idx) = hit.unwrap();
        let start_pos = project.layers[w_idx].warp.mask_polygon[v_idx];
        let mut scene = SceneEditorState::default();
        assert_eq!(scene.mode, EditMode::Layer, "starts in Layer mode");

        scene.mode = EditMode::Mask;
        scene.selected = Some(Selection::MaskVertex {
            warp: w_idx,
            idx: v_idx,
        });
        scene.drag = Some(DragSession {
            start_screen: pos,
            kind: DragKind::MaskVertex {
                warp: w_idx,
                idx: v_idx,
                start_pos,
            },
        });

        assert_eq!(
            scene.mode,
            EditMode::Mask,
            "mode must be Mask after vertex hit"
        );
        assert_eq!(
            scene.selected,
            Some(Selection::MaskVertex { warp: 0, idx: 0 })
        );
    }

    // 003-T3.29 — Layer-mode drag math now operates on the warp grid.
    // Centroid + translate + scale + rotate helpers each get a tight
    // unit test so the math is pinned independent of the egui layer.
    #[cfg(feature = "v3")]
    fn placement_grid() -> Vec<Vec<[f32; 2]>> {
        vec![
            vec![[0.25, 0.25], [0.75, 0.25]],
            vec![[0.25, 0.75], [0.75, 0.75]],
        ]
    }

    #[cfg(feature = "v3")]
    fn approx2(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
    }

    #[cfg(feature = "v3")]
    #[test]
    fn warp_grid_centroid_of_default_placement_is_screen_center() {
        let g = placement_grid();
        let c = super::warp_grid_centroid(&g);
        assert!(approx2(c, [0.5, 0.5]));
    }

    #[cfg(feature = "v3")]
    #[test]
    fn translated_grid_shifts_every_point() {
        let g = placement_grid();
        let out = super::translated_grid(&g, 0.1, -0.05);
        assert!(approx2(out[0][0], [0.35, 0.20]));
        assert!(approx2(out[1][1], [0.85, 0.70]));
    }

    /// Scale × 2 about (0.5, 0.5) sends (0.25, 0.25) → (0.0, 0.0)
    /// and (0.75, 0.75) → (1.0, 1.0). Centroid unchanged.
    #[cfg(feature = "v3")]
    #[test]
    fn scaled_grid_doubles_about_centroid() {
        let g = placement_grid();
        let out = super::scaled_grid(&g, 2.0);
        assert!(approx2(out[0][0], [0.0, 0.0]));
        assert!(approx2(out[1][1], [1.0, 1.0]));
        let c = super::warp_grid_centroid(&out);
        assert!(approx2(c, [0.5, 0.5]));
    }

    /// 90° rotation about the centroid swaps the corners cyclically.
    /// In y-down screen space, +90° sends top-left (0.25, 0.25) →
    /// top-right (0.75, 0.25) → bottom-right (0.75, 0.75) → ...
    #[cfg(feature = "v3")]
    #[test]
    fn rotated_grid_quarter_turn_cycles_corners() {
        let g = placement_grid();
        let theta = std::f32::consts::FRAC_PI_2; // 90° (y-down)
        let out = super::rotated_grid(&g, theta);
        // Top-left of the new quad should be at the OLD bottom-left.
        // Top-left is out[0][0] (the cell at row 0, col 0).
        assert!(approx2(out[0][0], [0.75, 0.25]));
        assert!(approx2(out[1][1], [0.25, 0.75]));
        let c = super::warp_grid_centroid(&out);
        assert!(approx2(c, [0.5, 0.5]));
    }

    /// 003-T3.29 — zero-delta drag is a byte-equal grid; the drag-stop
    /// path uses `grids_byte_equal` to skip emitting a no-op mutation.
    #[cfg(feature = "v3")]
    #[test]
    fn grids_byte_equal_skips_zero_delta_drags() {
        let a = placement_grid();
        let b = placement_grid();
        assert!(super::grids_byte_equal(&a, &b));
        let c = super::translated_grid(&a, 1e-3, 0.0);
        assert!(!super::grids_byte_equal(&a, &c));
    }
}
