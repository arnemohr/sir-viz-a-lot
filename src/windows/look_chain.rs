//! 004-T1.25–T1.29, T1.32 — Look chain section: unified per-layer effect
//! chain UI. Replaces the T1.24 stub.
//!
//! Gated on `#[cfg(feature = "v3")]` at the module-registration site in
//! `windows/mod.rs`; no inner attribute needed.

use egui::{Color32, Frame, RichText, Sense, Ui, Vec2};

use crate::effects::{Effect, EffectNode, IntentGroup, effect_is_no_op, intent_group};
use crate::modulators::Modulator;
use crate::project::command::{ModulatorField, Mutation};
use crate::project::schema::Project;
use crate::windows::control_panel::{
    ControlPanelState, EffectChange, load_presets_from_disk, modulator_slider,
};

// ---------------------------------------------------------------------------
// T1.25 helpers — intent group → glyph + color
// ---------------------------------------------------------------------------

/// Returns `(emoji_glyph, color)` for the given `IntentGroup`.
pub(crate) fn intent_group_glyph(g: IntentGroup) -> (&'static str, Color32) {
    match g {
        IntentGroup::Warp => ("\u{1F300}", Color32::from_rgb(0x7B, 0x9E, 0xFF)), // 🌀 soft blue
        IntentGroup::Color => ("\u{1F3A8}", Color32::from_rgb(0xFF, 0xA0, 0x60)), // 🎨 amber
        IntentGroup::Texture => ("\u{1F9F1}", Color32::from_rgb(0xA0, 0xC8, 0x80)), // 🧱 sage
        IntentGroup::Compose => ("\u{1F9E9}", Color32::from_rgb(0xCC, 0xCC, 0xCC)), // 🧩 grey
        IntentGroup::Animate => ("\u{1F30A}", Color32::from_rgb(0x80, 0xE0, 0xD0)), // 🌊 teal
        IntentGroup::Generative => ("\u{2728}", Color32::from_rgb(0xE0, 0xC0, 0xFF)), // ✨ lavender
    }
}

/// A full-layer-quad polygon in normalized output-space `[0,1]²`.
///
/// Winding matches `zone_templates::window_rectangle` (clockwise, y-down):
/// top-left → top-right → bottom-right → bottom-left.
pub(crate) fn full_quad_polygon() -> Vec<[f32; 2]> {
    vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
}

// ---------------------------------------------------------------------------
// Effect display name helper (master-detail overhaul)
// ---------------------------------------------------------------------------

