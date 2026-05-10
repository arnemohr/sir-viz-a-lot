//! 003-T3.3 — Selection-driven right-edge inspector panel.
//!
//! Appears when `scene_editor.selected.is_some()`. Branch on
//! `Selection` variant to show context-appropriate controls:
//!
//! - `Layer(idx)` — translate / scale / rotate / opacity + mapping sub-section.
//! - `WarpCorner { warp, r, c }` — read-only coords + "Reset this corner" button.
//! - `MaskVertex { warp, idx }` — read-only coords (editing is via canvas drag).
//! - `SourceRect { .. }` — tombstone label (schema v4 removed this variant).
//!
//! Esc anywhere inside the panel clears the selection.
//!
//! This module is `#[cfg(feature = "v3")]`-only; see `src/windows/mod.rs`.

use egui::Ui;

use crate::modulators::Modulator;
use crate::project::command::Mutation;
use crate::project::schema::Project;
use crate::windows::control_panel::{ControlPanelState, command_dragvalue_f32, command_slider};
use crate::windows::scene_editor::{
    SceneEditorState, Selection, effective_static_transform, mutate_transform_effect,
};

/// Render the right-edge inspector panel.
/// Called from `control_panel::show` when `scene.selected.is_some()`.
pub fn show(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    scene: &mut SceneEditorState,
) {
    // Esc clears selection and returns early — nothing else should render.
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        scene.selected = None;
        return;
    }

    let Some(selection) = scene.selected else {
        return;
    };

    match selection {
        Selection::Layer(idx) => show_layer(ui, project, st, scene, idx),
        Selection::WarpCorner { warp, r, c } => show_warp_corner(ui, project, st, warp, r, c),
        Selection::MaskVertex { warp, idx } => show_mask_vertex(ui, project, warp, idx),
        Selection::SourceRect { .. } => {
            ui.label("source_rect was removed in schema v4");
        }
    }
}

// ---------------------------------------------------------------------------
// Layer selection
// ---------------------------------------------------------------------------

fn show_layer(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    scene: &mut SceneEditorState,
    idx: usize,
) {
    let Some(layer) = project.layers.get(idx) else {
        // Layer was removed; clear the stale selection.
        scene.selected = None;
        return;
    };

    ui.strong(layer.id.clone());
    ui.separator();

    // ---- Transform controls ----
    // Read current static values for display.
    let (translate, scale, rotate) = effective_static_transform(layer);

    ui.label("Position");
    let new_tx = command_dragvalue_f32(ui, "inspector_tx", translate[0], -2.0..=2.0, " x");
    let new_ty = command_dragvalue_f32(ui, "inspector_ty", translate[1], -2.0..=2.0, " y");

    ui.label("Scale");
    let new_sx = command_dragvalue_f32(ui, "inspector_sx", scale[0], 0.05..=8.0, " x");
    let new_sy = command_dragvalue_f32(ui, "inspector_sy", scale[1], 0.05..=8.0, " y");

    ui.label("Rotate (deg)");
    let new_rot = command_dragvalue_f32(ui, "inspector_rot", rotate, -360.0..=360.0, "°");

    // If any transform field changed, emit a single SetLayerEffects mutation
    // (rule 2: whole effects-Vec Reverse for anything touching LayerConfig.effects).
    let transform_changed = [new_tx, new_ty, new_sx, new_sy, new_rot]
        .iter()
        .any(|v| v.is_some());
    if transform_changed {
        let final_tx = new_tx.unwrap_or(translate[0]);
        let final_ty = new_ty.unwrap_or(translate[1]);
        let final_sx = new_sx.unwrap_or(scale[0]);
        let final_sy = new_sy.unwrap_or(scale[1]);
        let final_rot = new_rot.unwrap_or(rotate);

        // Snapshot the old effects before mutating (rule 2).
        let old_effects = project.layers[idx].effects.clone();
        mutate_transform_effect(&mut project.layers[idx], |t, r, sx, sy| {
            *t = [final_tx, final_ty];
            *sx = Modulator::Static(final_sx);
            *sy = Modulator::Static(final_sy);
            *r = Modulator::Static(final_rot);
        });
        let new_effects = project.layers[idx].effects.clone();
        // Revert the live mutation; the drain re-applies `new_effects`.
        project.layers[idx].effects = old_effects.clone();
        st.pending_mutations.push(Mutation::SetLayerEffects(
            crate::project::command::SetLayerEffects {
                layer_idx: idx,
                new: new_effects,
                old: old_effects,
            },
        ));
    }

    ui.separator();

    // ---- Opacity ----
    let current_opacity = project.layers[idx].opacity;
    if let Some(new_op) = command_slider(
        ui,
        "inspector_opacity",
        "Opacity",
        current_opacity,
        0.0..=1.0,
    ) {
        st.pending_mutations
            .push(project.set_layer_opacity_mutation(idx, new_op));
    }

    ui.separator();

    // ---- Placement / Warp sub-section (003-T3.29 — warp is the layer's
    // placement on the projector under v5; the heading reflects that).
    ui.strong("Placement / Warp");
    let warp = &project.layers[idx].warp;
    ui.label(format!("{}×{} grid", warp.rows, warp.cols));
    ui.label(format!("mask vertices: {}", warp.mask_polygon.len()));

    ui.horizontal(|ui| {
        if ui.button("Edit warp").clicked() {
            tracing::info!(layer_idx = idx, "T3.7 will wire EditMode::Warp");
        }
        if ui.button("Edit mask").clicked() {
            tracing::info!(layer_idx = idx, "T3.7 will wire EditMode::Mask");
        }
    });

    ui.separator();

    // ---- More link ----
    if ui.small_button("More…").clicked() {
        st.advanced_open = true;
    }
}

