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
use crate::project::schema::{LayerConfig, Project};

/// Pixel radius for mask-vertex hit-testing in preview space (M11).
const MASK_HANDLE_HIT_PX: f32 = 9.0;
/// Pixel radius for the painted mask handle.
const MASK_HANDLE_DRAW_PX: f32 = 5.5;
/// Distance to a mask edge that counts as "double-click on this edge"
/// for the insert-vertex gesture (M11).
const MASK_EDGE_HIT_PX: f32 = 7.0;

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
        /// 003-T1.24 — pre-drag effects Vec snapshot, captured at
        /// drag_started for v3's `SetLayerEffects` Reverse storage
        /// (rule 2). `mutate_transform_effect` may append a default
        /// `Effect::Transform` to layers that don't have one — a
        /// per-field Reverse would leave a stray effect on undo.
        /// T1.24 covers Translate; T1.25 / T1.26 extend to Scale
        /// and Rotate.
        #[cfg(feature = "v3")]
        effects_snapshot: Vec<Effect>,
    },
    /// Mask polygon vertex move (M11). Captures the original normalized
    /// position so live drag is `start + delta_normalized`.
    MaskVertex {
        warp: usize,
        idx: usize,
        start_pos: [f32; 2],
    },
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
    for (w_idx, warp) in project.warps.iter().enumerate() {
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
    for (w_idx, warp) in project.warps.iter().enumerate() {
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
            // Hit-test priority: mask vertices first (small handles, easy
            // to miss), then layer body. Warp corners + source rect land
            // in M11 future work.
            if let Some((w_idx, v_idx)) = hit_mask_vertex(project, pos, preview_rect) {
                let start_pos = project.warps[w_idx].mask_polygon[v_idx];
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
                #[cfg(feature = "v3")]
                let effects_snapshot = project.layers[idx].effects.clone();
                scene.selected = Some(Selection::Layer(idx));
                scene.drag = Some(DragSession {
                    start_screen: pos,
                    kind: DragKind::LayerTransform {
                        start_translate: translate,
                        start_scale: scale,
                        start_rotate_deg: rotate,
                        mode,
                        #[cfg(feature = "v3")]
                        effects_snapshot,
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
                    ..
                } => {
                    if let Some(Selection::Layer(idx)) = scene.selected {
                        if let Some(layer) = project.layers.get_mut(idx) {
                            // Mutate the layer's first Effect::Transform —
                            // the field the M5 render pipeline actually reads.
                            // LayerConfig.transform is currently unused at
                            // render time; mutating it would just make drag
                            // visually do nothing.
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
                    if let Some(w) = project.warps.get_mut(*warp) {
                        if let Some(p) = w.mask_polygon.get_mut(*idx) {
                            p[0] = (start_pos[0] + dx).clamp(0.0, 1.0);
                            p[1] = (start_pos[1] + dy).clamp(0.0, 1.0);
                        }
                    }
                }
            }
        }
    }

    if response.drag_stopped() {
        // 003-T1.24 — emit a single SetLayerEffects covering the full
        // cumulative translate delta. We must revert the live-drag mutation
        // before emitting: `Mutation::SetLayerEffects::apply` debug_asserts
        // that project state == old at apply time, and the drain runs
        // `apply` in the same frame (which re-installs `new`), so no flash.
        // 003-T1.25 — Scale (shift-drag) follows the same pattern.
        // 003-T1.26 — Rotate (alt-drag) too. This is the canonical
        // effects-Vec Reverse case: rotating a layer that has no
        // Transform in its chain triggers `mutate_transform_effect`
        // to APPEND one. A per-field Reverse would leave the appended
        // effect on undo; the whole-Vec snapshot Reverse undoes the
        // append cleanly. The `effects_vec_reverse_no_stray_transform`
        // unit test in `src/project/command.rs` proves this.
        #[cfg(feature = "v3")]
        if let Some(drag) = scene.drag.as_ref() {
            if let DragKind::LayerTransform {
                mode,
                effects_snapshot,
                ..
            } = &drag.kind
            {
                if matches!(
                    mode,
                    DragMode::Translate | DragMode::Scale | DragMode::Rotate
                ) {
                    if let Some(Selection::Layer(layer_idx)) = scene.selected {
                        if let Some(layer) = project.layers.get_mut(layer_idx) {
                            let old = effects_snapshot.clone();
                            let new = layer.effects.clone();
                            // Revert live-drag mutation so `apply` sees
                            // project state == old (Reverse-storage rule 2).
                            layer.effects = old.clone();
                            emitted = Some(crate::project::command::Mutation::SetLayerEffects {
                                layer_idx,
                                new,
                                old,
                            });
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
                let start_pos = project.warps[w_idx].mask_polygon[v_idx];
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
                    if let Some(w) = project.warps.get_mut(*warp) {
                        if let Some(p) = w.mask_polygon.get_mut(*idx) {
                            p[0] = (start_pos[0] + dx).clamp(0.0, 1.0);
                            p[1] = (start_pos[1] + dy).clamp(0.0, 1.0);
                        }
                    }
                }
            }
        }
    }

    if response.drag_stopped() {
        scene.drag = None;
    }
}

/// Distinct, deterministic per-layer outline color. Cycles through an
/// 8-entry palette by layer index. The palette is hand-picked for high
/// contrast against typical projection content (mid-grey to dark
/// backgrounds) and against each other on screen.
pub fn layer_color(idx: usize) -> egui::Color32 {
    const PALETTE: [egui::Color32; 8] = [
        egui::Color32::from_rgb(255, 110, 130), // pink
        egui::Color32::from_rgb(110, 200, 255), // sky
        egui::Color32::from_rgb(180, 240, 130), // lime
        egui::Color32::from_rgb(255, 200, 90),  // amber
        egui::Color32::from_rgb(190, 130, 245), // violet
        egui::Color32::from_rgb(110, 230, 200), // teal
        egui::Color32::from_rgb(245, 150, 80),  // orange
        egui::Color32::from_rgb(180, 180, 220), // grey-violet
    ];
    PALETTE[idx % PALETTE.len()]
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
    let edge_color = egui::Color32::from_rgb(140, 100, 200);
    let edge_stroke = egui::Stroke::new(1.5, edge_color);
    for (w_idx, warp) in project.warps.iter().enumerate() {
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
                (
                    egui::Color32::from_rgb(255, 230, 110),
                    egui::Stroke::new(2.0, egui::Color32::WHITE),
                )
            } else {
                (
                    egui::Color32::from_rgb(180, 130, 220),
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 40)),
                )
            };
            painter.circle(center, MASK_HANDLE_DRAW_PX, fill, stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{BlendMode, LayerConfig, LayerKind, Transform2D};
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
}
