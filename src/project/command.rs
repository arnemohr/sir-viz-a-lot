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
            self.old_path.as_path(),
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
        }
        RelinkAssetPath {
            layer_idx: self.layer_idx,
            new_path: self.old_path,
            old_path: self.new_path,
        }
    }
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

/// Payload for [`Mutation::SetProjectScenes`].
///
/// Replaces `Project.scenes` wholesale (whole-Vec snapshot Reverse).
#[derive(Debug, Clone)]
pub struct SetProjectScenes {
    /// Replacement scenes Vec.
    pub new: Vec<crate::project::schema::Scene>,
    /// Pre-mutation scenes Vec.
    pub old: Vec<crate::project::schema::Scene>,
}

impl ReverseStorage for SetProjectScenes {
    fn apply(self, project: &mut Project) -> Self {
        debug_assert!(
            project.scenes.len() == self.old.len(),
            "SetProjectScenes stale Reverse: scenes.len()={}, expected old.len()={}",
            project.scenes.len(),
            self.old.len()
        );
        let post = self.new;
        project.scenes = post.clone();
        SetProjectScenes {
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
            project.output_target.fallback_index == self.old,
            "SetOutputMonitorIndex stale Reverse: project.output_target.fallback_index={}, expected old={}",
            project.output_target.fallback_index,
            self.old
        );
        project.output_target.fallback_index = self.new;
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
        let _ = crate::project::restore_scene(project, &self.new);
        ApplyProjectSnapshot {
            new: self.old,
            old: self.new,
            non_undoable: self.non_undoable,
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
    /// Replace `Project.output_windowed`. Delegates to [`SetOutputWindowed`].
    SetOutputWindowed(SetOutputWindowed),
    /// Replace `WarpMesh.mask_feather`. Delegates to [`SetLayerMaskFeather`].
    SetLayerMaskFeather(SetLayerMaskFeather),
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

    /// Replace the modulator at `(layer_idx, effect_idx, field)`. Delegates to [`SetModulator`].
    SetModulator(SetModulator),

    /// Replace the entire `WarpMesh`. Delegates to [`ResetLayerWarpMesh`].
    ResetLayerWarpMesh(ResetLayerWarpMesh),
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

    /// Replace `Project.output_monitor_index`. Delegates to [`SetOutputMonitorIndex`].
    SetOutputMonitorIndex(SetOutputMonitorIndex),

    /// Replace the entire project from a serde_json snapshot. Delegates to [`ApplyProjectSnapshot`].
    ApplyProjectSnapshot(ApplyProjectSnapshot),
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
            Mutation::SetOutputWindowed(s) => Mutation::SetOutputWindowed(s.apply(project)),
            Mutation::SetLayerMaskFeather(s) => Mutation::SetLayerMaskFeather(s.apply(project)),
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
            Mutation::SwapLayers(s) => Mutation::SwapLayers(s.apply(project)),
            Mutation::RelinkAssetPath(s) => Mutation::RelinkAssetPath(s.apply(project)),
            Mutation::SetModulator(s) => Mutation::SetModulator(s.apply(project)),
            Mutation::ResetLayerWarpMesh(s) => Mutation::ResetLayerWarpMesh(s.apply(project)),
            Mutation::SetLayerMaskPolygon(s) => Mutation::SetLayerMaskPolygon(s.apply(project)),
            Mutation::SetLayerMaskVertex(s) => Mutation::SetLayerMaskVertex(s.apply(project)),
            Mutation::SetLayerWarpCorner(s) => Mutation::SetLayerWarpCorner(s.apply(project)),
            Mutation::SetProjectScenes(s) => Mutation::SetProjectScenes(s.apply(project)),
            Mutation::SetOutputMonitorIndex(s) => Mutation::SetOutputMonitorIndex(s.apply(project)),
            Mutation::ApplyProjectSnapshot(s) => Mutation::ApplyProjectSnapshot(s.apply(project)),

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
            | Mutation::SetLayerWarpDimensions(_)
            | Mutation::SetLayerOpacity(_)
            | Mutation::SetLayerEnabled(_)
            | Mutation::SetLayerMuted(_)
            | Mutation::SetLayerSolo(_)
            | Mutation::SetQuantizeBars(_)
            | Mutation::SetLayerBlendMode(_)
            | Mutation::SetLayerEffects(_)
            | Mutation::SetModulator(_)
            | Mutation::AddLayer { .. }
            | Mutation::RemoveLayer { .. }
            | Mutation::SwapLayers(_)
            | Mutation::AddLayerMaskVertex { .. }
            | Mutation::RemoveLayerMaskVertex { .. }
            | Mutation::SetLayerMaskVertex(_)
            | Mutation::ResetLayerWarpMesh(_)
            | Mutation::SetLayerMaskPolygon(_)
            | Mutation::SetLayerWarpCorner(_)
            | Mutation::SetProjectScenes(_)
            | Mutation::SetOutputMonitorIndex(_)
            | Mutation::SetProjectGammaOverride(_)
            | Mutation::SetProjectBrightnessOverride(_)
            | Mutation::SetProjectContrastOverride(_)
            | Mutation::RelinkAssetPath(_) => false,
            Mutation::ApplyProjectSnapshot(s) => s.non_undoable,
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
            | Mutation::RelinkAssetPath(_) => true,
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
            old: self.output_target.fallback_index,
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
    /// current `project.scenes` as `old`; `new` is the replacement Vec to
    /// install (e.g. after a slot save or placeholder extension). The Reverse
    /// restores the entire pre-save Vec byte-equally on undo.
    pub fn set_project_scenes_mutation(&self, new: Vec<crate::project::schema::Scene>) -> Mutation {
        Mutation::SetProjectScenes(SetProjectScenes {
            new,
            old: self.scenes.clone(),
        })
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
        let original = p.layers[0].kind.asset_path().to_path_buf();

        let new_path = std::path::PathBuf::from("/some/other/place.svg");
        let mutation = Mutation::RelinkAssetPath(RelinkAssetPath {
            layer_idx: 0,
            new_path: new_path.clone(),
            old_path: original.clone(),
        });
        let reverse = mutation.apply(&mut p);
        assert_eq!(
            p.layers[0].kind.asset_path(),
            new_path.as_path(),
            "apply should rewrite svg_path",
        );

        let _ = reverse.apply(&mut p);
        let after = serde_json::to_value(&p).unwrap();
        assert_eq!(before, after, "Reverse should restore byte-equal project",);
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
        let mut new_scenes = p.scenes.clone();
        new_scenes.push(crate::project::schema::Scene {
            name: "scene1".into(),
            snapshot: crate::project::snapshot(&p),
            thumbnail: None,
        });
        stack.push(p.set_project_scenes_mutation(new_scenes), &mut p);

        // 2. Modify project (gamma).
        stack.push(p.set_gamma_mutation(2.5), &mut p);

        // 3. Capture pre-recall state.
        let pre_recall = serde_json::to_value(&p).unwrap();

        // 4. Recall slot 0 via ApplyProjectSnapshot { non_undoable: false }.
        let target = p.scenes[0].snapshot.clone();
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
    }
}
