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

use std::path::PathBuf;

use crate::project::schema::{BlendMode, LayerConfig, Project};

/// 003-T1.22 — addressing key for a single `Modulator` slot inside a
/// layer's effect chain. Combined with `(layer_idx, effect_idx)` it
/// uniquely identifies one of the 9 modulator-typed fields across all
/// `Effect` variants.
///
/// Variants are ordered by their position in `Effect`:
/// `Color { hue, saturation, brightness, contrast }`,
/// `Tint { amount }`, `Blur { radius_px }`,
/// `Transform { rotate_deg, scale_x, scale_y }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModulatorField {
    /// `Effect::Color::hue`.
    ColorHue,
    /// `Effect::Color::saturation`.
    ColorSaturation,
    /// `Effect::Color::brightness`.
    ColorBrightness,
    /// `Effect::Color::contrast`.
    ColorContrast,
    /// `Effect::Tint::amount`.
    #[allow(dead_code)]
    // Tint UI not yet wired (T1.23+); variant exists for schema completeness.
    TintAmount,
    /// `Effect::Blur::radius_px`.
    BlurRadius,
    /// `Effect::Transform::rotate_deg`.
    TransformRotateDeg,
    /// `Effect::Transform::scale_x`.
    TransformScaleX,
    /// `Effect::Transform::scale_y`.
    TransformScaleY,
    /// PCleanup.1.4 — `Effect::Feedback::decay`.
    FeedbackDecay,
}

/// Resolve a `ModulatorField` to the matching `&Modulator` slot
/// inside `effect`. Returns `None` if the field doesn't apply to the
/// effect's variant (e.g., `BlurRadius` on an `Effect::Color`).
fn modulator_at_ref(
    effect: &crate::effects::Effect,
    field: ModulatorField,
) -> Option<&crate::modulators::Modulator> {
    use crate::effects::Effect;
    match (effect, field) {
        (Effect::Color { hue, .. }, ModulatorField::ColorHue) => Some(hue),
        (Effect::Color { saturation, .. }, ModulatorField::ColorSaturation) => Some(saturation),
        (Effect::Color { brightness, .. }, ModulatorField::ColorBrightness) => Some(brightness),
        (Effect::Color { contrast, .. }, ModulatorField::ColorContrast) => Some(contrast),
        (Effect::Tint { amount, .. }, ModulatorField::TintAmount) => Some(amount),
        (Effect::Blur { radius_px }, ModulatorField::BlurRadius) => Some(radius_px),
        (Effect::Transform { rotate_deg, .. }, ModulatorField::TransformRotateDeg) => {
            Some(rotate_deg)
        }
        (Effect::Transform { scale_x, .. }, ModulatorField::TransformScaleX) => Some(scale_x),
        (Effect::Transform { scale_y, .. }, ModulatorField::TransformScaleY) => Some(scale_y),
        // PCleanup.1.4 — Effect::Feedback::decay.
        (Effect::Feedback { decay, .. }, ModulatorField::FeedbackDecay) => Some(decay),
        _ => None,
    }
}

/// Resolve a `ModulatorField` to the matching `&mut Modulator` slot
/// inside `effect`. Returns `None` if the field doesn't apply to the
/// effect's variant (e.g., `BlurRadius` on an `Effect::Color`).
fn modulator_at_mut(
    effect: &mut crate::effects::Effect,
    field: ModulatorField,
) -> Option<&mut crate::modulators::Modulator> {
    use crate::effects::Effect;
    match (effect, field) {
        (Effect::Color { hue, .. }, ModulatorField::ColorHue) => Some(hue),
        (Effect::Color { saturation, .. }, ModulatorField::ColorSaturation) => Some(saturation),
        (Effect::Color { brightness, .. }, ModulatorField::ColorBrightness) => Some(brightness),
        (Effect::Color { contrast, .. }, ModulatorField::ColorContrast) => Some(contrast),
        (Effect::Tint { amount, .. }, ModulatorField::TintAmount) => Some(amount),
        (Effect::Blur { radius_px }, ModulatorField::BlurRadius) => Some(radius_px),
        (Effect::Transform { rotate_deg, .. }, ModulatorField::TransformRotateDeg) => {
            Some(rotate_deg)
        }
        (Effect::Transform { scale_x, .. }, ModulatorField::TransformScaleX) => Some(scale_x),
        (Effect::Transform { scale_y, .. }, ModulatorField::TransformScaleY) => Some(scale_y),
        // PCleanup.1.4 — Effect::Feedback::decay.
        (Effect::Feedback { decay, .. }, ModulatorField::FeedbackDecay) => Some(decay),
        _ => None,
    }
}

/// V31.3.1 — type-level Reverse-storage guarantee for `Mutation` variants.
///
/// Each type that implements this trait carries enough state to apply
/// itself to a `Project` and return a reverse that, when applied again,
/// restores the prior state byte-equally. Returning `Self` (with `new`
/// and `old` swapped) matches the existing `Mutation::apply`/reverse
/// flow and makes the round-trip property trivially derivable per-
/// variant rather than enforced by hand across the whole match.
///
/// **Pattern (A) — enum-of-structs:** each `Mutation` variant is its
/// own struct implementing `ReverseStorage`; `Mutation` is a thin enum
/// wrapping the structs. The compile-time guarantee comes from the
/// `match` inside `Mutation::apply` requiring every arm to call
/// `s.apply(project)` — the compiler rejects a new arm that omits the
/// impl. V31.3.2 migrates the remaining variants.
///
/// # Trait-bound enforcement (compile-fail demonstration)
///
/// A type that does not implement `ReverseStorage` cannot satisfy the
/// bound. This doctest verifies the real trait rejects non-impl types —
/// unlike the deleted trybuild harness, this uses the actual symbol from
/// this module rather than a locally-defined stand-in.
///
/// ```compile_fail
/// use rmap::project::command::ReverseStorage;
/// struct NotReverseStorage;
/// fn requires_reverse_storage<T: ReverseStorage>() {}
/// fn _bad() { requires_reverse_storage::<NotReverseStorage>(); }
/// ```
pub trait ReverseStorage {
    /// Apply the mutation to `project` and return the reverse.
    ///
    /// The returned `Self` has `new` and `old` swapped so that calling
    /// `apply` on it again restores the project to its pre-mutation
    /// state. Implementations open with a `debug_assert!` verifying
    /// that the carried `old` matches the live project field — stale
    /// Reverse values panic in test/debug builds and compile out in
    /// release.
    fn apply(self, project: &mut Project) -> Self;
}

/// Payload for [`Mutation::SetGamma`].
///
/// Replaces `Project.gamma` with `new` and records the prior value in
/// `old` so the Reverse can restore it.
#[derive(Debug, Clone)]
pub struct SetGamma {
    /// Value to write to `Project.gamma`.
    pub new: f32,
    /// Value read from `Project.gamma` at construction time;
    /// `apply` `debug_assert!`s this matches the live state.
    pub old: f32,
}

impl ReverseStorage for SetGamma {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            (project.gamma - self.old).abs() < 1e-6,
            "SetGamma stale Reverse: project.gamma={}, expected old={}",
            project.gamma,
            self.old
        );
        project.gamma = self.new;
        SetGamma {
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetBrightness`].
#[derive(Debug, Clone)]
pub struct SetBrightness {
    /// Value to write.
    pub new: f32,
    /// Pre-mutation value.
    pub old: f32,
}

impl ReverseStorage for SetBrightness {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            (project.brightness - self.old).abs() < 1e-6,
            "SetBrightness stale Reverse: project.brightness={}, expected old={}",
            project.brightness,
            self.old
        );
        project.brightness = self.new;
        SetBrightness {
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetContrast`].
#[derive(Debug, Clone)]
pub struct SetContrast {
    /// Value to write.
    pub new: f32,
    /// Pre-mutation value.
    pub old: f32,
}

impl ReverseStorage for SetContrast {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            (project.contrast - self.old).abs() < 1e-6,
            "SetContrast stale Reverse: project.contrast={}, expected old={}",
            project.contrast,
            self.old
        );
        project.contrast = self.new;
        SetContrast {
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetCrossfadeDurationS`].
#[derive(Debug, Clone)]
pub struct SetCrossfadeDurationS {
    /// Value to write.
    pub new: f32,
    /// Pre-mutation value.
    pub old: f32,
}

impl ReverseStorage for SetCrossfadeDurationS {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            (project.crossfade_duration_s - self.old).abs() < 1e-6,
            "SetCrossfadeDurationS stale Reverse: project.crossfade_duration_s={}, expected old={}",
            project.crossfade_duration_s,
            self.old
        );
        project.crossfade_duration_s = self.new;
        SetCrossfadeDurationS {
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetProjectGammaOverride`].
/// Whole-`Option` Reverse so a `Some → None → Some` toggle round-trips byte-equally.
#[derive(Debug, Clone)]
pub struct SetProjectGammaOverride {
    /// Value to write (`None` clears the override).
    pub new: Option<f32>,
    /// Pre-mutation value.
    pub old: Option<f32>,
}

impl ReverseStorage for SetProjectGammaOverride {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.gamma_override == self.old,
            "SetProjectGammaOverride stale Reverse: project.gamma_override={:?}, expected old={:?}",
            project.gamma_override,
            self.old
        );
        project.gamma_override = self.new;
        SetProjectGammaOverride {
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetProjectBrightnessOverride`].
#[derive(Debug, Clone)]
pub struct SetProjectBrightnessOverride {
    /// Value to write.
    pub new: Option<f32>,
    /// Pre-mutation value.
    pub old: Option<f32>,
}

impl ReverseStorage for SetProjectBrightnessOverride {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.brightness_override == self.old,
            "SetProjectBrightnessOverride stale Reverse: project.brightness_override={:?}, expected old={:?}",
            project.brightness_override,
            self.old
        );
        project.brightness_override = self.new;
        SetProjectBrightnessOverride {
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetProjectContrastOverride`].
#[derive(Debug, Clone)]
pub struct SetProjectContrastOverride {
    /// Value to write.
    pub new: Option<f32>,
    /// Pre-mutation value.
    pub old: Option<f32>,
}

impl ReverseStorage for SetProjectContrastOverride {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.contrast_override == self.old,
            "SetProjectContrastOverride stale Reverse: project.contrast_override={:?}, expected old={:?}",
            project.contrast_override,
            self.old
        );
        project.contrast_override = self.new;
        SetProjectContrastOverride {
            new: self.old,
            old: self.new,
        }
    }
}

/// P0.7.3 — Payload for [`Mutation::SetEdgeBlend`].
/// Whole-`Option` Reverse so a `None → Some → None` toggle round-trips
/// byte-equally. Both fields carry the entire `Option<EdgeBlendConfig>` —
/// there is no per-sub-field Reverse because replacing a sub-field while
/// keeping the outer `Option::Some` alive would produce a stale Reverse
/// for the variant-change (`None ↔ Some`) direction.
#[derive(Debug, Clone)]
pub struct SetEdgeBlend {
    /// Value to write (`None` disables edge-blend).
    pub new: Option<crate::project::schema::EdgeBlendConfig>,
    /// Pre-mutation value.
    pub old: Option<crate::project::schema::EdgeBlendConfig>,
}

impl ReverseStorage for SetEdgeBlend {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.edge_blend == self.old,
            "SetEdgeBlend stale Reverse: project.edge_blend={:?}, expected old={:?}",
            project.edge_blend,
            self.old
        );
        project.edge_blend = self.new;
        SetEdgeBlend {
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetOutputWindowed`].
#[derive(Debug, Clone)]
pub struct SetOutputWindowed {
    /// Value to write.
    pub new: bool,
    /// Pre-mutation value.
    pub old: bool,
}

impl ReverseStorage for SetOutputWindowed {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.output_windowed == self.old,
            "SetOutputWindowed stale Reverse: project.output_windowed={}, expected old={}",
            project.output_windowed,
            self.old
        );
        project.output_windowed = self.new;
        SetOutputWindowed {
            new: self.old,
            old: self.new,
        }
    }
}

/// V31.6.1 — payload for [`Mutation::SetLayerMuted`].
///
/// Toggles a layer's `muted` flag and carries the prior value so the Reverse
/// can restore it. Whole-bool Reverse: bools are categorical (no lerp).
#[derive(Debug, Clone)]
pub struct SetLayerMuted {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// Value to write.
    pub new: bool,
    /// Pre-mutation value; `apply` `debug_assert!`s this matches the live state.
    pub old: bool,
}

impl ReverseStorage for SetLayerMuted {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerMuted: layer_idx out of range");
        debug_assert!(
            layer.muted == self.old,
            "SetLayerMuted stale Reverse: layer.muted={}, expected old={}",
            layer.muted,
            self.old
        );
        layer.muted = self.new;
        SetLayerMuted {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// V31.6.1 — payload for [`Mutation::SetLayerSolo`].
///
/// Replaces `Project.solo` (a project-level `Option<usize>` pointing to the
/// soloed layer index). Whole-`Option` Reverse so `None → Some(n) → None`
/// and `Some(a) → Some(b)` both round-trip byte-equally.
#[derive(Debug, Clone)]
pub struct SetLayerSolo {
    /// Value to write (`None` clears the solo; `Some(idx)` solos layer `idx`).
    pub new: Option<usize>,
    /// Pre-mutation value; `apply` `debug_assert!`s this matches the live state.
    pub old: Option<usize>,
}

impl ReverseStorage for SetLayerSolo {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.solo == self.old,
            "SetLayerSolo stale Reverse: project.solo={:?}, expected old={:?}",
            project.solo,
            self.old
        );
        project.solo = self.new;
        SetLayerSolo {
            new: self.old,
            old: self.new,
        }
    }
}

/// V31.7.2 — payload for [`Mutation::SetQuantizeBars`].
///
/// Replaces `Project.quantize_bars` (a project-level `Option<u8>` for
/// bar-quantized cue firing). Whole-`Option` Reverse so `None → Some(n) → None`
/// and `Some(a) → Some(b)` both round-trip byte-equally.
#[derive(Debug, Clone)]
pub struct SetQuantizeBars {
    /// Value to write (`None` means immediate fire; `Some(n)` quantizes to n bars).
    pub new: Option<u8>,
    /// Pre-mutation value; `apply` `debug_assert!`s this matches the live state.
    pub old: Option<u8>,
}

impl ReverseStorage for SetQuantizeBars {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.quantize_bars == self.old,
            "SetQuantizeBars stale Reverse: project.quantize_bars={:?}, expected old={:?}",
            project.quantize_bars,
            self.old
        );
        project.quantize_bars = self.new;
        SetQuantizeBars {
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetLayerMaskFeather`].
#[derive(Debug, Clone)]
pub struct SetLayerMaskFeather {
    /// Index into `Project.layers`; the layer's `warp` is the target.
    pub layer_idx: usize,
    /// Value to write.
    pub new: f32,
    /// Pre-mutation value.
    pub old: f32,
}

impl ReverseStorage for SetLayerMaskFeather {
    fn apply(self, project: &mut Project) -> Self {
        let warp = &mut project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerMaskFeather: layer_idx out of range")
            .warp;
        debug_assert!(
            (warp.mask_feather - self.old).abs() < 1e-6,
            "SetLayerMaskFeather stale Reverse: warp.mask_feather={}, expected old={}",
            warp.mask_feather,
            self.old
        );
        warp.mask_feather = self.new;
        SetLayerMaskFeather {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// P3.2.3 — payload for [`Mutation::SetMaskZoneRole`].
///
/// Follows the Whole-enum Reverse rule from `src/project/CLAUDE.md`:
/// store the full `Option<ZoneRole>` even though the value is small,
/// so future additions to `ZoneRole` cannot silently truncate the undo.
#[derive(Debug, Clone)]
pub struct SetMaskZoneRole {
    /// Index into `Project.layers`; the layer's `warp` is the target.
    pub layer_idx: usize,
    /// Role to write (or `None` to clear).
    pub new: Option<crate::project::schema::ZoneRole>,
    /// Pre-mutation role value.
    pub old: Option<crate::project::schema::ZoneRole>,
}

impl ReverseStorage for SetMaskZoneRole {
    fn apply(self, project: &mut Project) -> Self {
        let warp = &mut project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetMaskZoneRole: layer_idx out of range")
            .warp;
        debug_assert!(
            warp.zone_role == self.old,
            "SetMaskZoneRole stale Reverse: warp.zone_role={:?}, expected old={:?}",
            warp.zone_role,
            self.old
        );
        warp.zone_role = self.new;
        SetMaskZoneRole {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetLayerWarpDimensions`].
///
/// Editing rows or cols bilinear-resamples the grid (the existing T-M7-01
/// helper); the resample is lossy, so this variant follows Reverse rule 3 —
/// the `old_grid` snapshot lets undo restore the pre-mutation grid byte-equally
/// instead of attempting a reverse-resample.
#[derive(Debug, Clone)]
pub struct SetLayerWarpDimensions {
    /// Index into `Project.layers`; the layer's `warp` is the target.
    pub layer_idx: usize,
    /// Cell-row count to write.
    pub new_rows: u32,
    /// Cell-column count to write.
    pub new_cols: u32,
    /// Resampled grid to install.
    pub new_grid: Vec<Vec<[f32; 2]>>,
    /// Pre-mutation rows.
    pub old_rows: u32,
    /// Pre-mutation cols.
    pub old_cols: u32,
    /// Pre-mutation grid (full snapshot — see Reverse rule 3).
    pub old_grid: Vec<Vec<[f32; 2]>>,
}

impl ReverseStorage for SetLayerWarpDimensions {
    fn apply(self, project: &mut Project) -> Self {
        let warp = &mut project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerWarpDimensions: layer_idx out of range")
            .warp;
        debug_assert!(
            warp.rows == self.old_rows && warp.cols == self.old_cols,
            "SetLayerWarpDimensions stale Reverse: warp dims=({}, {}), expected old=({}, {})",
            warp.rows,
            warp.cols,
            self.old_rows,
            self.old_cols
        );
        let post_grid = self.new_grid;
        warp.grid = post_grid.clone();
        warp.rows = self.new_rows;
        warp.cols = self.new_cols;
        SetLayerWarpDimensions {
            layer_idx: self.layer_idx,
            new_rows: self.old_rows,
            new_cols: self.old_cols,
            new_grid: self.old_grid,
            old_rows: self.new_rows,
            old_cols: self.new_cols,
            old_grid: post_grid,
        }
    }
}

/// Payload for [`Mutation::SetLayerOpacity`].
#[derive(Debug, Clone)]
pub struct SetLayerOpacity {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// Value to write.
    pub new: f32,
    /// Pre-mutation value.
    pub old: f32,
}

impl ReverseStorage for SetLayerOpacity {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerOpacity: layer_idx out of range");
        debug_assert!(
            (layer.opacity - self.old).abs() < 1e-6,
            "SetLayerOpacity stale Reverse: layer.opacity={}, expected old={}",
            layer.opacity,
            self.old
        );
        layer.opacity = self.new;
        SetLayerOpacity {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetLayerEnabled`].
#[derive(Debug, Clone)]
pub struct SetLayerEnabled {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// Value to write.
    pub new: bool,
    /// Pre-mutation value.
    pub old: bool,
}

impl ReverseStorage for SetLayerEnabled {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerEnabled: layer_idx out of range");
        debug_assert!(
            layer.enabled == self.old,
            "SetLayerEnabled stale Reverse: layer.enabled={}, expected old={}",
            layer.enabled,
            self.old
        );
        layer.enabled = self.new;
        SetLayerEnabled {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetLayerBlendMode`].
/// Whole-enum Reverse (rule 1): stores the full old `BlendMode` value.
#[derive(Debug, Clone)]
pub struct SetLayerBlendMode {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// Value to write.
    pub new: BlendMode,
    /// Pre-mutation value (full enum — Reverse rule 1).
    pub old: BlendMode,
}

impl ReverseStorage for SetLayerBlendMode {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerBlendMode: layer_idx out of range");
        debug_assert!(
            layer.blend_mode == self.old,
            "SetLayerBlendMode stale Reverse: layer.blend_mode={:?}, expected old={:?}",
            layer.blend_mode,
            self.old
        );
        layer.blend_mode = self.new;
        SetLayerBlendMode {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetLayerEffects`].
///
/// Replaces a layer's effect chain wholesale (Reverse rule 2: Effects-Vec
/// Reverse). Both `new` and `old` are full `Vec<Effect>` snapshots.
#[derive(Debug, Clone)]
pub struct SetLayerEffects {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// Effect chain to install.
    pub new: Vec<crate::effects::Effect>,
    /// Pre-mutation snapshot of the chain.
    pub old: Vec<crate::effects::Effect>,
}

impl ReverseStorage for SetLayerEffects {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerEffects: layer_idx out of range");
        debug_assert!(
            layer.effects.len() == self.old.len(),
            "SetLayerEffects stale Reverse: effects.len()={}, expected old.len()={}",
            layer.effects.len(),
            self.old.len()
        );
        let post = self.new;
        layer.effects = post.clone();
        SetLayerEffects {
            layer_idx: self.layer_idx,
            new: self.old,
            old: post,
        }
    }
}

/// 004-T1.13 — Payload for [`Mutation::SetLayerEffectsAndMask`].
///
/// Symmetric combined snapshot: replaces both `LayerConfig.effects`
/// and `LayerConfig.warp.mask_polygon` atomically. Used by smart-fill
/// on add (spec D): when the operator adds an SDF-keyed preset to a
/// layer with empty `warp.mask_polygon`, the same mutation also
/// seeds a full-quad mask. Single undo step.
///
/// Both `new`/`old` pairs are full snapshots (Reverse rules 1 + 2:
/// whole-Vec mask polygon + whole-Vec effect chain).
#[derive(Debug, Clone)]
pub struct SetLayerEffectsAndMask {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// Effect chain to install.
    pub new_effects: Vec<crate::effects::Effect>,
    /// Pre-mutation snapshot of the effect chain.
    pub old_effects: Vec<crate::effects::Effect>,
    /// Mask polygon to install.
    pub new_mask_polygon: Vec<[f32; 2]>,
    /// Pre-mutation snapshot of the mask polygon.
    pub old_mask_polygon: Vec<[f32; 2]>,
}

impl ReverseStorage for SetLayerEffectsAndMask {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerEffectsAndMask: layer_idx out of range");
        debug_assert!(
            layer.effects.len() == self.old_effects.len(),
            "SetLayerEffectsAndMask stale Reverse: effects.len()={}, expected old.len()={}",
            layer.effects.len(),
            self.old_effects.len()
        );
        debug_assert!(
            layer.warp.mask_polygon.len() == self.old_mask_polygon.len(),
            "SetLayerEffectsAndMask stale Reverse: mask_polygon.len()={}, expected old.len()={}",
            layer.warp.mask_polygon.len(),
            self.old_mask_polygon.len()
        );
        let post_effects = self.new_effects;
        let post_mask = self.new_mask_polygon;
        layer.effects = post_effects.clone();
        layer.warp.mask_polygon = post_mask.clone();
        SetLayerEffectsAndMask {
            layer_idx: self.layer_idx,
            new_effects: self.old_effects,
            old_effects: post_effects,
            new_mask_polygon: self.old_mask_polygon,
            old_mask_polygon: post_mask,
        }
    }
}

/// Payload for [`Mutation::SwapLayers`]. Self-reverse.
#[derive(Debug, Clone)]
pub struct SwapLayers {
    /// First swap index.
    pub i: usize,
    /// Second swap index.
    pub j: usize,
}

impl ReverseStorage for SwapLayers {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            self.i < project.layers.len() && self.j < project.layers.len(),
            "SwapLayers index out of range: i={}, j={}, len={}",
            self.i,
            self.j,
            project.layers.len()
        );
        project.layers.swap(self.i, self.j);
        SwapLayers {
            i: self.i,
            j: self.j,
        }
    }
}

/// Payload for [`Mutation::RelinkAssetPath`].
///
/// Repoints a layer's asset path to a new location on disk. Both paths are
/// stored alongside the layer index so the proptest round-trip works without
/// re-reading project state at undo time.
#[derive(Debug, Clone)]
pub struct RelinkAssetPath {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// Replacement asset path.
    pub new_path: PathBuf,
    /// Pre-mutation asset path.
    pub old_path: PathBuf,
}

impl ReverseStorage for RelinkAssetPath {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("RelinkAssetPath: layer_idx out of range");
        debug_assert_eq!(
            layer.kind.asset_path(),
            Some(self.old_path.as_path()),
            "RelinkAssetPath: stale old_path for layer_idx={}",
            self.layer_idx,
        );
        match &mut layer.kind {
            crate::project::schema::LayerKind::Image { path, .. } => {
                *path = self.new_path.clone();
            }
            crate::project::schema::LayerKind::Svg { svg_path } => {
                *svg_path = self.new_path.clone();
            }
            crate::project::schema::LayerKind::Video { path, .. } => {
                *path = self.new_path.clone();
            }
            crate::project::schema::LayerKind::FxLayer { .. }
            | crate::project::schema::LayerKind::Ndi { .. } => {
                // RelinkAssetPath is only emitted by the missing-asset
                // audit (T1.38), which P0.1.2 gates on `asset_path()
                // .is_some()`. Variants without an asset path are
                // unreachable here; left explicit for exhaustiveness.
                debug_assert!(
                    false,
                    "RelinkAssetPath dispatched against {:?} which has no asset path",
                    layer.kind,
                );
            }
        }
        RelinkAssetPath {
            layer_idx: self.layer_idx,
            new_path: self.old_path,
            old_path: self.new_path,
        }
    }
}

/// Payload for [`Mutation::SetLayerKind`].
///
/// P0.5.1 — wholesale replace `LayerConfig.kind` for the layer at
/// `layer_idx`. Used to switch FX layer presets, change `params` on
/// an FxLayer, or relink an entire layer's source type. Whole-enum
/// Reverse (rule 1 in `src/project/CLAUDE.md`) — the prior
/// `LayerKind` is stored in `old` so a Sine → Static-style payload
/// loss is impossible.
///
/// Asymmetric in spirit (it can change the variant) but symmetric
/// in shape: the reverse is another `SetLayerKind` with values
/// swapped, fitting the `ReverseStorage` trait without needing the
/// AddLayer/RemoveLayer-style cross-variant exception.
#[derive(Debug, Clone)]
pub struct SetLayerKind {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// New `LayerKind` to install.
    pub new: crate::project::schema::LayerKind,
    /// Pre-mutation `LayerKind`; `apply` `debug_assert!`s the layer's
    /// current kind has the same shape (variant discriminant) so a
    /// stale Reverse panics in tests rather than silently corrupts.
    pub old: crate::project::schema::LayerKind,
}

impl ReverseStorage for SetLayerKind {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerKind: layer_idx out of range");
        debug_assert_eq!(
            std::mem::discriminant(&layer.kind),
            std::mem::discriminant(&self.old),
            "SetLayerKind stale Reverse: live discriminant != self.old discriminant for layer_idx={}",
            self.layer_idx,
        );
        layer.kind = self.new;
        SetLayerKind {
            layer_idx: self.layer_idx,
            new: self.old,
            old: layer.kind.clone(),
        }
    }
}

/// P1.2.1 — Payload for [`Mutation::SetLayerTreatment`].
///
/// Whole-`Option` Reverse so a `None ↔ Some(Treatment)` toggle
/// round-trips byte-equally. This is the mutation that handles
/// preset switches AND `overlay_path` / `collage_paths` edits — the
/// UI constructs a new `Treatment` with the desired field values and
/// dispatches this. Per-field path mutations would risk silently
/// dropping unrelated fields on the `None → Some` direction.
#[derive(Debug, Clone)]
pub struct SetLayerTreatment {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// Value to write (`None` removes the treatment).
    pub new: Option<crate::project::schema::Treatment>,
    /// Pre-mutation value.
    pub old: Option<crate::project::schema::Treatment>,
}

