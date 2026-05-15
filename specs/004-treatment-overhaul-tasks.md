# 004 Treatment Overhaul — Task Tracker

Companion to `specs/004-treatment-overhaul.md`. Each task is one commit unless noted.
Commit-message convention: `004-T1.4: migrate_v11_to_v12 + three unit tests` (matches `003-T1.*` style per repo CLAUDE.md).

Tick (`- [x]`) on commit landing. Phases roughly correspond to spec sections A–D plus the two review amendments (fit-uniform identity buffer, preset round-trip).

> **Execution order.** The repo's pre-commit hook (`.githooks/pre-commit:28`) runs `cargo check --workspace --all-targets` on every commit, so commits **cannot** leave the workspace compile-broken. The task IDs below are stable for commit-message references, but the *execution* order is bucketed as follows:
>
> 1. **Additive precursors** — land in any order, no compile dependency on the foundation cluster: T1.1, T1.2, T1.5, T1.6, T1.13, T1.18, T1.20, T1.21, T1.22, T1.23, T1.24.
> 2. **★ FOUNDATION-CLUSTER — single merged commit** (each tagged ★ below): T1.3 + T1.4 + T1.11 + T1.12 + T1.14 + T1.15 + T1.17 + T1.19, plus a temporary stub of `show_treatment_section` in `controls.rs` so its existing `set_layer_treatment_mutation` callers (six call sites at lines 969, 1004, 1051, 1061, 1105, 1116) stay alive until T1.30 deletes the section. Without the merge the build breaks at `audit.rs:535`, `command.rs:910+`, `scene_editor.rs:261`, `controls.rs:881+`, and every `effects[idx]` deref site.
> 3. **UI build** (after the cluster): T1.7, T1.8, T1.9, T1.16, T1.25, T1.26, T1.27, T1.28, T1.29, T1.30, T1.31, T1.32.
> 4. **Mutation graveyard** — T1.10 MUST land after T1.30+T1.31 (which delete the six remaining callers in `controls.rs`).
> 5. **Verification + ship**: T1.33, T1.34, T1.35, T1.36a, T1.36b.

---

## Phase 1 — Ship (2–3 weeks)

### 1a. Schema foundation (spec A.1–A.3)

- [x] **004-T1.1** Add `EffectNode { enabled: bool, effect: Effect }` with `default_enabled_true` helper.
  - Files: `src/effects/mod.rs`
  - Accept: doctest `deserialize {"effect": {"Color": {…}}}` returns `enabled: true`. `make ci` green.

- [x] **004-T1.2** Extend `Effect::Treatment` with `#[serde(default)] overlay_path: Option<PathBuf>` and `#[serde(default)] collage_paths: Vec<PathBuf>`.
  - Files: `src/effects/mod.rs`
  - Accept: existing fixtures (`assets/presets/*.json`) still deserialise. New fields round-trip in a unit test.

- [x] **★ 004-T1.3** Drop `LayerConfig.treatment`. Change `effects: Vec<Effect>` → `Vec<EffectNode>`. Bump `CURRENT_SCHEMA_VERSION` 11 → 12.
  - Files: `src/project/schema.rs` (lines ~10, ~248, ~271)
  - **Foundation cluster — merge with T1.4, T1.11, T1.12, T1.14, T1.15, T1.17, T1.19 in one commit.**
  - Accept: full cluster compiles workspace-wide; pre-commit `cargo check --all-targets` green.

- [x] **★ 004-T1.4** Migrator `migrate_v11_to_v12_fold_treatment_into_effects` + three unit tests (folds-treatment-first, idempotent on v12, byte-for-byte with missing overlay path).
  - Files: `src/project/migrate.rs` (extend the `0..=10` arm at line 48 to `0..=11`)
  - **Foundation cluster — see T1.3.**
  - Accept: three new tests pass. Load a v11 fixture with `treatment.preset_id = "tone_map"` + 2 effects → 3 EffectNodes, all `enabled: true`.

