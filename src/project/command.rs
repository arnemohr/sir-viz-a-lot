//! 003-T1.14 — typed project mutations with Reverse storage.
//!
//! `Mutation` is the type that flows through the v3 undo stack
//! (`UndoStack`, T-003-T1.15) and through the central
//! `apply_command` dispatcher (T-003-T1.16). Every variant carries
//! enough state to be reversible: applying a Mutation returns
//! another Mutation that, when applied to the same Project,
//! restores the prior state byte-equally.
//!
//! This is intentionally separate from `controls::Command` (the
//! existing keyboard / MIDI / OSC input-event enum). Input events
//! are session-scoped side-effects; Mutations are project-scoped
//! state transitions. The two will converge in a later refactor;
//! v3 keeps them separate for migration safety.
//!
//! # Reverse-storage rules — mandatory
//!
//! Three patterns make naive Reverse storage *wrong*. Every
//! variant constructor in this module must obey them. T-003-T1.17
//! locks them in via a property test that round-trips arbitrary
//! sequences of mutations through apply + undo and asserts the
//! resulting project is byte-equal to the starting state.
//!
//! 1. **Whole-enum Reverse.** Any variant that replaces an enum
//!    value (`Modulator`, `BlendMode`, `Effect`, `LayerKind`,
//!    `FitMode`) stores the *full old enum value*, not just the
//!    field that "looks" different. Variant-replacement loses
//!    unrelated fields silently otherwise.
//!
//! 2. **Effects-Vec Reverse.** Commands that touch a layer's
//!    effect chain (drag transform, preset apply) snapshot the
//!    entire `Vec<Effect>`, not just the changed effect. Reason:
//!    the `mutate_transform_effect` helper in
//!    `windows/scene_editor.rs` *appends* a default
//!    `Effect::Transform` to layers that don't have one — a
//!    per-field Reverse would leave a stray effect on undo.
//!
//! 3. **Snapshot Reverse.** Scene recall and crossfade tick
//!    replace the entire project from a `serde_json::Value`.
//!    They emit a single `Mutation::ApplyProjectSnapshot { ... }`
//!    whose Reverse is the previous full snapshot. Crossfade-
//!    tick mutations are flagged `non_undoable: true` and never
//!    enter the user-facing undo stack (they fire ~60×/s and
//!    would overwhelm any soft cap).
//!
//! # Runtime invariant
//!
//! Every mutator's `apply` body opens with a `debug_assert!`
//! that the carried `old` value matches the project's *current*
//! value pre-mutation. In test builds, a stale Reverse value
//! triggers an immediate panic with a helpful message. In
//! release builds the assert is compiled out — the property
//! test is the actual safety net.

#![deny(missing_docs)]

use crate::project::schema::Project;

/// 003-T1.14 — typed project mutations.
///
/// Each variant carries the previous value of every field it
/// touches so `apply` can produce a Reverse that restores the
/// pre-mutation state. Variants are added by T-003-T1.18+ as the
/// existing UI sites are migrated.
///
/// `non_undoable` discriminator: see [`Mutation::is_non_undoable`].
/// Currently the only non-undoable variant is the crossfade-tick
/// flavour of `ApplyProjectSnapshot`.
#[derive(Clone)]
#[non_exhaustive]
#[allow(dead_code)] // T-003-T1.18+ wires call sites; foundation lives here from T1.14.
pub enum Mutation {
    /// Replace `Project.gamma`. Reverse: same variant with
    /// `new` and `old` swapped.
    SetGamma {
        /// Value to write.
        new: f32,
        /// Value pulled from the project at construction time;
        /// `apply` `debug_assert!`s this matches the live state.
        old: f32,
    },
    /// Replace `Project.brightness`. Same shape as `SetGamma`.
    SetBrightness {
        /// Value to write.
        new: f32,
        /// Pre-mutation value.
        old: f32,
    },
    /// Replace `Project.contrast`. Same shape as `SetGamma`.
    SetContrast {
        /// Value to write.
        new: f32,
        /// Pre-mutation value.
        old: f32,
    },
}