impl ReverseStorage for SetLayerTreatment {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerTreatment: layer_idx out of range");
        debug_assert_eq!(
            layer.treatment, self.old,
            "SetLayerTreatment stale Reverse for layer_idx={}",
            self.layer_idx,
        );
        layer.treatment = self.new.clone();
        SetLayerTreatment {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// P1.2.1 — Payload for [`Mutation::SetLayerTreatmentParams`].
///
/// Whole-`HashMap` snapshot Reverse (parallels `SetFxLayerParams`
/// from P0.5.1). Per-key Reverse would lose unrelated keys silently
/// when a preset switch races a param edit. Only touches the
/// `params` field — `preset_id`, `overlay_path`, and `collage_paths`
/// flow through [`SetLayerTreatment`].
///
/// **Apply contract:** the layer's `treatment` must be `Some` at
/// apply time. The mutation panics with a clear message otherwise —
/// the UI dispatch site is responsible for guarding (param sliders
/// only render when treatment is active).
#[derive(Debug, Clone)]
pub struct SetLayerTreatmentParams {
    /// Index into `Project.layers`. Layer must have `treatment.is_some()`.
    pub layer_idx: usize,
    /// Replacement params HashMap (replaces the whole map, not per-key).
    pub new: std::collections::HashMap<String, f32>,
    /// Pre-mutation params (snapshot; `apply` `debug_assert!`s match).
    pub old: std::collections::HashMap<String, f32>,
}

impl ReverseStorage for SetLayerTreatmentParams {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerTreatmentParams: layer_idx out of range");
        let treatment = layer.treatment.as_mut().expect(
            "SetLayerTreatmentParams: layer has no treatment — \
             dispatch site must guard via `treatment.is_some()`",
        );
        debug_assert_eq!(
            treatment.params, self.old,
            "SetLayerTreatmentParams stale Reverse for layer_idx={}",
            self.layer_idx,
        );
        treatment.params = self.new.clone();
        SetLayerTreatmentParams {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// P2.5.6 — Payload for [`Mutation::SetFxLayerParams`].
///
/// Whole-`HashMap` snapshot Reverse (rule 1) for `FxLayer.params`.
/// Mirrors `SetLayerTreatmentParams`. Preset switches still go through
/// `SetLayerKind`; this variant handles lightweight per-param slider
/// edits without churning the whole `LayerKind`.
///
/// **Apply contract:** the layer must be `LayerKind::FxLayer` at apply
/// time. Panics otherwise — the UI dispatch site is responsible for
/// guarding (param sliders only render for FxLayer).
///
/// **Budget refusal:** if `new` contains a key whose value exceeds the
/// descriptor's `max_particle_count`, `apply` returns a no-op Reverse
/// (new == old) and leaves the project unchanged. The UI pre-flight
/// helper `Project::fx_layer_params_over_budget` detects this before
/// constructing the mutation and shows a warning toast instead.
#[derive(Debug, Clone)]
pub struct SetFxLayerParams {
    /// Index into `Project.layers`. Layer must be `LayerKind::FxLayer`.
    pub layer_idx: usize,
    /// Replacement params HashMap (replaces the whole map, not per-key).
    pub new: std::collections::HashMap<String, f32>,
    /// Pre-mutation params (snapshot; `apply` `debug_assert!`s match).
    pub old: std::collections::HashMap<String, f32>,
}

impl ReverseStorage for SetFxLayerParams {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetFxLayerParams: layer_idx out of range");
        let (preset_id, current_params) = match &mut layer.kind {
            crate::project::schema::LayerKind::FxLayer {
                preset_id, params, ..
            } => (preset_id.clone(), params),
            _ => panic!("SetFxLayerParams: layer is not FxLayer — dispatch site must guard"),
        };
        debug_assert_eq!(
            *current_params, self.old,
            "SetFxLayerParams stale Reverse for layer_idx={}",
            self.layer_idx,
        );

        // Budget check: refuse if any param exceeds its max_particle_count.
        if let Some(over) = particle_budget_exceeded(&preset_id, &self.new) {
            tracing::warn!(
                layer_idx = self.layer_idx,
                key = %over.0,
                value = over.1,
                max = over.2,
                "SetFxLayerParams: refused (over particle budget)",
            );
            // Return a no-op Reverse: new == old == current state.
            // The UI detects refusal via the pre-flight helper; the project
            // is unchanged so the slider snaps back on the next frame.
            return SetFxLayerParams {
                layer_idx: self.layer_idx,
                new: self.old.clone(),
                old: self.old,
            };
        }

        *current_params = self.new.clone();
        SetFxLayerParams {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// P2.5.6 — Returns `Some((key, value, max))` when `params` would
/// exceed any descriptor's `max_particle_count` budget for the given
/// preset. Returns `None` when all params are within budget.
fn particle_budget_exceeded(
    preset_id: &str,
    params: &std::collections::HashMap<String, f32>,
) -> Option<(String, f32, u32)> {
    for d in crate::render::fx_presets::fx_param_descriptors(preset_id) {
        if let Some(max) = d.max_particle_count {
            if let Some(&val) = params.get(d.key) {
                if val > max as f32 {
                    return Some((d.key.to_string(), val, max));
                }
            }
        }
    }
    None
}

/// Payload for [`Mutation::SetModulator`].
///
/// Whole-enum Reverse (rule 1): stores the full old `Modulator` value so a
/// variant switch (e.g. Sine → Static) round-trips byte-equally.
#[derive(Debug, Clone)]
pub struct SetModulator {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// Index into `LayerConfig.effects`.
    pub effect_idx: usize,
    /// Which modulator slot inside the effect.
    pub field: ModulatorField,
    /// Replacement modulator (full enum value).
    pub new: crate::modulators::Modulator,
    /// Pre-mutation modulator (full enum value — Reverse rule 1).
    pub old: crate::modulators::Modulator,
}

impl ReverseStorage for SetModulator {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetModulator: layer_idx out of range");
        let effect = layer
            .effects
            .get_mut(self.effect_idx)
            .expect("SetModulator: effect_idx out of range");
        let slot = modulator_at_mut(effect, self.field)
            .expect("SetModulator: field does not apply to this effect variant");
        *slot = self.new.clone();
        SetModulator {
            layer_idx: self.layer_idx,
            effect_idx: self.effect_idx,
            field: self.field,
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::ResetLayerWarpMesh`].
///
/// Replaces the entire `WarpMesh` at `layer_idx` (rule 3 snapshot Reverse).
#[derive(Debug, Clone)]
pub struct ResetLayerWarpMesh {
    /// Index into `Project.layers`; the layer's `warp` is the target.
    pub layer_idx: usize,
    /// Full `WarpMesh` to install.
    pub new: crate::project::schema::WarpMesh,
    /// Pre-mutation `WarpMesh` snapshot.
    pub old: crate::project::schema::WarpMesh,
}

impl ReverseStorage for ResetLayerWarpMesh {
    fn apply(self, project: &mut Project) -> Self {
        let warp = &mut project
            .layers
            .get_mut(self.layer_idx)
            .expect("ResetLayerWarpMesh: layer_idx out of range")
            .warp;
        debug_assert!(
            warp.rows == self.old.rows && warp.cols == self.old.cols,
            "ResetLayerWarpMesh stale Reverse: warp dims=({}, {}), expected old=({}, {})",
            warp.rows,
            warp.cols,
            self.old.rows,
            self.old.cols
        );
        let post = self.new;
        *warp = post.clone();
        ResetLayerWarpMesh {
            layer_idx: self.layer_idx,
            new: self.old,
            old: post,
        }
    }
}

/// P7.3.1 — Payload for [`Mutation::ResetLayerBezierMesh`].
///
/// Replaces the entire `BezierMesh` for the layer at `layer_idx`
/// (rule 3 snapshot Reverse — whole-Option snapshot).
#[derive(Debug, Clone)]
pub struct ResetLayerBezierMesh {
    /// Index into `Project.layers`; the layer's `bezier_mesh` is the target.
    pub layer_idx: usize,
    /// `BezierMesh` to install (Some) or clear (None).
    pub new: Option<crate::project::schema::BezierMesh>,
    /// Pre-mutation `bezier_mesh` snapshot.
    pub old: Option<crate::project::schema::BezierMesh>,
}

impl ReverseStorage for ResetLayerBezierMesh {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("ResetLayerBezierMesh: layer_idx out of range");
        // Debug-assert structural match: compare rows/cols when both are Some.
        if let (Some(current), Some(expected)) = (&layer.bezier_mesh, &self.old) {
            debug_assert!(
                current.rows == expected.rows && current.cols == expected.cols,
                "ResetLayerBezierMesh stale Reverse: bezier_mesh dims=({}, {}), \
                 expected old=({}, {})",
                current.rows,
                current.cols,
                expected.rows,
                expected.cols,
            );
        }
        let post = self.new;
        layer.bezier_mesh = post.clone();
        ResetLayerBezierMesh {
            layer_idx: self.layer_idx,
            new: self.old,
            old: post,
        }
    }
}

/// P7.3.3 — Payload for [`Mutation::MoveBezierAnchor`].
///
/// Moves a single anchor in `BezierMesh.anchors[row][col]` for the layer at
/// `layer_idx` and propagates its handles rigidly (the handle offsets relative to
/// the anchor are preserved so the curve shape doesn't change during drag).
///
/// Uses a per-field `[f32; 2]` Reverse — symmetric because the anchor position
/// is a leaf value, not a whole-enum or effects-vec situation.
#[derive(Debug, Clone)]
pub struct MoveBezierAnchor {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// Row of the anchor in `BezierMesh.anchors` (0..=rows).
    pub anchor_row: usize,
    /// Column of the anchor in `BezierMesh.anchors` (0..=cols).
    pub anchor_col: usize,
    /// New position to install `[x, y]` in normalised projector-space.
    pub new_pos: [f32; 2],
    /// Pre-mutation position snapshot — restored on undo.
    pub old_pos: [f32; 2],
    /// PCleanup.bezier-handle-reverse — pre-mutation snapshot of the
    /// horizontal-tangent handle at this anchor.  Apply propagates the
    /// anchor delta to handles via float `+= delta`, which is NOT
    /// bit-exact on reverse (float subtraction can drift by 1 ULP per
    /// round-trip).  Storing the original handle bits restores them
    /// exactly on undo, satisfying the V31.3.2 ReverseStorage trait
    /// invariant (every `Mutation::apply` opens with `debug_assert!`
    /// that the carried `old` value matches current state).
    pub old_h_handle: Option<[f32; 2]>,
    /// Pre-mutation snapshot of the vertical-tangent handle.  See
    /// [`Self::old_h_handle`] for the rationale.
    pub old_v_handle: Option<[f32; 2]>,
}

impl ReverseStorage for MoveBezierAnchor {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("MoveBezierAnchor: layer_idx out of range");
        let bm = layer
            .bezier_mesh
            .as_mut()
            .expect("MoveBezierAnchor: layer has no bezier_mesh");
        let anchor = bm
            .anchors
            .get_mut(self.anchor_row)
            .and_then(|row| row.get_mut(self.anchor_col))
            .expect("MoveBezierAnchor: anchor_row/col out of range");
        debug_assert_eq!(
            *anchor, self.old_pos,
            "MoveBezierAnchor stale Reverse: anchor=({}, {}), \
             current={:?}, expected old={:?}",
            self.anchor_row, self.anchor_col, anchor, self.old_pos,
        );
        *anchor = self.new_pos;

        // PCleanup.bezier-handle-reverse — two paths:
        //
        // FORWARD (from `move_bezier_anchor_mutation`): both `old_h_handle`
        // and `old_v_handle` are `None`.  Propagate the anchor delta to
        // each handle via float `+= delta`, then SNAPSHOT the result.
        // The returned reverse mutation carries those snapshots so undo
        // can restore them bit-exact (no float arithmetic on the reverse).
        //
        // REVERSE (from a prior apply): the snapshots are `Some(...)`.
        // Restore handles directly from the snapshot — no delta math —
        // so the round-trip is bit-exact and the proptest invariant
        // (apply N → undo N → byte-equal to start) holds.
        let restore_mode = self.old_h_handle.is_some() || self.old_v_handle.is_some();
        let mut captured_h: Option<[f32; 2]> = None;
        let mut captured_v: Option<[f32; 2]> = None;

        if restore_mode {
            // Restore from snapshots — bit-exact.
            if let Some(slot) = bm
                .handles_h
                .get_mut(self.anchor_row)
                .and_then(|row| row.get_mut(self.anchor_col))
            {
                *slot = self.old_h_handle;
            }
            if let Some(slot) = bm
                .handles_v
                .get_mut(self.anchor_row)
                .and_then(|row| row.get_mut(self.anchor_col))
            {
                *slot = self.old_v_handle;
            }
        } else {
            // Forward delta-propagate.  CAPTURE the pre-apply handle bits
            // (which become `old_*_handle` for the reverse — undoing this
            // mutation restores those bits exactly), then mutate.
            let delta = [
                self.new_pos[0] - self.old_pos[0],
                self.new_pos[1] - self.old_pos[1],
            ];
            if let Some(slot) = bm
                .handles_h
                .get_mut(self.anchor_row)
                .and_then(|row| row.get_mut(self.anchor_col))
            {
                if let Some(pos) = slot.as_mut() {
                    captured_h = Some(*pos); // PRE-apply value
                    pos[0] += delta[0];
                    pos[1] += delta[1];
                }
            }
            if let Some(slot) = bm
                .handles_v
                .get_mut(self.anchor_row)
                .and_then(|row| row.get_mut(self.anchor_col))
            {
                if let Some(pos) = slot.as_mut() {
                    captured_v = Some(*pos); // PRE-apply value
                    pos[0] += delta[0];
                    pos[1] += delta[1];
                }
            }
        }

        MoveBezierAnchor {
            layer_idx: self.layer_idx,
            anchor_row: self.anchor_row,
            anchor_col: self.anchor_col,
            new_pos: self.old_pos,
            old_pos: self.new_pos,
            // After a FORWARD apply, captured_* holds the PRE-apply
            // handle bits — these are the values undo needs to restore.
            // After a REVERSE apply, the snapshots should be None (the
            // next forward apply re-captures from pre-state).
            old_h_handle: captured_h,
            old_v_handle: captured_v,
        }
    }
}

/// P7.3.3 — Payload for [`Mutation::SetBezierHandle`].
///
/// Sets (or clears) a single tangent handle at `BezierMesh.handles_h[row][col]`
/// or `handles_v[row][col]` for the layer at `layer_idx`.
///
/// Per-field `Option<[f32; 2]>` Reverse — symmetric because a handle is a leaf
/// optional value; no enum-of-structs or effects-vec concern applies.
#[derive(Debug, Clone)]
pub struct SetBezierHandle {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// Row of the target anchor (0..=rows).
    pub anchor_row: usize,
    /// Column of the target anchor (0..=cols).
    pub anchor_col: usize,
    /// Which tangent slot to update.
    pub direction: crate::project::schema::BezierHandleDir,
    /// New handle position (`None` = clear to degenerate/straight).
    pub new_pos: crate::project::schema::BezierHandle,
    /// Pre-mutation handle snapshot.
    pub old_pos: crate::project::schema::BezierHandle,
}

impl ReverseStorage for SetBezierHandle {
    fn apply(self, project: &mut Project) -> Self {
        use crate::project::schema::BezierHandleDir;
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetBezierHandle: layer_idx out of range");
        let bm = layer
            .bezier_mesh
            .as_mut()
            .expect("SetBezierHandle: layer has no bezier_mesh");
        let handle = match self.direction {
            BezierHandleDir::Horizontal => bm
                .handles_h
                .get_mut(self.anchor_row)
                .and_then(|row| row.get_mut(self.anchor_col))
                .expect("SetBezierHandle: anchor_row/col out of range for handles_h"),
            BezierHandleDir::Vertical => bm
                .handles_v
                .get_mut(self.anchor_row)
                .and_then(|row| row.get_mut(self.anchor_col))
                .expect("SetBezierHandle: anchor_row/col out of range for handles_v"),
        };
        debug_assert_eq!(
            *handle, self.old_pos,
            "SetBezierHandle stale Reverse: handle dir={:?} ({}, {}), \
             current={:?}, expected old={:?}",
            self.direction, self.anchor_row, self.anchor_col, handle, self.old_pos,
        );
        *handle = self.new_pos;
        SetBezierHandle {
            layer_idx: self.layer_idx,
            anchor_row: self.anchor_row,
            anchor_col: self.anchor_col,
            direction: self.direction,
            new_pos: self.old_pos,
            old_pos: self.new_pos,
        }
    }
}

/// P7.5.1 / P7.6.1 — Payload for [`Mutation::SetLayerMaskGraph`].
///
/// Replaces the entire `MaskGraph` on `LayerConfig.mask_graph` for the
/// layer at `layer_idx` (whole-Option snapshot Reverse — rule 3).
/// Used for inverse mask, luma key, and chroma key toggling.
#[derive(Debug, Clone)]
pub struct SetLayerMaskGraph {
    /// Index into `Project.layers`; `layer.mask_graph` is the target.
    pub layer_idx: usize,
    /// New `MaskGraph` to install (`Some`) or clear (`None`).
    pub new: Option<crate::project::schema::MaskGraph>,
    /// Pre-mutation snapshot.
    pub old: Option<crate::project::schema::MaskGraph>,
}

impl ReverseStorage for SetLayerMaskGraph {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerMaskGraph: layer_idx out of range");
        let post = self.new;
        layer.mask_graph = post.clone();
        SetLayerMaskGraph {
            layer_idx: self.layer_idx,
            new: self.old,
            old: post,
        }
    }
}

/// Payload for [`Mutation::SetLayerMaskPolygon`].
///
/// Replaces `WarpMesh.mask_polygon` for the layer at `layer_idx`.
/// Both sides are full polygon snapshots (whole-Vec Reverse).
#[derive(Debug, Clone)]
pub struct SetLayerMaskPolygon {
    /// Index into `Project.layers`; the layer's `warp` is the target.
    pub layer_idx: usize,
    /// Polygon to install.
    pub new: Vec<[f32; 2]>,
    /// Pre-mutation polygon snapshot.
    pub old: Vec<[f32; 2]>,
}

impl ReverseStorage for SetLayerMaskPolygon {
    fn apply(self, project: &mut Project) -> Self {
        let warp = &mut project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerMaskPolygon: layer_idx out of range")
            .warp;
        debug_assert!(
            warp.mask_polygon.len() == self.old.len(),
            "SetLayerMaskPolygon stale Reverse: mask_polygon.len()={}, expected old.len()={}",
            warp.mask_polygon.len(),
            self.old.len()
        );
        let post = self.new;
        warp.mask_polygon = post.clone();
        SetLayerMaskPolygon {
            layer_idx: self.layer_idx,
            new: self.old,
            old: post,
        }
    }
}

/// Payload for [`Mutation::SetLayerMaskVertex`].
///
/// Replaces `WarpMesh.mask_polygon[idx]` with `new`. Reverse swaps `new` and `old`.
#[derive(Debug, Clone)]
pub struct SetLayerMaskVertex {
    /// Index into `Project.layers`; the layer's `warp` is the target.
    pub layer_idx: usize,
    /// Index of the vertex inside `mask_polygon`.
    pub idx: usize,
    /// Value to write.
    pub new: [f32; 2],
    /// Pre-mutation value; `apply` `debug_assert!`s this matches the live state.
    pub old: [f32; 2],
}

impl ReverseStorage for SetLayerMaskVertex {
    fn apply(self, project: &mut Project) -> Self {
        let warp = &mut project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerMaskVertex: layer_idx out of range")
            .warp;
        debug_assert!(
            self.idx < warp.mask_polygon.len(),
            "SetLayerMaskVertex idx out of range: idx={}, len={}",
            self.idx,
            warp.mask_polygon.len()
        );
        let cur = warp.mask_polygon[self.idx];
        debug_assert!(
            (cur[0] - self.old[0]).abs() < 1e-6 && (cur[1] - self.old[1]).abs() < 1e-6,
            "SetLayerMaskVertex stale Reverse: cur=[{}, {}], expected old=[{}, {}]",
            cur[0],
            cur[1],
            self.old[0],
            self.old[1]
        );
        warp.mask_polygon[self.idx] = self.new;
        SetLayerMaskVertex {
            layer_idx: self.layer_idx,
            idx: self.idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetLayerWarpCorner`].
///
/// Replaces `WarpMesh.grid[r][c]` for the layer at `layer_idx`.
#[derive(Debug, Clone)]
pub struct SetLayerWarpCorner {
    /// Index into `Project.layers`; the layer's `warp` is the target.
    pub layer_idx: usize,
    /// Grid row index (vertex coords, 0..=warp.rows).
    pub r: usize,
    /// Grid column index (vertex coords, 0..=warp.cols).
    pub c: usize,
    /// Value to write.
    pub new: [f32; 2],
    /// Pre-mutation value; `apply` `debug_assert!`s this matches the live state.
    pub old: [f32; 2],
}

impl ReverseStorage for SetLayerWarpCorner {
    fn apply(self, project: &mut Project) -> Self {
        let warp = &mut project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerWarpCorner: layer_idx out of range")
            .warp;
        debug_assert!(
            (warp.grid[self.r][self.c][0] - self.old[0]).abs() < 1e-6
                && (warp.grid[self.r][self.c][1] - self.old[1]).abs() < 1e-6,
            "SetLayerWarpCorner stale Reverse: cur=[{}, {}], expected old=[{}, {}]",
            warp.grid[self.r][self.c][0],
            warp.grid[self.r][self.c][1],
            self.old[0],
            self.old[1]
        );
        warp.grid[self.r][self.c] = self.new;
        SetLayerWarpCorner {
            layer_idx: self.layer_idx,
            r: self.r,
            c: self.c,
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetOutputRgbMatrix`].
///
/// P0.8.1 — extends the original P0.8.2 stub with `output_idx: usize`,
/// per the forward-looking comment added at P0.8.2 time: "the W7.1
/// `Vec<OutputTarget>` rename will extend this with `output_index: usize`."
/// Per-cell edits flow through this single Mutation as whole-matrix
/// replacements: simpler and matches the per-OutputTarget granularity.
#[derive(Debug, Clone)]
pub struct SetOutputRgbMatrix {
    /// Index into `Project.output_targets`. Panics on out-of-range in
    /// `apply` so misconfigured call sites surface immediately.
    pub output_idx: usize,
    /// 3×3 matrix to install. Row-major: row 0 maps RGB → R, etc.
    pub new: [[f32; 3]; 3],
    /// Pre-mutation matrix; `apply` `debug_assert_eq!`s this matches.
    pub old: [[f32; 3]; 3],
}

impl ReverseStorage for SetOutputRgbMatrix {
    fn apply(self, project: &mut Project) -> Self {
        let live = project
            .output_targets
            .get(self.output_idx)
            .expect("SetOutputRgbMatrix: output_idx out of range")
            .rgb_matrix;
        debug_assert_eq!(
            live, self.old,
            "SetOutputRgbMatrix stale Reverse: live matrix != self.old",
        );
        project
            .output_targets
            .get_mut(self.output_idx)
            .expect("SetOutputRgbMatrix: output_idx out of range")
            .rgb_matrix = self.new;
        SetOutputRgbMatrix {
            output_idx: self.output_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// PCleanup.7.3 — Payload for [`Mutation::SetOutputGammaOverride`].
/// Whole-`Option` Reverse so `Some → None → Some` round-trips byte-equally,
/// matching the `SetProjectGammaOverride` precedent.
#[derive(Debug, Clone)]
pub struct SetOutputGammaOverride {
    /// Index into `Project.output_targets`. Panics if out-of-range in `apply`.
    pub output_idx: usize,
    /// New gamma override value (`None` = clear per-output trim).
    pub new: Option<f32>,
    /// Pre-mutation value; `apply` `debug_assert_eq!`s this matches.
    pub old: Option<f32>,
}

impl ReverseStorage for SetOutputGammaOverride {
    fn apply(self, project: &mut Project) -> Self {
        let live = project
            .output_targets
            .get(self.output_idx)
            .expect("SetOutputGammaOverride: output_idx out of range")
            .gamma_override;
        debug_assert_eq!(
            live, self.old,
            "SetOutputGammaOverride stale Reverse: live={:?} expected old={:?}",
            live, self.old,
        );
        project
            .output_targets
            .get_mut(self.output_idx)
            .expect("SetOutputGammaOverride: output_idx out of range")
            .gamma_override = self.new;
        SetOutputGammaOverride {
            output_idx: self.output_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// PCleanup.7.3 — Payload for [`Mutation::SetOutputBrightnessOverride`].
#[derive(Debug, Clone)]
pub struct SetOutputBrightnessOverride {
    /// Index into `Project.output_targets`. Panics if out-of-range in `apply`.
    pub output_idx: usize,
    /// New brightness override value (`None` = clear per-output trim).
    pub new: Option<f32>,
    /// Pre-mutation value; `apply` `debug_assert_eq!`s this matches.
    pub old: Option<f32>,
}

impl ReverseStorage for SetOutputBrightnessOverride {
    fn apply(self, project: &mut Project) -> Self {
        let live = project
            .output_targets
            .get(self.output_idx)
            .expect("SetOutputBrightnessOverride: output_idx out of range")
            .brightness_override;
        debug_assert_eq!(
            live, self.old,
            "SetOutputBrightnessOverride stale Reverse: live={:?} expected old={:?}",
            live, self.old,
        );
        project
            .output_targets
            .get_mut(self.output_idx)
            .expect("SetOutputBrightnessOverride: output_idx out of range")
            .brightness_override = self.new;
        SetOutputBrightnessOverride {
            output_idx: self.output_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// PCleanup.7.3 — Payload for [`Mutation::SetOutputContrastOverride`].
#[derive(Debug, Clone)]
pub struct SetOutputContrastOverride {
    /// Index into `Project.output_targets`. Panics if out-of-range in `apply`.
    pub output_idx: usize,
    /// New contrast override value (`None` = clear per-output trim).
    pub new: Option<f32>,
    /// Pre-mutation value; `apply` `debug_assert_eq!`s this matches.
    pub old: Option<f32>,
}

impl ReverseStorage for SetOutputContrastOverride {
    fn apply(self, project: &mut Project) -> Self {
        let live = project
            .output_targets
            .get(self.output_idx)
            .expect("SetOutputContrastOverride: output_idx out of range")
            .contrast_override;
        debug_assert_eq!(
            live, self.old,
            "SetOutputContrastOverride stale Reverse: live={:?} expected old={:?}",
            live, self.old,
        );
        project
            .output_targets
            .get_mut(self.output_idx)
            .expect("SetOutputContrastOverride: output_idx out of range")
            .contrast_override = self.new;
        SetOutputContrastOverride {
            output_idx: self.output_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::SetProjectScenes`].
///
/// Replaces `Project.scenes` wholesale (whole-Vec snapshot Reverse).
#[derive(Debug, Clone)]
pub struct SetProjectScenes {
    /// Replacement scenes Vec.
    pub new: Vec<crate::project::schema::Cue>,
    /// Pre-mutation scenes Vec.
    pub old: Vec<crate::project::schema::Cue>,
}

impl ReverseStorage for SetProjectScenes {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.cues.len() == self.old.len(),
            "SetProjectScenes stale Reverse: scenes.len()={}, expected old.len()={}",
            project.cues.len(),
            self.old.len()
        );
        let post = self.new;
        project.cues = post.clone();
        SetProjectScenes {
            new: self.old,
            old: post,
        }
    }
}

// ---------------------------------------------------------------------------
// P6.2.2 — Per-cue timing mutations
// ---------------------------------------------------------------------------

/// P6.2.2 — Snapshot of all timing + binding fields on a single [`Cue`] for
/// atomic undo/redo. Storing the whole snapshot prevents stale-Reverse
/// corruption when future fields are added to `Cue`.
///
/// Follows Reverse-storage rule 2 (whole-struct snapshot, not per-field).
#[derive(Debug, Clone)]
#[allow(missing_docs)] // Fields mirror Cue struct; see schema.rs for field docs.
pub struct CueTimingSnapshot {
    pub in_time_s: f32,
    pub hold_time_s: Option<f32>,
    pub out_time_s: f32,
    pub fire_mode: crate::project::schema::CueFireMode,
    pub bpm_quantize: crate::project::schema::BpmQuantize,
    pub timecode_trigger: Option<crate::project::schema::TimecodePosition>,
    pub in_time_binding: Option<crate::project::schema::CcBinding>,
    pub hold_binding: Option<crate::project::schema::CcBinding>,
    pub out_time_binding: Option<crate::project::schema::CcBinding>,
    pub in_time_osc: Option<crate::project::schema::OscBinding>,
    pub hold_osc: Option<crate::project::schema::OscBinding>,
    pub out_time_osc: Option<crate::project::schema::OscBinding>,
}

impl CueTimingSnapshot {
    /// Capture all timing and binding fields from a cue.
    pub fn from_cue(cue: &crate::project::schema::Cue) -> Self {
        CueTimingSnapshot {
            in_time_s: cue.in_time_s,
            hold_time_s: cue.hold_time_s,
            out_time_s: cue.out_time_s,
            fire_mode: cue.fire_mode,
            bpm_quantize: cue.bpm_quantize,
            timecode_trigger: cue.timecode_trigger,
            in_time_binding: cue.in_time_binding.clone(),
            hold_binding: cue.hold_binding.clone(),
            out_time_binding: cue.out_time_binding.clone(),
            in_time_osc: cue.in_time_osc.clone(),
            hold_osc: cue.hold_osc.clone(),
            out_time_osc: cue.out_time_osc.clone(),
        }
    }

    /// Apply all fields in this snapshot to the given cue.
    pub fn apply_to_cue(&self, cue: &mut crate::project::schema::Cue) {
        cue.in_time_s = self.in_time_s;
        cue.hold_time_s = self.hold_time_s;
        cue.out_time_s = self.out_time_s;
        cue.fire_mode = self.fire_mode;
        cue.bpm_quantize = self.bpm_quantize;
        cue.timecode_trigger = self.timecode_trigger;
        cue.in_time_binding = self.in_time_binding.clone();
        cue.hold_binding = self.hold_binding.clone();
        cue.out_time_binding = self.out_time_binding.clone();
        cue.in_time_osc = self.in_time_osc.clone();
        cue.hold_osc = self.hold_osc.clone();
        cue.out_time_osc = self.out_time_osc.clone();
    }
}

/// P6.2.2 — Payload for [`Mutation::SetCueName`]. Stores both old and new
/// strings for symmetric undo per Reverse-storage rule 1.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct SetCueName {
    pub cue_idx: usize,
    pub new: String,
    pub old: String,
}

impl ReverseStorage for SetCueName {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.cues.get(self.cue_idx).map(|c| &c.name) == Some(&self.old),
            "SetCueName stale Reverse: cue[{}].name={:?}, expected old={:?}",
            self.cue_idx,
            project.cues.get(self.cue_idx).map(|c| &c.name),
            self.old,
        );
        let post = self.new.clone();
        project.cues[self.cue_idx].name = post.clone();
        SetCueName {
            cue_idx: self.cue_idx,
            new: self.old,
            old: post,
        }
    }
}

/// P6.2.2 — Payload for [`Mutation::SetCueTiming`]. Whole-struct snapshot
/// Reverse (rule 2) so future additions to `Cue` don't silently corrupt undo.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct SetCueTiming {
    pub cue_idx: usize,
    pub new: CueTimingSnapshot,
    pub old: CueTimingSnapshot,
}

impl ReverseStorage for SetCueTiming {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.cues.get(self.cue_idx).is_some(),
            "SetCueTiming: cue_idx {} out of range (len={})",
            self.cue_idx,
            project.cues.len(),
        );
        let post = CueTimingSnapshot::from_cue(&project.cues[self.cue_idx]);
        self.new.apply_to_cue(&mut project.cues[self.cue_idx]);
        SetCueTiming {
            cue_idx: self.cue_idx,
            new: self.old,
            old: post,
        }
    }
}

/// P6.2.2 — Payload for [`Mutation::SetProjectCues`]. Replaces `Project.cues`
/// wholesale (whole-Vec snapshot Reverse, rule 3). Reorders / deletes / saves
/// all go through this single variant.
#[derive(Debug, Clone)]
#[allow(missing_docs)]
pub struct SetProjectCues {
    pub new: Vec<crate::project::schema::Cue>,
    pub old: Vec<crate::project::schema::Cue>,
}

impl ReverseStorage for SetProjectCues {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.cues.len() == self.old.len(),
            "SetProjectCues stale Reverse: cues.len()={}, expected old.len()={}",
            project.cues.len(),
            self.old.len()
        );
        let post = self.new;
        project.cues = post.clone();
        SetProjectCues {
            new: self.old,
            old: post,
        }
    }
}

/// Payload for [`Mutation::SetOutputMonitorIndex`].
#[derive(Debug, Clone)]
pub struct SetOutputMonitorIndex {
    /// Value to write.
    pub new: usize,
    /// Pre-mutation value; `apply` `debug_assert!`s this matches.
    pub old: usize,
}

impl ReverseStorage for SetOutputMonitorIndex {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.primary_output_target().fallback_index == self.old,
            "SetOutputMonitorIndex stale Reverse: project.primary_output_target().fallback_index={}, expected old={}",
            project.primary_output_target().fallback_index,
            self.old
        );
        project.primary_output_target_mut().fallback_index = self.new;
        SetOutputMonitorIndex {
            new: self.old,
            old: self.new,
        }
    }
}

/// Payload for [`Mutation::ApplyProjectSnapshot`].
///
/// Replaces the entire project from a serde_json snapshot (Reverse rule 3).
/// `non_undoable: true` is reserved for the crossfade-tick path which fires
/// ~60×/s and must not enter the user-facing undo stack.
#[derive(Debug, Clone)]
pub struct ApplyProjectSnapshot {
    /// Snapshot to install.
    pub new: serde_json::Value,
    /// Project state captured before the apply call.
    pub old: serde_json::Value,
    /// `true` for crossfade-tick callers; `false` for user-triggered scene recall.
    pub non_undoable: bool,
}

impl ReverseStorage for ApplyProjectSnapshot {
    fn apply(self, project: &mut Project) -> Self {
        // Errors from restore_scene used to be silenced (`let _ = ...`),
        // which masked any malformed-snapshot failure. The defensive
        // restore_scene fix preserves project.cues across failure too,
        // but log loudly here so a future occurrence is visible in
        // ~/Library/Logs/rmap/rmap.log instead of silent.
        if let Err(e) = crate::project::restore_scene(project, &self.new) {
            tracing::error!(
                ?e,
                "ApplyProjectSnapshot::apply: restore_scene failed; \
                 project.cues preserved by the defensive guard but other \
                 fields may be in inconsistent state",
            );
        }
        ApplyProjectSnapshot {
            new: self.old,
            old: self.new,
            non_undoable: self.non_undoable,
        }
    }
}

/// P0.4.3 — payload for [`Mutation::SetVideoSpeed`].
///
/// Per-field Reverse: `speed` is a plain `f32` inside `LayerKind::Video`.
/// Whole-`LayerKind` Reverse (rule 1) is overkill because no variant
/// replacement is involved; per-field is consistent with `SetLayerOpacity`.
/// The `apply` impl `debug_assert!`s that the layer is in fact a `Video` —
/// stale Reverse panics in test / debug builds rather than silently
/// corrupting the project.
#[derive(Debug, Clone)]
pub struct SetVideoSpeed {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// New playback rate multiplier.
    pub new: f32,
    /// Pre-mutation playback rate.
    pub old: f32,
}

