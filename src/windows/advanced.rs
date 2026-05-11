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

use crate::project::schema::{BlendMode, LayerKind, Project};
use crate::windows::control_panel::{
    ControlPanelAction, ControlPanelState, EffectChange, command_checkbox, command_dragvalue_u32,
    command_slider, effect_label, show_effect,
};
use crate::windows::glossary::{GlossaryTerm, glossary_label};
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
// 003-T3.28 — per-display tone override section. Sits between Master and the
// per-layer block so the operator's mental model is "global → display → layer".
const HDR_DISPLAY_OUTPUT: &str = "adv_display_output";
// P0.4.3 — video-specific sub-section inside "Selected layer".
const HDR_VIDEO: &str = "adv_video";
// P1.2.3 — treatment picker sub-section inside "Selected layer".
const HDR_TREATMENT: &str = "adv_treatment";

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
    monitor_names: &[String],
    texture_upload_dropped: u64,
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
                    // T3.21 — glossary labels replace plain text in slider labels.
                    glossary_label(ui, GlossaryTerm::Gamma);
                    if let Some(new) = command_slider(ui, "gamma", "", project.gamma, 0.2..=4.0) {
                        st.pending_mutations.push(project.set_gamma_mutation(new));
                    }
                    glossary_label(ui, GlossaryTerm::Brightness);
                    if let Some(new) =
                        command_slider(ui, "brightness", "", project.brightness, -1.0..=1.0)
                    {
                        st.pending_mutations
                            .push(project.set_brightness_mutation(new));
                    }
                    glossary_label(ui, GlossaryTerm::Contrast);
                    if let Some(new) =
                        command_slider(ui, "contrast", "", project.contrast, 0.0..=4.0)
                    {
                        st.pending_mutations
                            .push(project.set_contrast_mutation(new));
                    }
                });

            ui.add_space(4.0);

            // ----------------------------------------------------------------
            // 1b. Display output / Output panel — branch on projector count.
            //
            // P0.8.1: with ≥2 output targets, the "Display output" section
            // (project-level overrides + primary-output RGB matrix) is
            // replaced by the OutputPanel, which hosts per-output sub-cards
            // (edge-blend, RGB matrix per output). The project-level override
            // sliders are intentionally not rendered in the ≥2 case: the
            // operator's mental model switches to "per-output controls", and
            // the schema doesn't yet carry per-output gamma/brightness/contrast
            // — rendering the project-level sliders here would be confusing.
            // A written TODO in `output_panel::show_display_overrides` covers
            // the schema gap for Phase 7.
            //
            // With exactly 1 output target, the pre-existing "Display output"
            // CollapsingHeader renders unchanged (edge_blend only makes sense
            // with ≥2 outputs, so the edge-blend section is absent here).
            //
            // 0 output targets should not happen (schema invariant), but we
            // defensively render nothing rather than panic or log.
            // ----------------------------------------------------------------
            // P0.7.5: when the toolbar's "Output" toggle is on, the peer
            // OutputPanel SidePanel owns the per-output surface and the
            // Advanced panel skips its duplicate. Avoids egui Grid-ID
            // collisions (`rmap_rgb_matrix_grid_0` would appear in both
            // surfaces simultaneously otherwise).
            #[cfg(feature = "v3")]
            let suppress_per_output = st.output_panel_open;
            #[cfg(not(feature = "v3"))]
            let suppress_per_output = false;

            if !suppress_per_output {
                match project.output_targets.len() {
                    0 => {
                        // Schema invariant violated — skip both surfaces silently.
                    }
                    1 => {
                        // Single projector: existing "Display output" unchanged.
                        egui::CollapsingHeader::new("Display output")
                            .id_salt(HDR_DISPLAY_OUTPUT)
                            .default_open(false)
                            .show(ui, |ui| {
                                glossary_label(ui, GlossaryTerm::DisplayOverride);
                                ui.label(
                                    "Override the master tone for the projector only. The \
                                     control-window preview always shows the pre-gamma image.",
                                );
                                show_display_overrides(ui, project, st);
                            });
                    }
                    _ => {
                        // ≥2 projectors: output panel replaces the single-output
                        // "Display output" CollapsingHeader entirely.
                        egui::CollapsingHeader::new("Output panel")
                            .id_salt("adv_output_panel")
                            .default_open(false)
                            .show(ui, |ui| {
                                crate::windows::output_panel::show(ui, project, st, monitor_names);
                            });
                    }
                }
            }

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
                            // T3.21 — glossary_label for the section's domain term.
                            glossary_label(ui, GlossaryTerm::BlendMode);
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
                    // P1.2.3 — Treatment picker. Sits between Blend mode and
                    // the Video section so the operator's mental model is
                    // "treat then effect". Visible only for Image / Video
                    // layers; SVG / FxLayer get an explanatory placeholder.
                    // --------------------------------------------------------
                    egui::CollapsingHeader::new("Treatment")
                        .id_salt(HDR_TREATMENT)
                        .default_open(false)
                        .show(ui, |ui| {
                            glossary_label(ui, GlossaryTerm::Treatment);
                            ui.add_space(4.0);
                            show_treatment_section(ui, project, st, layer_idx);
                        });

                    ui.add_space(4.0);

                    // --------------------------------------------------------
                    // P0.4.3 — Video-specific controls. Only rendered when the
                    // selected layer is a LayerKind::Video.
                    // --------------------------------------------------------
                    #[cfg(feature = "v3")]
                    if matches!(project.layers[layer_idx].kind, LayerKind::Video { .. }) {
                        egui::CollapsingHeader::new("Video")
                            .id_salt(HDR_VIDEO)
                            .default_open(true)
                            .show(ui, |ui| {
                                let (cur_speed, cur_loop) = match &project.layers[layer_idx].kind {
                                    LayerKind::Video {
                                        speed,
                                        loop_seamless,
                                        ..
                                    } => (*speed, *loop_seamless),
                                    _ => unreachable!(),
                                };

                                // Speed slider — 0.25× to 4.0× covers the
                                // useful operator range. Log scale so 1.0
                                // sits near the midpoint.
                                ui.label("Playback speed");
                                let mut speed_edit = cur_speed;
                                let resp = ui.add(
                                    egui::Slider::new(&mut speed_edit, 0.25_f32..=4.0_f32)
                                        .suffix("×")
                                        .logarithmic(true),
                                );
                                // Dispatch only on drag-release / focus-loss
                                // so the worker is not slammed with mid-drag
                                // control messages.
                                if (resp.drag_stopped() || resp.lost_focus())
                                    && (speed_edit - cur_speed).abs() > 1e-6
                                {
                                    st.pending_mutations.push(
                                        project.set_video_speed_mutation(layer_idx, speed_edit),
                                    );
                                    st.pending_video_controls.push((
                                        layer_idx,
                                        crate::video_layer::VideoControl::SetSpeed(speed_edit),
                                    ));
                                }

                                // Seamless loop toggle.
                                let mut loop_edit = cur_loop;
                                if ui.checkbox(&mut loop_edit, "Seamless loop").changed() {
                                    st.pending_mutations
                                        .push(project.set_video_loop_seamless_mutation(
                                            layer_idx, loop_edit,
                                        ));
                                    st.pending_video_controls.push((
                                        layer_idx,
                                        crate::video_layer::VideoControl::SetLoop(loop_edit),
                                    ));
                                }
                            });

                        ui.add_space(4.0);
                    }

                    // --------------------------------------------------------
                    // T3.13 + T3.14 — Effect chain editor (includes modulator picker)
                    // --------------------------------------------------------
                    egui::CollapsingHeader::new("Effect chain")
                        .id_salt(HDR_EFFECT_CHAIN)
                        .default_open(true)
                        .show(ui, |ui| {
                            // T3.21 — domain-term glossary label at the section top.
                            glossary_label(ui, GlossaryTerm::Effect);
                            ui.add_space(4.0);
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
                            // T3.21 — warp is the core domain term for this section.
                            glossary_label(ui, GlossaryTerm::Warp);
                            ui.add_space(4.0);
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
                    show_project_section(ui, project, st, monitor_names);
                });

            ui.add_space(4.0);

            // ----------------------------------------------------------------
            // 4. OSC bindings summary (P0.2.4, W2.4)
            //
            // Read-only walk of every `Modulator::OscBound` in the
            // project. Add / unbind happens through the parameter-row
            // picker (P0.2.3a-c); this surface is the operator's
            // single-page overview of "what's wired to OSC right now".
            // ----------------------------------------------------------------
            egui::CollapsingHeader::new("OSC bindings")
                .id_salt("rmap_osc_bindings_summary")
                .default_open(false)
                .show(ui, |ui| {
                    show_osc_bindings_summary(ui, project);
                });

            ui.add_space(4.0);

            // ----------------------------------------------------------------
            // 5. Diagnostics stub (T3.11) + P0.3.2 dropped-frames counter
            // ----------------------------------------------------------------
            egui::CollapsingHeader::new("Diagnostics")
                .id_salt(HDR_DIAGNOSTICS)
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("Audit findings and re-runnable checks will appear here.");
                    // P0.3.2 + P1.6.1 — aggregate dropped-frame counter
                    // combining audio (cpal callback overflow) and texture-
                    // upload (video / NDI producer overflow). P0.3.2 wired
                    // audio; P1.6.1 closes the deferred texture-upload half
                    // now that the video worker produces frames.
                    //
                    // The two counters are summed because the operator's
                    // concern is "is the renderer dropping work?", not which
                    // producer. A breakdown is logged via `tracing` already;
                    // showing the split inline would compete for visual
                    // weight in the diagnostics row.
                    let audio_dropped = crate::modulators::audio::dropped_count();
                    let total_dropped = audio_dropped.saturating_add(texture_upload_dropped);
                    ui.horizontal(|ui| {
                        crate::windows::glossary::glossary_label(
                            ui,
                            crate::windows::glossary::GlossaryTerm::DroppedFrames,
                        );
                        if total_dropped == 0 {
                            ui.weak("0");
                        } else {
                            ui.colored_label(
                                egui::Color32::from_rgb(0xc0, 0x80, 0x40),
                                format!("{total_dropped}"),
                            )
                            .on_hover_text(format!(
                                "audio: {audio_dropped} · texture-upload: \
                                 {texture_upload_dropped}",
                            ));
                        }
                    });
                });
        });

    ControlPanelAction::None
}

