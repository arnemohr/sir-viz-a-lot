# 004 Phase 0 — task breakdown

Companion task spec for [`004-phase-0.md`](004-phase-0.md). Each task
below is sized for a single PR.

## Implementation status (2026-05-11, after W5 + release housekeeping)

**Shipped (commit SHAs):**

- ✅ **P0.1.1** `8424f37` — Param::Bound + SourceRef removed.
- ✅ **P0.1.2** `47d2df9` — Schema v6 → v7 scaffold (LayerKind variants
  Video/FxLayer/Ndi as placeholders; OutputTarget.rgb_matrix added).
  The `output_target → output_targets: Vec<OutputTarget>` rename
  shipped separately in P0.7.1 (`a57ac8b`).
- ✅ **P0.1.3** `caa37b2` — `osc` + `midi` cargo features default-on.
- ✅ **P0.1.4** `3d02c6f` — Glossary entries for FxLayer / NDI source /
  edge-blend region / RGB matrix / MIDI-learn.
- ✅ **P0.2.1** `2d3baaa` — `Modulator::OscBound` + OSC value registry
  in `src/modulators/osc.rs`.
- ✅ **P0.2.2** `1d2244e` + follow-up `96437f2` — `Modulator::MidiBound`
  + MIDI CC value registry in `src/modulators/midi.rs`. CC decoder
  extension in `src/controls/midi.rs` shipped in the follow-up.
- ✅ **P0.2.3a** `bf58b7e` — BindingPicker + ParameterRow components
  in `src/windows/components/`.
- ✅ **P0.2.3b+c** `40b53ac` — `modulator_slider` migrated to
  BindingPicker; all 8 sources selectable from every modulator row
  (was: only Static / Sine).
- ✅ **P0.2.4** `dd4b497` — Read-only OSC bindings summary in
  Advanced panel. *Deferred:* listen-port config field, inline
  address edit, "+ Add binding" button.
- ✅ **P0.3.1** `0925347` — `TextureUploadQueue` skeleton in
  `src/render/texture_upload.rs` with 5 unit tests. *Deferred:*
  per-frame render-graph drain integration (lands with the first
  real producer in W4.2).
- ✅ **P0.3.2** `9a0b2e4` — Audio-drop counter + diagnostics surface
  with glossary popover. *Deferred:* texture-upload queue counter
  joins the aggregate when W4.2 wires the drain.
- ✅ **P0.4.1** `caecba9` — Decoder decision record only
  (`specs/004-phase-0-decoder-decision.md`): AVFoundation via
  objc2. *Deferred:* W4.2 / W4.3 / W4.4 integration (worker thread,
  render path, Selected-layer UI) — needs the
  `objc2-av-foundation` dep added and dev-environment validation.
- ✅ **P0.5.1** `b32fcc5` — `LayerKind::FxLayer { preset_id, params }`
  real fields + `SetLayerKind` Mutation (whole-enum Reverse).
- ✅ **P0.6.1** `5196c64` — NDI binding decision record only
  (`specs/004-phase-0-ndi-decision.md`): community `ndi` crate.
  **W6 (NDI input) is now deferred to v0.5** — the NewTek SDK's
  install + redistribution-license friction outweighs the
  capability's value for v0.4. The decision record stays on
  file and applies whenever the work resumes. Roadmap §1.1
  classifies NDI as "transport, not primary creative source",
  so the deferral matches the stated philosophy.
- ✅ **P0.7.1** `a57ac8b` (schema rename) + `66297d4` (launcher half)
  — `Vec<OutputTarget>` everywhere + launcher multi-output picker
  with per-row Identify flash. Launcher captures the secondary
  monitor in `LauncherState`; the actual second-window spawning
  ships in P0.7.2.
- ✅ **P0.2.5** `ac0365b` + follow-up `e98a7e6` — MIDI-learn
  workflow. New `src/controls/midi_learn.rs` module with a
  Mutex-behind-OnceLock state shared between UI and the midir
  callback thread. Right-click on a modulator row's label →
  "Learn next MIDI CC"; pulsing accent while armed; ESC cancels;
  30 s timeout with a toast. Captured CC dispatches
  `Command::MidiLearnCapture { target, channel, cc, scale,
  offset }` → `apply_command` builds `Modulator::MidiBound` via
  `SetModulator` (undoable). The follow-up threads range-derived
  `scale` + `offset` through arm-time so captured bindings sweep
  the parameter's full range (matches the picker's behaviour).
- ✅ **P0.7.2** `28d63e5` (part 1: container) + `4afac03` (part 2:
  second OutputWindow lifecycle). `EditingState.outputs:
  SmallVec<[OutputWindow; 2]>`, `OutputState` extracted to
  EditingState, `secondary_monitor` plumbed through
  `Command::Launch` / `LauncherAction::Launch`,
  `reconcile_output_targets` adapts `project.output_targets` to the
  launcher's selection, per-frame loop runs passes 1–4 once and
  passes 5–6 per output, audit walks every entry with "output N:"
  prefixes, `OutputWindow.monitor` field added for GoLive
  fullscreen, one `SleepAssertion` per active display, window-close
  shrinks the vec and exits when empty.
- ✅ **P0.7.3** `94abced` — Edge-blend overlap region rendering.
  `Project.edge_blend: Option<EdgeBlendConfig { overlap_px,
  falloff_curve }>` (non-bumping schema addition); `FalloffCurve`
  enum (Linear, Cosine); `SetEdgeBlend` mutation (snapshot Reverse);
  new `src/render/shaders/edge_blend.wgsl` + `src/render/edge_blend.rs`
  pipeline with multiply blend state. Per-output pass runs between
  gamma and overlay when `outputs.len() >= 2 && edge_blend.is_some()`.
  outputs[0] = right-edge falloff, outputs[1] = left-edge falloff
  (hardcoded v0.4 topology; Phase 7 generalises). GPU golden test
  deferred (CPU uniform-byte test included).
- ✅ **P0.7.4** `fddc6c6` — `TestPattern::EdgeBlendGradient` and
  `TestPattern::AlignmentCross` added to the `T` cycle with
  shaders under `src/render/shaders/`. P0.7.1 wires
  AlignmentCross into the launcher Identify button.
- ✅ **P0.7.5** `bc08cf3` — Output mode pill (minimum-viable
  toolbar toggle). New `st.output_panel_open` + "Output" toolbar
  toggle that opens the OutputPanel as a peer right-side SidePanel
  (animated width mirrors Advanced; Esc closes). Mutual exclusion
  with Advanced's per-output sections (avoids egui Grid-ID
  collisions and matches the spec's mode-pill semantic).
  **Deferred from spec:** the full Warp/Mask/Content/Output
  *cluster* (M3 follow-on; the v3 toolbar today only has a single
  Warp toggle), canvas mode-tint border (I11 — tint infra not
  established), pill keyboard binding.
- ✅ **P0.8.1** `7d697fe` — OutputPanel scaffold. New
  `src/windows/output_panel.rs`. `SetOutputRgbMatrix` extended
  in-place with `output_idx: usize` per the P0.8.2 forward-looking
  comment; single existing call site updated; proptest harness
  threaded with random output_idx. Edge-blend section at panel
  level (rationale: v0.4 data model has one shared edge). Sub-card
  per output target with monitor-name header, placeholder preview
  thumbnail (160×90 "Preview pending"), per-output RGB matrix
  editor. Advanced panel branches: ≥2 projectors → OutputPanel
  CollapsingHeader replaces "Display output" + "RGB Matrix";
  1 projector → existing surfaces unchanged. **Deferred:**
  per-output gamma/brightness/contrast overrides (schema doesn't
  carry them — would need `OutputTarget.{gamma,brightness,contrast}
  _override: Option<f32>` + cascading render lookup).
- ✅ **P0.8.2** `b1ea596` — Per-projector RGB matrix render path.
  `gamma.render` consumes `OutputTarget.rgb_matrix`; P0.7.2 routes
  the per-output target so each projector applies its own matrix.
- ✅ **P0.8.3** `c0e3181` — RGB matrix editor UI in the per-display
  Advanced panel (3×3 spinner grid + identity reset). P0.8.1's
  `show_rgb_matrix_editor` is now parameterised on `output_idx`
  so the editor serves both the 1-projector Advanced surface and
  the multi-projector OutputPanel sub-cards.
- ✅ **P0.5.2** `a8fefba` — SDF helper for shader consumers. New
  `src/render/shaders/sdf_helper.wgsl` with
  `sample_sdf_bilinear` / `sample_sdf_gradient` / `sample_sdf`
  taking the SDF texture as a function parameter (no global bind
  coupling). Exposed via `crate::render::sdf::SDF_HELPER_WGSL`;
  build.rs prefix table prepends it for consumers whose filename
  starts with `warp` or `fx_`. `warp.wgsl` refactored to use the
  helper (math identical → existing golden tests stay bit-exact).
  Narrow interpretation per advisor — Color/Blur/Transform are
  deliberately NOT plumbed (none of them consume SDF). **Spec
  correction:** the baker doesn't produce gradient; gradient is
  shader-side via central finite differences.
- ✅ **P0.5.3** `fbb4edc` — Mask-edge ripple wash preset + FxLayer
  real render path. New `src/render/fx_presets.rs` with
  `FxPresetPipeline::new_ripple_wash` + `FxParamsUniform`
  (fixed-shape struct, presets fill in what they read). New
  `src/render/shaders/fx_ripple_wash.wgsl` consumes the P0.5.2 SDF
  helper. `LayerState.fx_texture: Option<(Texture, View)>` allocated
  per FxLayer at output size; per-frame loop syncs the mask SDF,
  runs the preset pipeline into `fx_texture`, then the layer flows
  through the existing effect chain + warp unchanged. New demo
  `assets/demos/fx-ripple-wash.rmap.json` wired into `DEMO_LIST`.
- ✅ **P0.9.1–P0.9.4** `a2d58c1` — v0.4.0 release housekeeping.
  Version bump 0.3.1 → 0.4.0; `cargo build --profile release-show`
  clean (~74 s, 11 MB stripped binary); `cargo bundle --profile
  release-show` produces `.app` (no `[package.metadata.bundle]`
  configured — uses defaults). CHANGELOG + README + show-day
  checklist refreshed with v0.4 capabilities; system-deps audit
  confirms v0.4 has zero Homebrew dependencies (AVFoundation when
  video lands ships with macOS; NDI deferred to v0.5).

**Not yet started:**

- **P0.4.2 / P0.4.3** — Video playback. P0.4.1's decision record
  picked AVFoundation via objc2 (no system dep, no system-frameworks
  install — ships with macOS). Implementation needs:
  `objc2-av-foundation` Cargo dep, decoder worker thread, frame
  producer for `TextureUploadQueue` (P0.3.1's queue ready), render
  integration mirroring P0.5.3's FxLayer shape (texture allocated
  at layer init, filled per-frame from the decoder, then flows
  through the existing effect chain). Drag-and-drop in
  `layer_from_dropped_path` extended to mp4/mov/m4v. P0.4.3
  follows with playback-speed UI.
- **P0.9.5** — Show-day frame-budget perf gate. Deferred until
  P0.4.2 lands so the fixture (4 video layers + bindings + edge-
  blend) measures the full v0.4 surface; otherwise the gate
  benchmarks a strict subset and the recorded baseline doesn't
  reflect the acceptance line.

**Deferred to v0.5:**

- **P0.6.2 / P0.6.3** — NDI receiver + audit. Decision record
  (P0.6.1 `5196c64`) stays on file; the schema placeholder
  `LayerKind::Ndi` shipped by P0.1.2 stays in v7 so v0.5 needs
  no migration when the receiver lands. The "NDI source"
  glossary entry stays in v0.4 — operators see it on the (inert)
  NDI layer-row badge before the receiver lands. NDI ingest is
  classified as "transport, not primary creative source" per
  roadmap §1.1; the deferral matches the stated philosophy.

