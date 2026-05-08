//! 003-T3.11–T3.18 — Advanced disclosure panel.
//!
//! A structured right-edge side panel housing controls that were previously
//! scattered across the legacy v2 tabs.  Opened and closed by the toolbar
//! Advanced button or by pressing Esc while the panel is focused.
//!
//! Section order (per spec):
//!   1. Master   (gamma / brightness / contrast) — default-collapsed
//!   2. Selected layer (effect chain, blend mode, mapping) — default-open
//!   3. Project  (output_windowed, project-file save/load) — default-collapsed
//!   4. Diagnostics — default-collapsed stub
//!
//! T3.18: sub-section open/closed state persists across panel close/reopen
//! because egui's CollapsingHeader keyed by a stable id_source stores its
//! toggle in egui's per-frame memory, which survives widget re-creation on
//! the same widget tree (same egui context / window).  ScrollArea scroll
//! position is persisted the same way via `id_source("adv_scroll")`.
//!
//! This module is `#[cfg(feature = "v3")]`-only; see `src/windows/mod.rs`.

use egui::Ui;

use crate::project::schema::{BlendMode, Project};
use crate::windows::control_panel::{
    ControlPanelAction, ControlPanelState, EffectChange, command_checkbox, command_dragvalue_u32,
    command_slider, effect_label, show_effect,
};
use crate::windows::scene_editor::{SceneEditorState, Selection};

// ---------------------------------------------------------------------------
// T3.18 — stable id_source strings for CollapsingHeader / ScrollArea.
// These constants are pinned so persistence survives renaming the label text.
// ---------------------------------------------------------------------------
const SCROLL_ID: &str = "adv_scroll";
const HDR_MASTER: &str = "adv_master";
const HDR_SELECTED_LAYER: &str = "adv_selected_layer";
const HDR_EFFECT_CHAIN: &str = "adv_effect_chain";
const HDR_BLEND_MODE: &str = "adv_blend_mode";
const HDR_MAPPING: &str = "adv_mapping";
const HDR_PROJECT: &str = "adv_project";
const HDR_DIAGNOSTICS: &str = "adv_diagnostics";