### 1b. Render-graph plumbing fix (spec A.4 + review amendment)

- [x] **004-T1.5** Add 6 fields to `RenderCtx`: `sdf_view`, `zone_role`, `seed`, `t_layer_added_secs`, `overlay_view`, `collage_views`.
  - Files: `src/effects/mod.rs:101-147`
  - Accept: compiles; the single `RenderCtx` construction site in `app.rs` gets nullary defaults until T1.8 wires them.

- [x] **004-T1.6** Add `identity_fit_uniform: wgpu::Buffer` to `LayerState`; write once at layer init with `(mode=0, aspect=1, focal_x=0.5, focal_y=0.5)`. **Review amendment — prevents fit double-apply.**
  - Files: `src/app.rs` (LayerState construction site)
  - Accept: per-layer buffer exists, contents verified via wgpu queue read-back in a dev assert.

- [x] **004-T1.7** Rewrite the `Effect::Treatment` arm in `effects/mod.rs:295-345` to use `ctx.sdf_view`, `ctx.zone_role`, `ctx.seed`, `ctx.t_layer_added_secs`, `ctx.overlay_view`, `ctx.collage_views` instead of hardcoded nulls.
  - Files: `src/effects/mod.rs`
  - Accept: dispatch returns `true` for SDF-keyed treatments when ctx provides an `sdf_view` (was unconditionally `false`); unit test wires a stub `RenderCtx` and asserts the new branch.

- [x] **004-T1.8** Delete primary-treatment dispatch in `app.rs:4429-4559`. Make `svg_pipeline.render(...)` unconditional with `ls.fit_uniform`. Construct chain `RenderCtx` with `fit_uniform: &ls.identity_fit_uniform` (identity, **not** the per-layer one). Hoist overlay/collage texture loaders into the per-node loop for `Effect::Treatment` nodes only. Thread `ls.warp_renderer.sdf_view()`, `cfg.warp.zone_role`, `ls.layer_id.0`, `0.0` into the new ctx fields.
  - Files: `src/app.rs:4429-4593`
  - Accept (scoped — full 8-case test is T1.35): **(a)** render smoke — open any v11 fixture post-migrate, no panics, frame produced; **(b)** SDF-anywhere smoke — `ripple_lens` placed at chain position 2 (after Blur, Tint) produces visible wiggle, identical to position 0; **(c)** frame time ≤ 2% over v11 baseline measured on a 4-layer scene.

- [x] **004-T1.9** Per-node bypass + A/B-compare check at the chain loop: `if !node.enabled || st.ab_compare { continue; }`. Skipped passes do NOT flip the ping-pong.
  - Files: `src/app.rs` (the `for node in &cfg.effects` loop)
  - Accept (renderer-only — UI toggle is T1.25 / T1.29): hand-edit a project JSON to set `enabled: false` on one node, load → output matches the chain with that node removed. Toggle `ab_compare` via a dev override → all-bypass output matches raw source.

### 1c. Mutation updates (spec A.5; T1.10 deferred to 1j)

- [x] **★ 004-T1.11** Update `SetLayerEffects` to carry `Vec<EffectNode>` instead of `Vec<Effect>`. `ReverseStorage` impl shape unchanged.
  - Files: `src/project/command.rs:739-795`
  - **Foundation cluster — see T1.3.**
  - Accept: proptest_round_trip passes against `Vec<EffectNode>` arbitrary.

- [x] **★ 004-T1.12** Update `ModulatorField` and `modulator_at_{ref,mut}` to dereference `EffectNode.effect`.
  - Files: `src/project/command.rs:71-146`
  - **Foundation cluster — see T1.3.**
  - Accept: modulator-related mutations apply correctly against an `EffectNode`-wrapped chain (existing tests pass).

- [x] **004-T1.13** Add `SetLayerEffectsAndMask` mutation (renamed from `AddEffectNodeWithMaskFill` per review — symmetric payload). Snapshots both `Vec<EffectNode>` and `Vec<[f32; 2]>` mask_polygon. `ReverseStorage` swaps both.
  - Files: `src/project/command.rs`
  - Accept: unit test — apply, reverse, re-apply → returns to original.

