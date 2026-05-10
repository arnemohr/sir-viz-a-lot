//! `ParameterRow` — the canonical layout for a single editable
//! parameter (P0.2.3a, W2.3a).
//!
//! Per roadmap Appendix B: `label · unit · spinner · binding-picker
//! · learn-state pill`. The widget is a builder that lets callers
//! assemble the row without committing to the v3 parameter-edit
//! plumbing yet — P0.2.3b/c migrate the existing modulator rows to
//! this shape; P0.2.4 (OSC patch panel) and P0.2.5 (MIDI-learn) add
//! the remaining UX.
//!
//! ## Anatomy
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │ [hue]  [°]  [ 0.0 ▲▼ ]  [ fixed value ▼ ]  [ MIDI CC 21 ]    │
//! │ ^^^^   ^^^  ^^^^^^^^^^   ^^^^^^^^^^^^^^^^   ^^^^^^^^^^^^^    │
//! │ label  unit value-edit   binding-picker     learn-state pill │
//! └──────────────────────────────────────────────────────────────┘
//! ```
//!
//! - **Label**: the parameter name. Glossary popovers attach via
//!   `glossary_label` when applicable.
//! - **Unit**: optional unit string (`°`, `px`, `×`). Renders subdued.
//! - **Value-edit**: the variant-specific editor (slider, spinner,
//!   text field for OSC address, etc.). The caller supplies a
//!   closure that draws into the row's `Ui` so the row doesn't have
//!   to know every possible editor shape.
//! - **Binding picker**: see [`super::binding_picker`].
//! - **Learn-state pill**: shown only when MIDI-learn (P0.2.5) is
//!   armed against this parameter — pulses the warm accent.

use egui::Ui;

use super::binding_picker::{BindingSource, binding_picker};

/// Outcome of one render pass of the row. The caller inspects this
/// to dispatch a `Mutation` (the row itself stays Mutation-free so
/// it can render in non-v3 builds too).
#[derive(Debug, Clone, Default)]
pub struct RowOutcome {
    /// Operator picked a different binding source. The caller
    /// constructs the replacement `Modulator` payload (with
    /// parameter-range-appropriate defaults) and dispatches a
    /// `SetModulator` mutation.
    pub binding_changed: Option<BindingSource>,
}

/// Builder for one parameter-edit row. Collect the pieces, then call
/// [`Self::show`].
pub struct ParameterRow<'a> {
    label: &'a str,
    unit: Option<&'a str>,
    glossary: Option<crate::windows::glossary::GlossaryTerm>,
    salt: u64,
    current_source: BindingSource,
    learn_active: bool,
}

impl<'a> ParameterRow<'a> {
    /// Begin a new row. `label` is the parameter name; `salt` is a
    /// per-row id seed (combine with effect index + field id at the
    /// caller).
    pub fn new(label: &'a str, salt: u64, current: BindingSource) -> Self {
        Self {
            label,
            unit: None,
            glossary: None,
            salt,
            current_source: current,
            learn_active: false,
        }
    }

    /// Show a unit string after the label (e.g. "°", "px").
    pub fn unit(mut self, unit: &'a str) -> Self {
        self.unit = Some(unit);
        self
    }

    /// Attach a glossary popover to the label.
    pub fn glossary(mut self, term: crate::windows::glossary::GlossaryTerm) -> Self {
        self.glossary = Some(term);
        self
    }

    /// Mark the row as the current MIDI-learn target — the pill
    /// pulses the warm accent until the next CC arrives or ESC
    /// cancels (P0.2.5 wires the state).
    pub fn learn_active(mut self, active: bool) -> Self {
        self.learn_active = active;
        self
    }

    /// Render the row. `value_editor` is the variant-specific
    /// drawer — the caller draws sliders / text-edits / etc. into
    /// the row's `Ui`. Returns the [`RowOutcome`] so the caller can
    /// dispatch a `Mutation` after the borrow returns.
    pub fn show(self, ui: &mut Ui, value_editor: impl FnOnce(&mut Ui)) -> RowOutcome {
        let mut outcome = RowOutcome::default();
        ui.horizontal(|ui| {
            // Label (+ optional glossary popover).
            if let Some(term) = self.glossary {
                let _ = crate::windows::glossary::glossary_label(ui, term);
            } else {
                ui.label(self.label);
            }
            // Unit, subdued.
            if let Some(unit) = self.unit {
                ui.weak(unit);
            }

            // Value editor — caller's closure.
            value_editor(ui);

            // Binding picker.
            outcome.binding_changed = binding_picker(
                ui,
                ("rmap_param_row_picker", self.salt),
                self.current_source,
            );

            // Learn-state pill.
            if self.learn_active {
                ui.colored_label(egui::Color32::from_rgb(0xd0, 0xa0, 0x40), "● learning…");
            }
        });
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke: builder chain compiles and the row outcome defaults to
    /// no change. Egui rendering itself isn't exercised here — egui
    /// tests live with the harness; this confirms the API shape.
    #[test]
    fn builder_chain_smoke() {
        let row = ParameterRow::new("hue", 42, BindingSource::Fixed)
            .unit("°")
            .learn_active(false);
        // Field reads — confirms the builder retained values.
        assert_eq!(row.label, "hue");
        assert_eq!(row.unit, Some("°"));
        assert_eq!(row.salt, 42);
        assert!(matches!(row.current_source, BindingSource::Fixed));
        assert!(!row.learn_active);
    }

    /// Default `RowOutcome` reports no binding change.
    #[test]
    fn default_outcome_is_no_change() {
        let outcome = RowOutcome::default();
        assert!(outcome.binding_changed.is_none());
    }
}
