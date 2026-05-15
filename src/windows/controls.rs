//! 003-T3.11–T3.18 + P1.UX — **Controls** window.
//!
//! Floats over the canvas as an `egui::Window` (glossary-style); was a
//! right-edge SidePanel called "Advanced" pre-P1.UX. The rename + layout
//! change happened because the panel grew to contain Master, Display
//! output, every per-layer control, Project, OSC bindings, and
//! Diagnostics — i.e. the operator's primary work surface, not an
//! occasional "advanced disclosure". Toggled by the toolbar's
//! "Controls" button; Esc-inside or the window's close-X dismisses it.
//!
//! Section order:
//!   1. Master — gamma / brightness / contrast
//!   2. Display output — per-output overrides (1 output) or Output panel (≥2 outputs)
//!   3. Selected layer — Transform / Blend / Treatment / Source fit / Video / Effect chain / Placement / Mapping
//!   4. Project — windowed flag + save/load
//!   5. OSC bindings — read-only summary
//!   6. Diagnostics — fps + dropped-frame counters
//!
//! T3.18: sub-section open/closed state persists across window
//! close/reopen because egui's CollapsingHeader keyed by a stable
//! id_source stores its toggle in egui's per-frame memory, which
//! survives widget re-creation on the same widget tree. ScrollArea
//! scroll position persists the same way via `id_salt(SCROLL_ID)`.
//!
//! ## Section-header styling
//!
//! Top-level section headers (Master / Display output / Selected
//! layer / Project / OSC bindings / Diagnostics) and the
//! Selected-layer sub-section headers use the warm `ACCENT` colour
//! via [`section_header`] so they pop against the body text. Mirrors
//! the glossary's "headline" colour treatment so the two surfaces
//! feel like one design system.
//!
//! This module is `#[cfg(feature = "v3")]`-only; see `src/windows/mod.rs`.

use egui::Ui;

use crate::project::schema::{BlendMode, LayerKind, Project, ZoneRole};
use crate::windows::control_panel::{
    ControlPanelAction, ControlPanelState, EffectChange, command_checkbox, command_dragvalue_f32,
    command_dragvalue_u32, command_slider, effect_label, show_effect,
};
use crate::windows::glossary::{GlossaryTerm, glossary_label};
use crate::windows::scene_editor::{
    SceneEditorState, Selection, effective_static_transform, mutate_transform_effect,
};

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
// P1.2.4 — fit-mode + focal-point sub-section inside "Selected layer".
const HDR_SOURCE_FIT: &str = "adv_source_fit";
// P1.UX — Transform + Placement sub-sections (P/S/R, opacity, warp
// summary) — moved from the right-edge Inspector for the v0.5 layout
// consolidation.
const HDR_TRANSFORM: &str = "adv_transform";
const HDR_PLACEMENT: &str = "adv_placement";

/// Build a section-header `RichText` — accent-coloured + strong.
/// Mirrors the glossary's `headline` styling so the two surfaces share
/// a visual vocabulary.
fn section_header(label: &str) -> egui::RichText {
    egui::RichText::new(label)
        .color(crate::windows::theme::ACCENT)
        .strong()
}

