# 004 Phase 2 — task breakdown

Companion task spec for [`004-phase-2.md`](004-phase-2.md). Each task
below is sized for a single PR.

## Implementation status (2026-05-12)

- [x] P2.1.1 12b5dcd — glossary entries for Phase 2 domain terms
- [x] P2.1.2 62822eb — perf-gate refresh: 4× ripple_wash stub fixture
- [x] P2.1.3 f777c7e — CHANGELOG + README v0.6 placeholders
- [x] P2.2.1 8430b00 — FxPresetRegistry skeleton
- [x] P2.2.2 d423cec — FxParamDescriptor API
- [x] P2.2.4 9b7d58a — audit: UnknownFxPreset + UnknownTreatment findings (notes lib.rs parallel-render-tree workaround — see follow-up task in TaskList)
- [x] P2.3.1 0a73d7a — sample_sdf_normal helper + fx_ in SDF_CONSUMERS
- [x] P2.2.3 00b3dac — registry-driven FX dispatch (move ripple_wash arm out of app.rs)
- [x] P2.3.2 5d441eb — FxShaderInputs canonical bind-group contract + optional source/SSBO
- [x] P2.4.1 cdd7da8 — displacement_ripple Treatment preset
- [x] P2.4.2 63b63c9 — refraction Treatment preset
- [x] P2.4.3 8d499d8 — mask_edge_wave_wash FxLayer preset (FxPipelines wrapper)

---

## Operating model

- **Model:** Sonnet implements; Opus reviews. Same read-the-spec-first rule as
  Phase 1: read the originating spec section, read every CLAUDE.md the task
  touches, write the test alongside the implementation, run `make ci` before
  committing.
- **Pick one task at a time.** Read the source section it references in
  `004-phase-2.md` and the corresponding entry in `specs/roadmap.md` before
  starting.
- **Commit message format:** `004-P2.<workstream>.<task>: <title>` — e.g.
  `004-P2.2.1: FxPresetRegistry skeleton`.
- **Branching:** one branch per task; merge straight to `main` once CI is
  green.
- **Pre-commit hook** (`.githooks/pre-commit`, wired by `make setup`) runs
  rustfmt on staged files + `cargo check`. Heavier checks live in `make ci`;
  run that before opening a PR.
- **Tests:** every task ships with new or updated tests. For schema / Mutation /
  snapshot work, follow the v3 proptest pattern in `src/project/command.rs`. For
  render-path work, add a golden under `tests/golden/` (covered by `--features
  gpu-tests`); use `UPDATE_GOLDEN=1` to (re-)record the baseline. Where
  automation isn't possible (manual preset-browser UX, drag-reorder gesture),
  ship a manual smoke-test checklist — never nothing.
- **Read the right CLAUDE.md.** Tasks touching `src/project/` must read
  `src/project/CLAUDE.md` first (Mutation Reverse-storage rules, snapshot
  invariants). Tasks touching `src/render/` must read `src/render/CLAUDE.md`
  first (GPU lifecycle, panic_restore, build-time WGSL validation).
- **Don't bundle.** If a task tempts you to also fix something nearby, resist —
  that "something nearby" probably already has its own task ID below.
- **GPU bring-up tasks ship golden images.** Anything that touches
  `src/render/` and renders pixels needs a `tests/golden/` baseline added under
  `--features gpu-tests`; `UPDATE_GOLDEN=1` rewrites the baseline.
- **Preset architecture mirrors P0.5.x + P1.2.2.** Each FX preset is a
  four-file change: shader (`src/render/shaders/fx_*.wgsl`), pipeline
  constructor on `FxPresetPipeline` or `FxComputePipeline`, preset-id constant
  in `src/render/fx_presets.rs`, and dispatch arm in the registry function. Same
  recipe as `mask_edge_ripple_wash`.

## Task ID conventions

- IDs are flat-numbered within ten workstreams:
  - W1 — Setup + housekeeping (glossary, perf-gate refresh, CHANGELOG placeholder)
  - W2 — FX preset registry foundation (registry, descriptors, dispatch refactor, audit)
  - W3 — SDF-aware effect inputs (SDF normal helper, bind-group contract doc)
  - W4 — Wave / displacement preset family (three leaf presets)
  - W5 — Particle compute infrastructure + leaf presets + budget enforcement
  - W6 — Fluid family (advection infra + one bounded-fluid preset)
  - W7 — Effect-chain reordering (M7 follow-on)
  - W8 — Preset library UI + export (I2 follow-on)
  - W9 — Snapshot / determinism / undo
  - W10 — Release housekeeping + Phase 2 acceptance smoke test
- Tasks reference Phase 0 / Phase 1 precedents by their task ID where the
  pattern is reused (e.g. "mirrors P1.2.1 `SetLayerTreatmentParams` shape").

## Workstream summary

| WS | Theme | Tasks | Parallel-safe? | Touches |
|----|-------|-------|----------------|---------|
| 1 | Setup + housekeeping | 3 | All three parallel-safe | `src/windows/glossary.rs`, `tests/perf_frame_budget.rs`, `CHANGELOG.md`, `README.md` |
| 2 | FX preset registry foundation | 4 | P2.2.1 first; P2.2.2 + P2.2.3 + P2.2.4 serial after | `src/render/fx_presets.rs`, `src/app.rs:3805`, `src/project/audit.rs` |
| 3 | SDF-aware effect inputs | 2 | P2.3.1 first; P2.3.2 after | `src/render/shaders/sdf_helper.wgsl`, `src/render/fx_presets.rs` |
| 4 | Wave / displacement presets | 3 | P2.4.1 + P2.4.2 (Treatments) parallel after W3.1; P2.4.3 (FxLayer) parallel after W2.3 + W3 | new `src/render/shaders/treat_displacement_ripple.wgsl`, `treat_refraction.wgsl` (Treatments); new `fx_edge_wave_wash.wgsl` + `src/render/fx_presets.rs` (FxLayer) |
| 5 | Particle compute + presets + budget | 6 | P2.5.1 first; P2.5.2–P2.5.5 parallel; P2.5.6 parallel with P2.5.2–P2.5.5 | new `src/render/fx_compute.rs`, `src/render/shaders/fx_particles_*.wgsl`, `src/project/command.rs`, `src/project/schema.rs` |
| 6 | Fluid family | 2 | P2.6.1 first; P2.6.2 after | new `src/render/fx_fluid.rs`, `src/render/shaders/fx_fluid_*.wgsl` |
| 7 | Effect-chain reordering | 3 | All three parallel-safe after W2 lands | `src/windows/control_panel.rs:1648+`, `src/effects/registry.rs` |
| 8 | Preset library UI + export | 5 | P2.8.1 first; rest serial | new `src/windows/preset_browser.rs`, `src/render/fx_presets.rs`, `src/project/schema.rs` |
| 9 | Snapshot / determinism / undo | 2 | P2.9.1 after W5 lands; P2.9.2 needs gpu-tests | `src/project/command.rs`, `tests/headless_gpu.rs` |
| 10 | Release housekeeping + acceptance smoke | 5 | Last — depends on everything else | `Cargo.toml`, `CHANGELOG.md`, `README.md`, `docs/show-day-checklist.md` |

**Suggested PR sequencing:**

1. **P2.1.1 + P2.1.2 + P2.1.3** in parallel — quick independent wins.
2. **P2.2.1** (registry skeleton) — unblocks every preset in W4 / W5 / W6.
3. **P2.2.2 + P2.2.3 + P2.2.4** (descriptor API + dispatch refactor + audit) serial after P2.2.1.
4. **P2.3.1 + P2.3.2** (SDF normal + bind-group contract) in parallel with P2.2.2–P2.2.4.
5. **P2.4.1 + P2.4.2** (Treatment presets) parallel after P2.3.1 lands;
   **P2.4.3** (FxLayer preset) parallel after P2.2.3 + P2.3.2 land.
6. **P2.5.1** (particle compute infra + identity preset) — gates P2.5.2–P2.5.5.
7. **P2.5.2 + P2.5.3 + P2.5.4 + P2.5.5** in parallel; **P2.5.6** (budget enforcement) also parallel with this batch.
8. **P2.6.1** (fluid advection infra) after P2.5.1; **P2.6.2** after P2.6.1.
9. **P2.7.1 + P2.7.2 + P2.7.3** (effect-chain reorder UI) parallel with W4/W5 — independent code path.
10. **P2.8.1** (browser modal) after W2 lands; **P2.8.2 → P2.8.5** serial after P2.8.1.
11. **P2.9.1 + P2.9.2** after W5 + W6 land.
12. **P2.10.1 → P2.10.5** last; P2.10.5 runs the acceptance smoke against
    the v0.6 release candidate.

## Anticipated risks

These eight design decisions are locked — they were approved in the planning
phase. Each is a potential scope-creep site; call it out at task time if
implementation pressure pushes toward a different choice.

1. **FX presets are monolithic.** No user-visible stage recomposition
   (emitter / force-field / render). Each preset = one shader + one pipeline +
   one dispatch arm. The low-level emitter / particle graph editor is
   explicitly out of scope per `specs/roadmap.md` §11. Internal stages are an
   implementation detail of each preset, not user-visible recombinable units.

2. **Effect-chain reordering scope = `Vec<Effect>` only.** Drag-reorder the
   existing Color / Blur / Transform / External chain on all layer types. FX
   preset internal stages are NOT reorderable. W7 owns this work; do not extend
   reordering into FX-internal structure.

3. **Particle determinism = seed + time-offset.** Snapshot stores
   `(seed: u64, t_layer_added_secs: f32)`; particles are recomputed from the
   seed every frame in the compute shader. ~8 bytes per layer in the snapshot.
   Full particle state serialisation (for crossfade-interpolation) is deferred
   to Phase 4 if needed.

4. **Particle budget enforced at mutation time.** Each particle preset declares
   `max_particle_count` in its descriptor. The `SetFxLayerParams` mutation
   validates and refuses to commit when over-budget; UI shows an inline warning
   toast. Mutation does not commit; the project state does not change.

5. **GPU SDF normal computed in-shader.** Extend `sdf_helper.wgsl` with
   `sample_sdf_normal()` via finite differences against the existing R32Float
   SDF texture. No new GPU texture is allocated; the gradient is 4 texel
   fetches.