/// Human-readable name for displaying an effect in the chain row and detail panel header.
///
/// For non-Treatment variants, returns the variant name.
/// For `Effect::Treatment`, prettifies the preset id: replaces `_` with ` ` and
/// capitalizes the first letter (e.g. `"ripple_lens"` → `"Ripple lens"`).
fn effect_display_name(effect: &Effect) -> String {
    match effect {
        Effect::Color { .. } => "Color".to_string(),
        Effect::Blur { .. } => "Blur".to_string(),
        Effect::Tint { .. } => "Tint".to_string(),
        Effect::Transform { .. } => "Transform".to_string(),
        Effect::Feedback { .. } => "Feedback".to_string(),
        Effect::External { .. } => "External".to_string(),
        Effect::Treatment { id, .. } => {
            // Replace underscores with spaces and capitalize the first letter.
            let with_spaces = id.replace('_', " ");
            let mut chars = with_spaces.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let up: String = first.to_uppercase().collect();
                    up + chars.as_str()
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// T1.26 — expand-body: full param list
// ---------------------------------------------------------------------------

/// Staged treatment param change: (node_idx, key, value).
struct TreatmentParamChange {
    node_idx: usize,
    key: String,
    value: f32,
}

/// Render per-param sliders for a Treatment node.
/// Returns a list of (key, new_value) pairs for params that changed on
/// drag-stop or focus-loss. The caller dispatches SetLayerEffects after the loop.
fn show_treatment_params_read_only(
    ui: &mut Ui,
    id: &str,
    params: &std::collections::HashMap<String, f32>,
    node_idx: usize,
) -> Vec<TreatmentParamChange> {
    let mut changes = Vec::new();
    let descriptors = crate::render::treatments::param_descriptors(id);
    if descriptors.is_empty() {
        ui.weak(format!("'{id}' has no tunable parameters."));
        return changes;
    }
    for d in descriptors {
        let cur = params.get(d.key).copied().unwrap_or(d.default);
        let mut edit = cur;
        let resp = ui.add(egui::Slider::new(&mut edit, d.min..=d.max).text(d.label));
        if (resp.drag_stopped() || resp.lost_focus()) && (edit - cur).abs() > 1e-6 {
            changes.push(TreatmentParamChange {
                node_idx,
                key: d.key.to_string(),
                value: edit,
            });
        }
    }
    changes
}

/// Render sliders for all fields of a non-Treatment Effect.
/// Returns staged `EffectChange`s; also returns treatment param changes.
fn show_effect_full_params(
    ui: &mut Ui,
    effect: &Effect,
    node_idx: usize,
    layer_idx: usize,
    staged: &mut Vec<(usize, EffectChange)>,
    treatment_changes: &mut Vec<TreatmentParamChange>,
) {
    // We take `&Effect` (not `&mut Effect`) to avoid writing to the live
    // project during rendering. Modulator changes are returned as EffectChange;
    // Treatment param changes as TreatmentParamChange. Callers dispatch
    // mutations after the loop.
    //
    // Exception: Transform.translate is a plain [f32;2] — it's staged as
    // EffectChange::TransformTranslate{X,Y} and applied post-loop.
    match effect {
        Effect::Color {
            hue,
            saturation,
            brightness,
            contrast,
        } => {
            let mut hue_m = hue.clone();
            let mut sat_m = saturation.clone();
            let mut bri_m = brightness.clone();
            let mut con_m = contrast.clone();
            if let Some(c) = modulator_slider(
                ui,
                (node_idx, "hue"),
                "hue (deg)",
                &mut hue_m,
                -180.0..=180.0,
                ModulatorField::ColorHue,
                node_idx,
                layer_idx,
            ) {
                staged.push((node_idx, c));
            }
            if let Some(c) = modulator_slider(
                ui,
                (node_idx, "sat"),
                "saturation",
                &mut sat_m,
                0.0..=2.0,
                ModulatorField::ColorSaturation,
                node_idx,
                layer_idx,
            ) {
                staged.push((node_idx, c));
            }
            if let Some(c) = modulator_slider(
                ui,
                (node_idx, "bri"),
                "brightness",
                &mut bri_m,
                -1.0..=1.0,
                ModulatorField::ColorBrightness,
                node_idx,
                layer_idx,
            ) {
                staged.push((node_idx, c));
            }
            if let Some(c) = modulator_slider(
                ui,
                (node_idx, "con"),
                "contrast",
                &mut con_m,
                0.0..=2.0,
                ModulatorField::ColorContrast,
                node_idx,
                layer_idx,
            ) {
                staged.push((node_idx, c));
            }
        }
        Effect::Tint { amount, .. } => {
            let mut amount_m = amount.clone();
            if let Some(c) = modulator_slider(
                ui,
                (node_idx, "tamt"),
                "amount",
                &mut amount_m,
                0.0..=1.0,
                ModulatorField::TintAmount,
                node_idx,
                layer_idx,
            ) {
                staged.push((node_idx, c));
            }
        }
        Effect::Blur { radius_px } => {
            let mut r = radius_px.clone();
            if let Some(c) = modulator_slider(
                ui,
                (node_idx, "blur"),
                "radius (px)",
                &mut r,
                0.0..=32.0,
                ModulatorField::BlurRadius,
                node_idx,
                layer_idx,
            ) {
                staged.push((node_idx, c));
            }
        }
        Effect::Transform {
            translate,
            rotate_deg,
            scale_x,
            scale_y,
        } => {
            // translate uses command_slider pattern (plain f32 — no Modulator)
            let (tx_cur, ty_cur) = (translate[0], translate[1]);
            let mut tx_edit = tx_cur;
            let mut ty_edit = ty_cur;
            let resp_tx = ui.add(egui::Slider::new(&mut tx_edit, -1.0..=1.0).text("tx"));
            if (resp_tx.drag_stopped() || resp_tx.lost_focus()) && (tx_edit - tx_cur).abs() > 1e-6 {
                staged.push((node_idx, EffectChange::TransformTranslateX(tx_edit)));
            }
            let resp_ty = ui.add(egui::Slider::new(&mut ty_edit, -1.0..=1.0).text("ty"));
            if (resp_ty.drag_stopped() || resp_ty.lost_focus()) && (ty_edit - ty_cur).abs() > 1e-6 {
                staged.push((node_idx, EffectChange::TransformTranslateY(ty_edit)));
            }
            let mut rot_m = rotate_deg.clone();
            let mut scx_m = scale_x.clone();
            let mut scy_m = scale_y.clone();
            if let Some(c) = modulator_slider(
                ui,
                (node_idx, "rot"),
                "rotate (deg)",
                &mut rot_m,
                -180.0..=180.0,
                ModulatorField::TransformRotateDeg,
                node_idx,
                layer_idx,
            ) {
                staged.push((node_idx, c));
            }
            if let Some(c) = modulator_slider(
                ui,
                (node_idx, "scx"),
                "scale x",
                &mut scx_m,
                0.1..=3.0,
                ModulatorField::TransformScaleX,
                node_idx,
                layer_idx,
            ) {
                staged.push((node_idx, c));
            }
            if let Some(c) = modulator_slider(
                ui,
                (node_idx, "scy"),
                "scale y",
                &mut scy_m,
                0.1..=3.0,
                ModulatorField::TransformScaleY,
                node_idx,
                layer_idx,
            ) {
                staged.push((node_idx, c));
            }
        }
        Effect::Feedback { decay, offset } => {
            let mut d = decay.clone();
            if let Some(c) = modulator_slider(
                ui,
                (node_idx, "decay"),
                "decay (0=no trail, 1=hold)",
                &mut d,
                0.0..=1.0,
                ModulatorField::FeedbackDecay,
                node_idx,
                layer_idx,
            ) {
                staged.push((node_idx, c));
            }
            // offset is shown read-only since it's static [f32;2]
            ui.label(format!("offset x: {:.3}  y: {:.3}", offset[0], offset[1]));
            ui.weak("(offset editable via project file — drag-DragValue coming soon)");
        }
        Effect::External { id, params } => {
            ui.label(format!("id: {id}"));
            ui.label("params (JSON, edit via project file):");
            ui.label(
                serde_json::to_string_pretty(params).unwrap_or_else(|_| "<unprintable>".into()),
            );
        }
        Effect::Treatment { id, params, .. } => {
            let changes = show_treatment_params_read_only(ui, id, params, node_idx);
            treatment_changes.extend(changes);
        }
    }
}

// ---------------------------------------------------------------------------
// Detail panel — master-detail overhaul
// ---------------------------------------------------------------------------

/// Render the detail panel for the currently selected effect.
fn show_effect_detail_panel(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
    effect_idx: usize,
    staged_changes: &mut Vec<(usize, EffectChange)>,
    treatment_param_changes: &mut Vec<TreatmentParamChange>,
) {
    let node = &project.layers[layer_idx].effects[effect_idx];
    let effect_snap = node.effect.clone();
    let layer = &project.layers[layer_idx];

    // Compute no-op reason before any borrows that touch project mutably.
    let no_op_reason = if node.enabled {
        effect_is_no_op(node, layer)
    } else {
        None
    };

    // Intent glyph + color for header.
    let group = intent_group(&effect_snap);
    let (glyph, glyph_color) = intent_group_glyph(group);
    let display_name = effect_display_name(&effect_snap);

    // ---- Header row --------------------------------------------------------
    ui.horizontal(|ui| {
        ui.label(RichText::new(glyph).color(glyph_color));
        ui.heading(&display_name);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(egui::Button::new("\u{00D7}").small())
                .on_hover_text("Close detail panel")
                .clicked()
            {
                st.effect_detail_open = None;
            }
        });
    });
    ui.separator();

    // ---- Autofix banner ----------------------------------------------------
    if let Some(reason) = no_op_reason {
        // Amber-tinted frame
        let amber_bg = Color32::from_rgba_unmultiplied(0xFF, 0xB0, 0x30, 0x30);
        Frame::default()
            .fill(amber_bg)
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("\u{26A0} {reason}")).strong());
                    if ui.button("Auto-fix").clicked() {
                        handle_autofix(reason, effect_idx, layer_idx, project, st);
                    }
                });
            });
        ui.add_space(4.0);
    }

    // ---- Body — full params ------------------------------------------------
    match &effect_snap {
        Effect::Treatment { id, params, .. } => {
            let changes = show_treatment_params_read_only(ui, id, params, effect_idx);
            treatment_param_changes.extend(changes);
        }
        _ => {
            show_effect_full_params(
                ui,
                &effect_snap,
                effect_idx,
                layer_idx,
                staged_changes,
                treatment_param_changes,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Main entry point — T1.25–T1.32
// ---------------------------------------------------------------------------

/// 004-T1.25–T1.32 — Unified Look chain section.
///
/// Renders the full Look-chain rail for the given layer: A/B compare header
/// (T1.29), drag-reorder rows (T1.25), master-detail param panel (overhaul),
/// and the Add picker (T1.27 / T1.32).
pub fn show_look_chain_section(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
) {
    if layer_idx >= project.layers.len() {
        ui.weak("(no layer selected)");
        return;
    }

    // ---- Auto-close guard: clear stale detail selection --------------------
    let effects_len = project.layers[layer_idx].effects.len();
    if let Some((open_li, open_ei)) = st.effect_detail_open {
        if open_li != layer_idx || open_ei >= effects_len {
            st.effect_detail_open = None;
        }
    }

    // ---- T1.29 — A/B compare header toggle --------------------------------
    ui.horizontal(|ui| {
        ui.strong("Look chain");
        ui.add_space(8.0);
        let ab_label = if st.ab_compare {
            "A/B compare: ON"
        } else {
            "A/B compare"
        };
        if ui.selectable_label(st.ab_compare, ab_label).clicked() {
            st.ab_compare = !st.ab_compare;
        }
    });
    ui.separator();

    // ---- C.2 — Preset apply control (folded above the chain rail) ----------
    // Load preset list lazily (same pattern as the old Effects tab).
    if !st.presets_loaded {
        st.presets = load_presets_from_disk();
        st.presets_loaded = true;
    }
    if !st.presets.is_empty() {
        ui.horizontal(|ui| {
            ui.label("Preset:");
            st.preset_picker_index = st.preset_picker_index.min(st.presets.len() - 1);
            egui::ComboBox::from_id_salt("look_chain_preset_pick")
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
            if ui.button("Reload").clicked() {
                st.presets = load_presets_from_disk();
                st.preset_picker_index = 0;
            }
        });
        ui.add_space(2.0);
    }

    if effects_len == 0 {
        ui.weak("No effects. Use '+ Add to chain' below.");
    }

    // ---- Staged changes collected during the row loop / detail panel ------
    let mut staged_changes: Vec<(usize, EffectChange)> = Vec::new();
    let mut treatment_param_changes: Vec<TreatmentParamChange> = Vec::new();

    // ---- T1.25 — drag-reorder / remove state captured during loop ---------
    let mut pending_reorder: Option<(usize, usize)> = None;
    let mut pending_remove: Option<usize> = None;

    // ---- Master-detail layout split ----------------------------------------
    let detail_open = st.effect_detail_open;
    let available = ui.available_width();
    let narrow = available < 520.0;

    if detail_open.is_some() && !narrow {
        // Side-by-side: left rail + right detail panel.
        let left_width = available * 0.55;
        let right_width = available - left_width - 8.0;

        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(left_width, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    render_chain_rail(
                        ui,
                        project,
                        st,
                        layer_idx,
                        effects_len,
                        &mut pending_reorder,
                        &mut pending_remove,
                    );
                },
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                egui::vec2(right_width, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    if let Some((open_li, open_ei)) = detail_open {
                        // Bounds re-checked: auto-close guard at top handles stale indices.
                        if open_li == layer_idx
                            && open_ei < project.layers[layer_idx].effects.len()
                        {
                            show_effect_detail_panel(
                                ui,
                                project,
                                st,
                                open_li,
                                open_ei,
                                &mut staged_changes,
                                &mut treatment_param_changes,
                            );
                        }
                    }
                },
            );
        });
    } else {
        // Single-column: just the chain rail.
        render_chain_rail(
            ui,
            project,
            st,
            layer_idx,
            effects_len,
            &mut pending_reorder,
            &mut pending_remove,
        );
        // Narrow-mode: detail panel stacked below when selected.
        if let Some((open_li, open_ei)) = detail_open {
            if narrow
                && open_li == layer_idx
                && open_ei < project.layers[layer_idx].effects.len()
            {
                ui.separator();
                show_effect_detail_panel(
                    ui,
                    project,
                    st,
                    open_li,
                    open_ei,
                    &mut staged_changes,
                    &mut treatment_param_changes,
                );
            }
        }
    }

    // ---- Apply drag-reorder (P2.7.1 pattern) --------------------------------
    if let Some((src, dst)) = pending_reorder {
        let old = project.layers[layer_idx].effects.clone();
        let mut new = old.clone();
        let item = new.remove(src);
        new.insert(dst, item);
        st.pending_mutations
            .push(project.set_layer_effects_mutation(layer_idx, new));
    }

    // ---- Apply remove (P2.7.2 pattern) ---------------------------------------
    if let Some(remove_idx) = pending_remove {
        tracing::debug!(
            target: "rmap",
            "look_chain pending_remove dispatched remove_idx={remove_idx}"
        );
        // Auto-close detail if the removed effect was open.
        if st
            .effect_detail_open
            .map(|(l, e)| l == layer_idx && e == remove_idx)
            .unwrap_or(false)
        {
            st.effect_detail_open = None;
        }
        let mut new = project.layers[layer_idx].effects.clone();
        new.remove(remove_idx);
        st.pending_mutations
            .push(project.set_layer_effects_mutation(layer_idx, new));
    }

    // ---- Flush staged Modulator / TransformTranslate changes ----------------
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
            let mut new_effects = old.clone();
            for (eff_idx, change) in field_changes {
                if let Some(node) = new_effects.get_mut(eff_idx) {
                    if let Effect::Transform { translate, .. } = &mut node.effect {
                        match change {
                            EffectChange::TransformTranslateX(v) => translate[0] = v,
                            EffectChange::TransformTranslateY(v) => translate[1] = v,
                            EffectChange::ModulatorSwitch { .. } => unreachable!(),
                        }
                    }
                }
            }
            st.pending_mutations.push(Mutation::SetLayerEffects(
                crate::project::command::SetLayerEffects {
                    layer_idx,
                    new: new_effects,
                    old,
                },
            ));
        }
    }

    // ---- Flush Treatment param changes (T1.26) --------------------------------
    // Collect all treatment param edits from this frame into a single
    // SetLayerEffects mutation (Effects-Vec Reverse rule 2).
    if !treatment_param_changes.is_empty() {
        let old = project.layers[layer_idx].effects.clone();
        let mut new_effects = old.clone();
        for change in treatment_param_changes {
            if let Some(node) = new_effects.get_mut(change.node_idx) {
                if let Effect::Treatment { params, .. } = &mut node.effect {
                    params.insert(change.key, change.value);
                }
            }
        }
        st.pending_mutations.push(Mutation::SetLayerEffects(
            crate::project::command::SetLayerEffects {
                layer_idx,
                new: new_effects,
                old,
            },
        ));
    }

    // ---- T1.27 — Add picker (menu_button pattern) --------------------------
    ui.add_space(4.0);
    ui.menu_button("+ Add to chain \u{25BE}", |ui| {
        show_add_picker(ui, project, st, layer_idx);
    });
}

