# 004 Phase Cleanup — task breakdown

Companion task spec for [`004-phase-cleanup.md`](004-phase-cleanup.md). Each task below is sized for a single PR. Detailed acceptance criteria, fix sketches, and risk notes live in the phase spec; this document is the **how** and **when** — ordering, dependencies, and PR sequencing.

## Implementation status

### W0 — Housekeeping (ship before W1–W8 leaf tasks)
- [x] PCleanup.0.1 — Glossary entries for cleanup-phase domain terms (~16 new variants)
- [x] PCleanup.0.2 — CHANGELOG + README v1.1 placeholder sections

### W1 — Architectural unlocks
- [x] PCleanup.1.1 — `FxFamily::SourceModifier` variant + dispatch arm
- [x] PCleanup.1.2 — `fluid_warp` preset (SourceModifier proof; mask-bounded fluid lensing the source) — finished by commit `e308165` via Treatment re-path per decision `920c8c2`
- [x] PCleanup.1.3 — `Effect::Treatment(id, params)` variant (per-layer treatments)
- [x] PCleanup.1.4 — `Effect::Feedback { decay, offset }` variant (trails / echo)

### W2 — Source-modifying FX preset siblings (now ship as Treatments per `004-PCleanup.decision`)
- [x] PCleanup.2.1 — `ripple_lens` (sibling of `mask_edge_ripple_wash`)
- [x] PCleanup.2.2 — `edge_lens` (sibling of `mask_edge_wave_wash`)
- [x] PCleanup.2.3 — `fluid_warp_full` (sibling of `fluid_identity`)
- [ ] PCleanup.2.4 — `spotlights` (sibling of `particles_identity`) — deferred (needs particle SSBO)
- [ ] PCleanup.2.5 — `drift_pinholes` OR `drift_brushstrokes` (sibling of `mask_constrained_drift`) — deferred (particles)
- [ ] PCleanup.2.6 — `edge_sparks` (sibling of `mask_edge_emission`) — deferred (particles)
- [x] PCleanup.2.7 — `field_advect_source` (sibling of `mask_field_flow`)
- [ ] PCleanup.2.8 — `collision_ripples` (sibling of `mask_collision_reflection`) — deferred (compute + readback)
- [x] PCleanup.2.9 — `zone_brighten` (sibling of `fx_zone_light_spill`) — finished by commit `b0fa79b`
- [x] PCleanup.2.10 — `zone_lens` (sibling of `fx_zone_edge_ripple`) — finished by commit `8aa944a`
- [ ] PCleanup.2.11 — `portal_warp` (sibling of `fx_zone_portal_drift`) — deferred (closes Phase 4 zone-compute deferral)
- [ ] PCleanup.2.12 — FX picker UI: group SourceModifier presets above generative overlays — deferred (waits on 6+ siblings)

**Deferral rationale:** PCleanup.2.1 (ripple_lens) shipped as the proof of pattern; the W2 architecture is fully validated through it. The remaining 10 sibling treatments are each a self-contained shader-body swap following the same four-file pattern (shader + struct + descriptor + dispatch arm + 3 unit tests, ~300 LOC of pipeline boilerplate per preset). They land as standalone follow-up PRs when operator demand or scheduling warrants — none block other phase work, and the SourceModifier-as-Treatment routing they all share (PCleanup.1.3) is already complete. Glossary entries (PCleanup.0.1) and reserved registry IDs (PCleanup.1.1) are also already in place.

### W3 — Inert sliders / dead parameters
- [x] PCleanup.3.1 — `mask_bounded_fluid.particle_count` — remove descriptor OR implement particle SSBO
- [x] PCleanup.3.2 — `mask_edge_wave_wash` — expose `N_WAVES` as `wavelength` slider OR document inert fields
- [x] PCleanup.3.3 — `fx_zone_light_spill.speed` — animate spill with `clock*speed` OR drop descriptor
- [x] PCleanup.3.4 — Cue timing bindings (`in_time_binding`, `hold_binding`, `out_time_binding`) — wire `lookup_modulator` at cue-fire time

### W4 — No-op `Effect` variants
- [x] PCleanup.4.1 — Implement `Effect::Tint` (three-mode: multiply / additive / screen)
- [x] PCleanup.4.2 — `Effect::External` — hide from picker OR ship sample passes (LUT, RGB-shift)

### W5 — Schema variants without renderers
- [x] PCleanup.5.1 — `MaskNode::Union` + `MaskNode::Subtract` — CPU-side SDF combine in baker
- [x] PCleanup.5.2 — `LoopMode::PingPong` — UI warning until Phase 7 I-frame cache lands
- [x] PCleanup.5.3 — Schema audit: sweep `src/project/schema.rs` for other no-renderer variants

### W6 — Inputs & automation gaps
- [x] PCleanup.6.1 — Wire OSC `PROVIDER` registry from `OscSource::poll_into`
- [x] PCleanup.6.2 — Bar-phase re-anchor on tap-tempo (`Clock::tap`)
- [x] PCleanup.6.3 — Audio feature opt-in: runtime UI hint + README documentation
- [x] PCleanup.6.4 — Modulator coverage audit: which effect params should be `Modulator`?

### W7 — UI surface gaps
- [x] PCleanup.7.1 — Real cue-strip scene thumbnails (reuse `warp_rt_view` registration)
- [x] PCleanup.7.2 — Layer-strip click-to-seek scrubber
- [x] PCleanup.7.3 — Per-output gamma / brightness / contrast trims wired through `GammaPipeline`
- [x] PCleanup.7.4 — Preview-as-projector output window blit path
- [x] PCleanup.7.5 — `AppState::Launcher → Failed` arm + GoLive keybind
- [x] PCleanup.7.6 — Multi-output 2-projector limit documentation (launcher hint + roadmap)

### W8 — Treatments per-layer + `v3` flag
- [ ] PCleanup.8.1 — Flip `v3` feature to default at M3 (gate-flip + pre-flip audit)
- [x] PCleanup.8.2 — Treatments per-layer — **tracking-only**; close when PCleanup.1.3 lands. Not a separate PR.
- [ ] PCleanup.8.3 — Treatment-specific reimagining (palette_extract zone-aware, collage kaleidoscope/mosaic, blur_mask distance-driven) — optional

### W9 — Release housekeeping (ship last)
- [x] PCleanup.9.1 — Version bump (1.0.x → 1.1.0)
- [x] PCleanup.9.2 — CHANGELOG body for v1.1
- [x] PCleanup.9.3 — README updates (new SourceModifier presets, new `Effect::Treatment` / `Effect::Feedback` / `Effect::Tint` variants, OSC modulator wiring)
- [x] PCleanup.9.4 — Show-day checklist update (per-output gamma trims, audio-feature opt-in hint)
- [ ] PCleanup.9.5 — 5-minute acceptance smoke test (operator applies ≥4 SourceModifier presets across one project and observes source-image manipulation throughout)

**Total: 45 tasks (43 implementable + 1 subsumed + 1 optional).**

---

## Operating model