/// Render the Advanced panel body. Called from `control_panel::show` when
/// `st.advanced_open` is `true`, inside a `SidePanel::right("rmap_advanced")`.
///
/// Returns a `ControlPanelAction` (usually `None`; `RebuildLayers` if a
/// layer add/remove happens, which currently can't originate here but the
/// return type is kept consistent with the rest of the panel API).
pub fn show(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    scene: &SceneEditorState,
) -> ControlPanelAction {
    // Sync st.selected_layer from scene.selected so the migrated
    // effects-tab code that reads st.selected_layer stays correct.
    // Both `Layer(idx)` and `WarpCorner { warp: idx, .. }` imply a layer.
    let selected_layer_idx: Option<usize> = match scene.selected {
        Some(Selection::Layer(idx)) => Some(idx),
        Some(Selection::WarpCorner { warp, .. }) => Some(warp),
        _ => None,
    };
    if let Some(idx) = selected_layer_idx {
        if idx < project.layers.len() {
            st.selected_layer = idx;
        }
    }

    egui::ScrollArea::vertical()
        .id_salt(SCROLL_ID)
        .show(ui, |ui| {
            // ----------------------------------------------------------------
            // 1. Master (T3.12) — gamma / brightness / contrast
            // ----------------------------------------------------------------
            egui::CollapsingHeader::new("Master")
                .id_salt(HDR_MASTER)
                .default_open(false)
                .show(ui, |ui| {
                    if let Some(new) =
                        command_slider(ui, "gamma", "gamma", project.gamma, 0.2..=4.0)
                    {
                        st.pending_mutations.push(project.set_gamma_mutation(new));
                    }
                    if let Some(new) = command_slider(
                        ui,
                        "brightness",
                        "brightness",
                        project.brightness,
                        -1.0..=1.0,
                    ) {
                        st.pending_mutations
                            .push(project.set_brightness_mutation(new));
                    }
                    if let Some(new) =
                        command_slider(ui, "contrast", "contrast", project.contrast, 0.0..=4.0)
                    {
                        st.pending_mutations
                            .push(project.set_contrast_mutation(new));
                    }
                });

            ui.add_space(4.0);

            // ----------------------------------------------------------------
            // 2. Selected layer (T3.13 + T3.14 + T3.15 + T3.16) — only
            //    visible when a layer is selected.
            // ----------------------------------------------------------------
            egui::CollapsingHeader::new("Selected layer")
                .id_salt(HDR_SELECTED_LAYER)
                .default_open(true)
                .show(ui, |ui| {
                    let Some(layer_idx) = selected_layer_idx else {
                        ui.label("Select a layer to see per-layer controls.");
                        return;
                    };
                    if layer_idx >= project.layers.len() {
                        ui.label("Layer index out of range.");
                        return;
                    }

                    // --------------------------------------------------------
                    // T3.16 — Blend mode picker
                    // --------------------------------------------------------
                    egui::CollapsingHeader::new("Blend mode")
                        .id_salt(HDR_BLEND_MODE)
                        .default_open(true)
                        .show(ui, |ui| {
                            let current_mode = project.layers[layer_idx].blend_mode;
                            let mut staged: Option<BlendMode> = None;
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt(("adv_blend", layer_idx))
                                    .selected_text(blend_label(current_mode))
                                    .show_ui(ui, |ui| {
                                        for mode in [
                                            BlendMode::Normal,
                                            BlendMode::Add,
                                            BlendMode::Multiply,
                                            BlendMode::Screen,
                                        ] {
                                            if ui
                                                .selectable_label(
                                                    current_mode == mode,
                                                    blend_label(mode),
                                                )
                                                .clicked()
                                            {
                                                staged = Some(mode);
                                            }
                                        }
                                    });
                            });
                            if let Some(new) = staged {
                                if new != current_mode {
                                    st.pending_mutations.push(
                                        project.set_layer_blend_mode_mutation(layer_idx, new),
                                    );
                                }
                            }
                        });

                    ui.add_space(4.0);

                    // --------------------------------------------------------
                    // T3.13 + T3.14 — Effect chain editor (includes modulator picker)
                    // --------------------------------------------------------
                    egui::CollapsingHeader::new("Effect chain")
                        .id_salt(HDR_EFFECT_CHAIN)
                        .default_open(true)
                        .show(ui, |ui| {
                            show_effect_chain(ui, project, st, layer_idx);
                        });

                    ui.add_space(4.0);

                    // --------------------------------------------------------
                    // T3.15 — Mapping: mesh rows/cols + mask feather
                    // --------------------------------------------------------
                    egui::CollapsingHeader::new("Mapping")
                        .id_salt(HDR_MAPPING)
                        .default_open(false)
                        .show(ui, |ui| {
                            show_layer_mapping(ui, project, st, layer_idx);
                        });
                });

            ui.add_space(4.0);

            // ----------------------------------------------------------------
            // 3. Project — output_windowed + project file save/load (T3.11)
            // ----------------------------------------------------------------
            egui::CollapsingHeader::new("Project")
                .id_salt(HDR_PROJECT)
                .default_open(false)
                .show(ui, |ui| {
                    show_project_section(ui, project, st);
                });

            ui.add_space(4.0);

            // ----------------------------------------------------------------
            // 4. Diagnostics stub (T3.11)
            // ----------------------------------------------------------------
            egui::CollapsingHeader::new("Diagnostics")
                .id_salt(HDR_DIAGNOSTICS)
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("Audit findings and re-runnable checks will appear here.");
                });
        });

    ControlPanelAction::None
}