// ---------------------------------------------------------------------------
// Chain rail — row loop (extracted for master-detail split)
// ---------------------------------------------------------------------------

/// Render the drag-reorder row list. Collects `pending_reorder` and
/// `pending_remove`. The caller dispatches mutations and handles the detail
/// panel (which stages `staged_changes` / `treatment_param_changes`).
fn render_chain_rail(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
    effects_len: usize,
    pending_reorder: &mut Option<(usize, usize)>,
    pending_remove: &mut Option<usize>,
) {

    for idx in 0..effects_len {
        let (_, drop_payload) = ui.dnd_drop_zone::<usize, _>(egui::Frame::default(), |ui| {
            // Determine if this row is selected (for highlight frame).
            let is_selected = st.effect_detail_open == Some((layer_idx, idx));
            let highlight_color = if is_selected {
                // Use selection bg_fill with reduced alpha for a subtle highlight.
                let sel = ui.visuals().selection.bg_fill;
                Color32::from_rgba_unmultiplied(sel.r(), sel.g(), sel.b(), 60)
            } else {
                Color32::TRANSPARENT
            };

            // `ui.interact` over the row rect resolves "last wins" against
            // children with `Sense::click()` underneath — clicks on the drag
            // handle, status dot, and delete × can land on the row rect.
            // We compare the click position to each child rect after the fact
            // and suppress row-open if the click was inside any of them.
            let mut handle_rect: Option<egui::Rect> = None;
            let mut dot_rect: Option<egui::Rect> = None;
            let mut del_rect: Option<egui::Rect> = None;

            let row_resp = Frame::default()
                .fill(highlight_color)
                .inner_margin(egui::Margin::symmetric(2, 1))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // ---- Drag handle: ≡ glyph, payload = source index ------
                        let handle =
                            ui.add(egui::Label::new("\u{2261}").sense(Sense::click_and_drag()));
                        handle.dnd_set_drag_payload(idx);
                        handle_rect = Some(handle.rect);

                        // ---- Intent-group glyph --------------------------------
                        let group =
                            intent_group(&project.layers[layer_idx].effects[idx].effect);
                        let (glyph, glyph_color) = intent_group_glyph(group);
                        ui.label(RichText::new(glyph).color(glyph_color));

                        // ---- T1.25 — status dot (click toggles enabled) --------
                        let layer = &project.layers[layer_idx];
                        let node = &layer.effects[idx];
                        let no_op_reason = if node.enabled {
                            effect_is_no_op(node, layer)
                        } else {
                            None
                        };
                        let dot_color = if !node.enabled {
                            Color32::from_rgb(0x88, 0x88, 0x88) // grey — bypassed
                        } else if no_op_reason.is_some() {
                            Color32::from_rgb(0xFF, 0xB0, 0x30) // amber — no-op
                        } else {
                            Color32::from_rgb(0x40, 0xD0, 0x60) // green — active
                        };

                        // Allocate a clickable 12×12 rect for the dot
                        let dot_resp =
                            ui.allocate_response(Vec2::splat(12.0), Sense::click());
                        ui.painter()
                            .circle_filled(dot_resp.rect.center(), 5.0, dot_color);
                        dot_rect = Some(dot_resp.rect);
                        if dot_resp.clicked() {
                            let mut new_effects =
                                project.layers[layer_idx].effects.clone();
                            new_effects[idx].enabled = !new_effects[idx].enabled;
                            st.pending_mutations.push(
                                project.set_layer_effects_mutation(layer_idx, new_effects),
                            );
                        }
                        dot_resp.on_hover_text(if !node.enabled {
                            "Bypassed \u{2014} click to enable".to_string()
                        } else if let Some(reason) = no_op_reason {
                            format!("\u{26A0} {reason}")
                        } else {
                            "Active".to_string()
                        });

                        // ---- Effect name (flex, truncates) ---------------------
                        let effect_ref =
                            &project.layers[layer_idx].effects[idx].effect;
                        let name = effect_display_name(effect_ref);
                        ui.add(egui::Label::new(name).truncate());

                        // ---- Delete × button ----------------------------------
                        let del_resp = ui
                            .add(egui::Button::new("\u{00D7}").small())
                            .on_hover_text("Remove from chain");
                        del_rect = Some(del_resp.rect);
                        if del_resp.clicked() {
                            tracing::debug!(
                                target: "rmap",
                                "look_chain × click registered idx={idx}"
                            );
                            *pending_remove = Some(idx);
                        }
                    })
                });

            // Row-click sensing over the full row rect. Suppress when the
            // click landed inside the drag handle, status dot, or delete ×
            // (those have their own handlers), or while a drag is active.
            let row_rect = row_resp.response.rect;
            let row_click =
                ui.interact(row_rect, ui.id().with(("row_click", idx)), Sense::click());
            if row_click.clicked() && handle_not_dragged(ui) {
                let on_child = row_click.interact_pointer_pos().is_some_and(|p| {
                    handle_rect.is_some_and(|r| r.contains(p))
                        || dot_rect.is_some_and(|r| r.contains(p))
                        || del_rect.is_some_and(|r| r.contains(p))
                });
                if !on_child {
                    st.effect_detail_open = Some((layer_idx, idx));
                }
            }
        });

        if let Some(source) = drop_payload {
            let source_idx = *source;
            if source_idx != idx {
                *pending_reorder = Some((source_idx, idx));
            }
        }
    }
}

