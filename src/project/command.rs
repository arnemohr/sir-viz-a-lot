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

use crate::project::schema::{BlendMode, LayerConfig, Project};

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
    /// Replace `Project.crossfade_duration_s`. Same shape as `SetGamma`.
    SetCrossfadeDurationS {
        /// Value to write.
        new: f32,
        /// Pre-mutation value.
        old: f32,
    },
    /// Replace `Project.output_windowed`. Boolean toggle.
    SetOutputWindowed {
        /// Value to write.
        new: bool,
        /// Pre-mutation value.
        old: bool,
    },
    /// Replace `WarpMesh.mask_feather` for the warp at `warp_idx`.
    SetWarpMaskFeather {
        /// Index into `Project.warps`.
        warp_idx: usize,
        /// Value to write.
        new: f32,
        /// Pre-mutation value.
        old: f32,
    },
    /// Replace `WarpMesh.rows`/`cols`/`grid` for the warp at `warp_idx`.
    /// Editing rows or cols bilinear-resamples the grid (the existing
    /// T-M7-01 helper); the resample is lossy, so this variant follows
    /// Reverse rule 3 — the `old_grid` snapshot lets undo restore the
    /// pre-mutation grid byte-equally instead of attempting a reverse-
    /// resample.
    SetWarpDimensions {
        /// Index into `Project.warps`.
        warp_idx: usize,
        /// Cell-row count to write.
        new_rows: u32,
        /// Cell-column count to write.
        new_cols: u32,
        /// Resampled grid to install (caller pre-computes via
        /// [`crate::project::schema::resample_grid`] so this can be a
        /// snapshot Reverse without per-step bookkeeping).
        new_grid: Vec<Vec<[f32; 2]>>,
        /// Pre-mutation rows.
        old_rows: u32,
        /// Pre-mutation cols.
        old_cols: u32,
        /// Pre-mutation grid (full snapshot — see Reverse rule 3).
        old_grid: Vec<Vec<[f32; 2]>>,
    },

    /// Replace `LayerConfig.opacity` for the layer at `layer_idx`.
    SetLayerOpacity {
        /// Index into `Project.layers`.
        layer_idx: usize,
        /// Value to write.
        new: f32,
        /// Pre-mutation value.
        old: f32,
    },
    /// Replace `LayerConfig.enabled` for the layer at `layer_idx`.
    SetLayerEnabled {
        /// Index into `Project.layers`.
        layer_idx: usize,
        /// Value to write.
        new: bool,
        /// Pre-mutation value.
        old: bool,
    },
    /// Replace `LayerConfig.blend_mode` for the layer at `layer_idx`.
    /// Whole-enum Reverse (rule 1): stores the full old `BlendMode` value.
    SetLayerBlendMode {
        /// Index into `Project.layers`.
        layer_idx: usize,
        /// Value to write.
        new: BlendMode,
        /// Pre-mutation value (full enum — Reverse rule 1).
        old: BlendMode,
    },

    /// Insert `layer` at `position`. Reverse is `RemoveLayer { idx: position }`.
    /// The whole `LayerConfig` is stored — covers Reverse rule 1 (LayerKind
    /// enum) automatically.
    AddLayer {
        /// The layer to insert.
        layer: LayerConfig,
        /// Insertion index (0..=project.layers.len()).
        position: usize,
    },
    /// Remove the layer at `idx`. Reverse is `AddLayer { layer, position: idx }`
    /// where `layer` is captured during apply via `Vec::remove`.
    RemoveLayer {
        /// Index into `Project.layers`.
        idx: usize,
    },
    /// Swap the layers at `i` and `j`. Self-reverse.
    SwapLayers {
        /// First swap index.
        i: usize,
        /// Second swap index.
        j: usize,
    },

    /// Replace the entire project from a serde_json snapshot
    /// (Reverse rule 3: snapshot Reverse). T-003-T1.30 routes
    /// scene-recall and crossfade-tick through this variant.
    /// `non_undoable: true` is reserved for the crossfade-tick
    /// path which fires ~60×/s and must not enter the
    /// user-facing undo stack.
    ApplyProjectSnapshot {
        /// Snapshot to install.
        new: serde_json::Value,
        /// Project state captured before the apply call.
        old: serde_json::Value,
        /// `true` for crossfade-tick callers; `false` for
        /// user-triggered scene recall.
        non_undoable: bool,
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
            Mutation::SetCrossfadeDurationS { new, old } => {
                debug_assert!(
                    (project.crossfade_duration_s - old).abs() < 1e-6,
                    "SetCrossfadeDurationS stale Reverse: project.crossfade_duration_s={}, expected old={}",
                    project.crossfade_duration_s,
                    old
                );
                project.crossfade_duration_s = new;
                Mutation::SetCrossfadeDurationS { new: old, old: new }
            }
            Mutation::SetOutputWindowed { new, old } => {
                debug_assert!(
                    project.output_windowed == old,
                    "SetOutputWindowed stale Reverse: project.output_windowed={}, expected old={}",
                    project.output_windowed,
                    old
                );
                project.output_windowed = new;
                Mutation::SetOutputWindowed { new: old, old: new }
            }
            Mutation::SetWarpMaskFeather { warp_idx, new, old } => {
                let warp = project
                    .warps
                    .get_mut(warp_idx)
                    .expect("SetWarpMaskFeather: warp_idx out of range");
                debug_assert!(
                    (warp.mask_feather - old).abs() < 1e-6,
                    "SetWarpMaskFeather stale Reverse: warp.mask_feather={}, expected old={}",
                    warp.mask_feather,
                    old
                );
                warp.mask_feather = new;
                Mutation::SetWarpMaskFeather {
                    warp_idx,
                    new: old,
                    old: new,
                }
            }
            Mutation::SetWarpDimensions {
                warp_idx,
                new_rows,
                new_cols,
                new_grid,
                old_rows,
                old_cols,
                old_grid,
            } => {
                let warp = project
                    .warps
                    .get_mut(warp_idx)
                    .expect("SetWarpDimensions: warp_idx out of range");
                // Snapshot Reverse: only assert the scalar dimensions
                // match. The grid is restored byte-equally via the
                // stored snapshot, so any drift surfaces in the
                // proptest round-trip rather than per-mutation.
                debug_assert!(
                    warp.rows == old_rows && warp.cols == old_cols,
                    "SetWarpDimensions stale Reverse: warp dims=({}, {}), expected old=({}, {})",
                    warp.rows,
                    warp.cols,
                    old_rows,
                    old_cols
                );
                // Standard swap (rule 3 snapshot variant): the Reverse
                // restores `old_grid` and tracks the just-installed
                // `new_grid` as its own pre-apply state. Cloning
                // `new_grid` keeps both halves owned so the Reverse
                // round-trips without referencing `warp.grid` after
                // the in-place write.
                let post_grid = new_grid;
                warp.grid = post_grid.clone();
                warp.rows = new_rows;
                warp.cols = new_cols;
                Mutation::SetWarpDimensions {
                    warp_idx,
                    new_rows: old_rows,
                    new_cols: old_cols,
                    new_grid: old_grid,
                    old_rows: new_rows,
                    old_cols: new_cols,
                    old_grid: post_grid,
                }
            }
            Mutation::SetLayerOpacity {
                layer_idx,
                new,
                old,
            } => {
                let layer = project
                    .layers
                    .get_mut(layer_idx)
                    .expect("SetLayerOpacity: layer_idx out of range");
                debug_assert!(
                    (layer.opacity - old).abs() < 1e-6,
                    "SetLayerOpacity stale Reverse: layer.opacity={}, expected old={}",
                    layer.opacity,
                    old
                );
                layer.opacity = new;
                Mutation::SetLayerOpacity {
                    layer_idx,
                    new: old,
                    old: new,
                }
            }
            Mutation::SetLayerEnabled {
                layer_idx,
                new,
                old,
            } => {
                let layer = project
                    .layers
                    .get_mut(layer_idx)
                    .expect("SetLayerEnabled: layer_idx out of range");
                debug_assert!(
                    layer.enabled == old,
                    "SetLayerEnabled stale Reverse: layer.enabled={}, expected old={}",
                    layer.enabled,
                    old
                );
                layer.enabled = new;
                Mutation::SetLayerEnabled {
                    layer_idx,
                    new: old,
                    old: new,
                }
            }
            Mutation::SetLayerBlendMode {
                layer_idx,
                new,
                old,
            } => {
                let layer = project
                    .layers
                    .get_mut(layer_idx)
                    .expect("SetLayerBlendMode: layer_idx out of range");
                debug_assert!(
                    layer.blend_mode == old,
                    "SetLayerBlendMode stale Reverse: layer.blend_mode={:?}, expected old={:?}",
                    layer.blend_mode,
                    old
                );
                layer.blend_mode = new;
                Mutation::SetLayerBlendMode {
                    layer_idx,
                    new: old,
                    old: new,
                }
            }
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
                let layer = project.layers.remove(idx);
                Mutation::AddLayer {
                    layer,
                    position: idx,
                }
            }
            Mutation::SwapLayers { i, j } => {
                debug_assert!(
                    i < project.layers.len() && j < project.layers.len(),
                    "SwapLayers index out of range: i={}, j={}, len={}",
                    i,
                    j,
                    project.layers.len()
                );
                project.layers.swap(i, j);
                Mutation::SwapLayers { i, j }
            }
            Mutation::ApplyProjectSnapshot {
                new,
                old,
                non_undoable,
            } => {
                // Snapshot Reverse (rule 3): the previous
                // serialised project. Use restore_scene to
                // overwrite project state in-place.
                let _ = crate::project::restore_scene(project, &new);
                Mutation::ApplyProjectSnapshot {
                    new: old,
                    old: new,
                    non_undoable,
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
            Mutation::SetGamma { .. }
            | Mutation::SetBrightness { .. }
            | Mutation::SetContrast { .. }
            | Mutation::SetCrossfadeDurationS { .. }
            | Mutation::SetOutputWindowed { .. }
            | Mutation::SetWarpMaskFeather { .. }
            | Mutation::SetWarpDimensions { .. }
            | Mutation::SetLayerOpacity { .. }
            | Mutation::SetLayerEnabled { .. }
            | Mutation::SetLayerBlendMode { .. }
            | Mutation::AddLayer { .. }
            | Mutation::RemoveLayer { .. }
            | Mutation::SwapLayers { .. } => false,
            Mutation::ApplyProjectSnapshot { non_undoable, .. } => *non_undoable,
        }
    }

    /// Whether the renderer's per-layer GPU state is invalidated by this
    /// mutation. Layer-topology mutations (AddLayer / RemoveLayer / SwapLayers)
    /// invalidate the `state.layers` Vec; field-edit mutations don't —
    /// they touch project fields the renderer reads each frame.
    ///
    /// The app's undo / redo dispatch and the pending-mutation drain inspect
    /// this flag to decide whether to call `rebuild_layers_for_state` after
    /// the mutation lands.
    pub fn needs_layer_rebuild(&self) -> bool {
        matches!(
            self,
            Mutation::AddLayer { .. } | Mutation::RemoveLayer { .. } | Mutation::SwapLayers { .. }
        )
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

    /// Build a `SetCrossfadeDurationS` mutation.
    pub fn set_crossfade_duration_s_mutation(&self, new: f32) -> Mutation {
        Mutation::SetCrossfadeDurationS {
            new,
            old: self.crossfade_duration_s,
        }
    }

    /// Build a `SetOutputWindowed` mutation.
    pub fn set_output_windowed_mutation(&self, new: bool) -> Mutation {
        Mutation::SetOutputWindowed {
            new,
            old: self.output_windowed,
        }
    }

    /// Build a `SetWarpMaskFeather` mutation. Panics if `warp_idx` is
    /// out of range — call sites should guard with `project.warps.get`
    /// first; the helper is intentionally not optional so the contract
    /// stays unambiguous.
    pub fn set_warp_mask_feather_mutation(&self, warp_idx: usize, new: f32) -> Mutation {
        let warp = &self.warps[warp_idx];
        Mutation::SetWarpMaskFeather {
            warp_idx,
            new,
            old: warp.mask_feather,
        }
    }

    /// Build a `SetWarpDimensions` mutation. The new grid is computed
    /// here via [`crate::project::schema::resample_grid`] so callers
    /// don't have to reason about the lossy resample — they just pass
    /// the new cell counts.
    pub fn set_warp_dimensions_mutation(
        &self,
        warp_idx: usize,
        new_rows: u32,
        new_cols: u32,
    ) -> Mutation {
        let warp = &self.warps[warp_idx];
        let new_grid = crate::project::schema::resample_grid(&warp.grid, new_rows, new_cols);
        Mutation::SetWarpDimensions {
            warp_idx,
            new_rows,
            new_cols,
            new_grid,
            old_rows: warp.rows,
            old_cols: warp.cols,
            old_grid: warp.grid.clone(),
        }
    }

    /// Build a `SetLayerOpacity` mutation. Panics if `layer_idx` is
    /// out of range.
    pub fn set_layer_opacity_mutation(&self, layer_idx: usize, new: f32) -> Mutation {
        let layer = &self.layers[layer_idx];
        Mutation::SetLayerOpacity {
            layer_idx,
            new,
            old: layer.opacity,
        }
    }

    /// Build a `SetLayerEnabled` mutation. Panics if `layer_idx` is
    /// out of range.
    pub fn set_layer_enabled_mutation(&self, layer_idx: usize, new: bool) -> Mutation {
        let layer = &self.layers[layer_idx];
        Mutation::SetLayerEnabled {
            layer_idx,
            new,
            old: layer.enabled,
        }
    }

    /// Build a `SetLayerBlendMode` mutation. Whole-enum Reverse (rule 1):
    /// captures the full old `BlendMode` value. Panics if `layer_idx` is
    /// out of range.
    pub fn set_layer_blend_mode_mutation(&self, layer_idx: usize, new: BlendMode) -> Mutation {
        let layer = &self.layers[layer_idx];
        Mutation::SetLayerBlendMode {
            layer_idx,
            new,
            old: layer.blend_mode,
        }
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
        Mutation::SwapLayers { i, j }
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
        if p.layers.is_empty() {
            use std::path::PathBuf;
            p.layers.push(crate::project::schema::layer_from_svg_path(
                "test_layer",
                PathBuf::from("/tmp/rmap_test.svg"),
            ));
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
            WarpDimensions { rows: u32, cols: u32 },
            Snapshot,
            LayerOpacity(f32),
            LayerEnabled(bool),
            LayerBlendMode(BlendMode),
            AddLayer,
            RemoveLayer,
            SwapLayers,
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
                MutationKind::WarpMaskFeather(v) => project.set_warp_mask_feather_mutation(0, *v),
                MutationKind::WarpDimensions { rows, cols } => {
                    project.set_warp_dimensions_mutation(0, *rows, *cols)
                }
                MutationKind::Snapshot => {
                    // Build a snapshot mutation against the
                    // project's current state, with `new` flipping
                    // gamma so the mutation is observable.
                    let old = serde_json::to_value(project).unwrap();
                    let mut next = project.clone();
                    next.gamma += 0.1;
                    let new = serde_json::to_value(&next).unwrap();
                    Mutation::ApplyProjectSnapshot {
                        new,
                        old,
                        non_undoable: false,
                    }
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
            ]
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