impl ReverseStorage for SetVideoSpeed {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetVideoSpeed: layer_idx out of range");
        match &mut layer.kind {
            crate::project::schema::LayerKind::Video { speed, .. } => {
                debug_assert!(
                    (*speed - self.old).abs() < 1e-5,
                    "SetVideoSpeed stale Reverse: layer {} speed={}, expected old={}",
                    self.layer_idx,
                    *speed,
                    self.old
                );
                *speed = self.new;
            }
            _ => panic!(
                "SetVideoSpeed: layer {} is not a Video layer",
                self.layer_idx
            ),
        }
        SetVideoSpeed {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// P1.4.2 — payload for [`Mutation::SetVideoLoopMode`].
///
/// Per-field Reverse: `loop_mode` is a plain Copy enum inside
/// `LayerKind::Video`. Same rationale as [`SetVideoSpeed`] — per-field
/// is sufficient because no variant replacement is involved.
///
/// Replaced the P0.4.3 `SetVideoLoopSeamless` boolean mutation; old
/// saves loading via `migrate::migrate` have their `loop_seamless`
/// field normalised to the matching `LoopMode` variant before serde
/// ever sees the new shape, so no compatibility shim is needed at
/// this layer.
#[derive(Debug, Clone)]
pub struct SetVideoLoopMode {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// New loop mode.
    pub new: crate::project::schema::LoopMode,
    /// Pre-mutation loop mode.
    pub old: crate::project::schema::LoopMode,
}

impl ReverseStorage for SetVideoLoopMode {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetVideoLoopMode: layer_idx out of range");
        match &mut layer.kind {
            crate::project::schema::LayerKind::Video { loop_mode, .. } => {
                debug_assert_eq!(
                    *loop_mode, self.old,
                    "SetVideoLoopMode stale Reverse: layer {} loop_mode={:?}, expected old={:?}",
                    self.layer_idx, *loop_mode, self.old
                );
                *loop_mode = self.new;
            }
            _ => panic!(
                "SetVideoLoopMode: layer {} is not a Video layer",
                self.layer_idx
            ),
        }
        SetVideoLoopMode {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// P1.4.1 — payload for [`Mutation::SetVideoClipRange`].
///
/// In/out points are written as a pair (atomic undo restores both at
/// once). The Reverse stores `(old_in, old_out)`; the `apply` impl
/// `debug_assert!`s both old values match before writing the new pair.
/// Validation (clip_in < clip_out) is enforced at the builder site so
/// `apply` always receives a well-formed range.
#[derive(Debug, Clone, Copy)]
pub struct SetVideoClipRange {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// New in-point (seconds, ≥ 0).
    pub new_in: f32,
    /// New out-point (seconds, > new_in; `f32::INFINITY` means "end").
    pub new_out: f32,
    /// Pre-mutation in-point.
    pub old_in: f32,
    /// Pre-mutation out-point.
    pub old_out: f32,
}

/// P1.2.4 — payload for [`Mutation::SetLayerFocal`].
///
/// Applies to `LayerKind::Image` and `LayerKind::Video` — both
/// carry an identical `focal: [f32; 2]` field. Per-field Reverse
/// is sufficient because we're not crossing variant identity (the
/// layer's `kind` discriminant is unchanged by this mutation).
#[derive(Debug, Clone, Copy)]
pub struct SetLayerFocal {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// New focal point (normalised [0, 1]²).
    pub new: [f32; 2],
    /// Pre-mutation focal point.
    pub old: [f32; 2],
}

impl ReverseStorage for SetLayerFocal {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetLayerFocal: layer_idx out of range");
        let focal_ref: &mut [f32; 2] = match &mut layer.kind {
            crate::project::schema::LayerKind::Image { focal, .. }
            | crate::project::schema::LayerKind::Video { focal, .. } => focal,
            _ => panic!(
                "SetLayerFocal: layer {} is not an Image or Video layer",
                self.layer_idx
            ),
        };
        debug_assert!(
            (focal_ref[0] - self.old[0]).abs() < 1e-4 && (focal_ref[1] - self.old[1]).abs() < 1e-4,
            "SetLayerFocal stale Reverse: focal={:?} expected={:?}",
            focal_ref,
            self.old
        );
        *focal_ref = self.new;
        SetLayerFocal {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

/// P1.4.4 — payload for [`Mutation::SetVideoBpmLock`]. Per-field
/// Reverse on a bool; trivial round-trip.
#[derive(Debug, Clone, Copy)]
pub struct SetVideoBpmLock {
    /// Index into `Project.layers`.
    pub layer_idx: usize,
    /// New BPM-lock state.
    pub new: bool,
    /// Pre-mutation state.
    pub old: bool,
}

impl ReverseStorage for SetVideoBpmLock {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetVideoBpmLock: layer_idx out of range");
        match &mut layer.kind {
            crate::project::schema::LayerKind::Video { bpm_lock, .. } => {
                debug_assert_eq!(
                    *bpm_lock, self.old,
                    "SetVideoBpmLock stale Reverse: bpm_lock={} expected old={}",
                    *bpm_lock, self.old
                );
                *bpm_lock = self.new;
            }
            _ => panic!(
                "SetVideoBpmLock: layer {} is not a Video layer",
                self.layer_idx
            ),
        }
        SetVideoBpmLock {
            layer_idx: self.layer_idx,
            new: self.old,
            old: self.new,
        }
    }
}

impl ReverseStorage for SetVideoClipRange {
    fn apply(self, project: &mut Project) -> Self {
        let layer = project
            .layers
            .get_mut(self.layer_idx)
            .expect("SetVideoClipRange: layer_idx out of range");
        match &mut layer.kind {
            crate::project::schema::LayerKind::Video {
                clip_in, clip_out, ..
            } => {
                debug_assert!(
                    (*clip_in - self.old_in).abs() < 1e-4,
                    "SetVideoClipRange stale Reverse: clip_in={} expected={}",
                    *clip_in,
                    self.old_in
                );
                debug_assert!(
                    (*clip_out - self.old_out).abs() < 1e-4
                        || (clip_out.is_infinite() && self.old_out.is_infinite()),
                    "SetVideoClipRange stale Reverse: clip_out={} expected={}",
                    *clip_out,
                    self.old_out
                );
                *clip_in = self.new_in;
                *clip_out = self.new_out;
            }
            _ => panic!(
                "SetVideoClipRange: layer {} is not a Video layer",
                self.layer_idx
            ),
        }
        SetVideoClipRange {
            layer_idx: self.layer_idx,
            new_in: self.old_in,
            new_out: self.old_out,
            old_in: self.new_in,
            old_out: self.new_out,
        }
    }
}

// ---------------------------------------------------------------------------
// P5.3.3–P5.3.5 — Fixture-group mutations (behind `feature = "lighting"`)
// ---------------------------------------------------------------------------

/// P5.3.5 — replace all mutable fields of a `FixtureGroup` identified by `id`.
///
/// Whole-param Reverse: old `FixtureGroupParams` is stored in `old`.
#[cfg(feature = "lighting")]
#[derive(Debug, Clone)]
pub struct SetFixtureGroupParams {
    /// The ID of the group to mutate.
    pub id: crate::lighting::fixture::FixtureGroupId,
    /// New params to apply.
    pub new: crate::lighting::fixture::FixtureGroupParams,
    /// Old params captured before the mutation (for Reverse).
    pub old: crate::lighting::fixture::FixtureGroupParams,
}

#[cfg(feature = "lighting")]
impl SetFixtureGroupParams {
    /// Capture `old` from the current group state and set `new` to `params`.
    pub fn new(
        group: &crate::lighting::fixture::FixtureGroup,
        new: crate::lighting::fixture::FixtureGroupParams,
    ) -> Self {
        Self {
            id: group.id,
            new,
            old: crate::lighting::fixture::FixtureGroupParams::from_group(group),
        }
    }
}

#[cfg(feature = "lighting")]
impl ReverseStorage for SetFixtureGroupParams {
    fn apply(self, project: &mut Project) -> Self {
        let group = project
            .fixture_groups
            .iter_mut()
            .find(|g| g.id == self.id)
            .unwrap_or_else(|| {
                panic!(
                    "SetFixtureGroupParams: fixture group {:?} not found",
                    self.id
                )
            });
        debug_assert!(
            group.label == self.old.label,
            "SetFixtureGroupParams stale Reverse: label={:?} expected={:?}",
            group.label,
            self.old.label,
        );
        let before = crate::lighting::fixture::FixtureGroupParams::from_group(group);
        self.new.apply_to(group);
        SetFixtureGroupParams {
            id: self.id,
            new: before,
            old: self.new,
        }
    }
}

// ---------------------------------------------------------------------------
// P5.7.2–P5.7.4 — Fixture-chase mutations (behind `feature = "lighting"`)
// ---------------------------------------------------------------------------

/// P5.7.4 — replace all mutable fields of a `FixtureChase` identified by `id`.
#[cfg(feature = "lighting")]
#[derive(Debug, Clone)]
pub struct SetFixtureChaseParams {
    /// The ID of the chase to mutate.
    pub id: crate::lighting::chase::FixtureChaseid,
    /// New params to apply.
    pub new: crate::lighting::chase::FixtureChaseParams,
    /// Old params captured before the mutation (for Reverse).
    pub old: crate::lighting::chase::FixtureChaseParams,
}

#[cfg(feature = "lighting")]
impl ReverseStorage for SetFixtureChaseParams {
    fn apply(self, project: &mut Project) -> Self {
        let chase = project
            .fixture_chases
            .iter_mut()
            .find(|c| c.id == self.id)
            .unwrap_or_else(|| panic!("SetFixtureChaseParams: chase {:?} not found", self.id));
        debug_assert!(
            chase.label == self.old.label,
            "SetFixtureChaseParams stale Reverse: label={:?} expected={:?}",
            chase.label,
            self.old.label,
        );
        let before = crate::lighting::chase::FixtureChaseParams::from_chase(chase);
        self.new.apply_to(chase);
        SetFixtureChaseParams {
            id: self.id,
            new: before,
            old: self.new,
        }
    }
}

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
///
/// ## Asymmetric variants
///
/// `AddLayer`, `RemoveLayer`, `AddLayerMaskVertex`, and
/// `RemoveLayerMaskVertex` are intentionally **not** wrapped in
/// `ReverseStorage` structs. Their Reverse changes the variant
/// itself: `AddLayer`'s Reverse is `RemoveLayer` and vice versa.
/// The `ReverseStorage` trait's `fn apply(self, …) -> Self`
/// contract requires the Reverse to be the same type as `Self`,
/// which breaks for these pairs. Migrating them would require
/// changing the trait signature to `-> Mutation`, which defeats the
/// per-variant compile-time guarantee. They remain as inline match
/// arms in `Mutation::apply` with an explanatory comment.
#[derive(Debug, Clone)]
#[non_exhaustive]
#[allow(dead_code)] // T-003-T1.18+ wires call sites; foundation lives here from T1.14.
pub enum Mutation {
    /// Replace `Project.gamma`. Delegates to [`SetGamma`] which
    /// implements [`ReverseStorage`]; Reverse is the same variant
    /// with `new` and `old` swapped.
    SetGamma(SetGamma),
    /// Replace `Project.brightness`. Delegates to [`SetBrightness`].
    SetBrightness(SetBrightness),
    /// Replace `Project.contrast`. Delegates to [`SetContrast`].
    SetContrast(SetContrast),
    /// Replace `Project.crossfade_duration_s`. Delegates to [`SetCrossfadeDurationS`].
    SetCrossfadeDurationS(SetCrossfadeDurationS),
    /// 003-T3.28 — replace `Project.gamma_override`. Delegates to [`SetProjectGammaOverride`].
    SetProjectGammaOverride(SetProjectGammaOverride),
    /// 003-T3.28 — replace `Project.brightness_override`. Delegates to [`SetProjectBrightnessOverride`].
    SetProjectBrightnessOverride(SetProjectBrightnessOverride),
    /// 003-T3.28 — replace `Project.contrast_override`. See
    /// `SetProjectGammaOverride`.
    SetProjectContrastOverride(SetProjectContrastOverride),
    /// P0.7.3 — replace `Project.edge_blend`. Delegates to [`SetEdgeBlend`].
    SetEdgeBlend(SetEdgeBlend),
    /// Replace `Project.output_windowed`. Delegates to [`SetOutputWindowed`].
    SetOutputWindowed(SetOutputWindowed),
    /// Replace `WarpMesh.mask_feather`. Delegates to [`SetLayerMaskFeather`].
    SetLayerMaskFeather(SetLayerMaskFeather),
    /// P3.2.3 — replace `WarpMesh.zone_role`. Delegates to [`SetMaskZoneRole`].
    SetMaskZoneRole(SetMaskZoneRole),
    /// Replace `WarpMesh` dimensions. Delegates to [`SetLayerWarpDimensions`].
    SetLayerWarpDimensions(SetLayerWarpDimensions),
    /// Replace `LayerConfig.opacity`. Delegates to [`SetLayerOpacity`].
    SetLayerOpacity(SetLayerOpacity),
    /// Replace `LayerConfig.enabled`. Delegates to [`SetLayerEnabled`].
    SetLayerEnabled(SetLayerEnabled),
    /// V31.6.1 — replace `LayerConfig.muted`. Delegates to [`SetLayerMuted`].
    SetLayerMuted(SetLayerMuted),
    /// V31.6.1 — replace `Project.solo`. Delegates to [`SetLayerSolo`].
    SetLayerSolo(SetLayerSolo),
    /// V31.7.2 — replace `Project.quantize_bars`. Delegates to [`SetQuantizeBars`].
    SetQuantizeBars(SetQuantizeBars),
    /// Replace `LayerConfig.blend_mode`. Delegates to [`SetLayerBlendMode`].
    SetLayerBlendMode(SetLayerBlendMode),
    /// Replace a layer's effect chain wholesale. Delegates to [`SetLayerEffects`].
    SetLayerEffects(SetLayerEffects),
    /// 004-T1.13 — Replace a layer's effect chain AND mask polygon in a single
    /// undo step (smart-fill on add). Delegates to [`SetLayerEffectsAndMask`].
    SetLayerEffectsAndMask(SetLayerEffectsAndMask),

    /// P0.4.3 — replace `LayerKind::Video { speed, .. }` for a layer.
    /// Delegates to [`SetVideoSpeed`].
    SetVideoSpeed(SetVideoSpeed),
    /// P1.4.2 — replace `LayerKind::Video { loop_mode, .. }` for a layer.
    /// Delegates to [`SetVideoLoopMode`]. Replaces the P0.4.3
    /// `SetVideoLoopSeamless` boolean mutation.
    SetVideoLoopMode(SetVideoLoopMode),
    /// P1.4.1 — replace `LayerKind::Video { clip_in, clip_out, .. }` for
    /// a layer (atomic pair). Delegates to [`SetVideoClipRange`].
    SetVideoClipRange(SetVideoClipRange),
    /// P1.4.4 — replace `LayerKind::Video { bpm_lock, .. }` for a layer.
    /// Delegates to [`SetVideoBpmLock`].
    SetVideoBpmLock(SetVideoBpmLock),
    /// P1.2.4 — replace `LayerKind::{Image,Video} { focal, .. }` for a
    /// layer. Delegates to [`SetLayerFocal`].
    SetLayerFocal(SetLayerFocal),

    /// Insert `layer` at `position`. Reverse is `RemoveLayer { idx: position }`.
    ///
    /// **Asymmetric variant** — intentionally kept as an inline match arm
    /// (not wrapped in a `ReverseStorage` struct) because the Reverse changes
    /// variant identity: `AddLayer`'s reverse is `RemoveLayer`. See enum-level
    /// doc comment for the full rationale.
    AddLayer {
        /// The layer to insert.
        layer: LayerConfig,
        /// Insertion index (0..=project.layers.len()).
        position: usize,
    },
    /// Remove the layer at `idx`. Reverse is `AddLayer { layer, position: idx }`.
    ///
    /// **Asymmetric variant** — kept as inline match arm for the same reason as
    /// `AddLayer`. See enum-level doc comment.
    RemoveLayer {
        /// Index into `Project.layers`.
        idx: usize,
    },
    /// Swap the layers at `i` and `j`. Delegates to [`SwapLayers`].
    SwapLayers(SwapLayers),

    /// 003-T2.24 — repoint a layer's asset path. Delegates to [`RelinkAssetPath`].
    RelinkAssetPath(RelinkAssetPath),

    /// P0.5.1 — wholesale replace `LayerConfig.kind`. Used to switch
    /// FX presets, mutate an FxLayer's `params` map, or change a
    /// layer's source type. Whole-enum Reverse. Delegates to
    /// [`SetLayerKind`].
    SetLayerKind(SetLayerKind),

    /// P1.2.1 — replace a layer's `treatment` (whole-`Option`
    /// snapshot Reverse). Delegates to [`SetLayerTreatment`].
    /// Handles preset switches, `overlay_path` edits, and
    /// `collage_paths` edits.
    SetLayerTreatment(SetLayerTreatment),

    /// P1.2.1 — replace a layer's `treatment.params` map (whole-
    /// `HashMap` snapshot Reverse). Delegates to
    /// [`SetLayerTreatmentParams`].
    SetLayerTreatmentParams(SetLayerTreatmentParams),

    /// P2.5.6 — replace a `FxLayer`'s `params` map (whole-`HashMap`
    /// snapshot Reverse). Delegates to [`SetFxLayerParams`].
    /// Budget violations refuse without mutating the project (see
    /// `SetFxLayerParams::apply`).
    SetFxLayerParams(SetFxLayerParams),

    /// P0.8.2 — wholesale replace `Project.output_target.rgb_matrix`.
    /// Per-cell edits in the W8.3 calibration UI emit one of these
    /// per change. Delegates to [`SetOutputRgbMatrix`].
    SetOutputRgbMatrix(SetOutputRgbMatrix),

    /// PCleanup.7.3 — replace `OutputTarget.gamma_override` for output
    /// index. `None` clears the per-projector trim, falling back to the
    /// project-level cascade. Delegates to [`SetOutputGammaOverride`].
    SetOutputGammaOverride(SetOutputGammaOverride),
    /// PCleanup.7.3 — replace `OutputTarget.brightness_override`.
    SetOutputBrightnessOverride(SetOutputBrightnessOverride),
    /// PCleanup.7.3 — replace `OutputTarget.contrast_override`.
    SetOutputContrastOverride(SetOutputContrastOverride),

    /// Replace the modulator at `(layer_idx, effect_idx, field)`. Delegates to [`SetModulator`].
    SetModulator(SetModulator),

    /// Replace the entire `WarpMesh`. Delegates to [`ResetLayerWarpMesh`].
    ResetLayerWarpMesh(ResetLayerWarpMesh),
    /// P7.3.1 — Replace the entire `BezierMesh` (or set to `None`). Delegates to [`ResetLayerBezierMesh`].
    ResetLayerBezierMesh(ResetLayerBezierMesh),
    /// P7.3.3 — Move a single Bezier anchor and propagate handles rigidly. Delegates to [`MoveBezierAnchor`].
    MoveBezierAnchor(MoveBezierAnchor),
    /// P7.3.3 — Set (or clear) a single Bezier tangent handle. Delegates to [`SetBezierHandle`].
    SetBezierHandle(SetBezierHandle),
    /// P7.5.1/P7.6.1 — Replace the entire `MaskGraph` on a layer (or set to `None`). Delegates to [`SetLayerMaskGraph`].
    SetLayerMaskGraph(SetLayerMaskGraph),
    /// Replace `WarpMesh.mask_polygon`. Delegates to [`SetLayerMaskPolygon`].
    SetLayerMaskPolygon(SetLayerMaskPolygon),

    /// Insert a new vertex into `WarpMesh.mask_polygon` at `position`.
    ///
    /// **Asymmetric variant** — kept as inline match arm because the Reverse
    /// is `RemoveLayerMaskVertex`. See enum-level doc comment.
    AddLayerMaskVertex {
        /// Index into `Project.layers`; the layer's `warp` is the target.
        layer_idx: usize,
        /// Insertion index (0..=polygon.len()).
        position: usize,
        /// The vertex coordinates to insert (normalized output-space).
        point: [f32; 2],
    },
    /// Remove the vertex at `idx` from `WarpMesh.mask_polygon`.
    ///
    /// **Asymmetric variant** — kept as inline match arm because the Reverse
    /// is `AddLayerMaskVertex`. See enum-level doc comment.
    RemoveLayerMaskVertex {
        /// Index into `Project.layers`; the layer's `warp` is the target.
        layer_idx: usize,
        /// Index of the vertex to remove.
        idx: usize,
    },
    /// Replace `WarpMesh.mask_polygon[idx]`. Delegates to [`SetLayerMaskVertex`].
    SetLayerMaskVertex(SetLayerMaskVertex),

    /// Replace `WarpMesh.grid[r][c]`. Delegates to [`SetLayerWarpCorner`].
    SetLayerWarpCorner(SetLayerWarpCorner),

    /// Replace `Project.scenes` wholesale. Delegates to [`SetProjectScenes`].
    SetProjectScenes(SetProjectScenes),

    // -------------------------------------------------------------------
    // P6.2.2 — Per-cue timing mutations
    // -------------------------------------------------------------------
    /// P6.2.2 — Rename a single cue. Delegates to [`SetCueName`].
    SetCueName(SetCueName),
    /// P6.2.2 — Replace all timing + binding fields on a single cue.
    /// Delegates to [`SetCueTiming`].
    SetCueTiming(SetCueTiming),
    /// P6.2.2 — Replace `Project.cues` wholesale. Delegates to [`SetProjectCues`].
    SetProjectCues(SetProjectCues),

    /// Replace `Project.output_monitor_index`. Delegates to [`SetOutputMonitorIndex`].
    SetOutputMonitorIndex(SetOutputMonitorIndex),

    /// Replace the entire project from a serde_json snapshot. Delegates to [`ApplyProjectSnapshot`].
    ApplyProjectSnapshot(ApplyProjectSnapshot),

    // -------------------------------------------------------------------
    // P5 — Fixture-group mutations (behind `feature = "lighting"`)
    // -------------------------------------------------------------------
    /// P5.3.3 — insert a `FixtureGroup` into `project.fixture_groups`.
    ///
    /// **Asymmetric variant** — kept as inline match arm because its Reverse
    /// is `RemoveFixtureGroup`. Mirrors the `AddLayer`/`RemoveLayer` pattern.
    #[cfg(feature = "lighting")]
    AddFixtureGroup {
        /// The group to insert.
        group: crate::lighting::fixture::FixtureGroup,
    },
    /// P5.3.4 — remove a `FixtureGroup` from `project.fixture_groups`.
    ///
    /// **Asymmetric variant** — kept as inline match arm; Reverse is `AddFixtureGroup`.
    #[cfg(feature = "lighting")]
    RemoveFixtureGroup {
        /// ID of the group to remove.
        id: crate::lighting::fixture::FixtureGroupId,
    },
    /// P5.3.5 — replace all mutable fields of a `FixtureGroup`. Delegates
    /// to [`SetFixtureGroupParams`] which implements [`ReverseStorage`].
    #[cfg(feature = "lighting")]
    SetFixtureGroupParams(SetFixtureGroupParams),

    // -------------------------------------------------------------------
    // P5 — Fixture-chase mutations (behind `feature = "lighting"`)
    // -------------------------------------------------------------------
    /// P5.7.2 — insert a `FixtureChase` into `project.fixture_chases`.
    ///
    /// **Asymmetric variant** — Reverse is `RemoveFixtureChase`.
    #[cfg(feature = "lighting")]
    AddFixtureChase {
        /// The chase to insert.
        chase: crate::lighting::chase::FixtureChase,
    },
    /// P5.7.3 — remove a `FixtureChase` from `project.fixture_chases`.
    ///
    /// **Asymmetric variant** — Reverse is `AddFixtureChase`.
    #[cfg(feature = "lighting")]
    RemoveFixtureChase {
        /// ID of the chase to remove.
        id: crate::lighting::chase::FixtureChaseid,
    },
    /// P5.7.4 — replace all mutable fields of a `FixtureChase`. Delegates
    /// to [`SetFixtureChaseParams`].
    #[cfg(feature = "lighting")]
    SetFixtureChaseParams(SetFixtureChaseParams),
}

#[allow(dead_code)] // T-003-T1.18+ wires call sites.
impl Mutation {
    /// Apply the mutation to `project` and return its Reverse.
    /// In test builds, panics if the carried `old` value does not
    /// match the project's current state — catches contributor
    /// errors that would otherwise corrupt undo history.
    pub fn apply(self, project: &mut Project) -> Mutation {
        match self {
            // --- Symmetric variants: delegate to ReverseStorage::apply ---
            Mutation::SetGamma(s) => Mutation::SetGamma(s.apply(project)),
            Mutation::SetBrightness(s) => Mutation::SetBrightness(s.apply(project)),
            Mutation::SetContrast(s) => Mutation::SetContrast(s.apply(project)),
            Mutation::SetCrossfadeDurationS(s) => Mutation::SetCrossfadeDurationS(s.apply(project)),
            Mutation::SetProjectGammaOverride(s) => {
                Mutation::SetProjectGammaOverride(s.apply(project))
            }
            Mutation::SetProjectBrightnessOverride(s) => {
                Mutation::SetProjectBrightnessOverride(s.apply(project))
            }
            Mutation::SetProjectContrastOverride(s) => {
                Mutation::SetProjectContrastOverride(s.apply(project))
            }
            Mutation::SetEdgeBlend(s) => Mutation::SetEdgeBlend(s.apply(project)),
            Mutation::SetOutputWindowed(s) => Mutation::SetOutputWindowed(s.apply(project)),
            Mutation::SetLayerMaskFeather(s) => Mutation::SetLayerMaskFeather(s.apply(project)),
            Mutation::SetMaskZoneRole(s) => Mutation::SetMaskZoneRole(s.apply(project)),
            Mutation::SetLayerWarpDimensions(s) => {
                Mutation::SetLayerWarpDimensions(s.apply(project))
            }
            Mutation::SetLayerOpacity(s) => Mutation::SetLayerOpacity(s.apply(project)),
            Mutation::SetLayerEnabled(s) => Mutation::SetLayerEnabled(s.apply(project)),
            Mutation::SetLayerMuted(s) => Mutation::SetLayerMuted(s.apply(project)),
            Mutation::SetLayerSolo(s) => Mutation::SetLayerSolo(s.apply(project)),
            Mutation::SetQuantizeBars(s) => Mutation::SetQuantizeBars(s.apply(project)),
            Mutation::SetLayerBlendMode(s) => Mutation::SetLayerBlendMode(s.apply(project)),
            Mutation::SetLayerEffects(s) => Mutation::SetLayerEffects(s.apply(project)),
            Mutation::SetLayerEffectsAndMask(s) => {
                Mutation::SetLayerEffectsAndMask(s.apply(project))
            }
            Mutation::SetVideoSpeed(s) => Mutation::SetVideoSpeed(s.apply(project)),
            Mutation::SetVideoLoopMode(s) => Mutation::SetVideoLoopMode(s.apply(project)),
            Mutation::SetVideoClipRange(s) => Mutation::SetVideoClipRange(s.apply(project)),
            Mutation::SetVideoBpmLock(s) => Mutation::SetVideoBpmLock(s.apply(project)),
            Mutation::SetLayerFocal(s) => Mutation::SetLayerFocal(s.apply(project)),
            Mutation::SwapLayers(s) => Mutation::SwapLayers(s.apply(project)),
            Mutation::RelinkAssetPath(s) => Mutation::RelinkAssetPath(s.apply(project)),
            Mutation::SetLayerKind(s) => Mutation::SetLayerKind(s.apply(project)),
            Mutation::SetLayerTreatment(s) => Mutation::SetLayerTreatment(s.apply(project)),
            Mutation::SetLayerTreatmentParams(s) => {
                Mutation::SetLayerTreatmentParams(s.apply(project))
            }
            Mutation::SetFxLayerParams(s) => Mutation::SetFxLayerParams(s.apply(project)),
            Mutation::SetOutputRgbMatrix(s) => Mutation::SetOutputRgbMatrix(s.apply(project)),
            // PCleanup.7.3 — per-output tone-override mutations.
            Mutation::SetOutputGammaOverride(s) => {
                Mutation::SetOutputGammaOverride(s.apply(project))
            }
            Mutation::SetOutputBrightnessOverride(s) => {
                Mutation::SetOutputBrightnessOverride(s.apply(project))
            }
            Mutation::SetOutputContrastOverride(s) => {
                Mutation::SetOutputContrastOverride(s.apply(project))
            }
            Mutation::SetModulator(s) => Mutation::SetModulator(s.apply(project)),
            Mutation::ResetLayerWarpMesh(s) => Mutation::ResetLayerWarpMesh(s.apply(project)),
            Mutation::ResetLayerBezierMesh(s) => Mutation::ResetLayerBezierMesh(s.apply(project)),
            Mutation::MoveBezierAnchor(s) => Mutation::MoveBezierAnchor(s.apply(project)),
            Mutation::SetBezierHandle(s) => Mutation::SetBezierHandle(s.apply(project)),
            Mutation::SetLayerMaskGraph(s) => Mutation::SetLayerMaskGraph(s.apply(project)),
            Mutation::SetLayerMaskPolygon(s) => Mutation::SetLayerMaskPolygon(s.apply(project)),
            Mutation::SetLayerMaskVertex(s) => Mutation::SetLayerMaskVertex(s.apply(project)),
            Mutation::SetLayerWarpCorner(s) => Mutation::SetLayerWarpCorner(s.apply(project)),
            Mutation::SetProjectScenes(s) => Mutation::SetProjectScenes(s.apply(project)),
            Mutation::SetCueName(s) => Mutation::SetCueName(s.apply(project)),
            Mutation::SetCueTiming(s) => Mutation::SetCueTiming(s.apply(project)),
            Mutation::SetProjectCues(s) => Mutation::SetProjectCues(s.apply(project)),
            Mutation::SetOutputMonitorIndex(s) => Mutation::SetOutputMonitorIndex(s.apply(project)),
            Mutation::ApplyProjectSnapshot(s) => Mutation::ApplyProjectSnapshot(s.apply(project)),

            // --- P5 symmetric variants (lighting feature) ---
            #[cfg(feature = "lighting")]
            Mutation::SetFixtureGroupParams(s) => Mutation::SetFixtureGroupParams(s.apply(project)),
            #[cfg(feature = "lighting")]
            Mutation::SetFixtureChaseParams(s) => Mutation::SetFixtureChaseParams(s.apply(project)),

            // --- Asymmetric variants: kept as inline arms ---
            //
            // `AddLayer`, `RemoveLayer`, `AddLayerMaskVertex`, and
            // `RemoveLayerMaskVertex` are intentionally NOT wrapped in
            // `ReverseStorage` structs. Their Reverse crosses variant
            // boundaries: `AddLayer`'s reverse is `RemoveLayer` and vice
            // versa, making `fn apply(self, …) -> Self` impossible. Changing
            // the trait to return `Mutation` would defeat the per-variant
            // compile-time guarantee. These four stay here as documented
            // exceptions; all other variants use the trait.
            Mutation::AddLayer { layer, position } => {
                debug_assert!(
                    position <= project.layers.len(),
                    "AddLayer position out of range: position={}, len={}",
                    position,
                    project.layers.len()
                );
                project.layers.insert(position, layer);
                Mutation::RemoveLayer { idx: position }
            }
            Mutation::RemoveLayer { idx } => {
                debug_assert!(
                    idx < project.layers.len(),
                    "RemoveLayer idx out of range: idx={}, len={}",
                    idx,
                    project.layers.len()
                );
                // TODO(V31.6.2): if project.solo == Some(idx), clear it on remove.
                // Also shift solo index down when removed layer precedes the soloed one.
                let layer = project.layers.remove(idx);
                Mutation::AddLayer {
                    layer,
                    position: idx,
                }
            }
            Mutation::AddLayerMaskVertex {
                layer_idx,
                position,
                point,
            } => {
                let warp = &mut project
                    .layers
                    .get_mut(layer_idx)
                    .expect("AddLayerMaskVertex: layer_idx out of range")
                    .warp;
                debug_assert!(
                    position <= warp.mask_polygon.len(),
                    "AddMaskVertex position out of range: position={}, len={}",
                    position,
                    warp.mask_polygon.len()
                );
                warp.mask_polygon.insert(position, point);
                Mutation::RemoveLayerMaskVertex {
                    layer_idx,
                    idx: position,
                }
            }
            Mutation::RemoveLayerMaskVertex { layer_idx, idx } => {
                let warp = &mut project
                    .layers
                    .get_mut(layer_idx)
                    .expect("RemoveLayerMaskVertex: layer_idx out of range")
                    .warp;
                debug_assert!(
                    idx < warp.mask_polygon.len(),
                    "RemoveLayerMaskVertex idx out of range: idx={}, len={}",
                    idx,
                    warp.mask_polygon.len()
                );
                let point = warp.mask_polygon.remove(idx);
                Mutation::AddLayerMaskVertex {
                    layer_idx,
                    position: idx,
                    point,
                }
            }

            // --- P5.3.3 — AddFixtureGroup (asymmetric) ---
            #[cfg(feature = "lighting")]
            Mutation::AddFixtureGroup { group } => {
                project.fixture_groups.push(group);
                Mutation::RemoveFixtureGroup {
                    id: project.fixture_groups.last().unwrap().id,
                }
            }

            // --- P5.3.4 — RemoveFixtureGroup (asymmetric) ---
            #[cfg(feature = "lighting")]
            Mutation::RemoveFixtureGroup { id } => {
                let pos = project
                    .fixture_groups
                    .iter()
                    .position(|g| g.id == id)
                    .unwrap_or_else(|| panic!("RemoveFixtureGroup: id {:?} not found", id));
                let group = project.fixture_groups.remove(pos);
                Mutation::AddFixtureGroup { group }
            }

            // --- P5.7.2 — AddFixtureChase (asymmetric) ---
            #[cfg(feature = "lighting")]
            Mutation::AddFixtureChase { chase } => {
                project.fixture_chases.push(chase);
                Mutation::RemoveFixtureChase {
                    id: project.fixture_chases.last().unwrap().id,
                }
            }

            // --- P5.7.3 — RemoveFixtureChase (asymmetric) ---
            #[cfg(feature = "lighting")]
            Mutation::RemoveFixtureChase { id } => {
                let pos = project
                    .fixture_chases
                    .iter()
                    .position(|c| c.id == id)
                    .unwrap_or_else(|| panic!("RemoveFixtureChase: id {:?} not found", id));
                let chase = project.fixture_chases.remove(pos);
                Mutation::AddFixtureChase { chase }
            }
        }
    }

    /// Whether this mutation should be excluded from the
    /// user-facing undo stack. Today only crossfade-tick
    /// `ApplyProjectSnapshot` variants set this; gamma /
    /// brightness / contrast slider edits are all undoable.
    pub fn is_non_undoable(&self) -> bool {
        match self {
            Mutation::SetGamma(_)
            | Mutation::SetBrightness(_)
            | Mutation::SetContrast(_)
            | Mutation::SetCrossfadeDurationS(_)
            | Mutation::SetOutputWindowed(_)
            | Mutation::SetLayerMaskFeather(_)
            | Mutation::SetMaskZoneRole(_)
            | Mutation::SetLayerWarpDimensions(_)
            | Mutation::SetLayerOpacity(_)
            | Mutation::SetLayerEnabled(_)
            | Mutation::SetLayerMuted(_)
            | Mutation::SetLayerSolo(_)
            | Mutation::SetQuantizeBars(_)
            | Mutation::SetLayerBlendMode(_)
            | Mutation::SetLayerEffects(_)
            | Mutation::SetLayerEffectsAndMask(_)
            | Mutation::SetModulator(_)
            | Mutation::AddLayer { .. }
            | Mutation::RemoveLayer { .. }
            | Mutation::SwapLayers(_)
            | Mutation::AddLayerMaskVertex { .. }
            | Mutation::RemoveLayerMaskVertex { .. }
            | Mutation::SetLayerMaskVertex(_)
            | Mutation::ResetLayerWarpMesh(_)
            | Mutation::ResetLayerBezierMesh(_)
            | Mutation::MoveBezierAnchor(_)
            | Mutation::SetBezierHandle(_)
            | Mutation::SetLayerMaskGraph(_)
            | Mutation::SetLayerMaskPolygon(_)
            | Mutation::SetLayerWarpCorner(_)
            | Mutation::SetProjectScenes(_)
            | Mutation::SetCueName(_)
            | Mutation::SetCueTiming(_)
            | Mutation::SetProjectCues(_)
            | Mutation::SetOutputMonitorIndex(_)
            | Mutation::SetProjectGammaOverride(_)
            | Mutation::SetProjectBrightnessOverride(_)
            | Mutation::SetProjectContrastOverride(_)
            | Mutation::SetEdgeBlend(_)
            | Mutation::RelinkAssetPath(_)
            | Mutation::SetLayerKind(_)
            | Mutation::SetLayerTreatment(_)
            | Mutation::SetLayerTreatmentParams(_)
            | Mutation::SetFxLayerParams(_)
            | Mutation::SetOutputRgbMatrix(_)
            // PCleanup.7.3 — per-output trim overrides; same undo
            // policy as the project-level gamma/brightness/contrast
            // overrides above.
            | Mutation::SetOutputGammaOverride(_)
            | Mutation::SetOutputBrightnessOverride(_)
            | Mutation::SetOutputContrastOverride(_)
            | Mutation::SetVideoSpeed(_)
            | Mutation::SetVideoLoopMode(_)
            | Mutation::SetVideoClipRange(_)
            | Mutation::SetVideoBpmLock(_)
            | Mutation::SetLayerFocal(_) => false,
            Mutation::ApplyProjectSnapshot(s) => s.non_undoable,
            // P5 lighting mutations are always undoable.
            #[cfg(feature = "lighting")]
            Mutation::AddFixtureGroup { .. }
            | Mutation::RemoveFixtureGroup { .. }
            | Mutation::SetFixtureGroupParams(_)
            | Mutation::AddFixtureChase { .. }
            | Mutation::RemoveFixtureChase { .. }
            | Mutation::SetFixtureChaseParams(_) => false,
        }
    }

    /// Whether the renderer's per-layer GPU state is invalidated by this
    /// mutation. Layer-topology mutations (AddLayer / RemoveLayer / SwapLayers)
    /// invalidate the `state.layers` Vec; field-edit mutations don't —
    /// they touch project fields the renderer reads each frame.
    ///
    /// `ApplyProjectSnapshot { non_undoable: false }` (user-triggered scene
    /// recall) can change layer topology (it's taken precisely on topology
    /// mismatch or zero-duration recall), so it returns `true`. The
    /// `non_undoable: true` crossfade-tick variant never changes topology —
    /// topology compatibility is verified at scheduling time — so it stays
    /// `false`.
    ///
    /// The app's undo / redo dispatch and the pending-mutation drain inspect
    /// this flag to decide whether to call `rebuild_layers_for_state` after
    /// the mutation lands.
    pub fn needs_layer_rebuild(&self) -> bool {
        match self {
            Mutation::AddLayer { .. }
            | Mutation::RemoveLayer { .. }
            | Mutation::SwapLayers(_)
            | Mutation::RelinkAssetPath(_)
            | Mutation::SetLayerKind(_) => true,
            Mutation::ApplyProjectSnapshot(s) => !s.non_undoable,
            _ => false,
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
        Mutation::SetGamma(SetGamma {
            new,
            old: self.gamma,
        })
    }

    /// Build a `SetBrightness` mutation.
    pub fn set_brightness_mutation(&self, new: f32) -> Mutation {
        Mutation::SetBrightness(SetBrightness {
            new,
            old: self.brightness,
        })
    }

    /// Build a `SetContrast` mutation.
    pub fn set_contrast_mutation(&self, new: f32) -> Mutation {
        Mutation::SetContrast(SetContrast {
            new,
            old: self.contrast,
        })
    }

    /// Build a `SetCrossfadeDurationS` mutation.
    pub fn set_crossfade_duration_s_mutation(&self, new: f32) -> Mutation {
        Mutation::SetCrossfadeDurationS(SetCrossfadeDurationS {
            new,
            old: self.crossfade_duration_s,
        })
    }

    /// 003-T3.28 — build a `SetProjectGammaOverride` mutation.
    pub fn set_project_gamma_override_mutation(&self, new: Option<f32>) -> Mutation {
        Mutation::SetProjectGammaOverride(SetProjectGammaOverride {
            new,
            old: self.gamma_override,
        })
    }

    /// 003-T3.28 — build a `SetProjectBrightnessOverride` mutation.
    pub fn set_project_brightness_override_mutation(&self, new: Option<f32>) -> Mutation {
        Mutation::SetProjectBrightnessOverride(SetProjectBrightnessOverride {
            new,
            old: self.brightness_override,
        })
    }

    /// 003-T3.28 — build a `SetProjectContrastOverride` mutation.
    pub fn set_project_contrast_override_mutation(&self, new: Option<f32>) -> Mutation {
        Mutation::SetProjectContrastOverride(SetProjectContrastOverride {
            new,
            old: self.contrast_override,
        })
    }

    /// P0.7.3 — build a `SetEdgeBlend` mutation. Captures the project's
    /// current `edge_blend` value as `old`. Pass `None` to disable blending;
    /// `Some(cfg)` to enable / update it.
    pub fn set_edge_blend_mutation(
        &self,
        new: Option<crate::project::schema::EdgeBlendConfig>,
    ) -> Mutation {
        Mutation::SetEdgeBlend(SetEdgeBlend {
            new,
            old: self.edge_blend,
        })
    }

    /// P1.2.1 — build a `SetLayerTreatment` mutation for the given
    /// layer index. Captures the layer's current `treatment` as `old`.
    /// Pass `None` to remove the treatment; `Some(t)` to set / replace
    /// it. Panics if `layer_idx` is out of range — callers must guard
    /// first (the UI dispatch site has `layer_idx < layers.len()` as
    /// an invariant).
    pub fn set_layer_treatment_mutation(
        &self,
        layer_idx: usize,
        new: Option<crate::project::schema::Treatment>,
    ) -> Mutation {
        let old = self.layers[layer_idx].treatment.clone();
        Mutation::SetLayerTreatment(SetLayerTreatment {
            layer_idx,
            new,
            old,
        })
    }

    /// P1.2.1 — build a `SetLayerTreatmentParams` mutation for the
    /// given layer index. Captures the layer's current
    /// `treatment.params` as `old`. **Panics** if the layer has no
    /// treatment — the UI dispatch site must only call this when the
    /// preset is active (param sliders render only when
    /// `treatment.is_some()`).
    pub fn set_layer_treatment_params_mutation(
        &self,
        layer_idx: usize,
        new: std::collections::HashMap<String, f32>,
    ) -> Mutation {
        let old = self.layers[layer_idx]
            .treatment
            .as_ref()
            .expect(
                "set_layer_treatment_params_mutation called on a \
                 layer with no treatment — UI dispatch site must guard",
            )
            .params
            .clone();
        Mutation::SetLayerTreatmentParams(SetLayerTreatmentParams {
            layer_idx,
            new,
            old,
        })
    }

    /// P2.5.6 — build a `SetFxLayerParams` mutation for the given layer
    /// index. Captures the layer's current `FxLayer.params` as `old`.
    /// **Panics** if the layer is not `LayerKind::FxLayer` — the UI
    /// dispatch site must only call this when the layer is an FxLayer
    /// (param sliders render only for FxLayer).
    pub fn set_fx_layer_params_mutation(
        &self,
        layer_idx: usize,
        new: std::collections::HashMap<String, f32>,
    ) -> Mutation {
        let old = match &self.layers[layer_idx].kind {
            crate::project::schema::LayerKind::FxLayer { params, .. } => params.clone(),
            _ => panic!(
                "set_fx_layer_params_mutation called on a layer that is not FxLayer \
                 — UI dispatch site must guard"
            ),
        };
        Mutation::SetFxLayerParams(SetFxLayerParams {
            layer_idx,
            new,
            old,
        })
    }

    /// P2.5.6 — pre-flight budget check. Returns `Some((key, value, max))`
    /// if `new_params` would exceed any descriptor's `max_particle_count`
    /// for the current preset. Returns `None` when the params are within
    /// budget. **Panics** if the layer is not `LayerKind::FxLayer`.
    ///
    /// The UI calls this before dispatching `SetFxLayerParams` to surface
    /// a warning toast without going through the mutation path.
    pub fn fx_layer_params_over_budget(
        &self,
        layer_idx: usize,
        new_params: &std::collections::HashMap<String, f32>,
    ) -> Option<(String, f32, u32)> {
        let preset_id = match &self.layers[layer_idx].kind {
            crate::project::schema::LayerKind::FxLayer { preset_id, .. } => preset_id.clone(),
            _ => panic!(
                "fx_layer_params_over_budget called on a layer that is not FxLayer \
                 — UI dispatch site must guard"
            ),
        };
        particle_budget_exceeded(&preset_id, new_params)
    }

    /// P0.8.1 — build a `SetOutputRgbMatrix` mutation for the given output
    /// index. Captures the current matrix as `old`. Panics if `output_idx`
    /// is out of range — callers must guard first.
    pub fn set_output_rgb_matrix_mutation(
        &self,
        output_idx: usize,
        new: [[f32; 3]; 3],
    ) -> Mutation {
        let old = self.output_targets[output_idx].rgb_matrix;
        Mutation::SetOutputRgbMatrix(SetOutputRgbMatrix {
            output_idx,
            new,
            old,
        })
    }

    /// PCleanup.7.3 — build a `SetOutputGammaOverride` mutation. Captures
    /// the current per-output gamma-override as `old`. Panics on
    /// out-of-range `output_idx` — callers must guard.
    pub fn set_output_gamma_override_mutation(
        &self,
        output_idx: usize,
        new: Option<f32>,
    ) -> Mutation {
        let old = self.output_targets[output_idx].gamma_override;
        Mutation::SetOutputGammaOverride(SetOutputGammaOverride {
            output_idx,
            new,
            old,
        })
    }

    /// PCleanup.7.3 — build a `SetOutputBrightnessOverride` mutation.
    pub fn set_output_brightness_override_mutation(
        &self,
        output_idx: usize,
        new: Option<f32>,
    ) -> Mutation {
        let old = self.output_targets[output_idx].brightness_override;
        Mutation::SetOutputBrightnessOverride(SetOutputBrightnessOverride {
            output_idx,
            new,
            old,
        })
    }

    /// PCleanup.7.3 — build a `SetOutputContrastOverride` mutation.
    pub fn set_output_contrast_override_mutation(
        &self,
        output_idx: usize,
        new: Option<f32>,
    ) -> Mutation {
        let old = self.output_targets[output_idx].contrast_override;
        Mutation::SetOutputContrastOverride(SetOutputContrastOverride {
            output_idx,
            new,
            old,
        })
    }

    /// Build a `SetOutputWindowed` mutation.
    pub fn set_output_windowed_mutation(&self, new: bool) -> Mutation {
        Mutation::SetOutputWindowed(SetOutputWindowed {
            new,
            old: self.output_windowed,
        })
    }

    /// Build a `SetOutputMonitorIndex` mutation (T-003-T1.39). Captures
    /// the project's current `output_target.fallback_index` as `old`. Used by
    /// `ProjectAudit` to emit an autofix for `MonitorOutOfRange`.
    pub fn set_output_monitor_index_mutation(&self, new: usize) -> Mutation {
        Mutation::SetOutputMonitorIndex(SetOutputMonitorIndex {
            new,
            old: self.primary_output_target().fallback_index,
        })
    }

    /// P3.2.3 — build a `SetMaskZoneRole` mutation for the given layer.
    /// Captures the layer's current `warp.zone_role` as `old`. Panics if
    /// `layer_idx` is out of range.
    pub fn set_mask_zone_role_mutation(
        &self,
        layer_idx: usize,
        new: Option<crate::project::schema::ZoneRole>,
    ) -> Mutation {
        let old = self.layers[layer_idx].warp.zone_role;
        Mutation::SetMaskZoneRole(SetMaskZoneRole {
            layer_idx,
            new,
            old,
        })
    }

    /// Build a `SetLayerMaskFeather` mutation. Panics if `layer_idx` is
    /// out of range — call sites should guard with `project.layers.get`
    /// first; the helper is intentionally not optional so the contract
    /// stays unambiguous.
    pub fn set_layer_mask_feather_mutation(&self, layer_idx: usize, new: f32) -> Mutation {
        let warp = &self.layers[layer_idx].warp;
        Mutation::SetLayerMaskFeather(SetLayerMaskFeather {
            layer_idx,
            new,
            old: warp.mask_feather,
        })
    }

    /// Build a `SetLayerWarpDimensions` mutation. The new grid is computed
    /// here via [`crate::project::schema::resample_grid`] so callers
    /// don't have to reason about the lossy resample — they just pass
    /// the new cell counts. `layer_idx` indexes `Project.layers`.
    pub fn set_layer_warp_dimensions_mutation(
        &self,
        layer_idx: usize,
        new_rows: u32,
        new_cols: u32,
    ) -> Mutation {
        let warp = &self.layers[layer_idx].warp;
        let new_grid = crate::project::schema::resample_grid(&warp.grid, new_rows, new_cols);
        Mutation::SetLayerWarpDimensions(SetLayerWarpDimensions {
            layer_idx,
            new_rows,
            new_cols,
            new_grid,
            old_rows: warp.rows,
            old_cols: warp.cols,
            old_grid: warp.grid.clone(),
        })
    }

    /// Build a `SetLayerOpacity` mutation. Panics if `layer_idx` is
    /// out of range.
    pub fn set_layer_opacity_mutation(&self, layer_idx: usize, new: f32) -> Mutation {
        let layer = &self.layers[layer_idx];
        Mutation::SetLayerOpacity(SetLayerOpacity {
            layer_idx,
            new,
            old: layer.opacity,
        })
    }

    /// Build a `SetLayerEnabled` mutation. Panics if `layer_idx` is
    /// out of range.
    pub fn set_layer_enabled_mutation(&self, layer_idx: usize, new: bool) -> Mutation {
        let layer = &self.layers[layer_idx];
        Mutation::SetLayerEnabled(SetLayerEnabled {
            layer_idx,
            new,
            old: layer.enabled,
        })
    }

    /// V31.6.1 — build a `SetLayerMuted` mutation. Captures the current
    /// `muted` flag as `old`. Panics if `layer_idx` is out of range.
    pub fn set_layer_muted_mutation(&self, layer_idx: usize, new: bool) -> Mutation {
        let layer = &self.layers[layer_idx];
        Mutation::SetLayerMuted(SetLayerMuted {
            layer_idx,
            new,
            old: layer.muted,
        })
    }

    /// V31.6.1 — build a `SetLayerSolo` mutation. Captures the current
    /// `solo` value as `old`. `new = None` clears the solo;
    /// `new = Some(idx)` solos that layer.
    pub fn set_solo_mutation(&self, new: Option<usize>) -> Mutation {
        Mutation::SetLayerSolo(SetLayerSolo {
            new,
            old: self.solo,
        })
    }

    /// V31.7.2 — build a `SetQuantizeBars` mutation. Captures the current
    /// `quantize_bars` value as `old`. `new = None` means immediate fire;
    /// `new = Some(n)` quantizes cue firing to n bars.
    pub fn set_quantize_bars_mutation(&self, new: Option<u8>) -> Mutation {
        Mutation::SetQuantizeBars(SetQuantizeBars {
            new,
            old: self.quantize_bars,
        })
    }

    /// Build a `SetLayerBlendMode` mutation. Whole-enum Reverse (rule 1):
    /// captures the full old `BlendMode` value. Panics if `layer_idx` is
    /// out of range.
    pub fn set_layer_blend_mode_mutation(&self, layer_idx: usize, new: BlendMode) -> Mutation {
        let layer = &self.layers[layer_idx];
        Mutation::SetLayerBlendMode(SetLayerBlendMode {
            layer_idx,
            new,
            old: layer.blend_mode,
        })
    }

    /// Build an `AddLayer` mutation. Caller-supplied `position` is the
    /// insertion index (clamped at apply time). `layer` is moved into the
    /// mutation; the caller will not be able to use it afterwards.
    pub fn set_add_layer_mutation(&self, layer: LayerConfig, position: usize) -> Mutation {
        Mutation::AddLayer { layer, position }
    }

    /// Build a `RemoveLayer` mutation. `idx` must be a valid index into
    /// `Project.layers` at the time of apply.
    pub fn set_remove_layer_mutation(&self, idx: usize) -> Mutation {
        Mutation::RemoveLayer { idx }
    }

    /// Build a `SwapLayers` mutation. `i` and `j` must both be valid indices
    /// at the time of apply.
    pub fn set_swap_layers_mutation(&self, i: usize, j: usize) -> Mutation {
        Mutation::SwapLayers(SwapLayers { i, j })
    }

    /// Build a `SetLayerMaskVertex` mutation. Captures the current polygon vertex
    /// as `old`. Panics if `layer_idx` or `idx` are out of range.
    pub fn set_layer_mask_vertex_mutation(
        &self,
        layer_idx: usize,
        idx: usize,
        new: [f32; 2],
    ) -> Mutation {
        let warp = &self.layers[layer_idx].warp;
        Mutation::SetLayerMaskVertex(SetLayerMaskVertex {
            layer_idx,
            idx,
            new,
            old: warp.mask_polygon[idx],
        })
    }

    /// Build an `AddLayerMaskVertex` mutation. `position` is the insertion index
    /// (0..=polygon.len()); the caller must ensure `layer_idx` is valid.
    pub fn set_add_layer_mask_vertex_mutation(
        &self,
        layer_idx: usize,
        position: usize,
        point: [f32; 2],
    ) -> Mutation {
        Mutation::AddLayerMaskVertex {
            layer_idx,
            position,
            point,
        }
    }

    /// Build a `RemoveLayerMaskVertex` mutation. `idx` must be a valid index into
    /// `WarpMesh.mask_polygon` at the time of apply.
    pub fn set_remove_layer_mask_vertex_mutation(&self, layer_idx: usize, idx: usize) -> Mutation {
        Mutation::RemoveLayerMaskVertex { layer_idx, idx }
    }

    /// Build a `ResetLayerWarpMesh` mutation. Captures the current warp mesh
    /// as `old` (full snapshot Reverse — rule 3). `new` is the full
    /// `WarpMesh` to install; typically the caller constructs the
    /// identity mesh and passes it here. Panics if `layer_idx` is out
    /// of range.
    pub fn set_reset_layer_warp_mesh_mutation(
        &self,
        layer_idx: usize,
        new: crate::project::schema::WarpMesh,
    ) -> Mutation {
        let old = self.layers[layer_idx].warp.clone();
        Mutation::ResetLayerWarpMesh(ResetLayerWarpMesh {
            layer_idx,
            new,
            old,
        })
    }

    /// P7.5.1/P7.6.1 — Build a `SetLayerMaskGraph` mutation. Captures the current
    /// `mask_graph` as `old` (whole-Option snapshot Reverse — rule 3). Panics
    /// if `layer_idx` is out of range.
    pub fn set_layer_mask_graph_mutation(
        &self,
        layer_idx: usize,
        new: Option<crate::project::schema::MaskGraph>,
    ) -> Mutation {
        let old = self.layers[layer_idx].mask_graph.clone();
        Mutation::SetLayerMaskGraph(SetLayerMaskGraph {
            layer_idx,
            new,
            old,
        })
    }

    /// P7.3.1 — Build a `ResetLayerBezierMesh` mutation. Captures the current
    /// `bezier_mesh` as `old` (whole-Option snapshot Reverse — rule 3). Panics
    /// if `layer_idx` is out of range.
    pub fn set_reset_layer_bezier_mesh_mutation(
        &self,
        layer_idx: usize,
        new: Option<crate::project::schema::BezierMesh>,
    ) -> Mutation {
        let old = self.layers[layer_idx].bezier_mesh.clone();
        Mutation::ResetLayerBezierMesh(ResetLayerBezierMesh {
            layer_idx,
            new,
            old,
        })
    }

    /// P7.3.3 — Build a `MoveBezierAnchor` mutation. Captures the current anchor
    /// position as `old`. Panics if `layer_idx`, `anchor_row`, or `anchor_col`
    /// are out of range, or if the layer has no `bezier_mesh`.
    pub fn move_bezier_anchor_mutation(
        &self,
        layer_idx: usize,
        anchor_row: usize,
        anchor_col: usize,
        new_pos: [f32; 2],
    ) -> Mutation {
        let bm = self.layers[layer_idx]
            .bezier_mesh
            .as_ref()
            .expect("move_bezier_anchor_mutation: layer has no bezier_mesh");
        let old_pos = bm.anchors[anchor_row][anchor_col];
        Mutation::MoveBezierAnchor(MoveBezierAnchor {
            layer_idx,
            anchor_row,
            anchor_col,
            new_pos,
            old_pos,
            // PCleanup.bezier-handle-reverse — forward mutation: handle
            // snapshots are populated by apply, not by the constructor.
            old_h_handle: None,
            old_v_handle: None,
        })
    }

    /// P7.3.3 — Build a `SetBezierHandle` mutation. Captures the current handle
    /// value as `old`. Panics if `layer_idx`, `anchor_row`, or `anchor_col` are
    /// out of range, or if the layer has no `bezier_mesh`.
    pub fn set_bezier_handle_mutation(
        &self,
        layer_idx: usize,
        anchor_row: usize,
        anchor_col: usize,
        direction: crate::project::schema::BezierHandleDir,
        new_pos: crate::project::schema::BezierHandle,
    ) -> Mutation {
        use crate::project::schema::BezierHandleDir;
        let bm = self.layers[layer_idx]
            .bezier_mesh
            .as_ref()
            .expect("set_bezier_handle_mutation: layer has no bezier_mesh");
        let old_pos = match direction {
            BezierHandleDir::Horizontal => bm.handles_h[anchor_row][anchor_col],
            BezierHandleDir::Vertical => bm.handles_v[anchor_row][anchor_col],
        };
        Mutation::SetBezierHandle(SetBezierHandle {
            layer_idx,
            anchor_row,
            anchor_col,
            direction,
            new_pos,
            old_pos,
        })
    }

    /// Build a `SetLayerMaskPolygon` mutation. Captures the current
    /// `mask_polygon` as `old` (whole-Vec Reverse). Panics if
    /// `layer_idx` is out of range.
    pub fn set_layer_mask_polygon_mutation(
        &self,
        layer_idx: usize,
        new: Vec<[f32; 2]>,
    ) -> Mutation {
        let old = self.layers[layer_idx].warp.mask_polygon.clone();
        Mutation::SetLayerMaskPolygon(SetLayerMaskPolygon {
            layer_idx,
            new,
            old,
        })
    }

    /// Build a `SetLayerWarpCorner` mutation. Captures the current grid vertex
    /// as `old`. Panics if `layer_idx`, `r`, or `c` are out of range.
    pub fn set_layer_warp_corner_mutation(
        &self,
        layer_idx: usize,
        r: usize,
        c: usize,
        new: [f32; 2],
    ) -> Mutation {
        let old = self.layers[layer_idx].warp.grid[r][c];
        Mutation::SetLayerWarpCorner(SetLayerWarpCorner {
            layer_idx,
            r,
            c,
            new,
            old,
        })
    }

    /// Build a `SetProjectScenes` mutation (whole-Vec Reverse). Captures the
    /// current `project.cues` as `old`; `new` is the replacement Vec to
    /// install (e.g. after a slot save or placeholder extension). The Reverse
    /// restores the entire pre-save Vec byte-equally on undo.
    pub fn set_project_scenes_mutation(&self, new: Vec<crate::project::schema::Cue>) -> Mutation {
        Mutation::SetProjectScenes(SetProjectScenes {
            new,
            old: self.cues.clone(),
        })
    }

    /// P6.2.2 — Build a `SetProjectCues` mutation (whole-Vec Reverse, rule 3).
    /// Captures the current `project.cues` as `old`; `new` is the replacement.
    pub fn set_project_cues_mutation(&self, new: Vec<crate::project::schema::Cue>) -> Mutation {
        Mutation::SetProjectCues(SetProjectCues {
            new,
            old: self.cues.clone(),
        })
    }

    /// P6.2.2 — Build a `SetCueName` mutation. Captures the current name
    /// as `old`. Panics if `cue_idx` is out of range.
    pub fn set_cue_name_mutation(&self, cue_idx: usize, new: String) -> Mutation {
        let old = self.cues[cue_idx].name.clone();
        Mutation::SetCueName(SetCueName { cue_idx, new, old })
    }

    /// P6.2.2 — Build a `SetCueTiming` mutation. Captures the current timing
    /// snapshot as `old`. Panics if `cue_idx` is out of range.
    pub fn set_cue_timing_mutation(&self, cue_idx: usize, new: CueTimingSnapshot) -> Mutation {
        let old = CueTimingSnapshot::from_cue(&self.cues[cue_idx]);
        Mutation::SetCueTiming(SetCueTiming { cue_idx, new, old })
    }

    /// Build a `SetLayerEffects` mutation. Captures the current effect chain
    /// as `old` for the Effects-Vec Reverse (rule 2). `new` is moved into
    /// the mutation; the caller will not be able to use it afterwards.
    /// Panics if `layer_idx` is out of range.
    pub fn set_layer_effects_mutation(
        &self,
        layer_idx: usize,
        new: Vec<crate::effects::Effect>,
    ) -> Mutation {
        let layer = &self.layers[layer_idx];
        Mutation::SetLayerEffects(SetLayerEffects {
            layer_idx,
            new,
            old: layer.effects.clone(),
        })
    }

    /// 004-T1.13 — Build a `SetLayerEffectsAndMask` mutation. Captures the
    /// current effect chain and mask polygon as `old_*` for Effects-Vec
    /// Reverse (rule 2) and whole-Vec mask Reverse (rule 2). Both `new_*`
    /// arguments are moved into the mutation. Panics if `layer_idx` is out
    /// of range.
    pub fn set_layer_effects_and_mask_mutation(
        &self,
        layer_idx: usize,
        new_effects: Vec<crate::effects::Effect>,
        new_mask_polygon: Vec<[f32; 2]>,
    ) -> Mutation {
        let layer = &self.layers[layer_idx];
        Mutation::SetLayerEffectsAndMask(SetLayerEffectsAndMask {
            layer_idx,
            new_effects,
            old_effects: layer.effects.clone(),
            new_mask_polygon,
            old_mask_polygon: layer.warp.mask_polygon.clone(),
        })
    }

    /// Build a `SetModulator` mutation. Captures the current modulator at
    /// `(layer_idx, effect_idx, field)` as `old` for whole-enum Reverse
    /// (rule 1). `new` is the full `Modulator` enum value to install.
    /// Panics if the indices are out of range or `field` doesn't apply to
    /// the effect variant at `effect_idx`.
    pub fn set_modulator_mutation(
        &self,
        layer_idx: usize,
        effect_idx: usize,
        field: ModulatorField,
        new: crate::modulators::Modulator,
    ) -> Mutation {
        let layer = &self.layers[layer_idx];
        let effect = &layer.effects[effect_idx];
        let old = modulator_at_ref(effect, field)
            .expect("set_modulator_mutation: field does not apply to effect variant")
            .clone();
        Mutation::SetModulator(SetModulator {
            layer_idx,
            effect_idx,
            field,
            new,
            old,
        })
    }

    /// P0.4.3 — build a `SetVideoSpeed` mutation. Captures the current
    /// `speed` as `old`. Panics if `layer_idx` is out of range or if
    /// the layer is not a `LayerKind::Video`.
    pub fn set_video_speed_mutation(&self, layer_idx: usize, new: f32) -> Mutation {
        let old = match &self.layers[layer_idx].kind {
            crate::project::schema::LayerKind::Video { speed, .. } => *speed,
            _ => panic!(
                "set_video_speed_mutation: layer {} is not a Video layer",
                layer_idx
            ),
        };
        Mutation::SetVideoSpeed(SetVideoSpeed {
            layer_idx,
            new,
            old,
        })
    }

    /// P1.4.2 — build a `SetVideoLoopMode` mutation. Captures the current
    /// `loop_mode` as `old`. Panics if `layer_idx` is out of range or if
    /// the layer is not a `LayerKind::Video`. Replaces the P0.4.3
    /// `set_video_loop_seamless_mutation`.
    pub fn set_video_loop_mode_mutation(
        &self,
        layer_idx: usize,
        new: crate::project::schema::LoopMode,
    ) -> Mutation {
        let old = match &self.layers[layer_idx].kind {
            crate::project::schema::LayerKind::Video { loop_mode, .. } => *loop_mode,
            _ => panic!(
                "set_video_loop_mode_mutation: layer {} is not a Video layer",
                layer_idx
            ),
        };
        Mutation::SetVideoLoopMode(SetVideoLoopMode {
            layer_idx,
            new,
            old,
        })
    }

    /// P1.2.4 — build a `SetLayerFocal` mutation. Captures the layer's
    /// current focal as `old`. Panics if `layer_idx` is out of range
    /// or the layer is not an Image / Video layer (those are the only
    /// kinds that carry a `focal` field).
    pub fn set_layer_focal_mutation(&self, layer_idx: usize, new: [f32; 2]) -> Mutation {
        let old = match &self.layers[layer_idx].kind {
            crate::project::schema::LayerKind::Image { focal, .. }
            | crate::project::schema::LayerKind::Video { focal, .. } => *focal,
            _ => panic!(
                "set_layer_focal_mutation: layer {} is not an Image or Video layer",
                layer_idx
            ),
        };
        Mutation::SetLayerFocal(SetLayerFocal {
            layer_idx,
            new,
            old,
        })
    }

    /// P1.4.4 — build a `SetVideoBpmLock` mutation. Captures the
    /// layer's current `bpm_lock` as `old`. Panics if `layer_idx` is
    /// out of range or the layer is not a `LayerKind::Video`.
    pub fn set_video_bpm_lock_mutation(&self, layer_idx: usize, new: bool) -> Mutation {
        let old = match &self.layers[layer_idx].kind {
            crate::project::schema::LayerKind::Video { bpm_lock, .. } => *bpm_lock,
            _ => panic!(
                "set_video_bpm_lock_mutation: layer {} is not a Video layer",
                layer_idx
            ),
        };
        Mutation::SetVideoBpmLock(SetVideoBpmLock {
            layer_idx,
            new,
            old,
        })
    }

    /// P1.4.1 — build a `SetVideoClipRange` mutation. Captures the
    /// current `(clip_in, clip_out)` as `old_in, old_out`. Panics if
    /// `layer_idx` is out of range or the layer is not a
    /// `LayerKind::Video`. Caller is expected to validate
    /// `new_in < new_out` and `new_in >= 0` — invalid ranges build into
    /// an obviously-wrong mutation that `debug_assert!` will catch in
    /// test, but release builds carry the bad value forward.
    pub fn set_video_clip_range_mutation(
        &self,
        layer_idx: usize,
        new_in: f32,
        new_out: f32,
    ) -> Mutation {
        let (old_in, old_out) = match &self.layers[layer_idx].kind {
            crate::project::schema::LayerKind::Video {
                clip_in, clip_out, ..
            } => (*clip_in, *clip_out),
            _ => panic!(
                "set_video_clip_range_mutation: layer {} is not a Video layer",
                layer_idx
            ),
        };
        Mutation::SetVideoClipRange(SetVideoClipRange {
            layer_idx,
            new_in,
            new_out,
            old_in,
            old_out,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_project() -> Project {
        let mut p = Project::default();
        if p.layers.is_empty() {
            use std::path::PathBuf;
            p.layers.push(crate::project::schema::layer_from_svg_path(
                "test_layer",
                PathBuf::from("/tmp/rmap_test.svg"),
            ));
        }
        // Seed a 4-vertex mask polygon on the first layer's warp so
        // proptest can exercise mask-vertex mutations under the ≥3
        // guard (RemoveMaskVertex requires len > 3 to fire; an empty
        // polygon yields only fallback coverage).
        if p.layers[0].warp.mask_polygon.len() < 4 {
            p.layers[0].warp.mask_polygon = vec![[0.1, 0.1], [0.9, 0.1], [0.9, 0.9], [0.1, 0.9]];
        }
        // P2.9.1 — Append an FxLayer at index 1 so the `SetFxLayerParams`
        // proptest strategy has a real target. Uses RIPPLE_WASH_PRESET_ID
        // (no `max_particle_count` on any descriptor → every in-range value
        // commits). Non-zero `seed` and `t_layer_added_secs` ensure the
        // `ApplyProjectSnapshot` Reverse exercises both new P2.5.1 fields.
        // Layer 0 remains SVG so all strategies that target index 0 are
        // unaffected.
        if p.layers.len() == 1 {
            use crate::render::fx_presets::RIPPLE_WASH_PRESET_ID;
            p.layers.push(crate::project::schema::LayerConfig {
                id: "test_fx_layer".to_string(),
                kind: crate::project::schema::LayerKind::FxLayer {
                    preset_id: RIPPLE_WASH_PRESET_ID.to_string(),
                    params: std::collections::HashMap::new(),
                    seed: 42,
                    t_layer_added_secs: 1.5,
                },
                enabled: true,
                transform: crate::project::schema::Transform2D::default(),
                effects: crate::effects::default_effect_chain(),
                blend_mode: crate::project::schema::BlendMode::Normal,
                opacity: 1.0,
                warp: crate::project::schema::WarpMesh::default_placement(),
                muted: false,
                treatment: None,
                bezier_mesh: None,
                mask_graph: None,
            });
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

    /// V31.3.1 — exercises `ReverseStorage::apply` directly on the `SetGamma`
    /// struct, bypassing `Mutation::apply`. This confirms that the trait impl
    /// is self-consistent and that `Mutation::SetGamma(s) => s.apply(project)`
    /// delegation is not hiding a bug in the wrapper path.
    #[test]
    fn set_gamma_round_trip_via_trait() {
        let mut project = fresh_project();
        project.gamma = 1.0;

        // Apply forward: 1.0 → 2.5
        let s = SetGamma { new: 2.5, old: 1.0 };
        let reverse = s.apply(&mut project);
        assert!(
            (project.gamma - 2.5).abs() < 1e-6,
            "after forward apply, gamma should be 2.5; got {}",
            project.gamma
        );
        assert!(
            (reverse.new - 1.0).abs() < 1e-6,
            "reverse.new should be old value 1.0; got {}",
            reverse.new
        );
        assert!(
            (reverse.old - 2.5).abs() < 1e-6,
            "reverse.old should be new value 2.5; got {}",
            reverse.old
        );

        // Apply reverse: 2.5 → 1.0
        let _second_reverse = reverse.apply(&mut project);
        assert!(
            (project.gamma - 1.0).abs() < 1e-6,
            "after reverse apply, gamma should be restored to 1.0; got {}",
            project.gamma
        );
    }

    /// 003-T3.28 — Apply/Reverse on the per-display tone overrides round-
    /// trips through every transition the operator exercises:
    ///   None → Some(v) → None  (toggle on then off)
    ///   Some(a) → Some(b)      (drag slider while enabled)
    /// Only one variant is asserted explicitly; brightness/contrast share
    /// the same shape and are covered by `arb_mutation_kind` in proptest.
    #[test]
    fn set_project_gamma_override_round_trips() {
        let mut p = fresh_project();

        // None → Some(2.5)
        let m = p.set_project_gamma_override_mutation(Some(2.5));
        let reverse = m.apply(&mut p);
        assert_eq!(p.gamma_override, Some(2.5));

        // Reverse: Some(2.5) → None
        let _ = reverse.apply(&mut p);
        assert_eq!(p.gamma_override, None);

        // Some(a) → Some(b) chain
        p.gamma_override = Some(1.2);
        let m = p.set_project_gamma_override_mutation(Some(3.4));
        let reverse = m.apply(&mut p);
        assert_eq!(p.gamma_override, Some(3.4));
        let _ = reverse.apply(&mut p);
        assert_eq!(p.gamma_override, Some(1.2));
    }

    /// P0.7.3 — `SetEdgeBlend` round-trips through apply + Reverse for every
    /// transition the operator exercises:
    ///   None → Some(cfg) → None  (enable then disable)
    ///   Some(a) → Some(b)        (update config while enabled)
    #[test]
    fn set_edge_blend_round_trips() {
        use crate::project::schema::{EdgeBlendConfig, FalloffCurve};

        let mut p = fresh_project();
        assert_eq!(p.edge_blend, None);

        // None → Some(64, Cosine)
        let cfg_a = EdgeBlendConfig {
            overlap_px: 64,
            falloff_curve: FalloffCurve::Cosine,
        };
        let m = p.set_edge_blend_mutation(Some(cfg_a));
        let reverse = m.apply(&mut p);
        assert_eq!(p.edge_blend, Some(cfg_a));

        // Reverse: Some → None
        let _ = reverse.apply(&mut p);
        assert_eq!(p.edge_blend, None);

        // Some(a) → Some(b) chain
        p.edge_blend = Some(cfg_a);
        let cfg_b = EdgeBlendConfig {
            overlap_px: 128,
            falloff_curve: FalloffCurve::Linear,
        };
        let m = p.set_edge_blend_mutation(Some(cfg_b));
        let reverse = m.apply(&mut p);
        assert_eq!(p.edge_blend, Some(cfg_b));
        let _ = reverse.apply(&mut p);
        assert_eq!(p.edge_blend, Some(cfg_a));
    }

    /// P0.7.3 — `SetEdgeBlend` is undoable (user-driven config change).
    #[test]
    fn set_edge_blend_is_undoable() {
        use crate::project::schema::{EdgeBlendConfig, FalloffCurve};
        let p = fresh_project();
        let cfg = EdgeBlendConfig {
            overlap_px: 32,
            falloff_curve: FalloffCurve::Linear,
        };
        assert!(!p.set_edge_blend_mutation(Some(cfg)).is_non_undoable());
        assert!(!p.set_edge_blend_mutation(None).is_non_undoable());
    }

    /// P0.7.3 — `SetEdgeBlend` does not invalidate the layer GPU state.
    #[test]
    fn set_edge_blend_does_not_need_layer_rebuild() {
        use crate::project::schema::{EdgeBlendConfig, FalloffCurve};
        let p = fresh_project();
        let cfg = EdgeBlendConfig {
            overlap_px: 32,
            falloff_curve: FalloffCurve::Linear,
        };
        assert!(!p.set_edge_blend_mutation(Some(cfg)).needs_layer_rebuild());
        assert!(!p.set_edge_blend_mutation(None).needs_layer_rebuild());
    }

    /// P1.2.1 — `SetLayerTreatment` round-trip across the four
    /// state transitions:
    ///   None → Some(t1) → None  (enable then disable)
    ///   Some(a) → Some(b)        (switch preset / params atomically)
    #[test]
    fn set_layer_treatment_round_trips() {
        use crate::project::schema::Treatment;

        let mut p = fresh_project();
        assert!(p.layers[0].treatment.is_none());

        let t1 = Treatment {
            preset_id: "tone_map".into(),
            params: {
                let mut m = std::collections::HashMap::new();
                m.insert("exposure".to_string(), 0.5);
                m
            },
            overlay_path: None,
            collage_paths: vec![],
        };

        // None → Some(t1)
        let m = p.set_layer_treatment_mutation(0, Some(t1.clone()));
        let reverse = m.apply(&mut p);
        assert_eq!(p.layers[0].treatment, Some(t1.clone()));

        // Some(t1) → None (via reverse)
        let _ = reverse.apply(&mut p);
        assert!(p.layers[0].treatment.is_none());

        // Some(a) → Some(b)
        p.layers[0].treatment = Some(t1.clone());
        let t2 = Treatment {
            preset_id: "blur_mask".into(),
            params: std::collections::HashMap::new(),
            overlay_path: None,
            collage_paths: vec![],
        };
        let m = p.set_layer_treatment_mutation(0, Some(t2.clone()));
        let reverse = m.apply(&mut p);
        assert_eq!(p.layers[0].treatment, Some(t2));
        let _ = reverse.apply(&mut p);
        assert_eq!(p.layers[0].treatment, Some(t1));
    }

    /// P1.2.1 — `SetLayerTreatmentParams` snapshots the whole map so
    /// a preset switch racing a param edit doesn't lose keys silently.
    #[test]
    fn set_layer_treatment_params_round_trips() {
        use crate::project::schema::Treatment;

        let mut p = fresh_project();
        p.layers[0].treatment = Some(Treatment {
            preset_id: "tone_map".into(),
            params: {
                let mut m = std::collections::HashMap::new();
                m.insert("exposure".to_string(), 0.0);
                m.insert("contrast".to_string(), 1.0);
                m
            },
            overlay_path: None,
            collage_paths: vec![],
        });

        let mut new_params = std::collections::HashMap::new();
        new_params.insert("exposure".to_string(), 0.5);
        new_params.insert("contrast".to_string(), 1.2);
        new_params.insert("shoulder".to_string(), 0.7);

        let m = p.set_layer_treatment_params_mutation(0, new_params.clone());
        let reverse = m.apply(&mut p);
        assert_eq!(p.layers[0].treatment.as_ref().unwrap().params, new_params);

        // Reverse restores the original 2-key map (including dropping
        // the new `shoulder` key — whole-map snapshot, not merge).
        let _ = reverse.apply(&mut p);
        let restored = &p.layers[0].treatment.as_ref().unwrap().params;
        assert_eq!(restored.len(), 2);
        assert!(restored.contains_key("exposure"));
        assert!(restored.contains_key("contrast"));
        assert!(
            !restored.contains_key("shoulder"),
            "whole-map Reverse must drop keys not present pre-mutation"
        );
    }

    /// P1.2.1 — both treatment mutations are undoable + don't trigger
    /// a layer-GPU rebuild (the treatment runs inside the existing
    /// per-layer render pipeline; no layer-Vec reshape).
    #[test]
    fn treatment_mutations_are_undoable_and_no_rebuild() {
        use crate::project::schema::Treatment;
        let mut p = fresh_project();

        let t = Treatment {
            preset_id: "tone_map".into(),
            params: std::collections::HashMap::new(),
            overlay_path: None,
            collage_paths: vec![],
        };

        let m1 = p.set_layer_treatment_mutation(0, Some(t.clone()));
        assert!(!m1.is_non_undoable());
        assert!(!m1.needs_layer_rebuild());

        // Need a populated treatment for set_layer_treatment_params_mutation.
        p.layers[0].treatment = Some(t);
        let m2 = p.set_layer_treatment_params_mutation(0, std::collections::HashMap::new());
        assert!(!m2.is_non_undoable());
        assert!(!m2.needs_layer_rebuild());
    }

    /// P1.2.1 — builder panics on a layer without a treatment when
    /// asked for `set_layer_treatment_params_mutation`. The UI must
    /// guard this; the panic catches contract violations in tests.
    #[test]
    #[should_panic(
        expected = "set_layer_treatment_params_mutation called on a layer with no treatment"
    )]
    fn set_layer_treatment_params_mutation_panics_on_no_treatment() {
        let p = fresh_project();
        // `fresh_project`'s layer has treatment: None — panic expected.
        let _ = p.set_layer_treatment_params_mutation(0, std::collections::HashMap::new());
    }

    // -----------------------------------------------------------------------
    // P2.5.6 — SetFxLayerParams tests
    // -----------------------------------------------------------------------

    /// Helper: build a fresh project with an FxLayer at index 0.
    fn fresh_fx_project() -> crate::project::schema::Project {
        use crate::render::fx_presets::PARTICLES_IDENTITY_PRESET_ID;
        let mut p = fresh_project();
        // Replace the SVG layer with an FxLayer.
        p.layers[0].kind = crate::project::schema::LayerKind::FxLayer {
            preset_id: PARTICLES_IDENTITY_PRESET_ID.to_string(),
            params: {
                let mut m = std::collections::HashMap::new();
                m.insert("particle_count".to_string(), 8.0_f32);
                m
            },
            seed: 0,
            t_layer_added_secs: 0.0,
        };
        p
    }

    /// P2.5.6 — `SetFxLayerParams` apply + reverse restores the original
    /// params map byte-equal. Exercises the whole-HashMap snapshot Reverse.
    #[test]
    fn set_fx_layer_params_round_trip() {
        let mut p = fresh_fx_project();

        let mut new_params = std::collections::HashMap::new();
        new_params.insert("particle_count".to_string(), 12.0_f32);

        let m = p.set_fx_layer_params_mutation(0, new_params.clone());
        let reverse = m.apply(&mut p);

        // After apply the project should reflect `new_params`.
        let params_after = match &p.layers[0].kind {
            crate::project::schema::LayerKind::FxLayer { params, .. } => params.clone(),
            _ => panic!("expected FxLayer"),
        };
        assert_eq!(params_after, new_params, "apply should install new_params");

        // Applying the reverse should restore the original params.
        let _ = reverse.apply(&mut p);
        let params_restored = match &p.layers[0].kind {
            crate::project::schema::LayerKind::FxLayer { params, .. } => params.clone(),
            _ => panic!("expected FxLayer"),
        };
        assert_eq!(
            params_restored["particle_count"].to_bits(),
            8.0_f32.to_bits()
        );
    }

    /// P2.5.6 — when `new` exceeds `max_particle_count`, `apply` is a no-op:
    /// the project state is unchanged and the returned Reverse has new == old.
    #[test]
    fn set_fx_layer_params_over_budget_is_noop() {
        let mut p = fresh_fx_project();

        // max_particle_count for particles_identity is 16; 99999 exceeds it.
        let mut over_budget = std::collections::HashMap::new();
        over_budget.insert("particle_count".to_string(), 99999.0_f32);

        let m = p.set_fx_layer_params_mutation(0, over_budget.clone());
        let reverse = m.apply(&mut p);

        // Project state must be unchanged.
        let params_after = match &p.layers[0].kind {
            crate::project::schema::LayerKind::FxLayer { params, .. } => params.clone(),
            _ => panic!("expected FxLayer"),
        };
        assert_eq!(
            params_after["particle_count"].to_bits(),
            8.0_f32.to_bits(),
            "over-budget apply must not mutate the project"
        );

        // The returned Reverse is also a no-op (new == old == current).
        let reverse_inner = match reverse {
            Mutation::SetFxLayerParams(s) => s,
            _ => panic!("expected SetFxLayerParams"),
        };
        assert_eq!(
            reverse_inner.new, reverse_inner.old,
            "refused Reverse must have new == old"
        );
    }

    /// P2.5.6 — `fx_layer_params_over_budget` returns `Some(...)` when
    /// particle count exceeds the descriptor cap, and `None` when within.
    #[test]
    fn fx_layer_params_over_budget_pre_flight() {
        let p = fresh_fx_project();

        let mut within_budget = std::collections::HashMap::new();
        within_budget.insert("particle_count".to_string(), 14.0_f32);
        assert!(
            p.fx_layer_params_over_budget(0, &within_budget).is_none(),
            "14 <= 16: should be within budget"
        );

        let mut over_budget = std::collections::HashMap::new();
        over_budget.insert("particle_count".to_string(), 99.0_f32);
        let result = p.fx_layer_params_over_budget(0, &over_budget);
        assert!(result.is_some(), "99 > 16: should be over budget");
        let (key, val, max) = result.unwrap();
        assert_eq!(key, "particle_count");
        assert!((val - 99.0).abs() < 1e-6);
        assert_eq!(max, 16);
    }

    /// P2.5.6 — `SetFxLayerParams` is undoable and does not trigger a
    /// layer-GPU rebuild (params flow through the per-frame uniform write;
    /// no `LayerState` Vec reshape is needed).
    #[test]
    fn set_fx_layer_params_is_undoable_no_rebuild() {
        let p = fresh_fx_project();
        let mut params = std::collections::HashMap::new();
        params.insert("particle_count".to_string(), 10.0_f32);
        let m = p.set_fx_layer_params_mutation(0, params);
        assert!(!m.is_non_undoable(), "SetFxLayerParams must be undoable");
        assert!(
            !m.needs_layer_rebuild(),
            "SetFxLayerParams must not need layer rebuild"
        );
    }

    /// 003-T3.28 — overrides are user-driven and Cmd-Z reversible.
    #[test]
    fn project_overrides_are_undoable() {
        let p = fresh_project();
        assert!(
            !p.set_project_gamma_override_mutation(Some(2.0))
                .is_non_undoable()
        );
        assert!(
            !p.set_project_brightness_override_mutation(Some(0.1))
                .is_non_undoable()
        );
        assert!(
            !p.set_project_contrast_override_mutation(None)
                .is_non_undoable()
        );
    }

    /// 003-T2.24 — `RelinkAssetPath` round-trip restores the original
    /// asset path byte-equal when applied + reversed. The fresh
    /// project's seeded layer is `LayerKind::Svg { svg_path: ... }`
    /// (Cargo.toml as the standin asset); we point it at a different
    /// path, then undo via the returned Reverse, and assert the
    /// pre-mutation project serialises identically.
    #[test]
    fn relink_asset_path_round_trips() {
        let mut p = fresh_project();
        let before = serde_json::to_value(&p).unwrap();
        let original = p.layers[0]
            .kind
            .asset_path()
            .expect("fresh_project seeds a Svg layer with an asset path")
            .to_path_buf();

        let new_path = std::path::PathBuf::from("/some/other/place.svg");
        let mutation = Mutation::RelinkAssetPath(RelinkAssetPath {
            layer_idx: 0,
            new_path: new_path.clone(),
            old_path: original.clone(),
        });
        let reverse = mutation.apply(&mut p);
        assert_eq!(
            p.layers[0].kind.asset_path(),
            Some(new_path.as_path()),
            "apply should rewrite svg_path",
        );

        let _ = reverse.apply(&mut p);
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(before, after, "Reverse should restore byte-equal project",);
    }

    /// P0.5.1 — `SetLayerKind` round-trip restores the original
    /// `LayerKind` byte-equal when applied + reversed. Exercises the
    /// FxLayer params HashMap to confirm whole-enum Reverse covers
    /// the new field.
    #[test]
    fn set_layer_kind_fx_round_trips() {
        let mut p = fresh_project();
        let before = serde_json::to_value(&p).unwrap();
        let original_kind = p.layers[0].kind.clone();

        let mut params = std::collections::HashMap::new();
        params.insert("speed".to_string(), 1.5);
        params.insert("falloff".to_string(), 0.3);
        let new_kind = crate::project::schema::LayerKind::FxLayer {
            preset_id: "ripple_wash".to_string(),
            params,
            seed: 0,
            t_layer_added_secs: 0.0,
        };

        // Apply: replaces the Svg kind with FxLayer.
        // The discriminant check fires only on apply of the reverse,
        // so we provide a matching `old` (Svg) for the forward apply.
        let mutation = Mutation::SetLayerKind(SetLayerKind {
            layer_idx: 0,
            new: new_kind.clone(),
            old: original_kind.clone(),
        });
        let reverse = mutation.apply(&mut p);
        assert!(
            matches!(
                p.layers[0].kind,
                crate::project::schema::LayerKind::FxLayer { .. }
            ),
            "apply should install FxLayer kind",
        );
        if let crate::project::schema::LayerKind::FxLayer { params, .. } = &p.layers[0].kind {
            assert_eq!(params.len(), 2);
            assert!((params["speed"] - 1.5).abs() < 1e-6);
            assert!((params["falloff"] - 0.3).abs() < 1e-6);
        }

        let _ = reverse.apply(&mut p);
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(
            before, after,
            "Reverse should restore byte-equal project (whole-enum Reverse rule 1)",
        );
    }

    /// P0.8.1 — `SetOutputRgbMatrix` round-trips a non-identity
    /// matrix bit-exact through apply + reverse for output_idx 0.
    /// Confirms the whole-matrix Reverse rule + the exact-match
    /// `debug_assert_eq!` in `apply`.
    #[test]
    fn set_output_rgb_matrix_round_trips() {
        let mut p = fresh_project();
        let identity = crate::project::schema::rgb_matrix_identity();
        assert_eq!(p.primary_output_target().rgb_matrix, identity);

        let new_matrix = [[0.95, 0.03, 0.02], [0.04, 0.96, 0.00], [0.01, 0.02, 0.97]];
        let before = serde_json::to_value(&p).unwrap();

        let mutation = Mutation::SetOutputRgbMatrix(SetOutputRgbMatrix {
            output_idx: 0,
            new: new_matrix,
            old: identity,
        });
        let reverse = mutation.apply(&mut p);
        assert_eq!(p.output_targets[0].rgb_matrix, new_matrix);

        let _ = reverse.apply(&mut p);
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(
            before, after,
            "Reverse should restore the project byte-equal (whole-matrix Reverse)",
        );
    }

    /// PCleanup.7.3 — `SetOutputGammaOverride` round-trips through
    /// apply + reverse with whole-Option Reverse semantics (Some →
    /// None → Some restores byte-equal).
    #[test]
    fn set_output_gamma_override_round_trips() {
        let mut p = fresh_project();
        assert_eq!(p.output_targets[0].gamma_override, None);
        let before = serde_json::to_value(&p).unwrap();

        let m = p.set_output_gamma_override_mutation(0, Some(1.4));
        let reverse = m.apply(&mut p);
        assert_eq!(p.output_targets[0].gamma_override, Some(1.4));

        let _ = reverse.apply(&mut p);
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(
            before, after,
            "PCleanup.7.3: SetOutputGammaOverride Reverse must restore byte-equal"
        );
    }

    /// PCleanup.7.3 — same round-trip guarantee for the brightness +
    /// contrast override mutations.
    #[test]
    fn set_output_brightness_and_contrast_overrides_round_trip() {
        let mut p = fresh_project();
        let before = serde_json::to_value(&p).unwrap();

        let mb = p.set_output_brightness_override_mutation(0, Some(0.15));
        let rb = mb.apply(&mut p);
        let mc = p.set_output_contrast_override_mutation(0, Some(1.1));
        let rc = mc.apply(&mut p);
        assert_eq!(p.output_targets[0].brightness_override, Some(0.15));
        assert_eq!(p.output_targets[0].contrast_override, Some(1.1));

        // Reverse in reverse order to restore the original state.
        let _ = rc.apply(&mut p);
        let _ = rb.apply(&mut p);
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(
            before, after,
            "PCleanup.7.3: brightness + contrast override Reverse must restore byte-equal"
        );
    }

    /// P0.8.1 — `set_output_rgb_matrix_mutation(1, new)` on a project with
    /// 2 targets applies to `output_targets[1]` only; `output_targets[0]`
    /// is unchanged.
    #[test]
    fn set_output_rgb_matrix_per_output_round_trip() {
        use crate::project::schema::OutputTarget;
        let mut p = fresh_project();
        // Ensure a second output target exists.
        p.output_targets.push(OutputTarget::default());
        assert_eq!(p.output_targets.len(), 2);

        let identity = crate::project::schema::rgb_matrix_identity();
        let new_matrix = [[0.8, 0.1, 0.1], [0.1, 0.8, 0.1], [0.1, 0.1, 0.8]];

        let m = p.set_output_rgb_matrix_mutation(1, new_matrix);
        let _reverse = m.apply(&mut p);

        assert_eq!(
            p.output_targets[1].rgb_matrix, new_matrix,
            "output_targets[1].rgb_matrix should be updated",
        );
        assert_eq!(
            p.output_targets[0].rgb_matrix, identity,
            "output_targets[0].rgb_matrix must be unchanged",
        );
    }

    /// P0.8.1 — applying `SetOutputRgbMatrix` with an out-of-range
    /// `output_idx` must panic with a message containing "out of range".
    #[test]
    #[should_panic(expected = "out of range")]
    fn set_output_rgb_matrix_out_of_range_panics() {
        let mut p = fresh_project();
        let identity = crate::project::schema::rgb_matrix_identity();
        let m = Mutation::SetOutputRgbMatrix(SetOutputRgbMatrix {
            output_idx: 99,
            new: identity,
            old: identity,
        });
        let _ = m.apply(&mut p);
    }

    /// V31.1.4 follow-up — `Mutation::ApplyProjectSnapshot` round-trips
    /// a layer with `effects: vec![]` (empty) without silently filling
    /// in `default_effect_chain()` on undo or redo. The `proptest_round_trip`
    /// harness seeds the project via `fresh_project()`, which uses
    /// `layer_from_svg_path` and the default 3-element effect chain — so
    /// the empty-vec case is never exercised by the property tests. This
    /// closes that gap directly through the v3 mutation pipeline.
    #[test]
    fn apply_project_snapshot_preserves_empty_effects_vec() {
        let mut p = fresh_project();
        p.layers[0].effects = Vec::new();
        let before = serde_json::to_value(&p).unwrap();

        let mut target = p.clone();
        target.gamma = 2.0;
        let snap_target = serde_json::to_value(&target).unwrap();

        let forward = Mutation::ApplyProjectSnapshot(ApplyProjectSnapshot {
            new: snap_target,
            old: before.clone(),
            non_undoable: false,
        });
        let reverse = forward.apply(&mut p);
        assert!(
            p.layers[0].effects.is_empty(),
            "after forward apply, layer 0 effects should be empty; got {:?}",
            p.layers[0].effects,
        );

        let _redo = reverse.apply(&mut p);
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(
            before, after,
            "ApplyProjectSnapshot apply→undo should be byte-equal even with empty effects vec",
        );
        assert!(
            p.layers[0].effects.is_empty(),
            "after undo, layer 0 effects should still be empty; got {:?}",
            p.layers[0].effects,
        );
    }

    /// Stale Reverse storage triggers `debug_assert!` in test
    /// builds. Confirms the runtime safety net works.
    #[test]
    #[should_panic(expected = "SetGamma stale Reverse")]
    fn stale_old_value_panics_in_debug_builds() {
        let mut p = fresh_project();
        // Mismatched: claim old gamma is 99.0 when it's actually 1.0.
        let stale = Mutation::SetGamma(SetGamma {
            new: 2.0,
            old: 99.0,
        });
        let _ = stale.apply(&mut p);
    }

    /// 003-T1.22 — canonical whole-enum Reverse smoke test.
    ///
    /// Constructs a fully-populated `Modulator::Sine`, flips it to
    /// `Modulator::Static(0.7)` via `SetModulator`, then undoes via the
    /// returned Reverse. Asserts every field of the original `Sine` is
    /// restored byte-equal — the property that `new: Modulator` and
    /// `old: Modulator` (whole-enum Reverse) are designed to guarantee.
    #[test]
    fn modulator_whole_enum_reverse_round_trips_sine_to_static() {
        use crate::modulators::Modulator;
        let mut p = fresh_project();
        // Position a Sine in the seeded layer's Color::hue slot.
        let sine = Modulator::Sine {
            period_s: 2.0,
            amp: 0.3,
            phase: 1.0,
            offset: 0.5,
        };
        if let crate::effects::Effect::Color { hue, .. } = &mut p.layers[0].effects[0] {
            *hue = sine.clone();
        } else {
            panic!("fresh_project layer 0 effect 0 should be Color");
        }
        let before = serde_json::to_value(&p).unwrap();

        let mutation =
            p.set_modulator_mutation(0, 0, ModulatorField::ColorHue, Modulator::Static(0.7));
        let reverse = mutation.apply(&mut p);
        // Sanity: hue is now Static(0.7).
        if let crate::effects::Effect::Color { hue, .. } = &p.layers[0].effects[0] {
            assert!(
                matches!(hue, Modulator::Static(v) if (v - 0.7).abs() < 1e-6),
                "after apply, hue should be Static(0.7)"
            );
        }
        let _ = reverse.apply(&mut p);
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(
            before, after,
            "Sine fields (period_s/amp/phase/offset) must restore byte-equal"
        );
    }

    /// 003-T1.26 — canonical effects-Vec Reverse smoke test.
    ///
    /// `mutate_transform_effect` (in `windows/scene_editor.rs`) appends a
    /// default `Effect::Transform` when the layer's chain doesn't already
    /// have one, then mutates it. A naive per-field Reverse would leave
    /// the appended Transform on undo. The Effects-Vec Reverse rule
    /// (rule 2) protects against this: the snapshot captured before any
    /// drag mutation is the entire `Vec<Effect>`, so undo restores the
    /// pre-drag length and contents byte-equal.
    ///
    /// This test sets up a layer with NO Transform effect, simulates the
    /// drag-stopped emission `SetLayerEffects { new: <chain with appended
    /// Transform>, old: <original chain> }`, applies the Reverse, and
    /// asserts the chain is back to its original length and contents.
    #[test]
    fn effects_vec_reverse_no_stray_transform_after_undo() {
        use crate::effects::Effect;
        use crate::modulators::Modulator;
        let mut p = fresh_project();
        // Strip the default Transform from the seeded layer so the drag
        // append-or-mutate path takes the *append* branch.
        p.layers[0]
            .effects
            .retain(|e| !matches!(e, Effect::Transform { .. }));
        let pre_drag_len = p.layers[0].effects.len();
        let pre_drag = p.layers[0].effects.clone();
        assert!(
            !pre_drag
                .iter()
                .any(|e| matches!(e, Effect::Transform { .. })),
            "fixture should not contain Transform pre-drag"
        );
        let before = serde_json::to_value(&p).unwrap();

        // Simulate what handle_scene_input does for an alt-drag rotate:
        // append a default Transform, set rotate_deg, ship as new.
        let mut new = pre_drag.clone();
        new.push(Effect::Transform {
            translate: [0.0, 0.0],
            rotate_deg: Modulator::Static(45.0),
            scale_x: Modulator::Static(1.0),
            scale_y: Modulator::Static(1.0),
        });
        let mutation = Mutation::SetLayerEffects(SetLayerEffects {
            layer_idx: 0,
            new,
            old: pre_drag,
        });

        let reverse = mutation.apply(&mut p);
        // Sanity: post-apply, the chain has one more effect (the Transform).
        assert_eq!(
            p.layers[0].effects.len(),
            pre_drag_len + 1,
            "after apply, chain should have the appended Transform"
        );

        let _ = reverse.apply(&mut p);
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(
            p.layers[0].effects.len(),
            pre_drag_len,
            "after undo, chain length must equal pre-drag length (no stray Transform)"
        );
        assert!(
            !p.layers[0]
                .effects
                .iter()
                .any(|e| matches!(e, Effect::Transform { .. })),
            "after undo, chain must contain no Transform effect"
        );
        assert_eq!(before, after, "byte-equal restoration of full project");
    }

    /// All three sliders are undoable.
    #[test]
    fn slider_mutations_are_undoable() {
        let p = fresh_project();
        assert!(!p.set_gamma_mutation(1.0).is_non_undoable());
        assert!(!p.set_brightness_mutation(0.0).is_non_undoable());
        assert!(!p.set_contrast_mutation(1.0).is_non_undoable());
    }

    /// 003-T1.30 — canonical project-snapshot Reverse smoke test.
    ///
    /// Save scene to slot 0, modify project (e.g. change gamma), recall slot 0
    /// via `ApplyProjectSnapshot { non_undoable: false }`, then undo via
    /// `UndoStack` — project must be byte-equal to the pre-recall state.
    /// Exercises Reverse rule 3 end-to-end through the `SetProjectScenes` /
    /// `ApplyProjectSnapshot` interaction.
    #[test]
    fn snapshot_reverse_smoke_save_modify_recall_undo() {
        use crate::project::undo::UndoStack;
        let mut p = fresh_project();
        let mut stack = UndoStack::new();

        // 1. Save current state to slot 0.
        let mut new_cues = p.cues.clone();
        new_cues.push(crate::project::schema::Cue::new(
            "scene1",
            crate::project::snapshot(&p),
            None,
        ));
        stack.push(p.set_project_scenes_mutation(new_cues), &mut p);

        // 2. Modify project (gamma).
        stack.push(p.set_gamma_mutation(2.5), &mut p);

        // 3. Capture pre-recall state.
        let pre_recall = serde_json::to_value(&p).unwrap();

        // 4. Recall slot 0 via ApplyProjectSnapshot { non_undoable: false }.
        let target = p.cues[0].snapshot.clone();
        let cur = serde_json::to_value(&p).unwrap();
        let recall = Mutation::ApplyProjectSnapshot(ApplyProjectSnapshot {
            new: target,
            old: cur,
            non_undoable: false,
        });
        stack.push(recall, &mut p);

        // 5. Undo the recall.
        let undid = stack.undo(&mut p);
        assert!(undid.is_some(), "undo of recall should succeed");

        // 6. Project must be byte-equal to pre-recall state.
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(
            pre_recall, after,
            "undo of recall must restore pre-recall state"
        );
    }

    /// 003-T1.30 — crossfade tick non_undoable invariant.
    ///
    /// Crossfade ticks fire ~60×/s; if they polluted the undo stack a 5-second
    /// crossfade would consume the entire 200-entry cap. Push N (60)
    /// `ApplyProjectSnapshot` mutations with `non_undoable: true` through
    /// `UndoStack::push` and assert `stack.len() == 0` — i.e. N pushes do NOT
    /// grow the undo stack.
    #[test]
    fn crossfade_undo_excluded_from_stack() {
        use crate::project::undo::UndoStack;
        let mut p = fresh_project();
        let mut stack = UndoStack::new();

        let snap = serde_json::to_value(&p).unwrap();
        for _ in 0..60 {
            let m = Mutation::ApplyProjectSnapshot(ApplyProjectSnapshot {
                new: snap.clone(),
                old: snap.clone(),
                non_undoable: true,
            });
            stack.push(m, &mut p);
        }

        assert_eq!(
            stack.len(),
            0,
            "non_undoable crossfade ticks must not enter the undo stack"
        );
    }

    /// 004-V31.1.3 — apply + undo round-trip for `SetOutputWindowed`.
    ///
    /// Bool has only two values so a proptest would be overkill; instead
    /// we exhaustively cover all four (start, target) combinations.
    /// Verifies that `apply` writes `target` and that the returned Reverse
    /// restores `start` exactly.
    #[test]
    fn output_windowed_apply_undo_round_trip() {
        for (start, target) in [(false, true), (true, false), (false, false), (true, true)] {
            let mut p = fresh_project();
            p.output_windowed = start;
            let m = p.set_output_windowed_mutation(target);
            let reverse = m.apply(&mut p);
            assert_eq!(
                p.output_windowed, target,
                "apply should write `target` for {start} → {target}"
            );
            reverse.apply(&mut p);
            assert_eq!(
                p.output_windowed, start,
                "undo failed for {start} → {target}"
            );
        }
    }

    // ── V31.6.1 ── mute / solo Mutation tests ────────────────────────────────

    /// V31.6.1 — `SetLayerMuted` apply + undo round-trips all four (start, target)
    /// bool combinations and leaves the project byte-equal after undo.
    #[test]
    fn set_layer_muted_apply_undo_round_trip() {
        for (start, target) in [(false, true), (true, false), (false, false), (true, true)] {
            let mut p = fresh_project();
            p.layers[0].muted = start;
            let before = serde_json::to_value(&p).unwrap();
            let m = p.set_layer_muted_mutation(0, target);
            let reverse = m.apply(&mut p);
            assert_eq!(
                p.layers[0].muted, target,
                "apply should write `target` for {start} → {target}"
            );
            reverse.apply(&mut p);
            assert_eq!(
                p.layers[0].muted, start,
                "undo should restore `start` for {start} → {target}"
            );
            let after = serde_json::to_value(&p).unwrap();
            assert_eq!(
                before, after,
                "byte-equal after undo for {start} → {target}"
            );
        }
    }

    /// V31.6.1 — `SetLayerSolo` apply + undo round-trips `None → Some`,
    /// `Some → None`, and `Some(a) → Some(b)`.
    #[test]
    fn set_layer_solo_apply_undo_round_trip() {
        // None → Some(0)
        {
            let mut p = fresh_project();
            assert!(p.solo.is_none());
            let before = serde_json::to_value(&p).unwrap();
            let m = p.set_solo_mutation(Some(0));
            let reverse = m.apply(&mut p);
            assert_eq!(p.solo, Some(0));
            reverse.apply(&mut p);
            assert_eq!(p.solo, None);
            let after = serde_json::to_value(&p).unwrap();
            assert_eq!(before, after, "None → Some(0) round-trip byte-equal");
        }
        // Some(0) → None
        {
            let mut p = fresh_project();
            p.solo = Some(0);
            let before = serde_json::to_value(&p).unwrap();
            let m = p.set_solo_mutation(None);
            let reverse = m.apply(&mut p);
            assert_eq!(p.solo, None);
            reverse.apply(&mut p);
            assert_eq!(p.solo, Some(0));
            let after = serde_json::to_value(&p).unwrap();
            assert_eq!(before, after, "Some(0) → None round-trip byte-equal");
        }
        // Some(0) → Some(0) (no-op) — still round-trips cleanly
        {
            let mut p = fresh_project();
            p.solo = Some(0);
            let m = p.set_solo_mutation(Some(0));
            let reverse = m.apply(&mut p);
            assert_eq!(p.solo, Some(0));
            reverse.apply(&mut p);
            assert_eq!(p.solo, Some(0));
        }
    }

    /// V31.6.1 — stale Reverse for `SetLayerMuted` panics in debug builds.
    #[test]
    #[should_panic(expected = "SetLayerMuted stale Reverse")]
    fn stale_set_layer_muted_panics_in_debug_builds() {
        let mut p = fresh_project();
        p.layers[0].muted = false;
        // Claim old=true when it's actually false.
        let stale = Mutation::SetLayerMuted(SetLayerMuted {
            layer_idx: 0,
            new: true,
            old: true, // stale!
        });
        let _ = stale.apply(&mut p);
    }

    /// V31.6.1 — stale Reverse for `SetLayerSolo` panics in debug builds.
    #[test]
    #[should_panic(expected = "SetLayerSolo stale Reverse")]
    fn stale_set_layer_solo_panics_in_debug_builds() {
        let mut p = fresh_project();
        assert!(p.solo.is_none());
        // Claim old=Some(99) when it's actually None.
        let stale = Mutation::SetLayerSolo(SetLayerSolo {
            new: Some(0),
            old: Some(99), // stale!
        });
        let _ = stale.apply(&mut p);
    }

    /// V31.6.1 — mute and solo mutations are undoable.
    #[test]
    fn mute_solo_mutations_are_undoable() {
        let p = fresh_project();
        assert!(!p.set_layer_muted_mutation(0, true).is_non_undoable());
        assert!(!p.set_solo_mutation(Some(0)).is_non_undoable());
        assert!(!p.set_solo_mutation(None).is_non_undoable());
    }

    // ── V31.7.2 — quantize bars Mutation tests ────────────────────────────────

    /// V31.7.2 — `SetQuantizeBars` apply + undo round-trips `None → Some`,
    /// `Some → None`, and `Some(a) → Some(b)`.
    #[test]
    fn set_quantize_bars_apply_undo_round_trip() {
        // None → Some(4)
        {
            let mut p = fresh_project();
            assert!(p.quantize_bars.is_none());
            let before = serde_json::to_value(&p).unwrap();
            let m = p.set_quantize_bars_mutation(Some(4));
            let reverse = m.apply(&mut p);
            assert_eq!(p.quantize_bars, Some(4), "apply should write Some(4)");
            reverse.apply(&mut p);
            assert_eq!(p.quantize_bars, None, "undo should restore None");
            let after = serde_json::to_value(&p).unwrap();
            assert_eq!(before, after, "None → Some(4) round-trip byte-equal");
        }
        // Some(2) → Some(8)
        {
            let mut p = fresh_project();
            p.quantize_bars = Some(2);
            let before = serde_json::to_value(&p).unwrap();
            let m = p.set_quantize_bars_mutation(Some(8));
            let reverse = m.apply(&mut p);
            assert_eq!(p.quantize_bars, Some(8), "apply should write Some(8)");
            reverse.apply(&mut p);
            assert_eq!(p.quantize_bars, Some(2), "undo should restore Some(2)");
            let after = serde_json::to_value(&p).unwrap();
            assert_eq!(before, after, "Some(2) → Some(8) round-trip byte-equal");
        }
        // Some(8) → None
        {
            let mut p = fresh_project();
            p.quantize_bars = Some(8);
            let before = serde_json::to_value(&p).unwrap();
            let m = p.set_quantize_bars_mutation(None);
            let reverse = m.apply(&mut p);
            assert_eq!(p.quantize_bars, None, "apply should write None");
            reverse.apply(&mut p);
            assert_eq!(p.quantize_bars, Some(8), "undo should restore Some(8)");
            let after = serde_json::to_value(&p).unwrap();
            assert_eq!(before, after, "Some(8) → None round-trip byte-equal");
        }
    }

    /// V31.7.2 — stale Reverse for `SetQuantizeBars` panics in debug builds.
    #[test]
    #[should_panic(expected = "SetQuantizeBars stale Reverse")]
    fn stale_set_quantize_bars_panics_in_debug_builds() {
        let mut p = fresh_project();
        assert!(p.quantize_bars.is_none());
        // Claim old=Some(4) when it's actually None.
        let stale = Mutation::SetQuantizeBars(SetQuantizeBars {
            new: Some(2),
            old: Some(4), // stale!
        });
        let _ = stale.apply(&mut p);
    }

    /// V31.7.2 — quantize mutation is undoable.
    #[test]
    fn quantize_bars_mutation_is_undoable() {
        let p = fresh_project();
        assert!(!p.set_quantize_bars_mutation(Some(4)).is_non_undoable());
        assert!(!p.set_quantize_bars_mutation(None).is_non_undoable());
    }

    /// V31.7.2 — constructor captures the live `quantize_bars` value as `old`.
    #[test]
    fn set_quantize_bars_mutation_captures_old() {
        let mut p = fresh_project();
        p.quantize_bars = Some(2);
        let m = p.set_quantize_bars_mutation(Some(4));
        if let Mutation::SetQuantizeBars(payload) = m {
            assert_eq!(
                payload.old,
                Some(2),
                "constructor should capture the current quantize_bars as old"
            );
            assert_eq!(payload.new, Some(4));
        } else {
            panic!("expected Mutation::SetQuantizeBars");
        }
    }

    /// V31.6.1 — render-graph visibility rule unit test.
    ///
    /// Three layers:
    ///   0: unmuted,  1: muted,  2: unmuted
    ///
    /// Case A: no solo → 0 and 2 visible, 1 hidden.
    /// Case B: solo=Some(0) → only 0 visible.
    /// Case C: solo=Some(0), layers[0].muted=true → 0 still visible (soloed-and-muted edge case).
    #[test]
    fn render_visibility_rule() {
        use std::path::PathBuf;
        let mut p = Project::default();
        for i in 0..3 {
            p.layers.push(crate::project::schema::layer_from_svg_path(
                format!("l{i}"),
                PathBuf::from(format!("/tmp/l{i}.svg")),
            ));
        }
        // Case A: no solo, layer 1 muted.
        p.layers[1].muted = true;
        assert!(p.layer_is_visible(0), "A: layer 0 should be visible");
        assert!(
            !p.layer_is_visible(1),
            "A: layer 1 (muted) should be hidden"
        );
        assert!(p.layer_is_visible(2), "A: layer 2 should be visible");

        // Case B: solo=Some(0) — only layer 0 visible.
        p.solo = Some(0);
        assert!(p.layer_is_visible(0), "B: soloed layer 0 should be visible");
        assert!(!p.layer_is_visible(1), "B: layer 1 hidden by solo");
        assert!(!p.layer_is_visible(2), "B: layer 2 hidden by solo");

        // Case C: solo=Some(0) AND layers[0].muted=true — solo wins, layer 0 still visible.
        p.layers[0].muted = true;
        assert!(
            p.layer_is_visible(0),
            "C: soloed-and-muted layer should still be visible (solo takes precedence)"
        );
    }

    // ── SetVideoSpeed (P0.4.3) + SetVideoLoopMode (P1.4.2) unit tests ─────────────

    /// Helper: produce a project with one Video layer at index 0.
    fn fresh_video_project() -> crate::project::schema::Project {
        let mut p = Project::default();
        p.layers.clear();
        p.layers.push(crate::project::schema::layer_from_video_path(
            "test_video",
            std::path::PathBuf::from("/tmp/rmap_test.mp4"),
        ));
        p
    }

    /// P0.4.3 — `SetVideoSpeed` apply + undo round-trips across a set of
    /// speed transitions that cover the full 0.25..=4.0 slider range.
    #[test]
    fn set_video_speed_apply_undo_round_trip() {
        for (start, target) in [
            (1.0_f32, 2.0_f32),
            (2.0, 0.5),
            (0.25, 4.0),
            (1.0, 1.0), // no-op should still round-trip cleanly
        ] {
            let mut p = fresh_video_project();
            match &mut p.layers[0].kind {
                crate::project::schema::LayerKind::Video { speed, .. } => *speed = start,
                _ => unreachable!(),
            }
            let before = serde_json::to_value(&p).unwrap();
            let m = p.set_video_speed_mutation(0, target);
            let reverse = m.apply(&mut p);
            let got_speed = match &p.layers[0].kind {
                crate::project::schema::LayerKind::Video { speed, .. } => *speed,
                _ => unreachable!(),
            };
            assert!(
                (got_speed - target).abs() < 1e-6,
                "apply should write `target`={target}, got {got_speed} ({start} → {target})"
            );
            reverse.apply(&mut p);
            let after = serde_json::to_value(&p).unwrap();
            assert_eq!(before, after, "byte-equal after undo ({start} → {target})");
        }
    }

    /// P1.4.2 — `SetVideoLoopMode` apply + undo round-trips across the
    /// three enum variants.
    #[test]
    fn set_video_loop_mode_apply_undo_round_trip() {
        use crate::project::schema::LoopMode;
        let combos = [
            (LoopMode::Once, LoopMode::Loop),
            (LoopMode::Loop, LoopMode::Once),
            (LoopMode::Loop, LoopMode::PingPong),
            (LoopMode::PingPong, LoopMode::Loop),
            (LoopMode::Once, LoopMode::Once),
        ];
        for (start, target) in combos {
            let mut p = fresh_video_project();
            match &mut p.layers[0].kind {
                crate::project::schema::LayerKind::Video { loop_mode, .. } => {
                    *loop_mode = start;
                }
                _ => unreachable!(),
            }
            let before = serde_json::to_value(&p).unwrap();
            let m = p.set_video_loop_mode_mutation(0, target);
            let reverse = m.apply(&mut p);
            let got = match &p.layers[0].kind {
                crate::project::schema::LayerKind::Video { loop_mode, .. } => *loop_mode,
                _ => unreachable!(),
            };
            assert_eq!(
                got, target,
                "apply should write `target`={target:?} ({start:?} → {target:?})"
            );
            reverse.apply(&mut p);
            let after = serde_json::to_value(&p).unwrap();
            assert_eq!(
                before, after,
                "byte-equal after undo ({start:?} → {target:?})"
            );
        }
    }

    /// P0.4.3 — `SetVideoSpeed` mutation is undoable (not in the
    /// `is_non_undoable` branch).
    #[test]
    fn set_video_speed_is_undoable() {
        let p = fresh_video_project();
        assert!(
            !p.set_video_speed_mutation(0, 2.0).is_non_undoable(),
            "SetVideoSpeed should be undoable"
        );
    }

    /// P1.4.2 — `SetVideoLoopMode` mutation is undoable.
    #[test]
    fn set_video_loop_mode_is_undoable() {
        let p = fresh_video_project();
        assert!(
            !p.set_video_loop_mode_mutation(0, crate::project::schema::LoopMode::Once)
                .is_non_undoable(),
            "SetVideoLoopMode should be undoable"
        );
    }

    /// P0.4.3 — builder panics when the layer is not `LayerKind::Video`.
    #[test]
    #[should_panic(expected = "is not a Video layer")]
    fn set_video_speed_builder_panics_on_non_video_layer() {
        let p = fresh_project(); // SVG layer at index 0
        let _ = p.set_video_speed_mutation(0, 2.0);
    }

    /// P1.4.2 — builder panics when the layer is not `LayerKind::Video`.
    #[test]
    #[should_panic(expected = "is not a Video layer")]
    fn set_video_loop_mode_builder_panics_on_non_video_layer() {
        let p = fresh_project(); // SVG layer at index 0
        let _ = p.set_video_loop_mode_mutation(0, crate::project::schema::LoopMode::Once);
    }

    /// P0.4.3 — via undo stack: push a `SetVideoSpeed` mutation and undo
    /// it, confirming the stack correctly restores the prior speed.
    #[test]
    fn set_video_speed_undo_via_stack() {
        use crate::project::undo::UndoStack;
        let mut p = fresh_video_project();
        match &mut p.layers[0].kind {
            crate::project::schema::LayerKind::Video { speed, .. } => *speed = 1.0,
            _ => unreachable!(),
        }
        let before = serde_json::to_value(&p).unwrap();
        let mut stack = UndoStack::new();
        stack.push(p.set_video_speed_mutation(0, 3.0), &mut p);
        let after_apply = match &p.layers[0].kind {
            crate::project::schema::LayerKind::Video { speed, .. } => *speed,
            _ => unreachable!(),
        };
        assert!(
            (after_apply - 3.0).abs() < 1e-6,
            "speed should be 3.0 after apply"
        );
        stack.undo(&mut p);
        let after_undo = serde_json::to_value(&p).unwrap();
        assert_eq!(
            before, after_undo,
            "undo via stack should restore project byte-equally"
        );
    }

    /// P1.4.2 — via undo stack: push a `SetVideoLoopMode` mutation and undo it.
    #[test]
    fn set_video_loop_mode_undo_via_stack() {
        use crate::project::schema::LoopMode;
        use crate::project::undo::UndoStack;
        let mut p = fresh_video_project();
        match &mut p.layers[0].kind {
            crate::project::schema::LayerKind::Video { loop_mode, .. } => {
                *loop_mode = LoopMode::Loop;
            }
            _ => unreachable!(),
        }
        let before = serde_json::to_value(&p).unwrap();
        let mut stack = UndoStack::new();
        stack.push(p.set_video_loop_mode_mutation(0, LoopMode::Once), &mut p);
        let after_apply = match &p.layers[0].kind {
            crate::project::schema::LayerKind::Video { loop_mode, .. } => *loop_mode,
            _ => unreachable!(),
        };
        assert_eq!(
            after_apply,
            LoopMode::Once,
            "loop_mode should be Once after apply"
        );
        stack.undo(&mut p);
        let after_undo = serde_json::to_value(&p).unwrap();
        assert_eq!(
            before, after_undo,
            "undo via stack should restore project byte-equally"
        );
    }

    /// 003-T1.17 — property-based test for the Reverse-storage
    /// invariant (Risk R11 mitigation).
    ///
    /// For any sequence of `Mutation` applications, undoing them
    /// all must return the project to byte-equal serde_json. This
    /// is the runtime contract that lets future contributors
    /// migrate UI sites (T-003-T1.18+) without manually verifying
    /// Reverse correctness.
    ///
    /// Adding a new Mutation variant: extend `MutationKind` and
    /// the `to_mutation` match. The harness picks it up
    /// automatically.
    mod proptest_round_trip {
        use super::*;
        use crate::project::schema::BlendMode;
        use crate::project::undo::UndoStack;
        use proptest::prelude::*;

        /// Categories of mutation the harness generates.
        #[derive(Clone, Debug)]
        enum MutationKind {
            Gamma(f32),
            Brightness(f32),
            Contrast(f32),
            CrossfadeDurationS(f32),
            OutputWindowed(bool),
            WarpMaskFeather(f32),
            WarpDimensions {
                rows: u32,
                cols: u32,
            },
            Snapshot,
            LayerOpacity(f32),
            LayerEnabled(bool),
            LayerBlendMode(BlendMode),
            AddLayer,
            RemoveLayer,
            SwapLayers,
            LayerEffectsTransformTranslate {
                x: f32,
                y: f32,
            },
            /// 003-T1.22 — SetModulator round-trip coverage.
            SetModulator {
                effect_idx: usize,
                field: ModulatorField,
                new: crate::modulators::Modulator,
            },
            /// 003-T1.24 — drag-translate path: mirrors what
            /// `mutate_transform_effect` does (append-then-mutate) and
            /// emits a `SetLayerEffects` exactly as `handle_scene_input`
            /// does at drag_stopped.
            LayerEffectsDragTranslate {
                dx: f32,
                dy: f32,
            },
            /// 003-T1.26 — drag-rotate path: same append-or-mutate pattern
            /// as drag-translate but writes `Transform.rotate_deg`. Covers
            /// the canonical effects-Vec Reverse case proptest-side.
            LayerEffectsDragRotate {
                degrees: f32,
            },
            /// 003-T1.27 — set a single mask polygon vertex.
            SetMaskVertex {
                idx_pick: u8,
                x: f32,
                y: f32,
            },
            /// 003-T1.27 — insert a new mask polygon vertex.
            AddMaskVertex {
                position_pick: u8,
                x: f32,
                y: f32,
            },
            /// 003-T1.27 — remove a mask polygon vertex (only when len > 3).
            RemoveMaskVertex {
                idx_pick: u8,
            },
            /// 003-T1.28 — reset the warp mesh to a synthetic identity mesh.
            ResetWarpMesh {
                rows: u32,
                cols: u32,
            },
            /// 003-T1.28 — replace the entire mask polygon (covers both
            /// zone-template apply and clear-mask).
            SetMaskPolygon {
                vertices: Vec<[f32; 2]>,
            },
            /// 003-T1.39 — set output monitor index.
            OutputMonitorIndex(usize),
            /// T3.0c — set a single warp grid corner.
            LayerWarpCorner {
                r_pick: u32,
                c_pick: u32,
                x: f32,
                y: f32,
            },
            /// T3.28 — set per-display gamma override (`None` to clear).
            ProjectGammaOverride(Option<f32>),
            /// T3.28 — set per-display brightness override.
            ProjectBrightnessOverride(Option<f32>),
            /// T3.28 — set per-display contrast override.
            ProjectContrastOverride(Option<f32>),
            /// V31.6.1 — toggle a layer's muted flag.
            LayerMuted(bool),
            /// V31.6.1 — set the project-level solo index (`None` to clear).
            LayerSolo(Option<usize>),
            /// V31.7.2 — set the quantize-bars field (`None` = off; `Some(1/2/4/8)` = quantized).
            QuantizeBars(Option<u8>),
            /// P0.7.3 — set the edge-blend config (`None` = off, `Some(cfg)` = enabled).
            EdgeBlend(Option<(u32, bool)>),
            /// P0.8.1 — set the RGB matrix for an output target. `output_idx_pick`
            /// is taken mod `output_targets.len()` so the harness doesn't need to
            /// know the project's target count; `matrix` is the new 3×3 value.
            SetOutputRgbMatrix {
                output_idx_pick: u8,
                matrix: [[f32; 3]; 3],
            },
            /// P0.4.3 — set the playback speed of a Video layer (0.25..=4.0).
            /// Falls back to a no-op when no Video layer exists in the fixture.
            VideoSpeed(f32),
            /// P1.4.2 — set the loop mode of a Video layer (Once / Loop /
            /// PingPong). Falls back to a no-op when no Video layer exists.
            VideoLoopMode(crate::project::schema::LoopMode),
            /// P1.4.1 — set the (clip_in, clip_out) pair on a Video layer.
            /// Falls back to a no-op when no Video layer exists.
            VideoClipRange {
                clip_in: f32,
                clip_out: f32,
            },
            /// P1.4.4 — toggle BPM-lock on a Video layer. Falls back
            /// to a no-op when no Video layer exists.
            VideoBpmLock(bool),
            /// P1.2.4 — set the focal point on an Image / Video layer.
            /// Falls back to a no-op when neither variant is present
            /// in the project.
            LayerFocal([f32; 2]),
            /// P1.2.1 — set / clear a layer's treatment. `None` clears;
            /// `Some(preset_pick)` selects one of two preset_ids (toggle).
            /// Always targets layer index 0 (fresh_project has one layer).
            SetLayerTreatment(Option<bool>),
            /// P1.2.1 — set the treatment params HashMap. The strategy
            /// generates one or two key/value pairs; `to_mutation` falls
            /// back to a no-op if the layer's treatment is None (the
            /// builder panics otherwise — we don't want spurious test
            /// failures when the preceding step cleared the treatment).
            SetLayerTreatmentParams {
                exposure: f32,
                contrast: f32,
            },
            /// P2.9.1 — set a `RIPPLE_WASH_PRESET_ID` FxLayer's params HashMap.
            /// Targets layer 1 (the FxLayer appended by `fresh_project`),
            /// overwriting `wavelength` with a value in `[10.0, 400.0]` —
            /// the descriptor's full range, with no `max_particle_count` cap
            /// so every generated value commits. Falls back to a no-op gamma
            /// when layer 1 is absent or is not an FxLayer (possible after
            /// `RemoveLayer` steps reduce the project to 1 layer).
            SetFxLayerParams {
                wavelength: f32,
            },
            /// P3.6.1 — set the zone role of layer 0's warp. `None` clears the
            /// role; `Some(role_idx)` picks one of the seven roles by index
            /// modulo 7. Always targets layer 0 (always present in the fixture).
            SetMaskZoneRole(Option<u8>),
            /// P7.3.1 — reset the bezier mesh to a synthetic identity mesh
            /// (Some) or clear it (None). Targets layer 0 (always present).
            ResetBezierMesh {
                rows: u32,
                cols: u32,
                /// `true` = install `Some(BezierMesh)`, `false` = install `None`.
                some: bool,
            },
            /// P7.3.3 — move anchor (0,0) of layer 0's bezier mesh.
            /// Precondition: always first sets a 2×2 identity mesh so the anchor exists.
            MoveBezierAnchor {
                new_x: f32,
                new_y: f32,
            },
            /// P7.3.3 — set handle (horizontal or vertical) at anchor (0,0) of
            /// layer 0's bezier mesh. `None` clears it.
            SetBezierHandle {
                dir_h: bool,
                pos: Option<[f32; 2]>,
            },
            /// P7.5.1/P7.6.1 — set the MaskGraph to an identity mask (Some)
            /// or clear it (None). Targets layer 0 (always present).
            SetMaskGraph(bool),
            /// 004-T1.13 — replace both the effect chain and mask polygon in
            /// one step. `new_mask_polygon` is the polygon to install; effect
            /// chain is always the default chain (keeps the harness tractable).
            SetLayerEffectsAndMask {
                new_mask_polygon: Vec<[f32; 2]>,
            },
        }

        fn to_mutation(kind: &MutationKind, project: &Project) -> Mutation {
            match kind {
                MutationKind::Gamma(v) => project.set_gamma_mutation(*v),
                MutationKind::Brightness(v) => project.set_brightness_mutation(*v),
                MutationKind::Contrast(v) => project.set_contrast_mutation(*v),
                MutationKind::CrossfadeDurationS(v) => {
                    project.set_crossfade_duration_s_mutation(*v)
                }
                MutationKind::OutputWindowed(v) => project.set_output_windowed_mutation(*v),
                MutationKind::WarpMaskFeather(v) => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        project.set_layer_mask_feather_mutation(0, *v)
                    }
                }
                MutationKind::WarpDimensions { rows, cols } => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        project.set_layer_warp_dimensions_mutation(0, *rows, *cols)
                    }
                }
                MutationKind::Snapshot => {
                    // Build a snapshot mutation against the
                    // project's current state, with `new` flipping
                    // gamma so the mutation is observable.
                    let old = serde_json::to_value(project).unwrap();
                    let mut next = project.clone();
                    next.gamma += 0.1;
                    let new = serde_json::to_value(&next).unwrap();
                    Mutation::ApplyProjectSnapshot(ApplyProjectSnapshot {
                        new,
                        old,
                        non_undoable: false,
                    })
                }
                MutationKind::LayerOpacity(v) => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        project.set_layer_opacity_mutation(0, *v)
                    }
                }
                MutationKind::LayerEnabled(v) => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        project.set_layer_enabled_mutation(0, *v)
                    }
                }
                MutationKind::LayerBlendMode(v) => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        project.set_layer_blend_mode_mutation(0, *v)
                    }
                }
                MutationKind::AddLayer => {
                    use std::path::PathBuf;
                    let id = format!("test_layer_{}", project.layers.len());
                    let layer = crate::project::schema::layer_from_svg_path(
                        id,
                        PathBuf::from("/tmp/rmap_test.svg"),
                    );
                    project.set_add_layer_mutation(layer, project.layers.len())
                }
                MutationKind::RemoveLayer => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        project.set_remove_layer_mutation(0)
                    }
                }
                MutationKind::SwapLayers => {
                    if project.layers.len() < 2 {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        project.set_swap_layers_mutation(0, 1)
                    }
                }
                MutationKind::LayerEffectsTransformTranslate { x, y } => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        let mut new = project.layers[0].effects.clone();
                        for effect in new.iter_mut() {
                            if let crate::effects::Effect::Transform { translate, .. } = effect {
                                translate[0] = *x;
                                translate[1] = *y;
                                break;
                            }
                        }
                        project.set_layer_effects_mutation(0, new)
                    }
                }
                MutationKind::SetModulator {
                    effect_idx,
                    field,
                    new,
                } => {
                    if project.layers.is_empty() || project.layers[0].effects.len() <= *effect_idx {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        project.set_modulator_mutation(0, *effect_idx, *field, new.clone())
                    }
                }
                MutationKind::LayerEffectsDragTranslate { dx, dy } => {
                    // Mirror the append-then-mutate pattern of
                    // `mutate_transform_effect` + the drag-stopped emit in
                    // `handle_scene_input` (T-003-T1.24).
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        let old = project.layers[0].effects.clone();
                        let mut new = old.clone();
                        // Append a default Transform if the chain lacks one —
                        // the same thing `mutate_transform_effect` does.
                        if !new
                            .iter()
                            .any(|e| matches!(e, crate::effects::Effect::Transform { .. }))
                        {
                            new.push(crate::effects::Effect::Transform {
                                translate: [0.0, 0.0],
                                rotate_deg: crate::modulators::Modulator::Static(0.0),
                                scale_x: crate::modulators::Modulator::Static(1.0),
                                scale_y: crate::modulators::Modulator::Static(1.0),
                            });
                        }
                        for e in new.iter_mut() {
                            if let crate::effects::Effect::Transform { translate, .. } = e {
                                translate[0] += dx;
                                translate[1] += dy;
                                break;
                            }
                        }
                        Mutation::SetLayerEffects(SetLayerEffects {
                            layer_idx: 0,
                            new,
                            old,
                        })
                    }
                }
                MutationKind::LayerEffectsDragRotate { degrees } => {
                    // Same append-or-mutate pattern as drag-translate but
                    // writes Transform.rotate_deg as Modulator::Static
                    // (T-003-T1.26).
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        let old = project.layers[0].effects.clone();
                        let mut new = old.clone();
                        if !new
                            .iter()
                            .any(|e| matches!(e, crate::effects::Effect::Transform { .. }))
                        {
                            new.push(crate::effects::Effect::Transform {
                                translate: [0.0, 0.0],
                                rotate_deg: crate::modulators::Modulator::Static(0.0),
                                scale_x: crate::modulators::Modulator::Static(1.0),
                                scale_y: crate::modulators::Modulator::Static(1.0),
                            });
                        }
                        for e in new.iter_mut() {
                            if let crate::effects::Effect::Transform { rotate_deg, .. } = e {
                                *rotate_deg = crate::modulators::Modulator::Static(*degrees);
                                break;
                            }
                        }
                        Mutation::SetLayerEffects(SetLayerEffects {
                            layer_idx: 0,
                            new,
                            old,
                        })
                    }
                }
                // 003-T1.27 — mask vertex mutations.
                MutationKind::SetMaskVertex { idx_pick, x, y } => {
                    if project.layers.is_empty() || project.layers[0].warp.mask_polygon.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        let len = project.layers[0].warp.mask_polygon.len();
                        let idx = (*idx_pick as usize) % len;
                        project.set_layer_mask_vertex_mutation(
                            0,
                            idx,
                            [x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)],
                        )
                    }
                }
                MutationKind::AddMaskVertex {
                    position_pick,
                    x,
                    y,
                } => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        let len = project.layers[0].warp.mask_polygon.len();
                        let position = (*position_pick as usize) % (len + 1);
                        project.set_add_layer_mask_vertex_mutation(
                            0,
                            position,
                            [x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)],
                        )
                    }
                }
                MutationKind::RemoveMaskVertex { idx_pick } => {
                    if project.layers.is_empty() || project.layers[0].warp.mask_polygon.len() <= 3 {
                        project.set_gamma_mutation(project.gamma) // preserve ≥3 invariant
                    } else {
                        let len = project.layers[0].warp.mask_polygon.len();
                        let idx = (*idx_pick as usize) % len;
                        project.set_remove_layer_mask_vertex_mutation(0, idx)
                    }
                }
                // 003-T1.28 — reset warp mesh and set mask polygon.
                MutationKind::ResetWarpMesh { rows, cols } => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        // Build a valid (rows+1)×(cols+1) vertex grid for the
                        // given cell counts (same convention as default_warp_mesh).
                        let mut new_mesh = project.layers[0].warp.clone();
                        new_mesh.rows = *rows;
                        new_mesh.cols = *cols;
                        new_mesh.grid = (0..=*rows as usize)
                            .map(|r| {
                                (0..=*cols as usize)
                                    .map(|c| {
                                        let u = if *cols == 0 {
                                            0.0
                                        } else {
                                            c as f32 / *cols as f32
                                        };
                                        let v = if *rows == 0 {
                                            0.0
                                        } else {
                                            r as f32 / *rows as f32
                                        };
                                        [u, v]
                                    })
                                    .collect()
                            })
                            .collect();
                        project.set_reset_layer_warp_mesh_mutation(0, new_mesh)
                    }
                }
                MutationKind::SetMaskPolygon { vertices } => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        project.set_layer_mask_polygon_mutation(0, vertices.clone())
                    }
                }
                // 003-T1.39 — output monitor index coverage.
                MutationKind::OutputMonitorIndex(v) => {
                    project.set_output_monitor_index_mutation(*v)
                }
                // T3.0c — warp corner pin.
                MutationKind::LayerWarpCorner {
                    r_pick,
                    c_pick,
                    x,
                    y,
                } => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        let warp = &project.layers[0].warp;
                        if warp.grid.is_empty() || warp.grid[0].is_empty() {
                            project.set_gamma_mutation(project.gamma) // degenerate grid fallback
                        } else {
                            let r = (*r_pick as usize) % warp.grid.len();
                            let c = (*c_pick as usize) % warp.grid[0].len();
                            project.set_layer_warp_corner_mutation(0, r, c, [*x, *y])
                        }
                    }
                }
                // T3.28 — per-display tone overrides.
                MutationKind::ProjectGammaOverride(v) => {
                    project.set_project_gamma_override_mutation(*v)
                }
                MutationKind::ProjectBrightnessOverride(v) => {
                    project.set_project_brightness_override_mutation(*v)
                }
                MutationKind::ProjectContrastOverride(v) => {
                    project.set_project_contrast_override_mutation(*v)
                }
                // V31.6.1 — mute / solo coverage.
                MutationKind::LayerMuted(v) => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        project.set_layer_muted_mutation(0, *v)
                    }
                }
                MutationKind::LayerSolo(v) => {
                    // Clamp solo index to a valid layer index (or None).
                    let clamped = v.and_then(|idx| {
                        if idx < project.layers.len() {
                            Some(idx)
                        } else if !project.layers.is_empty() {
                            Some(0)
                        } else {
                            None
                        }
                    });
                    project.set_solo_mutation(clamped)
                }
                // V31.7.2 — quantize bars coverage.
                MutationKind::QuantizeBars(v) => project.set_quantize_bars_mutation(*v),
                // P0.7.3 — edge-blend; `bool` picks Linear (false) vs Cosine (true).
                MutationKind::EdgeBlend(v) => {
                    let new =
                        v.map(
                            |(overlap_px, cosine)| crate::project::schema::EdgeBlendConfig {
                                overlap_px,
                                falloff_curve: if cosine {
                                    crate::project::schema::FalloffCurve::Cosine
                                } else {
                                    crate::project::schema::FalloffCurve::Linear
                                },
                            },
                        );
                    project.set_edge_blend_mutation(new)
                }
                // P0.8.1 — per-output RGB matrix; `output_idx_pick` is modded by
                // `output_targets.len()` so the harness generates a valid index
                // without needing to know the project fixture's target count.
                MutationKind::SetOutputRgbMatrix {
                    output_idx_pick,
                    matrix,
                } => {
                    let n = project.output_targets.len();
                    let idx = if n == 0 {
                        return project.set_gamma_mutation(project.gamma); // degenerate
                    } else {
                        (*output_idx_pick as usize) % n
                    };
                    project.set_output_rgb_matrix_mutation(idx, *matrix)
                }
                // P0.4.3 — video speed / loop. Fall back to a no-op when
                // the project fixture contains no Video layers.
                MutationKind::VideoSpeed(v) => {
                    let idx = project.layers.iter().position(|l| {
                        matches!(l.kind, crate::project::schema::LayerKind::Video { .. })
                    });
                    match idx {
                        Some(i) => project.set_video_speed_mutation(i, *v),
                        None => project.set_gamma_mutation(project.gamma), // no-op fallback
                    }
                }
                MutationKind::VideoLoopMode(v) => {
                    let idx = project.layers.iter().position(|l| {
                        matches!(l.kind, crate::project::schema::LayerKind::Video { .. })
                    });
                    match idx {
                        Some(i) => project.set_video_loop_mode_mutation(i, *v),
                        None => project.set_gamma_mutation(project.gamma), // no-op fallback
                    }
                }
                MutationKind::VideoClipRange { clip_in, clip_out } => {
                    let idx = project.layers.iter().position(|l| {
                        matches!(l.kind, crate::project::schema::LayerKind::Video { .. })
                    });
                    match idx {
                        Some(i) => project.set_video_clip_range_mutation(i, *clip_in, *clip_out),
                        None => project.set_gamma_mutation(project.gamma),
                    }
                }
                MutationKind::VideoBpmLock(v) => {
                    let idx = project.layers.iter().position(|l| {
                        matches!(l.kind, crate::project::schema::LayerKind::Video { .. })
                    });
                    match idx {
                        Some(i) => project.set_video_bpm_lock_mutation(i, *v),
                        None => project.set_gamma_mutation(project.gamma),
                    }
                }
                MutationKind::LayerFocal(new) => {
                    let idx = project.layers.iter().position(|l| {
                        matches!(
                            l.kind,
                            crate::project::schema::LayerKind::Image { .. }
                                | crate::project::schema::LayerKind::Video { .. }
                        )
                    });
                    match idx {
                        Some(i) => project.set_layer_focal_mutation(i, *new),
                        None => project.set_gamma_mutation(project.gamma),
                    }
                }
                // P1.2.1 — Treatment mutations target layer 0
                // (fresh_project has exactly one layer). The harness
                // exercises None ↔ Some toggles + the whole-HashMap
                // params replacement.
                MutationKind::SetLayerTreatment(preset_pick) => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op
                    } else {
                        let new =
                            preset_pick.map(|use_tone_map| crate::project::schema::Treatment {
                                preset_id: if use_tone_map {
                                    "tone_map".into()
                                } else {
                                    "blur_mask".into()
                                },
                                params: std::collections::HashMap::new(),
                                overlay_path: None,
                                collage_paths: vec![],
                            });
                        project.set_layer_treatment_mutation(0, new)
                    }
                }
                MutationKind::SetLayerTreatmentParams { exposure, contrast } => {
                    if project.layers.is_empty() || project.layers[0].treatment.is_none() {
                        // The builder panics on no-treatment; emit a
                        // no-op gamma instead. The harness will reach
                        // this branch after a preceding step cleared
                        // the treatment, which is a valid sequence.
                        project.set_gamma_mutation(project.gamma)
                    } else {
                        let mut new = std::collections::HashMap::new();
                        new.insert("exposure".to_string(), *exposure);
                        new.insert("contrast".to_string(), *contrast);
                        project.set_layer_treatment_params_mutation(0, new)
                    }
                }
                MutationKind::SetFxLayerParams { wavelength } => {
                    // P2.9.1 — target layer 1 (the RIPPLE_WASH FxLayer
                    // appended by fresh_project). Guard: the layer must
                    // exist AND still be FxLayer (SwapLayers / RemoveLayer
                    // steps in the same proptest sequence can change the
                    // topology). Fall back to a no-op gamma when absent.
                    if project.layers.len() > 1
                        && matches!(
                            project.layers[1].kind,
                            crate::project::schema::LayerKind::FxLayer { .. }
                        )
                    {
                        let mut new = std::collections::HashMap::new();
                        new.insert("wavelength".to_string(), *wavelength);
                        project.set_fx_layer_params_mutation(1, new)
                    } else {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    }
                }
                MutationKind::SetMaskZoneRole(role_opt) => {
                    // P3.6.1 — target layer 0 (always present in fresh_project).
                    // `None` clears the role; `Some(idx)` picks one of the
                    // seven ZoneRole variants by index mod 7. Falls back to a
                    // no-op gamma if the project has no layers (shouldn't happen
                    // in fresh_project, but guards against RemoveLayer steps).
                    use crate::project::schema::ZoneRole;
                    if project.layers.is_empty() {
                        return project.set_gamma_mutation(project.gamma);
                    }
                    let new_role = role_opt.map(|idx| match idx % 7 {
                        0 => ZoneRole::Window,
                        1 => ZoneRole::Portal,
                        2 => ZoneRole::Void,
                        3 => ZoneRole::Spill,
                        4 => ZoneRole::Edge,
                        5 => ZoneRole::Highlight,
                        _ => ZoneRole::LightSource,
                    });
                    project.set_mask_zone_role_mutation(0, new_role)
                }
                // P7.3.1 — BezierMesh: reset to identity or clear.
                MutationKind::ResetBezierMesh { rows, cols, some } => {
                    if project.layers.is_empty() {
                        return project.set_gamma_mutation(project.gamma);
                    }
                    let new = if *some {
                        Some(crate::project::schema::BezierMesh::identity(*rows, *cols))
                    } else {
                        None
                    };
                    project.set_reset_layer_bezier_mesh_mutation(0, new)
                }
                // P7.3.3 — MoveBezierAnchor: move anchor (0,0) to (new_x, new_y).
                // Falls back to a no-op gamma if the layer has no bezier_mesh yet
                // (a preceding ResetBezierMesh { some: true } installs one first;
                // the proptest harness applies mutations sequentially so the sequence
                // becomes valid when ordered correctly).
                MutationKind::MoveBezierAnchor { new_x, new_y } => {
                    if project.layers.is_empty() || project.layers[0].bezier_mesh.is_none() {
                        return project.set_gamma_mutation(project.gamma);
                    }
                    project.move_bezier_anchor_mutation(0, 0, 0, [*new_x, *new_y])
                }
                // P7.3.3 — SetBezierHandle: set or clear a handle at anchor (0,0).
                // Falls back to a no-op gamma if no bezier_mesh is present.
                MutationKind::SetBezierHandle { dir_h, pos } => {
                    if project.layers.is_empty() || project.layers[0].bezier_mesh.is_none() {
                        return project.set_gamma_mutation(project.gamma);
                    }
                    let dir = if *dir_h {
                        crate::project::schema::BezierHandleDir::Horizontal
                    } else {
                        crate::project::schema::BezierHandleDir::Vertical
                    };
                    project.set_bezier_handle_mutation(0, 0, 0, dir, *pos)
                }
                // P7.5.1/P7.6.1 — MaskGraph: identity mask (Some) or clear (None).
                MutationKind::SetMaskGraph(some) => {
                    if project.layers.is_empty() {
                        return project.set_gamma_mutation(project.gamma);
                    }
                    let new = if *some {
                        Some(crate::project::schema::MaskGraph::identity())
                    } else {
                        None
                    };
                    project.set_layer_mask_graph_mutation(0, new)
                }
                // 004-T1.13 — SetLayerEffectsAndMask: install a fresh default
                // effect chain alongside a new mask polygon.
                MutationKind::SetLayerEffectsAndMask { new_mask_polygon } => {
                    if project.layers.is_empty() {
                        project.set_gamma_mutation(project.gamma) // no-op fallback
                    } else {
                        use crate::effects::default_effect_chain;
                        project.set_layer_effects_and_mask_mutation(
                            0,
                            default_effect_chain(),
                            new_mask_polygon.clone(),
                        )
                    }
                }
            }
        }

        fn arb_blend_mode() -> impl Strategy<Value = BlendMode> {
            prop_oneof![
                Just(BlendMode::Normal),
                Just(BlendMode::Add),
                Just(BlendMode::Multiply),
                Just(BlendMode::Screen),
            ]
        }

        /// 003-T1.22 — strategy covering all six `Modulator` variants.
        fn arb_modulator() -> impl Strategy<Value = crate::modulators::Modulator> {
            use crate::modulators::Modulator;
            prop_oneof![
                (-1.0_f32..=1.0).prop_map(Modulator::Static),
                (
                    0.05_f32..=10.0,
                    0.0_f32..=1.0,
                    0.0_f32..=std::f32::consts::TAU,
                    -1.0_f32..=1.0
                )
                    .prop_map(|(period_s, amp, phase, offset)| Modulator::Sine {
                        period_s,
                        amp,
                        phase,
                        offset,
                    }),
                (0.05_f32..=10.0, 0.0_f32..=1.0, -1.0_f32..=1.0).prop_map(
                    |(period_s, amp, offset)| Modulator::Triangle {
                        period_s,
                        amp,
                        offset,
                    }
                ),
                (0.05_f32..=10.0, 0.0_f32..=1.0, -1.0_f32..=1.0).prop_map(
                    |(period_s, amp, offset)| Modulator::Noise {
                        period_s,
                        amp,
                        offset,
                    }
                ),
                (0.25_f32..=4.0, 0.0_f32..=1.0, -1.0_f32..=1.0).prop_map(
                    |(divisor, amp, offset)| Modulator::Bpm {
                        divisor,
                        amp,
                        offset,
                    }
                ),
                (0u8..=7, 0.0_f32..=1.0, 0.0_f32..=1.0, -1.0_f32..=1.0).prop_map(
                    |(band, smoothing, amp, offset)| Modulator::Audio {
                        band,
                        smoothing,
                        amp,
                        offset,
                    }
                ),
                // P0.2.1 — OscBound covers a small set of plausible
                // addresses; range bounds match the static-modulator
                // pattern (scale + offset both -1.0..=1.0).
                (
                    prop_oneof![
                        Just("/rmap/blur/radius".to_string()),
                        Just("/rmap/color/hue".to_string()),
                        Just("/foo/bar".to_string()),
                    ],
                    -1.0_f32..=1.0,
                    -1.0_f32..=1.0,
                )
                    .prop_map(|(addr, scale, offset)| Modulator::OscBound {
                        addr,
                        scale,
                        offset,
                    }),
                // P0.2.2 — MidiBound covers the full CC × channel
                // matrix with the same scale / offset shape.
                (0u8..=127, 0u8..=15, -1.0_f32..=1.0, -1.0_f32..=1.0).prop_map(
                    |(cc, channel, scale, offset)| Modulator::MidiBound {
                        cc,
                        channel,
                        scale,
                        offset,
                    },
                ),
            ]
        }

        /// 003-T1.22 — (effect_idx, field) pairs valid for `default_effect_chain()`
        /// (Color[0] → Blur[1] → Transform[2]). TintAmount omitted — no Tint in chain.
        fn arb_field_and_effect_idx() -> impl Strategy<Value = (usize, ModulatorField)> {
            prop_oneof![
                Just((0, ModulatorField::ColorHue)),
                Just((0, ModulatorField::ColorSaturation)),
                Just((0, ModulatorField::ColorBrightness)),
                Just((0, ModulatorField::ColorContrast)),
                Just((1, ModulatorField::BlurRadius)),
                Just((2, ModulatorField::TransformRotateDeg)),
                Just((2, ModulatorField::TransformScaleX)),
                Just((2, ModulatorField::TransformScaleY)),
            ]
        }

        fn arb_mutation_kind() -> impl Strategy<Value = MutationKind> {
            // Float ranges match the existing slider widgets so
            // debug_assert!s stay well-defined (no NaN).
            prop_oneof![
                (0.2_f32..=4.0).prop_map(MutationKind::Gamma),
                (-1.0_f32..=1.0).prop_map(MutationKind::Brightness),
                (0.0_f32..=4.0).prop_map(MutationKind::Contrast),
                (0.0_f32..=5.0).prop_map(MutationKind::CrossfadeDurationS),
                any::<bool>().prop_map(MutationKind::OutputWindowed),
                (0.0_f32..=0.25).prop_map(MutationKind::WarpMaskFeather),
                (1u32..=8, 1u32..=8)
                    .prop_map(|(rows, cols)| MutationKind::WarpDimensions { rows, cols }),
                Just(MutationKind::Snapshot),
                (0.0_f32..=1.0).prop_map(MutationKind::LayerOpacity),
                any::<bool>().prop_map(MutationKind::LayerEnabled),
                arb_blend_mode().prop_map(MutationKind::LayerBlendMode),
                Just(MutationKind::AddLayer),
                Just(MutationKind::RemoveLayer),
                Just(MutationKind::SwapLayers),
                (-1.0_f32..=1.0, -1.0_f32..=1.0)
                    .prop_map(|(x, y)| { MutationKind::LayerEffectsTransformTranslate { x, y } }),
                (arb_field_and_effect_idx(), arb_modulator()).prop_map(
                    |((effect_idx, field), new)| MutationKind::SetModulator {
                        effect_idx,
                        field,
                        new,
                    }
                ),
                // 003-T1.24 — drag-translate coverage: append-then-mutate path.
                (-0.5_f32..=0.5, -0.5_f32..=0.5)
                    .prop_map(|(dx, dy)| MutationKind::LayerEffectsDragTranslate { dx, dy }),
                // 003-T1.26 — drag-rotate coverage: same append-or-mutate path,
                // distinct field (rotate_deg).
                (-180.0_f32..=180.0)
                    .prop_map(|degrees| MutationKind::LayerEffectsDragRotate { degrees }),
                // 003-T1.27 — mask vertex set/add/remove.
                (any::<u8>(), 0.0_f32..=1.0, 0.0_f32..=1.0)
                    .prop_map(|(idx_pick, x, y)| MutationKind::SetMaskVertex { idx_pick, x, y }),
                (any::<u8>(), 0.0_f32..=1.0, 0.0_f32..=1.0).prop_map(|(position_pick, x, y)| {
                    MutationKind::AddMaskVertex {
                        position_pick,
                        x,
                        y,
                    }
                }),
                any::<u8>().prop_map(|idx_pick| MutationKind::RemoveMaskVertex { idx_pick }),
                // 003-T1.28 — reset warp mesh coverage.
                (1u32..=4, 1u32..=4)
                    .prop_map(|(rows, cols)| MutationKind::ResetWarpMesh { rows, cols }),
                // 003-T1.28 — set mask polygon coverage (includes empty for clear-mask).
                proptest::collection::vec(
                    (0.0_f32..=1.0, 0.0_f32..=1.0).prop_map(|(x, y)| [x, y]),
                    0..6,
                )
                .prop_map(|vertices| MutationKind::SetMaskPolygon { vertices }),
                // 003-T1.39 — output monitor index coverage.
                (0usize..=4).prop_map(MutationKind::OutputMonitorIndex),
                // T3.0c — warp corner pin coverage.
                (any::<u32>(), any::<u32>(), 0.0_f32..=1.0, 0.0_f32..=1.0).prop_map(
                    |(r_pick, c_pick, x, y)| MutationKind::LayerWarpCorner {
                        r_pick,
                        c_pick,
                        x,
                        y,
                    },
                ),
                // T3.28 — per-display tone overrides; cover both Some/None
                // arms so the whole-Option Reverse round-trips through the
                // toggle states an operator actually exercises.
                proptest::option::weighted(0.5, 0.2_f32..=4.0)
                    .prop_map(MutationKind::ProjectGammaOverride),
                proptest::option::weighted(0.5, -1.0_f32..=1.0)
                    .prop_map(MutationKind::ProjectBrightnessOverride),
                proptest::option::weighted(0.5, 0.0_f32..=4.0)
                    .prop_map(MutationKind::ProjectContrastOverride),
                // V31.6.1 — mute / solo round-trip coverage.
                any::<bool>().prop_map(MutationKind::LayerMuted),
                // Solo index: None (clear) or Some(0..=2) (fresh_project has 1 layer;
                // to_mutation clamps to a valid index or falls back to 0 when layers > 0).
                proptest::option::weighted(0.5, 0usize..=2).prop_map(MutationKind::LayerSolo),
                // V31.7.2 — quantize bars: None (off) or one of 1/2/4/8.
                proptest::option::weighted(
                    0.5,
                    prop_oneof![Just(1u8), Just(2u8), Just(4u8), Just(8u8)],
                )
                .prop_map(MutationKind::QuantizeBars),
                // P0.7.3 — edge-blend: None / Some(overlap_px, cosine).
                proptest::option::weighted(0.5, (0u32..=512, any::<bool>()),)
                    .prop_map(MutationKind::EdgeBlend),
                // P0.8.1 — per-output RGB matrix: random index (modded in
                // `to_mutation`) + arbitrary 3×3 matrix values in [-2, 2].
                // Each row is generated as a tuple-of-3-ranges (proptest maps
                // `(a, b, c)` to `(f32, f32, f32)`), then assembled into a
                // `[[f32;3];3]`.
                (
                    any::<u8>(),
                    (-2.0_f32..=2.0_f32, -2.0_f32..=2.0_f32, -2.0_f32..=2.0_f32),
                    (-2.0_f32..=2.0_f32, -2.0_f32..=2.0_f32, -2.0_f32..=2.0_f32),
                    (-2.0_f32..=2.0_f32, -2.0_f32..=2.0_f32, -2.0_f32..=2.0_f32),
                )
                    .prop_map(
                        |(output_idx_pick, (r00, r01, r02), (r10, r11, r12), (r20, r21, r22))| {
                            MutationKind::SetOutputRgbMatrix {
                                output_idx_pick,
                                matrix: [[r00, r01, r02], [r10, r11, r12], [r20, r21, r22]],
                            }
                        }
                    ),
                // P0.4.3 — video speed (0.25..=4.0). Falls back to a no-op
                // in `to_mutation` when the project fixture has no Video
                // layers.
                (0.25_f32..=4.0_f32).prop_map(MutationKind::VideoSpeed),
                // P1.4.2 — video loop mode (Once / Loop / PingPong).
                (0u8..=2u8).prop_map(|n| {
                    let mode = match n {
                        0 => crate::project::schema::LoopMode::Once,
                        1 => crate::project::schema::LoopMode::Loop,
                        _ => crate::project::schema::LoopMode::PingPong,
                    };
                    MutationKind::VideoLoopMode(mode)
                }),
                // P1.4.1 — video clip range (clip_in < clip_out). Both
                // values bounded so the proptest doesn't generate NaN.
                (0.0_f32..60.0_f32, 0.05_f32..1.0_f32).prop_map(|(start, dur)| {
                    MutationKind::VideoClipRange {
                        clip_in: start,
                        clip_out: start + dur,
                    }
                }),
                // P1.4.4 — video BPM-lock toggle.
                any::<bool>().prop_map(MutationKind::VideoBpmLock),
                // P1.2.4 — focal point in [0, 1]².
                (0.0_f32..=1.0_f32, 0.0_f32..=1.0_f32)
                    .prop_map(|(x, y)| { MutationKind::LayerFocal([x, y]) }),
                // P1.2.1 — Treatment toggle (None / Some(tone_map | blur_mask))
                // and params edit. The params variant falls back to a no-op
                // when treatment is None (set_layer_treatment_params_mutation
                // panics otherwise — the fallback keeps proptest sequences
                // valid even when a preceding step cleared the treatment).
                proptest::option::weighted(0.5, any::<bool>())
                    .prop_map(MutationKind::SetLayerTreatment),
                (-2.0_f32..=2.0_f32, 0.5_f32..=1.5_f32).prop_map(|(exposure, contrast)| {
                    MutationKind::SetLayerTreatmentParams { exposure, contrast }
                },),
                // P2.9.1 — FxLayer params targeting layer 1 (RIPPLE_WASH).
                // `wavelength` spans the full descriptor range [10, 400] —
                // RIPPLE_WASH has no `max_particle_count` on any descriptor
                // so every value in range commits. When layer 1 is absent or
                // not FxLayer (after RemoveLayer / SwapLayers steps), the
                // `to_mutation` dispatch falls back to a no-op gamma, which
                // is a valid sequence for the round-trip harness.
                (10.0_f32..=400.0_f32)
                    .prop_map(|wavelength| { MutationKind::SetFxLayerParams { wavelength } }),
                // P3.6.1 — SetMaskZoneRole: None (clear) or Some(0..=6) (one
                // of the seven ZoneRole variants; to_mutation mods by 7).
                proptest::option::weighted(0.5, 0u8..=6u8).prop_map(MutationKind::SetMaskZoneRole),
                // P7.3.1 — ResetBezierMesh: identity mesh (Some) or clear (None).
                (1u32..=4, 1u32..=4, any::<bool>()).prop_map(|(rows, cols, some)| {
                    MutationKind::ResetBezierMesh { rows, cols, some }
                }),
                // P7.3.3 — MoveBezierAnchor: move anchor (0,0) to a random position.
                (0.0_f32..=1.0_f32, 0.0_f32..=1.0_f32)
                    .prop_map(|(new_x, new_y)| { MutationKind::MoveBezierAnchor { new_x, new_y } }),
                // P7.3.3 — SetBezierHandle: set or clear a handle at anchor (0,0).
                (
                    any::<bool>(),
                    proptest::option::weighted(
                        0.7,
                        (0.0_f32..=1.0_f32, 0.0_f32..=1.0_f32).prop_map(|(x, y)| [x, y]),
                    ),
                )
                    .prop_map(|(dir_h, pos)| MutationKind::SetBezierHandle { dir_h, pos }),
                // P7.5.1/P7.6.1 — SetMaskGraph: identity MaskGraph or clear.
                any::<bool>().prop_map(MutationKind::SetMaskGraph),
                // 004-T1.13 — SetLayerEffectsAndMask: new mask polygon (0..6 vertices);
                // effect chain is always `default_effect_chain()` in `to_mutation`.
                proptest::collection::vec(
                    (0.0_f32..=1.0, 0.0_f32..=1.0).prop_map(|(x, y)| [x, y]),
                    0..6,
                )
                .prop_map(|new_mask_polygon| MutationKind::SetLayerEffectsAndMask {
                    new_mask_polygon,
                }),
            ]
        }

        /// Finite-f32 strategy covering edge cases useful for bit-exact round-trip
        /// assertions on `crossfade_duration_s`: 0.0, -0.0, subnormals, 1.0,
        /// near-max (4.999 / 5.0), and arbitrary values in [0.0, 5.0].
        fn arb_crossfade_duration_s() -> impl Strategy<Value = f32> {
            prop_oneof![
                Just(0.0_f32),
                Just(-0.0_f32),
                Just(f32::MIN_POSITIVE),       // smallest normal
                Just(f32::MIN_POSITIVE / 2.0), // a real subnormal
                Just(1.0_f32),
                Just(4.999_f32),
                Just(5.0_f32),
                (0.0_f32..=5.0_f32), // fuzzy range
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(1024))]

            /// 004-V31.1.2 — `SetCrossfadeDurationS` apply→undo restores the prior
            /// value with bit-exact (`f32::to_bits()`) equality.
            ///
            /// Regression guard against any future regression in
            /// `SetCrossfadeDurationS::apply`. Verified meaningful via
            /// discriminator: temporarily mutating `apply` to drop the
            /// `new ↔ old` swap fails this proptest with the minimal
            /// counterexample `prior = 0.0, new = -0.0` — the case bit-exact
            /// `f32::to_bits()` comparison catches but a `(a-b).abs() < ε`
            /// check would silently pass. The deferred audit T1.37 was
            /// surfaced before the V31.3.2 trait migration; this test lands
            /// as a regression guard rather than a reproduction of the
            /// historical bug.
            ///
            /// Uses `f32::to_bits()` instead of an epsilon comparison because
            /// the spec requires bit-exact restoration of the prior value; an
            /// epsilon check would miss cases where the implementation stores
            /// a rounded copy of `old` rather than the original bits.
            ///
            /// Non-finite f32 (NaN / Inf) are intentionally excluded: they
            /// would trigger the `debug_assert!` epsilon guard inside `apply`
            /// and are out of scope for this task (see V31.1.1 for the
            /// static-modulator non-finite case).
            #[test]
            fn set_crossfade_duration_s_apply_undo_bit_exact(
                prior in arb_crossfade_duration_s(),
                new   in arb_crossfade_duration_s(),
            ) {
                let mut p = fresh_project();
                p.crossfade_duration_s = prior;
                let mutation = p.set_crossfade_duration_s_mutation(new);
                // apply: writes `new` into the project, returns the undo mutation.
                let reverse = mutation.apply(&mut p);
                prop_assert_eq!(
                    p.crossfade_duration_s.to_bits(),
                    new.to_bits(),
                    "apply should write `new` into the project"
                );
                // undo: apply the reverse mutation, should restore `prior` exactly.
                let _ = reverse.apply(&mut p);
                prop_assert_eq!(
                    p.crossfade_duration_s.to_bits(),
                    prior.to_bits(),
                    "undo should restore the prior value bit-exactly"
                );
            }
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(256))]

            /// Apply N → undo N → byte-equal to start.
            #[test]
            fn apply_then_undo_round_trips(
                kinds in proptest::collection::vec(arb_mutation_kind(), 0..50),
            ) {
                let mut p = fresh_project();
                let before = serde_json::to_value(&p).unwrap();
                let mut stack = UndoStack::new();
                for kind in &kinds {
                    let m = to_mutation(kind, &p);
                    stack.push(m, &mut p);
                }
                while stack.undo(&mut p).is_some() {}
                let after = serde_json::to_value(&p).unwrap();
                prop_assert_eq!(before, after);
            }

            /// Apply N → undo all → redo all → equal to post-apply.
            #[test]
            fn undo_redo_round_trips(
                kinds in proptest::collection::vec(arb_mutation_kind(), 0..50),
            ) {
                let mut p = fresh_project();
                let mut stack = UndoStack::new();
                for kind in &kinds {
                    let m = to_mutation(kind, &p);
                    stack.push(m, &mut p);
                }
                let after_apply = serde_json::to_value(&p).unwrap();
                let undo_count = stack.len();
                for _ in 0..undo_count {
                    stack.undo(&mut p);
                }
                for _ in 0..undo_count {
                    stack.redo(&mut p);
                }
                let after_redo = serde_json::to_value(&p).unwrap();
                prop_assert_eq!(after_apply, after_redo);
            }
        }

        // -------------------------------------------------------------------
        // P6.2.2 — `SetCueTiming` and `SetProjectCues` proptest round-trips.
        // -------------------------------------------------------------------

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(512))]

            /// P6.2.2 — `SetCueTiming` apply→reverse restores the original
            /// timing snapshot on a single cue. Exercises the whole-struct
            /// Reverse rule (rule 2): all fields are captured atomically.
            #[test]
            fn set_cue_timing_round_trips(
                in_time_s in 0.0_f32..=60.0_f32,
                hold_time_s in proptest::option::of(0.0_f32..=300.0_f32),
                out_time_s in 0.0_f32..=60.0_f32,
                follow in proptest::bool::ANY,
                bars in proptest::option::of(
                    proptest::strategy::Just(1u8)
                        .prop_union(proptest::strategy::Just(2))
                        .or(proptest::strategy::Just(4))
                        .or(proptest::strategy::Just(8))
                ),
            ) {
                use crate::project::schema::{BpmQuantize, Cue, CueFireMode};

                let mut p = fresh_project();
                // Push a cue with default timing.
                p.cues.push(Cue::new("test-cue", serde_json::json!({}), None));

                let orig_snapshot = CueTimingSnapshot::from_cue(&p.cues[0]);

                let new_snapshot = CueTimingSnapshot {
                    in_time_s,
                    hold_time_s,
                    out_time_s,
                    fire_mode: if follow { CueFireMode::Follow } else { CueFireMode::GoOnTrigger },
                    bpm_quantize: match bars {
                        Some(n) => BpmQuantize::Bars(n),
                        None => BpmQuantize::Off,
                    },
                    timecode_trigger: None,
                    in_time_binding: None,
                    hold_binding: None,
                    out_time_binding: None,
                    in_time_osc: None,
                    hold_osc: None,
                    out_time_osc: None,
                };

                // Apply the mutation then immediately apply the returned reverse.
                let m = p.set_cue_timing_mutation(0, new_snapshot);
                let reverse = m.apply(&mut p);
                let _ = reverse.apply(&mut p);

                // Timing fields must be restored bit-exactly.
                let restored = CueTimingSnapshot::from_cue(&p.cues[0]);
                prop_assert_eq!(orig_snapshot.in_time_s.to_bits(), restored.in_time_s.to_bits());
                prop_assert_eq!(orig_snapshot.out_time_s.to_bits(), restored.out_time_s.to_bits());
                prop_assert_eq!(orig_snapshot.hold_time_s, restored.hold_time_s);
                prop_assert_eq!(orig_snapshot.fire_mode, restored.fire_mode);
                prop_assert_eq!(orig_snapshot.bpm_quantize, restored.bpm_quantize);
            }

            /// P6.2.2 — `SetProjectCues` apply→reverse restores the original
            /// cue vec (whole-Vec Reverse, rule 3).
            #[test]
            fn set_project_cues_round_trips(
                name_a in "[a-z]{1,8}",
                name_b in "[a-z]{1,8}",
            ) {
                use crate::project::schema::Cue;

                let mut p = fresh_project();
                let orig_cues = p.cues.clone();

                let new_cues = vec![
                    Cue::new(name_a.as_str(), serde_json::json!({}), None),
                    Cue::new(name_b.as_str(), serde_json::json!({}), None),
                ];

                let m = p.set_project_cues_mutation(new_cues);
                let reverse = m.apply(&mut p);
                prop_assert_eq!(p.cues.len(), 2, "apply installed 2 cues");
                let _ = reverse.apply(&mut p);
                prop_assert_eq!(p.cues.len(), orig_cues.len(), "reverse restored original len");
            }
        }
    }

    /// P2.7.1 — `SetLayerEffects` with a reordered Vec round-trips through
    /// apply + reverse correctly. Verifies the Effects-Vec Reverse rule 2
    /// for the drag-reorder path specifically: apply installs the new order;
    /// undo (apply the returned reverse) restores the original order
    /// byte-exactly.
    ///
    /// Start: `[Color, Blur, Transform]` — the default chain from
    /// `default_effect_chain()`.
    /// Reorder to: `[Blur, Color, Transform]` (drag Blur above Color,
    /// src=1, dst=0: remove(1)→[Color,Transform], insert(0,Blur)→result).
    #[test]
    fn set_layer_effects_reorder_round_trips() {
        use crate::effects::{Effect, default_effect_chain};

        let mut p = fresh_project();
        // Install a known three-element chain: [Color, Blur, Transform].
        p.layers[0].effects = default_effect_chain();
        let original = p.layers[0].effects.clone();
        assert_eq!(original.len(), 3, "default chain should have 3 effects");
        assert!(
            matches!(original[0], Effect::Color { .. }),
            "original[0] should be Color"
        );
        assert!(
            matches!(original[1], Effect::Blur { .. }),
            "original[1] should be Blur"
        );
        assert!(
            matches!(original[2], Effect::Transform { .. }),
            "original[2] should be Transform"
        );

        // Build reordered chain: [Blur, Color, Transform].
        let mut reordered = original.clone();
        let item = reordered.remove(1); // remove Blur
        reordered.insert(0, item); // insert before Color
        assert!(
            matches!(reordered[0], Effect::Blur { .. }),
            "reordered[0] should be Blur"
        );
        assert!(
            matches!(reordered[1], Effect::Color { .. }),
            "reordered[1] should be Color"
        );
        assert!(
            matches!(reordered[2], Effect::Transform { .. }),
            "reordered[2] should be Transform"
        );

        // Build and apply the mutation.
        let mutation = p.set_layer_effects_mutation(0, reordered.clone());
        let reverse = mutation.apply(&mut p);

        // After apply: chain should match the reordered order.
        assert!(
            matches!(p.layers[0].effects[0], Effect::Blur { .. }),
            "after apply: effects[0] should be Blur"
        );
        assert!(
            matches!(p.layers[0].effects[1], Effect::Color { .. }),
            "after apply: effects[1] should be Color"
        );
        assert!(
            matches!(p.layers[0].effects[2], Effect::Transform { .. }),
            "after apply: effects[2] should be Transform"
        );

        // Apply the reverse (undo): should restore the original order.
        let _ = reverse.apply(&mut p);
        let after_undo = serde_json::to_value(&p.layers[0].effects).unwrap();
        let original_val = serde_json::to_value(&original).unwrap();
        assert_eq!(
            after_undo, original_val,
            "after undo: effect chain should be restored to original order"
        );
    }

    /// P2.7.2 — verify that appending a default `Effect::Blur` via
    /// `SetLayerEffects` and then applying the reverse restores the
    /// original empty-effects list (Effects-Vec Reverse rule 2).
    #[test]
    fn set_layer_effects_append_default_blur_round_trips() {
        use crate::effects::Effect;
        use crate::modulators::Modulator;

        let mut p = fresh_project();
        // Start with an empty effect chain so the "append" semantics are clear.
        p.layers[0].effects = vec![];
        let original = p.layers[0].effects.clone();
        assert_eq!(original.len(), 0, "starting chain should be empty");

        // Build a chain with a single default Blur (matches default_effect_chain defaults).
        let default_blur = Effect::Blur {
            radius_px: Modulator::Static(0.0),
        };
        let new_chain = vec![default_blur];

        // Build and apply the mutation (Effects-Vec Reverse snapshots the full old vec).
        let mutation = p.set_layer_effects_mutation(0, new_chain);
        let reverse = mutation.apply(&mut p);

        // After apply: chain should have one Blur entry.
        assert_eq!(
            p.layers[0].effects.len(),
            1,
            "after apply: chain should have 1 effect"
        );
        assert!(
            matches!(p.layers[0].effects[0], Effect::Blur { .. }),
            "after apply: effects[0] should be Blur"
        );

        // Apply the reverse (undo): should restore the original empty chain.
        let _ = reverse.apply(&mut p);
        let after_undo = serde_json::to_value(&p.layers[0].effects).unwrap();
        let original_val = serde_json::to_value(&original).unwrap();
        assert_eq!(
            after_undo, original_val,
            "after undo: effect chain should be restored to empty"
        );
    }

    // --- 004-T1.13 SetLayerEffectsAndMask tests ---

    /// 004-T1.13 — `SetLayerEffectsAndMask` apply → reverse → re-apply
    /// restores both `effects` and `mask_polygon` identically.
    ///
    /// Verifies Reverse rules 1+2 (whole-Vec effect chain + whole-Vec mask
    /// polygon). Starts with non-trivial values on BOTH fields so a stub
    /// that only swaps one field would fail the assertion.
    #[test]
    fn set_layer_effects_and_mask_round_trips() {
        use crate::effects::{Effect, default_effect_chain};
        use crate::modulators::Modulator;

        let mut p = fresh_project();

        // Install a distinct, non-trivial starting state on both fields.
        let initial_effects = vec![Effect::Blur {
            radius_px: Modulator::Static(5.0),
        }];
        let initial_mask = vec![[0.1_f32, 0.2], [0.8, 0.2], [0.8, 0.8], [0.1, 0.8]];
        p.layers[0].effects = initial_effects.clone();
        p.layers[0].warp.mask_polygon = initial_mask.clone();

        // Snapshot the whole-layer before the mutation.
        let before_layer = serde_json::to_value(&p.layers[0]).unwrap();

        // New values: default chain (3 entries) + a different polygon.
        let new_effects = default_effect_chain();
        let new_mask = vec![[0.0_f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

        // Apply: installs new_effects + new_mask.
        let m = p.set_layer_effects_and_mask_mutation(0, new_effects.clone(), new_mask.clone());
        let reverse = m.apply(&mut p);

        // Post-apply: both fields must match the new values.
        assert_eq!(
            p.layers[0].effects.len(),
            new_effects.len(),
            "after apply: effects len should match new_effects"
        );
        assert_eq!(
            p.layers[0].warp.mask_polygon,
            new_mask,
            "after apply: mask_polygon should match new_mask"
        );

        // Undo (apply the reverse): should restore both fields to initial state.
        let re_apply = reverse.apply(&mut p);
        let after_undo_layer = serde_json::to_value(&p.layers[0]).unwrap();
        assert_eq!(
            before_layer, after_undo_layer,
            "after undo: whole layer must be byte-equal to pre-mutation state"
        );

        // Re-apply (redo): should restore both fields to the new values.
        let _ = re_apply.apply(&mut p);
        assert_eq!(
            p.layers[0].effects.len(),
            new_effects.len(),
            "after redo: effects len should match new_effects"
        );
        assert_eq!(
            p.layers[0].warp.mask_polygon,
            new_mask,
            "after redo: mask_polygon should match new_mask"
        );
    }

    /// 004-T1.13 — `SetLayerEffectsAndMask` is undoable (not non-undoable).
    #[test]
    fn set_layer_effects_and_mask_is_undoable() {
        use crate::effects::default_effect_chain;

        let p = fresh_project();
        let m = p.set_layer_effects_and_mask_mutation(
            0,
            default_effect_chain(),
            vec![[0.0_f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        );
        assert!(
            !m.is_non_undoable(),
            "SetLayerEffectsAndMask must be undoable (is_non_undoable must return false)"
        );
    }

    // --- P3.2.3 SetMaskZoneRole tests ---

    fn project_with_fx_layer_and_mask() -> Project {
        let layer = crate::project::schema::layer_from_fx_preset(
            "l0",
            "mask_edge_ripple_wash",
            Default::default(),
            0,
        );
        let mut p = Project::default();
        p.layers.push(layer);
        p
    }

    /// P3.2.3 — `SetMaskZoneRole::apply` sets `warp.zone_role` and returns the
    /// correct reverse (old/new swapped).
    #[test]
    fn set_mask_zone_role_apply_and_reverse() {
        use crate::project::schema::ZoneRole;

        let mut p = project_with_fx_layer_and_mask();
        assert_eq!(p.layers[0].warp.zone_role, None);

        // Apply: None → Some(Window).
        let m = p.set_mask_zone_role_mutation(0, Some(ZoneRole::Window));
        let reverse = m.apply(&mut p);
        assert_eq!(
            p.layers[0].warp.zone_role,
            Some(ZoneRole::Window),
            "zone_role must be Window after apply"
        );

        // Reverse: Some(Window) → None.
        let _ = reverse.apply(&mut p);
        assert_eq!(
            p.layers[0].warp.zone_role, None,
            "zone_role must be None after reverse"
        );
    }

    /// P3.2.3 — `set_mask_zone_role_mutation` captures the correct `old` value
    /// so the debug_assert fires when a stale Reverse is applied.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "SetMaskZoneRole stale Reverse")]
    fn set_mask_zone_role_stale_reverse_panics() {
        use crate::project::schema::ZoneRole;

        let mut p = project_with_fx_layer_and_mask();
        // Build a mutation with stale old = Some(Portal) but the current state is None.
        let stale = Mutation::SetMaskZoneRole(SetMaskZoneRole {
            layer_idx: 0,
            new: Some(ZoneRole::Window),
            old: Some(ZoneRole::Portal), // stale — actual is None
        });
        // This must panic in debug builds.
        let _ = stale.apply(&mut p);
    }

    /// P3.2.3 — `SetMaskZoneRole` is undoable (not a non-undoable mutation).
    #[test]
    fn set_mask_zone_role_is_undoable() {
        use crate::project::schema::ZoneRole;

        let p = project_with_fx_layer_and_mask();
        let m = p.set_mask_zone_role_mutation(0, Some(ZoneRole::Edge));
        assert!(
            !m.is_non_undoable(),
            "SetMaskZoneRole must be undoable (is_non_undoable must return false)"
        );
    }

    // -----------------------------------------------------------------------
    // P4.8.2 — Wizard commit / cancel `ApplyProjectSnapshot` round-trip tests
    // -----------------------------------------------------------------------

    /// P4.8.2 — Wizard cancel: non-undoable `ApplyProjectSnapshot` round-trip.
    ///
    /// Simulates the `wizard_cancel` path: apply (install the pre-wizard
    /// snapshot) then apply the Reverse (go back to the mid-wizard state).
    /// The final state must equal the pre-wizard state (byte-for-byte).
    #[test]
    fn wizard_cancel_apply_project_snapshot_round_trip() {
        let pre_wizard = fresh_project();
        let pre_snap = crate::project::snapshot(&pre_wizard);

        let mut mid_wizard = pre_wizard.clone();
        mid_wizard.gamma = 3.0 + 0.14_f32; // avoids approx_constant lint
        let mid_snap = crate::project::snapshot(&mid_wizard);

        // Cancel mutation: restore pre-wizard snapshot (non-undoable).
        let cancel_mutation = Mutation::ApplyProjectSnapshot(ApplyProjectSnapshot {
            new: pre_snap.clone(),
            old: mid_snap,
            non_undoable: true,
        });
        assert!(
            cancel_mutation.is_non_undoable(),
            "wizard cancel mutation must be non-undoable"
        );

        let mut p = mid_wizard;
        let reverse = cancel_mutation.apply(&mut p);

        // After applying: project must equal pre-wizard state.
        let after_snap = crate::project::snapshot(&p);
        assert_eq!(
            pre_snap, after_snap,
            "after wizard cancel, project must equal pre-wizard snapshot"
        );

        // Applying the Reverse restores the mid-wizard state.
        let _ = reverse.apply(&mut p);
        let restored_gamma = p.gamma;
        // 3.14 is not the standard f32::consts::PI; use a non-round value to
        // distinguish from the default (1.0) while avoiding the approx_constant lint.
        let expected_gamma = 3.0 + 0.14_f32;
        assert!(
            (restored_gamma - expected_gamma).abs() < 1e-2,
            "after Reverse of cancel, gamma must equal mid-wizard value"
        );
    }

    /// P4.8.2 — Wizard commit: undoable `ApplyProjectSnapshot` round-trip.
    ///
    /// Simulates the `wizard_commit` path: apply (install the generated
    /// template JSON) then apply the Reverse (restore the pre-wizard state).
    /// `is_non_undoable()` must return `false` for the commit mutation.
    #[test]
    fn wizard_commit_apply_project_snapshot_round_trip() {
        let pre_wizard = fresh_project();
        let pre_snap = crate::project::snapshot(&pre_wizard);

        // Simulate a generated template project (two layers).
        let mut generated = pre_wizard.clone();
        generated.gamma = 2.0 + 0.71_f32; // avoids approx_constant lint
        generated
            .layers
            .push(crate::project::schema::layer_from_fx_preset(
                "scene_window_reveal_fx_0",
                "mask_edge_ripple_wash",
                std::collections::HashMap::new(),
                0,
            ));
        let generated_snap = crate::project::snapshot(&generated);

        // Commit mutation: apply generated JSON (undoable).
        let commit_mutation = Mutation::ApplyProjectSnapshot(ApplyProjectSnapshot {
            new: generated_snap.clone(),
            old: pre_snap.clone(),
            non_undoable: false,
        });
        assert!(
            !commit_mutation.is_non_undoable(),
            "wizard commit mutation must be undoable (is_non_undoable must return false)"
        );

        let mut p = pre_wizard;
        let reverse = commit_mutation.apply(&mut p);

        // After applying: project must equal generated state.
        let after_snap = crate::project::snapshot(&p);
        assert_eq!(
            generated_snap, after_snap,
            "after wizard commit, project must equal generated template snapshot"
        );

        // Undo (Reverse): restore_scene preserves the pre-wizard snapshot layers
        // and keeps any layers whose IDs weren't in pre_snap (the template-generated
        // layer is kept as a "post-save addition" per restore_scene semantics).
        // The important invariant: pre-wizard project settings (gamma) are restored.
        let _ = reverse.apply(&mut p);
        assert!(
            (p.gamma - 1.0_f32).abs() < 1e-4,
            "after Cmd-Z, gamma must be restored to pre-wizard value (1.0), got {}",
            p.gamma
        );
        // The pre-wizard layers are present in the restored project.
        assert!(
            p.layers.iter().any(|l| l.id == "test_layer"),
            "pre-wizard layer 'test_layer' must be present after undo"
        );
        // is_non_undoable check for commit.
        let commit2 = Mutation::ApplyProjectSnapshot(ApplyProjectSnapshot {
            new: pre_snap.clone(),
            old: pre_snap.clone(),
            non_undoable: false,
        });
        assert!(
            !commit2.is_non_undoable(),
            "wizard commit ApplyProjectSnapshot must have non_undoable = false"
        );
    }

    // ---------------------------------------------------------------------------
    // P5.3.3–P5.3.5 + P5.7.2–P5.7.4 — Fixture mutation round-trips
    // ---------------------------------------------------------------------------

    #[cfg(feature = "lighting")]
    mod lighting_mutations {
        use super::*;
        use crate::lighting::chase::{ChaseStep, FixtureChase, FixtureChaseParams, FixtureChaseid};
        use crate::lighting::fixture::{
            FixtureGroup, FixtureGroupId, FixtureGroupParams, FixturePersonality, FixtureSource,
            OutputStrategy,
        };
        use crate::lighting::universe::UniverseId;

        fn empty_project_with_lighting() -> Project {
            Project {
                fixture_groups: Vec::new(),
                fixture_chases: Vec::new(),
                ..Project::default()
            }
        }

        fn make_group(id: u64) -> FixtureGroup {
            FixtureGroup {
                id: FixtureGroupId(id),
                label: format!("group-{id}"),
                personality: FixturePersonality::default_rgb(),
                universe_id: UniverseId::default(),
                base_channel: 0,
                fixture_count: 2,
                output_strategy: OutputStrategy::RgbDirect,
                source: FixtureSource::default(),
                rgbw_config: crate::lighting::rgbw::RgbwConfig::default(),
            }
        }

        fn make_chase(id: u64, group_id: u64) -> FixtureChase {
            FixtureChase {
                id: FixtureChaseid(id),
                label: format!("chase-{id}"),
                group_id: FixtureGroupId(group_id),
                steps: vec![
                    ChaseStep {
                        color: (255, 0, 0),
                        hold_beats: 1,
                    },
                    ChaseStep {
                        color: (0, 0, 255),
                        hold_beats: 1,
                    },
                ],
                beat_divisor: 2,
            }
        }

        /// P5.3.3 — AddFixtureGroup → undo (RemoveFixtureGroup) → groups unchanged.
        #[test]
        fn add_fixture_group_undo_round_trip() {
            let mut p = empty_project_with_lighting();
            let group = make_group(1);
            let id = group.id;

            // Apply: group is added.
            let reverse = Mutation::AddFixtureGroup { group }.apply(&mut p);
            assert_eq!(p.fixture_groups.len(), 1, "group should be added");
            assert_eq!(p.fixture_groups[0].id, id);

            // Reverse (RemoveFixtureGroup): group is removed.
            let _ = reverse.apply(&mut p);
            assert!(
                p.fixture_groups.is_empty(),
                "group should be removed on undo"
            );
        }

        /// P5.3.4 — RemoveFixtureGroup → undo (AddFixtureGroup) → group is back at same index.
        #[test]
        fn remove_fixture_group_undo_round_trip() {
            let mut p = empty_project_with_lighting();
            let group = make_group(2);
            p.fixture_groups.push(group);

            // Apply: group is removed.
            let reverse = Mutation::RemoveFixtureGroup {
                id: FixtureGroupId(2),
            }
            .apply(&mut p);
            assert!(p.fixture_groups.is_empty());

            // Reverse (AddFixtureGroup): group is back.
            let _ = reverse.apply(&mut p);
            assert_eq!(p.fixture_groups.len(), 1);
            assert_eq!(p.fixture_groups[0].id, FixtureGroupId(2));
        }

        /// P5.3.5 — SetFixtureGroupParams → undo → original label.
        #[test]
        fn set_fixture_group_params_undo_restores_label() {
            let mut p = empty_project_with_lighting();
            let group = make_group(3);
            p.fixture_groups.push(group);

            let old_params = FixtureGroupParams::from_group(&p.fixture_groups[0]);
            let mut new_params = old_params.clone();
            new_params.label = "mutated-label".to_string();

            let mutation = Mutation::SetFixtureGroupParams(SetFixtureGroupParams {
                id: FixtureGroupId(3),
                new: new_params.clone(),
                old: old_params.clone(),
            });
            let reverse = mutation.apply(&mut p);
            assert_eq!(p.fixture_groups[0].label, "mutated-label");

            // Undo restores the original label.
            let _ = reverse.apply(&mut p);
            assert_eq!(p.fixture_groups[0].label, old_params.label);
        }

        /// P5.7.2 — AddFixtureChase → undo → chases unchanged.
        #[test]
        fn add_fixture_chase_undo_round_trip() {
            let mut p = empty_project_with_lighting();
            let chase = make_chase(10, 1);
            let id = chase.id;

            let reverse = Mutation::AddFixtureChase { chase }.apply(&mut p);
            assert_eq!(p.fixture_chases.len(), 1);
            assert_eq!(p.fixture_chases[0].id, id);

            let _ = reverse.apply(&mut p);
            assert!(p.fixture_chases.is_empty());
        }

        /// P5.7.3 — RemoveFixtureChase → undo → chase back at same index.
        #[test]
        fn remove_fixture_chase_undo_round_trip() {
            let mut p = empty_project_with_lighting();
            let chase = make_chase(11, 1);
            p.fixture_chases.push(chase);

            let reverse = Mutation::RemoveFixtureChase {
                id: FixtureChaseid(11),
            }
            .apply(&mut p);
            assert!(p.fixture_chases.is_empty());

            let _ = reverse.apply(&mut p);
            assert_eq!(p.fixture_chases.len(), 1);
            assert_eq!(p.fixture_chases[0].id, FixtureChaseid(11));
        }

        /// P5.7.4 — SetFixtureChaseParams → undo → original beat_divisor.
        #[test]
        fn set_fixture_chase_params_undo_restores_beat_divisor() {
            let mut p = empty_project_with_lighting();
            let chase = make_chase(12, 1);
            p.fixture_chases.push(chase);

            let old_params = FixtureChaseParams::from_chase(&p.fixture_chases[0]);
            let mut new_params = old_params.clone();
            new_params.beat_divisor = 4;

            let mutation = Mutation::SetFixtureChaseParams(SetFixtureChaseParams {
                id: FixtureChaseid(12),
                new: new_params,
                old: old_params.clone(),
            });
            let reverse = mutation.apply(&mut p);
            assert_eq!(p.fixture_chases[0].beat_divisor, 4);

            let _ = reverse.apply(&mut p);
            assert_eq!(p.fixture_chases[0].beat_divisor, old_params.beat_divisor);
        }

        // P5.10.2 — proptest extension: fixture-group Mutation round-trips.
        //
        // Uses proptest's property-based strategy to generate random fixture
        // labels, universe IDs, base channels, etc. and verify that
        // Apply → Reverse is a no-op (1000 cases each).
        mod proptest_lighting {
            use super::*;
            use proptest::prelude::*;

            // Strategy: generate a random label string.
            fn arb_label() -> impl Strategy<Value = String> {
                "[a-z][a-z0-9_-]{0,15}".prop_map(|s| s)
            }

            // Strategy: generate a random FixtureGroup with a given id.
            fn arb_fixture_group(id: u64) -> impl Strategy<Value = FixtureGroup> {
                (arb_label(), 0u16..=32767u16, 0u8..=240, 1u8..=16).prop_map(
                    move |(label, univ, base_ch, count)| FixtureGroup {
                        id: FixtureGroupId(id),
                        label,
                        personality: FixturePersonality::default_rgb(),
                        universe_id: UniverseId(univ),
                        base_channel: base_ch,
                        fixture_count: count,
                        output_strategy: OutputStrategy::RgbDirect,
                        source: FixtureSource::default(),
                        rgbw_config: crate::lighting::rgbw::RgbwConfig::default(),
                    },
                )
            }

            proptest! {
                /// P5.10.2 — AddFixtureGroup → RemoveFixtureGroup (reverse) is a no-op.
                #[test]
                fn proptest_add_remove_fixture_group_is_noop(
                    group in arb_fixture_group(100),
                ) {
                    let mut p = empty_project_with_lighting();
                    let initial_len = p.fixture_groups.len();

                    let reverse = Mutation::AddFixtureGroup { group }.apply(&mut p);
                    prop_assert_eq!(p.fixture_groups.len(), initial_len + 1);

                    let _ = reverse.apply(&mut p);
                    prop_assert_eq!(p.fixture_groups.len(), initial_len);
                }

                /// P5.10.2 — SetFixtureGroupParams → undo → original label restored.
                #[test]
                fn proptest_set_fixture_group_params_undo_restores_label(
                    group in arb_fixture_group(200),
                    new_label in arb_label(),
                ) {
                    let mut p = empty_project_with_lighting();
                    let original_label = group.label.clone();
                    p.fixture_groups.push(group);

                    let old_params = FixtureGroupParams::from_group(&p.fixture_groups[0]);
                    let mut new_params = old_params.clone();
                    new_params.label = new_label;

                    let reverse = Mutation::SetFixtureGroupParams(SetFixtureGroupParams {
                        id: FixtureGroupId(200),
                        new: new_params,
                        old: old_params,
                    }).apply(&mut p);

                    // Undo: label must return to original.
                    let _ = reverse.apply(&mut p);
                    prop_assert_eq!(&p.fixture_groups[0].label, &original_label);
                }

                /// P5.10.2 — SetFixtureGroupParams universe_id → undo → original.
                #[test]
                fn proptest_set_fixture_group_params_undo_restores_universe(
                    group in arb_fixture_group(201),
                    new_univ in 0u16..=32767u16,
                ) {
                    let mut p = empty_project_with_lighting();
                    let original_univ = group.universe_id;
                    p.fixture_groups.push(group);

                    let old_params = FixtureGroupParams::from_group(&p.fixture_groups[0]);
                    let mut new_params = old_params.clone();
                    new_params.universe_id = UniverseId(new_univ);

                    let reverse = Mutation::SetFixtureGroupParams(SetFixtureGroupParams {
                        id: FixtureGroupId(201),
                        new: new_params,
                        old: old_params,
                    }).apply(&mut p);

                    let _ = reverse.apply(&mut p);
                    prop_assert_eq!(p.fixture_groups[0].universe_id, original_univ);
                }

                /// P5.10.2 — SetFixtureChaseParams beat_divisor → undo → original.
                #[test]
                fn proptest_set_fixture_chase_params_undo_restores_beat_divisor(
                    orig_divisor in 1u8..=8u8,
                    new_divisor in 1u8..=8u8,
                ) {
                    let mut p = empty_project_with_lighting();
                    let chase = FixtureChase {
                        id: FixtureChaseid(300),
                        label: "prop-chase".to_string(),
                        group_id: FixtureGroupId(1),
                        steps: vec![ChaseStep { color: (255, 0, 0), hold_beats: 1 }],
                        beat_divisor: orig_divisor,
                    };
                    p.fixture_chases.push(chase);

                    let old_params = FixtureChaseParams::from_chase(&p.fixture_chases[0]);
                    let mut new_params = old_params.clone();
                    new_params.beat_divisor = new_divisor;

                    let reverse = Mutation::SetFixtureChaseParams(SetFixtureChaseParams {
                        id: FixtureChaseid(300),
                        new: new_params,
                        old: old_params,
                    }).apply(&mut p);

                    let _ = reverse.apply(&mut p);
                    prop_assert_eq!(p.fixture_chases[0].beat_divisor, orig_divisor);
                }

                /// P7.12.1 — `SetFixtureGroupParams` with RGBW config change →
                /// undo → original `RgbwConfig` restored.
                ///
                /// `RgbwConfig` is part of `FixtureGroupParams` (whole-struct
                /// Reverse — rule 1).  A dedicated `SetRgbwConfig` Mutation is
                /// not needed; RGBW edits flow through `SetFixtureGroupParams`.
                /// This test exercises both enable and CCT round-trip.
                #[test]
                fn proptest_set_fixture_group_params_undo_restores_rgbw_config(
                    group in arb_fixture_group(202),
                    new_cct in 2000u16..=8000u16,
                    new_w_scale in 0.0_f32..=2.0_f32,
                    enabled in any::<bool>(),
                ) {
                    let mut p = empty_project_with_lighting();
                    let original_rgbw = group.rgbw_config.clone();
                    p.fixture_groups.push(group);

                    let old_params = FixtureGroupParams::from_group(&p.fixture_groups[0]);
                    let mut new_params = old_params.clone();
                    new_params.rgbw_config = crate::lighting::rgbw::RgbwConfig {
                        enabled,
                        w_channel_cct_k: new_cct,
                        w_scale: new_w_scale,
                    };

                    let reverse = Mutation::SetFixtureGroupParams(SetFixtureGroupParams {
                        id: FixtureGroupId(202),
                        new: new_params,
                        old: old_params,
                    })
                    .apply(&mut p);

                    // Undo: RgbwConfig must return to original.
                    let _ = reverse.apply(&mut p);
                    prop_assert_eq!(
                        &p.fixture_groups[0].rgbw_config,
                        &original_rgbw,
                        "undo must restore RgbwConfig exactly"
                    );
                }
            }
        }

        // P7.12.1 — Phase 7 Mutation coverage note.
        //
        // The proptest round-trip harness (`apply_then_undo_round_trips` and
        // `apply_then_undo_redo_round_trips`) already covers all new Phase 7
        // mutations added to `arb_mutation_kind()`:
        //
        //   • `ResetBezierMesh`  — W3.1 (P7.3.1)
        //   • `MoveBezierAnchor` — W3.3 (P7.3.3)
        //   • `SetBezierHandle`  — W3.3 (P7.3.3)
        //   • `SetMaskGraph`     — W5/W6 (P7.5.1 + P7.6.1); umbrella mutation
        //                          covering Polygon, Inverse, LumaKey, ChromaKey
        //                          node replacements via `SetLayerMaskGraph`.
        //
        // Dedicated `SetMaskInverse`, `SetLumaKey`, and `SetChromaKey` mutations
        // are NOT defined — the spec intended a single `SetLayerMaskGraph`
        // mutation that replaces the whole graph.  This is the documented design
        // choice (whole-enum Reverse rule 1 — simpler than per-node mutations).
        //
        // `SetRgbwConfig` is covered via `SetFixtureGroupParams` (see
        // `proptest_set_fixture_group_params_undo_restores_rgbw_config` above).
    }
}