/// Returns `true` when nothing is currently being DnD-dragged. A drag in
/// progress should not also open the detail panel.
fn handle_not_dragged(ui: &Ui) -> bool {
    ui.ctx().dragged_id().is_none()
}

// ---------------------------------------------------------------------------
// T1.27 — Add picker body (intent-grouped)
// ---------------------------------------------------------------------------

fn show_add_picker(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
) {
    use crate::effects::IntentGroup::*;

    // Section order per spec C.4: Warp / Color / Texture / Compose / Animate / Generative
    for group in [Warp, Color, Texture, Compose, Animate, Generative] {
        let (glyph, _color) = intent_group_glyph(group);
        let section_name = intent_group_name(group);
        ui.weak(format!("{glyph} {section_name}"));

        // Non-Treatment Effect variants in this group
        for (variant_label, hover, make_effect) in non_treatment_variants_for_group(group) {
            if ui
                .selectable_label(false, variant_label)
                .on_hover_text(hover)
                .clicked()
            {
                let new_node = EffectNode {
                    enabled: true,
                    effect: make_effect(),
                };
                add_node_to_chain(new_node, project, st, layer_idx);
                ui.close();
            }
        }

        // Treatment presets in this group
        for (preset_id, preset_label) in crate::render::treatments::registry() {
            let preset_group = crate::render::treatments::intent_group_for_preset(preset_id);
            if preset_group != group {
                continue;
            }
            let cap = crate::render::treatments::capability(preset_id);
            let hint = build_capability_hint(&cap);
            if ui
                .selectable_label(false, *preset_label)
                .on_hover_text(hint)
                .clicked()
            {
                // Seed params from descriptors so headline slider has a value
                let mut params = std::collections::HashMap::new();
                for d in crate::render::treatments::param_descriptors(preset_id) {
                    params.insert(d.key.to_string(), d.default);
                }
                let new_node = EffectNode {
                    enabled: true,
                    effect: Effect::Treatment {
                        id: preset_id.to_string(),
                        params,
                        overlay_path: None,
                        collage_paths: vec![],
                    },
                };

                // T1.32 — smart-fill: SDF-keyed preset + empty mask_polygon
                // → SetLayerEffectsAndMask (single undo step).
                let needs_sdf = cap.requires_sdf;
                let mask_empty = project.layers[layer_idx].warp.mask_polygon.is_empty();
                if needs_sdf && mask_empty {
                    let mut new_effects = project.layers[layer_idx].effects.clone();
                    new_effects.push(new_node);
                    st.pending_mutations
                        .push(project.set_layer_effects_and_mask_mutation(
                            layer_idx,
                            new_effects,
                            full_quad_polygon(),
                        ));
                    // TODO(004-T1.32): zone-fill deferred — cap.requires_zone &&
                    // zone_role.is_none() → SetLayerEffectsAndMask doesn't carry
                    // zone_role; zone-role assignment is a separate field not
                    // covered by a combined mutation in v1.
                } else {
                    add_node_to_chain(new_node, project, st, layer_idx);
                }
                ui.close();
            }
        }

        ui.add_space(2.0);
    }
}