**Test status:**

- 495 tests pass under `--features v3,midi`.
- 262 tests pass under default features.
- New tests by workstream:
  - W2 (modulator path + components + MIDI-learn state): ~13 tests.
  - W3 (texture-upload queue): 5 tests.
  - W5 (FxLayer round-trip + SDF helper smoke + ripple wash params
    + demo project load): ~5 tests.
  - W7 (reconcile + per-target audit + edge-blend schema /
    mutation / uniform byte-layout): ~12 tests.
  - W8 (RGB matrix render + per-output mutation, out-of-range
    panic test): tests landed across P0.8.2 / P0.8.1.

**Pre-existing issues (not introduced by this work):**

- `make lint` (clippy `--all-features`) fails on
  `src/project/mod.rs` due to a Rust 1.92 / clippy upgrade
  (`field_reassign_with_default`, `absurd_extreme_comparisons`).
  Verified pre-existing on the branch base before any of these
  commits. Cleanup is orthogonal to v0.4 scope.

---

## Operating model

- **Model:** Opus implements; **no separate review step.** Opus is
  expected to bring the rigour that v3.1 split between Sonnet (impl)
  and Opus (review) into a single pass. That means: read the spec
  section, read every CLAUDE.md the task touches, write the test
  *first*, run `make ci` before committing.
- **Pick one task at a time.** Read the source section it references
  in `004-phase-0.md` and the corresponding entry in
  `specs/roadmap.md` before starting.
- **Commit message format:** `004-P0.<workstream>.<task>: <title>` —
  e.g. `004-P0.1.1: remove Param::Bound and SourceRef dead code`.
- **Branching:** one branch per task; merge straight to `main` once
  CI is green.
- **Pre-commit hook** (`.githooks/pre-commit`, wired by `make setup`)
  runs rustfmt on staged files + `cargo check`. Heavier checks live
  in `make ci`; run that before opening a PR.
- **Tests:** every task ships with new or updated tests. Phase 0
  introduces new subsystems — silent-corruption traps multiply.
  Most tasks are greenfield (no existing bug to reproduce); write
  the test alongside the implementation. For schema / Mutation /
  snapshot work, follow the v3 proptest pattern in
  `src/project/command.rs`. Where automation isn't possible
  (visual / hardware-driven flows), ship a manual smoke-test
  checklist instead — never nothing.
- **Read the right CLAUDE.md.** Tasks touching `src/project/` must
  read `src/project/CLAUDE.md` first (Mutation Reverse-storage rules,
  snapshot invariants). Tasks touching `src/render/` must read
  `src/render/CLAUDE.md` first (GPU lifecycle, panic_restore,
  build-time WGSL validation).
- **Don't bundle.** If a task tempts you to also fix something
  nearby, resist — that "something nearby" probably already has its
  own task ID below.
- **GPU bring-up tasks ship golden images.** Anything that touches
  `src/render/` and renders pixels needs a `tests/golden/` baseline
  added under `--features gpu-tests`; `UPDATE_GOLDEN=1` rewrites the
  baseline.

## Task ID conventions

- IDs are flat-numbered within nine workstreams:
  - W1 — Setup + housekeeping (Param::Bound removal, schema v7
    scaffold, cargo defaults, glossary)
  - W2 — Live-input bindings (Modulator + UI)
  - W3 — Texture-upload pipeline (foundation for video + NDI)
  - W4 — Video playback (Anchor A)
  - W5 — FX layer foundations (Anchor B kickoff)
  - W6 — NDI input
  - W7 — Two-projector edge-blend stub
  - W8 — Output panel + per-projector colour calibration
  - W9 — Release housekeeping
- Tasks reference back to the originating section of `004-phase-0.md`
  via the **Source** field.

## Workstream summary

| WS | Theme | Tasks | Parallel-safe? | Touches |
|----|-------|-------|----------------|---------|
| 1 | Setup + housekeeping | 4 | W1.1 first; W1.2 unblocks W4–W8; rest parallel-safe | `src/controls/`, `src/project/schema.rs`, `Cargo.toml`, `src/windows/glossary.rs` |
| 2 | Live-input bindings | 7 | W2.1 + W2.2 first; W2.3a then 2.3b then 2.3c serial; W2.4 / W2.5 parallel after 2.3a | `src/modulators/`, `src/controls/`, `src/windows/control_panel.rs`, new `src/windows/components/` |
| 3 | Texture-upload pipeline | 2 | Land before W4.3 + W6.2 | `src/render/`, new `src/render/texture_upload.rs` |
| 4 | Video playback | 3 | W4.1 first (decoder decision + bring-up); rest serial | new `src/video_layer/`, `src/render/`, `src/project/schema.rs` |
| 5 | FX layer foundations | 3 | Internally serial; depends on W1.2 | `src/project/schema.rs`, `src/render/`, `src/effects/` |
| 6 | ~~NDI input~~ (deferred to v0.5) | 1 of 3 | Decision record (P0.6.1) shipped; W6.2 + W6.3 move to v0.5 | (see decision record) |
| 7 | Two-projector edge-blend | 5 | Internally serial; depends on W1.2 | `src/app.rs`, `src/render/`, `src/project/schema.rs`, `src/test_patterns.rs` |
| 8 | Output panel + calibration | 3 | Internally serial; depends on W7.2 | `src/render/`, `src/windows/control_panel.rs` |
| 9 | Release housekeeping | 5 | Last — depends on everything else | `Cargo.toml`, `CHANGELOG.md`, `README.md`, `Makefile` |

**Suggested order for sequencing PRs:**

1. **W1.1** (Param::Bound removal) — tiny dead-code cleanup; clears
   the deck.
2. **W1.2** (schema v7 scaffold) — unblocks W4, W5, W7, W8.
3. **W2.1 + W2.2 + W3.1 + W4.1** in parallel — independent
   engine kick-offs (Modulator variants, texture-upload skeleton,
   decoder decision + bring-up). W6 (NDI input) was originally
   in this batch but is deferred to v0.5.
4. Per-workstream sequential progress through W2–W8.
5. **W9** at the end (release housekeeping; P0.9.4 ffmpeg-deps
   should land alongside P0.4.1 if the chosen decoder needs them).

---

## Workstream 1 — Setup + housekeeping

Clear the deck before the bigger engine work lands. Each task here
is intentionally small and ships ahead of the workstreams that
depend on it.

### P0.1.1 — remove `Param::Bound` and `SourceRef` dead code