### 1d. Auxiliary call sites (spec A.6 + review amendment)

- [x] **★ 004-T1.14** Update `mutate_transform_effect` (scene_editor.rs:252-280) to push `EffectNode { enabled: true, effect: Effect::Transform {…} }` instead of raw `Effect::Transform`.
  - Files: `src/windows/scene_editor.rs:261`
  - **Foundation cluster — see T1.3.**
  - Accept: scene-editor transform drag still mutates the first Transform; ReverseStorage rule 2 still holds.

- [x] **★ 004-T1.15** Update any other `LayerConfig` constructors that set `effects: vec![Effect::…]` to wrap in `EffectNode`. Grep call sites in `src/project/scene_instantiation.rs`, `scene_templates.rs`, tests.
  - Files: as found
  - **Foundation cluster — see T1.3.**
  - Accept: `cargo check --workspace --all-targets` green.

- [x] **004-T1.16** **Review amendment.** Change `Preset.effects: Vec<Effect>` → `Vec<EffectNode>` (control_panel.rs:236-238). Migrate the three asset JSONs (`assets/presets/architectural_wash.json`, `candle_flicker.json`, `soft_pulse.json`) by wrapping each entry as `{"enabled": true, "effect": {…}}`.
  - Files: `src/windows/control_panel.rs`, `assets/presets/*.json`
  - Accept: load each preset → 3 EffectNodes with `enabled: true`. Export-as-preset → import → bypass state round-trips.

### 1e. Audit updates (spec F.3–F.4)

- [x] **★ 004-T1.17** Rewrite the primary audit iteration at `audit.rs:535-590` to walk `layer.effects` and match on `EffectNode.effect == Effect::Treatment`. Apply the existing `UnknownTreatment` / `MissingAsset` checks per node.
  - Files: `src/project/audit.rs`
  - **Foundation cluster — see T1.3.**
  - Accept: unknown preset id at any chain position triggers `UnknownTreatment`. Missing overlay file at any position triggers `MissingAsset`.

- [x] **004-T1.18** Extend `AuditKind::UnknownTreatment` and the two `MissingAsset` variants with `effect_idx: usize` so the Look-chain row can highlight the specific node.
  - Files: `src/project/audit.rs` (kind definitions + emitters)
  - Accept: kind carries `effect_idx`; UI can read it for chip-row targeting (consumed later in T1.28). (Additive — can land before the cluster.)

- [x] **★ 004-T1.19** Rewrite audit tests at lines 1166, 1194, 1231, 1756, 1782, 1803 to push `EffectNode { enabled: true, effect: Effect::Treatment {…} }` into `effects` instead of setting `layer.treatment = Some(...)`.
  - Files: `src/project/audit.rs` (tests)
  - **Foundation cluster — see T1.3.**
  - Accept: tests pass against the new schema.

### 1f. Capability metadata + no-op detection (spec B)

- [x] **004-T1.20** Add `PresetCapability { requires_sdf, requires_zone, is_particle, headline_param }` and `pub fn capability(preset_id: &str) -> PresetCapability` covering all registered presets.
  - Files: `src/render/treatments.rs` (near `ParamDescriptor` ~line 251)
  - Accept: every preset in `registry()` has a non-default capability entry (test: assert no preset returns the all-false-no-headline default).

- [x] **004-T1.21** Add `pub fn treatment_is_no_op(preset_id, params, layer) -> Option<&'static str>` covering identity-default, empty-mask, missing-zone, missing-overlay cases.
  - Files: `src/render/treatments.rs`
  - Accept: unit tests for the five canonical no-op reasons (mask, zone, amplitude=0, identity tone_map, missing overlay).

