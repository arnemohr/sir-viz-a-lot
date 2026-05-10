//! Reusable egui widgets for the Advanced panel (P0.2.3a, W2.3a).
//!
//! Per roadmap Appendix B (component vocabulary), these are the
//! standardised pieces every parameter-edit row uses:
//!
//! - [`binding_picker::BindingPicker`] — the dropdown that switches a
//!   parameter between `Static` / `Sine` / `Triangle` / `Noise` /
//!   `Bpm` / `Audio` / `OscBound` / `MidiBound`. Replaces the bare
//!   `static` dropdown called out in roadmap I3.
//! - [`parameter_row::ParameterRow`] — the row composition: label
//!   + unit + spinner + binding-picker + binding-indicator pill.
//!
//! P0.2.3a ships the components as standalone widgets — no call-site
//! migration. P0.2.3b migrates one canonical row (`Color.hue`) to
//! lock the recipe; P0.2.3c applies it to every other modulator row.

pub mod binding_picker;
pub mod parameter_row;