/// Append `new_node` to the chain and dispatch `SetLayerEffects`.
fn add_node_to_chain(
    new_node: EffectNode,
    project: &mut Project,
    st: &mut ControlPanelState,
    layer_idx: usize,
) {
    let mut new_effects = project.layers[layer_idx].effects.clone();
    new_effects.push(new_node);
    st.pending_mutations
        .push(project.set_layer_effects_mutation(layer_idx, new_effects));
}

/// Human-readable name for an IntentGroup (section headers in the picker).
fn intent_group_name(g: IntentGroup) -> &'static str {
    match g {
        IntentGroup::Warp => "Warp",
        IntentGroup::Color => "Color",
        IntentGroup::Texture => "Texture",
        IntentGroup::Compose => "Compose",
        IntentGroup::Animate => "Animate",
        IntentGroup::Generative => "Generative",
    }
}

/// One Effect variant entry in the add-picker. `(display_label, hover_text, factory_fn)`.
type EffectPickerEntry = (&'static str, &'static str, Box<dyn Fn() -> Effect>);

/// Non-Treatment Effect variants belonging to a given IntentGroup.
fn non_treatment_variants_for_group(group: IntentGroup) -> Vec<EffectPickerEntry> {
    match group {
        IntentGroup::Warp => vec![(
            "Transform",
            "Translate, rotate, scale the layer",
            Box::new(|| Effect::Transform {
                translate: [0.0, 0.0],
                rotate_deg: Modulator::Static(0.0),
                scale_x: Modulator::Static(1.0),
                scale_y: Modulator::Static(1.0),
            }) as Box<dyn Fn() -> Effect>,
        )],
        IntentGroup::Color => vec![
            (
                "Color grade",
                "Hue shift, saturation, brightness, contrast",
                Box::new(|| Effect::Color {
                    hue: Modulator::Static(0.0),
                    saturation: Modulator::Static(1.0),
                    brightness: Modulator::Static(0.0),
                    contrast: Modulator::Static(1.0),
                }) as Box<dyn Fn() -> Effect>,
            ),
            (
                "Tint",
                "Overlay a solid color tint on the layer",
                Box::new(|| Effect::Tint {
                    rgba: [1.0, 1.0, 1.0, 1.0],
                    amount: Modulator::Static(0.0),
                    mode: crate::effects::tint::TintMode::Multiply,
                }) as Box<dyn Fn() -> Effect>,
            ),
        ],
        IntentGroup::Texture => vec![(
            "Blur",
            "Gaussian blur",
            Box::new(|| Effect::Blur {
                radius_px: Modulator::Static(0.0),
            }) as Box<dyn Fn() -> Effect>,
        )],
        IntentGroup::Animate => vec![(
            "Feedback",
            "Trails / motion smear \u{2014} blends previous frame into current",
            Box::new(|| Effect::Feedback {
                decay: Modulator::Static(0.0),
                offset: [0.0, 0.0],
            }) as Box<dyn Fn() -> Effect>,
        )],
        IntentGroup::Compose | IntentGroup::Generative => vec![],
    }
}

