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
use crate::project::scene_templates::scene_registry;

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
                ui.label("Media slot picker — coming in P4.4.2.");
            }
            WizardStep::ZoneBinding => {
                ui.label("Zone binding — coming in P4.4.3.");
            }
            WizardStep::Palette => {
                ui.label("Palette & mood — coming in P4.4.4.");
            }
            WizardStep::Tempo => {
                ui.label("Tempo sync — coming in P4.4.5.");
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
