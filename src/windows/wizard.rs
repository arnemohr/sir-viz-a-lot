//! P4.3.1 — Scene wizard UI module.
//!
//! This module handles the wizard overlay rendered while `AppState::SceneWizard`
//! is active. The wizard walks the operator through five steps:
//!
//! 0. `TemplateSelect` — pick a scene template from the registry.
//! 1. `Media` — assign media files to each template slot.
//! 2. `ZoneBinding` — bind project zone roles to template zone roles.
//! 3. `Palette` — pick a colour palette and mood.
//! 4. `Tempo` — configure BPM sync.
//!
//! After step 4 the operator confirms, which calls `instantiate_template` and
//! dispatches `Mutation::ApplyProjectSnapshot`. Cancel at any step restores the
//! pre-wizard state via a non-undoable `ApplyProjectSnapshot`.
//!
//! # Pattern
//!
//! Mirrors `handle_launcher_window_event` in `src/app.rs`. The function
//! `draw_wizard_panel` is called from `App::window_event` via the
//! `AppState::SceneWizard` arm.

use crate::project::scene_instantiation::WizardChoices;
use crate::project::scene_templates::{
    MediaSlotKind, MoodHint, PaletteHint, SceneTemplate, scene_registry,
};
use crate::project::schema::ZoneRole;

// ---------------------------------------------------------------------------
// Wizard step enum
// ---------------------------------------------------------------------------

/// The current step displayed in the scene wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WizardStep {
    /// Step 0: template picker.
    #[default]
    TemplateSelect,
    /// Step 1: media slot assignment.
    Media,
    /// Step 2: zone role binding.
    ZoneBinding,
    /// Step 3: palette + mood picker.
    Palette,
    /// Step 4: tempo sync toggle (last step before Confirm).
    Tempo,
}

impl WizardStep {
    /// Returns the previous step, or `None` if already at the first step.
    pub fn prev(self) -> Option<Self> {
        match self {
            Self::TemplateSelect => None,
            Self::Media => Some(Self::TemplateSelect),
            Self::ZoneBinding => Some(Self::Media),
            Self::Palette => Some(Self::ZoneBinding),
            Self::Tempo => Some(Self::Palette),
        }
    }

    /// Returns the next step, or `None` if already at the last step.
    pub fn next(self) -> Option<Self> {
        match self {
            Self::TemplateSelect => Some(Self::Media),
            Self::Media => Some(Self::ZoneBinding),
            Self::ZoneBinding => Some(Self::Palette),
            Self::Palette => Some(Self::Tempo),
            Self::Tempo => None, // Confirm is handled separately.
        }
    }

    /// Short label for the step header.
    pub fn label(self) -> &'static str {
        match self {
            Self::TemplateSelect => "Select Template",
            Self::Media => "Assign Media",
            Self::ZoneBinding => "Bind Zones",
            Self::Palette => "Palette & Mood",
            Self::Tempo => "Tempo",
        }
    }
}

// ---------------------------------------------------------------------------
// Wizard action
// ---------------------------------------------------------------------------

/// What the wizard panel requests the caller (AppState) to do.
#[allow(dead_code)] // wired by P4.3.2 (cancel) and P4.3.3 (commit)
pub enum WizardAction {
    /// Advance to the next step (no mutation dispatched).
    Next,
    /// Go back one step (no mutation dispatched).
    Back,
    /// Cancel the wizard; caller should dispatch non-undoable
    /// `ApplyProjectSnapshot` with the pre-wizard snapshot.
    Cancel,
    /// Commit the wizard choices; caller calls `instantiate_template` and
    /// dispatches undoable `ApplyProjectSnapshot`.
    Confirm,
}

// ---------------------------------------------------------------------------
// Wizard panel draw
// ---------------------------------------------------------------------------