- [x] **004-T1.22** Add `pub fn effect_is_no_op(node: &EffectNode) -> Option<&'static str>` for Color/Tint/Blur/Transform/Feedback identity states. Treatment variant delegates to `treatment_is_no_op`.
  - Files: `src/effects/mod.rs`
  - Accept: five unit tests, one per variant.

- [x] **004-T1.23** Add `pub fn intent_group(...) -> IntentGroup` for both Effect variants and treatment preset_ids. Six groups: Warp / Color / Texture / Compose / Animate / Generative. Every preset assigned exactly one group.
  - Files: `src/effects/mod.rs`, `src/render/treatments.rs`
  - Accept: unit test asserts every preset_id and every Effect variant maps to a group.

### 1g. Look chain UI (spec C)

- [x] **004-T1.24** Scaffold `src/windows/look_chain.rs` with `pub fn show_look_chain_section(ui, project, st, layer_idx)` empty stub. Register in `src/windows/mod.rs`.
  - Files: `src/windows/look_chain.rs` (new), `src/windows/mod.rs`
  - Accept: compiles and the section renders an empty placeholder when called.

- [x] **004-T1.25** Implement row rendering: drag handle (⋮⋮), intent-group glyph + color, status dot (green/amber/grey), headline-param slider on-row, expand chevron, delete `×`. Mirror `control_panel.rs:1596-1656` for drag-reorder and `control_panel.rs:1622-1628` for delete.
  - Files: `src/windows/look_chain.rs`
  - Accept: a 3-node chain renders correctly; drag reorders nodes; status dot color reflects `is_no_op`.

- [x] **004-T1.26** Implement expanded params panel. Treatment params reuse `controls.rs:993-1009` loop; non-treatment variants reuse existing slider blocks. Use `modulator_slider` (`control_panel.rs:2695`) for `Modulator`-typed slots only.
  - Files: `src/windows/look_chain.rs`
  - Accept: expanding ripple_lens shows amplitude/wavelength/speed; expanding Blur shows the existing radius_px modulator slider.

- [x] **004-T1.27** Implement `+ Add to chain ▾` picker as `egui::Popup` with intent-grouped sections. Each entry: glyph + name + hover description + capability hints ("Needs a mask polygon"). Click → wrap + append + dispatch `SetLayerEffects`.
  - Files: `src/windows/look_chain.rs`
  - Accept: picker opens, every preset listed under exactly one group, capability hints visible on hover, adding an entry appends to chain.

- [x] **004-T1.28** Implement autofix chips. When `is_no_op` returns `Some(reason)`, render inline `[auto-fix]` button. Dispatch the corresponding Mutation (`SetLayerMaskPolygon` for mask, `SetLayerEffects` for params nudge, file dialog → `SetLayerEffects` for overlay).
  - Files: `src/windows/look_chain.rs`
  - Accept: empty-mask layer with ripple_lens shows chip; click → mask added → status dot goes green; one undo step.

- [x] **004-T1.29** Add `pub ab_compare: bool` to `ControlPanelState` (default `false`). Render header `[ A/B compare ]` toggle. Thread the flag into the render call so the loop in T1.9 reads it. NOT in project snapshot — does not undo, does not crossfade.
  - Files: `src/windows/look_chain.rs`, `src/windows/control_panel.rs` (state struct), `src/app.rs` (render entry)
  - Accept: toggle off → chain output. Toggle on → raw source. Cmd-Z does not touch A/B state.

- [x] **004-T1.30** Splice `look_chain::show_look_chain_section` into `controls.rs:339` (replace `show_treatment_section` call; rename CollapsingHeader from `"Treatment"` to `"Look chain"`, `default_open(true)`). Delete `show_treatment_section` body (controls.rs:857-1013) AND the six call sites of `set_layer_treatment_mutation` at lines 969, 1004, 1051, 1061, 1105, 1116.
  - Files: `src/windows/controls.rs`
  - Accept: Layers tab shows Look chain. Old Treatment section gone. No remaining callers of `set_layer_treatment_mutation` / `set_layer_treatment_params_mutation` (`grep` returns clean). `cargo check` green.