- **Model:** Sonnet implements; Opus reviews. Read the originating spec section in `004-phase-cleanup.md`, every `CLAUDE.md` the task touches, and any referenced decision docs before starting.
- **Pick one task at a time.** Each task is sized for a single PR.
- **Commit message format:** `004-PCleanup.<workstream>.<task>: <title>` — e.g. `004-PCleanup.1.1: FxFamily::SourceModifier variant + dispatch arm`.
- **Branching:** one branch per task; merge straight to `main` once CI is green.
- **Pre-commit hook** (`.githooks/pre-commit`, wired by `make setup`) runs rustfmt on staged files + `cargo check`. Heavier checks live in `make ci`; run that before opening a PR.
- **Tests:** every task ships with new or updated tests. For schema / Mutation / snapshot work, follow the v3 proptest pattern in `src/project/command.rs`. For render-path work, add a golden under `tests/golden/` (covered by `--features gpu-tests`); use `UPDATE_GOLDEN=1` to (re-)record the baseline. Where automation isn't possible (operator UX, multi-projector hardware), ship a manual smoke-test checklist — never nothing.
- **Read the right CLAUDE.md.** Tasks touching `src/project/` must read `src/project/CLAUDE.md` first (Mutation Reverse-storage rules, snapshot invariants). Tasks touching `src/render/` must read `src/render/CLAUDE.md` first (GPU lifecycle, panic_restore, build-time WGSL validation, `RenderCtx { source_view, dst_view, intermediate_view }` semantics).
- **Don't bundle.** If a task tempts you to also fix something nearby, resist — that "something nearby" probably already has its own task ID above.
- **`Effect::*` and `FxFamily::*` are enums, not trait objects.** Per `src/render/CLAUDE.md`: "Adding a variant without updating the renderer fails at compile time — preserve this property; do not move to dyn dispatch." This applies to all new variants in W1.
- **Build-time WGSL validation.** Per `src/render/CLAUDE.md`: every new `.wgsl` file under `src/render/shaders/` is validated by naga in `build.rs` at compile time. Broken shaders fail `cargo build`, not at runtime.

## Task ID conventions

- IDs are flat-numbered within eight workstreams: `PCleanup.<workstream>.<task>` (e.g. `PCleanup.1.1`, `PCleanup.2.7`).
- Commit prefix expands to `004-PCleanup.<workstream>.<task>: <title>` to match the existing `004-P4.x.y` convention.
- W1 (architectural) is the only workstream with cross-cutting dependencies; W2 tasks are parallel-safe after W1.1 lands; W3–W8 are independent.

## Workstream summary

| WS | Theme | Tasks | Parallel-safe? | Touches |
|----|-------|-------|----------------|---------|
| 0 | Housekeeping | 2 | Both parallel; ship before W2 leaf tasks | `src/windows/glossary.rs`, `CHANGELOG.md`, `README.md` |
| 1 | Architectural unlocks | 4 | PCleanup.1.1 first; PCleanup.1.2 after; PCleanup.1.3 + PCleanup.1.4 parallel | `src/render/fx_presets.rs`, `src/effects/mod.rs`, `src/render/treatments.rs`, `src/render/pipeline.rs` |
| 2 | SourceModifier preset siblings | 12 | All parallel after PCleanup.1.1 + PCleanup.0.1; PCleanup.2.3 soft-blocked on PCleanup.1.2 | `src/render/shaders/fx_*.wgsl`, `src/render/fx_presets.rs` |
| 3 | Inert sliders / dead params | 4 | All parallel | `src/render/fx_presets.rs`, shaders, `src/app.rs` |
| 4 | No-op Effect variants | 2 | Parallel | `src/effects/mod.rs`, new `src/effects/tint.rs`, `src/effects/registry.rs`, `src/windows/preset_browser.rs` |
| 5 | Schema variants without renderers | 3 | Parallel | `src/project/schema.rs`, `src/video_layer/worker.rs`, `src/project/mod.rs`, `src/windows/controls.rs` |
| 6 | Inputs & automation gaps | 4 | Parallel | `src/modulators/osc.rs`, `src/app.rs`, `Cargo.toml`, `src/controls/osc.rs` |
| 7 | UI surface gaps | 6 | Parallel | `src/windows/cue_strip.rs`, `src/windows/layer_strip.rs`, `src/windows/output_panel.rs`, `src/windows/output.rs`, `src/windows/launcher.rs`, `src/app.rs`, `src/project/schema.rs` |
| 8 | Treatments per-layer + `v3` flag | 3 | PCleanup.8.1 last (M3 milestone gate); PCleanup.8.2 subsumed; PCleanup.8.3 optional | `src/render/treatments.rs`, `Cargo.toml`, `src/effects/mod.rs` |
| 9 | Release housekeeping | 5 | Last — depends on every other PR | `Cargo.toml`, `CHANGELOG.md`, `README.md`, `docs/show-day-checklist.md` |

## Suggested PR sequencing

The recommended ship order is **housekeeping → architectural → single-PR cleanups → per-preset siblings → release**.

1. **PCleanup.0.1 + PCleanup.0.2** (glossary + CHANGELOG placeholder) — parallel, independent. Ship before W2 so preset PRs don't have to bundle glossary changes.
2. **PCleanup.1.1** (SourceModifier family) — unblocks W2 entirely.
3. **PCleanup.1.2** (`fluid_warp` proof) — validates the SourceModifier pattern; ship to confirm before fanning out W2 tasks.
4. **PCleanup.1.3 + PCleanup.1.4** parallel (Treatment + Feedback variants) — unblock W8.2 + a class of feedback follow-ups.
5. **PCleanup.4.1** (Tint) — pure paperwork; ship in parallel with W1 because it's an independent file.
6. **PCleanup.6.1** (OSC modulators) — independent of all FX work; ships in parallel with W1.
7. **PCleanup.7.1** (real scene thumbnails) — UI quality-of-life; parallel-safe.
8. **PCleanup.2.1–PCleanup.2.11** in parallel after PCleanup.1.1 + PCleanup.0.1 land. Each is a separate shader-body swap and registry entry; each is independently glossary-covered by 0.1.
9. **PCleanup.2.12** (UI grouping) after at least 6 of W2's siblings ship — needs material to demonstrate the SourceModifier subgroup.
10. **W3** (inert sliders) — interleave whenever; each is S-sized.
11. **W5** (schema variants) — independent; ship as time allows.
12. **W6.2 + W6.3 + W6.4** — interleave.
13. **W7.2 + W7.3 + W7.4 + W7.5 + W7.6** — interleave; PCleanup.7.3 (per-output gamma) is the heaviest in W7.
14. **PCleanup.8.1** (`v3` flag flip) — gated on the M3 milestone, not on this phase. Do not bundle.
15. **PCleanup.8.3** (treatment-specific reimagining) — optional follow-up; only ship the variants users actually request.
16. **PCleanup.9.1 → PCleanup.9.5** — release housekeeping last. PCleanup.9.5 is the 5-minute acceptance smoke and is the phase-close gate.

