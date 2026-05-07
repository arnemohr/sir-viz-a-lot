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

use crate::project::schema::Project;

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

/// Drag-time snapshot. The full `Transform2D` is captured at mouse-down so
/// the live drag computes `start + delta` rather than accumulating per-frame
/// deltas (no float drift). `mode` is locked at drag-start based on
/// modifier keys: plain drag = Translate, Shift = Scale, Alt = Rotate.
#[derive(Debug, Clone)]
pub struct DragSession {
    pub start_screen: Pos2,
    pub start_translate: [f32; 2],
    pub start_scale: [f32; 2],
    pub start_rotate_deg: f32,
    pub mode: DragMode,
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

/// Hit-test screen-space `pos` against every layer in `project`, walking
/// reverse draw order so the topmost layer wins. Returns the layer index
/// of the topmost match.
///
/// "Inside" means "the screen-space point falls inside the layer's
/// post-`Transform` axis-aligned rectangle in normalized output space".
/// Modulator-driven Transform components are not factored in: selection
/// is operator-edit-time, not animation-time, so we use the static base
/// translate / scale only. Matches the way every comparable tool
/// (MadMapper, Resolume editors) handles hit-testing.
pub fn hit_layer(
    project: &Project,
    pos_screen: Pos2,
    preview_rect: egui::Rect,
) -> Option<usize> {
    let pos_norm = screen_to_normalized(pos_screen, preview_rect)?;
    for (idx, layer) in project.layers.iter().enumerate().rev() {
        if !layer.enabled {
            continue;
        }
        let t = &layer.transform;
        // Layer's static rect in normalized output space, centered on
        // `0.5 + translate`, sized by `scale`. Translate is in normalized
        // output units (matches the way the runtime transform shader uses it).
        let half = [t.scale[0].abs() * 0.5, t.scale[1].abs() * 0.5];
        let center = [0.5 + t.translate[0], 0.5 + t.translate[1]];
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
/// - Mouse-up: clear `drag` (selection persists).
/// - Click outside any layer: clear selection.
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
            match hit_layer(project, pos, preview_rect) {
                Some(idx) => {
                    let t = &project.layers[idx].transform;
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
                        start_translate: t.translate,
                        start_scale: t.scale,
                        start_rotate_deg: t.rotate_deg,
                        mode,
                    });
                }
                None => scene.selected = None,
            }
        }
    }

    if response.dragged() {
        if let (Some(pos), Some(drag), Some(Selection::Layer(idx))) =
            (pointer, scene.drag.as_ref(), scene.selected)
        {
            if let Some(layer) = project.layers.get_mut(idx) {
                let dx = (pos.x - drag.start_screen.x) / preview_rect.width().max(1.0);
                let dy = (pos.y - drag.start_screen.y) / preview_rect.height().max(1.0);
                match drag.mode {
                    DragMode::Translate => {
                        layer.transform.translate = [
                            drag.start_translate[0] + dx,
                            drag.start_translate[1] + dy,
                        ];
                    }
                    DragMode::Scale => {
                        // Uniform scale by the diagonal magnitude. Sign of dy
                        // (downward = larger; matches operator expectation
                        // that "drag away from the layer" enlarges).
                        let factor = (1.0 + (dx + dy)).max(0.05);
                        layer.transform.scale = [
                            drag.start_scale[0] * factor,
                            drag.start_scale[1] * factor,
                        ];
                    }
                    DragMode::Rotate => {
                        // Horizontal drag distance maps to degrees. 1.0 normalized
                        // (full preview width) = 360°, so a quarter-drag is 90°.
                        layer.transform.rotate_deg = drag.start_rotate_deg + dx * 360.0;
                    }
                }
            }
        }
    }

    if response.drag_stopped() {
        scene.drag = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::{LayerConfig, LayerKind, Transform2D, BlendMode};
    use std::path::PathBuf;

    fn dummy_layer(id: &str, translate: [f32; 2], scale: [f32; 2]) -> LayerConfig {
        LayerConfig {
            id: id.into(),
            kind: LayerKind::Svg {
                svg_path: PathBuf::from("/tmp/x.svg"),
            },
            enabled: true,
            transform: Transform2D {
                translate,
                rotate_deg: 0.0,
                scale,
                anchor: [0.0, 0.0],
            },
            effects: Vec::new(),
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
        }
    }

    /// Click at the center of the preview hits a layer that covers the
    /// whole frame (translate=0, scale=1).
    #[test]
    fn hit_layer_center_full_frame() {
        let mut project = Project::default();
        project.layers.push(dummy_layer("a", [0.0, 0.0], [1.0, 1.0]));
        let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0));
        let pos = egui::pos2(50.0, 50.0);
        assert_eq!(hit_layer(&project, pos, rect), Some(0));
    }

    /// Topmost (last) of two stacked layers wins.
    #[test]
    fn hit_layer_top_of_stack_wins() {
        let mut project = Project::default();
        project.layers.push(dummy_layer("bottom", [0.0, 0.0], [1.0, 1.0]));
        project.layers.push(dummy_layer("top", [0.0, 0.0], [0.5, 0.5]));
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
        project.layers.push(dummy_layer("bottom", [0.0, 0.0], [1.0, 1.0]));
        project.layers.push(top);
        let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0));
        assert_eq!(hit_layer(&project, egui::pos2(50.0, 50.0), rect), Some(0));
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