/// Draw the scene wizard panel in a `&mut egui::Ui`.
///
/// Returns `Some(WizardAction)` when the operator triggers a navigation
/// or commit/cancel action, `None` if the panel is still active with no
/// transition requested.
///
/// Called from `App::window_event` while `AppState::SceneWizard` is active,
/// inside `ctrl.render(device, queue, |ui| { draw_wizard_panel(ui, ...) })`.
#[allow(dead_code)] // wired by P4.3.1 routing skeleton in app.rs
pub fn draw_wizard_panel(
    ui: &mut egui::Ui,
    step: WizardStep,
    choices: &mut WizardChoices,
) -> Option<WizardAction> {
    let mut action: Option<WizardAction> = None;

    ui.vertical(|ui| {
        // ----- Header -----
        ui.heading("Scene Wizard");
        ui.label(format!("Step: {}", step.label()));
        ui.separator();

        // ----- Step body -----
        match step {
            WizardStep::TemplateSelect => {
                draw_template_select_step(ui, choices);
            }
            WizardStep::Media => {
                // P4.4.2 — media slot picker.
                if let Some(template) = selected_template(choices) {
                    draw_media_step(ui, template, choices);
                } else {
                    ui.label("Select a template first.");
                }
            }
            WizardStep::ZoneBinding => {
                // P4.4.3 — zone binding picker (Phase 3 zone API is live).
                if let Some(template) = selected_template(choices) {
                    draw_zone_binding_step(ui, template, choices);
                } else {
                    ui.label("Select a template first.");
                }
            }
            WizardStep::Palette => {
                // P4.4.4 — palette + mood picker.
                if let Some(template) = selected_template(choices) {
                    draw_palette_step(ui, template, choices);
                } else {
                    ui.label("Select a template first.");
                }
            }
            WizardStep::Tempo => {
                // P4.4.5 — tempo sync toggle.
                if let Some(template) = selected_template(choices) {
                    draw_tempo_step(ui, template, choices);
                } else {
                    ui.label("Select a template first.");
                }
            }
        }

        ui.separator();

        // ----- Footer navigation -----
        ui.horizontal(|ui| {
            // Cancel button (always visible).
            if ui.button("Cancel").clicked() {
                action = Some(WizardAction::Cancel);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if step == WizardStep::Tempo {
                    // Last step: show Confirm instead of Next.
                    if ui.button("Confirm").clicked() {
                        action = Some(WizardAction::Confirm);
                    }
                } else {
                    // Can advance if a template is selected (required for step 0).
                    let can_advance =
                        step != WizardStep::TemplateSelect || !choices.template_id.is_empty();
                    ui.add_enabled_ui(can_advance, |ui| {
                        if ui.button("Next →").clicked() {
                            action = Some(WizardAction::Next);
                        }
                    });
                }

                // Back button (hidden on step 0).
                if step.prev().is_some() && ui.button("← Back").clicked() {
                    action = Some(WizardAction::Back);
                }
            });
        });
    });

    action
}

/// Step 0: scrollable template selection grid.
///
/// Renders one card per registered template. Clicking a card sets
/// `choices.template_id`. P4.4.1 will expand this stub.
fn draw_template_select_step(ui: &mut egui::Ui, choices: &mut WizardChoices) {
    let registry = scene_registry();

    if registry.is_empty() {
        ui.label("No scene templates registered yet. Built-in templates land in W5.");
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("template_select_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    for (i, template) in registry.iter().enumerate() {
                        let selected = choices.template_id == template.id;
                        let card_label = egui::RichText::new(&template.display_name)
                            .strong()
                            .color(if selected {
                                egui::Color32::GOLD
                            } else {
                                egui::Color32::WHITE
                            });
                        if ui.button(card_label).clicked() {
                            choices.template_id = template.id.clone();
                        }
                        ui.label(&template.description);
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
        });
}

// ---------------------------------------------------------------------------
// Helper: resolve the currently-selected template.
// ---------------------------------------------------------------------------

/// Returns a reference to the selected `SceneTemplate`, or `None` if no
/// template is selected or the ID no longer matches the registry.
fn selected_template<'r>(choices: &WizardChoices) -> Option<&'r SceneTemplate> {
    if choices.template_id.is_empty() {
        return None;
    }
    scene_registry()
        .iter()
        .find(|t| t.id == choices.template_id)
}

// ---------------------------------------------------------------------------
// P4.4.2 — Step 1: media slot picker
// ---------------------------------------------------------------------------

/// Renders one file-picker row per media slot in the template.
///
/// Assigned paths are stored in `WizardChoices.media_slots`. Slots may be
/// left empty — the "Next" button always advances (empty slots produce layers
/// with empty paths; operator can assign media post-commit).
fn draw_media_step(ui: &mut egui::Ui, template: &SceneTemplate, choices: &mut WizardChoices) {
    if template.media_slots.is_empty() {
        ui.label("This template requires no media slots.");
        return;
    }

    egui::Grid::new("wizard_media_grid")
        .num_columns(3)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            for slot in &template.media_slots {
                // Label column.
                ui.label(&slot.label);

                // Path display column.
                let path_str = choices
                    .media_slots
                    .get(&slot.name)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "(none)".to_string());
                ui.label(egui::RichText::new(&path_str).monospace().small());

                // Action column: Choose / Clear.
                ui.horizontal(|ui| {
                    if ui.small_button("Choose…").clicked() {
                        // Pick a file with the appropriate filter.
                        let filter = match slot.accepts.first() {
                            Some(MediaSlotKind::Video) => ("Video", &["mp4", "mov", "m4v"][..]),
                            _ => ("Images", &["jpg", "jpeg", "png", "webp", "gif"][..]),
                        };
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(filter.0, filter.1)
                            .set_title(format!("Choose — {}", slot.label))
                            .pick_file()
                        {
                            choices.media_slots.insert(slot.name.clone(), path);
                        }
                    }
                    if choices.media_slots.contains_key(&slot.name)
                        && ui.small_button("✕").clicked()
                    {
                        choices.media_slots.remove(&slot.name);
                    }
                });

                ui.end_row();
            }
        });
}

// ---------------------------------------------------------------------------
// P4.4.3 — Step 2: zone binding picker
// ---------------------------------------------------------------------------