6. **New `SetFxLayerParams` mutation** (whole-HashMap snapshot Reverse, mirrors
   P1.2.1's `SetLayerTreatmentParams`). Preset *switches* still go through
   `SetLayerKind`. Slider drags dispatch `SetFxLayerParams` so the undo stack
   records one entry per gesture, not a full `LayerKind` churn.

7. **Preset library storage:** built-in presets compiled in (no on-disk
   distribution); user presets at
   `~/Library/Application Support/rmap/presets/*.rmap-preset.json`; star state
   at `~/Library/Application Support/rmap/preset_stars.json`. Delete only
   applies to user presets; built-ins are read-only.

8. **One PR per preset.** Each Wave / Particle / Fluid preset is its own leaf
   task (~four-file change: shader + pipeline constructor + preset-id constant +
   dispatch arm). Do NOT bundle a family into one task. This is the scope-creep
   guard for W4, W5, and W6.

---

## Workstream 1 — Setup + housekeeping

Quick independent wins that ship before the heavier workstreams.

### P2.1.1 — Glossary entries for Phase 2 domain terms

**Source:** `004-phase-2.md` Capability set (FX preset families: particle,
wave, fluid); Engine implications ("emitter stage", "force-field stage",
"render stage", "mask-constrained particle effects", "emitter masking");
roadmap §"FX preset library".
**Type:** docs / UX
**Depends on:** none
**Files:** `src/windows/glossary.rs` (existing `GlossaryTerm` enum).

**What:** Phase 2 introduces a cluster of GPU / VFX terms that operators will
see in the preset library browser, the control panel, and error toasts. Adding
glossary entries before those UI surfaces ship means W4–W8 tasks can wire
`glossary_label(ui, GlossaryTerm::X)` calls without waiting on a separate
docs task. Pattern is identical to P1.1.3 — extend the `GlossaryTerm` enum
with new variants and add short (~30 word) operator-facing definitions to the
match arm.

Two groups of terms:

**Domain terms (15):** *particle*, *force field*, *fluid sim*, *preset
library*, *mask-constrained*, *emitter masking*, *SDF normal*,
*displacement preset*, *refraction preset*, *wave preset*, *particle
budget*, *seed (determinism)*, *effect-chain reorder*, *user preset*,
*built-in preset*.

**Built-in preset display labels (8):** *mask-edge ripple wash* (existing
from v0.4 — confirm an entry exists; add if not), *mask-edge wave wash*,
*mask-constrained drift*, *mask-edge emission*, *mask field flow*,
*mask collision reflection*, *mask-bounded fluid*, *displacement ripple*,
*refraction*. Phase 1 explicitly punted these to Phase 2
(`004-phase-1-tasks.md` P1.1.3 "Out of scope: glossary entries for FX
preset names (Phase 2 owns the FX library)") — this task closes that
deferral.

**Steps:**
1. Read `src/windows/glossary.rs:491` — locate the `GlossaryTerm` enum, the
   display match, and `EXPECTED_VARIANT_COUNT` (currently 39 after Phase 1).
2. Add one enum variant per term listed above (~24 terms total: 15 domain +
   ~9 preset labels — count `mask_edge_ripple_wash` only if not already
   present from Phase 0).
3. Write a short definition (~30 words) for each in the display match arm.
   Definitions explain what the operator sees / controls, not the
   implementation.
4. Bump `EXPECTED_VARIANT_COUNT` from 39 to the new total.

**Tests:**
- Unit test: the existing glossary exhaustiveness test covers all new variants
  (it will fail to compile otherwise if the pattern-match is exhaustive).
- Manual: hover each new label when it appears in the W4–W8 UIs; confirm
  the popover shows.

**Acceptance:**
- [ ] All domain terms + all built-in preset display labels have
      `GlossaryTerm` variants and definitions.
- [ ] `EXPECTED_VARIANT_COUNT` bumped to match.
- [ ] Existing exhaustiveness tests still pass.
- [ ] Definitions are operator-facing copy, not implementation notes.
- [ ] `make ci` clean.

**Out of scope:** Phase 7 terms (luma key, chroma key, inverse mask).

---

### P2.1.2 — Show-day perf-gate refresh: 4-layer particle scene

**Source:** `004-phase-2.md` Acceptance criteria ("Particle counts per layer
are enforced to keep the show-day frame budget"); roadmap §"Show-day
reliability".
**Type:** engine (defensive)
**Depends on:** none (sets up the baseline; actual particle tasks land later)
**Files:** `tests/perf_frame_budget.rs`.

**What:** the existing perf gate (`tests/perf_frame_budget.rs`, wired in
P0.9.5) validates a representative scene against a p99 frame-time target on the
M-series baseline. Phase 1 deferred a proper refresh (P1.7.4 was not fully
shipped). Phase 2 needs a new fixture scenario: a scene with four
`FxLayer` items at maximum particle budget — which doesn't exist yet, so this
task writes the test structure and a stub fixture that will be populated by
P2.5.1. The fixture today substitutes a maximally-parametrised `ripple_wash`
FxLayer (four of them); the test asserts p99 within the 16.6 ms target on the
M-series baseline. When P2.5.1 lands, the fixture is updated in-place to use
real particle layers.

**Steps:**
1. Read `tests/perf_frame_budget.rs` — understand the existing test structure
   (frame-render loop, sample count, p99 computation, skip conditions).
2. Add a new test function `perf_four_fx_layers_within_budget` that builds a
   four-layer scene, each an `FxLayer { preset_id: RIPPLE_WASH_PRESET_ID,
   params: <max amplitude> }`, and asserts p99 ≤ 16.6 ms.
3. Mark the test with `#[cfg(feature = "gpu-tests")]` and the appropriate skip
   condition (no adapter available).
4. Document in a comment that the fixture will be updated in P2.5.1 to use
   particle presets at max budget.
5. Record the current M-series baseline in a comment (`perf_baseline_ms: f64`).

**Tests:**
- GPU test (`--features gpu-tests`): runs on hardware with a wgpu adapter;
  skipped otherwise. CI does not provide an adapter — this is an explicit
  skip condition.
- The test itself is the deliverable; there are no secondary tests for a test
  file.

**Acceptance:**
- [ ] New `perf_four_fx_layers_within_budget` test exists under
      `--features gpu-tests`.
- [ ] Baseline M-series result documented in a comment.
- [ ] Test skips cleanly when no GPU adapter is available (matches existing
      skip pattern).
- [ ] `make ci` clean.

**Out of scope:** populating the fixture with real particle layers (P2.5.1);
the Path-A refactor of the perf gate's internal render-path stub (carried from
P1.7.4).

---

### P2.1.3 — CHANGELOG + README Phase 2 placeholder section

**Source:** `004-phase-2.md` Goal ("Promote masks from visibility shapes to
effect sources").
**Type:** docs / UX
**Depends on:** none
**Files:** `CHANGELOG.md`, `README.md`.

**What:** drop a shell section for v0.6 in both files so W10 tasks only need to
fill body text rather than establish document structure. No version bump yet
(that's P2.10.1). CHANGELOG gets an unreleased section header. README gets a
stub paragraph for the FX preset library. Pattern mirrors what was done for
v0.5 before Phase 1 shipped.

**Steps:**
1. In `CHANGELOG.md`, add an `## [Unreleased] — v0.6` section above the v0.5
   entry with three placeholder subsections: `### FX Preset Library`,
   `### Effect-Chain Reordering`, `### Particle / Wave / Fluid Families`.
2. In `README.md`, add a "FX Preset Library (v0.6)" subsection under the
   Features list with a one-sentence placeholder.
3. Do not change any version strings; those are owned by P2.10.1.

**Tests:**
- No automated tests for documentation files.
- Manual: verify `CHANGELOG.md` and `README.md` render correctly with a
  Markdown previewer.

**Acceptance:**
- [ ] `CHANGELOG.md` has an `[Unreleased] — v0.6` header with placeholder
      subsections.
- [ ] `README.md` has a stub FX Preset Library entry.
- [ ] No version strings changed.
- [ ] `make ci` clean.

**Out of scope:** filling the CHANGELOG body (P2.10.2); README prose
(P2.10.3); version bump (P2.10.1).

---

## Workstream 2 — FX preset registry foundation

The architectural workstream. Introduces the registry + descriptor API that
every W4 / W5 / W6 preset depends on, and migrates the hard-coded dispatch arm
in `app.rs` to the new pattern.

### P2.2.1 — `FxPresetRegistry` skeleton

**Source:** `004-phase-2.md` Capability set ("Real preset library"); Engine
implications ("FX layers need a richer pipeline with an emitter stage...").
**Type:** engine
**Depends on:** none
**Files:** `src/render/fx_presets.rs`, `src/app.rs:3805`.

**What:** today the only FX preset (`mask_edge_ripple_wash`) is built directly
into `FxPresetPipeline::new_ripple_wash` with no registry. The `treatments.rs`
module ships a `registry()` function that returns `Vec<(preset_id, label)>`
and an `is_registered()` predicate — Phase 2 mirrors this shape for FX. This
task introduces the `FxPresetRegistry` type (or equivalent free functions
following `treatments.rs`'s style) and registers `mask_edge_ripple_wash` as
its sole initial entry. No behavior change; the existing render path is
untouched.

The registry must accommodate three preset families with different pipeline
shapes: fragment-only (Wave), compute + render (Particle), compute + render
(Fluid). The design choice for v0.6: a `FxFamily` enum tag on each descriptor
entry tells the dispatch layer which pipeline branch to use. This tag is
internal — the operator sees only `(preset_id, display_label)`.

**Steps:**
1. Read `src/render/treatments.rs:136` — internalise the static-slice
   pattern: `pub fn registry() -> &'static [(&'static str, &'static str)]`.
   FX mirrors this shape (static slice, not `Vec`) so the registry is
   zero-allocation at call sites.
2. Add `pub fn fx_registry() -> &'static [FxPresetEntry]` to
   `src/render/fx_presets.rs` where `FxPresetEntry` holds
   `{ preset_id: &'static str, label: &'static str, family: FxFamily }`.
   `FxFamily` is `Fragment | ComputeParticle | ComputeFluid`.
3. Register `mask_edge_ripple_wash` as the first entry with
   `family: FxFamily::Fragment`.
4. Add `pub fn fx_is_registered(preset_id: &str) -> bool` and
   `pub fn fx_display_label(preset_id: &str) -> Option<&'static str>`.
5. No change to the existing hard-coded dispatch in `app.rs:3805` — that
   migration is P2.2.3.

**Tests:**
- Unit test: `fx_registry()` contains `RIPPLE_WASH_PRESET_ID`.
- Unit test: `fx_is_registered(RIPPLE_WASH_PRESET_ID)` returns `true`;
  `fx_is_registered("bogus")` returns `false`.
- Unit test: `fx_display_label(RIPPLE_WASH_PRESET_ID)` returns `Some(_)`.

**Acceptance:**
- [ ] `fx_registry()` returns `&'static [FxPresetEntry]`;
      `fx_is_registered()` and `fx_display_label()` exist in
      `src/render/fx_presets.rs`.
- [ ] `mask_edge_ripple_wash` appears in the registry with `FxFamily::Fragment`.
- [ ] Existing render path produces bit-exact identical output (no behaviour
      change).
- [ ] `make ci` clean.

**Out of scope:** `FxParamDescriptor` API (P2.2.2); dispatch refactor
(P2.2.3); audit (P2.2.4).

---

### P2.2.2 — `FxParamDescriptor` API

**Source:** `004-phase-2.md` Capability set ("Real preset library with
browser, search").
**Type:** engine
**Depends on:** P2.2.1.
**Files:** `src/render/fx_presets.rs`.

**What:** Treatment presets ship `ParamDescriptor` entries (`{ key, label, min,
max, default }`) so the UI can render sliders without hardcoding ranges.
FX presets today have no equivalent — the UI exposes params via a raw number
input. This task adds `FxParamDescriptor` (paralleling `treatments.rs`'s
descriptor type) and populates it for `mask_edge_ripple_wash`. A new optional
field `max_particle_count: Option<u32>` appears on the descriptor for particle
family presets; fragment presets leave it `None`.

The descriptor table is the source of truth for P2.5.6's budget enforcement:
the mutation checks `descriptor.max_particle_count` against the params value.

**Steps:**
1. Define `pub struct FxParamDescriptor { pub key: &'static str, pub label:
   &'static str, pub min: f32, pub max: f32, pub default: f32, pub
   max_particle_count: Option<u32> }`. The `max_particle_count` field is only
   meaningful when `key == "particle_count"` or similar — document this
   convention.
2. Add `pub fn fx_param_descriptors(preset_id: &str) -> Vec<FxParamDescriptor>`
   to `src/render/fx_presets.rs`.
3. Populate the descriptor for `mask_edge_ripple_wash` (wave amplitude,
   wave speed, wave count, decay — verify against the actual shader params in
   `src/render/shaders/fx_ripple_wash.wgsl`).
4. A preset with no registered descriptor returns an empty `Vec` (defensive).

**Tests:**
- Unit test: `fx_param_descriptors(RIPPLE_WASH_PRESET_ID)` returns a non-empty
  list.
- Unit test: each descriptor's `min < max` and `default` is in `[min, max]`.
- Unit test: `fx_param_descriptors("bogus")` returns an empty Vec without
  panic.

**Acceptance:**
- [ ] `FxParamDescriptor` type exists with all five fields.
- [ ] `mask_edge_ripple_wash` descriptor is populated and valid.
- [ ] `max_particle_count: None` for fragment family presets.
- [ ] `make ci` clean.

**Out of scope:** dispatch refactor (P2.2.3); `SetFxLayerParams` mutation
(P2.5.6); particle budget validation (P2.5.6).

---

### P2.2.3 — Dispatch refactor: registry-driven

**Source:** `004-phase-2.md` Engine implications; roadmap §"FX preset
library".
**Type:** render
**Depends on:** P2.2.1.
**Files:** `src/app.rs:3805` (hard-coded FX dispatch arm),
`src/render/fx_presets.rs`.

**What:** the current dispatch in `app.rs:3805` is a hard-coded
`if preset_id == RIPPLE_WASH_PRESET_ID { ... }`. Adding W4 / W5 / W6 presets
to this arm would create an unbounded chain of `if`/`else if` blocks that must
all live in `app.rs`. This task replaces that arm with a call to
`fx_presets::dispatch(preset_id, ...)` whose implementation lives in
`src/render/fx_presets.rs`. The existing ripple-wash render path must produce
bit-exact identical output — verified by a golden test.

The `dispatch` function returns a `bool` (like `treatments::dispatch`) so the
caller can fall through to the "unknown preset / no-op" path.

**Steps:**
1. Add `pub fn dispatch(preset_id: &str, inputs: &FxShaderInputs, ...) -> bool`
   to `src/render/fx_presets.rs`. The signature mirrors `treatments::dispatch`;
   consult `src/render/treatments.rs` for the full shape.
2. Move the ripple-wash pipeline call from `app.rs:3805` into the dispatch arm
   inside `fx_presets::dispatch`.
3. Replace the `if preset_id == RIPPLE_WASH_PRESET_ID` block in `app.rs:3805`
   with `if !fx_presets::dispatch(preset_id, ...)`.
4. Add a `FxShaderInputs` struct (or extend `TreatmentInputs` equivalent) for
   the inputs that every FX preset receives (SDF view, clock buffer, params
   uniform). Document the slot assignment in module rustdoc.
5. Add an unknown-preset arm that returns `false`; existing audit (P2.2.4)
   warns the operator.

**Tests:**
- Golden test (`--features gpu-tests`): ripple-wash rendered through the new
  dispatch path produces bit-exact identical output to the pre-P2.2.3 baseline.
- Unit test: `dispatch(RIPPLE_WASH_PRESET_ID, ...)` returns `true`.
- Unit test: `dispatch("bogus", ...)` returns `false` without panic.

**Acceptance:**
- [ ] `app.rs:3805` no longer contains a hard-coded `if preset_id ==
      RIPPLE_WASH_PRESET_ID` block.
- [ ] `fx_presets::dispatch` handles `mask_edge_ripple_wash`.
- [ ] Golden-test baseline confirms bit-exact identical output.
- [ ] `make ci` clean.

**Out of scope:** audit for unknown presets (P2.2.4); new preset families
(W4–W6).

---

### P2.2.4 — Audit: `AuditKind::UnknownFxPreset` + retrofit `UnknownTreatment`

**Source:** `004-phase-2.md` Engine implications ("An FxLayer that carries an
unknown preset_id is left invisible; the audit emits a warning").
**Type:** engine (defensive)
**Depends on:** P2.2.1.
**Files:** `src/project/audit.rs:407` (existing treatment audit
placeholder), `src/render/fx_presets.rs` (registry lookup), optionally
`src/render/treatments.rs` (export `is_registered` to the audit crate).

**What:** a project loaded with an unrecognised `FxLayer.preset_id` silently
renders nothing (the dispatch returns `false`). The operator has no
indication why. **This is the first unknown-preset audit in the codebase** —
Phase 1 deliberately punted (`audit.rs:419` says *"(1) unknown preset_id —
placeholder until W3 ships"*); only empty preset_id strings produce a Warn
finding today. The reason Phase 1 punted: there was no registry the audit
could consult. P2.2.1 ships that registry, which unblocks this task.

While we're here, retrofit the same lookup against Treatments — the
infrastructure is symmetric (`treatments::is_registered()` already exists,
shipped in Phase 1). A one-line audit extension closes the placeholder
Phase 1 left behind.

**Steps:**
1. Read `src/project/audit.rs:407+` — understand the existing treatment
   audit block (empty-preset_id check; "placeholder until W3 ships" comment).
2. Add `AuditKind::UnknownFxPreset { layer_idx: usize, preset_id: String }`
   variant (Severity::Warn — operator may be running an older project with
   a preset that exists in a newer build).
3. Add `AuditKind::UnknownTreatment { layer_idx, preset_id }` variant
   (same severity; retrofit for the Phase 1 placeholder).
4. In the audit pass, iterate layers: for `FxLayer { preset_id, .. }`, call
   `fx_presets::fx_is_registered(preset_id)`; if false, emit
   `UnknownFxPreset`. For `LayerConfig.treatment.preset_id`, call
   `treatments::is_registered(...)`; if false, emit `UnknownTreatment`.
5. Confirm severity is Warn (not Critical) — the layer is invisible but the
   project still loads. Operator gets an actionable finding without losing
   the rest of their scene.

**Tests:**
- Unit test: `FxLayer { preset_id: "definitely_fake", .. }` produces an
  `UnknownFxPreset` Warn finding.
- Unit test: `Treatment { preset_id: "definitely_fake", .. }` produces an
  `UnknownTreatment` Warn finding.
- Unit test: valid `preset_id = RIPPLE_WASH_PRESET_ID` produces neither
  finding.
- Unit test: the existing empty-preset_id treatment Warn (Phase 1) continues
  to fire.

**Acceptance:**
- [ ] Both `AuditKind::UnknownFxPreset` and `AuditKind::UnknownTreatment`
      variants exist.
- [ ] Audit uses `fx_is_registered()` + `treatments::is_registered()`
      (no hardcoded string lists).
- [ ] Severity is Warn for both.
- [ ] The Phase 1 placeholder comment in `audit.rs:419` is removed.
- [ ] `make ci` clean.

**Out of scope:** missing-asset audit for FX overlay textures (not applicable
in v0.6; FxLayer parameters are all `f32`); audit for over-budget particle
counts (P2.5.6).

---

## Workstream 3 — SDF-aware effect inputs

Extends `sdf_helper.wgsl` so W4 / W5 / W6 shaders can read the mask's
geometric derivatives without each shader reimplementing the same finite-
difference stencil.

### P2.3.1 — `sample_sdf_normal()` in `sdf_helper.wgsl`

**Source:** `004-phase-2.md` Engine implications ("distance, gradient, normal,
and signed distance to nearest edge are available to effect shaders as fragment
inputs"); "GPU SDF normal — in-shader computation" (Anticipated risk #5).
**Type:** render
**Depends on:** none (sdf_helper.wgsl is a standalone WGSL include).
**Files:** `src/render/shaders/sdf_helper.wgsl`, `build.rs` (SDF_CONSUMERS
table, to confirm `fx_` prefix is already listed).

**What:** the existing `sdf_helper.wgsl` exposes `sample_sdf_bilinear`,
`sample_sdf_gradient`, and `sample_sdf`. Missing: `sample_sdf_normal(t_sdf:
texture_2d<f32>, uv: vec2<f32>) -> vec2<f32>` — the normalised gradient
vector, pointing away from the nearest mask edge. Wave and particle presets
need this to orient particles or distort pixels along the mask boundary. The
function is implemented via finite differences (4 texel fetches) against the
existing R32Float SDF texture; no new GPU buffer is required.

CPU unit tests (not GPU golden tests) are feasible for this: the analytic case
for a circle SDF has `normal(uv) ≈ normalize(uv - center)` which can be
approximated with a hand-crafted R32Float buffer.

**Steps:**
1. Read `src/render/shaders/sdf_helper.wgsl` — understand the existing helper
   function conventions (offset step size, coordinate system).
2. Add `fn sample_sdf_normal(t_sdf: texture_2d<f32>, uv: vec2<f32>) ->
   vec2<f32>` using a central-difference stencil:
   `(sample(uv + dx) - sample(uv - dx), sample(uv + dy) - sample(uv - dy))`,
   then `normalize`. Document the epsilon / step parameter.
3. Confirm `build.rs` SDF_CONSUMERS includes the `fx_` prefix so any new
   `fx_*.wgsl` shader that imports the helper gets the prepend at compile time.
   If not, add it.
4. Write CPU unit tests that construct a synthetic circle SDF buffer
   (pixel data), evaluate `sample_sdf_normal` equivalently in Rust, and
   compare against the analytic radial normal to within a tolerance.

**Tests:**
- Unit test (CPU): analytic circle SDF → `sample_sdf_normal` returns a
  vector within 5% of the expected radial direction at 4 cardinal UV
  positions.
- Build-time: naga validates the updated `sdf_helper.wgsl` as part of
  `cargo build` (existing `build.rs` mechanism).

**Acceptance:**
- [ ] `sample_sdf_normal` exists in `sdf_helper.wgsl` and compiles through
      naga.
- [ ] CPU unit test verifies the analytic circle case within tolerance.
- [ ] `build.rs` SDF_CONSUMERS includes `fx_` prefix (add if missing).
- [ ] `make ci` clean.

**Out of scope:** exposing normal as a texture (the in-shader computation is
sufficient — Anticipated risk #5); adding normals to the warp pipeline (Phase
7 / Phase 2 scope cut).

---

### P2.3.2 — `FxShaderInputs` bind-group contract doc + helper struct

**Source:** `004-phase-2.md` "SDF-aware effect inputs" capability stanza.
**Type:** render
**Depends on:** P2.3.1.
**Files:** `src/render/fx_presets.rs` (module-level rustdoc).

**What:** every FX preset shader takes the same canonical bind-group slots.
Today that contract is implicit (ripple_wash happens to use bindings 0–3: SDF
texture, sampler, params uniform, clock uniform). Phase 2 adds a source texture
(for displacement / refraction presets that read the underlying layer content)
and a compute SSBO (for particle presets). Rather than let each preset reinvent
the slot numbering, this task formalises the contract in a `FxShaderInputs`
struct and documents the canonical slot assignment in module rustdoc.

Canonical slot assignment (documented in module rustdoc):
- Binding 0: SDF texture (R32Float, unfilterable)
- Binding 1: sampler (NonFiltering)
- Binding 2: `FxParamsUniform` (8 × f32)
- Binding 3: clock uniform
- Binding 4: source texture (Rgba8UnormSrgb; fragment presets only)
- Binding 5: particle SSBO (compute presets only; fragment presets leave unbound)

**Steps:**
1. Define `pub struct FxShaderInputs<'a>` in `src/render/fx_presets.rs`
   with fields for each canonical slot (mirrors `TreatmentInputs<'a>` from
   `src/render/treatments.rs`).
2. Update `fx_presets::dispatch` (from P2.2.3) to accept `&FxShaderInputs`
   instead of individual arguments.
3. Update the existing ripple-wash pipeline call inside dispatch to use the
   struct fields.
4. Write module-level rustdoc on `src/render/fx_presets.rs` documenting the
   slot assignment, the struct shape, and the invariant that all Phase 2
   presets must use these slots.

**Tests:**
- Compile check: `dispatch` now takes `&FxShaderInputs`; the single existing
  call site in `app.rs` must be updated and must compile.
- Unit test: `FxShaderInputs` can be constructed with dummy field values
  without panicking (smoke test of the struct's `Default`-like builder if
  one is added).

**Acceptance:**
- [ ] `FxShaderInputs<'a>` struct exists in `src/render/fx_presets.rs`.
- [ ] Module rustdoc documents the canonical slot assignment.
- [ ] `dispatch` accepts `&FxShaderInputs`; existing call site updated.
- [ ] `make ci` clean.

**Out of scope:** per-preset bind-group construction (each preset task owns
its own bind-group setup); adding a second sampler slot (not needed for v0.6
presets).

---

## Workstream 4 — Wave / displacement preset family

Three leaf presets, split across two subsystems by what each preset does to
the pixel:

- **Self-illuminated waves** belong on `FxLayer` (a *source* layer that
  produces its own pixels from mask + clock alone). P2.4.3
  (`mask_edge_wave_wash`) lands here, alongside the existing
  `mask_edge_ripple_wash`.
- **Mask-driven distortions of underlying content** belong as **Treatments**
  (which already run on Image/Video layers and already carry SDF inputs via
  `TreatmentInputs::sdf` since P1.3.2). P2.4.1 (`displacement_ripple`) and
  P2.4.2 (`refraction`) land here. From a VJ workflow perspective this is the
  better fit: refraction needs an underlying image to bend — drop a video,
  draw a mask, pick the treatment from the dropdown. One click less than the
  FxLayer path and works on real source content.

Each leaf is independent and follows the four-file change recipe: shader +
pipeline constructor + preset-id constant + dispatch arm. Treatments use
Phase 1's recipe (`treat_*.wgsl` + `treatments.rs` registry); the FxLayer
preset uses Phase 2's W2 recipe.

### P2.4.1 — `displacement_ripple` Treatment preset

**Source:** `004-phase-2.md` Capability set / FX preset families / Wave
("mask-driven displacement"). Architectural placement: Treatment (not
FxLayer) — see W4 intro. Mirrors P1.3.2's `blur_mask` Treatment shape (also
SDF-gated, runs on underlying Image/Video content).
**Type:** render (shader + pipeline)
**Depends on:** P2.3.1 (`sample_sdf_normal`). **Does NOT depend on W2** —
this is a Treatment, not an FxLayer preset.
**Files:** new `src/render/shaders/treat_displacement_ripple.wgsl`,
`src/render/treatments.rs` (pipeline constructor + preset-id constant +
dispatch arm + descriptor entry), `build.rs` (confirm `treat_displacement`
prefix is in SDF_CONSUMERS so `sdf_helper.wgsl` is auto-prepended).

**What:** a Treatment that runs on Image or Video layers and distorts the
underlying texture along the mask boundary — pixels near the mask edge are
sampled from a displaced UV, producing a "glass lens at the window edge"
look. The displacement vector follows the SDF normal (away from the nearest
mask edge) and is modulated by a sinusoid keyed to SDF distance. Far from
the edge, displacement decays to zero so the centre of the masked region is
untouched. Three params: `amplitude` (displacement magnitude in UV units,
0..=0.05), `frequency` (spatial frequency of the ripple, 1..=20), `decay`
(how quickly displacement falls off with SDF distance, 0..=1).

This is the **practical operator path** for refraction-style effects on
real source content: drop a video, draw a mask around the part you want to
distort (a window, a portal, an architectural opening), pick this Treatment
from the dropdown. Treatments already carry `source` and `sdf` bindings via
`TreatmentInputs` — no new GPU plumbing required.

**Steps:**
1. Read `src/render/shaders/treat_blur_mask_h.wgsl` (P1.3.2) for the
   SDF-aware Treatment shader pattern. Reuse the same bind-group layout:
   source, sampler, SDF, params uniform.
2. Write `treat_displacement_ripple.wgsl`: sample `sdf_normal` at the
   fragment's UV, compute displaced UV
   `= uv + normal * amplitude * sin(sdf_dist * frequency * TAU) *
   smoothstep(0, decay_band, sdf_dist)`, then `textureSample(t_source,
   displaced_uv)`.
3. Add `pub const DISPLACEMENT_RIPPLE_PRESET_ID: &str =
   "displacement_ripple"` to `src/render/treatments.rs`.
4. Pipeline constructor: build with the single-pass treatment helper
   (`build_single_pass_treatment` / `draw_single_pass_treatment` — shared by
   identity + tone_map + luminance_reveal in Phase 1).
5. Register the preset in `treatments::registry()` and populate
   `param_descriptors()`.
6. Confirm `build.rs` SDF_CONSUMERS includes `treat_displacement` prefix
   (already includes `treat_blur` per P1.3.2; add this one if missing).

**Tests:**
- Golden test (`--features gpu-tests`): a fixture polygon mask + a checker-
  board source produces a visible displacement ramp at the edge matching a
  saved baseline.
- Unit test: `treatments::param_descriptors(DISPLACEMENT_RIPPLE_PRESET_ID)`
  returns 3 entries with valid min/max/default ranges.
- Unit test: identity-default case (`amplitude = 0`) produces bit-exact
  passthrough of the source.
- Build-time: naga validates `treat_displacement_ripple.wgsl`.

**Acceptance:**
- [ ] WGSL parses and validates via `build.rs` naga.
- [ ] Preset registered in `treatments::registry()` and visible in the
      Treatment dropdown on Image/Video layers.
- [ ] Golden baseline added under `tests/golden/`.
- [ ] `amplitude = 0` produces passthrough (identity-default rule).
- [ ] `make ci` clean.

**Out of scope:** animated displacement (time-varying amplitude — add
`speed` param in a follow-up if needed); applying displacement to FxLayer
output (FxLayer is a source layer; Treatments only run on Image/Video).

---

### P2.4.2 — `refraction` Treatment preset

**Source:** `004-phase-2.md` Capability set / FX preset families / Wave
("refraction-style distortion"). Architectural placement: Treatment (not
FxLayer) — refraction is meaningless without an underlying image to bend
light through.
**Type:** render (shader + pipeline)
**Depends on:** P2.3.1 (`sample_sdf_normal`). **Does NOT depend on W2** —
this is a Treatment, not an FxLayer preset.
**Files:** new `src/render/shaders/treat_refraction.wgsl`,
`src/render/treatments.rs`, `build.rs` (confirm `treat_refraction` prefix is
in SDF_CONSUMERS).

**What:** a Treatment that bends pixel-rays at the mask boundary using a
Snell-like offset — the result reads as light bending through glass at the
mask edge. Distinct from `displacement_ripple` (P2.4.1): refraction has no
spatial-frequency oscillation, just a smooth steady bend whose magnitude is
controlled by `ior`. Two params: `ior` (index of refraction, 1.0..=2.0;
1.0 = no refraction = identity), `edge_width` (band around the edge where
refraction applies, 0.0..=0.3 normalised SDF distance).

VJ use case: project mapped onto a real window. Operator drops a video of
a cityscape, draws a polygon around the window outline, picks `refraction`
treatment — the city video bends at the window edge as if seen through
glass. Combined with `displacement_ripple` on a separate layer, this is the
"watery window" effect that audiences read as magical.

**Steps:**
1. Reuse the bind-group layout from `treat_blur_mask_*.wgsl` and
   `treat_displacement_ripple.wgsl` (P2.4.1).
2. Write `treat_refraction.wgsl`:
   `refracted_uv = uv + normal * (ior - 1.0) *
   smoothstep(0, edge_width, abs(sdf_dist))`, clamp to [0, 1], sample the
   source texture.
3. Add `pub const REFRACTION_PRESET_ID: &str = "refraction"`.
4. Pipeline constructor via `build_single_pass_treatment` helper.
5. Register in `treatments::registry()` and `param_descriptors()`.
6. Confirm `build.rs` SDF_CONSUMERS includes `treat_refraction` prefix.

**Tests:**
- Golden test (`--features gpu-tests`): checkerboard source through a
  circular mask produces visible edge-bending matching a saved baseline.
- Unit test: `treatments::param_descriptors(REFRACTION_PRESET_ID)` returns
  2 entries.
- Unit test: identity-default case (`ior = 1.0`) produces bit-exact
  passthrough of the source.
- Build-time: naga validates `treat_refraction.wgsl`.

**Acceptance:**
- [ ] WGSL parses and validates.
- [ ] Preset registered in `treatments::registry()` and visible in the
      Treatment dropdown on Image/Video layers.
- [ ] `ior = 1.0` produces passthrough (identity-default rule).
- [ ] Golden baseline added.
- [ ] `make ci` clean.

**Out of scope:** chromatic aberration (wavelength-dependent `ior` —
Phase 7); animated `ior` modulation (Phase 4 scene grammars); refraction
on FxLayer output (FxLayer is a source; Treatments only run on Image/Video).

---

### P2.4.3 — `mask_edge_wave_wash` preset

**Source:** `004-phase-2.md` Capability set / FX preset families / Wave
("mask-edge ripple wash"; plan doc distinguishes this from the existing
concentric ripple as a "traveling wave along the mask edge").
**Type:** render (shader + pipeline)
**Depends on:** P2.2.3, P2.3.1.
**Files:** new `src/render/shaders/fx_edge_wave_wash.wgsl`,
`src/render/fx_presets.rs`.

**What:** a fragment-only preset related to `mask_edge_ripple_wash` but
distinct in character: rather than concentric rings expanding from the edge,
this preset produces a wave that *travels along* the edge — the intensity
pattern at any point is a function of the arc-length position along the mask
boundary, not the radial distance from it. Requires using `atan2` on the SDF
gradient direction as a proxy for angular position around the mask. Three
params: `wave_speed` (0..=5), `wave_width` (0..=0.3, fraction of edge band
illuminated at once), `colour: f32` (0..=1, hue rotate of the wave emission
tint).

This preset does NOT read the source texture (binding 4 is unused); the output
is a self-illuminated wave overlay, like the existing ripple wash.

**Steps:**
1. Write `fx_edge_wave_wash.wgsl`: use `atan2(normal.y, normal.x)` as the
   angular position `phi`; compute wave intensity = `sin(phi * N_waves -
   clock * wave_speed)`. Apply SDF-edge gating (`smoothstep` to confine
   emission to a `wave_width` band around the edge).
2. Add `pub const EDGE_WAVE_WASH_PRESET_ID: &str = "mask_edge_wave_wash"`.
3. Add `FxPresetPipeline::new_edge_wave_wash`; bind group is the same as
   ripple_wash (SDF, sampler, params, clock — no source texture needed).
4. Register, dispatch, populate descriptors.

**Tests:**
- Golden test (`--features gpu-tests`): a circular mask at mid-clock produces
  a wave-band pattern matching a saved baseline.
- Unit test: descriptors for `mask_edge_wave_wash` return 3 entries.
- Build-time: naga validates `fx_edge_wave_wash.wgsl`.

**Acceptance:**
- [ ] WGSL parses and validates.
- [ ] Pipeline, constant, dispatch arm, descriptors present.
- [ ] Visual output is a traveling wave along the edge (distinct from
      concentric rings of `mask_edge_ripple_wash`).
- [ ] Golden baseline added.
- [ ] `make ci` clean.

**Out of scope:** multi-wave interference patterns (Phase 4+); MIDI-sync of
wave_speed (Phase 6).

---

## Workstream 5 — Particle compute infrastructure + presets + budget enforcement

The most complex workstream. P2.5.1 must land first; leaf presets (P2.5.2–
P2.5.5) are then parallel; P2.5.6 (budget enforcement) can land in parallel
with the leaf presets since it touches the mutation layer, not the shaders.

### P2.5.1 — Compute pipeline + SSBO scaffolding + `particles_identity` proof-point

**Source:** `004-phase-2.md` Engine implications ("GPU particle simulation:
compute-shader (or transform-feedback) approach with double-buffered particle
state"); "Particle determinism = seed + time-offset" (Anticipated risk #3).
**Type:** engine + render
**Depends on:** P2.2.3, P2.3.2.
**Files:** new `src/render/fx_compute.rs`, new
`src/render/shaders/fx_particles_identity.wgsl` (compute + vertex + fragment),
`src/project/schema.rs` (`LayerKind::FxLayer`), `src/project/command.rs`.

**What:** particle presets require a compute-shader pipeline that reads a
particle SSBO (double-buffered), updates positions / velocities, then a vertex
shader that emits quads per live particle, then a fragment shader that colours
them. None of this infrastructure exists; this task proves the
compute → vertex → fragment pipeline contract with an "identity" particle
preset: N particles spawned at a grid, sitting still (zero velocity), each
rendered as a 2×2 px dot.

Snapshot changes: `LayerKind::FxLayer` gains `seed: u64` (serde-default 0) and
`t_layer_added_secs: f32` (serde-default 0.0). The compute shader uses
`seed` to initialise particle positions deterministically; the clock offset
`clock_secs - t_layer_added_secs` gives the particle system's local time. See
Anticipated risk #3 for the rationale.

**Non-breaking schema change.** Both fields are added with `#[serde(default)]`,
so older v7 project files load unchanged with `seed = 0` and
`t_layer_added_secs = 0.0`. Per `src/project/CLAUDE.md` §"Schema additions",
this means **no `CURRENT_SCHEMA_VERSION` bump and no `migrate.rs` step is
required** — the change is purely additive. Mirrors the P1.2.1 pattern for
`LayerConfig.treatment`.

**Steps:**
1. Read `src/render/CLAUDE.md` — GPU bring-up split, per-frame render-graph
   order, and `panic_restore` wrapper. The compute dispatch must sit between
   per-layer rasterise and the existing effect chain.
2. Read `src/project/CLAUDE.md` — rule 1 (whole-enum Reverse) for the schema
   change; the `seed` + `t_layer_added_secs` additions to `FxLayer` are
   serde-defaulted so no migration is needed.
3. Add `seed: u64` and `t_layer_added_secs: f32` to `LayerKind::FxLayer` with
   `#[serde(default)]`.
4. Create `src/render/fx_compute.rs` with:
   - `FxComputePipeline`: owns the compute pipeline, two SSBOs (double-buffer),
     and the vertex + fragment pipelines for quad rendering.
   - `FxComputePipeline::new_particles_identity(device, ...)` constructor.
   - `dispatch_compute(encoder, ...)` + `draw_particles(render_pass, ...)`.
5. Write the three WGSL files:
   - `fx_particles_identity_compute.wgsl`: reads `seed`, computes grid
     positions, stores into the output SSBO. Zero velocity.
   - `fx_particles_vertex.wgsl`: reads position from SSBO, emits a 2×2 px
     quad.
   - `fx_particles_fragment.wgsl`: outputs a constant white colour.
6. Register `particles_identity` in `fx_registry()` with
   `family: FxFamily::ComputeParticle`.
7. Wire the compute dispatch into the per-frame render loop in `app.rs` for
   `FxFamily::ComputeParticle` layers (new branch alongside the existing
   fragment-only dispatch).

**Tests:**
- Golden test (`--features gpu-tests`): `particles_identity` with N=16 on a
  circular mask produces a grid of 16 white dots matching a saved baseline.
- Unit test: `LayerKind::FxLayer` with no `seed`/`t_layer_added_secs` fields
  in JSON loads with both defaulted to 0.
- Build-time: naga validates all three WGSL files.

**Acceptance:**
- [ ] Compute pipeline + double-buffered SSBO infrastructure exists in
      `src/render/fx_compute.rs`.
- [ ] `seed` and `t_layer_added_secs` added to `LayerKind::FxLayer` (non-
      breaking serde).
- [ ] `particles_identity` registered and renders 16 stationary dots.
- [ ] Golden baseline added.
- [ ] `make ci` clean.

**Out of scope:** particle physics (velocity, forces — leaf presets own those);
particle budget enforcement (P2.5.6); fluid infrastructure (P2.6.1 reuses the
compute scaffolding).

---

### P2.5.2 — `mask_constrained_drift` preset

**Source:** `004-phase-2.md` Capability set / Particle ("mask-constrained
drift"); "One PR per preset" (Anticipated risk #8).
**Type:** render (shader + pipeline)
**Depends on:** P2.5.1.
**Files:** new `src/render/shaders/fx_particles_drift.wgsl` (compute),
`src/render/fx_compute.rs` (constructor + dispatch arm),
`src/render/fx_presets.rs` (preset-id constant, registry entry, descriptors).

**What:** particles spawn anywhere inside the mask (SDF > 0), drift slowly in
a random direction derived from their seed, and despawn when they cross the
mask boundary (SDF ≤ 0) — then respawn at a random interior location. The
effect reads as slow smoke or dust drifting inside the masked region. Three
params: `particle_count` (1..=2048, capped at `max_particle_count` from the
descriptor), `drift_speed` (0.0..=0.05 UV/s), `particle_size` (0.5..=4.0 px).

**Steps:**
1. Write the compute shader: on each tick, compute the new position
   `= old_pos + normalize(rand_dir(seed, id)) * drift_speed * dt`. Sample
   the SDF at the new position; if SDF ≤ 0, respawn at a seeded interior
   location.
2. Add `pub const CONSTRAINED_DRIFT_PRESET_ID: &str = "mask_constrained_drift"`.
3. Add `FxComputePipeline::new_constrained_drift`.
4. Register, dispatch, populate `fx_param_descriptors` with
   `max_particle_count: Some(2048)` on the `particle_count` descriptor.

**Tests:**
- Golden test (`--features gpu-tests`): N=64 particles in a circular mask after
  100 compute ticks match a saved baseline (deterministic from `seed=42`).
- Unit test: descriptors include `max_particle_count: Some(2048)`.

**Acceptance:**
- [ ] Particles stay inside the mask boundary.
- [ ] `max_particle_count` in the descriptor is `Some(2048)`.
- [ ] Golden baseline added.
- [ ] `make ci` clean.

**Out of scope:** particle-particle collisions (Phase 4+); turbulence field
(a separate force-field preset).

---

### P2.5.3 — `mask_edge_emission` preset

**Source:** `004-phase-2.md` Capability set / Particle ("mask-edge emission");
"One PR per preset" (Anticipated risk #8).
**Type:** render (shader + pipeline)
**Depends on:** P2.5.1.
**Files:** new `src/render/shaders/fx_particles_edge_emission.wgsl`,
`src/render/fx_compute.rs`, `src/render/fx_presets.rs`.

**What:** particles spawn along the mask edge (SDF ≈ 0) and fly outward in the
direction of the SDF normal, decelerating as they travel. When they travel far
enough from the edge, they despawn and new ones spawn at the edge. The visual
reads as light or sparks emanating from the masked boundary. Three params:
`particle_count` (1..=1024), `emission_speed` (0.01..=0.15 UV/s),
`lifetime_secs` (0.5..=5.0).

**Steps:**
1. Write the compute shader: spawn at edge (SDF distance ≈ 0, random angular
   position); velocity = `sdf_normal * emission_speed`; integrate; age-out
   at `lifetime_secs`.
2. Add `pub const EDGE_EMISSION_PRESET_ID: &str = "mask_edge_emission"`.
3. Add `FxComputePipeline::new_edge_emission`.
4. Register, dispatch, populate descriptors with `max_particle_count:
   Some(1024)`.

**Tests:**
- Golden test (`--features gpu-tests`): N=32 particles spawned at the edge of
  a circle mask at clock=0 match a saved baseline.
- Unit test: descriptors include `max_particle_count: Some(1024)`.

**Acceptance:**
- [ ] Particles spawn at the mask edge and travel outward.
- [ ] `max_particle_count` is `Some(1024)`.
- [ ] Golden baseline added.
- [ ] `make ci` clean.

**Out of scope:** colour ramp over particle lifetime (a future `colour_over_life`
param); MIDI-triggered burst emission (Phase 6).

---

### P2.5.4 — `mask_field_flow` preset

**Source:** `004-phase-2.md` Capability set / Particle ("field-driven flow");
"One PR per preset" (Anticipated risk #8).
**Type:** render (shader + pipeline)
**Depends on:** P2.5.1, P2.3.1.
**Files:** new `src/render/shaders/fx_particles_field_flow.wgsl`,
`src/render/fx_compute.rs`, `src/render/fx_presets.rs`.

**What:** particles move according to the SDF gradient field. The SDF gradient
points away from the nearest mask edge, so particles either converge toward
the boundary or diverge away from it depending on a `flow_direction` sign
param. The effect reads as a flow following the contours of the mask. Three
params: `particle_count` (1..=2048), `flow_speed` (0.0..=0.1 UV/s),
`flow_direction` (−1.0 = inward / +1.0 = outward, clamped at extremes).

**Steps:**
1. Write the compute shader: velocity `= sample_sdf_gradient(t_sdf, pos) *
   flow_direction * flow_speed`. Integrate; respawn particles that leave the
   valid SDF domain.
2. Add `pub const FIELD_FLOW_PRESET_ID: &str = "mask_field_flow"`.
3. Add `FxComputePipeline::new_field_flow`.
4. Register, dispatch, populate descriptors with `max_particle_count:
   Some(2048)`.

**Tests:**
- Golden test (`--features gpu-tests`): N=64 particles in a gradient flow field
  at clock=2 match a saved baseline.
- Unit test: descriptor `flow_direction` range is `[-1.0, 1.0]`.

**Acceptance:**
- [ ] Particles follow the SDF gradient direction.
- [ ] `flow_direction` param clamps to valid range.
- [ ] Golden baseline added.
- [ ] `make ci` clean.

**Out of scope:** curl-noise field overlays (Phase 4+); multi-layer field
blending (Phase 5).

---

### P2.5.5 — `mask_collision_reflection` preset

**Source:** `004-phase-2.md` Capability set / Particle ("collision/reflection
at boundary"); "One PR per preset" (Anticipated risk #8).
**Type:** render (shader + pipeline)
**Depends on:** P2.5.1, P2.3.1.
**Files:** new `src/render/shaders/fx_particles_collision_reflection.wgsl`,
`src/render/fx_compute.rs`, `src/render/fx_presets.rs`.

**What:** particles move freely inside the mask and bounce elastically when they
reach the boundary. Reflection uses the SDF normal as the surface normal:
`v_reflected = v - 2 * dot(v, n) * n`. The effect reads as billiard-ball
physics inside the mask shape. Three params: `particle_count` (1..=512),
`speed` (0.01..=0.2 UV/s), `restitution` (0.5..=1.0; fraction of speed
retained after each bounce).

**Steps:**
1. Write the compute shader: move particle by `velocity * dt`; if SDF ≤ 0,
   push back to the boundary and reflect velocity using `sample_sdf_normal`.
2. Add `pub const COLLISION_REFLECTION_PRESET_ID: &str =
   "mask_collision_reflection"`.
3. Add `FxComputePipeline::new_collision_reflection`.
4. Register, dispatch, populate descriptors with `max_particle_count:
   Some(512)`.

**Tests:**
- Golden test (`--features gpu-tests`): N=16 particles in a square mask after
  200 ticks match a saved baseline (deterministic bounce from `seed=1`).
- Unit test: descriptor `restitution` default is between 0.5 and 1.0.

**Acceptance:**
- [ ] Particles reflect off the mask boundary.
- [ ] `max_particle_count` is `Some(512)`.
- [ ] Golden baseline added.
- [ ] `make ci` clean.

**Out of scope:** particle-particle collision (Phase 4+); gravity field
(a future param on `mask_field_flow`).

---

### P2.5.6 — Particle budget enforcement: `SetFxLayerParams` mutation + UI warning

**Source:** `004-phase-2.md` Acceptance criteria ("Particle counts per layer
are enforced to keep the show-day frame budget; over-budget configurations
refuse to commit with an inline warning"); "Particle budget — mutation-time
refusal" (Anticipated risk #4); "New `SetFxLayerParams` mutation" (Anticipated
risk #6).
**Type:** engine (schema + Mutation + UI)
**Depends on:** P2.2.2 (`FxParamDescriptor.max_particle_count`), P2.2.3.
**Files:** `src/project/command.rs`, `src/project/schema.rs`,
`src/windows/control_panel.rs:1648+` (or wherever the FX param UI is rendered).

**What:** today FX params (the `HashMap<String, f32>` on `FxLayer`) are
mutated via `SetLayerKind` (whole-enum Reverse). That works but is heavy —
a single slider drag churns the entire `LayerKind`. This task introduces
`SetFxLayerParams` (whole-HashMap snapshot Reverse, mirrors P1.2.1's
`SetLayerTreatmentParams`) for lightweight param edits. Preset switches still
use `SetLayerKind`.

The mutation's `apply` method validates the incoming params against the
preset's descriptor: if any `max_particle_count`-gated param would exceed
its limit, the mutation refuses to commit and returns an error. The UI
translates this refusal into an inline warning toast.

**Steps:**
1. Read `src/project/CLAUDE.md` rules 1 and 2 — `SetFxLayerParams` uses rule
   1 (whole-HashMap snapshot Reverse) matching `SetLayerTreatmentParams`.
2. Add `Mutation::SetFxLayerParams { layer_idx: usize, new: HashMap<String,
   f32>, old: HashMap<String, f32> }` to `src/project/command.rs`.
3. Implement `ReverseStorage` for `SetFxLayerParams`; `apply` must:
   - `debug_assert!` the carried `old` matches the current state.
   - Validate `new` against `fx_param_descriptors(preset_id)`: if any
     `max_particle_count` field would be exceeded, return an error variant
     without mutating the project.
4. Add builders on `Project`: `set_fx_layer_params_mutation(layer_idx,
   new_params)` which captures `old` automatically.
5. UI: wherever FX layer params are rendered (locate the control panel section
   for the selected FxLayer), dispatch `SetFxLayerParams` on slider
   drag-release. On refusal, show an inline warning toast: "Particle count
   exceeds budget (max: N)".
6. Extend the proptest harness in `command.rs` with `SetFxLayerParams`.

**Tests:**
- Proptest: `SetFxLayerParams` round-trip via `apply` covers both allowed and
  refused mutations.
- Unit test: a `SetFxLayerParams` mutation that exceeds `max_particle_count`
  returns an error and does not change project state.
- Unit test: `Project::set_fx_layer_params_mutation` captures the correct
  `old` value.
- Manual smoke: push the particle_count slider past the limit; verify the
  inline warning toast appears and the slider snaps back.

**Acceptance:**
- [ ] `SetFxLayerParams` mutation exists and implements `ReverseStorage`.
- [ ] Over-budget mutation refuses to commit (project state unchanged).
- [ ] UI shows an inline warning toast on refusal.
- [ ] Proptest harness covers `SetFxLayerParams`.
- [ ] `make ci` clean.

**Out of scope:** per-project global particle budget (sum across layers —
Phase 4+); budget warnings at project-load time (the audit can be extended
in a follow-up).

---

## Workstream 6 — Fluid family

A two-task workstream: advection infrastructure first, then the masked-fluid
preset. The compute scaffolding from W5 is reused.

### P2.6.1 — Fluid advection infrastructure + `fluid_identity` proof-point

**Source:** `004-phase-2.md` Out of scope ("Fluid sim with full Navier–Stokes
pressure projection — start with a simple advection + dissipation scheme").
**Type:** engine + render
**Depends on:** P2.5.1 (reuses compute scaffolding).
**Files:** new `src/render/fx_fluid.rs`, new
`src/render/shaders/fx_fluid_advect.wgsl` (compute),
`src/render/shaders/fx_fluid_identity.wgsl` (fragment),
`src/render/fx_presets.rs`.

**What:** grid-based fluid simulation requires a velocity field texture (e.g.
256×256 RG16Float) updated each frame by a compute shader implementing
advection + dissipation. This is NOT Navier–Stokes with pressure projection —
the scope cut is explicit. The advection kernel moves each cell's velocity
along the current field (semi-Lagrangian backtrack), then multiplies by a
dissipation factor to decay energy. The `fluid_identity` proof-point preset
renders the velocity field as colour (R = Vx, G = Vy), proving the
compute → render pipeline contract without needing a beautiful visual.

**Steps:**
1. Create `src/render/fx_fluid.rs` with:
   - `FxFluidPipeline`: owns the velocity texture (256×256 RG16Float), compute
     pipeline for advection, and a fragment pipeline for rendering.
   - `FxFluidPipeline::new_fluid_identity(device, ...)`.
   - `dispatch_fluid(encoder, ...)` + `draw_fluid(render_pass, ...)`.
2. Write `fx_fluid_advect.wgsl`: semi-Lagrangian advection (backtrack by
   `-velocity * dt`, bilinear sample, store result) + dissipation
   `* (1.0 - dissipation_rate * dt)`.
3. Write `fx_fluid_identity.wgsl`: map velocity field to colour.
4. Add `pub const FLUID_IDENTITY_PRESET_ID: &str = "fluid_identity"`.
5. Register in `fx_registry()` with `FxFamily::ComputeFluid`.
6. Wire the fluid dispatch into the per-frame render loop for
   `FxFamily::ComputeFluid` layers.

**Tests:**
- Golden test (`--features gpu-tests`): a circular initial velocity blob
  after 10 advection ticks matches a saved baseline.
- Build-time: naga validates both WGSL files.
- Unit test: velocity texture dimensions are 256×256 and format is RG16Float.

**Acceptance:**
- [ ] Velocity field texture + advection compute pipeline exist in
      `src/render/fx_fluid.rs`.
- [ ] `fluid_identity` renders velocity as colour.
- [ ] `FxFamily::ComputeFluid` dispatch wired in per-frame render loop.
- [ ] Golden baseline added.
- [ ] `make ci` clean.

**Out of scope:** Navier–Stokes pressure projection (explicitly out of scope
per `004-phase-2.md`); vorticity confinement (Phase 4+).

---

### P2.6.2 — `mask_bounded_fluid` preset

**Source:** `004-phase-2.md` Capability set / Fluid ("grid-based fluid sim
with mask as boundary, particles as visualisation"); "One PR per preset"
(Anticipated risk #8).
**Type:** render (shader + pipeline)
**Depends on:** P2.6.1, P2.5.1.
**Files:** new `src/render/shaders/fx_fluid_bounded.wgsl` (compute — extends
advection with SDF-derived no-slip boundary), `src/render/fx_fluid.rs`,
`src/render/fx_presets.rs`.

**What:** extends the fluid advection from P2.6.1 with a mask-constrained
no-slip boundary condition: velocity cells outside the mask (SDF ≤ 0) are
zeroed each tick, and boundary cells have their velocity reflected using the
SDF normal. Particles (reusing the SSBO infrastructure from P2.5.1) visualise
the flow by advecting with the velocity field. Two params:
`particle_count` (1..=512), `dissipation` (0.9..=1.0, higher = less decay).

**Steps:**
1. Write the extended compute shader: after standard advection, sample SDF at
   each cell; if SDF ≤ 0, set velocity to zero (no-slip). At boundary cells
   (|SDF| < epsilon), reflect velocity using `sample_sdf_normal`.
2. Particle visualisation: advect particles with bilinear-sampled velocity
   field; respawn particles that leave the mask.
3. Add `pub const BOUNDED_FLUID_PRESET_ID: &str = "mask_bounded_fluid"`.
4. Add `FxFluidPipeline::new_bounded_fluid`.
5. Register, dispatch, populate descriptors with `max_particle_count:
   Some(512)`.

**Tests:**
- Golden test (`--features gpu-tests`): a vortex seeded at the centre of a
  circular mask at clock=5 matches a saved baseline.
- Unit test: `max_particle_count` is `Some(512)` in the descriptor.
- Build-time: naga validates `fx_fluid_bounded.wgsl`.

**Acceptance:**
- [ ] Fluid stays inside the mask (no velocity outside the boundary).
- [ ] Particles advect with the fluid velocity field.
- [ ] `max_particle_count` is `Some(512)`.
- [ ] Golden baseline added.
- [ ] `make ci` clean.

**Out of scope:** multiple velocity emitters (Phase 4+); pressure projection
for divergence-free flow (explicitly out of scope).

---

## Workstream 7 — Effect-chain reordering (M7 follow-on)

M7 identified the fixed-order effect chain (Color → Blur → Transform →
External) as a UX ceiling. The reorder is purely a UI + mutation problem because
the render loop already iterates `Vec<Effect>` in order (`app.rs:4041`). No
render-path changes are needed.

### P2.7.1 — Drag-reorder UI on the effect chain

**Source:** `004-phase-2.md` Capability set / Layer + chain ("Effect-chain
reordering across all layer types"); roadmap M7 follow-on.
**Type:** UI
**Depends on:** P2.2.1 (confirms `SetLayerEffects` mutation exists — it was
shipped in Phase 0 / Phase 1).
**Files:** `src/windows/control_panel.rs:1648+`.

**What:** the Selected-layer effect chain in `control_panel.rs:1648+` renders
Color, Blur, Transform, External as fixed-position collapsing headers. Phase 2
makes the chain reorderable by wrapping each row in an egui drag-and-drop
interaction. Dropping a row at a new position dispatches `SetLayerEffects` with
the reordered `Vec<Effect>` — the existing wholesale snapshot Reverse handles
undo correctly (see `src/project/CLAUDE.md` rule 2). No render-loop change
needed: `app.rs:4041` already iterates `Vec<Effect>` in order.

**Steps:**
1. Read `src/windows/control_panel.rs:1648+` — understand the current
   hard-coded header rendering for each effect type.
2. Build a reorderable list using egui's drag-and-drop API (inspect existing
   drag sites in the codebase for the egui pattern currently in use).
3. On drop, construct the reordered `Vec<Effect>` and dispatch
   `SetLayerEffects { layer_idx, new: reordered_vec, old: original_vec }`.
4. Visual: a drag handle (≡ glyph) on the left of each effect row.
5. Applies to all layer types (Image, Video, SVG, FxLayer) — the effect chain
   is on `LayerConfig`, not per-kind.

**Tests:**
- Unit test: reordering a `vec![Color, Blur, Transform]` to
  `vec![Blur, Color, Transform]` via `SetLayerEffects` round-trips through
  the proptest harness (extends the existing proptest with this case).
- Manual smoke: drag Blur above Color on an Image layer; confirm the render
  loop applies Blur first; confirm undo restores the original order.

**Acceptance:**
- [ ] Effect chain rows are drag-reorderable in the Selected-layer panel.
- [ ] Drop dispatches `SetLayerEffects` with the new order.
- [ ] Undo restores the original order.
- [ ] Applies to all layer types that carry effects.
- [ ] `make ci` clean.

**Out of scope:** FX preset internal stage reordering (Anticipated risk #2);
per-effect enable/disable toggle (Phase 4+).

---

### P2.7.2 — Add / Remove buttons on the effect chain

**Source:** `004-phase-2.md` Capability set / Layer + chain ("Effect-chain
reordering across all layer types — effect chain becomes reorderable").
**Type:** UI
**Depends on:** P2.7.1 (logically extends the same UI section).
**Files:** `src/windows/control_panel.rs:1648+`.

**What:** operators can only add an effect today by editing project JSON.
This task adds + and × buttons to the effect chain UI. The + button opens a
dropdown listing available effect types; clicking one dispatches `SetLayerEffects`
with the new effect appended. The × button on each row dispatches
`SetLayerEffects` with that entry removed. Both operations use the existing
wholesale `Vec<Effect>` snapshot Reverse — no new mutation variant needed.

**Steps:**
1. Add an "Add effect" button below the effect chain list. On click, show a
   popup menu listing available `Effect` variants (Color, Blur, Transform,
   External). Selecting one appends a default-constructed effect and dispatches
   `SetLayerEffects`.
2. Add a × button at the right of each effect row. On click, remove that entry
   from the vec and dispatch `SetLayerEffects`.
3. Default-constructed effects should match the existing defaults used by
   `default_effect_chain()` in `src/effects/mod.rs` — consult that function
   for the correct default values.

**Tests:**
- Manual smoke: add a Blur effect to an Image layer that has none; confirm
  it appears in the chain and renders; undo removes it.
- Unit test: `SetLayerEffects` with an appended default `Effect::Blur` round-
  trips through the proptest harness.

**Acceptance:**
- [ ] "Add effect" button opens a dropdown with all `Effect` variants.
- [ ] × button removes the selected effect.
- [ ] Both operations are undoable via `SetLayerEffects`.
- [ ] Default values match `default_effect_chain()`.
- [ ] `make ci` clean.

**Out of scope:** per-effect parameter editors (already exist in the collapsing
headers shipped in Phase 0); adding effects via a separate preset search UI
(Phase 4+).

---

### P2.7.3 — Promote `Effect::External` to a first-class menu entry

**Source:** `004-phase-2.md` Capability set / Layer + chain; roadmap M7 notes
on external pass hooks.
**Type:** UI
**Depends on:** P2.7.2.
**Files:** `src/windows/control_panel.rs:1648+`, `src/effects/registry.rs`.

**What:** `Effect::External` exists in the enum and there is a hook in
`src/effects/registry.rs` for registering external pass implementations, but
the effect is not visible in the Add Effect dropdown added by P2.7.2 — it is
dead-coded behind an advanced flag. This task exposes it as a first-class entry
so operators and plugin authors can wire up external passes without editing JSON.
The UI for configuring an `Effect::External` (plugin identifier, pass params)
is already rendered in the existing collapsing header section.

**Steps:**
1. Read `src/effects/registry.rs` — understand the `ExternalPass` extension
   point and any existing guard that prevents it from appearing in the menu.
2. Add `Effect::External(ExternalPassConfig::default())` to the "Add effect"
   dropdown from P2.7.2.
3. When no external passes are registered, the menu entry is still visible but
   its collapsing header renders a note: "No external passes registered —
   connect a plugin to populate this entry." (Operator-facing copy.)
4. When passes are registered via `ExternalPassRegistry::register(id, pass)`
   (see `src/effects/registry.rs:75`), the collapsing header exposes the
   pass selector.

**Tests:**
- Unit test: the effects dropdown list includes `External` as an entry.
- Manual smoke: add an External effect with no registered passes; confirm the
  placeholder message appears; undo removes the effect.

**Acceptance:**
- [ ] `Effect::External` appears in the Add Effect dropdown.
- [ ] Empty-registry placeholder message is clear.
- [ ] Undo via `SetLayerEffects` works.
- [ ] `make ci` clean.

**Out of scope:** implementing an actual external pass (plugin authors own
that); external pass parameter schema (Phase 4+).

---

## Workstream 8 — Preset library UI + export (I2 follow-on)

Replaces the opaque "Apply / Reload" UI with a real preset browser. W8.1 is
the architectural task; the rest are serial enhancements.

### P2.8.1 — Preset library browser modal

**Source:** `004-phase-2.md` Capability set / Layer + chain ("Real preset
library with browser"); roadmap I2 follow-on.
**Type:** UI
**Depends on:** P2.2.1 (registry), P2.2.2 (descriptors).
**Files:** new `src/windows/preset_browser.rs`, `src/windows/control_panel.rs`
(trigger site), `src/app.rs` (dispatch path for `SetLayerKind` on preset
selection).

**What:** a modal overlay (egui `Window`) showing all registered FX presets in
a grid view. Each cell shows the preset's display label, its family tag
(Wave / Particle / Fluid), and a thumbnail (placeholder silhouette for v0.6 —
live golden images are Phase 4+). Clicking a cell dispatches `SetLayerKind` to
apply the preset to the currently selected FxLayer. Opening the browser on a
non-FxLayer shows an informational message ("Select an FX layer to pick a
preset").

The browser sources its entries from `fx_registry()` plus any user presets
found on disk at `~/Library/Application Support/rmap/presets/` (read-only at
open time; no live-watch in v0.6).

**Steps:**
1. Create `src/windows/preset_browser.rs` with `PresetBrowserWindow` struct
   and an `show(ctx, state, sender)` method.
2. Grid view: `ui.horizontal_wrapped(...)` with one cell per preset entry.
   Each cell shows label + family badge. Click → dispatch `SetLayerKind {
   layer_idx, new: LayerKind::FxLayer { preset_id, params: default_params,
   seed: <new random>, t_layer_added_secs: <current clock> }, old:
   current_kind }`.
3. Trigger: add a "Browse presets…" button to the Selected-layer FxLayer
   section in `control_panel.rs`. Opens the modal with the correct layer
   index in context.
4. Read user presets from `~/Library/Application Support/rmap/presets/`.
   Parse `.rmap-preset.json` files (schema from P2.8.5); show them in a
   separate "User presets" section of the grid.
5. **Remove the legacy "Apply / Reload" UI.** Locate the existing opaque
   preset_id text-input + "Apply"/"Reload" buttons in the Selected-layer
   FxLayer section of `control_panel.rs` and delete them. The new
   "Browse presets…" button is the sole entry point for changing an FxLayer's
   preset. Source spec is explicit: the preset library *replaces* the
   Apply/Reload pair — leaving both around creates a confusing dual entry
   point.

**Tests:**
- Unit test: `PresetBrowserWindow::collect_presets(registry, user_dir)` returns
  at least the built-in entries from `fx_registry()`.
- Manual smoke: open the browser on an FxLayer; click a preset; confirm the
  layer's `preset_id` updates and the control panel reflects the new params.

**Acceptance:**
- [ ] Preset browser modal opens from the Selected-layer FxLayer panel.
- [ ] All built-in presets appear in the grid.
- [ ] Clicking a preset dispatches `SetLayerKind` and updates the layer.
- [ ] Non-FxLayer selected → informational message shown instead of grid.
- [ ] Legacy "Apply / Reload" controls are removed from `control_panel.rs`.
- [ ] `make ci` clean.

**Out of scope:** live thumbnails (Phase 4+); sorting / filtering (P2.8.2).

---

### P2.8.2 — Search / filter in the preset browser

**Source:** `004-phase-2.md` Capability set ("browser, search").
**Type:** UI
**Depends on:** P2.8.1.
**Files:** `src/windows/preset_browser.rs`.

**What:** a single-line text input at the top of the modal filters the displayed
presets by substring match on `preset_id` and display label (case-insensitive).
Also: three toggle buttons (Wave / Particle / Fluid) filter by family tag.
The filtered view updates on every keystroke (no debounce needed — the list is
small).

**Steps:**
1. Add a `filter_query: String` field to `PresetBrowserWindow` state.
2. Add an `egui::TextEdit` single-line input at the top of the modal.
3. Add three toggle buttons for family filtering.
4. Filter the entry list: show entry if `filter_query` is empty or matches
   `preset_id.contains(&query)` or `label.contains(&query)`, AND family
   matches the selected toggles.

**Tests:**
- Unit test: filtering `"ripple"` from a list of all built-in preset IDs
  returns exactly the entries whose id or label contains that substring.
- Manual smoke: type "particle" in the filter; confirm only particle family
  presets appear.

**Acceptance:**
- [ ] Text filter narrows the grid in real-time.
- [ ] Family toggle buttons work independently and in combination.
- [ ] Clearing the filter restores the full grid.
- [ ] `make ci` clean.

**Out of scope:** full-text search across preset param names (Phase 4+);
saved search history (Phase 4+).

---

### P2.8.3 — Star / favourite in the preset browser

**Source:** `004-phase-2.md` plan; "Preset library storage" (Anticipated risk
#7).
**Type:** UI + engine
**Depends on:** P2.8.1.
**Files:** `src/windows/preset_browser.rs`, new `src/render/preset_stars.rs`
(or inline persistence helper).

**What:** a star toggle on each preset cell in the browser. Star state persists
to `~/Library/Application Support/rmap/preset_stars.json` — a flat JSON array
of starred `preset_id` strings. The browser shows starred presets at the top
of the grid. No mutation needed (star state is session-level preference, not
project state — it does not affect `LayerConfig` or the undo stack).

**Steps:**
1. Define a `PresetStars` helper that reads / writes
   `~/Library/Application Support/rmap/preset_stars.json`. Format: `{ "starred":
   ["preset_id_1", "preset_id_2"] }`.
2. `PresetStars::is_starred(preset_id) -> bool` and
   `PresetStars::toggle(preset_id)` (mutates in-memory + writes file).
3. In the browser grid, render a star icon (★ / ☆) on each cell. Click calls
   `PresetStars::toggle`. Starred entries sort to the top.
4. Load `PresetStars` on browser open; write on each toggle.

**Tests:**
- Unit test: `toggle` followed by `is_starred` returns true; second `toggle`
  returns false.
- Unit test: `PresetStars` serialises / deserialises round-trip correctly.
- Manual smoke: star a preset, close and reopen the browser; preset appears
  starred and at the top.

**Acceptance:**
- [ ] Star toggle persists across app restarts (written to `preset_stars.json`).
- [ ] Starred presets sort to the top of the browser grid.
- [ ] Star state does not affect the undo stack.
- [ ] `make ci` clean.

**Out of scope:** starred presets surfaced in a quick-pick toolbar (Phase 4
wishlist); shared star state across machines (not in scope for local-only v0.6).

---

### P2.8.4 — Save / delete user presets

**Source:** `004-phase-2.md` plan; "Preset library storage" (Anticipated risk
#7).
**Type:** UI + engine
**Depends on:** P2.8.1.
**Files:** `src/windows/preset_browser.rs`, `src/render/fx_presets.rs` (user
preset loader).

**What:** operators can save the current FxLayer's `preset_id + params` as a
named user preset. Saved presets appear in the "User presets" section of the
browser (P2.8.1). A × button on user preset cells deletes the on-disk file.
Built-in presets have no delete button (they are read-only — Anticipated risk
#7).

**Steps:**
1. "Save as preset…" button in the Selected-layer FxLayer section (near the
   "Browse presets…" button from P2.8.1). Opens a small name-entry dialog.
2. On confirm, write
   `~/Library/Application Support/rmap/presets/<slug>.rmap-preset.json` with
   the schema from P2.8.5. Slug is derived from the name (lowercase,
   spaces → underscores, strip non-ASCII).
3. "Delete" button on user preset cells in the browser. Prompts "Delete
   'name'? This cannot be undone." On confirm, remove the file and refresh
   the browser list.
4. If a user preset file is malformed at load time, skip it and log a warning
   (not an audit finding — user presets are outside the project file).

**Tests:**
- Unit test: the slug derivation function handles special characters and
  spaces correctly.
- Manual smoke: save a tuned ripple-wash preset under a custom name; reopen
  the browser; confirm the user preset appears; delete it; confirm it
  disappears.

**Acceptance:**
- [ ] "Save as preset…" writes a `.rmap-preset.json` file to the user
      preset directory.
- [ ] User presets appear in the browser's "User presets" section.
- [ ] Delete button removes the file with a confirmation prompt.
- [ ] Built-in presets have no delete button.
- [ ] `make ci` clean.

**Out of scope:** import (P2.8.5); preset versioning / history (Phase 4+).

---

### P2.8.5 — `.rmap-preset.json` export / import

**Source:** `004-phase-2.md` Acceptance criteria ("The preset library exports a
single `.rmap-preset.json` per preset that can be shared across projects
without media or warp data").
**Type:** engine + UI
**Depends on:** P2.8.4.
**Files:** `src/render/fx_presets.rs` (schema + writer + reader),
`src/windows/preset_browser.rs` (import button + drag-drop).

**What:** `.rmap-preset.json` is the single-file preset transport format.
Schema: `{ "preset_id": String, "params": HashMap<String, f32>, "name": String,
"author": Option<String> }`. No media, no warp, no mask — only the preset
identifier and its param overrides. This is also what P2.8.4 writes for user
presets. This task adds an explicit export button and a drag-drop import path.

**Steps:**
1. Define `RmapPresetJson` as a serde struct in `src/render/fx_presets.rs`
   with the four fields above. Add `pub fn write_preset(path, preset) ->
   Result<()>` and `pub fn read_preset(path) -> Result<RmapPresetJson>`.
2. "Export…" button in the preset browser for both built-in and user presets:
   opens an `rfd::FileDialog` save dialog; writes the `.rmap-preset.json` file.
3. "Import…" button in the browser (or via drag-drop of a `.rmap-preset.json`
   onto the app window): reads the file, validates that the `preset_id` is
   registered (or is a user preset), and writes it to the user preset directory
   (making it available via P2.8.1).
4. Schema validation on import: if `preset_id` is unknown, show a toast: "This
   preset requires a version of rmap that supports '<preset_id>'. It was not
   imported."
5. Write a cross-project transport test: export a tuned preset from one in-
   memory project context; import it into another; verify `preset_id` + `params`
   are identical.

**Tests:**
- Unit test (cross-project transport): `write_preset` + `read_preset` round-
  trip produces identical `preset_id` + `params`. No media or warp fields in
  the output.
- Unit test: import of a file with an unknown `preset_id` returns an error
  (not a panic).
- Manual smoke: export a tuned preset; drag it into a second project window;
  confirm the import and the params match.

**Acceptance:**
- [ ] `.rmap-preset.json` schema matches `{ preset_id, params, name, author }`.
- [ ] Export and import round-trip with identical fields.
- [ ] No media, warp, or mask data in the exported file.
- [ ] Unknown-preset import shows a clear toast and does not crash.
- [ ] `make ci` clean.

**Out of scope:** bulk export of all user presets (phase 4+); preset pack /
bundle format (Phase 4+); cloud preset sharing (out of scope permanently per
roadmap §11).

---

## Workstream 9 — Snapshot / determinism / undo

Verifies the correctness invariants for FX layer state across the full
project lifecycle.

### P2.9.1 — Proptest extension: `SetFxLayerParams` round-trip

**Source:** `004-phase-2.md` Acceptance criteria ("FX layer state survives
scene recall and undo — proptest harness in `src/project/` extended to cover
FX layer mutations").
**Type:** engine (defensive)
**Depends on:** P2.5.6 (introduces `SetFxLayerParams`).
**Files:** `src/project/command.rs` (proptest harness section).

**What:** the proptest round-trip harness in
`src/project/command.rs::tests::proptest_round_trip` exercises every
`Mutation` variant through a generate → apply → apply-reverse cycle and
asserts the project state returns to its pre-mutation form. `SetFxLayerParams`
must be added to the strategy so its Reverse-storage rule is continuously
exercised. The `seed` and `t_layer_added_secs` fields added to `LayerKind::
FxLayer` by P2.5.1 must also round-trip correctly.

**Steps:**
1. Read `src/project/CLAUDE.md` — proptest harness section (cites the round-
   trip invariant).
2. Extend the proptest strategy in `command.rs::tests` to include
   `SetFxLayerParams` with arbitrary `HashMap<String, f32>` values that are
   within the valid range for a registered preset (use `RIPPLE_WASH_PRESET_ID`
   as the fixture preset).
3. Verify the harness also covers `LayerKind::FxLayer` with populated `seed`
   and `t_layer_added_secs` values (may already be covered if the
   `LayerKind::Arbitrary` derivation includes these fields).
4. If `SetLayerKind` round-trip for `FxLayer` is not already exercised, add
   that strategy arm.

**Tests:**
- Proptest (100 iterations minimum): `SetFxLayerParams` apply + reverse +
  assert round-trip is identity.
- Proptest: `LayerKind::FxLayer { seed, t_layer_added_secs }` JSON round-trip
  is identity including the new fields.

**Acceptance:**
- [ ] `SetFxLayerParams` is in the proptest strategy.
- [ ] `LayerKind::FxLayer` snapshot round-trip includes `seed` +
      `t_layer_added_secs`.
- [ ] All 100 proptest iterations pass.
- [ ] `make ci` clean.

**Out of scope:** end-to-end scene-recall test (proptest exercises the mutation
layer, not the full app pipeline); P2.9.2 GPU determinism test (separate task).

---

### P2.9.2 — Determinism test: same seed = bit-exact pixel output

**Source:** `004-phase-2.md` Acceptance criteria ("FX layer state survives
scene recall and undo"); "Particle determinism = seed + time-offset"
(Anticipated risk #3).
**Type:** engine (defensive)
**Depends on:** P2.5.2 (`mask_constrained_drift` as the fixture preset).
**Files:** `tests/headless_gpu.rs`, `tests/golden/`.

**What:** two render runs with the same `seed` (using `mask_constrained_drift`)
must produce bit-exact pixel output. A third run with a different seed must
produce a visibly different image (at least one pixel differs). This verifies
the determinism contract from Anticipated risk #3 under the full GPU rendering
stack — proptest alone cannot check this since it operates on the CPU.

**Steps:**
1. Read `tests/headless_gpu.rs` — understand the existing headless render
   harness and how golden images are compared.
2. Add `test_particle_determinism_same_seed`: render `mask_constrained_drift`
   (N=64, seed=42, clock=5.0) twice; compare pixel buffers; assert bit-exact.
3. Add `test_particle_determinism_different_seed`: render with seed=42 and
   seed=43 at the same clock; assert at least one pixel differs.
4. Record the seed=42 render as a golden image under `tests/golden/` using
   `UPDATE_GOLDEN=1`.

**Tests:**
- GPU test (`--features gpu-tests`): both assertions run only when a wgpu
  adapter is available.

**Acceptance:**
- [ ] Same seed = bit-exact identical pixel output (two independent renders).
- [ ] Different seed = at least one pixel differs.
- [ ] Golden baseline for seed=42 added under `tests/golden/`.
- [ ] `make ci` clean.

**Out of scope:** cross-session determinism (OS timer resolution may introduce
sub-frame differences in `t_layer_added_secs`; document the known limitation);
video frame determinism (separate concern).

---

## Workstream 10 — Release housekeeping

These tasks are sequenced last. P2.10.1 owns the version bump; the rest fill
in the CHANGELOG and documentation shells from P2.1.3.

### P2.10.1 — Version bump 0.5 → 0.6

**Source:** `004-phase-2.md` Goal ("Phase 2 delivers: the full preset library
and effect-chain reordering").
**Type:** release
**Depends on:** all preceding workstreams.
**Files:** `Cargo.toml`.

**What:** bump the crate version from 0.5.x to 0.6.0, verify the
`release-show` profile builds cleanly, and confirm `make build-show` produces
a `.app` bundle. Pattern mirrors P1.7.1.

**Steps:**
1. Edit `version` in `Cargo.toml` from `0.5.x` to `0.6.0`.
2. Run `make build-release` and `make bundle` to confirm both profiles
   compile.
3. Confirm `make ci` is clean after the bump.

**Tests:**
- `make build-show` completes without errors (the `release-show` LTO profile
  is the canonical show-day build).

**Acceptance:**
- [ ] `Cargo.toml` version is `0.6.0`.
- [ ] `make build-show` completes cleanly.
- [ ] `make bundle` produces a `.app`.
- [ ] `make ci` clean.

**Out of scope:** crate publishing (rmap is not published to crates.io); git
tag (operator's discretion).

---

### P2.10.2 — CHANGELOG body for v0.6

**Source:** `004-phase-2.md` — the full feature set delivered.
**Type:** docs / UX
**Depends on:** P2.10.1 (version bump establishes the section header).
**Files:** `CHANGELOG.md`.

**What:** fill in the `## [Unreleased] — v0.6` shell from P2.1.3 with the
actual release notes: FX preset library (Wave / Particle / Fluid families),
effect-chain reordering, `.rmap-preset.json` format, `SetFxLayerParams`
mutation, particle budget enforcement. Move the "Unreleased" marker to a new
empty section above. Write at the operator level, not the engineer level.

**Steps:**
1. Replace the placeholder subsections in `CHANGELOG.md` with real entries
   grouped by: "FX Preset Library", "Effect-Chain Reordering", "Particle /
   Wave / Fluid Families", "Export / Import".
2. Use past tense and operator-facing language: "Operators can now pick from
   14 built-in presets..." not "Added FxPresetRegistry".
3. Add a new empty `## [Unreleased]` section above v0.6 for future changes.

**Tests:**
- No automated tests for documentation.
- Manual: confirm the changelog renders without broken Markdown.

**Acceptance:**
- [ ] v0.6 section is fully populated (no placeholder text remains).
- [ ] A new empty `[Unreleased]` section is above v0.6.
- [ ] Language is operator-facing.
- [ ] `make ci` clean.

**Out of scope:** blog post / announcement copy (outside the repo).

---

### P2.10.3 — README updates for FX preset library

**Source:** `004-phase-2.md` Goal.
**Type:** docs / UX
**Depends on:** P2.10.1.
**Files:** `README.md`.

**What:** fill in the README stub from P2.1.3 with prose describing the FX
preset library, the three preset families, the preset browser, and the
`.rmap-preset.json` export format. Also: update any version badge or feature
table that references v0.5.

**Steps:**
1. Expand the "FX Preset Library (v0.6)" subsection with 2–3 sentences
   describing the families and the three-click UX flow ("drop a mask, pick
   a preset, see it run").
2. Update any feature matrix rows that said "coming in v0.6" to checkmarks.
3. Update version badges if present.

**Tests:**
- No automated tests.
- Manual: confirm the README renders correctly on GitHub Markdown.

**Acceptance:**
- [ ] FX Preset Library section is substantively populated.
- [ ] No stale v0.5-only feature claims remain.
- [ ] `make ci` clean.

**Out of scope:** documentation site / generated API docs (not in scope for
v0.6); video walkthroughs.

---

### P2.10.4 — Show-day checklist additions for v0.6

**Source:** `004-phase-2.md` Acceptance criteria; roadmap §"Show-day
reliability".
**Type:** docs / UX
**Depends on:** P2.10.1.
**Files:** `docs/show-day-checklist.md`.

**What:** `docs/show-day-checklist.md` is the operator's pre-show ritual.
Phase 2 introduces three new things that need show-day verification steps:
particle budget, the preset library, and effect-chain reorder. Pattern mirrors
Phase 1's P1.7.3 (items 27–31 added for treatments + video grammar).

New checklist items to add:
- Particle budget: "If using particle presets, verify each FxLayer's particle
  count is within budget (no inline warning visible in the control panel)."
- Preset library: "Confirm all FX presets load without `UnknownFxPreset` audit
  warnings in the diagnostics strip."
- Effect-chain order: "Confirm effect-chain order on each layer matches your
  saved scene (check undo stack is clear before going live)."

**Steps:**
1. Read `docs/show-day-checklist.md` — understand the numbering convention
   and category structure.
2. Add the three new items in the appropriate sections (likely "GPU / Effects"
   and "Project / Scene" categories). Assign sequential item numbers.
3. Keep wording in the checklist's established imperative tone.

**Tests:**
- No automated tests.
- Manual: read through the checklist end-to-end and confirm the new items
  fit the flow.

**Acceptance:**
- [ ] Three new checklist items added (particle budget, preset library audit,
      effect-chain order).
- [ ] Item numbers are sequential (no gaps).
- [ ] Wording matches the imperative tone of existing items.
- [ ] `make ci` clean.

**Out of scope:** automating the checklist (future `make preflight` target);
MIDI-trigger verification steps (Phase 6).

---

### P2.10.5 — Phase 2 acceptance smoke test ("three-click" UX)

**Source:** `004-phase-2.md` Acceptance criteria, line 1
("An operator can drop a polygon mask, pick 'mask-edge ripple wash' from
the preset library, and see it run within three clicks").
**Type:** docs / UX (manual smoke checklist)
**Depends on:** P2.8.1, P2.5.6, P2.7.1, P2.10.4.
**Files:** `docs/show-day-checklist.md` (new sub-section) or new
`docs/phase-2-acceptance.md`.

**What:** the Phase 2 spec's first acceptance criterion is an end-to-end UX
flow that no leaf task verifies in isolation. This task writes a manual
smoke-test script that an operator (or reviewer) runs against the v0.6
build to confirm the headline acceptance criteria from `004-phase-2.md`
end-to-end:

1. **Three-click acceptance** — fresh project → draw polygon → open preset
   browser → pick "Mask-edge ripple wash" → preset runs within three clicks.
2. **Particle budget enforcement** — push `particle_count` past
   `max_particle_count`; mutation refuses, inline warning toast appears,
   slider snaps back, project state unchanged.
3. **Scene recall preserves FxLayer state** — save a scene with a particle
   layer; recall it; particles render identically (same seed).
4. **Effect-chain reorder + undo** — drag Blur above Color on an Image
   layer; render order changes; undo restores.
5. **Preset export / import** — export a tuned preset, import into a fresh
   project; preset_id + params identical, no media or warp data in the file.

Each step has a pass/fail box. The whole script should run in under five
minutes against a debug build with the demo project.

**Steps:**
1. Pick a file location — preferred: a new sub-section at the bottom of
   `docs/show-day-checklist.md` titled "Phase 2 acceptance smoke test", so
   it lives alongside the existing operator-facing checklist.
2. Write the five-step script (above) as numbered items with pass/fail
   checkboxes and expected outcomes.
3. Cross-reference each step with the originating Phase 2 acceptance
   criterion line so future readers can trace requirement → verification.

**Tests:**
- No automated tests (this *is* the manual test).
- Run the script once against the v0.6 release-candidate build; record
  pass/fail per step in a commit comment when the script lands.

**Acceptance:**
- [ ] Five-step smoke script exists in the docs.
- [ ] Each step references the source spec line it verifies.
- [ ] Script runs cleanly against the v0.6 build (recorded in commit
      comment).
- [ ] `make ci` clean.

**Out of scope:** automating the smoke test (selenium-style UI scripting
not in scope for v0.6); regression coverage (the proptest + golden tests
land per-feature).
