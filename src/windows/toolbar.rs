//! 003-T3.4: top-of-canvas toolbar rendered under `--features v3`.
//!
//! Left side: project-name placeholder + Undo / Redo buttons (disabled when
//! stacks are empty). Right side: Warp toggle (flips `EditMode` between Layer
//! and Warp), Advanced disclosure toggle (binds to `st.advanced_open`), and a
//! Go-live stub (Phase 4 wires the fullscreen transition).
//!
//! Returns `Some(ControlPanelAction)` when the operator triggers Undo or Redo;
//! returns `None` for all other interactions (mode / disclosure toggles have no
//! action to propagate — they write directly into the mutable state they were
//! handed).

use crate::project::schema::Project;
use crate::windows::control_panel::{ControlPanelAction, ControlPanelInputs, ControlPanelState};
use crate::windows::scene_editor::{EditMode, SceneEditorState};

/// Flip `EditMode` between `Layer` and `Warp`.
///
/// Any mode other than `Warp` (e.g. `Mask`, `Inspect`) is treated as "not
/// warp" — clicking the Warp button takes the operator to `Warp`. Clicking
/// it again returns to `Layer`. The previous non-Warp mode is not remembered;
/// T3.4's button is a binary Layer↔Warp toggle only.
pub fn flip_warp(mode: EditMode) -> EditMode {
    match mode {
        EditMode::Warp => EditMode::Layer,
        _ => EditMode::Warp,
    }
}

/// Render the top-of-canvas toolbar and return any undo/redo action that was
/// requested. Warp / Advanced / Go-live interactions write directly into `scene`
/// and `st` rather than going through `ControlPanelAction`.
pub fn show(
    ui: &mut egui::Ui,
    _project: &Project,
    st: &mut ControlPanelState,
    scene: &mut SceneEditorState,
    inputs: &ControlPanelInputs,
) -> Option<ControlPanelAction> {
    let mut action: Option<ControlPanelAction> = None;
    ui.horizontal(|ui| {
        // --- Left side ---
        // 003-T4.9 / T4.10: project name with dirty indicator.
        let label_text = if inputs.dirty {
            format!("• {}", inputs.project_name)
        } else {
            inputs.project_name.clone()
        };
        ui.label(label_text);
        ui.add_space(12.0);
        if ui
            .add_enabled(inputs.can_undo, egui::Button::new("⟲ Undo"))
            .clicked()
        {
            action = Some(ControlPanelAction::RequestUndo);
        }
        if ui
            .add_enabled(inputs.can_redo, egui::Button::new("⟳ Redo"))
            .clicked()
        {
            action = Some(ControlPanelAction::RequestRedo);
        }
        ui.add_space(8.0);
        // 003-T4.8: Save button — enabled only when dirty (nothing to save
        // otherwise). Save-as is always available.
        if ui
            .add_enabled(inputs.dirty, egui::Button::new("Save"))
            .clicked()
        {
            action = Some(ControlPanelAction::RequestSave);
        }
        if ui.button("Save as\u{2026}").clicked() {
            action = Some(ControlPanelAction::RequestSaveAs);
        }

        // --- Right side --- push remaining widgets to the right edge
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Go-live stub — Phase 4 wires the fullscreen output transition
            let _ = ui.button("Go live");
            ui.add_space(8.0);

            // Advanced disclosure toggle
            ui.toggle_value(&mut st.advanced_open, "Advanced");
            ui.add_space(8.0);

            // Warp toggle: any non-Warp mode ↔ Warp
            let mut is_warp = scene.mode == EditMode::Warp;
            if ui.toggle_value(&mut is_warp, "Warp").clicked() {
                scene.mode = flip_warp(scene.mode);
            }
        });
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 003-T3.4 — `flip_warp` cycles `Layer ↔ Warp`. Starting from `Layer`
    /// arrives at `Warp`; starting from `Warp` returns to `Layer`.
    #[test]
    fn toolbar_warp_button_toggles_mode() {
        assert_eq!(flip_warp(EditMode::Layer), EditMode::Warp);
        assert_eq!(flip_warp(EditMode::Warp), EditMode::Layer);
    }

    /// Non-Layer modes (Mask, Inspect) are also treated as "not Warp" by the
    /// toolbar toggle, so pressing Warp always enters `EditMode::Warp`.
    #[test]
    fn toolbar_warp_button_from_other_modes() {
        assert_eq!(flip_warp(EditMode::Mask), EditMode::Warp);
        assert_eq!(flip_warp(EditMode::Inspect), EditMode::Warp);
    }
}