- [x] **004-T1.31** Remove the Effects tab. Delete `ControlTab::Effects` variant, selector at `control_panel.rs:586`, match arm at line 918, `show_effects_tab` (1494+), and inline `add_effect_picker`. Move the Preset apply control (control_panel.rs:1547-1559) into the Look chain header.
  - Files: `src/windows/control_panel.rs`
  - Accept: tab strip shows Scene / Layers / Scenes only. Preset apply still works from the new Look-chain header.

### 1h. Smart-fill on add (spec D)

- [x] **004-T1.32** Wire `SetLayerEffectsAndMask` into the Add-picker handler: when the operator picks an SDF-keyed preset on a layer with empty `warp.mask_polygon`, the same mutation also seeds a full-quad mask. Same shape for `requires_zone && zone_role.is_none()` → default `ZoneRole::Window`.
  - Files: `src/windows/look_chain.rs`, `src/project/command.rs` (use the mutation from T1.13)
  - Accept: add ripple_lens to mask-less video → ripples appear immediately, one undo step reverses both.

### 1i. Mutation graveyard (depends on UI splice complete — T1.30 + T1.31 landed)

- [x] **004-T1.10** Delete `SetLayerTreatment` (905-941), `SetLayerTreatmentParams` (943-987), their enum variants (~2418, ~2423), apply match arms (2603-2606), undoability entries (2808-2809), constructors `set_layer_treatment_mutation` (~2947), `set_layer_treatment_params_mutation` (~2966), and their round-trip tests (~3812-3925) and proptest entries (~5131, 5137, 5577, 5595, 5940, 5942).
  - Files: `src/project/command.rs`
  - **Order**: T1.30 must have already deleted the six `controls.rs` callers; otherwise this fails `cargo check`.
  - Accept: `grep -n 'SetLayerTreatment\|set_layer_treatment' src/` returns clean. `make test` green. `make lint` green.

### 1j. Verification + ship gate

- [x] **004-T1.33** One-shot first-launch toast: "Effects merged into the Layers tab as Look chain." Gated on a session flag at `~/Library/Application Support/rmap/ui_flags.json`.
  - Files: `src/windows/controls.rs` or new `src/windows/onboarding.rs`
  - Accept: toast shows once per machine; subsequent launches silent.

- [x] **004-T1.34** Refresh GPU goldens for the per-layer effect chain. Confirm ripple_lens at chain position 0 produces bit-identical output to v11 primary-slot ripple_lens (the identity-fit fix should make this exact, modulo blit precision).
  - Files: `tests/golden/`, run `UPDATE_GOLDEN=1 cargo nextest run --features gpu-tests`
  - Accept: `make test-gpu` green. Pixel delta < 1 LSB.

- [x] **004-T1.35** Run the 8-case manual verification from spec §Verification (migration smoke, SDF-anywhere, no-mask autofix, per-node bypass, A/B compare, drag-reorder, picker grouping, render perf within 2% of v11).
  - Accept: all 8 pass. Record results in a comment on the final ship commit.

- [x] **004-T1.36a** Root docs update: `README.md` schema-version mention (if any); `CLAUDE.md` — remove the Effects-tab reference (line 586 region in the architecture overview) and document the Look chain as the per-layer treatment+effects surface.
  - Files: `README.md`, `CLAUDE.md`
  - Accept: docs describe v12 state; no remaining `Effects tab` references.

- [x] **004-T1.36b** Sub-CLAUDE.md updates:
  - `src/render/CLAUDE.md` — replace `effects: Vec<Effect>` with `effects: Vec<EffectNode>` in the per-frame render-graph order section; document the six new `RenderCtx` fields and the `identity_fit_uniform` invariant (svg_pipeline owns layer fit; chain effects always receive identity fit).
  - `src/project/CLAUDE.md` — note that the Effects-Vec Reverse rule (rule 2) now snapshots `Vec<EffectNode>`; the `mutate_transform_effect` reference appends `EffectNode { enabled: true, effect: Effect::Transform {…} }`.
  - Files: `src/render/CLAUDE.md`, `src/project/CLAUDE.md`
  - Accept: both files describe v12 state.