/// Render the Controls window body. Called from `control_panel::show`
/// when `st.controls_open` is `true`, inside an
/// `egui::Window::new("Controls")`.
///
/// Returns a `ControlPanelAction` (usually `None`; `RebuildLayers` if a
/// layer add/remove happens, which currently can't originate here but the
/// return type is kept consistent with the rest of the panel API).
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    scene: &SceneEditorState,
    monitor_names: &[String],
    texture_upload_dropped: u64,
    #[cfg(feature = "lighting")] dmx_active: bool,
    #[cfg(feature = "lighting")] dmx_packet_rate: u64,
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
            egui::CollapsingHeader::new(section_header("Master"))
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
                        egui::CollapsingHeader::new(section_header("Display output"))
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
                        egui::CollapsingHeader::new(section_header("Output panel"))
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
            egui::CollapsingHeader::new(section_header("Selected layer"))
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

                    // Header strip: layer id + tiny separator. Mirrors
                    // the cue the right-edge inspector used to give
                    // ("you're editing this layer").
                    ui.strong(project.layers[layer_idx].id.clone());
                    ui.add_space(2.0);

                    // --------------------------------------------------------
                    // P4.6.1 — Scene-aware header: FX preset params above the
                    // fold for FxLayer layers.
                    //
                    // For FxLayer, the operator's first question is "which
                    // preset parameters tune this effect?" — not "where is it
                    // placed?". We render the FX params section first for
                    // FxLayer and fall through to the standard order for all
                    // other layer kinds.
                    // --------------------------------------------------------
                    #[cfg(feature = "v3")]
                    if matches!(project.layers[layer_idx].kind, LayerKind::FxLayer { .. }) {
                        // P2.8.1 / P2.8.4 — preset browser trigger buttons.
                        ui.horizontal(|ui| {
                            if ui.button("Browse presets…").clicked() {
                                st.preset_browser.open_for_layer(layer_idx);
                            }
                            if ui.button("Save as preset…").clicked() {
                                st.preset_browser.open_save_dialog(layer_idx);
                            }
                        });
                        ui.add_space(2.0);

                        egui::CollapsingHeader::new("FX params")
                            .id_salt("p461_fx_params_above_fold")
                            .default_open(true)
                            .show(ui, |ui| {
                                show_fx_params_section(ui, project, st, layer_idx);
                            });
                        ui.add_space(4.0);
                    }

                    // --------------------------------------------------------
                    // P1.UX — Transform (position / scale / rotate / opacity)
                    // moved from the right-edge Inspector. Lives at the top
                    // for non-FxLayer layers; below FX params for FxLayer.
                    // --------------------------------------------------------
                    egui::CollapsingHeader::new("Transform")
                        .id_salt(HDR_TRANSFORM)
                        .default_open(true)
                        .show(ui, |ui| {
                            show_transform_section(ui, project, st, layer_idx);
                        });

                    ui.add_space(4.0);

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
                    // T1.30 — Look chain. Replaces the Treatment picker;
                    // show_look_chain_section is the T1.25-T1.32 body.
                    // --------------------------------------------------------
                    egui::CollapsingHeader::new("Look chain")
                        .id_salt(HDR_TREATMENT)
                        .default_open(true)
                        .show(ui, |ui| {
                            crate::windows::look_chain::show_look_chain_section(ui, project, st, layer_idx);
                        });

                    ui.add_space(4.0);

                    // --------------------------------------------------------
                    // P1.2.4 — Source fit (cover/contain/stretch) + focal
                    // point. Shown for Image and Video layers; both carry
                    // the same fit + focal fields. SVG / FxLayer / NDI
                    // hide this section since they don't have a
                    // resampled raster source.
                    // --------------------------------------------------------
                    let kind_has_fit = matches!(
                        project.layers[layer_idx].kind,
                        LayerKind::Image { .. } | LayerKind::Video { .. }
                    );
                    if kind_has_fit {
                        egui::CollapsingHeader::new("Source fit")
                            .id_salt(HDR_SOURCE_FIT)
                            .default_open(false)
                            .show(ui, |ui| {
                                show_source_fit_section(ui, project, st, layer_idx);
                            });
                        ui.add_space(4.0);
                    }

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
                                let (
                                    cur_speed,
                                    cur_loop_mode,
                                    cur_clip_in,
                                    cur_clip_out,
                                    cur_bpm_lock,
                                ) = match &project.layers[layer_idx].kind {
                                    LayerKind::Video {
                                        speed,
                                        loop_mode,
                                        clip_in,
                                        clip_out,
                                        bpm_lock,
                                        ..
                                    } => (*speed, *loop_mode, *clip_in, *clip_out, *bpm_lock),
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

                                // P1.4.2 — loop mode combobox (replaces the
                                // P0.4.3 seamless-loop checkbox).
                                ui.horizontal(|ui| {
                                    ui.label("Loop mode");
                                    let mut staged: Option<crate::project::schema::LoopMode> = None;
                                    egui::ComboBox::from_id_salt((
                                        "adv_video_loop_mode",
                                        layer_idx,
                                    ))
                                    .selected_text(loop_mode_label(cur_loop_mode))
                                    .show_ui(ui, |ui| {
                                        for mode in [
                                            crate::project::schema::LoopMode::Loop,
                                            crate::project::schema::LoopMode::Once,
                                            crate::project::schema::LoopMode::PingPong,
                                        ] {
                                            let resp = ui.selectable_label(
                                                cur_loop_mode == mode,
                                                loop_mode_label(mode),
                                            );
                                            // PCleanup.5.2 — PingPong currently
                                            // falls back to forward Loop because
                                            // reverse H.264 decode needs the
                                            // I-frame cache that Phase 7 will
                                            // add. Hover-tip the picker entry
                                            // so an operator doesn't have to
                                            // dig through specs to understand
                                            // why selecting it produced a
                                            // plain Loop.
                                            let resp = if matches!(
                                                mode,
                                                crate::project::schema::LoopMode::PingPong
                                            ) {
                                                resp.on_hover_text(
                                                    "Reverse playback isn't \
                                                     implemented yet — selecting \
                                                     PingPong currently plays \
                                                     forward at the configured \
                                                     speed, then loops. Real \
                                                     ping-pong needs the I-frame \
                                                     cache landing in Phase 7.",
                                                )
                                            } else {
                                                resp
                                            };
                                            if resp.clicked() {
                                                staged = Some(mode);
                                            }
                                        }
                                    });
                                    if let Some(new_mode) = staged {
                                        if new_mode != cur_loop_mode {
                                            st.pending_mutations.push(
                                                project.set_video_loop_mode_mutation(
                                                    layer_idx, new_mode,
                                                ),
                                            );
                                            st.pending_video_controls.push((
                                                layer_idx,
                                                crate::video_layer::VideoControl::SetLoopMode(
                                                    new_mode,
                                                ),
                                            ));
                                        }
                                    }
                                });

                                // P1.4.1 — In / Out points. Two number
                                // inputs with `f32::INFINITY` displayed as
                                // an empty / sentinel value for "no trim".
                                // Edits are clamped (clip_in < clip_out,
                                // clip_in >= 0) before dispatch; invalid
                                // ranges are silently rejected without a
                                // mutation so undo doesn't carry a bad
                                // value forward.
                                ui.add_space(2.0);
                                ui.label("In / Out points (s)");
                                let mut in_edit = cur_clip_in;
                                let mut out_edit_display = if cur_clip_out.is_finite() {
                                    cur_clip_out
                                } else {
                                    0.0
                                };
                                let out_is_sentinel = !cur_clip_out.is_finite();
                                let mut dispatched = false;
                                ui.horizontal(|ui| {
                                    ui.label("In");
                                    let in_resp = ui.add(
                                        egui::DragValue::new(&mut in_edit)
                                            .range(0.0_f32..=3600.0_f32)
                                            .speed(0.05)
                                            .max_decimals(2),
                                    );
                                    ui.label("Out");
                                    let mut out_drag_val = if out_is_sentinel {
                                        f32::INFINITY
                                    } else {
                                        out_edit_display
                                    };
                                    let out_resp = ui.add(
                                        egui::DragValue::new(&mut out_drag_val)
                                            .range(0.0_f32..=3600.0_f32)
                                            .speed(0.05)
                                            .max_decimals(2)
                                            .custom_formatter(|v, _| {
                                                if v.is_infinite() {
                                                    "end".to_string()
                                                } else {
                                                    format!("{v:.2}")
                                                }
                                            }),
                                    );
                                    out_edit_display = out_drag_val;
                                    if (in_resp.drag_stopped() || in_resp.lost_focus())
                                        || (out_resp.drag_stopped() || out_resp.lost_focus())
                                    {
                                        let new_in = in_edit.max(0.0);
                                        let new_out = if out_drag_val.is_finite() {
                                            out_drag_val
                                        } else {
                                            f32::INFINITY
                                        };
                                        let in_changed = (new_in - cur_clip_in).abs() > 1e-4;
                                        let out_changed = !((new_out.is_infinite()
                                            && cur_clip_out.is_infinite())
                                            || (new_out - cur_clip_out).abs() < 1e-4);
                                        let valid = new_in >= 0.0 && new_out > new_in;
                                        if (in_changed || out_changed) && valid {
                                            st.pending_mutations.push(
                                                project.set_video_clip_range_mutation(
                                                    layer_idx, new_in, new_out,
                                                ),
                                            );
                                            st.pending_video_controls.push((
                                                layer_idx,
                                                crate::video_layer::VideoControl::SetClipRange {
                                                    clip_in: new_in,
                                                    clip_out: new_out,
                                                },
                                            ));
                                            dispatched = true;
                                        }
                                    }
                                });
                                let _ = dispatched;

                                // P1.4.4 — BPM-lock toggle. When on, the
                                // per-frame dispatch loop in `app.rs`
                                // computes effective speed as
                                // `manual_speed × (current_bpm / 120)`
                                // and sends `VideoControl::SetSpeed` on
                                // BPM change. The schema field is
                                // toggled here; the dispatch lives in
                                // `app.rs`.
                                ui.add_space(2.0);
                                let mut bpm_edit = cur_bpm_lock;
                                if ui
                                    .checkbox(
                                        &mut bpm_edit,
                                        "BPM-lock (scale speed with clock BPM, 120 = identity)",
                                    )
                                    .changed()
                                {
                                    st.pending_mutations.push(
                                        project.set_video_bpm_lock_mutation(layer_idx, bpm_edit),
                                    );
                                    // No VideoControl on toggle — the
                                    // per-frame loop dispatches SetSpeed
                                    // on next BPM tick.
                                }
                            });

                        ui.add_space(4.0);
                    }

                    // --------------------------------------------------------
                    // T3.13 + T3.14 — Effect chain editor (includes modulator picker)
                    // (P4.6.1: FX params moved above Transform for FxLayer layers)
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
                    // P4.6.2 — "Advanced" disclosure for raw layer params
                    // (warp mesh, mask polygon controls, placement).
                    //
                    // Collapsed by default for FxLayer (the operator arrived
                    // via the wizard and the FX params card is the primary
                    // surface). Expanded by default for all other layer kinds
                    // (Image, Video, SVG) to preserve pre-P4.6 UX.
                    // --------------------------------------------------------
                    {
                        let is_fx_layer =
                            matches!(project.layers[layer_idx].kind, LayerKind::FxLayer { .. });
                        egui::CollapsingHeader::new("Advanced")
                            .id_salt("p462_advanced")
                            .default_open(!is_fx_layer)
                            .show(ui, |ui| {
                                // Placement / Warp summary (P1.UX).
                                egui::CollapsingHeader::new("Placement")
                                    .id_salt(HDR_PLACEMENT)
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        show_placement_section(ui, project, layer_idx);
                                    });

                                ui.add_space(4.0);

                                // Mapping: mesh rows/cols + mask feather (T3.15).
                                egui::CollapsingHeader::new("Mapping")
                                    .id_salt(HDR_MAPPING)
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        glossary_label(ui, GlossaryTerm::Warp);
                                        ui.add_space(4.0);
                                        show_layer_mapping(ui, project, st, layer_idx);
                                    });
                            });
                    }
                });

            ui.add_space(4.0);

            // ----------------------------------------------------------------
            // 3. Project — output_windowed + project file save/load (T3.11)
            // ----------------------------------------------------------------
            egui::CollapsingHeader::new(section_header("Project"))
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
            egui::CollapsingHeader::new(section_header("OSC bindings"))
                .id_salt("rmap_osc_bindings_summary")
                .default_open(false)
                .show(ui, |ui| {
                    show_osc_bindings_summary(ui, project);
                });

            ui.add_space(4.0);

            // ----------------------------------------------------------------
            // 5. Diagnostics stub (T3.11) + P0.3.2 dropped-frames counter
            // ----------------------------------------------------------------
            egui::CollapsingHeader::new(section_header("Diagnostics"))
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

                    // P5.9.1 — DMX activity LED.
                    // P5.9.2 — Art-Net packet-rate badge.
                    #[cfg(feature = "lighting")]
                    {
                        ui.horizontal(|ui| {
                            // Activity LED: green when active, grey otherwise.
                            let led_color = if dmx_active {
                                egui::Color32::from_rgb(0x40, 0xc0, 0x40) // green
                            } else {
                                egui::Color32::from_rgb(0x60, 0x60, 0x60) // grey
                            };
                            // Small circle LED.
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 4.0, led_color);
                            let rate_text = format!("DMX: {} pkt/s", dmx_packet_rate);
                            ui.label(&rate_text).on_hover_text(if dmx_active {
                                "DMX Art-Net output active"
                            } else {
                                "DMX Art-Net output inactive"
                            });
                        });
                    }
                });
        });

    ControlPanelAction::None
}

