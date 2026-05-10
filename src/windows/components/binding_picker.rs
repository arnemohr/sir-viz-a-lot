//! `BindingPicker` — the dropdown that switches a `Modulator` between
//! its available shapes (P0.2.3a, W2.3a).
//!
//! Replaces the bare `static` dropdown called out in roadmap I3
//! ("Effect chain `static` dropdown is the binding mode"). The
//! roadmap calls for an antenna / jack icon plus a binding-indicator
//! pill; we land the option set + label vocabulary here. Icons + the
//! indicator pill follow as a refinement once a single canonical
//! migration (P0.2.3b) confirms the surface area.
//!
//! ## Option set
//!
//! Eight sources, presented in the order operators reach for them:
//!
//! | Slug      | Modulator variant         | Notes                       |
//! |-----------|---------------------------|-----------------------------|
//! | `fixed`   | `Static(f32)`             | Renamed from "static" per   |
//! |           |                           | roadmap I3 — the operator-  |
//! |           |                           | facing label is "fixed      |
//! |           |                           | value", reserving "static"  |
//! |           |                           | to mean "static binding".   |
//! | `sine`    | `Sine { ... }`            |                             |
//! | `tri`     | `Triangle { ... }`        |                             |
//! | `noise`   | `Noise { ... }`           |                             |
//! | `bpm`     | `Bpm { ... }`             | Beat-locked.                |
//! | `audio`   | `Audio { band, ... }`     | 8-band FFT (audio feature). |
//! | `osc`     | `OscBound { addr, ... }`  | UDP listener (osc feature). |
//! | `midi`    | `MidiBound { cc, ch, ..}` | Decoder (midi feature).     |
//!
//! The picker emits a [`BindingSource`] when the operator changes
//! the selection; the caller maps that to a fresh `Modulator`
//! payload (default values per variant).

use crate::modulators::Modulator;

/// Discriminator over the eight kinds the picker exposes. Independent
/// of `Modulator` so callers can construct the new payload with
/// whatever defaults the parameter range demands (the picker doesn't
/// know the slider min/max).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingSource {
    Fixed,
    Sine,
    Triangle,
    Noise,
    Bpm,
    Audio,
    Osc,
    Midi,
}

impl BindingSource {
    /// Operator-facing label rendered in the picker's button + the
    /// dropdown rows.
    pub fn label(self) -> &'static str {
        match self {
            BindingSource::Fixed => "fixed value",
            BindingSource::Sine => "sine",
            BindingSource::Triangle => "tri",
            BindingSource::Noise => "noise",
            BindingSource::Bpm => "bpm",
            BindingSource::Audio => "audio",
            BindingSource::Osc => "osc",
            BindingSource::Midi => "midi",
        }
    }

    /// Compact label rendered in the parameter-row's "currently
    /// bound" indicator (next to the spinner). Same as `label()`
    /// for now; pulled out so the binding-indicator pill (P0.2.3b)
    /// can specialise per variant ("MIDI CC 21 / Ch 1" etc.).
    pub fn short_label(self) -> &'static str {
        self.label()
    }

    /// Every variant in the order they appear in the dropdown.
    pub fn all() -> [BindingSource; 8] {
        [
            BindingSource::Fixed,
            BindingSource::Sine,
            BindingSource::Triangle,
            BindingSource::Noise,
            BindingSource::Bpm,
            BindingSource::Audio,
            BindingSource::Osc,
            BindingSource::Midi,
        ]
    }

    /// Read the source kind off a live `Modulator`.
    pub fn from_modulator(m: &Modulator) -> Self {
        match m {
            Modulator::Static(_) => BindingSource::Fixed,
            Modulator::Sine { .. } => BindingSource::Sine,
            Modulator::Triangle { .. } => BindingSource::Triangle,
            Modulator::Noise { .. } => BindingSource::Noise,
            Modulator::Bpm { .. } => BindingSource::Bpm,
            Modulator::Audio { .. } => BindingSource::Audio,
            Modulator::OscBound { .. } => BindingSource::Osc,
            Modulator::MidiBound { .. } => BindingSource::Midi,
        }
    }
}

/// Render the picker dropdown. Returns `Some(new_source)` when the
/// operator picks a different source; otherwise `None`. The caller
/// is responsible for constructing the replacement `Modulator`
/// payload — the picker doesn't know about parameter ranges, so a
/// single shared default would be wrong (e.g. `Sine.amp` should
/// scale with the parameter's range).
pub fn binding_picker(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash,
    current: BindingSource,
) -> Option<BindingSource> {
    let mut new_source: Option<BindingSource> = None;
    egui::ComboBox::from_id_salt(salt)
        .selected_text(current.label())
        .show_ui(ui, |ui| {
            for src in BindingSource::all() {
                if ui.selectable_label(src == current, src.label()).clicked() && src != current {
                    new_source = Some(src);
                }
            }
        });
    new_source
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// `BindingSource::all()` covers every `Modulator` variant exactly
    /// once. If a new variant lands without a matching `BindingSource`,
    /// `from_modulator` fails to compile (exhaustive match in the
    /// implementation) — but the round-trip via `all()` would still
    /// silently miss it. This test pins both directions.
    #[test]
    fn all_variants_mapped_round_trip() {
        let samples = [
            Modulator::Static(0.0),
            Modulator::Sine {
                period_s: 1.0,
                amp: 0.5,
                phase: 0.0,
                offset: 0.0,
            },
            Modulator::Triangle {
                period_s: 1.0,
                amp: 0.5,
                offset: 0.0,
            },
            Modulator::Noise {
                period_s: 1.0,
                amp: 0.5,
                offset: 0.0,
            },
            Modulator::Bpm {
                divisor: 1.0,
                amp: 0.5,
                offset: 0.0,
            },
            Modulator::Audio {
                band: 0,
                smoothing: 0.0,
                amp: 1.0,
                offset: 0.0,
            },
            Modulator::OscBound {
                addr: "/test".into(),
                scale: 1.0,
                offset: 0.0,
            },
            Modulator::MidiBound {
                cc: 21,
                channel: 0,
                scale: 1.0,
                offset: 0.0,
            },
        ];
        let mut seen: HashMap<BindingSource, usize> = HashMap::new();
        for m in samples.iter() {
            let s = BindingSource::from_modulator(m);
            *seen.entry(s).or_insert(0) += 1;
        }
        assert_eq!(seen.len(), 8, "every BindingSource must appear once");
        for src in BindingSource::all() {
            assert_eq!(seen.get(&src), Some(&1), "missing variant: {src:?}");
        }
    }

    /// Each label is non-empty.
    #[test]
    fn labels_are_non_empty() {
        for src in BindingSource::all() {
            assert!(!src.label().is_empty());
            assert!(!src.short_label().is_empty());
        }
    }

    /// Roadmap I3: "Static" → "fixed value" — the operator-facing
    /// label avoids the "static" jargon that conflicts with "static
    /// binding" semantics elsewhere in the picker vocabulary.
    #[test]
    fn fixed_value_is_relabelled() {
        assert_eq!(BindingSource::Fixed.label(), "fixed value");
    }
}
