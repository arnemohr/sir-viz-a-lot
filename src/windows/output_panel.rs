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
use crate::windows::control_panel::{ControlPanelAction, ControlPanelState};
use crate::windows::controls::show_rgb_matrix_editor;
use crate::windows::glossary::{GlossaryTerm, glossary_label};
use crate::windows::theme;

#[cfg(feature = "lighting")]
use crate::lighting::fixture::FixtureGroup;
#[cfg(feature = "lighting")]
use crate::project::command::Mutation;

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

    // -------------------------------------------------------------------------
    // 3. Lighting section (P5.8.1 + P5.8.2 + P5.8.3) — behind `feature = "lighting"`
    //
    // A collapsible "Lighting" section docked at the bottom of the Output
    // panel. Phase 5 scope:
    //   - Art-Net destination IP + port text field.
    //   - Fixture group list with per-row: label, universe, base channel,
    //     fixture count, and delete button.
    //   - "+ Add fixture group" button.
    //   - Personality sub-section per row (channel count + role dropdowns).
    //   - Canvas-region UV coordinate pair (P5.8.4 stub: text field, no drag UI).
    // -------------------------------------------------------------------------
    #[cfg(feature = "lighting")]
    {
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        ui.collapsing("Lighting (Art-Net)", |ui| {
            show_lighting_section(ui, project, st);
        });
    }

    ControlPanelAction::None
}