// ---------------------------------------------------------------------------
// Transform + Placement section bodies (P1.UX, lifted from
// `windows::inspector`).
// ---------------------------------------------------------------------------
/// Position / Scale / Rotate (drag-values) + Opacity (slider) for the
/// selected layer. Identical math to the right-edge Inspector's
/// `show_layer` — the transform fields are stored as `Modulator::Static`
/// values inside the layer's first `Effect::Transform`. The effects-Vec
/// Reverse rule applies: snapshot the old vec, mutate, then revert +
/// push the `SetLayerEffects` mutation so the drain reapplies the new
/// vec atomically.
fn show_transform_section(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
) {
    let layer = &project.layers[layer_idx];
    let (translate, scale, rotate) = effective_static_transform(layer);

    ui.label("Position");
    let new_tx = command_dragvalue_f32(ui, "adv_tx", translate[0], -2.0..=2.0, " x");
    let new_ty = command_dragvalue_f32(ui, "adv_ty", translate[1], -2.0..=2.0, " y");

    ui.label("Scale");
    let new_sx = command_dragvalue_f32(ui, "adv_sx", scale[0], 0.05..=8.0, " x");
    let new_sy = command_dragvalue_f32(ui, "adv_sy", scale[1], 0.05..=8.0, " y");

    ui.label("Rotate (deg)");
    let new_rot = command_dragvalue_f32(ui, "adv_rot", rotate, -360.0..=360.0, "°");

    let transform_changed = [new_tx, new_ty, new_sx, new_sy, new_rot]
        .iter()
        .any(|v| v.is_some());
    if transform_changed {
        let final_tx = new_tx.unwrap_or(translate[0]);
        let final_ty = new_ty.unwrap_or(translate[1]);
        let final_sx = new_sx.unwrap_or(scale[0]);
        let final_sy = new_sy.unwrap_or(scale[1]);
        let final_rot = new_rot.unwrap_or(rotate);

        // Effects-Vec Reverse rule: snapshot the whole effects vec
        // before mutating, then revert + emit a SetLayerEffects
        // mutation so the drain reapplies the new vec atomically.
        let old_effects = project.layers[layer_idx].effects.clone();
        mutate_transform_effect(&mut project.layers[layer_idx], |t, r, sx, sy| {
            *t = [final_tx, final_ty];
            *sx = crate::modulators::Modulator::Static(final_sx);
            *sy = crate::modulators::Modulator::Static(final_sy);
            *r = crate::modulators::Modulator::Static(final_rot);
        });
        let new_effects = project.layers[layer_idx].effects.clone();
        project.layers[layer_idx].effects = old_effects.clone();
        st.pending_mutations
            .push(crate::project::command::Mutation::SetLayerEffects(
                crate::project::command::SetLayerEffects {
                    layer_idx,
                    new: new_effects,
                    old: old_effects,
                },
            ));
    }

    ui.add_space(6.0);
    let current_opacity = project.layers[layer_idx].opacity;
    if let Some(new_op) = command_slider(ui, "adv_opacity", "Opacity", current_opacity, 0.0..=1.0) {
        st.pending_mutations
            .push(project.set_layer_opacity_mutation(layer_idx, new_op));
    }
}