// ---------------------------------------------------------------------------
// Treatment section body (P1.2.3)
// ---------------------------------------------------------------------------
/// Render the Treatment picker for the selected layer.
///
/// - Image / Video: combobox of registered presets + "None"; per-param
///   sliders for the active preset. Edits dispatch `SetLayerTreatment` /
///   `SetLayerTreatmentParams` on drag-release (matches the Video speed
///   slider pattern so mid-drag jitter does not flood the undo stack).
/// - SVG / FxLayer: explanatory label; no controls. Treatments are an
///   image-grammar concept; FxLayer carries its own preset library.
fn show_treatment_section(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
) {
    let is_image_or_video = matches!(
        project.layers[layer_idx].kind,
        LayerKind::Image { .. } | LayerKind::Video { .. }
    );
    if !is_image_or_video {
        ui.label(
            "Treatments apply to image and video layers; FX layers use their own preset library.",
        );
        return;
    }

    // ----- Preset combobox -----
    let current_preset_id: Option<String> = project.layers[layer_idx]
        .treatment
        .as_ref()
        .map(|t| t.preset_id.clone());
    let registry = crate::render::treatments::registry();

    // Label rendered in the combobox selected slot.
    let current_label: &str = match &current_preset_id {
        None => "None",
        Some(id) => registry
            .iter()
            .find(|(rid, _)| *rid == id.as_str())
            .map(|(_, label)| *label)
            // Unknown preset (hand-edited project) — surface the raw id so
            // the operator notices and the audit hint maps cleanly.
            .unwrap_or(id.as_str()),
    };

    let mut staged_change: Option<Option<String>> = None;
    ui.horizontal(|ui| {
        ui.label("Preset");
        egui::ComboBox::from_id_salt(("adv_treatment_preset", layer_idx))
            .selected_text(current_label)
            .show_ui(ui, |ui| {
                // "None" option — clears the treatment.
                if ui
                    .selectable_label(current_preset_id.is_none(), "None")
                    .clicked()
                    && current_preset_id.is_some()
                {
                    staged_change = Some(None);
                }
                for (preset_id, label) in registry {
                    let is_current = current_preset_id.as_deref() == Some(*preset_id);
                    if ui.selectable_label(is_current, *label).clicked() && !is_current {
                        staged_change = Some(Some((*preset_id).to_string()));
                    }
                }
            });
    });

    if let Some(new_preset) = staged_change {
        // Build new Treatment by carrying over params keys that the new
        // preset documents; missing keys are filled with descriptor
        // defaults. This means switching presets does not lose the
        // operator's earlier slider tweaks for shared parameter names
        // (intentional: identity → tone_map → identity round-trips
        // common params like "exposure").
        let next: Option<crate::project::schema::Treatment> = match new_preset {
            None => None,
            Some(preset_id) => {
                let descriptors = crate::render::treatments::param_descriptors(preset_id.as_str());
                let mut params: std::collections::HashMap<String, f32> =
                    std::collections::HashMap::new();
                let old_params = project.layers[layer_idx]
                    .treatment
                    .as_ref()
                    .map(|t| t.params.clone())
                    .unwrap_or_default();
                for d in descriptors {
                    let v = old_params.get(d.key).copied().unwrap_or(d.default);
                    params.insert(d.key.to_string(), v);
                }
                Some(crate::project::schema::Treatment {
                    preset_id,
                    params,
                    overlay_path: None,
                    collage_paths: Vec::new(),
                })
            }
        };
        st.pending_mutations
            .push(project.set_layer_treatment_mutation(layer_idx, next));
    }

    // ----- Per-param sliders (only when a preset is active) -----
    let preset_id_for_params = project.layers[layer_idx]
        .treatment
        .as_ref()
        .map(|t| t.preset_id.clone());
    if let Some(preset_id) = preset_id_for_params {
        let descriptors = crate::render::treatments::param_descriptors(preset_id.as_str());
        if descriptors.is_empty() {
            ui.add_space(2.0);
            ui.weak("This preset has no tunable parameters.");
        } else {
            ui.add_space(4.0);
            // Read current params HashMap; we'll write a new map on
            // drag-release and dispatch the mutation. Reading via clone
            // keeps the borrow on `project` short.
            let current_params: std::collections::HashMap<String, f32> = project.layers[layer_idx]
                .treatment
                .as_ref()
                .expect("treatment is_some — guarded by preset_id_for_params")
                .params
                .clone();
            for d in descriptors {
                let cur = current_params.get(d.key).copied().unwrap_or(d.default);
                let mut edit = cur;
                let resp = ui.add(egui::Slider::new(&mut edit, d.min..=d.max).text(d.label));
                // Dispatch on drag-release / focus-loss so the mutation
                // history records one undoable step per gesture rather
                // than one per drag tick.
                if (resp.drag_stopped() || resp.lost_focus()) && (edit - cur).abs() > 1e-6 {
                    let mut next_params = current_params.clone();
                    next_params.insert(d.key.to_string(), edit);
                    st.pending_mutations
                        .push(project.set_layer_treatment_params_mutation(layer_idx, next_params));
                    // After dispatch, stop iterating: the next frame
                    // will read the freshly-written params. Continuing
                    // here would compare subsequent sliders against a
                    // stale `current_params` clone.
                    break;
                }
            }
        }
    }
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
                // P0.2.5: pass layer_idx for the MIDI-learn context menu.
                if let Some(change) = show_effect(ui, idx, effect, true, layer_idx) {
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
            st.pending_mutations.push(Mutation::SetLayerEffects(
                crate::project::command::SetLayerEffects {
                    layer_idx,
                    new,
                    old,
                },
            ));
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

    // T3.21 — grid detail is a domain term; use the glossary primitive.
    glossary_label(ui, GlossaryTerm::GridDetail);
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

    // T3.21 — zone templates shape the mask polygon; label with glossary term.
    ui.horizontal(|ui| {
        glossary_label(ui, GlossaryTerm::MaskPolygon);
        ui.label("templates:");
        let old_polygon = project.layers[layer_idx].warp.mask_polygon.clone();
        for (name, build) in crate::project::zone_templates::all_templates() {
            if ui.button(name).clicked() {
                use crate::project::command::Mutation;
                st.pending_mutations.push(Mutation::SetLayerMaskPolygon(
                    crate::project::command::SetLayerMaskPolygon {
                        layer_idx,
                        new: build(),
                        old: old_polygon.clone(),
                    },
                ));
            }
        }
        if ui.button("clear mask").clicked() {
            use crate::project::command::Mutation;
            st.pending_mutations.push(Mutation::SetLayerMaskPolygon(
                crate::project::command::SetLayerMaskPolygon {
                    layer_idx,
                    new: Vec::new(),
                    old: old_polygon,
                },
            ));
        }
    });

    // T3.21 — mask feather is a domain term.
    glossary_label(ui, GlossaryTerm::MaskFeather);
    let w = &project.layers[layer_idx].warp;
    let cur_feather = w.mask_feather;
    if let Some(new) = command_slider(
        ui,
        &format!("adv_mask_feather_{layer_idx}"),
        "",
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
// Display output sub-section body (T3.28)
// ---------------------------------------------------------------------------
/// Render the three override rows. Each row is a checkbox + slider pair:
/// the checkbox toggles `Some(master) ↔ None`; the slider is only
/// interactive when the override is `Some`. Both edits route through the
/// matching `set_project_*_override_mutation` so undo is byte-equal.
fn show_display_overrides(ui: &mut Ui, project: &mut Project, st: &mut ControlPanelState) {
    if let Some(new) = override_row(
        ui,
        "Gamma",
        "adv_display_gamma",
        project.gamma,
        project.gamma_override,
        0.2..=4.0,
    ) {
        st.pending_mutations
            .push(project.set_project_gamma_override_mutation(new));
    }
    if let Some(new) = override_row(
        ui,
        "Brightness",
        "adv_display_brightness",
        project.brightness,
        project.brightness_override,
        -1.0..=1.0,
    ) {
        st.pending_mutations
            .push(project.set_project_brightness_override_mutation(new));
    }
    if let Some(new) = override_row(
        ui,
        "Contrast",
        "adv_display_contrast",
        project.contrast,
        project.contrast_override,
        0.0..=4.0,
    ) {
        st.pending_mutations
            .push(project.set_project_contrast_override_mutation(new));
    }

    // P0.8.3 — per-projector RGB matrix calibration (3×3 grid).
    // `output_idx: 0` targets the primary output; the multi-output path
    // in `output_panel` passes the per-sub-card index instead.
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);
    show_rgb_matrix_editor(ui, project, st, 0);
}

/// 3×3 RGB matrix editor with per-cell spinners + reset-to-identity.
///
/// Parameterised on `output_idx` so it can serve both the single-output
/// surface (index 0, via `show_display_overrides`) and the per-output
/// sub-cards in `output_panel` (index `i`).
///
/// Edits dispatch a `SetOutputRgbMatrix` mutation per change so undo
/// rolls back bit-exact. The card title shows a small dot when the
/// matrix is non-identity so the operator can see at a glance that
/// per-projector colour calibration is active.
pub(crate) fn show_rgb_matrix_editor(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    output_idx: usize,
) {
    use crate::project::schema::rgb_matrix_identity;

    let identity = rgb_matrix_identity();
    let current_matrix = project
        .output_targets
        .get(output_idx)
        .map(|t| t.rgb_matrix)
        .unwrap_or(identity);
    let is_identity = current_matrix == identity;

    ui.horizontal(|ui| {
        glossary_label(ui, GlossaryTerm::RgbMatrix);
        if !is_identity {
            ui.colored_label(egui::Color32::from_rgb(0xd0, 0xa0, 0x40), "●");
        }
    });

    let mut new_matrix = current_matrix;
    let mut changed = false;
    let row_labels = ["R out", "G out", "B out"];
    let col_labels = ["·R", "·G", "·B"];
    egui::Grid::new(format!("rmap_rgb_matrix_grid_{output_idx}"))
        .num_columns(4)
        .spacing([6.0, 4.0])
        .show(ui, |ui| {
            // Header row.
            ui.label("");
            for col in col_labels {
                ui.weak(col);
            }
            ui.end_row();
            for (r, row_label) in row_labels.iter().enumerate() {
                ui.weak(*row_label);
                for cell in new_matrix[r].iter_mut() {
                    let mut v = *cell;
                    let resp = ui.add(
                        egui::DragValue::new(&mut v)
                            .speed(0.005)
                            .range(-2.0..=2.0_f32)
                            .fixed_decimals(3),
                    );
                    if resp.changed() && (v - *cell).abs() > 0.0 {
                        *cell = v;
                        changed = true;
                    }
                }
                ui.end_row();
            }
        });

    if changed && new_matrix != current_matrix {
        st.pending_mutations
            .push(project.set_output_rgb_matrix_mutation(output_idx, new_matrix));
    }

    ui.horizontal(|ui| {
        let reset = ui.add_enabled(!is_identity, egui::Button::new("Reset to identity"));
        if reset.clicked() {
            st.pending_mutations
                .push(project.set_output_rgb_matrix_mutation(output_idx, identity));
        }
        ui.add_enabled(false, egui::Button::new("Calibrate…"))
            .on_disabled_hover_text(
                "Hardware measurement workflow — Phase 7. Edit cells manually \
                 in the meantime.",
            );
    });
}

/// Render one checkbox + slider row. Returns `Some(new_override)` only
/// when the user actually changed something this frame:
///   * checkbox flip on  → `Some(Some(master))` (capture current master).
///   * checkbox flip off → `Some(None)`.
///   * slider drag while enabled → `Some(Some(new_value))`.
fn override_row(
    ui: &mut Ui,
    label: &str,
    id: &str,
    master: f32,
    override_value: Option<f32>,
    range: std::ops::RangeInclusive<f32>,
) -> Option<Option<f32>> {
    let pre_enabled = override_value.is_some();
    let mut enabled = pre_enabled;
    let mut current = override_value.unwrap_or(master);
    let mut staged: Option<Option<f32>> = None;

    ui.horizontal(|ui| {
        if ui.checkbox(&mut enabled, label).changed() {
            staged = Some(if enabled { Some(master) } else { None });
        }
        // `push_id` scopes the slider's auto-generated id to a stable
        // string so toggling the checkbox doesn't relocate the widget
        // and reset its drag state.
        ui.push_id(id, |ui| {
            let resp = ui.add_enabled(pre_enabled, egui::Slider::new(&mut current, range));
            if resp.drag_stopped() && pre_enabled {
                staged = Some(Some(current));
            }
        });
    });

    staged
}

// ---------------------------------------------------------------------------
// Project sub-section body (T3.11)
// ---------------------------------------------------------------------------
fn show_project_section(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    monitor_names: &[String],
) {
    use std::path::Path;

    // 003-T4.11 — show the human-readable name for the current output monitor.
    // `crate::monitors::list()` on macOS resolves `NSScreen::localizedName`
    // (e.g. "BenQ TH685") so the operator can confirm the right display is
    // selected without memorising numeric indices.  Falls back to "monitor N"
    // when the live list is shorter than the stored index (display unplugged).
    let idx = project.primary_output_target().fallback_index;
    let monitor_label = monitor_names
        .get(idx)
        .cloned()
        .unwrap_or_else(|| format!("monitor {idx}"));
    ui.label(format!("Output: {monitor_label}"));

    ui.add_space(4.0);

    // V31.10.4: crossfade duration slider — surfaces the field that previously
    // only had a v2-Scenes-tab UI. The v3 cue strip is the canonical recall
    // surface, so the project-level crossfade knob lives here in the Project
    // section. Crossfade only fires when both scenes share layer topology
    // (same layer paths in the same order); structural changes snap instantly
    // regardless of this setting (see snapshots_share_layer_topology in
    // src/project/mod.rs).
    ui.label("Cue crossfade duration");
    if let Some(new) = command_slider(
        ui,
        "adv_crossfade_duration_s",
        "seconds",
        project.crossfade_duration_s,
        0.0..=5.0,
    ) {
        st.pending_mutations
            .push(project.set_crossfade_duration_s_mutation(new));
    }
    ui.weak(
        "0 = instant snap. Crossfade only fires when both cues share the same \
         layer paths in the same order; structural changes snap instantly.",
    );

    ui.add_space(4.0);

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
// P0.2.4 (W2.4) — OSC bindings summary
// ---------------------------------------------------------------------------

/// Walk every layer's effect chain and render a one-line entry per
/// `Modulator::OscBound` parameter. Columns: address · layer.id ·
/// param-name · live value bar.
///
/// Read-only for v0.4. Operators add / unbind via the parameter-row
/// picker (P0.2.3a-c). Inline editing of the address + a port-config
/// row + a "+ Add binding" button can land in a follow-up commit.
fn show_osc_bindings_summary(ui: &mut Ui, project: &Project) {
    let mut any_binding = false;

    for (layer_idx, layer) in project.layers.iter().enumerate() {
        for (effect_idx, effect) in layer.effects.iter().enumerate() {
            // Walk every Modulator field on every effect type. Each
            // (effect_kind, field_name) pair gets a row when the
            // modulator is `OscBound`.
            let bindings = collect_osc_bindings_in_effect(effect);
            for (field_name, addr, scale, offset) in bindings {
                any_binding = true;
                ui.horizontal(|ui| {
                    ui.monospace(format!("{addr:32}"));
                    ui.weak(format!(
                        "→ layer {layer_idx} ({}) · effect {effect_idx} ({}) · {field_name}",
                        layer.id,
                        effect_kind_label(effect),
                    ));
                    let live = crate::modulators::osc::current_value(addr) * scale + offset;
                    ui.weak(format!("= {live:.3}"));
                });
            }
        }
    }

    if !any_binding {
        ui.weak(
            "No OSC bindings yet. Pick `osc` in any parameter row's binding picker \
             to attach an OSC address.",
        );
    }
}

/// Collect every `(field_name, addr, scale, offset)` for OSC-bound
/// modulators inside a single `Effect`. Field names are the same
/// strings used by `ModulatorField` so the operator can correlate
/// the summary row with the parameter row in Selected-layer.
fn collect_osc_bindings_in_effect(
    effect: &crate::effects::Effect,
) -> Vec<(&'static str, &str, f32, f32)> {
    use crate::effects::Effect;
    use crate::modulators::Modulator;

    fn extract<'a>(
        name: &'static str,
        m: &'a Modulator,
    ) -> Option<(&'static str, &'a str, f32, f32)> {
        if let Modulator::OscBound {
            addr,
            scale,
            offset,
        } = m
        {
            Some((name, addr.as_str(), *scale, *offset))
        } else {
            None
        }
    }

    let mut out: Vec<(&'static str, &str, f32, f32)> = Vec::new();
    match effect {
        Effect::Color {
            hue,
            saturation,
            brightness,
            contrast,
        } => {
            out.extend(extract("hue", hue));
            out.extend(extract("saturation", saturation));
            out.extend(extract("brightness", brightness));
            out.extend(extract("contrast", contrast));
        }
        Effect::Tint { amount, .. } => {
            out.extend(extract("amount", amount));
        }
        Effect::Blur { radius_px } => {
            out.extend(extract("radius_px", radius_px));
        }
        Effect::Transform {
            rotate_deg,
            scale_x,
            scale_y,
            ..
        } => {
            out.extend(extract("rotate_deg", rotate_deg));
            out.extend(extract("scale_x", scale_x));
            out.extend(extract("scale_y", scale_y));
        }
        Effect::External { .. } => {
            // External effects opaque to this walk — their params are
            // a JSON blob, not a Modulator chain.
        }
    }
    out
}

fn effect_kind_label(effect: &crate::effects::Effect) -> &'static str {
    use crate::effects::Effect;
    match effect {
        Effect::Color { .. } => "Color",
        Effect::Tint { .. } => "Tint",
        Effect::Blur { .. } => "Blur",
        Effect::Transform { .. } => "Transform",
        Effect::External { .. } => "External",
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
        assert_eq!(HDR_DISPLAY_OUTPUT, "adv_display_output");
    }
}