// ---------------------------------------------------------------------------
// WarpCorner selection
// ---------------------------------------------------------------------------

fn show_warp_corner(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
    r: usize,
    c: usize,
) {
    ui.strong(format!("Layer {layer_idx}, corner ({r}, {c})"));
    ui.separator();

    let Some(layer) = project.layers.get(layer_idx) else {
        return;
    };
    let Some(row) = layer.warp.grid.get(r) else {
        return;
    };
    let Some(pos) = row.get(c) else {
        return;
    };
    let (x, y) = (pos[0], pos[1]);

    ui.label(format!("x: {x:.4}"));
    ui.label(format!("y: {y:.4}"));

    ui.add_space(4.0);

    if ui.button("Reset this corner").clicked() {
        let rows = layer.warp.rows;
        let cols = layer.warp.cols;
        let u = if cols == 0 {
            0.0
        } else {
            c as f32 / cols as f32
        };
        let v = if rows == 0 {
            0.0
        } else {
            r as f32 / rows as f32
        };
        let identity_position = [u, v];
        let m = project.set_layer_warp_corner_mutation(layer_idx, r, c, identity_position);
        st.pending_mutations.push(m);
    }
}

// ---------------------------------------------------------------------------
// MaskVertex selection
// ---------------------------------------------------------------------------

fn show_mask_vertex(ui: &mut Ui, project: &Project, layer_idx: usize, idx: usize) {
    ui.strong(format!("Layer {layer_idx}, mask vertex {idx}"));
    ui.separator();

    let Some(layer) = project.layers.get(layer_idx) else {
        return;
    };
    let Some(pos) = layer.warp.mask_polygon.get(idx) else {
        return;
    };
    let (x, y) = (pos[0], pos[1]);

    ui.label(format!("x: {x:.4}"));
    ui.label(format!("y: {y:.4}"));
    ui.label("(drag the vertex in the canvas to edit)");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The `Selection::Layer(0)` variant is the one read in `show_layer`.
    /// This test confirms the dispatch shape compiles and the variant
    /// carries the expected index.
    #[test]
    fn selection_layer_variant_round_trips() {
        let sel = Selection::Layer(0);
        assert!(matches!(sel, Selection::Layer(0)));
        match sel {
            Selection::Layer(idx) => assert_eq!(idx, 0),
            _ => panic!("expected Selection::Layer"),
        }
    }

    /// The `Selection::WarpCorner` variant carries the expected warp/r/c.
    #[test]
    fn selection_warp_corner_variant() {
        let sel = Selection::WarpCorner {
            warp: 2,
            r: 1,
            c: 0,
        };
        match sel {
            Selection::WarpCorner { warp, r, c } => {
                assert_eq!(warp, 2);
                assert_eq!(r, 1);
                assert_eq!(c, 0);
            }
            _ => panic!("expected Selection::WarpCorner"),
        }
    }

    /// `Selection::MaskVertex` round-trips.
    #[test]
    fn selection_mask_vertex_variant() {
        let sel = Selection::MaskVertex { warp: 1, idx: 3 };
        match sel {
            Selection::MaskVertex { warp, idx } => {
                assert_eq!(warp, 1);
                assert_eq!(idx, 3);
            }
            _ => panic!("expected Selection::MaskVertex"),
        }
    }

    /// Reset-corner identity formula: for a 1×1 grid (rows=cols=1),
    /// corner (0,0) → [0.0, 0.0], corner (0,1) → [1.0, 0.0], etc.
    #[test]
    fn warp_corner_identity_formula() {
        let rows = 1u32;
        let cols = 1u32;
        let cases = [
            (0usize, 0usize, [0.0f32, 0.0f32]),
            (0, 1, [1.0, 0.0]),
            (1, 0, [0.0, 1.0]),
            (1, 1, [1.0, 1.0]),
        ];
        for (r, c, expected) in cases {
            let u = if cols == 0 {
                0.0
            } else {
                c as f32 / cols as f32
            };
            let v = if rows == 0 {
                0.0
            } else {
                r as f32 / rows as f32
            };
            let pos = [u, v];
            assert_eq!(
                pos, expected,
                "identity mismatch at ({r},{c}): got {pos:?}, expected {expected:?}"
            );
        }
    }
}