/// Placement / Warp read-out + Edit warp / Edit mask buttons.
/// Replaces the right-edge Inspector's identically-named section.
fn show_placement_section(ui: &mut Ui, project: &Project, layer_idx: usize) {
    let warp = &project.layers[layer_idx].warp;
    ui.weak(format!("{}×{} warp grid", warp.rows, warp.cols));
    ui.weak(format!("mask vertices: {}", warp.mask_polygon.len()));
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        if ui.button("Edit warp").clicked() {
            tracing::info!(
                target: "rmap::ux",
                layer_idx,
                event = "advanced_edit_warp_clicked",
                "edit-warp action — wiring to EditMode::Warp is a follow-up",
            );
        }
        if ui.button("Edit mask").clicked() {
            tracing::info!(
                target: "rmap::ux",
                layer_idx,
                event = "advanced_edit_mask_clicked",
                "edit-mask action — wiring to EditMode::Mask is a follow-up",
            );
        }
    });
}

// ---------------------------------------------------------------------------
// FX preset parameter sliders (P2.5.6)
// ---------------------------------------------------------------------------
/// Render per-param sliders for the selected `FxLayer`.
///
/// For each `FxParamDescriptor` from `fx_param_descriptors(preset_id)`:
/// - Renders a slider over `[d.min, d.max]` with the current value from
///   `layer.params.get(d.key).copied().unwrap_or(d.default)`.
/// - On drag-release or focus-loss, runs a pre-flight budget check via
///   `Project::fx_layer_params_over_budget`. If over budget: pushes a
///   warning toast and does NOT dispatch the mutation (slider snaps back
///   on the next frame because the project state is unchanged).
/// - If within budget, dispatches `SetFxLayerParams` via the mutation queue.
///
/// This function is only called when the layer is an FxLayer; callers must
/// ensure that invariant.
#[cfg(feature = "v3")]
fn show_fx_params_section(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
) {
    let (preset_id, current_params) = match &project.layers[layer_idx].kind {
        LayerKind::FxLayer {
            preset_id, params, ..
        } => (preset_id.clone(), params.clone()),
        _ => return, // guard — caller guarantees FxLayer
    };

    let descriptors = crate::render::fx_presets::fx_param_descriptors(preset_id.as_str());
    if descriptors.is_empty() {
        ui.weak("This FX preset has no tunable parameters.");
        return;
    }

    for d in descriptors {
        let cur = current_params.get(d.key).copied().unwrap_or(d.default);
        let mut edit = cur;
        let resp = ui.add(egui::Slider::new(&mut edit, d.min..=d.max).text(d.label));
        // Dispatch only on drag-release / focus-loss to avoid flooding
        // the undo stack with mid-drag ticks — mirrors the Video-speed
        // and Treatment-params patterns.
        if (resp.drag_stopped() || resp.lost_focus()) && (edit - cur).abs() > 1e-6 {
            let mut next_params = current_params.clone();
            next_params.insert(d.key.to_string(), edit);

            // Pre-flight budget check — surface a warning toast and refuse
            // to dispatch rather than calling the mutation which would be a
            // no-op but would still generate an undo-stack entry.
            if let Some((_key, _val, max)) =
                project.fx_layer_params_over_budget(layer_idx, &next_params)
            {
                st.pending_toasts
                    .push(crate::windows::toast::Toast::warn(format!(
                        "Particle count exceeds budget (max: {max})"
                    )));
            } else {
                st.pending_mutations
                    .push(project.set_fx_layer_params_mutation(layer_idx, next_params));
            }
            // Stop iterating after the first drag-release — the next frame
            // will read fresh params. Continuing here would compare later
            // sliders against the now-stale `current_params` clone.
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Effect chain section body (T3.13 + T3.14)
// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Source-fit section body (P1.2.4)
// ---------------------------------------------------------------------------
/// Render the fit-mode picker + focal-point sliders for Image and Video
/// layers. Both carry identical `fit` + `focal` fields; this section
/// edits them through one mutation pair (`SetLayerFit` is not shipped
/// — fit is read-only in the v0.5 UI; an Image fit picker landed in
/// P0.1.2 elsewhere). Focal sliders are visible only for `Cover`
/// (focal is meaningless for Contain / Stretch).
///
/// **Out of scope:** click-to-set focal on a thumbnail preview. The
/// 16:9 preview-thumbnail approach the spec describes needs egui
/// texture registration of the image / video frame, which is the same
/// infra that P1.4.5's thumbnail strip needs — deferred together to
/// Phase 7. Numeric sliders are the v0.5 affordance.
fn show_source_fit_section(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
) {
    let (cur_fit, cur_focal) = match &project.layers[layer_idx].kind {
        crate::project::schema::LayerKind::Image { fit, focal, .. }
        | crate::project::schema::LayerKind::Video { fit, focal, .. } => (*fit, *focal),
        _ => return,
    };

    // Read-only fit display + Cover-only focal sliders.
    ui.horizontal(|ui| {
        ui.label("Fit");
        let label = match cur_fit {
            crate::project::schema::FitMode::Cover => "Cover",
            crate::project::schema::FitMode::Contain => "Contain",
            crate::project::schema::FitMode::Stretch => "Stretch",
        };
        ui.weak(label);
        ui.weak("(set on import for now; click-to-set focal coming in Phase 7)");
    });

    if !matches!(cur_fit, crate::project::schema::FitMode::Cover) {
        ui.add_space(2.0);
        ui.weak("Focal point applies only to `Cover` fit. Switch to Cover to enable.");
        return;
    }

    ui.add_space(4.0);
    ui.label("Focal point (normalised 0-1; 0.5/0.5 = centre)");
    let mut fx = cur_focal[0];
    let mut fy = cur_focal[1];
    let resp_x = ui.add(egui::Slider::new(&mut fx, 0.0_f32..=1.0_f32).text("X"));
    let resp_y = ui.add(egui::Slider::new(&mut fy, 0.0_f32..=1.0_f32).text("Y"));

    let changed = (resp_x.drag_stopped() || resp_x.lost_focus())
        || (resp_y.drag_stopped() || resp_y.lost_focus());
    if changed {
        let new = [fx.clamp(0.0, 1.0), fy.clamp(0.0, 1.0)];
        if (new[0] - cur_focal[0]).abs() > 1e-4 || (new[1] - cur_focal[1]).abs() > 1e-4 {
            st.pending_mutations
                .push(project.set_layer_focal_mutation(layer_idx, new));
        }
    }
}

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
                // 004-T1.16 — Preset.effects is now Vec<EffectNode>; clone directly.
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
        // 004-T1.12 — dereference EffectNode.effect before passing to show_effect/effect_label
        let node = &mut project.layers[layer_idx].effects[idx];
        let effect = &mut node.effect;
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
                // 004-T1.12 — dereference EffectNode.effect
                if let Some(crate::effects::Effect::Transform { translate, .. }) =
                    new.get_mut(effect_idx).map(|n| &mut n.effect)
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

    // P3.4.1 — Zone role palette. Appears below the feather slider, before
    // the vertex list. A ComboBox (closed palette: seven roles + None) lets
    // the operator tag the mask polygon with a semantic zone role.
    // UX constraint: no new top-level pill; this is a sub-mode inside Mask.
    ui.add_space(4.0);
    glossary_label(ui, GlossaryTerm::ZoneTag);
    let cur_zone = project.layers[layer_idx].warp.zone_role;
    let selected_label = match cur_zone {
        None => "None",
        Some(ZoneRole::Window) => "Window",
        Some(ZoneRole::Portal) => "Portal",
        Some(ZoneRole::Void) => "Void",
        Some(ZoneRole::Spill) => "Spill",
        Some(ZoneRole::Edge) => "Edge",
        Some(ZoneRole::Highlight) => "Highlight",
        Some(ZoneRole::LightSource) => "Light Source",
    };
    egui::ComboBox::from_id_salt(("zone_role_picker", layer_idx))
        .selected_text(selected_label)
        .show_ui(ui, |ui| {
            // "None" option — clears the zone tag.
            if ui.selectable_label(cur_zone.is_none(), "None").clicked() && cur_zone.is_some() {
                st.pending_mutations
                    .push(project.set_mask_zone_role_mutation(layer_idx, None));
            }

            // Seven role options with glossary tooltips.
            let roles = [
                (
                    Some(ZoneRole::Window),
                    GlossaryTerm::ZoneRoleWindow,
                    "Window",
                ),
                (
                    Some(ZoneRole::Portal),
                    GlossaryTerm::ZoneRolePortal,
                    "Portal",
                ),
                (Some(ZoneRole::Void), GlossaryTerm::ZoneRoleVoid, "Void"),
                (Some(ZoneRole::Spill), GlossaryTerm::ZoneRoleSpill, "Spill"),
                (Some(ZoneRole::Edge), GlossaryTerm::ZoneRoleEdge, "Edge"),
                (
                    Some(ZoneRole::Highlight),
                    GlossaryTerm::ZoneRoleHighlight,
                    "Highlight",
                ),
                (
                    Some(ZoneRole::LightSource),
                    GlossaryTerm::ZoneRoleLightSource,
                    "Light Source",
                ),
            ];
            for (role, term, label) in roles {
                ui.horizontal(|ui| {
                    if ui.selectable_label(cur_zone == role, label).clicked() && cur_zone != role {
                        st.pending_mutations
                            .push(project.set_mask_zone_role_mutation(layer_idx, role));
                    }
                    glossary_label(ui, term);
                });
            }
        });

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

/// Human-readable label for a [`crate::project::schema::LoopMode`] in the
/// Selected-layer Video combobox.
fn loop_mode_label(m: crate::project::schema::LoopMode) -> &'static str {
    match m {
        crate::project::schema::LoopMode::Once => "Once (stop at end)",
        crate::project::schema::LoopMode::Loop => "Loop (seamless)",
        // P1.4.2 ships PingPong as a forward-only stub — true
        // reverse decode needs the I-frame cache (Phase 7). The
        // label is explicit so the operator isn't surprised when
        // the clip just loops normally.
        crate::project::schema::LoopMode::PingPong => {
            "Ping-pong (currently loops; reverse in Phase 7)"
        }
    }
}

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
        // 004-T1.12 — dereference EffectNode.effect
        for (effect_idx, node) in layer.effects.iter().enumerate() {
            let effect = &node.effect;
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
        Effect::Treatment { .. } => {
            // PCleanup.1.3 — Treatment params are a HashMap<String, f32>
            // (static scalars), not Modulators. Per-treatment slider UX
            // sources its descriptors from
            // `treatments::param_descriptors(id)` instead of this walk.
        }
        Effect::Feedback { decay, .. } => {
            // PCleanup.1.4 — `decay` is the only Modulator field on
            // Feedback. `offset` is a static `[f32; 2]` (similar to
            // Effect::Transform's `translate`).
            out.extend(extract("decay", decay));
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
        // PCleanup.1.3 — per-layer treatment dispatch.
        Effect::Treatment { .. } => "Treatment",
        // PCleanup.1.4 — feedback / trails.
        Effect::Feedback { .. } => "Feedback",
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