// ---------------------------------------------------------------------------
// Effect chain section body (T3.13 + T3.14)
// ---------------------------------------------------------------------------
fn show_effect_chain(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
) {
    use crate::project::command::Mutation;

    // Preset picker at top of section.
    if !st.presets_loaded {
        st.presets = crate::windows::control_panel::load_presets_from_disk();
        st.presets_loaded = true;
    }
    ui.horizontal(|ui| {
        ui.label("Preset:");
        if st.presets.is_empty() {
            ui.label("(none — assets/presets/*.json not found)");
        } else {
            st.preset_picker_index = st.preset_picker_index.min(st.presets.len() - 1);
            egui::ComboBox::from_id_salt("adv_preset_pick")
                .selected_text(st.presets[st.preset_picker_index].name.clone())
                .show_ui(ui, |ui| {
                    for (i, preset) in st.presets.iter().enumerate() {
                        if ui
                            .selectable_label(st.preset_picker_index == i, &preset.name)
                            .clicked()
                        {
                            st.preset_picker_index = i;
                        }
                    }
                });
            if ui.button("Apply").clicked() {
                let new = st.presets[st.preset_picker_index].effects.clone();
                st.pending_mutations
                    .push(project.set_layer_effects_mutation(layer_idx, new));
            }
        }
        if ui.button("Reload").clicked() {
            st.presets = crate::windows::control_panel::load_presets_from_disk();
            st.preset_picker_index = 0;
        }
    });

    ui.add_space(4.0);

    let effects_len = project.layers[layer_idx].effects.len();
    if effects_len == 0 {
        ui.label("No effects on this layer.");
        return;
    }

    let mut staged_changes: Vec<(usize, EffectChange)> = Vec::new();
    for idx in 0..effects_len {
        let effect = &mut project.layers[layer_idx].effects[idx];
        egui::CollapsingHeader::new(effect_label(effect))
            .id_salt(("adv_eff", layer_idx, idx))
            .default_open(true)
            .show(ui, |ui| {
                // T3.17: pass inside_advanced = true → show JSON for External
                if let Some(change) = show_effect(ui, idx, effect, true) {
                    staged_changes.push((idx, change));
                }
            });
    }

    if !staged_changes.is_empty() {
        let mut field_changes: Vec<(usize, EffectChange)> = Vec::new();
        for (effect_idx, change) in staged_changes {
            match change {
                EffectChange::ModulatorSwitch {
                    effect_idx: ei,
                    field,
                    new,
                } => {
                    st.pending_mutations
                        .push(project.set_modulator_mutation(layer_idx, ei, field, new));
                }
                other => field_changes.push((effect_idx, other)),
            }
        }
        if !field_changes.is_empty() {
            let old = project.layers[layer_idx].effects.clone();
            let mut new = old.clone();
            for (effect_idx, change) in field_changes {
                if let Some(crate::effects::Effect::Transform { translate, .. }) =
                    new.get_mut(effect_idx)
                {
                    match change {
                        EffectChange::TransformTranslateX(v) => translate[0] = v,
                        EffectChange::TransformTranslateY(v) => translate[1] = v,
                        EffectChange::ModulatorSwitch { .. } => unreachable!(),
                    }
                }
            }
            st.pending_mutations.push(Mutation::SetLayerEffects {
                layer_idx,
                new,
                old,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Mapping sub-section body (T3.15)
// ---------------------------------------------------------------------------
fn show_layer_mapping(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
) {
    let w = &project.layers[layer_idx].warp;
    let cur_rows = w.rows.max(1);
    let cur_cols = w.cols.max(1);

    ui.label("Grid detail");
    ui.horizontal(|ui| {
        let new_rows_opt = command_dragvalue_u32(
            ui,
            &format!("adv_warp_rows_{layer_idx}"),
            cur_rows,
            1..=8u32,
            "rows ",
        );
        let new_cols_opt = command_dragvalue_u32(
            ui,
            &format!("adv_warp_cols_{layer_idx}"),
            cur_cols,
            1..=8u32,
            "cols ",
        );
        if new_rows_opt.is_some() || new_cols_opt.is_some() {
            let new_rows = new_rows_opt.unwrap_or(cur_rows).max(1);
            let new_cols = new_cols_opt.unwrap_or(cur_cols).max(1);
            if new_rows != cur_rows || new_cols != cur_cols {
                // Use the project helper so resample_grid is called once
                // and old values are captured correctly.
                st.pending_mutations.push(
                    project.set_layer_warp_dimensions_mutation(layer_idx, new_rows, new_cols),
                );
            }
        }
        let w = &project.layers[layer_idx].warp;
        ui.label(format!("({} × {} cells)", w.rows, w.cols));
    });

    ui.add_space(4.0);

    // Zone templates + clear
    ui.horizontal(|ui| {
        ui.label("Zone:");
        let old_polygon = project.layers[layer_idx].warp.mask_polygon.clone();
        for (name, build) in crate::project::zone_templates::all_templates() {
            if ui.button(name).clicked() {
                use crate::project::command::Mutation;
                st.pending_mutations.push(Mutation::SetLayerMaskPolygon {
                    layer_idx,
                    new: build(),
                    old: old_polygon.clone(),
                });
            }
        }
        if ui.button("clear mask").clicked() {
            use crate::project::command::Mutation;
            st.pending_mutations.push(Mutation::SetLayerMaskPolygon {
                layer_idx,
                new: Vec::new(),
                old: old_polygon,
            });
        }
    });

    // Mask feather slider
    let w = &project.layers[layer_idx].warp;
    let cur_feather = w.mask_feather;
    if let Some(new) = command_slider(
        ui,
        &format!("adv_mask_feather_{layer_idx}"),
        "mask feather",
        cur_feather,
        0.0..=0.25,
    ) {
        st.pending_mutations
            .push(project.set_layer_mask_feather_mutation(layer_idx, new));
    }

    let w = &project.layers[layer_idx].warp;
    ui.label(format!(
        "mask: {} vertices ({})",
        w.mask_polygon.len(),
        if w.mask_polygon.len() >= 3 {
            "active"
        } else {
            "none — needs ≥ 3 vertices"
        }
    ));
}

// ---------------------------------------------------------------------------
// Project sub-section body (T3.11)
// ---------------------------------------------------------------------------
fn show_project_section(ui: &mut Ui, project: &mut Project, st: &mut ControlPanelState) {
    use std::path::Path;

    if let Some(new) = command_checkbox(ui, "Windowed output", project.output_windowed) {
        st.pending_mutations
            .push(project.set_output_windowed_mutation(new));
    }
    ui.label("Opens a 1280×720 window instead of fullscreen. Restart rmap to apply.");

    ui.add_space(4.0);
    ui.label("Save / load JSON projects (*.rmap.json).");
    ui.horizontal(|ui| {
        let edit = egui::TextEdit::singleline(&mut st.project_save_path)
            .desired_width(260.0)
            .hint_text("my_show.rmap.json");
        let resp = ui.add(edit);
        if resp.changed() {
            st.project_save_message.clear();
        }
        if ui.button("Save").clicked() {
            let trim = st.project_save_path.trim();
            if trim.is_empty() {
                st.project_save_message = "Enter a path ending in .rmap.json".into();
            } else if !trim.ends_with(".rmap.json") {
                st.project_save_message = "Filename should end with .rmap.json".into();
            } else {
                match project.save(Path::new(trim)) {
                    Ok(()) => {
                        st.project_save_message = format!("Saved to {trim}");
                    }
                    Err(e) => {
                        st.project_save_message = format!("Save failed: {e}");
                    }
                }
            }
        }
    });
    if !st.project_save_message.is_empty() {
        ui.label(&st.project_save_message);
    }
}

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

fn blend_label(m: BlendMode) -> &'static str {
    match m {
        BlendMode::Normal => "Normal",
        BlendMode::Add => "Add",
        BlendMode::Multiply => "Multiply",
        BlendMode::Screen => "Screen",
    }
}

// ---------------------------------------------------------------------------
// T3.18 — smoke test: section id_source strings are stable
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    /// T3.18 — pin the id_source constants so a rename of the label text
    /// doesn't silently break egui's open/closed persistence (which is keyed
    /// off the id, not the display string).
    #[test]
    fn section_id_sources_are_stable() {
        assert_eq!(SCROLL_ID, "adv_scroll");
        assert_eq!(HDR_MASTER, "adv_master");
        assert_eq!(HDR_SELECTED_LAYER, "adv_selected_layer");
        assert_eq!(HDR_EFFECT_CHAIN, "adv_effect_chain");
        assert_eq!(HDR_BLEND_MODE, "adv_blend_mode");
        assert_eq!(HDR_MAPPING, "adv_mapping");
        assert_eq!(HDR_PROJECT, "adv_project");
        assert_eq!(HDR_DIAGNOSTICS, "adv_diagnostics");
    }
}