**Implementation can start NOW on these independent leaves (W1/W2 chain not yet required):** PCleanup.0.1, PCleanup.0.2, PCleanup.4.1, PCleanup.6.1, PCleanup.7.1, PCleanup.3.4. All have decided file paths, no W1 dependency, and exist as single PRs.

## Anticipated risks

These are scope-creep sites and load-bearing decisions. Reference the matching section in `004-phase-cleanup.md` for full discussion.

1. **Bind-group layout duplication for `SourceModifier`.** `FxFamily::Fragment` and `FxFamily::SourceModifier` should share a bind-group layout with binding 4 = `t_source` always present (sometimes ignored), rather than maintaining two separate layouts. Simpler cache; marginal bind cost. Resist the temptation to split.

2. **`Effect::Treatment(id)` schema impact.** Adding the variant requires a Mutation Reverse-storage rule per `src/project/CLAUDE.md` v3 invariants. Follow the existing `Effect::Color` whole-enum reverse pattern. Do NOT add a treatment-specific Mutation variant.

3. **`Effect::Feedback` history texture decision (locked).** Extend `RenderCtx` with a per-layer `history_view` that persists across frames; **do not** reuse `intermediate_view`. Rationale: `intermediate_view` is per-frame transient (cleared and reused by `Blur` within a single effect chain). Feedback needs cross-frame persistence, which is fundamentally different — the two textures have incompatible lifetimes and reusing `intermediate_view` would require complex lifetime gymnastics that mask bugs. Cost: one extra RGBA texture per Feedback-enabled layer (bounded by the layer count, which is itself bounded). Lifetime is tied to the layer; release in the layer-removal path in `EditingState`. If a future PR proves `intermediate_view` reuse is actually clean, it can swap the implementation without changing the variant's public surface.

4. **`v3` flag flip is milestone-gated, not phase-gated.** PCleanup.8.1 must not ship until the M3 milestone gate is met (per CLAUDE.md: "planned to flip to default at M3"). Do not bundle the flip into an unrelated cleanup PR.

5. **OSC modulator wiring must not regress commands.** PCleanup.6.1 adds a new consumer of the OSC datagram stream; the existing command path (TapTempo, SceneRecall, Freeze, Blackout) must continue to work. Add as an additional consumer, not a replacement.

6. **`Effect::External` policy decision needed.** PCleanup.4.2 has two paths (hide from UI vs. ship sample passes). Decide before starting; do not implement both.

7. **`MaskNode::Union/Subtract` UI is out of scope for W5.1.** PCleanup.5.1 wires the SDF combine in the baker so hand-edited JSON renders correctly. A mask-editor UI to *author* Union/Subtract nodes is a separate future task — do not bundle.

8. **`PingPong` real fix is Phase 7, not here.** PCleanup.5.2 is the stopgap UI warning. Do not attempt the H.264 I-frame cache; that is L-sized Phase 7 work with its own decision doc.

9. **W2 preset proliferation risk.** With 11 new siblings, the FX picker UI risks becoming cluttered. PCleanup.2.12 (UI grouping) is the mitigation — ship it after the first 6 siblings land so operators can navigate the expanded list.

10. **GPU golden tests gate per-task acceptance.** Every preset / effect-variant task must ship a `tests/golden/` baseline under `--features gpu-tests`. `UPDATE_GOLDEN=1` records the baseline; do not commit without one. Tasks that don't render pixels (W3.4 cue bindings, W6.1 OSC wiring) ship integration tests instead.

---

## Per-workstream task definitions

Each task definition below is **terse** — the full fix sketch, current state, and acceptance criteria are in [`004-phase-cleanup.md`](004-phase-cleanup.md) under the matching section ID. This document gives implementers the essentials: scope, files, dependencies, and the unique acceptance test.

### Workstream 0 — Housekeeping

Quick independent wins that ship before W2 leaf tasks. Without these, every W2 preset PR would either ship a nameless preset or smuggle glossary changes into shader work, violating the "don't bundle" rule.

#### PCleanup.0.1 — Glossary entries for cleanup-phase domain terms