/// Short description + capability hints for a treatment picker row.
fn build_capability_hint(cap: &crate::render::treatments::PresetCapability) -> String {
    let mut parts = Vec::new();
    if cap.requires_sdf {
        parts.push("Needs a mask polygon");
    }
    if cap.requires_zone {
        parts.push("Needs a zone role");
    }
    if cap.is_particle {
        parts.push("Particle-based");
    }
    if parts.is_empty() {
        "No special requirements".to_string()
    } else {
        parts.join(" \u{00B7} ")
    }
}

// ---------------------------------------------------------------------------
// T1.28 — autofix dispatch
// ---------------------------------------------------------------------------

/// Dispatch the appropriate mutation for an autofix chip click.
///
/// Cases handled end-to-end:
/// - `"Needs a mask polygon"` → `SetLayerMaskPolygon` with a full-quad polygon.
/// - `"Amplitude at 0"` → `SetLayerEffects` with headline param nudged to mid-range.
/// - `"All params at identity"` → `SetLayerEffects` with `exposure` nudged to 1.0.
///
/// Deferred stubs:
/// - `"Needs a zone role"` — zone-role assignment is a separate field not covered
///   by the combined `SetLayerEffectsAndMask` mutation in v1.
/// - `"Overlay file missing"` — detection requires extending `effect_is_no_op` to
///   check `overlay_path` (not yet wired in `src/effects/mod.rs`).
///   TODO(004-T1.28): wire overlay-missing via rfd file dialog once detection lands.
fn handle_autofix(
    reason: &str,
    node_idx: usize,
    layer_idx: usize,
    project: &mut Project,
    st: &mut ControlPanelState,
) {
    match reason {
        "Needs a mask polygon" => {
            // Acceptance criterion: mask case works end-to-end (spec §C.5).
            st.pending_mutations
                .push(project.set_layer_mask_polygon_mutation(layer_idx, full_quad_polygon()));
        }
        "Amplitude at 0" => {
            let mut new_effects = project.layers[layer_idx].effects.clone();
            if let Some(node) = new_effects.get_mut(node_idx) {
                if let Effect::Treatment { id, params, .. } = &mut node.effect {
                    let cap = crate::render::treatments::capability(id);
                    if let Some(key) = cap.headline_param {
                        let descs = crate::render::treatments::param_descriptors(id);
                        let nudge = descs
                            .iter()
                            .find(|d| d.key == key)
                            .map(|d| (d.default + d.max) * 0.5_f32)
                            .unwrap_or(0.3);
                        params.insert(key.to_string(), nudge);
                    }
                }
            }
            st.pending_mutations
                .push(project.set_layer_effects_mutation(layer_idx, new_effects));
        }
        "All params at identity" => {
            let mut new_effects = project.layers[layer_idx].effects.clone();
            if let Some(node) = new_effects.get_mut(node_idx) {
                if let Effect::Treatment { params, .. } = &mut node.effect {
                    // For tone_map at identity: nudge exposure to 1.0 stop
                    params.insert("exposure".to_string(), 1.0_f32);
                }
            }
            st.pending_mutations
                .push(project.set_layer_effects_mutation(layer_idx, new_effects));
        }
        "Needs a zone role" => {
            // Deferred: SetLayerEffectsAndMask doesn't carry zone_role.
            // Zone-role assignment needs a separate mutation not yet wired here.
            tracing::warn!(
                reason,
                "autofix chip: 'Needs a zone role' — zone-role assignment deferred"
            );
        }
        _ => {
            // TODO(004-T1.28): overlay-missing detection requires extending
            // effect_is_no_op to check overlay_path in src/effects/mod.rs.
            tracing::warn!(reason, "autofix chip: unknown reason, no action taken");
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modulators::Modulator;

    #[test]
    fn effect_display_name_non_treatment_variants() {
        assert_eq!(
            effect_display_name(&Effect::Color {
                hue: Modulator::Static(0.0),
                saturation: Modulator::Static(1.0),
                brightness: Modulator::Static(0.0),
                contrast: Modulator::Static(1.0),
            }),
            "Color"
        );
        assert_eq!(
            effect_display_name(&Effect::Blur {
                radius_px: Modulator::Static(0.0),
            }),
            "Blur"
        );
        assert_eq!(
            effect_display_name(&Effect::Tint {
                rgba: [1.0, 1.0, 1.0, 1.0],
                amount: Modulator::Static(0.0),
                mode: crate::effects::tint::TintMode::Multiply,
            }),
            "Tint"
        );
        assert_eq!(
            effect_display_name(&Effect::Transform {
                translate: [0.0, 0.0],
                rotate_deg: Modulator::Static(0.0),
                scale_x: Modulator::Static(1.0),
                scale_y: Modulator::Static(1.0),
            }),
            "Transform"
        );
        assert_eq!(
            effect_display_name(&Effect::Feedback {
                decay: Modulator::Static(0.0),
                offset: [0.0, 0.0],
            }),
            "Feedback"
        );
        assert_eq!(
            effect_display_name(&Effect::External {
                id: "test".to_string(),
                params: Default::default(),
            }),
            "External"
        );
    }

    #[test]
    fn effect_display_name_treatment_prettify() {
        let treatment = Effect::Treatment {
            id: "ripple_lens".to_string(),
            params: Default::default(),
            overlay_path: None,
            collage_paths: vec![],
        };
        assert_eq!(effect_display_name(&treatment), "Ripple lens");

        let single = Effect::Treatment {
            id: "tone_map".to_string(),
            params: Default::default(),
            overlay_path: None,
            collage_paths: vec![],
        };
        assert_eq!(effect_display_name(&single), "Tone map");

        let no_underscore = Effect::Treatment {
            id: "blur".to_string(),
            params: Default::default(),
            overlay_path: None,
            collage_paths: vec![],
        };
        assert_eq!(effect_display_name(&no_underscore), "Blur");
    }
}