/// P5.8.1-P5.8.4 — Render the Lighting sub-section of the Output panel.
///
/// Called inside a `CollapsingHeader`; gated on `feature = "lighting"`.
#[cfg(feature = "lighting")]
fn show_lighting_section(ui: &mut Ui, project: &mut Project, st: &mut ControlPanelState) {
    // --- Art-Net destination ---
    ui.horizontal(|ui| {
        ui.label("Art-Net dest:");
        let mut dest = project
            .artnet_dest
            .clone()
            .unwrap_or_else(|| "255.255.255.255:6454".to_string());
        let resp = ui.text_edit_singleline(&mut dest);
        if resp.lost_focus() {
            project.artnet_dest = Some(dest);
        }
    });

    ui.add_space(4.0);

    // --- Fixture group list ---
    if project.fixture_groups.is_empty() {
        ui.label(
            egui::RichText::new("No fixture groups — add one below.")
                .color(theme::TEXT_SECONDARY)
                .italics(),
        );
    } else {
        // Collect mutations to emit after the loop (avoid borrow conflict).
        let mut pending: Vec<Mutation> = Vec::new();

        // Iterate by index so we can reference project.fixture_groups[i]
        // without keeping a reference across a mutable call.
        let group_ids: Vec<_> = project.fixture_groups.iter().map(|g| g.id).collect();

        for (i, group_id) in group_ids.iter().copied().enumerate() {
            let group = &project.fixture_groups[i];

            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    // --- Label row ---
                    ui.horizontal(|ui| {
                        ui.label("Label:");
                        let mut label = group.label.clone();
                        if ui.text_edit_singleline(&mut label).lost_focus()
                            && label != group.label
                        {
                            let mut params = crate::lighting::fixture::FixtureGroupParams::from_group(&project.fixture_groups[i]);
                            params.label = label;
                            pending.push(Mutation::SetFixtureGroupParams(
                                crate::project::command::SetFixtureGroupParams::new(&project.fixture_groups[i], params),
                            ));
                        }

                        // Delete button.
                        if ui
                            .button(egui::RichText::new("✕").color(egui::Color32::from_rgb(200, 60, 60)))
                            .on_hover_text("Remove this fixture group")
                            .clicked()
                        {
                            pending.push(Mutation::RemoveFixtureGroup { id: group_id });
                        }
                    });

                    let group = &project.fixture_groups[i];

                    // --- Universe / base channel / fixture count ---
                    ui.horizontal(|ui| {
                        ui.label("Universe:");
                        let mut univ = group.universe_id.as_u16();
                        if ui
                            .add(egui::DragValue::new(&mut univ).range(0u16..=32767))
                            .changed()
                        {
                            let mut params = crate::lighting::fixture::FixtureGroupParams::from_group(&project.fixture_groups[i]);
                            params.universe_id = crate::lighting::universe::UniverseId(univ);
                            pending.push(Mutation::SetFixtureGroupParams(
                                crate::project::command::SetFixtureGroupParams::new(&project.fixture_groups[i], params),
                            ));
                        }

                        ui.label("Base ch:");
                        let mut base = group.base_channel;
                        if ui
                            .add(egui::DragValue::new(&mut base).range(0u8..=255))
                            .changed()
                        {
                            let mut params = crate::lighting::fixture::FixtureGroupParams::from_group(&project.fixture_groups[i]);
                            params.base_channel = base;
                            pending.push(Mutation::SetFixtureGroupParams(
                                crate::project::command::SetFixtureGroupParams::new(&project.fixture_groups[i], params),
                            ));
                        }

                        ui.label("Fixtures:");
                        let mut count = group.fixture_count;
                        if ui
                            .add(egui::DragValue::new(&mut count).range(1u8..=255))
                            .changed()
                        {
                            let mut params = crate::lighting::fixture::FixtureGroupParams::from_group(&project.fixture_groups[i]);
                            params.fixture_count = count;
                            pending.push(Mutation::SetFixtureGroupParams(
                                crate::project::command::SetFixtureGroupParams::new(&project.fixture_groups[i], params),
                            ));
                        }
                    });

                    let group = &project.fixture_groups[i];

                    // --- P5.8.3 — Personality sub-section ---
                    ui.collapsing(
                        format!("Personality: {} ({}ch)", group.personality.label, group.personality.channel_count()),
                        |ui| {
                            use crate::lighting::fixture::ChannelRole;
                            let group = &project.fixture_groups[i];
                            for (ch_idx, role) in group.personality.channels.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("Ch {}:", ch_idx + 1));
                                    let mut sel = role.clone();
                                    egui::ComboBox::from_id_salt(format!("ch_role_{}_{}", i, ch_idx))
                                        .selected_text(channel_role_label(&sel))
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut sel, ChannelRole::Red, "Red");
                                            ui.selectable_value(&mut sel, ChannelRole::Green, "Green");
                                            ui.selectable_value(&mut sel, ChannelRole::Blue, "Blue");
                                        });
                                    if sel != *role {
                                        let mut params = crate::lighting::fixture::FixtureGroupParams::from_group(&project.fixture_groups[i]);
                                        params.personality.channels[ch_idx] = sel;
                                        pending.push(Mutation::SetFixtureGroupParams(
                                            crate::project::command::SetFixtureGroupParams::new(&project.fixture_groups[i], params),
                                        ));
                                    }
                                });
                            }
                        },
                    );

                    let group = &project.fixture_groups[i];

                    // --- P5.8.4 — Canvas region (UV text fields; drag UI is a future enhancement) ---
                    if let crate::lighting::fixture::FixtureSource::CanvasRegion { uv_min, uv_max } = &group.source {
                        ui.collapsing("Canvas region", |ui| {
                            let (uv_min, uv_max) = (*uv_min, *uv_max);
                            ui.horizontal(|ui| {
                                ui.label("UV min:");
                                let mut u0 = uv_min.0;
                                let mut v0 = uv_min.1;
                                let changed_u = ui
                                    .add(egui::DragValue::new(&mut u0).range(0.0f32..=1.0).speed(0.005))
                                    .changed();
                                let changed_v = ui
                                    .add(egui::DragValue::new(&mut v0).range(0.0f32..=1.0).speed(0.005))
                                    .changed();
                                if changed_u || changed_v {
                                    let mut params = crate::lighting::fixture::FixtureGroupParams::from_group(&project.fixture_groups[i]);
                                    params.source = crate::lighting::fixture::FixtureSource::CanvasRegion {
                                        uv_min: (u0, v0),
                                        uv_max,
                                    };
                                    pending.push(Mutation::SetFixtureGroupParams(
                                        crate::project::command::SetFixtureGroupParams::new(&project.fixture_groups[i], params),
                                    ));
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("UV max:");
                                let mut u1 = uv_max.0;
                                let mut v1 = uv_max.1;
                                let changed_u = ui
                                    .add(egui::DragValue::new(&mut u1).range(0.0f32..=1.0).speed(0.005))
                                    .changed();
                                let changed_v = ui
                                    .add(egui::DragValue::new(&mut v1).range(0.0f32..=1.0).speed(0.005))
                                    .changed();
                                if changed_u || changed_v {
                                    let mut params = crate::lighting::fixture::FixtureGroupParams::from_group(&project.fixture_groups[i]);
                                    params.source = crate::lighting::fixture::FixtureSource::CanvasRegion {
                                        uv_min,
                                        uv_max: (u1, v1),
                                    };
                                    pending.push(Mutation::SetFixtureGroupParams(
                                        crate::project::command::SetFixtureGroupParams::new(&project.fixture_groups[i], params),
                                    ));
                                }
                            });
                        });
                    }
                });

            ui.add_space(4.0);
        }

        // Emit accumulated mutations after the loop.
        #[cfg(feature = "v3")]
        st.pending_mutations.extend(pending);
    }

    ui.add_space(4.0);

    // --- "+ Add fixture group" button ---
    if ui.button("+ Add fixture group").clicked() {
        let group = FixtureGroup::new_default();
        #[cfg(feature = "v3")]
        st.pending_mutations
            .push(Mutation::AddFixtureGroup { group });
    }
}

/// Short label for a `ChannelRole` for display in the personality editor.
#[cfg(feature = "lighting")]
fn channel_role_label(role: &crate::lighting::fixture::ChannelRole) -> &'static str {
    use crate::lighting::fixture::ChannelRole;
    match role {
        ChannelRole::Red => "Red",
        ChannelRole::Green => "Green",
        ChannelRole::Blue => "Blue",
        #[allow(unreachable_patterns)]
        _ => "Other",
    }
}