#[allow(dead_code)] // T-003-T1.18+ wires call sites.
impl Mutation {
    /// Apply the mutation to `project` and return its Reverse.
    /// In test builds, panics if the carried `old` value does not
    /// match the project's current state — catches contributor
    /// errors that would otherwise corrupt undo history.
    pub fn apply(self, project: &mut Project) -> Mutation {
        match self {
            Mutation::SetGamma { new, old } => {
                debug_assert!(
                    (project.gamma - old).abs() < 1e-6,
                    "SetGamma stale Reverse: project.gamma={}, expected old={}",
                    project.gamma,
                    old
                );
                project.gamma = new;
                Mutation::SetGamma { new: old, old: new }
            }
            Mutation::SetBrightness { new, old } => {
                debug_assert!(
                    (project.brightness - old).abs() < 1e-6,
                    "SetBrightness stale Reverse: project.brightness={}, expected old={}",
                    project.brightness,
                    old
                );
                project.brightness = new;
                Mutation::SetBrightness { new: old, old: new }
            }
            Mutation::SetContrast { new, old } => {
                debug_assert!(
                    (project.contrast - old).abs() < 1e-6,
                    "SetContrast stale Reverse: project.contrast={}, expected old={}",
                    project.contrast,
                    old
                );
                project.contrast = new;
                Mutation::SetContrast { new: old, old: new }
            }
        }
    }

    /// Whether this mutation should be excluded from the
    /// user-facing undo stack. Today only crossfade-tick
    /// `ApplyProjectSnapshot` variants set this; gamma /
    /// brightness / contrast slider edits are all undoable.
    pub fn is_non_undoable(&self) -> bool {
        match self {
            Mutation::SetGamma { .. }
            | Mutation::SetBrightness { .. }
            | Mutation::SetContrast { .. } => false,
        }
    }
}

/// 003-T1.14 helper constructors. Each reads the project's
/// *current* value and wraps it as the `old` field of the
/// corresponding `Mutation` variant. Call sites cannot forget to
/// snapshot the old state because the constructor does it for
/// them — this is the type-system-friendly substitute for the
/// originally-planned compile-time enforcement (deferred to v3.1).
#[allow(dead_code)] // T-003-T1.18+ wires call sites.
impl Project {
    /// Build a `SetGamma` mutation whose Reverse will restore the
    /// project's current gamma.
    pub fn set_gamma_mutation(&self, new: f32) -> Mutation {
        Mutation::SetGamma {
            new,
            old: self.gamma,
        }
    }

    /// Build a `SetBrightness` mutation.
    pub fn set_brightness_mutation(&self, new: f32) -> Mutation {
        Mutation::SetBrightness {
            new,
            old: self.brightness,
        }
    }

    /// Build a `SetContrast` mutation.
    pub fn set_contrast_mutation(&self, new: f32) -> Mutation {
        Mutation::SetContrast {
            new,
            old: self.contrast,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_project() -> Project {
        let json = serde_json::json!({
            "schema_version": 3,
            "layers": [],
            "warps": [],
        });
        let mut p: Project = serde_json::from_value(json).expect("project deserialise");
        if p.warps.is_empty() {
            p.warps.push(crate::project::schema::default_warp_mesh());
        }
        p
    }

    /// Apply + Reverse round-trip leaves the project byte-equal
    /// in `serde_json::Value` form. This is the property test in
    /// miniature; T-003-T1.17 generalises it across arbitrary
    /// mutation sequences.
    #[test]
    fn set_gamma_apply_then_reverse_round_trips() {
        let mut p = fresh_project();
        let before = serde_json::to_value(&p).unwrap();
        let mutation = p.set_gamma_mutation(2.5);
        let reverse = mutation.apply(&mut p);
        assert!((p.gamma - 2.5).abs() < 1e-6);
        let _ = reverse.apply(&mut p);
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(before, after, "round-trip should be byte-equal");
    }

    /// Stale Reverse storage triggers `debug_assert!` in test
    /// builds. Confirms the runtime safety net works.
    #[test]
    #[should_panic(expected = "SetGamma stale Reverse")]
    fn stale_old_value_panics_in_debug_builds() {
        let mut p = fresh_project();
        // Mismatched: claim old gamma is 99.0 when it's actually 1.0.
        let stale = Mutation::SetGamma {
            new: 2.0,
            old: 99.0,
        };
        let _ = stale.apply(&mut p);
    }

    /// All three sliders are undoable.
    #[test]
    fn slider_mutations_are_undoable() {
        let p = fresh_project();
        assert!(!p.set_gamma_mutation(1.0).is_non_undoable());
        assert!(!p.set_brightness_mutation(0.0).is_non_undoable());
        assert!(!p.set_contrast_mutation(1.0).is_non_undoable());
    }
}
