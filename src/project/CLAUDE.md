# `src/project/` — schema, scene snapshots, v3 mutations

Load-bearing invariants live here. These are silent-corruption traps, not stylistic preferences — read this before editing.

## Scene snapshots (`mod.rs`)

- `Project::save` writes via temp file + atomic `rename` (Unix-atomic; a future Windows port will need `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`).
- `snapshot(&project)` is the lossless `serde_json::Value` form used for scene slots.
- **`restore_scene` ≠ `restore`.** `restore_scene` preserves `project.scenes` and `project.crossfade_duration_s`; `restore` overwrites them. Both are session-level state — a naïve restore wipes the slot list and surprises the operator mid-show. Regression tests guard this: `recall_preserves_other_slots`, `restore_scene_preserves_crossfade_duration`. Do not "simplify" by collapsing the two functions.
- `interpolate(a, b, t)` blends snapshots field-by-field: numbers blend linearly, equal-length arrays/objects recurse, everything else snaps at `t = 0.5` (so categorical changes like `BlendMode::Normal -> Add` flip cleanly).
- `interpolate` is only safe when **`snapshots_share_layer_topology(a, b)` returns true**. Structural mismatches (different layer counts, different `kind` per index) must snap instantly so per-layer GPU state stays consistent with `project.layers`. Any new code path that interpolates between snapshots must gate on this function.

## v3: typed mutations + undo (`command.rs`, `undo.rs`)

**V31.3.2 (landed) — all `Mutation` variants use `ReverseStorage`**. Pattern (A) — enum-of-structs: each `Mutation` variant has a payload struct implementing the `ReverseStorage` trait (see `command.rs`). The three rules below are now enforced at the type level — a new symmetric variant won't compile until its `impl ReverseStorage` is written. A `compile_fail` doctest on `ReverseStorage` itself (in `command.rs`) demonstrates the trait-bound enforcement using the real symbol (run with `cargo test --features v3 --doc`).

**Asymmetric exception:** `AddLayer`, `RemoveLayer`, `AddLayerMaskVertex`, and `RemoveLayerMaskVertex` are intentionally kept as inline match arms in `Mutation::apply` (not wrapped in `ReverseStorage` structs). Their Reverse crosses variant boundaries (e.g. `AddLayer`'s reverse is `RemoveLayer`), making `fn apply(self, …) -> Self` impossible without changing the trait signature to return `Mutation`, which would defeat the per-variant compile-time guarantee. These four are the documented exceptions.

The undo system has three **mandatory Reverse-storage rules**. Get them wrong and projects silently corrupt on undo. The proptest harness (`project::command::tests::proptest_round_trip`) is the runtime safety net, but it cannot enumerate every future variant — adding a new `Mutation` variant means re-applying these rules by hand:

1. **Whole-enum Reverse.** Variant replacements (`Modulator`, `BlendMode`, `Effect`, `LayerKind`, `FitMode`) store the *full* old enum value, not just the field that "looks" different. Variant-replacement loses unrelated fields silently otherwise.

2. **Effects-Vec Reverse.** Anything touching `LayerConfig.effects` snapshots the entire `Vec<Effect>`, not just the changed effect. Reason: the `mutate_transform_effect` helper in `windows/scene_editor.rs` *appends* a default `Effect::Transform` to layers that don't have one — a per-field Reverse would leave a stray effect on undo.

3. **Snapshot Reverse.** Scene recall and crossfade tick replace the entire project from a `serde_json::Value`. They emit `Mutation::ApplyProjectSnapshot { new, old, non_undoable }`. Crossfade ticks fire ~60×/s and **must** set `non_undoable: true` so they never enter the user-facing undo stack.

Every `Mutation::apply` opens with a `debug_assert!` that the carried `old` value matches the project's *current* state — stale Reverse panics in test/debug builds, compiles out in release. Use `Project::set_*_mutation(...)` constructors when migrating UI call sites (T-003-T1.18+); they capture `old` automatically so contributors can't forget to snapshot the pre-mutation state.

## Command vs Mutation — do not collapse

`controls::Command` (an input event from keyboard / MIDI / OSC) and `project::command::Mutation` (a typed project state transition) are **deliberately separate** during the v3 migration. Input events are session-scoped side-effects; mutations are project-scoped reversible state changes. They will converge in a later refactor — do not pre-empt it without a written plan.

## Schema additions

- Numeric fields default through serde; if a new optional field is added with a non-zero "identity" value (e.g. scale = 1.0, not 0.0), set the serde default explicitly. The repo has been bitten by `transform.scale = [0.0, 0.0]` collapsing layers to invisible; defaults need to round-trip to identity, not zero.
- Bump `CURRENT_SCHEMA_VERSION` and add a step to `migrate.rs` for any breaking change. Old projects must continue to load.