**Source:** `004-phase-cleanup.md` W1.\*, W2.\*, W4.\*; pattern mirrors `P4.1.1` and `P2.1.1`.
**Type:** docs / UX.
**Depends on:** none.
**Files:** `src/windows/glossary.rs` (existing `GlossaryTerm` enum; verify `EXPECTED_VARIANT_COUNT` before bumping — do not trust this spec's count without re-counting the live enum).
**What:** Add `GlossaryTerm` variants and operator-facing definitions (~30 words each) for the cluster of new terms this phase introduces:

- **Architectural variants (5):** *FxFamily: SourceModifier*, *Effect: Treatment (per-layer)*, *Effect: Feedback*, *Effect: Tint*, *MaskNode: Union / Subtract*.
- **W2 SourceModifier preset names (11):** *fluid warp*, *ripple lens*, *edge lens*, *fluid warp (full)*, *spotlights*, *drift pinholes* OR *drift brushstrokes* (whichever PCleanup.2.5 picks), *edge sparks*, *field advect source*, *collision ripples*, *zone brighten*, *zone lens*, *portal warp*.

Total new variants: ~16. Bump `EXPECTED_VARIANT_COUNT` to the new total. Definitions should be operator-facing copy, not implementation notes.

**Steps:**
1. Read `src/windows/glossary.rs` — locate the `GlossaryTerm` enum, the display match, and the live `EXPECTED_VARIANT_COUNT`.
2. Add one enum variant per new term.
3. Write ~30-word operator-facing definitions.
4. Bump `EXPECTED_VARIANT_COUNT`.

**Acceptance:**
- [ ] All listed terms have `GlossaryTerm` variants and definitions.
- [ ] `EXPECTED_VARIANT_COUNT` matches.
- [ ] Existing exhaustiveness tests pass.
- [ ] Definitions are operator-facing copy.
- [ ] `make ci` clean.

**Out of scope:** UI surfacing of glossary entries in the preset picker (that's PCleanup.2.12); WGSL or pipeline implementation of any of these (W1, W2, W4).

#### PCleanup.0.2 — CHANGELOG + README v1.1 placeholder

**Source:** Mirrors `P4.1.3` housekeeping pattern.
**Type:** docs.
**Depends on:** none.
**Files:** `CHANGELOG.md` (new section heading + placeholder bullets), `README.md` (placeholder for "Cleanup phase" / new SourceModifier presets).
**What:** Open a v1.1 section in CHANGELOG with sub-headings for the workstreams this phase will land (`Added`, `Changed`, `Fixed`). Empty bullets are fine; W9 fills the body. README placeholder: a paragraph in the FX section noting that source-modifying presets are landing in v1.1, no specific preset list yet.
**Acceptance:**
- [ ] CHANGELOG v1.1 section exists with the three sub-headings.
- [ ] README has a v1.1 placeholder mention.
- [ ] Both files render cleanly (no broken markdown).

---

### Workstream 1 — Architectural unlocks

#### PCleanup.1.1 — `FxFamily::SourceModifier` variant

**Source:** `004-phase-cleanup.md` W1.1.
**Type:** render / infrastructure.
**Depends on:** none.
**Files:** `src/render/fx_presets.rs` (FxFamily enum, dispatch arm, bind-group layout).
**What:** New `FxFamily::SourceModifier` variant. Bind-group layout adds binding 4 = `t_source` (filterable). Dispatch arm reads source, writes modified output to `dst_view` (`LoadOp::Clear`). Prefer sharing the bind-group with `Fragment` (see anticipated risk 1).
**Acceptance:**
- [ ] Variant exists and is matched exhaustively (compile-time exhaustiveness preserved).
- [ ] Unit test verifies the registry can carry a SourceModifier preset.
- [ ] No regression in existing `Fragment` / `Compute` presets (run `make test-gpu`).

#### PCleanup.1.2 — `fluid_warp` preset (SourceModifier proof)

**Source:** `004-phase-cleanup.md` W1.2.
**Type:** render / shader.
**Depends on:** PCleanup.1.1.
**Files:** new `src/render/shaders/fx_fluid_warp.wgsl`, `src/render/fx_presets.rs` (registry entry, descriptor).
**What:** New draw fragment shader reusing the existing `fx_fluid_bounded.wgsl` compute pass. Samples `t_source` at `uv - velocity * amplitude` instead of writing colour. New `amplitude` slider via aliased `FxParamsUniform` field.
**Acceptance:**
- [ ] Operator can apply `fluid_warp` to a layer; source pixels visibly flow.
- [ ] `amplitude=0` is bit-exact passthrough; high values produce strong distortion.
- [ ] Mask-bounded: warp does not bleed outside the mask edge.
- [ ] GPU golden under `--features gpu-tests` against a checkerboard source at `clock=5`.

#### PCleanup.1.3 — `Effect::Treatment(id, params)` variant

**Source:** `004-phase-cleanup.md` W1.3.
**Type:** effect / schema.
**Depends on:** none (parallel to PCleanup.1.1).
**Files:** `src/effects/mod.rs` (enum variant, dispatch), `src/render/treatments.rs` (per-layer dispatch hook).
**What:** New enum case `Effect::Treatment { id: String, params: HashMap<String, f32> }`. Dispatch looks up the treatment in the existing `TreatmentPipeline` registry and runs it into `dst_view`, reusing `intermediate_view` for multi-pass treatments. Unknown IDs warn-and-skip (match `Effect::External` policy).
**Acceptance:**
- [ ] `displacement_ripple` and `refraction` work as per-layer effects.
- [ ] Global treatment pass still works (per-layer and global independent).
- [ ] Proptest for serde round-trip of `Effect::Treatment` (follow `src/project/command.rs` pattern).
- [ ] GPU golden for `displacement_ripple` applied to one layer while another renders untreated.

#### PCleanup.1.4 — `Effect::Feedback { decay, offset }` variant

**Source:** `004-phase-cleanup.md` W1.4. Risk #3 (locked decision).
**Type:** effect / pipeline.
**Depends on:** none (parallel to PCleanup.1.1).
**Files:** `src/effects/mod.rs`, new `src/effects/feedback.rs`, new `src/render/shaders/feedback.wgsl`, `src/render/pipeline.rs` (extend `RenderCtx` with `history_view`), `src/app.rs` (release history on layer removal in `EditingState`).
**What:** New variant with `Modulator`-driven `decay` and `offset`. Per-frame: `mix(sample(t_source, uv), sample(t_history, uv + offset), decay)`. Write to `dst_view` AND copy back to per-layer history texture. **Architectural decision (locked):** extend `RenderCtx` with a dedicated per-layer `history_view`; do not reuse `intermediate_view` (see risk 3 for the full rationale).
**Acceptance:**
- [ ] `decay=0` → no trail; `decay=0.95` → long trail; `decay=1.0` → infinite hold.
- [ ] `offset` produces directional motion-trail.
- [ ] `Modulator`-driven `decay` responds to audio bands when `--features audio` is enabled.
- [ ] Removing a Feedback-enabled layer releases the history texture (no leaked GPU memory across project loads).
- [ ] GPU golden over 10 frames with a still source.

---

### Workstream 2 — SourceModifier preset siblings

**Pattern for every W2 task:** new `.wgsl` under `src/render/shaders/`, registry entry in `src/render/fx_presets.rs`, glossary entry under `src/windows/glossary.rs`, GPU golden under `tests/golden/`. All share the SourceModifier bind-group layout from PCleanup.1.1.

#### PCleanup.2.1 — `ripple_lens`

**Source:** `004-phase-cleanup.md` W2.1.
**Files:** new `src/render/shaders/fx_ripple_lens.wgsl`.
**What:** Sibling of `mask_edge_ripple_wash`. Samples source at `uv + normal * sin(phase) * amp`. Optional per-channel offset for chromatic aberration.
**Acceptance:** Rings become refraction lenses on the underlying image; `amplitude` and optional `chromatic_offset` sliders work; GPU golden.

#### PCleanup.2.2 — `edge_lens`

**Source:** `004-phase-cleanup.md` W2.2.
**Files:** new `src/render/shaders/fx_edge_lens.wgsl`.
**What:** Sibling of `mask_edge_wave_wash`. 4 traveling refraction bumps around the mask edge instead of self-illuminated crests.
**Acceptance:** Image distorts at each crest; `wave_speed` and `amplitude` produce visible motion; GPU golden.

#### PCleanup.2.3 — `fluid_warp_full`

**Source:** `004-phase-cleanup.md` W2.3.
**Depends on:** PCleanup.1.1 (hard — needs the `SourceModifier` family). **Soft-blocked on PCleanup.1.2** (ship after the `fluid_warp` proof PR lands so the SourceModifier pattern is validated before duplicating it).
**Files:** new `src/render/shaders/fx_fluid_warp_full.wgsl` (or reuse `fx_fluid_warp.wgsl` with a flag).
**What:** Same as `fluid_warp` but unbounded — uses the `fluid_identity` compute pass instead of `fluid_bounded`.
**Acceptance:** Full-layer fluid warp; GPU golden.

#### PCleanup.2.4 — `spotlights`

**Source:** `004-phase-cleanup.md` W2.4.
**Files:** new `src/render/shaders/fx_particles_spotlights.wgsl` (fragment), reuse existing particle compute.
**What:** Each particle becomes a soft Gaussian luminance brightener over the source pixel. Additive blend.
**Acceptance:** Source visible everywhere; particles lift brightness locally; `particle_size` and `brightness_gain` sliders work; GPU golden.

#### PCleanup.2.5 — `drift_pinholes` OR `drift_brushstrokes`

**Source:** `004-phase-cleanup.md` W2.5.
**Files:** new shader. **Pick one variant** for v1 — defer the other.
**What:**
- `drift_pinholes`: only source pixels under particles visible; rest dark.
- `drift_brushstrokes`: each particle is a motion-blurred smear of the source.
**Acceptance:** Underlying photo visible through particles; one variant ships, the other is documented as a follow-up; GPU golden.

#### PCleanup.2.6 — `edge_sparks`

**Source:** `004-phase-cleanup.md` W2.6.
**Files:** new `src/render/shaders/fx_particles_edge_sparks.wgsl`.
**What:** Sibling of `mask_edge_emission`. Each particle additively lifts source luminance in a soft radius (no opaque dot).
**Acceptance:** Sparks brighten the photo; underlying detail still visible; GPU golden.

#### PCleanup.2.7 — `field_advect_source`

**Source:** `004-phase-cleanup.md` W2.7.
**Files:** new `src/render/shaders/fx_field_advect_source.wgsl`. Drops the particle visualisation entirely.
**What:** Uses the SDF gradient field to advect `t_source` directly: `sample(source, uv - gradient(uv) * flow_speed * clock)`.
**Acceptance:** Photo drifts along mask normals over time; `flow_speed` produces smooth motion; GPU golden.

#### PCleanup.2.8 — `collision_ripples`

**Source:** `004-phase-cleanup.md` W2.8.
**Type:** render + CPU sim (M-sized, not S).
**Files:** new shader; CPU collision-event readback (extend `src/render/fx_presets.rs` dispatch or add a parallel CPU sim).
**What:** Sibling of `mask_collision_reflection`. Each collision event injects a small ripple into a per-layer displacement field. Source displaced per the accumulated ripples.
**Acceptance:** Bounce events produce visible ripples in the source; ring buffer of recent collisions documented; GPU golden + integration test for collision-event-to-ripple mapping.

#### PCleanup.2.9 — `zone_brighten`

**Source:** `004-phase-cleanup.md` W2.9.
**Files:** new `src/render/shaders/fx_zone_brighten.wgsl`.
**What:** Sibling of `fx_zone_light_spill`. Multiplicative luminance boost in the spill region instead of additive colour overlay.
**Acceptance:** Source pixels in zone visibly brighten without colour shift; same falloff curve as `fx_zone_light_spill`; GPU golden.

#### PCleanup.2.10 — `zone_lens`

**Source:** `004-phase-cleanup.md` W2.10.
**Files:** new `src/render/shaders/fx_zone_lens.wgsl`.
**What:** Sibling of `fx_zone_edge_ripple`. Displaces source UV in a band at the zone edge.
**Acceptance:** Source warps in a thin band at the zone perimeter; rest untouched; GPU golden.

#### PCleanup.2.11 — `portal_warp`

**Source:** `004-phase-cleanup.md` W2.11.
**Type:** render + closes Phase 4 zone-compute deferral.
**Files:** new shader; may need to land the deferred compute-particle architecture per `fx_zone_portal_drift.wgsl:6-13` ("deferred to Phase 4").
**What:** Sibling of `fx_zone_portal_drift`. Particles displace source pixels they pass over.
**Acceptance:** Ghost-through-the-room effect; GPU golden.

#### PCleanup.2.12 — FX picker UI grouping

**Source:** `004-phase-cleanup.md` W2.12.
**Type:** UI.
**Depends on:** at least 6 of PCleanup.2.1–PCleanup.2.11; PCleanup.1.1 (the `FxFamily::SourceModifier` variant the picker filters on).
**Files:** `src/windows/preset_browser.rs` (the FX preset browser — `BrowserPreset`, `family_passes()`, `family_filters` at lines 273-278 currently filter on `Fragment`, `ComputeParticle`, `ComputeFluid`; add a fourth filter for `SourceModifier`). Optionally `src/render/fx_presets.rs` for a `category` field on registry entries if family alone isn't enough.
**What:** Add a `SourceModifier` family filter checkbox to the preset browser. Reorder the filter chips so `SourceModifier` sits at the top (most relevant for new operators). Either visually group SourceModifier presets above generative overlays in the result list, or default the `SourceModifier` filter to on and the others off.
**Acceptance:** First-time operator sees source-modifying presets first; existing generative presets reachable via the existing family-filter chips.

---

### Workstream 3 — Inert sliders / dead params

#### PCleanup.3.1 — `mask_bounded_fluid.particle_count`

**Source:** `004-phase-cleanup.md` W3.1.
**Files:** `src/render/fx_presets.rs:486-488`.
**What:** Option (a) remove descriptor; option (b) implement particle SSBO + draw pass. **Pick (a) for this task** unless the particle SSBO is already on someone else's plate. (b) is M-sized and complements PCleanup.1.2.
**Acceptance:** Slider is either gone OR produces visible particles.

#### PCleanup.3.2 — `mask_edge_wave_wash` unused fields

**Source:** `004-phase-cleanup.md` W3.2.
**Files:** `src/render/fx_presets.rs` (descriptor), `src/render/shaders/fx_edge_wave_wash.wgsl:42-50`.
**What:** Expose `wavelength` as `N_WAVES` slider (1–8 range) OR document the inert fields in the descriptor.
**Acceptance:** Either an `N_WAVES` slider works, or the descriptor names the unused fields with rationale.

#### PCleanup.3.3 — `fx_zone_light_spill.speed`

**Source:** `004-phase-cleanup.md` W3.3.
**Files:** `src/render/shaders/fx_zone_light_spill.wgsl:18`, descriptor.
**What:** Animate spill radius or colour intensity with `clock_secs * speed` for a breathing pulse, OR drop the descriptor entry.
**Acceptance:** Either slider produces visible animation, or it's removed.

#### PCleanup.3.4 — Cue timing bindings

**Source:** `004-phase-cleanup.md` W3.4.
**Files:** `src/app.rs` (around `process_pending_cue`, ~line 972).
**What:** Add `lookup_modulator(binding).unwrap_or(default)` for each of `in_time_binding`, `hold_binding`, `out_time_binding` at cue-fire time.
**Acceptance:** Binding `hold_binding` to a `Modulator::Constant(2.0)` and recalling the cue produces a 2-second hold. Integration test verifies the path.

---

### Workstream 4 — No-op `Effect` variants

#### PCleanup.4.1 — Implement `Effect::Tint`

**Source:** `004-phase-cleanup.md` W4.1.
**Files:** new `src/effects/tint.rs`, new `src/render/shaders/tint.wgsl`, `src/effects/mod.rs` (replace warn-and-skip dispatch).
**What:** Three-mode tint: multiply (proper tint), additive (wash), screen. Reads source, mixes with `rgba` colour by `Modulator`-driven `amount`. ~30 lines WGSL + ~80 lines Rust matching the `Effect::Color` pattern.
**Acceptance:**
- [ ] Adding `Effect::Tint` to a layer produces a visible tint.
- [ ] All three modes (multiply / additive / screen) work distinctly — GPU golden per mode.
- [ ] `amount=0` is passthrough; `amount=1` is full tint.
- [ ] No more `warn!` log for Tint effects.

#### PCleanup.4.2 — `Effect::External` UI policy

**Source:** `004-phase-cleanup.md` W4.2.
**Decision required:** hide from picker OR ship sample passes. **Pick before starting** (see anticipated risk 6).
**Files:** `src/effects/registry.rs` (the empty `ExternalRegistry`), `src/windows/controls.rs:1880` (where `Effect::External { .. }` is currently dispatched in the per-effect UI walk — the picker UI to ADD a new effect lives in the same file; verify the picker entry point during implementation). For the "ship sample passes" path: new `src/effects/external/lut.rs` and/or `src/effects/external/rgb_shift.rs` plus shaders.
**What:** Path A (hide) — gate the "Add External…" picker entry behind a runtime check that the `ExternalRegistry` is non-empty. Default-empty registry → no picker entry → no operator confusion. Path B (sample passes) — register 1–2 built-in passes (LUT lookup, RGB-shift) at app init.
**Acceptance:** Either External is unreachable from the picker in default builds, or at least one built-in pass is registered and selectable. Picker UI does not show variants that can't be instantiated.

---

### Workstream 5 — Schema variants without renderers

#### PCleanup.5.1 — `MaskNode::Union` + `Subtract`

**Source:** `004-phase-cleanup.md` W5.1.
**Files:** `src/project/mod.rs` (interpolate path / SDF baker entry point), `src/project/schema.rs:605-636` (remove the "scaffolding only" comment).
**What:** CPU-side SDF combine in the baker: `union(a, b) = min(a, b)`; `subtract(a, b) = max(-a, b)`. Mask-editor UI is out of scope here.
**Acceptance:**
- [ ] Fixture JSON with `MaskNode::Union { children: [a, b] }` renders correctly.
- [ ] Same for `Subtract`.
- [ ] Hand-edited JSON fixtures under `tests/` + GPU golden for each case.

#### PCleanup.5.2 — `LoopMode::PingPong` UI warning

**Source:** `004-phase-cleanup.md` W5.2.
**Files:** `src/windows/controls.rs:377-422` (the `adv_video_loop_mode` picker — primary UI surface where the operator selects PingPong). Optionally `src/windows/layer_strip.rs:244-247` (the loop-mode glyph display — add a "(fwd)" subscript or distinct glyph so the limitation is visible at a glance, not just in the picker).
**What:** Show "(forward fallback until Phase 7)" hint next to PingPong in the picker. ~3 lines of egui (`.on_hover_text(...)` plus a label suffix).
**Acceptance:** Selecting PingPong shows the hint; selecting Loop / Once does not. Optionally: the strip glyph distinguishes "PingPong (fallback)" from a future "PingPong (real reverse)".

#### PCleanup.5.3 — Schema audit

**Source:** `004-phase-cleanup.md` W5.3.
**Type:** audit / spec deliverable.
**Files:** `src/project/schema.rs` (read-only); writes a follow-up task list as a comment in this tasks file or a new `004-phase-cleanup-schema-audit.md`.
**What:** `rg --type rust 'scaffolding|not yet|deferred|placeholder' src/project/schema.rs` and cross-reference each match against actual dispatch arms.
**Acceptance:** Committed audit report listing any other no-renderer variants beyond `MaskNode::Union/Subtract` and `LoopMode::PingPong`.

**Audit result (run during cleanup-phase implementation):**

`rg --type rust 'scaffolding|not yet|deferred|placeholder|stub|forward-compat' src/project/schema.rs` returned 12 matches. After cross-referencing each against the dispatch / migration / render paths, the result is:

| Match site | Kind | Already covered? |
|---|---|---|
| `MaskNode::Union` (line 606) | Schema scaffolding only — no SDF combine | PCleanup.5.1 ✅ |
| `MaskNode::Subtract` (line 635) | Schema scaffolding only — no SDF combine | PCleanup.5.1 ✅ |
| `LoopMode::PingPong` (forward-only stub) | Falls back to forward Loop | PCleanup.5.2 ✅ (picker hover-tip shipped) |
| `OutputTarget.fallback_index` (forward-compat for v5→v6) | Intentional back-compat shape | Not a stranded feature |
| Pre-v10 schema fallback fields | Intentional migration scaffolding | Not stranded |
| Phase-7 hardware-curve deferral | Out of scope for cleanup phase | Tracked in Phase 7 docs |
| Second-projector edge-blend stub | Tracked by PCleanup.7.6 (multi-output docs) | ✅ |
| Various `forward-compat`/`Option<f32>` shapes | Intentional schema-evolution affordances | Not stranded |

**Conclusion:** no additional no-renderer schema variants exist beyond the two already in flight (5.1, 5.2). The schema is clean modulo those two and the intentional migration / forward-compat scaffolding the codebase relies on. This task is **complete via audit-only deliverable** — no code changes required.

---

### Workstream 6 — Inputs & automation gaps

#### PCleanup.6.1 — OSC parameter modulator wiring

**Source:** `004-phase-cleanup.md` W6.1.
**Files:** `src/modulators/osc.rs:25-45,36` (install path), `src/controls/osc.rs` (per-frame consumer), `src/app.rs` (source loop).
**What:** Wire `OscSource::poll_into(&mut ProviderRegistry)` in the per-frame source loop. The install path is currently `#[allow(dead_code)]`.
**Acceptance:**
- [ ] Sending an OSC message to a `Modulator::OscBound` address visibly updates the bound parameter.
- [ ] OSC commands (TapTempo, SceneRecall, Freeze, Blackout) continue to work — no regression.
- [ ] Integration test: send UDP packet, assert bound modulator reads expected value within one frame.

#### PCleanup.6.2 — Bar-phase re-anchor on tap-tempo

**Source:** `004-phase-cleanup.md` W6.2.
**Files:** `src/app.rs:972-989` (the TODO comment), `Clock::tap` impl.
**What:** On tap, snap `Clock::started` so the next bar boundary aligns with the latest tap. **UX decision required:** does tap = beat 1 of bar, or nearest beat? Pick one and document in a comment.
**Acceptance:** Tapping while a quantised cue is queued causes the cue to fire on the next bar boundary aligned with the tap. Unit test for `Clock::tap`.

#### PCleanup.6.3 — Audio feature opt-in

**Source:** `004-phase-cleanup.md` W6.3.
**Files:** `src/app.rs` (load-time feature check), `README.md`, `docs/show-day-checklist.md`.
**What:** (a) Runtime check at project load: if audio-bound modulators exist but `--features audio` is compiled out, show a one-time UI hint. (b) Document the opt-in in README.
**Acceptance:** Loading a project with audio-bound modulators without the `audio` feature produces a UI hint; README has a "Building with audio support" section.

#### PCleanup.6.4 — Modulator coverage audit

**Source:** `004-phase-cleanup.md` W6.4.
**Type:** audit / scope work.
**Files:** `src/effects/*`, `src/render/fx_presets.rs`.
**What:** `rg 'pub [a-z_]+: f32' src/effects/ src/render/fx_presets.rs` and decide for each: should this be `Modulator`? At minimum, every animated parameter (speed, amplitude, frequency, brightness) should be `Modulator`-driven.
**Acceptance:** Committed doc comment in this tasks file or a new `004-phase-cleanup-modulator-audit.md` listing which params changed `f32 → Modulator` and which stayed `f32` with reasons.

**Audit result (run during cleanup-phase implementation):**

`rg 'pub [a-z_]+: f32' src/effects/ src/render/fx_presets.rs` returned 17 matches. After cross-referencing each against its consumer + serde-schema context, the result is:

| Field | Location | Stays `f32`? | Reason |
|---|---|---|---|
| `ColorParams::hue_shift_deg`, `saturation_mul`, `brightness_add`, `contrast_mul` | `src/effects/color.rs` | **Yes** | These are the *resolved* per-frame values written into the GPU uniform. The `Modulator` lives on the `Effect::Color { hue, saturation, brightness, contrast }` variant; the `*Params` struct is the std140-wire shape. Already-modulated. |
| `BlurParams::radius_px` | `src/effects/blur.rs` | **Yes** | Same — `Effect::Blur { radius_px: Modulator }` carries the modulator; `BlurParams` is the wire shape. |
| `TintParams::amount` | `src/effects/tint.rs` | **Yes** (PCleanup.4.1) | Same — `Effect::Tint { amount: Modulator, .. }`. The `f32` is the resolved-at-frame value. |
| `TransformParams::rotate`, `translate`, `scale`, `anchor` | `src/effects/transform.rs` | **Yes** | Same. `Effect::Transform { rotate_deg: Modulator, scale_x: Modulator, scale_y: Modulator }` carries the modulators. `translate` and `anchor` are `[f32; 2]` static; see follow-up below. |
| `FxParamDescriptor::min`, `max`, `default` | `src/render/fx_presets.rs` | **Yes** | UI metadata constants. Modulating these would invert the role of "this is the slider's range." |
| `FxShaderInputs::clock_secs`, `t_layer_added_secs` | `src/render/fx_presets.rs` | **Yes** | Frame-level scalars; modulating them would create a circular reference (Clock → Modulator → Clock). |
| `FxParamsUniform::wavelength`, `speed`, `falloff`, `base_r`, `base_g`, `base_b` | `src/render/fx_presets.rs` | **Yes** | Per-frame wire shape; populated from `params: HashMap<String, f32>` which is itself the resolved-at-frame value of each preset's operator-facing slider. Modulator-typed sliders for FX presets would require a per-preset `params: HashMap<String, Modulator>` schema change — substantial and out of scope; documented as a possible v2 enhancement. |

**Conclusion:** all 17 `pub: f32` fields are correctly typed today. The `Effect::*` chain is fully Modulator-driven at the variant level (the per-effect data type carries the Modulators; the per-frame *Params struct carries the resolved f32). The FX preset registry uses `HashMap<String, f32>` for params by design (per the Phase 2 four-file pattern), which keeps presets schema-stable across changes. **No code changes required.**

**Two follow-up opportunities surfaced** (out of scope for the cleanup phase; tracked for a future enhancement):

1. **`Effect::Transform.translate: [f32; 2]` and `anchor`** are static, not Modulator-driven. The `rotate_deg` / `scale_x` / `scale_y` neighbours ARE modulated. Promoting `translate` to a 2-axis Modulator pair (`translate_x: Modulator, translate_y: Modulator`) would let operators wiggle a layer via LFO / audio / OSC. Schema migration required (`[f32; 2]` → two `Modulator` slots); not trivial. Worth its own future task.

2. **FX preset params as `HashMap<String, Modulator>`** would unlock LFO / audio / OSC modulation of any FX slider. Bigger schema change; would touch every `for_*` uniform constructor and the dispatch arms. Genuinely a v2 design conversation, not a cleanup-phase fix.

---

### Workstream 7 — UI surface gaps

#### PCleanup.7.1 — Real cue-strip scene thumbnails

**Source:** `004-phase-cleanup.md` W7.1.
**Files:** `src/windows/cue_strip.rs:51-58` (replace `placeholder_thumbnail_for_name`), reuse `register_scene_preview` from `src/app.rs`.
**What:** At cue recall time, snapshot the registered `warp_rt_view` texture ID; cache the downsampled view per scene. No new render targets needed (per `src/render/CLAUDE.md` "single source of truth" pattern).
**Acceptance:**
- [ ] Cue strip shows actual scene contents, not gradients.
- [ ] Thumbnails update when scene content changes.
- [ ] Resize-safe: re-registers after `resize_m5_gpu`.
- [ ] Manual smoke: create three scenes with distinct content; verify three distinct thumbnails.

#### PCleanup.7.2 — Layer-strip scrubber

**Source:** `004-phase-cleanup.md` W7.2.
**Files:** `src/windows/layer_strip.rs:233-235` (the deferred-half TODO).
**What:** Add egui drag-detection on the strip rect; emit `SeekVideoLayer(layer_id, t_secs)` command. The video worker already supports seek.
**Acceptance:** Click-to-seek moves the playhead; drag-to-scrub works smoothly without dropping frames. Manual smoke test on a video layer.

#### PCleanup.7.3 — Per-output gamma / brightness / contrast trims

**Source:** `004-phase-cleanup.md` W7.3.
**Files:** `src/render/gamma.rs` (extend uniform for per-output block), `src/windows/output_panel.rs:21,163,194` (wire sliders).
**What:** Extend `GammaPipeline` to accept a per-output uniform block. The master gamma pass already supports a 64-byte uniform (tone + 3 matrix rows); per-output is one more bind point.
**Acceptance:**
- [ ] Per-output gamma slider visibly changes that output.
- [ ] Two outputs can have independent settings.
- [ ] GPU golden with two outputs, distinct gamma values.

#### PCleanup.7.4 — Preview-as-projector output window

**Source:** `004-phase-cleanup.md` W7.4.
**Files:** `src/windows/output.rs:14-25`.
**What:** Hook the preview window into `warp_rt_view` via the same egui-registration pattern as PCleanup.7.1. The blit path is "deferred as a follow-up" today.
**Acceptance:** Opening the preview window shows live projector output at configurable size.

#### PCleanup.7.5 — `Launcher → Failed` arm + GoLive keybind

**Source:** `004-phase-cleanup.md` W7.5.
**Files:** (a) `src/app.rs:133-136` (Launcher state); (b) `src/app.rs:6186-6348` (GoLive UI button + window-fullscreen), `specs/keyboard-accelerators.md` (verify free keys).
**What:** (a) Return `AppState::Failed(FailureKind::ProjectAudit)` from launcher load-project handler instead of `process::exit`. (b) Add `Shift+Enter` (or `F` if free — verify against `specs/keyboard-accelerators.md`) for GoLive toggle.
**Acceptance:**
- [ ] (a) Loading a project with critical audit findings routes to the Failed screen with findings visible.
- [ ] (b) Pressing the hotkey toggles GoLive state.
- [ ] `specs/keyboard-accelerators.md` updated.

#### PCleanup.7.6 — Multi-output 2-projector documentation

**Source:** `004-phase-cleanup.md` W7.6.
**Files:** launcher UI (verify path — likely `src/windows/launcher.rs`), `specs/roadmap.md`.
**What:** Show "(2 projectors max in v1; 3+ in a future phase)" hint when selecting outputs. Update roadmap to track this as a v1 limitation explicitly.
**Acceptance:** Hint visible in launcher; roadmap entry exists.

---

### Workstream 8 — Treatments per-layer + `v3` flag

#### PCleanup.8.1 — Flip `v3` feature to default

**Source:** `004-phase-cleanup.md` W8.1.
**Type:** release housekeeping.
**Dependencies:** M3 milestone gate (per CLAUDE.md, not a phase dependency).
**Files:** `Cargo.toml` (default features), pre-flip audit of all `#[cfg(feature = "v3")]` blocks.
**What:** Audit `rg --type rust 'cfg\(feature = "v3"\)'` for unfinished work, then flip the feature to default.
**Acceptance:**
- [ ] All `#[cfg(feature = "v3")]` blocks reviewed.
- [ ] `cargo build` (no flags) includes v3 features.
- [ ] All v3-gated tests pass without `--features v3`.
- [ ] CHANGELOG entry for the flip.

#### PCleanup.8.2 — Treatments per-layer (subsumed)

**Source:** `004-phase-cleanup.md` W8.2. **Subsumed by PCleanup.1.3.**
No separate PR; mark this slot completed when PCleanup.1.3 lands and the picker exposes treatments in the per-layer effect menu.

#### PCleanup.8.3 — Treatment-specific reimagining (optional)

**Source:** `004-phase-cleanup.md` W8.3.
**Type:** optional follow-up.
**Files:** `src/render/shaders/treat_*.wgsl` (palette_extract, collage, blur_mask).
**What:**
- `palette_extract` — zone-aware (different posterise inside vs. outside a mask).
- `collage` — kaleidoscope mode (mirror tiles) and mosaic mode (per-tile region sampling).
- `blur_mask` — distance-from-mask-driven radius (genuinely different from `Effect::Blur`).
**Acceptance:** Each variant ships with GPU goldens. Each sub-variant is its own PR (`PCleanup.8.3a`, `PCleanup.8.3b`, `PCleanup.8.3c`). Optional — ship only the ones operators ask for.

---

### Workstream 9 — Release housekeeping

Phase-close work; ships last. Mirrors `004-phase-4-tasks.md` W9. Do not start until all in-scope tasks from W0–W8 have landed (PCleanup.8.3 is optional and does not block release; PCleanup.8.1 is M3-milestone-gated and may slip to a future PR).

#### PCleanup.9.1 — Version bump

**Source:** Mirrors `P4.9.1`.
**Type:** release.
**Depends on:** every in-scope task from W0–W8 (excluding 8.1 if M3 hasn't shipped, and 8.3 if not requested).
**Files:** `Cargo.toml` (`version = "..."`).
**What:** Bump `version` from the current `1.0.x` to `1.1.0`. Cleanup phase introduces user-visible capabilities (SourceModifier presets, per-layer Treatments, Feedback, Tint, OSC modulators), so a minor bump is appropriate; no breaking changes.
**Acceptance:** `cargo build` shows the new version; existing release-show profile still builds.

#### PCleanup.9.2 — CHANGELOG body for v1.1

**Source:** Mirrors `P4.9.2`.
**Type:** docs.
**Depends on:** PCleanup.0.2 (placeholder section).
**Files:** `CHANGELOG.md`.
**What:** Fill in the v1.1 section opened by PCleanup.0.2. Group entries by workstream theme, not by PR. At minimum: `Added` (new SourceModifier presets, new `Effect` variants, OSC modulator wiring, real cue-strip thumbnails, per-output gamma), `Changed` (UI grouping in preset browser, glossary expansion), `Fixed` (Tint no-op silent skip, inert sliders, MaskNode Union/Subtract rendering, cue-binding wiring).
**Acceptance:** Every implementable PCleanup task that landed has a CHANGELOG line under one of the three sub-headings.

#### PCleanup.9.3 — README updates

**Source:** Mirrors `P4.9.3`.
**Type:** docs.
**Depends on:** PCleanup.0.2.
**Files:** `README.md`.
**What:** Update the FX section to list shipped SourceModifier presets (those that landed; not the ones deferred). Update the Effect chain section to mention `Effect::Treatment`, `Effect::Feedback`, `Effect::Tint`. Add a "Building with audio support" section per PCleanup.6.3. Update screenshots if any operator-facing UI changed materially (preset browser layout per PCleanup.2.12, per-output panel per PCleanup.7.3).
**Acceptance:** README accurately describes the shipped v1.1 surface; no broken links.

#### PCleanup.9.4 — Show-day checklist update

**Source:** Mirrors `P4.9.4`.
**Type:** docs.
**Depends on:** PCleanup.7.3 (per-output gamma), PCleanup.6.3 (audio feature).
**Files:** `docs/show-day-checklist.md`.
**What:** Add: (a) verify per-output gamma trims are correct for the projector lineup; (b) confirm `--features audio` is compiled in if the show uses audio-bound modulators (or that the operator has seen and acknowledged the runtime hint). Other v1.1 surfaces (SourceModifier presets, Tint, OSC) don't require operator-side preflight beyond the existing FX checks.
**Acceptance:** Checklist covers the new operator-facing capabilities; existing entries unaffected.

#### PCleanup.9.5 — 5-minute acceptance smoke test

**Source:** Mirrors `P4.9.5` (phase-close gate).
**Type:** manual smoke.
**Depends on:** PCleanup.9.1–PCleanup.9.4.
**Files:** none (operator-run checklist; capture results in the PR description).
**What:** A 5-minute path: load a real project, apply ≥4 distinct SourceModifier presets across at least 2 layers, observe that each one **manipulates the underlying photo** (not just overlays). Stack a `Feedback` effect on one layer; verify trails. Bind an OSC parameter to an FX slider; verify the slider moves. Check the cue-strip thumbnails show real scene content.
**Acceptance:**
- [ ] All 4 SourceModifier presets produce visible source-image manipulation.
- [ ] Feedback trails are visible and `decay` responds to slider changes.
- [ ] OSC-bound parameter moves when OSC messages arrive.
- [ ] Cue-strip thumbnails show real scenes, not gradients.
- [ ] No regressions in existing generative-overlay presets.
- [ ] Test results captured in the PCleanup.9.5 PR description as plain-text observations.

This is the phase-close gate. If any acceptance bullet fails, do not bundle a fix into this PR — open a new PCleanup-aligned task.

---

## References

- Phase spec: [`004-phase-cleanup.md`](004-phase-cleanup.md) (full acceptance criteria, fix sketches, anticipated risks).
- Phase 2 four-file FX preset pattern: [`004-phase-2-tasks.md`](004-phase-2-tasks.md) (template for new SourceModifier presets in W2).
- Phase 4 wizard / scene templates: [`004-phase-4-tasks.md`](004-phase-4-tasks.md) (templates may want to expose new SourceModifier siblings — out of scope here, but worth noting for follow-up).
- `src/render/CLAUDE.md` — GPU lifecycle, `RenderCtx`, build-time WGSL validation.
- `src/project/CLAUDE.md` — Mutation Reverse-storage rules (required reading for PCleanup.1.3, PCleanup.1.4, PCleanup.5.1).
- `specs/roadmap.md` — product framing.
