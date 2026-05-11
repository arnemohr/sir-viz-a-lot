//! P0.8.1 — Output panel.
//!
//! Activates when `project.output_targets.len() >= 2`. Renders an
//! edge-blend section at panel level (single shared edge per the v0.4
//! data model — `Project.edge_blend: Option<EdgeBlendConfig>`) plus
//! a sub-card per output target. Each sub-card carries:
//!   * Header: "Output N: {monitor name}"
//!   * Placeholder preview thumbnail (v3.1 widget reuse deferred —
//!     just an empty rect with a label).
//!   * Per-output RGB matrix editor (the 3×3 grid from P0.8.3,
//!     parameterised on output_idx via the extended
//!     `SetOutputRgbMatrix` Mutation).
//!
//! **Per-output gamma / brightness / contrast trims are deferred.**
//! The spec calls them "per-display" but the schema today only carries
//! project-level `Project.{gamma,brightness,contrast}_override:
//! Option<f32>`. True per-output trims need
//! `OutputTarget.{gamma,brightness,contrast}_override: Option<f32>` +
//! a cascading lookup in the gamma render path. Non-breaking schema
//! addition; not yet in scope for v0.4.
//! TODO: P0.8.1 — wire per-output gamma/brightness/contrast overrides
//! when OutputTarget gains those fields (Phase 7).
//!
//! **Edge-blend lives at panel level, not per-sub-card.** v0.4 has one
//! shared edge between outputs[0] and outputs[1]. Phase 7 generalises
//! to per-edge / per-output topology; until then a single panel-level
//! control matches the data model. The placement rationale:
//! `Project.edge_blend: Option<EdgeBlendConfig>` is a single shared
//! config — there is no `outputs[i].edge_blend`. Rendering it at
//! panel level (above the sub-cards) avoids the false impression that
//! it belongs to one particular output.

use egui::Ui;

use crate::project::schema::{EdgeBlendConfig, FalloffCurve, Project};
use crate::windows::advanced::show_rgb_matrix_editor;
use crate::windows::control_panel::{ControlPanelAction, ControlPanelState};
use crate::windows::glossary::{GlossaryTerm, glossary_label};
use crate::windows::theme;

/// Render the Output panel body.
///
/// Called from `advanced::show` when `project.output_targets.len() >= 2`,
/// inside an `egui::CollapsingHeader`. The function is also `pub` so future
/// callers (mode-pill panel, inspector) can embed it directly.
///
/// `monitor_names` is the live list returned by `crate::monitors::list()`
/// (or an empty slice in headless tests). Sub-card headers fall back to
/// `"Display {fallback_index}"` when the list is shorter than the index.
pub fn show(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    monitor_names: &[String],
) -> ControlPanelAction {
    // -------------------------------------------------------------------------
    // 1. Edge-blend section (panel level)
    //
    // v0.4 data model: one shared `Project.edge_blend: Option<EdgeBlendConfig>`
    // between outputs[0] and outputs[1]. Placing this control above the per-
    // output sub-cards makes the panel-level scope visible to the operator.
    // Phase 7 will generalise to per-edge configs at which point this section
    // moves or is duplicated per sub-card.
    // -------------------------------------------------------------------------
    glossary_label(ui, GlossaryTerm::EdgeBlendRegion);

    let edge_blend_enabled = project.edge_blend.is_some();
    let mut new_enabled = edge_blend_enabled;
    if ui.checkbox(&mut new_enabled, "Enable").changed() {
        let new_cfg = if new_enabled {
            Some(EdgeBlendConfig::default())
        } else {
            None
        };
        st.pending_mutations
            .push(project.set_edge_blend_mutation(new_cfg));
    }

    if let Some(cfg) = project.edge_blend {
        ui.add_space(4.0);

        // Overlap width slider (0..=512 px).
        ui.horizontal(|ui| {
            ui.label("Overlap:");
            let mut overlap_px = cfg.overlap_px;
            let resp = ui.add(egui::Slider::new(&mut overlap_px, 0u32..=512).suffix(" px"));
            if (resp.drag_stopped() || resp.lost_focus()) && overlap_px != cfg.overlap_px {
                let new_cfg = EdgeBlendConfig {
                    overlap_px,
                    falloff_curve: cfg.falloff_curve,
                };
                st.pending_mutations
                    .push(project.set_edge_blend_mutation(Some(new_cfg)));
            }
        });

        // Falloff curve picker.
        ui.horizontal(|ui| {
            ui.label("Falloff:");
            let current_label = match cfg.falloff_curve {
                FalloffCurve::Linear => "Linear",
                FalloffCurve::Cosine => "Cosine",
            };
            egui::ComboBox::from_id_salt("output_panel_falloff_curve")
                .selected_text(current_label)
                .show_ui(ui, |ui| {
                    let mut selected = cfg.falloff_curve;
                    ui.selectable_value(&mut selected, FalloffCurve::Linear, "Linear");
                    ui.selectable_value(&mut selected, FalloffCurve::Cosine, "Cosine");
                    if selected != cfg.falloff_curve {
                        let new_cfg = EdgeBlendConfig {
                            overlap_px: cfg.overlap_px,
                            falloff_curve: selected,
                        };
                        st.pending_mutations
                            .push(project.set_edge_blend_mutation(Some(new_cfg)));
                    }
                });
        });
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // -------------------------------------------------------------------------
    // 2. Per-output sub-cards
    //
    // One `egui::Frame::group` per `output_targets[i]`, in index order.
    // Each card contains:
    //   a) Header line: "Output {i}: {monitor name}"
    //   b) Placeholder preview thumbnail (160×90)
    //   c) Per-output RGB matrix editor
    // -------------------------------------------------------------------------
    // Collect a snapshot of target count + fallback indices to avoid
    // a borrow conflict between `project.output_targets` (needed for
    // the header) and `show_rgb_matrix_editor` (takes `&mut Project`).
    let fallback_indices: Vec<usize> = project
        .output_targets
        .iter()
        .map(|t| t.fallback_index)
        .collect();

    for (i, &fallback_index) in fallback_indices.iter().enumerate() {
        let name = monitor_names
            .get(fallback_index)
            .cloned()
            .unwrap_or_else(|| format!("Display {fallback_index}"));

        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                // ---- a) Header --------------------------------------------------
                ui.strong(format!("Output {i}: {name}"));
                ui.add_space(4.0);

                // ---- b) Placeholder preview thumbnail ---------------------------
                // TODO: P0.8.1 placeholder — wire to v3.1 preview-thumbnail
                // widget when it lands.
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(160.0, 90.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 4.0, theme::BG_PANEL);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Preview pending",
                    egui::FontId::proportional(11.0),
                    theme::TEXT_SECONDARY,
                );
                ui.add_space(4.0);

                // ---- c) Per-output RGB matrix editor ----------------------------
                show_rgb_matrix_editor(ui, project, st, i);
            });

        ui.add_space(4.0);
    }

    ControlPanelAction::None
}