**Source:** `004-phase-0.md` MIDI section ("`Param::Bound` and
`SourceRef` are removed in a setup PR")
**Type:** cleanup
**Depends on:** none
**Files:** `src/controls/param.rs`, `src/controls/mod.rs`,
`src/app.rs:847` (the `Reserved for Param::Bound` comment).

**What:** delete the dead `Param<T>` enum, `SourceRef`, and the
`InputState::read` / `Source::read` registry hooks. Effect parameters
already use `Modulator` directly; the `Bound` arm has zero callers
and the `bound_returns_zero_v1` test confirms the path is inert.

**Steps:**
1. Confirm zero non-test callers: `rg "Param::|Param<|SourceRef"
   src/` returns only `src/controls/param.rs` and the comment in
   `src/app.rs:847`.
2. Delete `src/controls/param.rs`.
3. Remove the `pub mod param` line in `src/controls/mod.rs` and any
   re-exports.
4. Remove the `read` / source-registry methods on `InputState` /
   `Source` if they are unused after the Param removal.
5. Remove the comment block at `src/app.rs:847` that reserves the
   path.

**Tests:**
- No new tests; the deleted file had three. `make ci` clean is
  the regression check.

**Acceptance:**
- [ ] `src/controls/param.rs` is deleted.
- [ ] `rg "Param::Bound|SourceRef|bound_returns_zero" src/` returns
      no hits.
- [ ] `make ci` clean.

**Out of scope:** introducing `Modulator::OscBound` /
`Modulator::MidiBound` (those are W2.1 / W2.2).

---

### P0.1.2 — schema v6 → v7 scaffold

**Source:** `004-phase-0.md` Engine implications ("Schema migrates
v6 → v7")
**Type:** schema + migration
**Depends on:** v3.1's W2 (`OutputTarget` + v5→v6) has landed —
`OutputTarget` already exists in `src/project/schema.rs:155` and
`CURRENT_SCHEMA_VERSION` is already 6.
**Files:** `src/project/schema.rs` (`LayerKind` at line 45,
`OutputTarget` at line 155, `CURRENT_SCHEMA_VERSION` at line 9),
`src/project/migrate.rs`, `src/project/command.rs` (new Mutation
variants + `ReverseStorage` impls), `src/project/audit.rs` (walk
new variants), proptest fixtures.

**What:** add the v7 schema scaffold so the dependent workstreams
have something to extend: `output_target: OutputTarget` becomes
`output_targets: Vec<OutputTarget>` (single element on migration);
`OutputTarget` gains `rgb_matrix: [[f32; 3]; 3]` (default identity,
populated by W8.2's render path); `LayerKind` gains stub variants
for `Video`, `FxLayer`, and `Ndi` that render a placeholder
rectangle. Migration is automatic.

**Steps:**
1. Read `src/project/CLAUDE.md` end-to-end. The three Reverse-
   storage rules are load-bearing here:
   - **Whole-enum Reverse** (rule 1): adding `LayerKind` variants
     means every Mutation that replaces a `LayerKind` snapshots
     the full enum.
   - **AddLayer/RemoveLayer exception** is documented; the new
     variants must be reachable through `Mutation::AddLayer`.
   - The `ReverseStorage` trait (V31.3.2 landed) enforces this at
     the type level — a new variant won't compile without its
     impl. The `compile_fail` doctest in `command.rs` is the
     reference.
2. Read `src/project/migrate.rs` for the v2 → … → v6 chain
   (`migrate_v5_to_v6_output_target` at line 327 is the closest
   precedent).
3. Bump `CURRENT_SCHEMA_VERSION` from 6 to 7 in `schema.rs:9`.
4. In the v7 schema: rename `output_target` → `output_targets:
   Vec<OutputTarget>`. The migration wraps the existing v6 value
   in a single-element vec. `OutputTarget` gains `#[serde(default
   = "rgb_matrix_identity")] rgb_matrix: [[f32; 3]; 3]`.
5. Extend `LayerKind` with `Video { path: PathBuf, /* W4 fills
   real fields */ }`, `FxLayer { preset_id: String, /* W5 fills
   params */ }`, and `Ndi { source_name: String }`. Each currently
   renders a coloured placeholder rectangle keyed off the variant
   discriminant (clearly identifiable in the canvas).
6. Add Mutation variants for the new layer types via the
   `ReverseStorage` pattern; `Mutation::AddLayer { kind:
   LayerKind, ... }` already covers AddLayer for them (per the
   exception list).
7. Update `src/project/audit.rs` to walk the new variants without
   panicking; missing-file warnings already exist for `Image` and
   `Svg`, mirror them for `Video`.
8. Update the `LayerKind::asset_path` helper at `schema.rs:62` —
   `Video` returns `path`; `FxLayer` and `Ndi` have no asset path
   (the helper signature may need to change to `Option<&Path>` or
   gain a sibling).

**Tests:**
- v6 fixture (saved under v3.1) loads under v7 with
  `output_targets.len() == 1`, `rgb_matrix` defaulted to identity,
  and identical observable behaviour.
- v7 fixture round-trips identically including a project with all
  three new layer variants present.
- Mutation proptest extends the existing harness in
  `command.rs:2849` to cover the new variants (whole-enum Reverse
  for `LayerKind` swaps, snapshot round-trip).
- `cargo test --features v3 --doc` runs the `compile_fail` doctest
  on `ReverseStorage`.

**Acceptance:**
- [ ] `CURRENT_SCHEMA_VERSION` is 7.
- [ ] v6 fixtures load through migration with no behaviour change.
- [ ] `LayerKind::Video`, `LayerKind::FxLayer`, `LayerKind::Ndi`
      exist as placeholder variants and render an obvious placeholder.
- [ ] `OutputTarget.rgb_matrix` exists, defaults to identity.
- [ ] `Mutation::AddLayer` accepts every new `LayerKind` variant.
- [ ] Mutation proptest covers the new variants under all three
      Reverse-storage rules.
- [ ] `make ci` + `make test-gpu` clean.

**Out of scope:** real video / FX / NDI rendering (W4 / W5 / W6);
the Vec<OutputTarget> picker UI (W7.1); the rgb_matrix render
path (W8.2).

---

### P0.1.3 — flip `osc` and `midi` cargo features to default

**Source:** `004-phase-0.md` Engine implications ("Cargo features:
`osc` and `midi` move to default-on")
**Type:** packaging
**Depends on:** W2.1, W2.2 (don't flip until the binding UI is
real); land at the end of W2 if so.
**Files:** `Cargo.toml`, CI matrix (if any feature-gated tests
exist).

**What:** add `osc` and `midi` to the default feature set. `audio`
stays gated (cpal build cost). The `--no-default-features` opt-out
remains available.

**Steps:**
1. Edit `[features]` in `Cargo.toml`: add `osc` and `midi` to
   `default = [...]`.
2. Confirm `cargo build --no-default-features` still compiles
   (tests the opt-out path).
3. Confirm `cargo build` (defaults) brings in both features.
4. Update the `Cargo features` section of the project's CLAUDE.md
   to reflect the new defaults.

**Tests:**
- CI builds default + `--no-default-features` (add to `make ci` or
  CI matrix if not already covered).

**Acceptance:**
- [ ] `osc` and `midi` are in the default feature set.
- [ ] `cargo build --no-default-features` still succeeds.
- [ ] `audio` remains opt-in.
- [ ] CLAUDE.md updated.

**Out of scope:** any binding-UI work (W2).

---

### P0.1.4 — glossary entries for new domain terms

**Source:** `004-phase-0.md` Engine implications ("Glossary gains
entries + popovers for the new domain terms")
**Type:** docs (data only — popovers attach incrementally)
**Depends on:** none. Popover wiring happens in the workstream
that introduces each term's UI surface (W2 / W4 / W5 / W6 / W7);
this task lands the data so those tasks can call
`glossary_label(ui, GlossaryTerm::Foo)` without tripping over
missing variants.
**Files:** `src/windows/glossary.rs` (`GlossaryTerm` enum at
line 30, `entry()` match at line 64, `all_terms()` slice at
line 202, `glossary_label()` helper at line 239).

**What:** add five new `GlossaryTerm` variants — `FxLayer`, NDI
source, edge-blend region, RGB matrix, MIDI-learn — wire each to
a `GlossaryEntry` match arm, and append to `all_terms()`.
Popover attachment to UI surfaces is each downstream task's
responsibility.

**Steps:**
1. Read `src/windows/glossary.rs` end-to-end (it's small).
2. Add five variants to the `GlossaryTerm` enum.
3. Add a match arm in `entry()` for each, returning a
   `GlossaryEntry` with this wording (refine if the implementation
   reveals nuance):
   - **FxLayer** — A layer whose visual content is generated from
     its mask rather than from media. Used for ripple,
     displacement, and (Phase 2) particle / fluid effects.
   - **NDI source** — A live video stream received over the
     network from another machine (e.g. an OBS instance). v0.4
     supports NDI as input only; output is Phase 7.
   - **Edge-blend region** — The overlap zone between two
     projectors where image brightness is feathered so the seam
     becomes invisible.
   - **RGB matrix** — A 3×3 colour-correction matrix applied
     per-projector to compensate for differences in projector
     colour response.
   - **MIDI-learn** — A workflow where the next incoming MIDI
     control change automatically binds to the parameter you
     right-clicked on.
4. Append all five to `all_terms()`.
5. Confirm the in-app Glossary window (T4.11 path) renders the new
   entries by visiting it from the editor.

**Tests:**
- Unit test: `entry()` returns a non-empty `summary` for every
  variant in `all_terms()` (extend the existing invariant test if
  one exists; add one if not).

**Acceptance:**
- [ ] Five new `GlossaryTerm` variants exist.
- [ ] `entry()` returns a populated `GlossaryEntry` for each.
- [ ] `all_terms()` includes the five new variants.
- [ ] Glossary window lists them.

**Out of scope:** wiring `glossary_label(...)` calls to specific
UI surfaces — each downstream workstream wires its own.

---

## Workstream 2 — Live-input bindings (Modulator + UI)

Add OSC and MIDI as parameter signal sources by extending
`Modulator` (the path audio already uses) and shipping the picker /
learn / registry UX once for both transports.

### P0.2.1 — `Modulator::OscBound { addr }` + OSC value registry

**Source:** `004-phase-0.md` OSC section ("Engine: introduce
`Modulator::OscBound { addr }` …")
**Type:** engine
**Depends on:** P0.1.1 (clean baseline); independent of P0.1.2.
**Files:** `src/modulators/mod.rs`, `src/controls/osc.rs`, new
`src/controls/osc_registry.rs` (or fold into `osc.rs`),
`src/project/command.rs` (Mutation variant).

**What:** add a new `Modulator::OscBound { addr: String, scale: f32,
offset: f32 }` variant that resolves through a process-wide OSC
value registry, structured analogously to `audio::PROVIDER`.

**Steps:**
1. Read `src/modulators/audio.rs` lines 47–63 — the
   `OnceLock<Arc<dyn AudioProvider>>` + `current_band` pattern is
   the template.
2. Read `src/project/CLAUDE.md` (whole-enum Reverse rule applies:
   `Modulator` is replaced wholesale by `SetModulator` Mutations,
   so a new variant slots in cleanly under the existing
   `ReverseStorage` impl).
3. Define an `OscProvider` trait with `value(addr: &str) -> f32`
   and a `start_default` that opens the existing UDP listener and
   maintains a `RwLock<HashMap<String, f32>>` of last-seen values.
   Provider lifecycle: install once at startup via `OnceLock::set`,
   never replaced (FFT smoothing argument from the audio path
   applies symmetrically to OSC value smoothing if added later).
4. Add `Modulator::OscBound { addr, scale, offset }` to the
   enum in `src/modulators/mod.rs`. The resolve arm reads from the
   registry, applies `scale` + `offset`, and falls back to `0.0`
   when no provider is installed (matches `Modulator::Audio`
   semantics).
5. Make the variant `Serialize + Deserialize` (the rest of the
   enum already is).
6. Wire installation at app startup behind `cfg(feature = "osc")`,
   matching the audio path.
7. Extend the proptest in `src/project/command.rs:2849` (the
   modulator strategy) to cover `OscBound` round-trip.

**Tests:**
- Unit test: with a stub `OscProvider` returning canned values,
  `Modulator::OscBound` resolves through the registry and applies
  scale/offset.
- Unit test: with no provider installed, returns 0.0.
- Proptest: `OscBound` round-trips through the snapshot path.

**Acceptance:**
- [ ] `Modulator::OscBound { addr, scale, offset }` exists and
      resolves through the registry.
- [ ] Save / load round-trip preserves the variant identically.
- [ ] Snapshot / scene-recall covers the variant.
- [ ] `make ci` clean.

**Out of scope:** binding-picker UI (P0.2.3); MIDI variant
(P0.2.2); learn workflow (P0.2.5).

---

### P0.2.2 — `Modulator::MidiBound { cc, channel }` + MIDI CC registry

**Source:** `004-phase-0.md` MIDI section ("Engine: extend
`src/controls/midi.rs` decoder past Note On 60–71 …")
**Type:** engine
**Depends on:** P0.1.1; independent of P0.2.1 but mirrors its
structure.
**Files:** `src/modulators/mod.rs`, `src/controls/midi.rs`, new
`src/controls/midi_registry.rs` (or fold into `midi.rs`).

**What:** add `Modulator::MidiBound { cc: u8, channel: u8, scale,
offset }` and extend the MIDI decoder to populate a process-wide
CC value registry (today only Note On 60–71 is decoded).

**Steps:**
1. Read `src/controls/midi.rs` for the existing decoder (Note On
   handling).
2. Read `src/project/CLAUDE.md` whole-enum Reverse rule (same
   applies as for `OscBound`).
3. Extend the decoder to handle Control Change messages: capture
   `(channel, cc, value)`, write `value as f32 / 127.0` into a
   `RwLock<[[f32; 128]; 16]>` (16 channels × 128 CCs, fixed-size
   so no allocation on the hot path).
4. Define a `MidiProvider` trait analogous to `AudioProvider`,
   with `cc(channel: u8, cc: u8) -> f32`. Install at startup under
   `cfg(feature = "midi")`. Single-set lifecycle (`OnceLock`).
5. Add `Modulator::MidiBound { cc, channel, scale, offset }` to
   the enum. Resolve: `provider.cc(channel, cc) * scale + offset`,
   falling back to 0.0.
6. Extend the modulator proptest strategy at
   `src/project/command.rs:2849` to cover the new variant.

**Tests:**
- Unit test: stub provider with canned CC values; `MidiBound`
  resolves correctly.
- Unit test: with no provider installed, returns 0.0.
- Unit test: extended Control Change decoder unpacks (channel, cc,
  value) correctly across the full byte range.
- Proptest: `MidiBound` round-trips through snapshot.

**Acceptance:**
- [ ] CC decoder handles all 16 channels × 128 CCs.
- [ ] `Modulator::MidiBound` exists and resolves through the
      registry.
- [ ] Round-trip + snapshot covered.
- [ ] `make ci` clean.

**Out of scope:** the picker UI (P0.2.3); the MIDI-learn workflow
(P0.2.5).

---

### P0.2.3a — introduce `BindingPicker` + `ParameterRow` components

**Source:** `004-phase-0.md` OSC section ("Implements
`BindingPicker` and `ParameterRow` per Appendix B"); roadmap
Appendix B (component vocabulary).
**Type:** UI component
**Depends on:** P0.2.1 + P0.2.2.
**Files:** new `src/windows/components/binding_picker.rs`,
`src/windows/components/parameter_row.rs`,
`src/windows/components/mod.rs`.

**What:** add the components as standalone widgets — no call-site
migration yet. Lets P0.2.4 / P0.2.5 / W3-W7 reference them while
the migration (2.3b/c) lands separately.

**Steps:**
1. Read roadmap Appendix B and the existing modulator UI at
   `src/windows/control_panel.rs:1779–1850`.
2. Define `BindingPicker` (the dropdown + binding-indicator pill
   combo) with options `static · sine · tri · noise · bpm · audio
   · osc · midi`. Rename the displayed label of the `Static`
   option to **"fixed value"** (per roadmap I3 — "static" is jargon
   that conflicts with the picker's "static binding" meaning).
3. Define `ParameterRow` as the composition: `label · unit ·
   spinner · BindingPicker · learn-state pill`. API shape:
   `ParameterRow::new(label, unit).spinner(...).binding(...)`.
4. Icon vocabulary: antenna for OSC, jack for MIDI, mic for audio,
   clock for bpm, sine-wave for sine/tri/noise.
5. Provide a demo / playground harness (egui example or
   integration test) so the components can be exercised in
   isolation.

**Tests:**
- Demo harness that exercises every picker state.
- Snapshot-style egui test (if a harness exists in the repo —
  check `src/windows/` test files).

**Acceptance:**
- [ ] `BindingPicker` and `ParameterRow` exist as named widgets in
      a new `components/` module.
- [ ] Every binding source (8 options) renders correctly in
      isolation.
- [ ] Static option labelled "fixed value" in the UI.
- [ ] Demo harness documented.

**Out of scope:** migrating any existing call site (P0.2.3b/c).

---

### P0.2.3b — migrate one canonical effect parameter row

**Source:** continuation of P0.2.3a.
**Type:** UI migration (proof of approach)
**Depends on:** P0.2.3a.
**Files:** `src/windows/control_panel.rs` (Color effect's `hue`
parameter row, the cleanest existing example).

**What:** migrate exactly one parameter row — `Color.hue` — from
the inline modulator dropdown (~`control_panel.rs:1779`) to the
new `ParameterRow` component. Lock down the migration shape
before the bulk rewrite.

**Steps:**
1. Pick `Color.hue` as the canonical row (it has a Static
   default + every modulator type works against it).
2. Replace the inline rendering with a `ParameterRow` call.
3. Confirm undo / save / load / scene-recall behaviour is
   identical (manual smoke + existing tests).
4. Document the migration recipe inline as a comment so 2.3c can
   apply it mechanically.

**Tests:**
- Manual smoke: switch `hue` between every modulator, confirm
  visual parity with v3.
- Integration test: switch from `static` to `osc`, edit the
  address, switch back, undo restores the prior static value.

**Acceptance:**
- [ ] `Color.hue` renders through `ParameterRow`.
- [ ] Visual parity confirmed against v3 baseline.
- [ ] OSC + MIDI are selectable on this row and resolve correctly.
- [ ] Migration recipe documented.

**Out of scope:** migrating other rows (P0.2.3c).

---

### P0.2.3c — migrate remaining parameter rows

**Source:** continuation of P0.2.3b.
**Type:** UI migration (mechanical)
**Depends on:** P0.2.3b.
**Files:** `src/windows/control_panel.rs` (every other modulator
dropdown call site at ~1779, ~1845, ~1897, ~2020), and
`src/windows/scene_editor.rs` (rows at ~220, ~252).

**What:** apply the recipe from 2.3b to every remaining parameter
row in the Advanced panel and scene editor.

**Steps:**
1. Walk every Modulator-bearing parameter (Color sat / brightness
   / contrast, Blur radius, Transform rotate / scale, etc.).
2. Mechanical replacement; no behaviour change beyond what 2.3b
   established.
3. Spot-check each row visually after migration.

**Tests:**
- Existing UI integration tests still pass.
- Manual smoke: every parameter still works (toggle modulators,
  bind via OSC/MIDI, undo).

**Acceptance:**
- [ ] No remaining inline modulator dropdowns in
      `control_panel.rs` or `scene_editor.rs`.
- [ ] `make ci` clean.
- [ ] Manual smoke: every parameter row reachable from Advanced
      uses `ParameterRow`.

**Out of scope:** patch panel (P0.2.4); MIDI-learn (P0.2.5).

---

### P0.2.4 — OSC patch panel (binding editor + port config)

**Source:** `004-phase-0.md` OSC section ("Visual patch panel: OSC
address → layer parameter mapping")
**Type:** UI
**Depends on:** P0.2.3a (only — does not block on 2.3b/c).
**Files:** `src/windows/control_panel.rs`, new
`src/windows/osc_patch.rs`, `src/project/schema.rs` (project-level
`osc_listen_port: u16` field; default 9000).

**What:** a dedicated patch-panel view in the Advanced panel that
shows the OSC listen port, lists every active OSC binding (address
→ layer.parameter), allows editing addresses, and shows live
values for incoming messages. Read-only summary of the same data
the parameter-row picker (P0.2.3a) writes — this is a second view
of one truth.

**Steps:**
1. Add a collapsible "OSC bindings" section in the Advanced panel.
2. **Port row:** show the current listen port (default 9000) with
   an inline edit; changing it tears down + restarts the UDP
   listener. Persisted as `osc_listen_port: u16` on the project.
3. Walk the project's layers + effects, collect every parameter
   whose modulator is `OscBound`, render as a table: `address |
   layer | param | live value | unbind`.
4. The "live value" column reads from the OSC registry each frame
   and renders a small bar.
5. Inline edit: clicking an address opens a text edit; committing
   replaces the address (Mutation, undoable).
6. "Add binding" button at the bottom opens a parameter picker
   (which layer? which parameter?) → adds an `OscBound` modulator
   to that parameter with a stub address.

**Tests:**
- Integration test: programmatically add an `OscBound` modulator,
  verify it appears in the patch panel, edit the address via the
  Mutation, verify the table updates.
- Manual smoke: send OSC messages from a test sender (e.g.
  TouchOSC), verify the live value bar tracks.

**Acceptance:**
- [ ] Patch panel lists every active OSC binding.
- [ ] Listen port is editable + persisted; restart on change.
- [ ] Address editing is undoable.
- [ ] "Add binding" opens parameter picker and creates an
      `OscBound` modulator.
- [ ] Live value bar tracks incoming OSC.

**Out of scope:** OSC output (Phase 7); MIDI patch panel — MIDI
uses learn UX (P0.2.5) not a patch panel.

---

### P0.2.5 — MIDI-learn workflow

**Source:** `004-phase-0.md` MIDI section ("UX: binding picker on
every parameter row, MIDI-learn workflow …")
**Type:** UI
**Depends on:** P0.2.3a, P0.2.2.
**Files:** `src/windows/control_panel.rs`, `src/controls/midi.rs`
(learn-mode hook), new `src/controls/midi_learn.rs`.

**What:** right-click any `ParameterRow` → "Learn next MIDI CC".
The row enters listening state (pulsing accent ring). The next
incoming CC binds to the parameter as `Modulator::MidiBound { cc,
channel }`. ESC cancels.

**Steps:**
1. Add a context menu to `ParameterRow` with "Learn next MIDI CC".
2. Define a process-wide `MidiLearnState` (single-slot:
   `Option<ParameterRef>`) protected by a Mutex.
3. When learn-mode is armed, the MIDI decoder (P0.2.2) checks
   `MidiLearnState` on every CC message: if armed, capture the
   `(channel, cc)`, dispatch a `Mutation` that sets the parameter's
   modulator to `MidiBound`, clear the learn state.
4. Pulsing accent ring on the row while listening (use the warm
   accent at varying alpha; respect the v3 colour-blind palette).
5. ESC cancels the learn state without binding.
6. Listening times out after 30 seconds (no input) and clears
   automatically with a toast. (Industry norm; matches Ableton /
   Resolume.)

**Tests:**
- Unit test: simulate "armed → CC arrives → bound" by injecting a
  CC message into the decoder; verify the parameter mutates.
- Unit test: ESC clears the learn state.
- Manual smoke: arm a parameter, twist a CC on a real or virtual
  controller, verify the binding.

**Acceptance:**
- [ ] Right-click on `ParameterRow` shows "Learn next MIDI CC".
- [ ] Listening state has a pulsing accent ring.
- [ ] Next CC arrives → parameter is bound (Mutation, undoable).
- [ ] ESC cancels; 30 s timeout cancels with a toast.

**Out of scope:** OSC-learn (the patch panel covers OSC by typing
addresses; an OSC-learn equivalent is a Phase 6 item).

---

## Workstream 3 — Texture-upload pipeline

The threading + GPU-upload abstraction that both video (W4) and
NDI (W6) consume. Designed to be inverted later for Phase 7
Syphon / Spout output.

### P0.3.1 — thread-safe texture-upload skeleton

**Source:** `004-phase-0.md` Engine implications ("Texture-upload
pipeline for video is the foundation for Phase 7 …")
**Type:** engine
**Depends on:** none (early task).
**Files:** new `src/render/texture_upload.rs`,
`src/render/mod.rs`.

**What:** a `TextureUploadQueue` that producers (video decoder
threads, NDI receiver threads) push `(layer_id, frame_bytes,
width, height, format)` into; the render thread drains the queue
each frame and uploads via `wgpu::Queue::write_texture`.

**Steps:**
1. Read `src/render/CLAUDE.md` (per-frame render-graph order;
   surface acquisition; panic_restore wrapping).
2. Define a `TextureFrame` struct carrying `layer_id`, raw bytes
   (Box<[u8]>), dimensions, format, and a presentation timestamp
   (for video frame-budget enforcement).
3. Use a `crossbeam-channel::bounded` queue (consistent with the
   audio path in `src/modulators/audio.rs`); choose a depth that
   gives ~1 frame of slack at 60fps.
4. Drain on the main render thread inside the per-frame render
   graph, BEFORE layer drawing. Cap drain count per frame (e.g.
   8) so a producer flood can't stall the frame.
5. On dropped frames (queue full, sender uses `try_send`),
   increment a `dropped_frame_count` atomic that the diagnostics
   badge (P0.3.2) reads.
6. Wrap the drain in `panic_restore::run_frame_assert_unwind_safe`
   per `src/render/CLAUDE.md`.

**Tests:**
- Unit test: stub producer pushes 8 frames, drain consumes all 8,
  textures land in the wgpu queue (use a headless wgpu adapter
  per the existing `gpu-tests` feature).
- Unit test: producer push at queue-full uses `try_send` and
  increments the dropped-frame counter.
- Stress test: producer pushes 1000 frames in a tight loop while
  drain runs at 60 Hz — no panics, drop count is bounded.

**Acceptance:**
- [ ] `TextureUploadQueue` exists with documented sender / drain
      API.
- [ ] Drain runs inside the per-frame render graph and respects
      `panic_restore`.
- [ ] Dropped-frame atomic increments on overflow.
- [ ] Stress test passes under `cargo nextest run --features
      gpu-tests`.

**Out of scope:** the video decoder (W4); the NDI receiver (W6);
the diagnostics surface (P0.3.2).

---

### P0.3.2 — dropped-frame counter in diagnostics surface

**Source:** `004-phase-0.md` UX items resolved ("N5 capability
follow-on — diagnostics surface gains dropped-frame count")
**Type:** UI
**Depends on:** P0.3.1.
**Files:** `src/windows/control_panel.rs` (diagnostics section,
or wherever fps + panic-restored badge currently lives),
`src/modulators/audio.rs` (audio drop counter — see step 2).

**What:** add a "dropped frames" counter next to the fps + panic
badge. Aggregates drops from every bounded queue: video / NDI
texture-upload (W3.1) AND the audio FFT channel (`audio.rs:170`,
which already drops on overflow but doesn't currently count).

**Steps:**
1. Locate the existing diagnostics badge (search for `panic_restore`
   or `fps`).
2. Add a `dropped_audio_count` atomic to `src/modulators/audio.rs`
   incremented in the `tx.try_send` overflow path (today the
   `let _ = tx.try_send(...)` line silently drops).
3. The diagnostics widget reads both atomics each frame; renders
   either an aggregated `dropped: N/s` or two side-by-side
   counters (`vid: N · audio: N`). Pick aggregated for less
   chrome noise.
4. Render: subdued text when zero, accent when positive; track
   the per-second delta and fade after 5 s of zero increments.
5. Add a glossary entry for "dropped frames" (in P0.1.4, or
   alongside this task if 1.4 hasn't landed yet).

**Tests:**
- Manual smoke: force the texture-upload producer to overflow
  (W3.1's stress hook), confirm the counter updates.
- Manual smoke: cause audio overflow (extreme FFT load), confirm
  the counter increments.

**Acceptance:**
- [ ] Counter renders in the diagnostics surface.
- [ ] Aggregates video/NDI AND audio drops.
- [ ] Updates per second; fades to subdued when zero.
- [ ] Glossary entry exists.

**Out of scope:** other diagnostics surfaces (DMX universe LED —
Phase 5 follow-on).

---

## Workstream 4 — Video playback (Anchor A)

Land mp4 / H.264 video as a first-class layer with seamless loop
and configurable playback speed. Operator UX (thumbnail scrubbing,
in/out points, rate, BPM lock) is **deferred to Phase 1**.

### P0.4.1 — decoder decision + background-thread bring-up

**Source:** `004-phase-0.md` Video playback ("Decoder library:
`ffmpeg` bindings or `symphonia` + a video codec crate (decision
belongs to v0.4 implementation, not roadmap)")
**Type:** decision + engine
**Depends on:** P0.1.2, P0.3.1.
**Files:** new `specs/004-phase-0-decoder-decision.md` (decision
record, **first commit on the branch**); new `src/video_layer/`
(directory mirroring `src/svg_layer/`); `Cargo.toml`;
`src/render/texture_upload.rs` (producer integration);
`src/project/schema.rs` (`LayerKind::Video` from P0.1.2 gains
real fields).

**What:** combine the decoder selection and the initial
background-thread bring-up into one PR. First commit is the
decision record (so the rationale is reviewable before code
ships); subsequent commits are the integration. Spawning a
spike-only PR creates a stranded artefact.

**Steps:**
1. Evaluate `ffmpeg-next`, `symphonia` + a codec crate, and any
   other viable mp4 / H.264 option. Build a throwaway prototype
   with each: open an mp4, decode N frames, hand them to a
   `wgpu::Texture`. Measure time-to-first-frame, steady-state
   decode cost, build complexity (system deps, licensing), API
   ergonomics.
2. Write `specs/004-phase-0-decoder-decision.md` with the matrix,
   the decision, and the rationale. Commit.
3. Add the chosen crate to `Cargo.toml` behind a new `video`
   cargo feature (default-on; opt-out via `--no-default-features`).
   If the chosen crate needs system deps (e.g. `ffmpeg-next`
   needs ffmpeg dylibs), coordinate with **P0.9.4** which lands
   the `make setup` + `cargo bundle` plumbing.
4. Create `src/video_layer/` directory with `mod.rs`, `worker.rs`
   (mirroring `src/svg_layer/worker.rs`).
5. `VideoLayerWorker::start(path) -> Sender<VideoControl>` —
   returns a control channel carrying `Play | Pause | SetSpeed(f32)
   | SetLoop(bool) | Stop`.
6. Worker decoder loop: in the `Play` state, decode the next
   frame, push onto the texture-upload queue, sleep for
   `1.0 / fps` modulated by rate. On EOF: seek to 0 and continue
   (seamless loop). On `Pause`: **block on `Receiver::recv` until
   the next control message** (don't use `thread::park` — `unpark`
   coalesces wakes silently and is hard to reason about under
   rapid play/pause toggles).
7. Layer state in the project schema: `LayerKind::Video { path:
   PathBuf, speed: f32, loop_seamless: bool }` — replacing the
   placeholder fields from P0.1.2.
8. Audit: missing files surface as audit warnings (mirror the
   image-layer audit at `src/project/audit.rs`).

**Tests:**
- Integration test: load a fixture mp4 (small, in `tests/`), run
  one second of decode, assert the texture-upload queue received
  ≥ 24 frames.
- Loop test: decode past EOF, assert seamless wrap.
- Pause/play test: send Pause, then Play; worker resumes within
  one frame interval.
- Audit test: missing-file project surfaces a warning.

**Acceptance:**
- [ ] Decision record committed under `specs/` as the first
      commit.
- [ ] One decoder crate added to `Cargo.toml` behind a `video`
      feature (default-on).
- [ ] `cargo build --no-default-features` still succeeds.
- [ ] Per-layer worker thread decodes mp4 / H.264.
- [ ] Frames land in the texture-upload queue.
- [ ] Seamless loop works (no perceptible pause at wrap).
- [ ] Pause / play resumes via the control channel (no
      `thread::park`).
- [ ] Missing file → audit warning.

**Out of scope:** render integration (P0.4.2); UI controls
(P0.4.3); thumbnail scrubbing, in/out points, rate including
reverse, BPM-locked playback — all **Phase 1**.

---

### P0.4.2 — `LayerKind::Video` render integration

**Source:** `004-phase-0.md` Video playback (texture upload
through GPU each frame)
**Type:** render
**Depends on:** P0.4.1.
**Files:** `src/render/`, `src/project/schema.rs` (`LayerKind`
match arms in render dispatch).

**What:** the render path consumes the texture uploaded by the
video worker (P0.4.1) and draws it as a textured quad through the
existing per-layer warp + mask + effects pipeline. Existing
features (blend mode, opacity, transform, color/blur effects)
all apply to video frames identically to still images.

**Steps:**
1. Read `src/render/CLAUDE.md` for the per-frame render-graph
   order.
2. Replace the placeholder `LayerKind::Video` render path
   (P0.1.2) with a real binding to the texture handle owned by
   the layer's worker.
3. Confirm: warp + mask + every existing effect chain stage
   apply to video frames the same way they apply to image
   layers. The "every layer is a textured quad after upload"
   invariant should make this near-free.
4. Add a golden-image test: a fixture single-frame mp4 renders
   identically to the same frame as a PNG.

**Tests:**
- Golden-image test under `--features gpu-tests`: video frame
  rendered with no effects matches PNG of same frame within 1
  LSB tolerance.
- Golden test: video + Color effect produces the same output as
  PNG + Color effect.
- Manual smoke: drag an mp4 onto the canvas; warp + mask + blur
  all work.

**Acceptance:**
- [ ] `LayerKind::Video` renders the live texture.
- [ ] Warp / mask / effects apply identically to video and stills.
- [ ] Golden tests pass.

**Out of scope:** UI affordances on the left rail beyond what
exists for image layers — Phase 1.

---

### P0.4.3 — playback speed control + per-layer settings UI

**Source:** `004-phase-0.md` Video playback ("Seamless loop,
configurable playback speed")
**Type:** UI + Mutation
**Depends on:** P0.4.2.
**Files:** `src/windows/control_panel.rs` (Selected-layer card),
`src/project/command.rs` (Mutation variant + `ReverseStorage`
impl).

**What:** Selected-layer card for `LayerKind::Video` exposes:
playback speed slider (0.25× to 4×), seamless loop toggle.
Mutations are undoable per `src/project/CLAUDE.md` (whole-enum
Reverse for the LayerKind, plus per-field Mutations following the
v3 pattern).

**Steps:**
1. Selected-layer card detects `LayerKind::Video` and renders the
   video-specific row.
2. Mutation: `SetVideoSpeed { layer: usize, prev: f32, new: f32 }`
   with `ReverseStorage` impl.
3. Mutation: `SetVideoLoopSeamless { layer: usize, prev: bool,
   new: bool }` with `ReverseStorage` impl.
4. Both mutations push a control message to the worker thread
   (P0.4.1's `Sender<VideoControl>`).
5. Speed change does not re-encode; rate is applied at decode-time
   sleep in the worker.

**Tests:**
- Mutation proptest: speed + loop round-trip through snapshot
  + undo.
- Manual smoke: change speed mid-playback, confirm no visual
  artefact.

**Acceptance:**
- [ ] Speed slider + loop toggle render in the Selected-layer
      card.
- [ ] Both are undoable; proptest covers them.
- [ ] Speed change takes effect within one frame.

**Out of scope:** reverse playback (Phase 1); BPM-locked playback
(Phase 1).

---

## Workstream 5 — FX layer foundations (Anchor B kickoff)

Land the engine prerequisites for Phase 2's full FX preset library:
the `FxLayer` variant, SDF distance + gradient as fragment-shader
inputs, and one proof-point preset.

### P0.5.1 — `LayerKind::FxLayer` data fields + Mutations

**Source:** `004-phase-0.md` FX layer foundations ("Layer enum
gains an `FxLayer` variant alongside Image / SVG / (now) Video")
**Type:** engine (schema + Mutation)
**Depends on:** P0.1.2.
**Files:** `src/project/schema.rs` (`LayerKind::FxLayer` real
fields), `src/project/command.rs` (Mutation variants),
`src/project/audit.rs` (unknown-preset warning).

**What:** populate the `LayerKind::FxLayer` placeholder from
P0.1.2 with real fields. Render still placeholder until P0.5.3
ships the actual shader; this task lands the data model + undo
surface so P0.5.2 / P0.5.3 can build on it without churn.

**Steps:**
1. Replace the P0.1.2 placeholder fields with: `LayerKind::FxLayer
   { preset_id: String, params: HashMap<String, f32> }`. (String-
   keyed params are sufficient at v0.4 — the proof preset typed-
   field optimisation can wait until Phase 2 brings the full
   library.)
2. Read `src/project/CLAUDE.md` rules 1 + 2.
3. Mutation: `SetFxLayerPreset { layer, prev_kind: LayerKind, new:
   String }` — uses **whole-LayerKind Reverse** because changing
   preset_id changes the variant's payload structure (per rule 1).
4. Mutation: `SetFxLayerParams { layer, prev: HashMap<String,
   f32>, new: HashMap<String, f32> }` — snapshots the **whole map**
   on each edit, not individual entries. Per rule 1 (variant-
   replacement loses unrelated fields silently otherwise) the
   per-key approach risks dropping unrelated keys when a preset
   change races a param edit.
5. Audit: an `FxLayer` with an unknown `preset_id` surfaces a
   warning. Mirror the missing-image audit shape.
6. Render path stays placeholder — P0.5.3 wires the actual shader
   dispatch.

**Tests:**
- Mutation proptest covers both new variants under all three
  Reverse-storage rules.
- Snapshot round-trip preserves the variant identically (including
  empty + populated `params`).
- Audit test: unknown `preset_id` surfaces a non-fatal warning.

**Acceptance:**
- [ ] `LayerKind::FxLayer { preset_id, params }` carries real
      fields.
- [ ] Both Mutations exist with `ReverseStorage` impls; proptest
      coverage extended.
- [ ] Unknown preset is non-fatal (audit warning + placeholder
      render until P0.5.3).
- [ ] `make ci` clean.

**Out of scope:** the SDF inputs (P0.5.2); the proof preset
shader + render integration (P0.5.3).

---

### P0.5.2 — expose SDF distance + gradient to effect shaders

**Source:** `004-phase-0.md` FX layer foundations ("Mask SDF
distance + gradient (already present in `src/render/sdf.rs`)
exposed to effect shaders as fragment inputs, not just as alpha")
**Type:** render / shader
**Depends on:** P0.5.1.
**Files:** `src/render/sdf.rs`, `src/render/shaders/*.wgsl`,
WGSL include / bind-group plumbing.

**What:** the existing SDF computation already produces distance +
gradient at every fragment; today only the alpha is consumed.
Pass distance + gradient to effect shaders as named fragment
inputs.

**Steps:**
1. Read `src/render/sdf.rs` to see the existing SDF computation.
2. Extend the per-fragment data structure (or the bind group) to
   carry `sdf_distance: f32` and `sdf_gradient: vec2<f32>`.
3. Update the WGSL shader interface (header / common include) so
   any effect shader can reference these inputs by name.
4. Confirm via `build.rs` (naga validation) that all existing
   shaders still parse with the extended interface.
5. No behaviour change for Image / SVG / Video layers — they
   already ignore the new inputs.

**Tests:**
- Build-time validation: every shader still parses (covered by
  `build.rs`).
- Golden test: existing shaders produce bit-exact output (the
  new inputs must not perturb anything that doesn't read them).

**Acceptance:**
- [ ] `sdf_distance` + `sdf_gradient` available to every fragment
      shader.
- [ ] No regression in existing layer rendering (golden tests
      bit-exact).
- [ ] `cargo build` parses every shader (build.rs naga path).

**Out of scope:** any specific shader that consumes the new
inputs (P0.5.3).

---

### P0.5.3 — `Mask-edge ripple wash` preset + render integration

**Source:** `004-phase-0.md` FX layer foundations ("One proof-
point preset — `Mask-edge ripple wash` — demonstrates the shader
path end-to-end")
**Type:** shader + render + content
**Depends on:** P0.5.1, P0.5.2.
**Files:** new `src/render/shaders/fx_ripple_wash.wgsl`, new
`src/render/fx_presets.rs` (preset registry — lives next to the
render code, not under a non-existent `src/layers/`),
`src/render/` (replace P0.5.1's placeholder render with the
preset dispatch).

**What:** ship one FX preset (the proof point) **and** wire the
real render path. Up to this task, `LayerKind::FxLayer` rendered
as a placeholder; this task makes it dispatch the preset's
shader. FxLayer renders procedurally to an intermediate texture;
the existing per-layer effect chain (Color → Blur → Transform)
runs against that texture unchanged.

**Steps:**
1. Write `fx_ripple_wash.wgsl`: fragment computes `ripple =
   sin(sdf_distance * wavelength - time * speed) *
   exp(-sdf_distance / falloff)`; colour = `base_colour * (0.5 +
   0.5 * ripple)`. Consumes the SDF inputs from P0.5.2.
2. Create `src/render/fx_presets.rs`: a preset registry mapping
   `preset_id` → (shader handle, parameter list with defaults).
3. Replace P0.5.1's placeholder render: when `LayerKind::FxLayer`
   is dispatched, look up the preset, render the shader to a
   layer-sized intermediate texture, then feed that texture into
   the existing layer pipeline (warp + mask + effect chain) so
   downstream features Just Work.
4. Ship a demo project (`assets/demos/`) that drops the preset
   onto a polygon mask.
5. Golden-image test under `--features gpu-tests`.

**Tests:**
- Golden test: preset rendered against a fixture polygon mask
  matches a baseline image.
- Golden test: preset + Color effect produces the expected
  combination (proves the effect chain runs after FX render).
- Manual smoke: draw a polygon mask in the editor, pick the
  preset, confirm the ripple emanates from the edge.

**Acceptance:**
- [ ] `fx_ripple_wash.wgsl` parses (build.rs naga).
- [ ] Preset registered + selectable from the FxLayer parameter
      surface.
- [ ] Real shader dispatch replaces P0.5.1's placeholder render.
- [ ] Effect chain (Color/Blur/Transform) runs against the FX
      output unchanged.
- [ ] Demo project loads and renders the preset.
- [ ] Golden tests pass.

**Out of scope:** the full preset library — Phase 2; the richer
FX pipeline (emitter / force-field / render stages) — Phase 2.

---

## Workstream 6 — NDI input

Receive NDI streams as layer sources. Distinct from Phase 7 NDI
output.

### P0.6.1 — NDI SDK Rust binding selection + integration

**Source:** `004-phase-0.md` NDI input layer ("Requires the NDI
SDK and a Rust binding")
**Type:** decision + integration
**Depends on:** none.
**Files:** new `src/ndi_layer/` directory (mirroring
`src/svg_layer/`) with `mod.rs`, `Cargo.toml` (new `ndi` cargo
feature, **default-on** — phase 0 acceptance requires NDI input
to work in a default build), `docs/ndi-setup.md` (operator-facing
setup notes for SDK install).

**What:** evaluate Rust NDI bindings (`ndi-rs`, etc.), pick one,
write a minimal "list NDI sources on the network" smoke. The
chosen crate ships behind `--features ndi` (default-on) with a
clear build-time error if the NDI SDK isn't installed. NDI
licensing notes go in `docs/ndi-setup.md`.

**Steps:**
1. Audit available crates; check NDI SDK licensing requirements
   for redistribution.
2. Pick a binding; add behind the new `ndi` cargo feature.
   **Default-on** in `[features]` so `cargo build` satisfies
   v0.4 acceptance. `--no-default-features` opts out for users
   who can't install the SDK.
3. The build script (or the crate's own build script) produces a
   clear error pointing at `docs/ndi-setup.md` when the SDK isn't
   on the system.
4. Implement `ndi::list_sources() -> Vec<NdiSourceInfo>` and a
   smoke test (manual: run on a network with another NDI source
   and confirm enumeration).
5. Write `docs/ndi-setup.md` with how operators install the SDK
   on macOS (and Windows/Linux for completeness, even though
   v0.4 is macOS-only).
6. Coordinate with **P0.9.4** (build deps) for `make setup` and
   `cargo bundle` integration if the SDK needs runtime dylibs in
   the `.app`.

**Tests:**
- Manual smoke: enumerate sources on a test network.
- Unit test: with a stub provider, enumeration returns the
  canned list.

**Acceptance:**
- [ ] One NDI binding crate added behind `--features ndi`
      (default-on).
- [ ] `cargo build --no-default-features` still succeeds.
- [ ] Missing SDK produces an actionable build error.
- [ ] `ndi::list_sources()` works on a real network.
- [ ] `docs/ndi-setup.md` exists.

**Out of scope:** the receiver thread (P0.6.2); the audit (P0.6.3).

---

### P0.6.2 — `LayerKind::Ndi` receiver + render integration

**Source:** `004-phase-0.md` NDI input layer ("Receive an NDI
stream as a layer source")
**Type:** engine + render
**Depends on:** P0.6.1, P0.3.1, P0.1.2.
**Files:** `src/ndi_layer/` (worker module), `src/render/`,
`src/project/schema.rs` (`LayerKind::Ndi` real fields),
`src/render/texture_upload.rs` (producer integration).

**What:** spawn a per-NDI-layer receiver thread that polls frames
from the NDI source and pushes them into the texture-upload
queue. Render path consumes them like video. Reconnect logic
lives here (the receiver owns its connection lifecycle).

**Steps:**
1. `NdiReceiver::start(source: NdiSourceInfo) -> JoinHandle` —
   returns the worker handle.
2. Worker loop: `recv_video` (blocking with timeout), push frame
   bytes onto the texture-upload queue. On `recv_video` error or
   timeout exceeding 5 s with no frame: drop the connection,
   sleep 5 s, attempt to reopen. The runtime `connected: bool`
   on the layer is updated on every state change.
3. Replace placeholder `LayerKind::Ndi` render with a real bind
   to the live texture (mirror P0.4.2).
4. Project schema: `LayerKind::Ndi { source_name: String }` —
   replacing the placeholder fields from P0.1.2. The operator
   selects an NDI source by name.

**Tests:**
- Integration test: with a stub `NdiReceiver` returning canned
  frames, the texture-upload queue receives them and the layer
  renders.
- Reconnect test: stub source disappears, `connected` flips to
  false within 5 s; stub source returns, `connected` flips back
  to true within ~5 s.
- Manual smoke: another machine on the network running OBS NDI
  output appears as a selectable source; dropping it onto the
  canvas renders the live stream.

**Acceptance:**
- [ ] Per-layer NDI receiver thread.
- [ ] Frames land in the texture-upload queue.
- [ ] Render path draws live NDI through warp + mask + effects.
- [ ] Reconnect within ~5 s of source returning.

**Out of scope:** load-time audit + UI badge (P0.6.3).

---

### P0.6.3 — NDI source-unavailable audit + UI badge

**Source:** `004-phase-0.md` NDI input layer ("Project audit
warns when a referenced NDI source is offline at load")
**Type:** audit + UI
**Depends on:** P0.6.2 (which already owns the reconnect logic
and the `connected: bool` runtime state).
**Files:** `src/project/audit.rs`, left-rail UI module (search
for the v3 layer-row rendering).

**What:** at project load, every `LayerKind::Ndi` checks whether
its named source is enumerable. If not, audit warning + "source
unavailable" badge on the layer row in the left rail. The badge
follows the runtime `connected: bool` set by the receiver
(P0.6.2), so reconnect causes the badge to clear without any
explicit work here.

**Steps:**
1. Add a new `AuditKind::NdiSourceUnavailable { source_name:
   String }` to `src/project/audit.rs` (mirror the shape of
   `OutputTargetUuidNotFound` at line 142).
2. Walk every `LayerKind::Ndi` at audit time; call
   `ndi::list_sources()`; emit the warning per missing source.
3. Wire the left-rail layer row to read the runtime `connected`
   field and render a "source unavailable" badge when false.
4. Glossary popover from P0.1.4 (`NDI source` term) attaches to
   the badge.

**Tests:**
- Integration test: load a project with a non-existent NDI
  source, assert audit warning + badge.
- Integration test: with a stub source that comes online
  mid-session, badge clears within ~5 s (driven by P0.6.2's
  reconnect loop).

**Acceptance:**
- [ ] `AuditKind::NdiSourceUnavailable` exists.
- [ ] Project audit warns on missing NDI sources at load.
- [ ] "Source unavailable" badge renders on the layer row when
      `connected` is false.

**Out of scope:** the reconnect loop itself (P0.6.2 owns it); UI
for changing the source name (Phase 6).

---

## Workstream 7 — Two-projector edge-blend stub

Light up a second `OutputWindow` on a second monitor. Single
logical canvas spans both projectors; the overlap is the edge-blend
region.

### P0.7.1 — `Vec<OutputTarget>` UI + launcher multi-output picker

**Source:** `004-phase-0.md` Two-projector section ("the launcher's
projector picker recognises both displays"); roadmap I15
("Clickable monitor selector + test-pattern affordance").
**Type:** schema + UI
**Depends on:** P0.1.2 (schema already carries the vec; this
exposes it).
**Files:** `src/app.rs` (launcher path), `src/project/schema.rs`,
`src/test_patterns.rs` (re-use the existing TestPatternRenderer
for the per-monitor identification flash).

**What:** the launcher's projector picker grows to a multi-output
selector — operator picks zero, one, or two displays. Per row,
an "identify" button flashes a test pattern on that physical
display so operators can confirm which monitor is which.

**Steps:**
1. Locate the launcher's projector picker UI in `src/app.rs`.
2. Replace the single-display dropdown with a list of rows (one
   per detected monitor): `[checkbox] [name + resolution]
   [identify]`.
3. **Constraint:** at most 2 checkboxes selectable. The third
   click on an already-2-selected list is ignored with an inline
   hint ("Phase 7 grows multi-output beyond 2 projectors").
4. The "identify" button opens a borderless window on that
   monitor for 5 s rendering `TestPattern::Crosshair` (already
   exists per `src/test_patterns.rs`), then closes itself. Re-uses
   the v3 launcher's identify-flow if one exists; otherwise add
   a minimal one.
5. Save the selection as `Vec<OutputTarget>` (UUIDs prioritised,
   index fallback per the v3.1 OutputTarget model — the
   `OutputTarget` type already lives at `schema.rs:155`).
6. UI hint: zero outputs = editor-only (preview window only); one
   output = standard v3 behaviour; two = the second-window flow
   (P0.7.2) takes over.
7. Update `src/project/audit.rs` so the existing
   `OutputTargetUuidNotFound` audit walks the vec instead of the
   singular field.

**Tests:**
- Integration test: load a project with two `output_targets`,
  confirm the launcher renders both as selected.
- Unit test: attempting to select a third checkbox is a no-op.
- Manual smoke: connect a second monitor, refresh the picker,
  click "identify" on each row, confirm flash on the right
  display.

**Acceptance:**
- [ ] Launcher renders a multi-output selector with one row per
      monitor.
- [ ] At most 2 selectable; over-select is a no-op with hint.
- [ ] Identify button flashes a 5 s crosshair on the chosen
      display.
- [ ] Selection saves as `Vec<OutputTarget>`.
- [ ] Zero / one / two outputs all load correctly.

**Out of scope:** the second OutputWindow lifecycle (P0.7.2);
edge-blend rendering (P0.7.3).

---

### P0.7.2 — second `OutputWindow` lifecycle

**Source:** `004-phase-0.md` Two-projector section ("Second
`OutputWindow` on a second monitor")
**Type:** engine
**Depends on:** P0.7.1.
**Files:** `src/app.rs` (`EditingState.output: OutputWindow` at
line 304 / 377 / 1482 — currently singular; needs a structural
extension), `src/windows/output.rs` (`OutputWindow`),
`src/render/` (render-graph extension to drive multiple surfaces),
`src/show_day/sleep_assertion.rs`.

**What:** on `output_targets.len() == 2`, spawn a second
`OutputWindow` on the second monitor. The "single logical canvas
spans both projectors" model means each window draws **its own
viewport into the shared logical canvas** — not a copy of the
same render. Closing one window doesn't panic; reopening works.

**Steps:**
1. Read `src/render/CLAUDE.md` for the GPU bring-up split (device
   before surface) and per-frame render-graph order.
2. Extend `EditingState` from `output: OutputWindow` to a
   container holding 1–2 `OutputWindow`s (e.g. `outputs:
   SmallVec<[OutputWindow; 2]>`). The init path
   (`init_output_window` at `src/app.rs:1497`) becomes one call
   per active target.
3. Per-frame render: each `OutputWindow` performs its own render
   pass against its own surface, drawing the slice of the
   logical canvas that maps to its physical viewport. The
   "render canvas once and copy" framing is wrong — surfaces
   have different sizes and pixel formats; each gets its own
   command encoder.
4. Audit at `src/project/audit.rs:250` (the
   `output_target.fallback_index >= monitor_count` check) walks
   every entry in the vec.
5. Window-close handling: closing either window removes the
   matching `OutputTarget` from the vec; the surviving window
   keeps running.
6. Sleep-assertion (`src/show_day/sleep_assertion.rs`) holds an
   IOPMAssertion per active display.

**Tests:**
- Integration test (with manual setup): two windows on two
  displays render correctly — each shows its slice of the canvas.
- Test: closing window 1 leaves window 2 functional and the vec
  shrinks to length 1.
- Test: closing window 2 leaves window 1 functional and the
  output target list shrinks.

**Acceptance:**
- [ ] `EditingState` carries 1–2 `OutputWindow`s.
- [ ] Each window renders its own viewport into the logical
      canvas (no panics on different surface sizes).
- [ ] Close-and-reopen of either window doesn't panic.
- [ ] Sleep is suppressed on both displays during go-live.

**Out of scope:** edge-blend rendering (P0.7.3); test patterns
(P0.7.4); Output mode pill (P0.7.5).

---

### P0.7.3 — edge-blend overlap region rendering

**Source:** `004-phase-0.md` Two-projector section ("Shared blend
region with configurable overlap and falloff")
**Type:** render / shader
**Depends on:** P0.7.2.
**Files:** `src/render/`, new
`src/render/shaders/edge_blend.wgsl`.

**What:** the configured overlap region between the two projectors
gets a falloff (gradient blend) applied at present time, so the
seam where the two beams cross becomes invisible.

**Steps:**
1. Add `EdgeBlendConfig { overlap_px: u32, falloff_curve:
   FalloffCurve }` to the project schema (per-edge configurable).
2. Write `edge_blend.wgsl` — a multiply-blend that ramps brightness
   from 1.0 → 0.0 across the overlap region, applied per-output
   surface at present time (NOT in the canvas; the canvas itself
   stays full-brightness).
3. Render integration: both `OutputWindow`s sample the same
   canvas slice in their overlap region but apply the
   complementary falloff curve so the sum equals 1.0.
4. UI: per-edge overlap + falloff sliders (live in the OutputPanel
   from W8.1).

**Tests:**
- Golden test: against a fixed reference projector gamma (2.2 by
  default), the rendered overlap matches a baseline image. The
  baseline is what an aligned-and-blended seam looks like; the
  acceptance is "matches baseline", not "linear sum to 1.0"
  (real edge-blend uses a gamma-corrected curve, not linear).
- Manual smoke: physically project onto a wall with two
  projectors, confirm the seam is invisible.

**Acceptance:**
- [ ] Per-edge overlap + falloff config in schema.
- [ ] Falloff WGSL parses + runs.
- [ ] Golden test against the gamma-corrected baseline passes.
- [ ] Manual smoke: no visible seam at typical projector gamma.

**Out of scope:** full automatic calibration — Phase 7;
hardware-measurement-driven gamma curve fitting — Phase 7.

---

### P0.7.4 — edge-blend gradient + alignment cross test patterns

**Source:** `004-phase-0.md` Two-projector section ("Edge-blend
gradient + alignment cross extend the existing `T` test-pattern
cycle")
**Type:** content
**Depends on:** P0.7.2.
**Files:** `src/test_patterns.rs` (the `TestPattern` enum + the
`TestPatternRenderer` already exist; today's variants are `None
| Grid50 | Crosshair | White100 | White50 | White25 | ColorBars`),
new WGSL shaders under `src/render/shaders/`.

**What:** extend the `T` cycle with two new patterns: an
edge-blend gradient (horizontal 0→1 ramp across the canvas,
useful for verifying overlap falloff) and an alignment cross
(centred on each physical output, with quarter / half / three-
quarter reference markings, for two-projector physical
alignment).

**Steps:**
1. Read `src/test_patterns.rs` — `TestPattern` is a `Copy` enum,
   `TestPatternRenderer::render` matches every variant
   exhaustively at line 217.
2. Add `TestPattern::EdgeBlendGradient` and
   `TestPattern::AlignmentCross` to the enum. The exhaustiveness
   check at line 217 will force the renderer match arms to
   compile.
3. `EdgeBlendGradient`: horizontal 0→1 luminance ramp across
   the full canvas. Useful for verifying P0.7.3's falloff
   curve under a real projector gamma.
4. `AlignmentCross`: centred cross plus 25% / 50% / 75% tick
   marks on each axis. When two projectors are active each
   draws its own alignment cross within its own viewport
   (per-output rendering from P0.7.2).
5. Glossary popover from P0.1.4 attaches if a "test pattern"
   entry exists in the glossary already (it does — see
   `GlossaryTerm::TestPattern` in `src/windows/glossary.rs`).

**Tests:**
- Golden test per new pattern under `--features gpu-tests`.
- Manual smoke: press `T` repeatedly, confirm both new patterns
  appear in the cycle.

**Acceptance:**
- [ ] `TestPattern::EdgeBlendGradient` and `AlignmentCross` exist.
- [ ] Both render without media on the canvas.
- [ ] Each is reachable via the `T` cycle.
- [ ] Glossary entry for "test pattern" mentions the new members.

**Out of scope:** dot grid, colour bars, focus chart, geometry
verify — those are roadmap §9.2 follow-ons for Phase 7.

---

### P0.7.5 — `Output` mode pill in the M3 mode cluster

**Source:** `004-phase-0.md` UX items resolved ("M3 capability
follow-on — mode pill cluster grows toward *Output* / *Cue*
peers; the second projector landing here makes *Output* the
natural first new peer pill").
**Type:** UI
**Depends on:** P0.7.2, P0.8.1.
**Files:** `src/windows/control_panel.rs` (the v3 mode pill
cluster — Warp / Mask / Content).

**What:** the v3 mode pill cluster is `Warp · Mask · Content`.
With two projectors live, add `Output` as a fourth peer pill;
clicking it opens the `OutputPanel` (P0.8.1) as the right-rail
content, replacing the Selected-layer card while active.

**Steps:**
1. Locate the mode pill cluster in `control_panel.rs`. Add
   `ModePill::Output` to the enum (or equivalent shape).
2. Pill is **only visible when `output_targets.len() >= 1`** —
   no point in an Output mode for editor-only projects.
3. Active state: opens `OutputPanel` (P0.8.1) and tints the
   canvas border per the v3 mode-tint convention (I11). Choose
   a colour distinct from Warp / Mask (cool desaturated, e.g.
   teal).
4. Keyboard binding: extend the v3 mode pill chord set with
   one for Output (consistent with how Warp/Mask/Content
   currently bind, if they do — verify).

**Tests:**
- Manual smoke: with one or two projectors configured, click
  Output, panel opens, canvas border tints.
- Test: with zero projectors, the Output pill is hidden.

**Acceptance:**
- [ ] `ModePill::Output` exists in the cluster.
- [ ] Visible only when `output_targets.len() >= 1`.
- [ ] Clicking opens `OutputPanel`.
- [ ] Canvas border picks up the mode tint.

**Out of scope:** `Cue` mode pill (Phase 6).

---

## Workstream 8 — Output panel + per-projector colour calibration

Recommendation K kickoff. The persistent Output badge collapses
into an Output panel hosting per-output controls: gamma trim,
edge-blend slider, RGB matrix.

### P0.8.1 — `OutputPanel` scaffold (badge stays for 1 projector)

**Source:** `004-phase-0.md` Two-projector section ("The Output
**panel** (rather than badge) starts here too — see
`specs/roadmap.md` §7 Recommendation K")
**Type:** UI
**Depends on:** P0.7.2.
**Files:** new `src/windows/output_panel.rs`,
`src/windows/control_panel.rs` (host the panel + conditional
badge → panel switch).

**What:** the v3.1 Output badge becomes an `OutputPanel` **only
when `output_targets.len() >= 2`**. With a single projector the
v3.1 badge stays as-is — Recommendation K explicitly says the
badge "starts to collapse out of an Output panel **as the second
projector arrives**". The panel docks into the right-side region
(M5 docking model) — it never adds a new column.

**Steps:**
1. Read roadmap Appendix B (Recommendation K) and §6 M5 (panel
   docking model — every new surface docks into the right-side
   region).
2. Implement `OutputPanel` as an egui region with one sub-card
   per output target. Docks into the right rail; opening it
   replaces the Selected-layer card while active (mirrors the
   M3 mode pill behaviour from P0.7.5).
3. Branch on `output_targets.len()`:
   - 0 → no badge, no panel.
   - 1 → v3.1 badge only (no panel).
   - ≥2 → badge becomes a panel-toggle; clicking opens the
     `OutputPanel`.
4. Sub-card content per output:
   - Preview thumbnail (re-uses the v3.1 preview-thumbnail
     widget if landed; placeholder otherwise).
   - Gamma trim slider (already exists per-display in v3 — move
     it into the sub-card; the per-display schema field stays
     where it is).
   - Edge-blend overlap + falloff sliders (from P0.7.3) appear
     only on the sub-card representing the projector that
     borders the overlap.
   - RGB matrix card (placeholder slot; populated by P0.8.3).

**Tests:**
- Manual smoke (1 projector): badge unchanged from v3.1.
- Manual smoke (2 projectors): badge expands into the panel on
  click; sub-cards render correctly for both outputs.

**Acceptance:**
- [ ] `OutputPanel` exists.
- [ ] 1-projector path: v3.1 badge unchanged.
- [ ] 2-projector path: badge opens the panel.
- [ ] One sub-card per output target.
- [ ] Existing per-display gamma trim moved into the sub-card.
- [ ] Edge-blend sliders (P0.7.3) appear in the panel.
- [ ] Panel docks into the right-side region (no new columns).

**Out of scope:** the RGB matrix UI (P0.8.3); Phase 7's full
calibration verify.

---

### P0.8.2 — RGB matrix render path

**Source:** `004-phase-0.md` Per-projector colour calibration
("Extends the existing per-display gamma / brightness / contrast
override with a full RGB matrix")
**Type:** render + Mutation
**Depends on:** P0.8.1, P0.1.2 (which already adds
`OutputTarget.rgb_matrix` defaulting to identity).
**Files:** `src/render/`, new `src/render/shaders/rgb_matrix.wgsl`
(or extend the existing gamma pass), `src/project/command.rs`
(Mutation variant + `ReverseStorage` impl).

**What:** wire the per-output RGB matrix render path. The schema
field already lives on `OutputTarget` thanks to P0.1.2; this task
is render + Mutation only — no further schema bump.

**Steps:**
1. Confirm `OutputTarget.rgb_matrix` exists (P0.1.2 added it,
   defaulting to identity).
2. Apply the matrix in the per-output present-time pipeline,
   after gamma / brightness / contrast (so the matrix sees
   corrected luminance, not raw scene values).
3. Mutation: `SetOutputRgbMatrix { output_index: usize, prev:
   [[f32; 3]; 3], new: [[f32; 3]; 3] }` with `ReverseStorage`
   impl. Per-cell edits flow through this Mutation as
   whole-matrix replacements (whole-enum-style, simpler than
   per-cell).

**Tests:**
- Mutation proptest: matrix round-trips through snapshot + undo
  bit-exact.
- Golden test: identity matrix produces pixel-identical output
  to the un-matrixed pipeline (within 1 LSB tolerance).
- Golden test: a 50%-red-channel-only matrix produces the
  expected output.

**Acceptance:**
- [ ] Render path applies `rgb_matrix` per-output.
- [ ] Identity matrix is bit-exact equivalent to no-matrix path.
- [ ] Mutation proptest covers the new variant.
- [ ] Mutation undo restores matrix bit-exact.

**Out of scope:** the matrix-editing UI (P0.8.3); RGBW + colour-
temperature mixing — Phase 7.

---

### P0.8.3 — RGB matrix editing UI

**Source:** `004-phase-0.md` Per-projector colour calibration
("manual adjustment tool beyond the current slider trio")
**Type:** UI
**Depends on:** P0.8.2.
**Files:** `src/windows/output_panel.rs`.

**What:** the OutputPanel's RGB matrix card renders a 3×3 grid of
spinners (-2.0..=2.0 each cell), an "identity" reset button, and
a "calibrate" button stub (Phase 7 will fill the calibrate flow).

**Steps:**
1. 3×3 spinner grid in the matrix card.
2. "Reset to identity" button → Mutation that writes the identity
   matrix.
3. "Calibrate" button → disabled with tooltip "Hardware
   measurement workflow — Phase 7".
4. Visible feedback when matrix is non-identity (the card title
   shows a small dot).

**Tests:**
- Manual smoke: edit a cell, observe the projected output change
  in real-time; reset → identity restored; undo restores prior
  values.

**Acceptance:**
- [ ] 3×3 spinner grid edits the matrix live.
- [ ] Identity reset works and is undoable.
- [ ] Non-identity state is visually distinct.

**Out of scope:** automated calibration workflow (Phase 7).

---

## Workstream 9 — Release housekeeping

Tail-end work to ship v0.4.0.

### P0.9.1 — version bump + `release-show` profile validation

**Source:** v0.4 release framing
**Type:** release
**Depends on:** every other workstream.
**Files:** `Cargo.toml`, `src/main.rs` (version string if any).

**What:** bump the version from v0.3.x to v0.4.0; verify the
`release-show` profile (LTO=fat, panic=abort, stripped) builds
cleanly with all phase 0 capabilities enabled.

**Steps:**
1. Bump `version` in `Cargo.toml`.
2. `make build-show` — confirm clean build.
3. `make bundle` — confirm `.app` produces.
4. Smoke: launch the bundle, drop a video + an FX layer + an
   NDI source onto the canvas, run for 10 minutes, confirm no
   panics, no leaks (Activity Monitor RSS stable).

**Tests:**
- Manual: 10-minute soak.
- CI: `make ci` clean.

**Acceptance:**
- [ ] Version bumped to 0.4.0.
- [ ] `make build-show` + `make bundle` clean.
- [ ] 10-minute soak passes.

**Out of scope:** signing / notarisation — separate concern.

---

### P0.9.2 — `CHANGELOG.md` + `README.md` updates

**Source:** v0.4 release framing
**Type:** docs
**Depends on:** every other workstream.
**Files:** `CHANGELOG.md`, `README.md`.

**What:** write the v0.4.0 changelog entry covering every phase 0
capability; refresh README to reflect the new feature set.

**Steps:**
1. Open `CHANGELOG.md` (or create if absent).
2. Add a v0.4.0 entry organised by capability (video, FX layer,
   NDI input, two-projector, OSC + MIDI binding, per-projector
   calibration). Each line one CHANGELOG-grade sentence.
3. README updates: feature list, supported media types now
   includes mp4 / H.264 + NDI input; binding picker mentioned in
   the OSC / MIDI section.
4. Spot-check the in-app `?` button still opens the README in
   the browser (per v3 wiring).

**Tests:**
- Manual: read both docs end-to-end.

**Acceptance:**
- [ ] `CHANGELOG.md` v0.4.0 entry covers every shipped capability.
- [ ] README accurately describes v0.4.
- [ ] In-app help still opens README.

**Out of scope:** marketing copy / website.

---

### P0.9.3 — show-day checklist refresh

**Source:** `docs/show-day-checklist.md` (lives in the repo)
**Type:** docs
**Depends on:** every other workstream.
**Files:** `docs/show-day-checklist.md`.

**What:** the v3 show-day checklist documents single-projector
operation; v0.4 introduces video, FX layers, NDI, two projectors,
and bindings. Add v0.4-specific checklist items.

**Steps:**
1. Read the existing checklist.
2. Add sections for:
   - Two-projector setup: monitor identification, `pmset -g
     assertions` confirms both displays held awake, edge-blend
     gradient pattern shows 1.0 sum across overlap.
   - Video layers: confirm decode keeps up at target fps; check
     `dropped frames: 0` in the diagnostics surface during
     warm-up.
   - NDI sources: confirm both machines on the same VLAN; warn
     about firewalls.
   - MIDI controller: confirm the controller appears in the
     learn-target dropdown before the show.
   - OSC sender: confirm the patch panel receives traffic
     before go-live.

**Tests:**
- Manual: walk through the checklist on a real two-projector +
  video + binding setup.

**Acceptance:**
- [ ] Checklist covers every v0.4 surface that has a show-day
      failure mode.
- [ ] Manual walkthrough passed.

**Out of scope:** Phase 5 light-rig checks; Phase 7 NDI output
checks.

---

### P0.9.4 — system-deps for ffmpeg + NDI in `make setup` and `cargo bundle`

**Source:** P0.4.1 (decoder) and P0.6.1 (NDI) likely require
system libraries.
**Type:** build / packaging
**Depends on:** P0.4.1, P0.6.1.
**Files:** `Makefile`, `mise.toml` (if relevant), `Cargo.toml`
(`[package.metadata.bundle]`), `docs/show-day-checklist.md`.

**What:** if the chosen video decoder (P0.4.1) or the NDI binding
(P0.6.1) needs native libraries (e.g. `ffmpeg-next` requires
`ffmpeg`; the NDI SDK ships dylibs), wire `make setup` to install
or check for them on macOS and update `cargo bundle` so the
`.app` bundles the dylibs.

**Steps:**
1. Determine the actual system-deps from P0.4.1 and P0.6.1 —
   skip steps below for any dep that turns out to be self-
   contained.
2. If ffmpeg is needed: `make setup` runs `brew list ffmpeg ||
   brew install ffmpeg` (idempotent; non-fatal warning on
   non-macOS).
3. If NDI SDK dylibs are needed: ensure `cargo bundle --profile
   release-show` copies them into `Contents/Frameworks/` and
   adjusts `@rpath` so the bundled `.app` runs without a
   system-wide install. macOS `install_name_tool` is the usual
   path.
4. Update `docs/show-day-checklist.md` and the README's setup
   section.
5. Verify a bundled `.app` runs on a fresh macOS user account
   (no Homebrew) without errors.

**Tests:**
- Manual: `make setup` on a clean checkout; `make bundle`;
  launch the bundle on a separate macOS user.
- CI: smoke that `make build-show` succeeds on the CI image.

**Acceptance:**
- [ ] `make setup` installs / verifies all required system deps
      (or skips with a clear note on non-macOS).
- [ ] `cargo bundle --profile release-show` produces a
      self-contained `.app`.
- [ ] Bundled `.app` opens video + NDI on a fresh macOS account.

**Out of scope:** signing / notarisation.

---

### P0.9.5 — show-day frame-budget gate

**Source:** `004-phase-0.md` Acceptance criteria ("Show-day frame
budget is unchanged with up to four video layers, one NDI input,
two projectors, and active OSC + MIDI bindings")
**Type:** perf gate
**Depends on:** every other workstream.
**Files:** new `tests/perf_frame_budget.rs` (or similar harness
under `--features gpu-tests`), `docs/show-day-checklist.md`.

**What:** a perf gate that verifies the v0.4 acceptance line is
true. Headless wgpu adapter renders a fixture project (4 video
layers + 1 NDI input mocked + edge-blend across 2 simulated
outputs + active OSC/MIDI bindings) and measures frame time over
N seconds.

**Steps:**
1. Build a fixture project under `tests/fixtures/` with:
   - 4 `LayerKind::Video` referencing small test mp4s.
   - 1 `LayerKind::Ndi` with a stub source (no real network).
   - 2 entries in `output_targets`.
   - A handful of `Modulator::OscBound` / `MidiBound` bound to
     effect parameters; stub providers feed values.
2. Drive a headless wgpu render at 60 Hz for 10 seconds.
3. Assert: 99th-percentile frame time ≤ 16.6 ms; no drops on
   the texture-upload queue; no panic_restore triggers.
4. Document the baseline number in
   `docs/show-day-checklist.md` so regressions later have a
   reference.

**Tests:**
- The harness IS the test. Runs under `make test-gpu`.
- Manual smoke: same fixture on real hardware passes the
  10-minute soak (P0.9.1).

**Acceptance:**
- [ ] Perf gate runs in CI (`make test-gpu`).
- [ ] 99p frame time documented + asserted.
- [ ] No drops; no panic_restore triggers during the run.

**Out of scope:** profiling-driven optimisation if the gate
fails — that becomes new tasks.

---

## Cross-workstream notes

- **Schema bumps.** One bump only: v6 → v7 in P0.1.2. All new
  fields (Vec<OutputTarget>, OutputTarget.rgb_matrix, three new
  LayerKind variants with their data fields) land together under
  v7. P0.5.1 / P0.4.1 / P0.6.2 fill the placeholder fields in
  place — no further bump.
- **Cargo features added.** `video` (P0.4.1, default-on),
  `ndi` (P0.6.1, default-on with a clear build-time error if the
  NDI SDK isn't installed). Existing flips: `osc` and `midi` move
  to default-on (P0.1.3). `audio` stays opt-in.
- **System-deps.** P0.9.4 covers `make setup` + `cargo bundle`
  changes for any native libraries the chosen video decoder /
  NDI binding require.
- **Glossary attachment.** P0.1.4 lands the data; downstream
  tasks (P0.2.4, P0.5.3, P0.6.3, P0.7.4, P0.8.1) attach
  `glossary_label(...)` calls to the UI surfaces they introduce.
- **Reverse-storage rules.** Every Mutation in this phase follows
  `src/project/CLAUDE.md`'s three rules (whole-enum, effects-vec,
  snapshot) enforced via the `ReverseStorage` trait. Tasks that
  add Mutations explicitly call this out.
- **Acceptance gate for shipping v0.4.0.** Every workstream
  acceptance box checked + P0.9.5 perf gate green + 10-minute
  soak under `make build-show` (P0.9.1) + show-day checklist
  walkthrough on real hardware (P0.9.3).