/// Renders one binding row per `zones_consumed` entry in the template.
///
/// Phase 3's `ZoneRole` tagging is live. If the project has no masks tagged
/// for a required role, the row shows an actionable message. The "Next" button
/// always advances regardless of binding state.
fn draw_zone_binding_step(
    ui: &mut egui::Ui,
    template: &SceneTemplate,
    choices: &mut WizardChoices,
) {
    if template.zones_consumed.is_empty() {
        ui.label("This template does not use zone binding.");
        return;
    }

    ui.label("Bind project zones to the roles this template expects:");
    ui.add_space(4.0);

    egui::Grid::new("wizard_zone_grid")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            for role in &template.zones_consumed {
                let role_label = zone_role_label(*role);
                ui.label(role_label);

                // Check whether the zone binding is already present.
                let bound = choices.zone_bindings.contains(role);
                let mut checked = bound;
                if ui.checkbox(&mut checked, "bind this role").changed() {
                    if checked {
                        if !choices.zone_bindings.contains(role) {
                            choices.zone_bindings.push(*role);
                        }
                    } else {
                        choices.zone_bindings.retain(|r| r != role);
                    }
                }

                ui.end_row();
            }
        });

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(
            "Tag masks with these zone roles in Mask mode before applying the template \
             for the best result. Zones improve the output but are not required.",
        )
        .small()
        .italics(),
    );
}

/// Short operator-facing label for a `ZoneRole`.
fn zone_role_label(role: ZoneRole) -> &'static str {
    match role {
        ZoneRole::Window => "Window",
        ZoneRole::Portal => "Portal",
        ZoneRole::Void => "Void",
        ZoneRole::Spill => "Spill",
        ZoneRole::Edge => "Edge",
        ZoneRole::Highlight => "Highlight",
        ZoneRole::LightSource => "Light Source",
    }
}

// ---------------------------------------------------------------------------
// P4.4.4 — Step 3: palette + mood picker
// ---------------------------------------------------------------------------

/// Renders three palette toggle-buttons and three mood toggle-buttons.
/// Defaults are taken from the template; operator can override.
fn draw_palette_step(ui: &mut egui::Ui, template: &SceneTemplate, choices: &mut WizardChoices) {
    let palette = choices.palette.unwrap_or(template.palette);
    let mood = choices.mood.unwrap_or(template.mood);

    ui.label("Choose a colour palette:");
    ui.horizontal(|ui| {
        for (label, variant) in [
            ("Warm", PaletteHint::Warm),
            ("Cool", PaletteHint::Cool),
            ("Neutral", PaletteHint::Neutral),
        ] {
            let selected = palette == variant;
            let btn = egui::Button::new(label).selected(selected);
            if ui.add(btn).clicked() {
                choices.palette = Some(variant);
            }
        }
    });

    ui.add_space(8.0);
    ui.label("Choose a mood:");
    ui.horizontal(|ui| {
        for (label, variant) in [
            ("Calm", MoodHint::Calm),
            ("Energetic", MoodHint::Energetic),
            ("Ethereal", MoodHint::Ethereal),
        ] {
            let selected = mood == variant;
            let btn = egui::Button::new(label).selected(selected);
            if ui.add(btn).clicked() {
                choices.mood = Some(variant);
            }
        }
    });
}

// ---------------------------------------------------------------------------
// P4.4.5 — Step 4: tempo picker
// ---------------------------------------------------------------------------

/// Renders a BPM-sync checkbox pre-ticked according to `template.tempo_sync`.
/// The BPM value is read-only (the wizard does not change project BPM).
fn draw_tempo_step(ui: &mut egui::Ui, template: &SceneTemplate, choices: &mut WizardChoices) {
    // Initialise from template default on first entry if not yet set.
    if !choices.tempo_sync && template.tempo_sync {
        choices.tempo_sync = true;
    }

    ui.horizontal(|ui| {
        ui.checkbox(&mut choices.tempo_sync, "Sync animation to project BPM");
    });

    if choices.tempo_sync {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Animation speed will be locked to the project BPM. \
                 Set the BPM in the BPM strip before going live.",
            )
            .small()
            .italics(),
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wizard_step_prev_on_first_step_returns_none() {
        assert_eq!(WizardStep::TemplateSelect.prev(), None);
    }

    #[test]
    fn wizard_step_media_prev_returns_template_select() {
        assert_eq!(WizardStep::Media.prev(), Some(WizardStep::TemplateSelect));
    }

    #[test]
    fn wizard_step_tempo_next_returns_none() {
        assert_eq!(WizardStep::Tempo.next(), None);
    }

    #[test]
    fn wizard_step_labels_are_non_empty() {
        for step in [
            WizardStep::TemplateSelect,
            WizardStep::Media,
            WizardStep::ZoneBinding,
            WizardStep::Palette,
            WizardStep::Tempo,
        ] {
            assert!(
                !step.label().is_empty(),
                "WizardStep::{step:?} label must not be empty"
            );
        }
    }
}