- [x] **004-T1.36c** Final ship gate: `make ci` (fmt + lint + nextest + doctests), `make test-gpu`, `make build-show` (release profile).
  - Accept: all three targets green.

---

## Phase 2 — Live preset thumbnails (1–2 weeks) — sketched

- [ ] **004-T2.1** Allocate a per-preset thumbnail-cache texture set: ~36 textures at 256×256 (one per registered preset), owned at session level (NOT the per-layer ping-pong, which holds chain intermediates). Render pass fills each cell once per source change.
- [ ] **004-T2.2** Add the preset-grid render pass: one fragment pass per preset against a downsampled (256×256) source of the currently-selected layer.
- [ ] **004-T2.3** Register each thumbnail as an egui ImageId. Display in `+ Add to chain ▾` picker tiles.
- [ ] **004-T2.4** Frame-budget check: confirm ≤ 2 ms/frame added GPU cost on Apple silicon at 4K. Cache invalidates only on source-texture change, not every frame.

## Phase 3 — Modulator-bound treatment params (1 week) — sketched

- [ ] **004-T3.1** Change `Treatment.params: HashMap<String, f32>` → `HashMap<String, Modulator>`.
- [ ] **004-T3.2** Schema bump v12 → v13, migrator wraps each `f32` in `Modulator::Static(f32)`.
  - Accept: load every project in `tests/fixtures/` post-migrate, verify all modulators deserialize round-trip (guards against `Modulator` enum-variant ambiguity with the `Static` form).
- [ ] **004-T3.3** Update treatment-pipeline param read sites to call `.value(clock)` on each Modulator.
- [ ] **004-T3.4** Unify the headline-param slider in `look_chain.rs` — replace plain `egui::Slider` with `modulator_slider` for treatment params. MIDI-learn and audio-bind work uniformly.

## Phase 4 — Aspirational (multi-week)

- [ ] **004-T4.1** Stage-thumbnail strip at top of Look chain: live preview after each node, tap to solo-up-to-here.
- [ ] **004-T4.2** Vibe presets: curated whole-chain templates from a starter library (dub / acid-rave / cinema / etc.).
- [ ] **004-T4.3** Smart suggestions: "this layer would benefit from edge_sparks" surfaced as an unobtrusive hint based on layer content + chain composition.
- [ ] **004-T4.4** Operator-recorded chain templates (save current chain as a named preset, recall on any layer).

---

## Cross-cutting follow-ups (not blocking ship)

- [ ] **004-T5.1** Topology gating + interpolate snap-cut: update `snapshots_share_layer_topology` (`src/project/mod.rs`) to also compare per-layer `effects.len()` so cue crossfades snap-cut on chain-length differences. Add a unit test that verifies `Project::interpolate` snaps `EffectNode.enabled` at t=0.5 (categorical, same as `BlendMode`), and snap-cuts whole-vec when chain lengths differ.
- [ ] **004-T5.2** Audit autofix toast wiring (`app.rs:6230-6235`) — let toast clicks dispatch the carried Mutation, parallel to the Look-chain chip pattern.
- [ ] **004-T5.3** Track `LayerConfig.added_at_secs` so particle treatments (spotlights et al.) on Image/Video layers animate from layer-add rather than project-start. Schema v13 candidate.
- [ ] **004-T5.4** FX-layer warp default: new FxLayer (and Ndi) layers get `WarpMesh::identity()` instead of `default_placement()`, so generative content fills the canvas by default. Add `WarpMesh::default_for_kind(&LayerKind)` helper; update call sites at `schema.rs:1357, 1391, 1412` and `command.rs:3658`. Existing scenes unaffected (existing layers keep their warp). Same UX principle as the unification — no invisible bounds silently clipping output.
