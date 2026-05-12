# 004 Phase 4 — task breakdown

Companion task spec for [`004-phase-4.md`](004-phase-4.md). Each task
below is sized for a single PR.

## Implementation status

- [x] P4.1.1 bb8d53f — glossary entries for Phase 4 domain terms
- [x] P4.1.2 d23feb5 — perf-gate refresh: scene-wizard + multi-template stub fixture
- [x] P4.1.3 0980fc8 — CHANGELOG + README v0.8 placeholder sections
- [x] P4.2.1 28d862e — `SceneTemplate` struct + `SceneTemplateRegistry` skeleton
- [x] P4.2.2 8b7e115 — JSON schema + `.rmap-scene.json` serde round-trip
- [x] P4.2.3 9abe045 — `SceneTemplate` instantiation via `ApplyProjectSnapshot`
- [x] P4.2.4 d564ba3 — audit: `AuditKind::TemplateZonesMissing` + zones-consumed check
- [x] P4.3.1 862d74d — `AppState::SceneWizard` variant + routing skeleton
- [x] P4.3.2 2fbc79b — wizard cancel / back navigation + pre-wizard snapshot stash
- [x] P4.3.3 f9f9f98 — wizard commit → `ApplyProjectSnapshot` + return to Editing
- [x] P4.4.1 e960e18 — wizard step 0: template-select picker UI
- [x] P4.4.2 1970532 — wizard step 1: media-slot picker UI
- [x] P4.4.3 1970532 — wizard step 2: zone-binding picker UI
- [x] P4.4.4 1970532 — wizard step 3: palette + mood picker UI
- [x] P4.4.5 1970532 — wizard step 4: tempo picker UI
- [x] P4.5.1 b668f28 — built-in scene template: `window_reveal`
- [x] P4.5.2 b668f28 — built-in scene template: `pixel_drift`
- [x] P4.5.3 b668f28 — built-in scene template: `collage_bloom`
- [x] P4.5.4 b668f28 — built-in scene template: `glow_behind_openings`
- [x] P4.5.5 b668f28 — built-in scene template: `fragmented_portrait`
- [x] P4.5.6 b668f28 — built-in scene template: `architectural_wash` (upgrade from FX preset label)
- [x] P4.5.7 b668f28 — built-in scene template: `mask_edge_ripple_wash_scene`
- [x] P4.5.8 b668f28 — built-in scene template: `light_spill_from_windows`
- [x] P4.6.1 e74ceca — selected-layer card: scene-aware header (template params above the fold)
- [x] P4.6.2 e74ceca — selected-layer card: "Advanced" disclosure for raw layer params
- [x] P4.7.1 0895dca — mode hint banner: capability-availability hints
- [x] P4.8.1 ec88f9f — proptest extension: `SceneTemplate` serde + registry round-trip
- [x] P4.8.2 56d63c5 — proptest extension: wizard commit / cancel `ApplyProjectSnapshot` round-trip
- [x] P4.8.3 934fe40 — GPU golden: `window_reveal` template renders deterministically
- [x] P4.9.1 6e614a5 — version bump 0.7.0 → 0.8.0
- [x] P4.9.2 28a6cf7 — CHANGELOG body for v0.8
- [x] P4.9.3 28a6cf7 — README — Scene grammars section
- [x] P4.9.4 28a6cf7 — show-day checklist: scene template validation, zone-binding audit
- [ ] P4.9.5 — Phase 4 acceptance smoke test (manual: 5-minute operator path)

---

## Operating model

- **Model:** Sonnet implements; Opus reviews. Read the originating spec
  section, every CLAUDE.md the task touches, and the decision docs above
  before starting.
- **Pick one task at a time.** Read `004-phase-4.md` and the relevant
  entry in `specs/roadmap.md` before starting.
- **Commit message format:** `004-P4.<workstream>.<task>: <title>` — e.g.
  `004-P4.2.1: SceneTemplate struct + registry skeleton`.
- **Branching:** one branch per task; merge straight to `main` once CI is
  green.
- **Pre-commit hook** (`.githooks/pre-commit`, wired by `make setup`) runs
  rustfmt on staged files + `cargo check`. Heavier checks live in `make ci`;
  run that before opening a PR.
- **Tests:** every task ships with new or updated tests. For schema /
  Mutation / snapshot work, follow the v3 proptest pattern in
  `src/project/command.rs`. For render-path work, add a golden under
  `tests/golden/` (covered by `--features gpu-tests`); use `UPDATE_GOLDEN=1`
  to (re-)record the baseline. Where automation isn't possible (manual wizard
  UX, 5-minute smoke test), ship a manual smoke-test checklist — never nothing.
- **Read the right CLAUDE.md.** Tasks touching `src/project/` must read
  `src/project/CLAUDE.md` first (Mutation Reverse-storage rules, snapshot
  invariants). Tasks touching `src/render/` must read `src/render/CLAUDE.md`
  first (GPU lifecycle, panic_restore, build-time WGSL validation).
- **Don't bundle.** If a task tempts you to also fix something nearby,
  resist — that "something nearby" probably already has its own task ID below.
- **Scene template ≠ FX preset (four-file pattern does not apply).** A
  scene template is a recipe that assembles existing primitives (FX presets
  from Phase 2, zones from Phase 3, media slots). A built-in template is a
  `SceneTemplate` struct value registered in `SceneTemplateRegistry`. There
  is no new WGSL shader, no new wgpu pipeline, no new dispatch arm. The
  four-file FX preset pattern from Phase 2 is explicitly *not* the model here.
- **Phase 3 dependency.** Phase 4 templates consume `ZoneRole`-tagged masks
  from Phase 3. Tasks that need zone binding (P4.4.3, P4.5.4, P4.5.8,
  P4.5.1) are marked BLOCKED on Phase 3's zone interface landing. If Phase 3
  ships a `ZoneRole` enum and `zones_for_role()` query before Phase 4 begins,
  remove the BLOCKED annotation. If Phase 3 has not shipped, stub zone
  binding with a `Vec<ZoneRole>` placeholder in `WizardChoices` that the
  zone-binding step populates with empty vecs; templates render without zone
  binding (fallback to full-canvas).
- **GPU bring-up tasks ship golden images.** Anything that renders pixels
  through a new scene-template path needs a `tests/golden/` baseline under
  `--features gpu-tests`.

## Task ID conventions

- IDs are flat-numbered within nine workstreams:
  - W1 — Setup + housekeeping (glossary, perf-gate, CHANGELOG/README placeholder)
  - W2 — Scene template schema + Mutation foundation
  - W3 — Wizard state machine (AppState wiring, navigation, cancel/back, commit)
  - W4 — Wizard step UIs (five steps: template-select, media, zones, palette/mood, tempo)
  - W5 — Built-in scene templates (one task per template)
  - W6 — Scene-aware selected-layer card refactor
  - W7 — Capability-availability mode hint banner
  - W8 — Snapshot / proptest / determinism
  - W9 — Release housekeeping + 5-minute acceptance smoke test
- Tasks cite Phase 2 / Phase 3 precedents by task ID where the pattern is
  reused (e.g. "mirrors P2.8.5's storage location decision").

## Workstream summary

| WS | Theme | Tasks | Parallel-safe? | Touches |
|----|-------|-------|----------------|---------|
| 1 | Setup + housekeeping | 3 | All three parallel-safe | `src/windows/glossary.rs`, `tests/perf_frame_budget.rs`, `CHANGELOG.md`, `README.md` |
| 2 | Scene template schema + Mutation | 4 | P4.2.1 first; P4.2.2 + P4.2.3 + P4.2.4 serial after | new `src/project/scene_templates.rs`, `src/project/audit.rs` |
| 3 | Wizard state machine | 3 | P4.3.1 first (BLOCKED); P4.3.2 + P4.3.3 serial after | `src/app.rs`, new `src/windows/wizard.rs` |
| 4 | Wizard step UIs | 5 | P4.4.1 first; P4.4.2–P4.4.5 parallel after | `src/windows/wizard.rs` |
| 5 | Built-in scene templates | 8 | P4.2.1 first; P4.5.1–P4.5.8 parallel after | `src/project/scene_templates.rs` |
| 6 | Scene-aware selected-layer card | 2 | P4.6.1 first; P4.6.2 after | `src/windows/control_panel.rs`, `src/windows/inspector.rs` |
| 7 | Mode hint banner | 1 | After W3 lands; extends existing `mode_banner` in `scene_editor.rs` | `src/windows/scene_editor.rs:1145` |
| 8 | Snapshot / proptest / determinism | 3 | P4.8.1 after W2; P4.8.2 after W3; P4.8.3 after P4.5.1 + gpu-tests | `src/project/command.rs`, `tests/headless_gpu.rs` |
| 9 | Release housekeeping + acceptance smoke | 5 | Last — depends on everything else | `Cargo.toml`, `CHANGELOG.md`, `README.md`, `docs/show-day-checklist.md` |

**Suggested PR sequencing:**

1. Resolve both decision docs (wizard state machine + scene template schema).
   Gate every W2–W5 task on those decisions landing first.
2. **P4.1.1 + P4.1.2 + P4.1.3** in parallel — independent housekeeping.
3. **P4.2.1** (SceneTemplate struct + registry) — unblocks W5 and W2.2–W2.4.
4. **P4.2.2 + P4.2.3 + P4.2.4** serial after P4.2.1.
5. **P4.3.1** (wizard `AppState` variant) — unblocks W4 and W3.2–W3.3.
6. **P4.3.2 + P4.3.3** serial after P4.3.1.
7. **P4.4.1** (template picker) after P4.3.1; **P4.4.2 + P4.4.4 + P4.4.5**
   parallel after P4.4.1; **P4.4.3** (zone binding) parallel but BLOCKED on
   Phase 3.
8. **P4.5.1–P4.5.8** in parallel after P4.2.1 lands — each template is a
   standalone registry entry.
9. **P4.6.1 + P4.6.2** serial; can run in parallel with W5.
10. **P4.7.1** after W3 is green (extends existing `mode_banner`).
11. **P4.8.1 + P4.8.2** after W2 + W3; **P4.8.3** after P4.5.1 + gpu adapter.
12. **P4.9.1 → P4.9.5** last; P4.9.5 runs the 5-minute acceptance smoke.

## Anticipated risks

These design decisions are locked (or pending the decision docs above). Each
is a potential scope-creep site; call it out at task time if implementation
pressure pushes toward a different choice.

1. **Scene templates are read-only recipes, not mutable scene state.** A
   `SceneTemplate` is a static value in the registry. It carries no warp
   geometry, no absolute positions, and no per-project parameters. Templates
   are applied via the wizard; the resulting layers live in `project.layers`
   as ordinary `LayerConfig` entries. Template identity is NOT tracked on
   the live layer — "which template produced this layer" is not stored.

2. **Instantiation = `ApplyProjectSnapshot`, not a new `Mutation` variant.**
   See `004-phase-4-scene-template-schema-decision.md` §Sub-question 4.
   The wizard builds a scratch `Project` in memory and commits it as a single
   `ApplyProjectSnapshot { non_undoable: false }`. No new `Mutation` variant
   is needed. This is the scope-creep guard for W2 and W3 — resist any
   impulse to add a `LoadSceneTemplate` Mutation variant.

3. **Wizard state machine shape: new `AppState::SceneWizard` variant.**
   See `004-phase-4-wizard-state-machine-decision.md`. The wizard is NOT a
   flag on `EditingState`. Adding a `wizard: Option<…>` field to
   `EditingState` is explicitly out of scope for this phase.

4. **Zone binding stubbed when Phase 3 is absent.** If Phase 3's `ZoneRole`
   enum and zone-query API are not yet shipped when Phase 4 begins, the
   zone-binding wizard step (P4.4.3) uses a `Vec<ZoneRole>` placeholder with
   empty defaults. Templates that declare `zones_consumed` render without zone
   binding (full-canvas fallback). Remove the placeholder and implement real
   zone binding once Phase 3 lands.

5. **Built-in templates declare `zones_consumed` but may render without them.**
   Each built-in template lists which `ZoneRole` tags it consumes. The
   instantiation pass checks whether the project contains zones of those roles
   and emits an `AuditKind::TemplateZonesMissing` Warn finding if not. The
   template still instantiates — zones improve the output but are not required.
   This is the fallback policy; it is not a fallback for skipping Phase 3.

6. **One PR per built-in template.** Each of the eight scene templates in W5
   is its own leaf task (a new `SceneTemplate` struct value in
   `src/project/scene_templates.rs`, plus a registry-entry constant and a unit
   test). Do NOT bundle two templates into one task. This is the scope-creep
   guard for W5.

7. **The `architectural_wash` upgrade is not a greenfield template.** The
   Phase 4 plan specifically calls out: "The 'Architectural Wash' template
   (already a v3 preset name in the effect chain dropdown) is upgraded to a
   full scene template." P4.5.6 replaces or supplements the existing FX preset
   label with a `SceneTemplate` that consumes `media + zones`, not just
   parameters. The existing `mask_edge_ripple_wash` FX preset stays in the
   registry; the new `architectural_wash` scene template composes it with
   media and zone bindings.

8. **Mode hint banner extends; it does not replace.** `mode_banner` at
   `src/windows/scene_editor.rs:1145` already exists. P4.7.1 extends it with
   capability-availability hints — it does NOT rewrite the mode banner or
   change existing hint copy. Adding a new hint for a capability that belongs
   to a future phase requires citing the future phase in the copy (e.g.
   "Bezier handles — Phase 7").

---

## Workstream 1 — Setup + housekeeping

Quick independent wins that ship before the heavier workstreams.

### P4.1.1 — Glossary entries for Phase 4 domain terms

**Source:** `004-phase-4.md` Capability set ("scene templates", "scene grammar",
"wizard", template names); UX items ("mode hint banner",
"capability-availability hints", "template parameters", "Advanced disclosure").
**Type:** docs / UX
**Depends on:** none
**Files:** `src/windows/glossary.rs` (existing `GlossaryTerm` enum;
`EXPECTED_VARIANT_COUNT` is currently 63 after Phase 2).

**What:** Phase 4 introduces a cluster of scene-level terms that operators will
encounter in the wizard, the template picker, the selected-layer card, and the
mode hint banner. Adding glossary entries before those surfaces ship means W3–W7
tasks can call `glossary_label(ui, GlossaryTerm::X)` without a separate docs
task. Pattern is identical to P1.1.3 and P2.1.1 — extend the `GlossaryTerm`
enum with new variants and add short (~30-word) operator-facing definitions.

**Domain terms (~6):** *scene template*, *scene grammar*, *wizard*, *palette
(mood palette)*, *mood*, *tempo sync*.

**Capability-availability hint terms (~2):** *bezier handles* (Phase 7
follow-on); *fluid sim* (Phase 2 preset — confirm entry exists from P2.1.1;
add if missing).

**Built-in template display labels (8):** *window reveal*, *pixel drift*,
*collage bloom*, *glow behind openings*, *fragmented portrait*,
*architectural wash* (confirm existing entry, upgrade wording if needed),
*mask-edge ripple wash scene*, *light spill from windows*.

Total new variants: ~14–16 (verify `EXPECTED_VARIANT_COUNT` arithmetic before
bumping; do not trust these estimates without counting the existing enum).

**Steps:**
1. Read `src/windows/glossary.rs` — locate the `GlossaryTerm` enum, the
   display match, and `EXPECTED_VARIANT_COUNT` (currently 63 after Phase 2 /
   Phase 3 additions — verify the live count before assuming 63).
2. Add one enum variant per new term.
3. Write a short (~30 word) operator-facing definition in the display match arm.
4. Bump `EXPECTED_VARIANT_COUNT` to the new total.

**Acceptance:**
- [ ] All domain terms + capability-availability hint terms + all built-in
      template display labels have `GlossaryTerm` variants and definitions.
- [ ] `EXPECTED_VARIANT_COUNT` bumped to match.
- [ ] Existing exhaustiveness tests still pass.
- [ ] Definitions are operator-facing copy, not implementation notes.
- [ ] `make ci` clean.

**Out of scope:** Phase 5 terms (fixture, DMX universe, Art-Net, sACN).

---

### P4.1.2 — Show-day perf-gate refresh: scene-wizard + multi-template stub fixture

**Source:** `004-phase-4.md` Acceptance criteria ("A new operator can produce a
coherent immersive scene in under five minutes starting from the launcher");
`specs/roadmap.md` §"Show-day reliability".
**Type:** engine (defensive)
**Depends on:** none (sets baseline; real template layers land later in W5)
**Files:** `tests/perf_frame_budget.rs`.

**What:** the existing perf gate validates a representative scene against a p99
frame-time target on the M-series baseline. Phase 4 introduces the wizard
transition (`AppState::SceneWizard → AppState::Editing`) and multi-template
scenes (potentially several FX layers composited with image layers). This task
extends the perf gate with a stub fixture: a four-layer scene composed of two
image layers and two FxLayer presets, simulating a post-wizard scene. The test
asserts p99 ≤ 16.6 ms. When W5 templates land, the fixture is updated in-place
to use real template-generated layers.

**Steps:**
1. Read `tests/perf_frame_budget.rs` — understand existing test structure
   (frame-render loop, sample count, p99 computation, skip conditions).
2. Add `perf_scene_template_scene_within_budget` that builds a four-layer scene
   (two image layers, two `FxLayer { preset_id: RIPPLE_WASH_PRESET_ID }`), and
   asserts p99 ≤ 16.6 ms.
3. Mark with `#[cfg(feature = "gpu-tests")]` and the appropriate skip condition.
4. Document in a comment that the fixture will be updated in W5 to use real
   template-generated layers.

**Acceptance:**
- [ ] New `perf_scene_template_scene_within_budget` test exists under
      `--features gpu-tests`.
- [ ] Test skips cleanly when no GPU adapter is available.
- [ ] `make ci` clean.

**Out of scope:** real template-generated layer fixture (W5); wizard transition
timing (separate concern).

---

### P4.1.3 — CHANGELOG + README Phase 4 placeholder sections

**Source:** `004-phase-4.md` Goal ("Move from a renderer-centric experience to
a scene-centric product").
**Type:** docs / UX
**Depends on:** none
**Files:** `CHANGELOG.md`, `README.md`.

**What:** drop a shell section for v0.7 in both files so W9 tasks only need to
fill body text. No version bump yet (that is P4.9.1). Pattern mirrors P2.1.3.

**Steps:**
1. In `CHANGELOG.md`, add `## [Unreleased] — v0.7` above the v0.6 entry with
   three placeholder subsections: `### Scene Templates`, `### Scene Wizard`,
   `### Selected-Layer Scene Card`.
2. In `README.md`, add a "Scene Grammars (v0.7)" subsection under Features with
   a one-sentence placeholder.

**Acceptance:**
- [ ] `CHANGELOG.md` has an `[Unreleased] — v0.7` header with placeholders.
- [ ] `README.md` has a stub Scene Grammars entry.
- [ ] No version strings changed.
- [ ] `make ci` clean.

**Out of scope:** filling CHANGELOG body (P4.9.2); README prose (P4.9.3);
version bump (P4.9.1).

---

## Workstream 2 — Scene template schema + Mutation foundation

The architectural workstream. Introduces the `SceneTemplate` type, JSON
schema, and instantiation path that every W5 template and W3 wizard step
depends on.

> **BLOCKED on:** `004-phase-4-scene-template-schema-decision.md` — resolve
> before starting P4.2.1.

### P4.2.1 — `SceneTemplate` struct + `SceneTemplateRegistry` skeleton

**Source:** `004-phase-4.md` Engine implications ("Scene template format:
portable JSON schema, lives alongside the per-project file but is reusable
across projects"); `004-phase-4-scene-template-schema-decision.md` §Option 1B.
**Type:** engine
**Depends on:** decision doc resolved.
**Files:** new `src/project/scene_templates.rs`, `src/project/mod.rs` (mod
declaration).

**What:** introduce `SceneTemplate` (recipe model: `id`, `display_name`,
`description`, `zones_consumed`, `media_slots`, `fx_presets_used`, `palette`,
`mood`, `tempo_sync`, `builtin`) and `SceneTemplateRegistry` (free functions
mirroring `src/render/fx_presets.rs`'s registry pattern: `fn scene_registry()`,
`fn scene_is_registered()`, `fn scene_display_label()`). Register no built-in
templates yet — those are W5. The `MediaSlotDescriptor` type (name, label,
accepts: `Vec<MediaSlotKind>`) is also introduced here.

**Steps:**
1. Read `src/project/zone_templates.rs` and `src/render/fx_presets.rs` —
   internalise the existing registry patterns before introducing a new one.
2. Read `src/project/CLAUDE.md` — note serde default rules for new optional
   fields.
3. Create `src/project/scene_templates.rs` with:
   - `pub struct SceneTemplate { id, display_name, description,
     zones_consumed: Vec<ZoneRole>, media_slots: Vec<MediaSlotDescriptor>,
     fx_presets_used: Vec<&'static str>, palette: PaletteHint,
     mood: MoodHint, tempo_sync: bool, builtin: bool }`
   - `pub struct MediaSlotDescriptor { name, label, accepts: Vec<MediaSlotKind> }`
   - `pub enum MediaSlotKind { Image, Video, Any }`
   - `pub enum PaletteHint { Warm, Cool, Neutral }`
   - `pub enum MoodHint { Calm, Energetic, Ethereal }`
   - `pub fn scene_registry() -> &'static [SceneTemplate]` (empty slice for now)
   - `pub fn scene_is_registered(id: &str) -> bool`
   - `pub fn scene_display_label(id: &str) -> Option<&'static str>`
4. Note: `ZoneRole` is defined by Phase 3. If Phase 3 has not yet landed,
   stub it as `pub enum ZoneRole { Window, Portal, Void, Spill, Edge,
   Highlight, LightSource }` in this file with a `// TODO: import from
   src/project/zones.rs once Phase 3 lands` comment.
5. Add `pub mod scene_templates;` to `src/project/mod.rs`.

**Tests:**
- Unit test: `scene_registry()` returns a non-empty or empty slice without
  panic (empty is correct at this stage).
- Unit test: `scene_is_registered("nonexistent")` returns `false`.
- Unit test: `scene_display_label("nonexistent")` returns `None`.

**Acceptance:**
- [ ] `SceneTemplate`, `MediaSlotDescriptor`, `MediaSlotKind`, `PaletteHint`,
      `MoodHint` types exist and are `serde::{Serialize, Deserialize}`.
- [ ] Registry free functions exist in `src/project/scene_templates.rs`.
- [ ] `ZoneRole` is available (either from Phase 3 or stubbed with a comment).
- [ ] `make ci` clean.

**Out of scope:** JSON file serde round-trip (P4.2.2); instantiation mutation
(P4.2.3); audit (P4.2.4); any built-in templates (W5).

---

### P4.2.2 — JSON schema + `.rmap-scene.json` serde round-trip

**Source:** `004-phase-4-scene-template-schema-decision.md` §Sub-question 2 +
§Sub-question 3 (file extension = `.rmap-scene.json`; user templates at
`~/Library/Application Support/rmap/scenes/`).
**Type:** engine
**Depends on:** P4.2.1.
**Files:** `src/project/scene_templates.rs`, new
`src/windows/scene_io.rs` (mirrors `src/windows/preset_io.rs`).

**What:** P4.2.1 introduces the `SceneTemplate` struct with `serde` derives.
This task ensures the full JSON round-trip is correct, adds a
`load_user_scene_templates()` function that reads `~/Library/Application
Support/rmap/scenes/*.rmap-scene.json` (mirroring `preset_io.rs`'s load
pattern from P2.8.5), and documents the file extension convention.

**Steps:**
1. Read `src/windows/preset_io.rs` — understand the P2.8.5 load/save pattern.
2. Add `pub fn load_user_scene_templates() -> Vec<SceneTemplate>` to a new
   `src/windows/scene_io.rs`. It reads `~/.../rmap/scenes/*.rmap-scene.json`,
   deserialises each with `serde_json::from_str`, logs errors but does not
   panic on malformed files (mirrors `preset_io`'s defensive behaviour).
3. Add `pub fn save_user_scene_template(template: &SceneTemplate) -> Result<()>`
   that serialises to `~/.../rmap/scenes/{id}.rmap-scene.json`.
4. Add round-trip unit tests for every `SceneTemplate` field using a fixture
   JSON string.
5. Confirm that a built-in template (`builtin: true`) serialises but the save
   function returns `Err(...)` for built-in templates (read-only enforcement).

**Tests:**
- Unit test: `SceneTemplate` round-trips through `serde_json::to_string` +
  `serde_json::from_str` with all fields populated.
- Unit test: loading a `.rmap-scene.json` fixture with a missing optional field
  produces a valid `SceneTemplate` (serde defaults).
- Unit test: saving a `builtin: true` template returns an error.

**Acceptance:**
- [ ] `load_user_scene_templates()` and `save_user_scene_template()` exist.
- [ ] Built-in templates are read-only (save returns an error).
- [ ] Round-trip test passes for all fields.
- [ ] `make ci` clean.

**Out of scope:** star/favourite state for scenes (`scene_stars.json` — P4.9
follow-up); the wizard UI for import/export (deferred to Phase 7 per Phase 4's
"scene packs / export-import → Phase 7" out-of-scope statement).

---

### P4.2.3 — `SceneTemplate` instantiation via `ApplyProjectSnapshot`

**Source:** `004-phase-4.md` Scene editor flow ("Once committed, the scene
drops into the standard Editing mode"); `004-phase-4-scene-template-schema-decision.md`
§Sub-question 4 (Option 4B).
**Type:** engine
**Depends on:** P4.2.1, P4.3.1 (wizard `AppState` must exist for the caller
site — can be developed and unit-tested independently, but integration requires
P4.3.1).
**Files:** new `src/project/scene_instantiation.rs`, `src/project/mod.rs`.

**What:** implement `instantiate_template(template: &SceneTemplate, choices:
&WizardChoices) -> serde_json::Value` — a pure function that:
1. Clones the current project JSON (passed in as `base_project: serde_json::Value`).
2. Removes all existing layers from the clone.
3. Calls `AddLayer` mutations in sequence on a scratch `Project` (deserialised
   from the clone) to build the template's layer stack, assigning `choices.media_slots`
   to layer `path` fields and `choices.zone_bindings` to zone role fields.
4. Serialises the resulting `Project` back to `serde_json::Value`.

The wizard's commit step (P4.3.3) passes this value to `ApplyProjectSnapshot {
new: instantiate_template(...), old: pre_wizard_snapshot, non_undoable: false }`.

**Steps:**
1. Read `src/project/CLAUDE.md` — the three Reverse-storage rules. Confirm that
   the scratch `Project` build path does NOT produce any undo entries (mutations
   are applied directly to the scratch project, not dispatched through the undo
   stack).
2. Define `pub struct WizardChoices { pub template_id: String, pub
   media_slots: HashMap<String, PathBuf>, pub zone_bindings:
   Vec<ZoneRole>, pub palette: PaletteHint, pub mood: MoodHint,
   pub tempo_sync: bool }` (here or in the wizard module — confirm with P4.3.1).
3. Implement `pub fn instantiate_template(template: &SceneTemplate,
   choices: &WizardChoices, base_project: serde_json::Value) ->
   serde_json::Value`.
4. The function must not panic; return the base project unchanged if
   `template.id` is not found in the registry (defensive).

**Tests:**
- Unit test: `instantiate_template` for a minimal `SceneTemplate` with one
  media slot produces a `serde_json::Value` with one layer whose path matches
  `choices.media_slots["slot_0"]`.
- Unit test: calling with an empty `media_slots` map produces a layer with an
  empty path (not a panic).
- Unit test: the returned JSON deserialises back to a valid `Project`.

**Acceptance:**
- [ ] `instantiate_template` is a pure function with no side effects.
- [ ] No panics on malformed or incomplete choices.
- [ ] Returned JSON is a valid `Project`.
- [ ] `make ci` clean.

**Out of scope:** wizard UI (W3 / W4); undo integration (P4.3.3); zone
binding when Phase 3 has not shipped (stub with empty `zone_bindings`).

---

### P4.2.4 — Audit: `AuditKind::UnknownSceneTemplate` + zones-consumed check

**Source:** `004-phase-4.md` Acceptance criteria ("Every scene template
documents which zones it consumes, so the zone-mapping step is unambiguous").
**Type:** engine (defensive)
**Depends on:** P4.2.1.
**Files:** `src/project/audit.rs`.

**What:** a project that references a scene template ID (if we decide to
track template origin — see Note below) should not silently ignore missing
templates. More load-bearing: a project whose layers were generated from a
template that requires `zones_consumed` but whose project has no zones of
those roles should emit an informational Warn finding so the operator can add
zone tags.

> **Note on template-ID tracking.** Phase 4's instantiation commits via
> `ApplyProjectSnapshot` and does NOT store which template produced which
> layers (Anticipated risk #1 above: "Template identity is NOT tracked on the
> live layer"). Therefore `AuditKind::UnknownSceneTemplate` is not applicable
> at project-load time. This task instead focuses on the zones-consumed check:
> after the wizard commits, if the resulting project has layers whose
> `fx_presets_used` include zone-consuming presets but the project has no
> `ZoneRole`-tagged masks, emit a `TemplateZonesMissing` Warn finding.

**Steps:**
1. Read `src/project/audit.rs` — understand the existing audit pass structure.
2. Add `AuditKind::TemplateZonesMissing { zone_roles: Vec<ZoneRole> }` variant
   (Severity::Warn — scene renders without zones but operator likely intended them).
3. In the audit pass, check: if any `FxLayer` in `project.layers` uses a
   preset listed in any built-in template's `fx_presets_used`, and the project
   has no masks tagged with the corresponding `ZoneRole`, emit
   `TemplateZonesMissing`. Skip this check if Phase 3's zone tagging is not
   yet shipped (guard with `#[cfg(feature = "v3")]` or equivalent).
4. Finding is advisory; project still loads.

**Tests:**
- Unit test: a project with an FX layer using `RIPPLE_WASH_PRESET_ID` and a
  template that declares `zones_consumed: [ZoneRole::Window]`, but no
  `ZoneRole::Window` masks in the project, produces a `TemplateZonesMissing`
  Warn finding.
- Unit test: same project WITH a `ZoneRole::Window` mask produces no
  `TemplateZonesMissing` finding.

**Acceptance:**
- [ ] `AuditKind::TemplateZonesMissing` variant exists with Severity::Warn.
- [ ] Audit uses `scene_registry()` to look up `zones_consumed`; no hardcoded
      preset IDs in the audit logic.
- [ ] Finding is advisory (project loads regardless).
- [ ] `make ci` clean.

**Out of scope:** `UnknownSceneTemplate` (not applicable — template identity
not stored on layers); zone-tagging UI (Phase 3).

---

## Workstream 3 — Wizard state machine

Introduces the `AppState::SceneWizard` variant and the state transitions
(`Editing → SceneWizard → Editing`). All three tasks must be taken in order.

> **BLOCKED on:** `004-phase-4-wizard-state-machine-decision.md` — resolve
> before starting P4.3.1.

### P4.3.1 — `AppState::SceneWizard` variant + routing skeleton

**Source:** `004-phase-4.md` Engine implications ("Scene editor state machine
for the wizard-style flow. Reuse the existing v3 launcher / state-machine
plumbing where possible"); `004-phase-4-wizard-state-machine-decision.md`
Option A.
**Type:** engine
**Depends on:** decision doc resolved; P4.2.1 (`WizardChoices` type).
**Files:** `src/app.rs`, new `src/windows/wizard.rs`, `src/app/mod.rs`
(if the `src/app/` directory exists).

**What:** add `AppState::SceneWizard(SceneWizardState)` to the `AppState` enum.
`SceneWizardState` holds: `pre_wizard_snapshot: serde_json::Value`,
`choices: WizardChoices`, `step: WizardStep`, `editing: EditingState`.
Add stub routing arm in `App::window_event` (`SceneWizard(s) =>
handle_wizard_window_event(s, ...)`) that delegates to a new
`src/windows/wizard.rs` module. The stub renders a placeholder "Scene Wizard
(coming)" panel in the control window. No functional wizard yet.

`WizardStep` enum: `TemplateSelect`, `Media`, `ZoneBinding`, `Palette`,
`Tempo`, `Confirm`.

`ControlFlow` for `SceneWizard`: `Poll` (canvas must keep animating).
`AppState::is_running` extended to include `SceneWizard`.

**Steps:**
1. Read `src/app.rs:129` — internalise the `AppState` enum, `is_running`,
   `control_flow`, `editing_mut` implementations.
2. Add `SceneWizard(SceneWizardState)` to `AppState`.
3. Extend `is_running` and `control_flow` (`Poll`) for the new variant.
4. Add `editing_mut` pass-through: `SceneWizard` returns `None` (wizard does
   not expose `EditingState` directly).
5. Add stub `handle_wizard_window_event` in `src/windows/wizard.rs`.
6. Add an entry-point function `enter_scene_wizard(state: EditingState,
   snapshot: serde_json::Value) -> AppState` that constructs
   `AppState::SceneWizard(SceneWizardState { ... })`. Called from the
   "New scene from template" action.

**Tests:**
- Unit test: `AppState::SceneWizard(stub).control_flow() == ControlFlow::Poll`.
- Unit test: `AppState::SceneWizard(stub).is_running() == true`.
- Unit test: `AppState::SceneWizard(stub).editing_mut() == None`.
- Compile test: the new variant in `AppState::is_running` + `control_flow`
  produces no non-exhaustive pattern warning.

**Acceptance:**
- [ ] `AppState::SceneWizard` variant exists in `src/app.rs`.
- [ ] `is_running`, `control_flow`, `editing_mut` cover the new variant.
- [ ] Stub wizard panel renders without panic.
- [ ] `make ci` clean.

**Out of scope:** step UIs (W4); cancel/back (P4.3.2); commit (P4.3.3).

---

### P4.3.2 — Wizard cancel / back navigation + pre-wizard snapshot stash

**Source:** `004-phase-4-wizard-state-machine-decision.md` Option A
(stash = `pre_wizard_snapshot: serde_json::Value`; cancel = non-undoable
`ApplyProjectSnapshot`).
**Type:** engine
**Depends on:** P4.3.1.
**Files:** `src/windows/wizard.rs`, `src/app.rs`.

**What:** implement the cancel and back-navigation paths:
- **Cancel:** dispatches `ApplyProjectSnapshot { new: pre_wizard_snapshot,
  old: current_project_json, non_undoable: true }` to restore the
  pre-wizard state, then transitions `AppState::SceneWizard →
  AppState::Editing(editing)` (moving the stashed `EditingState` back).
- **Back:** decrements `step` in `WizardChoices`. Does NOT dispatch any
  mutation. The step enum must include a `prev()` method.
- **Keyboard:** `Escape` cancels; `←` backs one step.
- **UI:** add Cancel and Back buttons to the wizard panel footer.

**Steps:**
1. Implement `WizardStep::prev() -> Option<WizardStep>`.
2. Wire `Escape` key → cancel in `handle_wizard_window_event`.
3. Wire `←` / Back button → back.
4. Implement the cancel dispatch: read the pre-wizard snapshot from
   `SceneWizardState.pre_wizard_snapshot`; dispatch `ApplyProjectSnapshot {
   non_undoable: true }` into the editing state; reconstruct `AppState::Editing`.

**Tests:**
- Unit test: `WizardStep::TemplateSelect.prev()` returns `None`.
- Unit test: `WizardStep::Media.prev()` returns `Some(WizardStep::TemplateSelect)`.
- Integration smoke: starting wizard and immediately cancelling leaves
  `project` byte-equal to the pre-wizard snapshot.

**Acceptance:**
- [ ] Cancel restores the pre-wizard project via non-undoable
      `ApplyProjectSnapshot`.
- [ ] Back decrements the step without dispatching a mutation.
- [ ] Escape key cancels the wizard.
- [ ] `make ci` clean.

**Out of scope:** wizard commit (P4.3.3); step-specific UIs (W4).

---

### P4.3.3 — Wizard commit → `ApplyProjectSnapshot` + return to Editing

**Source:** `004-phase-4.md` Scene editor flow ("Once committed, the scene
drops into the standard Editing mode"); `004-phase-4-scene-template-schema-decision.md`
§Option 4B.
**Type:** engine
**Depends on:** P4.3.1, P4.2.3 (`instantiate_template`).
**Files:** `src/windows/wizard.rs`, `src/app.rs`.

**What:** implement the wizard commit path:
- On "Confirm" (final step): call `instantiate_template(template, choices,
  base_project)` to build the new project JSON.
- Dispatch `ApplyProjectSnapshot { new: generated_json,
  old: pre_wizard_snapshot, non_undoable: false }` so the operator can
  undo the entire wizard with one Cmd-Z.
- Transition `AppState::SceneWizard → AppState::Editing(editing)` with the
  mutated `EditingState` (project now reflects the template).
- Add "Confirm" button on the Tempo step footer; wire `Return` as the
  keyboard shortcut.

**Steps:**
1. Wire "Confirm" button + `Return` key in `handle_wizard_window_event`.
2. Call `instantiate_template(template, &choices, base_project_json)`.
3. Dispatch `ApplyProjectSnapshot { non_undoable: false }` into
   `editing.command_tx`.
4. Reconstruct `AppState::Editing(editing)` and return it.

**Tests:**
- Integration smoke: wizard commit produces a project with the expected number
  of layers for the chosen template.
- Integration smoke: immediately pressing Cmd-Z after commit restores the
  pre-wizard project (undo round-trip).

**Acceptance:**
- [ ] Wizard commit dispatches a user-undoable `ApplyProjectSnapshot`.
- [ ] Post-commit `AppState` is `Editing` with the new project.
- [ ] Cmd-Z after commit restores the pre-wizard state.
- [ ] `make ci` clean.

**Out of scope:** per-step validation (error toast if a required media slot is
empty — deferred to a follow-up; the template still instantiates with empty
paths).

---

## Workstream 4 — Wizard step UIs

Five steps: template select, media, zone binding, palette/mood, tempo.
All depend on P4.3.1. P4.4.3 is additionally BLOCKED on Phase 3.

### P4.4.1 — Wizard step 0: template-select picker UI

**Source:** `004-phase-4.md` Scene editor flow (wizard-style entry: starts by
selecting a template).
**Type:** UI
**Depends on:** P4.3.1, P4.2.1 (`scene_registry()`).
**Files:** `src/windows/wizard.rs`.

**What:** implement the first wizard step: a scrollable grid of template cards,
each showing `display_name`, `description`, and a placeholder thumbnail. The
operator clicks a card to select the template; clicking "Next" advances to
`WizardStep::Media`. The selection is stored in `WizardChoices.template_id`.

**Steps:**
1. Read `src/windows/preset_browser.rs` — the preset browser modal (P2.8.1)
   is the closest existing precedent for a scrollable card grid.
2. Implement `draw_template_select_step(ui, wizard_state)` in `wizard.rs` using
   `egui::Grid` or `egui::ScrollArea`.
3. Each card: template name (bold), one-line description, placeholder 64×64
   px grey rect as thumbnail (real thumbnails deferred).
4. Selected card is highlighted with the warm accent colour.
5. "Next" button is disabled until a template is selected.

**Acceptance:**
- [ ] All registered templates appear in the grid.
- [ ] Clicking a card selects it (accent highlight).
- [ ] "Next" is disabled until a selection exists.
- [ ] `make ci` clean.

**Out of scope:** real template thumbnails (deferred to Phase 7
professionalisation); search/filter within the template picker (deferred).

---

### P4.4.2 — Wizard step 1: media-slot picker UI

**Source:** `004-phase-4.md` Scene editor flow ("media → zones → palette →
mood → tempo").
**Type:** UI
**Depends on:** P4.3.1, P4.2.1 (`MediaSlotDescriptor`).
**Files:** `src/windows/wizard.rs`.

**What:** for each `MediaSlotDescriptor` in the selected template, render a
labelled file-picker row (path DragValue or "Choose…" button that opens an
`rfd` file dialog). Assigned paths are stored in
`WizardChoices.media_slots`. Slots may be left empty; the "Next" button
always advances (empty slots produce layers with empty paths — the operator
can assign media after committing).

**Steps:**
1. Read `src/windows/file_dialogs.rs` — understand the existing `rfd` file-
   picker pattern.
2. For each slot: label from `MediaSlotDescriptor.label`, a read-only
   path display, and a "Choose…" button that opens a file dialog filtered to
   `MediaSlotKind` (images for `Image`, video for `Video`, both for `Any`).
3. A ✕ clear button removes the assigned path.

**Acceptance:**
- [ ] All media slots for the selected template are listed with their labels.
- [ ] Choosing a file assigns it to `WizardChoices.media_slots`.
- [ ] Clear button works.
- [ ] Empty slots do not prevent advancing to the next step.
- [ ] `make ci` clean.

---

### P4.4.3 — Wizard step 2: zone-binding picker UI

**Source:** `004-phase-4.md` Scene editor flow ("zones"); acceptance criteria
("Every scene template documents which zones it consumes, so the zone-mapping
step is unambiguous").
**Type:** UI
**Depends on:** P4.3.1, P4.2.1; **BLOCKED on Phase 3** (`ZoneRole` tagging +
zone-query API from Phase 3 must be shipped).
**Files:** `src/windows/wizard.rs`.

**What:** for each `ZoneRole` in the selected template's `zones_consumed`,
render a row showing the role label and a dropdown listing all masks in the
project that are tagged with that role (from Phase 3's zone API). The operator
confirms or reassigns the binding. If no masks are tagged for a role, the row
shows a "No zones tagged as X in this project" message with a link hint ("Tag
a mask in Mask mode").

**Phase 3 gate:** if Phase 3 is not yet shipped, stub this step with a
"Zone binding — requires Phase 3 zones" placeholder panel that still allows
"Next" to proceed.

**Acceptance:**
- [ ] Each `zones_consumed` role has a binding row.
- [ ] Rows show available tagged masks from the project.
- [ ] Unbound roles show an actionable message.
- [ ] Step advances regardless of binding state (zones improve; they don't gate).
- [ ] `make ci` clean.

---

### P4.4.4 — Wizard step 3: palette + mood picker UI

**Source:** `004-phase-4.md` Scene editor flow ("palette → mood").
**Type:** UI
**Depends on:** P4.3.1, P4.2.1 (`PaletteHint`, `MoodHint`).
**Files:** `src/windows/wizard.rs`.

**What:** two rows of large toggle-buttons: three palette choices (Warm / Cool /
Neutral) and three mood choices (Calm / Energetic / Ethereal). The defaults
are taken from `template.palette` and `template.mood`. Selections are stored
in `WizardChoices.palette` and `WizardChoices.mood`.

**Steps:**
1. Render palette row as three wide `egui::Button` toggles; selected button
   uses the warm accent colour.
2. Render mood row identically.
3. Pre-select template defaults on step entry.

**Acceptance:**
- [ ] Both palette and mood rows are rendered.
- [ ] Template defaults are pre-selected on step entry.
- [ ] Selections update `WizardChoices`.
- [ ] `make ci` clean.

---

### P4.4.5 — Wizard step 4: tempo picker UI

**Source:** `004-phase-4.md` Scene editor flow ("tempo").
**Type:** UI
**Depends on:** P4.3.1.
**Files:** `src/windows/wizard.rs`.

**What:** a single row: a labelled checkbox "Sync to project BPM" (pre-ticked
if `template.tempo_sync == true`) and a read-only display of the current
project BPM (from `EditingState`'s BPM clock). The wizard does not change
project BPM; it only records whether the template should be BPM-locked.
Selection is stored in `WizardChoices.tempo_sync`. This is the last step
before "Confirm".

**Steps:**
1. Read the BPM from `SceneWizardState.editing` — the same field the
   `cue_strip.rs` BPM display uses.
2. Render the checkbox + BPM display.
3. Render the "Confirm" button in the footer (wired in P4.3.3).

**Acceptance:**
- [ ] Checkbox pre-ticked according to `template.tempo_sync`.
- [ ] Current BPM displayed (read-only).
- [ ] `WizardChoices.tempo_sync` updated on toggle.
- [ ] `make ci` clean.

---

## Workstream 5 — Built-in scene templates

Eight built-in templates, each a standalone PR. All depend on P4.2.1 (registry
skeleton). Templates that consume `ZoneRole` tags additionally depend on Phase 3
(noted inline). The four-file FX preset pattern does NOT apply — each template
is a `SceneTemplate` struct value plus a unit test.

### P4.5.1 — Built-in template: `window_reveal`

**Source:** `004-phase-4.md` Capability set ("`window reveal`"); usability rule
(3 steps: template → media → zones → done).
**Type:** engine
**Depends on:** P4.2.1; **zone binding depends on Phase 3.**
**Files:** `src/project/scene_templates.rs`.

**What:** register the `window_reveal` template. Recipe:
- `zones_consumed: [ZoneRole::Window]`
- `media_slots: [{ name: "bg", label: "Background image", accepts: [Image, Video] }]`
- `fx_presets_used: [RIPPLE_WASH_PRESET_ID]`
- `palette: Warm`, `mood: Calm`, `tempo_sync: false`
- `description`: "A soft reveal that flows light through tagged window zones."

Instantiation produces: one Image/Video layer (`bg` slot) + one FxLayer
(`mask_edge_ripple_wash`) with default params.

**Acceptance:**
- [ ] `window_reveal` appears in `scene_registry()`.
- [ ] `scene_is_registered("window_reveal")` returns `true`.
- [ ] `instantiate_template` with a valid `WizardChoices` produces a two-layer
      project without panic.
- [ ] `make ci` clean.

---

### P4.5.2 — Built-in template: `pixel_drift`

**Source:** `004-phase-4.md` Capability set ("`pixel drift`").
**Type:** engine
**Depends on:** P4.2.1.
**Files:** `src/project/scene_templates.rs`.

**What:** register the `pixel_drift` template. Recipe:
- `zones_consumed: []` (full-canvas; no zone binding required)
- `media_slots: [{ name: "source", label: "Source media", accepts: [Image, Video] }]`
- `fx_presets_used: ["mask_constrained_drift"]` (P2.5.2 particle preset)
- `palette: Cool`, `mood: Calm`, `tempo_sync: false`
- `description`: "Fine particles drift gently across the source media."

Instantiation produces: one Image/Video layer + one FxLayer
(`mask_constrained_drift`).

**Acceptance:**
- [ ] `pixel_drift` appears in `scene_registry()`.
- [ ] Instantiation produces a two-layer project.
- [ ] `make ci` clean.

---

### P4.5.3 — Built-in template: `collage_bloom`

**Source:** `004-phase-4.md` Capability set ("`collage bloom`").
**Type:** engine
**Depends on:** P4.2.1.
**Files:** `src/project/scene_templates.rs`.

**What:** register the `collage_bloom` template. Recipe:
- `zones_consumed: []`
- `media_slots: [{ name: "slot_a", label: "Image A", accepts: [Image] },
                 { name: "slot_b", label: "Image B", accepts: [Image] },
                 { name: "slot_c", label: "Image C", accepts: [Image] },
                 { name: "slot_d", label: "Image D", accepts: [Image] }]`
- `fx_presets_used: ["mask_edge_emission"]` (P2.5.3 particle preset)
- `palette: Warm`, `mood: Energetic`, `tempo_sync: false`
- `description`: "A four-image collage with particles blooming from the
  edges of each image."

Instantiation produces: one `collage` Treatment layer (P1.3.6) with the four
media slots assigned to its collage slots, plus one FxLayer
(`mask_edge_emission`).

**Acceptance:**
- [ ] `collage_bloom` appears in `scene_registry()`.
- [ ] Instantiation produces a two-layer project (collage + FxLayer).
- [ ] `make ci` clean.

---

### P4.5.4 — Built-in template: `glow_behind_openings`

**Source:** `004-phase-4.md` Capability set ("`glow behind openings`").
**Type:** engine
**Depends on:** P4.2.1; **zone binding depends on Phase 3** (consumes
`ZoneRole::Portal`).
**Files:** `src/project/scene_templates.rs`.

**What:** register the `glow_behind_openings` template. Recipe:
- `zones_consumed: [ZoneRole::Portal]`
- `media_slots: [{ name: "glow_source", label: "Glow source",
                   accepts: [Image, Video] }]`
- `fx_presets_used: ["mask_bounded_fluid"]` (P2.6.2 fluid preset)
- `palette: Warm`, `mood: Ethereal`, `tempo_sync: false`
- `description`: "Fluid light pools in portal zones, evoking glow from
  behind architectural openings."

Instantiation produces: one Image/Video layer + one FxLayer
(`mask_bounded_fluid`).

**Acceptance:**
- [ ] `glow_behind_openings` appears in `scene_registry()`.
- [ ] Instantiation produces a two-layer project.
- [ ] `make ci` clean.

---

### P4.5.5 — Built-in template: `fragmented_portrait`

**Source:** `004-phase-4.md` Capability set ("`fragmented portrait`").
**Type:** engine
**Depends on:** P4.2.1.
**Files:** `src/project/scene_templates.rs`.

**What:** register the `fragmented_portrait` template. Recipe:
- `zones_consumed: []`
- `media_slots: [{ name: "portrait", label: "Portrait image",
                   accepts: [Image] }]`
- `fx_presets_used: ["mask_collision_reflection"]` (P2.5.5 particle preset)
- `palette: Neutral`, `mood: Energetic`, `tempo_sync: false`
- `description`: "A portrait broken into fragments by colliding particles
  at the mask boundary."

Instantiation produces: one Image layer + one FxLayer
(`mask_collision_reflection`).

**Acceptance:**
- [ ] `fragmented_portrait` appears in `scene_registry()`.
- [ ] Instantiation produces a two-layer project.
- [ ] `make ci` clean.

---

### P4.5.6 — Built-in template: `architectural_wash` (upgrade from FX preset label)

**Source:** `004-phase-4.md` Acceptance criteria ("The 'Architectural Wash'
template (already a v3 preset name in the effect chain dropdown) is upgraded
to a full scene template that consumes media + zones, not just a parameter
preset").
**Type:** engine
**Depends on:** P4.2.1; **zone binding depends on Phase 3** (consumes
`ZoneRole::Edge`).
**Files:** `src/project/scene_templates.rs`.

**What:** register the `architectural_wash` template. This is an *upgrade*,
not a new greenfield template. The existing `mask_edge_ripple_wash` FX preset
remains in the FX preset registry — this task layers a scene template on top
that contextualises it with media input and zone binding.

Recipe:
- `zones_consumed: [ZoneRole::Edge]`
- `media_slots: [{ name: "surface", label: "Architectural surface",
                   accepts: [Image, Video] }]`
- `fx_presets_used: [RIPPLE_WASH_PRESET_ID]`
- `palette: Cool`, `mood: Calm`, `tempo_sync: false`
- `description`: "A gentle wave wash that traces the edges of architectural
  surfaces tagged as edge zones. Upgrade of the v3 Architectural Wash preset."
- Note in the module doc: "The underlying FX preset (`mask_edge_ripple_wash`)
  is unchanged; this scene template adds media + zone composition."

Instantiation produces: one Image/Video layer + one FxLayer
(`mask_edge_ripple_wash`).

**Acceptance:**
- [ ] `architectural_wash` appears in `scene_registry()`.
- [ ] The existing `mask_edge_ripple_wash` FX preset is unchanged and still
      registered in `fx_presets.rs`.
- [ ] Instantiation produces a two-layer project.
- [ ] `make ci` clean.

---

### P4.5.7 — Built-in template: `mask_edge_ripple_wash_scene`

**Source:** `004-phase-4.md` Capability set ("`mask-edge ripple wash`").
**Type:** engine
**Depends on:** P4.2.1.
**Files:** `src/project/scene_templates.rs`.

**What:** register `mask_edge_ripple_wash_scene` — the pure FX-only scene
template for the ripple wash preset, without zone binding or media. This is
the simplest on-ramp for operators who just want the effect without the full
architectural-wash narrative. Recipe:
- `zones_consumed: []`
- `media_slots: []` (FX-only; no source media required)
- `fx_presets_used: [RIPPLE_WASH_PRESET_ID]`
- `palette: Neutral`, `mood: Calm`, `tempo_sync: false`
- `description`: "The classic mask-edge ripple wash as a standalone scene.
  No media required."

Instantiation produces: one FxLayer (`mask_edge_ripple_wash`).

**Acceptance:**
- [ ] `mask_edge_ripple_wash_scene` appears in `scene_registry()`.
- [ ] Instantiation produces a one-layer project.
- [ ] `make ci` clean.

---

### P4.5.8 — Built-in template: `light_spill_from_windows`

**Source:** `004-phase-4.md` Capability set ("`light-spill from windows`").
**Type:** engine
**Depends on:** P4.2.1; **zone binding depends on Phase 3** (consumes
`ZoneRole::Window`).
**Files:** `src/project/scene_templates.rs`.

**What:** register the `light_spill_from_windows` template. Recipe:
- `zones_consumed: [ZoneRole::Window]`
- `media_slots: [{ name: "interior", label: "Interior light source",
                   accepts: [Image, Video] }]`
- `fx_presets_used: ["mask_field_flow"]` (P2.5.4 particle preset)
- `palette: Warm`, `mood: Ethereal`, `tempo_sync: false`
- `description`: "Light appears to spill outward from tagged window zones,
  as if an interior source is leaking through the aperture."

Instantiation produces: one Image/Video layer + one FxLayer
(`mask_field_flow`).

**Acceptance:**
- [ ] `light_spill_from_windows` appears in `scene_registry()`.
- [ ] Instantiation produces a two-layer project.
- [ ] `make ci` clean.

---

## Workstream 6 — Scene-aware selected-layer card refactor

Refactors the existing Selected-layer card in the control panel so that when
a layer was produced by a scene template wizard, template parameters appear
above the fold and raw layer parameters are hidden under an "Advanced"
disclosure. Since template identity is not tracked on layers (Anticipated
risk #1), the "above the fold" heuristic is: FxLayer params are displayed as
their FX preset's `FxParamDescriptor` labels (already available from P2.2.2),
while warp, mask, and gamma settings move under "Advanced".

### P4.6.1 — Selected-layer card: scene-aware header (template params above the fold)

**Source:** `004-phase-4.md` UX items resolved ("The canonical Selected-layer
card becomes a scene-aware view — template parameters above the fold, raw layer
parameters under 'Advanced'").
**Type:** UI
**Depends on:** none (pure UI refactor; no new engine types required).
**Files:** `src/windows/control_panel.rs` (selected-layer section),
`src/windows/inspector.rs` (if scene inspector lives here — verify location).

**What:** restructure the selected-layer section so that the FX preset
parameters (for FxLayer kinds) appear first in the card, rendered using
`FxParamDescriptor.label` and `FxParamDescriptor` slider ranges from P2.2.2.
All other controls (blend mode, opacity, mask feather, warp mesh settings,
gamma override) are still present but visually demoted — placed after the
FX preset parameters with a lighter style. This task does NOT add the
"Advanced" disclosure; that is P4.6.2.

**Steps:**
1. Read `src/windows/control_panel.rs` — locate the selected-layer rendering
   function.
2. For `LayerKind::FxLayer` layers: render `FxParamDescriptor` sliders first.
3. For all layer kinds: render the remaining controls after.

**Acceptance:**
- [ ] FxLayer: FX preset param sliders appear before blend mode / opacity /
      warp settings in the selected-layer card.
- [ ] Non-FxLayer layers: card ordering is unchanged from pre-P4.6.1.
- [ ] `make ci` clean.

**Out of scope:** "Advanced" disclosure (P4.6.2).

---

### P4.6.2 — Selected-layer card: "Advanced" disclosure for raw layer params

**Source:** `004-phase-4.md` UX items resolved ("template parameters above
the fold, raw layer parameters under 'Advanced'").
**Type:** UI
**Depends on:** P4.6.1.
**Files:** `src/windows/control_panel.rs`.

**What:** wrap warp mesh settings, mask polygon controls, gamma override, and
edge-blend settings for the selected layer inside a collapsing `CollapsingHeader`
labelled "Advanced". The header is collapsed by default when the layer has a
non-empty FX preset (i.e. the operator arrived via the wizard). It is expanded
by default for layers with no FX preset (e.g. plain Image layers), preserving
the pre-P4.6 experience for existing projects.

**Steps:**
1. Wrap the relevant sections in `egui::CollapsingHeader::new("Advanced")`.
2. Default open state: `!matches!(layer.kind, LayerKind::FxLayer { .. })`.
3. Confirm the collapsed state is persisted across frames via egui's ID system
   (not reset on every draw call).

**Acceptance:**
- [ ] FxLayer selected: "Advanced" is collapsed by default.
- [ ] Non-FxLayer selected: "Advanced" is open by default.
- [ ] Contents of "Advanced" are the same as pre-P4.6 for non-FxLayer layers.
- [ ] `make ci` clean.

---

## Workstream 7 — Capability-availability mode hint banner

### P4.7.1 — Mode hint banner: capability-availability hints

**Source:** `004-phase-4.md` UX items resolved ("I10 capability follow-on —
mode hint banner carries capability-availability hints inline ('Bezier handles
— coming Phase 7', 'Fluid sim — Phase 2 preset')").
**Type:** UI
**Depends on:** P4.3.1 (wizard `AppState` must exist so the banner can display
a hint inside the wizard state too).
**Files:** `src/windows/scene_editor.rs:1145` (`mode_banner` + `mode_banner_copy`).

**What:** extend the existing `mode_banner` function with inline
capability-availability hints. The hints are static copy, not runtime feature
detection. A hint appears only when the current mode or selection makes the
capability relevant:
- In Mask mode with a mask selected: "Bezier handles — Phase 7"
- In Mask mode with no mask: "Fluid sim — Phase 2 preset in the FX layer menu"
- In Scene Wizard (TemplateSelect step): "AI scene generation — not planned
  (pick a template instead)"

Extend `mode_banner_copy` (already `pub`) to accept a `hint_context: HintContext`
parameter, where `HintContext` carries enough state to decide which hint to show
(current mode, current selection, current `AppState` discriminant).

**Steps:**
1. Read `src/windows/scene_editor.rs:1110–1165` — understand `mode_banner_copy`
   and `mode_banner`.
2. Define `pub struct HintContext { pub mode: EditMode, pub has_selection: bool,
   pub in_wizard: bool }`.
3. Add a `hint: Option<&'static str>` return value to `mode_banner_copy` (or a
   separate `pub fn capability_hint(ctx: HintContext) -> Option<&'static str>`).
4. Render the hint as italic grey text below the existing banner copy if
   `Some`.

**Tests:**
- Unit tests for each hint condition via `capability_hint(ctx)`.
- Existing `mode_banner_copy` unit tests must still pass.

**Acceptance:**
- [ ] Hints appear in the mode banner for the documented conditions.
- [ ] No hint shows in default `Editing` mode with no selection.
- [ ] `make ci` clean.

**Out of scope:** dynamic capability detection at runtime (hints are static
copy, not feature flags); adding hints for Phase 5 / Phase 6 capabilities
(those phases own their own hint copy).

---

## Workstream 8 — Snapshot / proptest / determinism

### P4.8.1 — Proptest extension: `SceneTemplate` serde + registry round-trip

**Source:** `004-phase-4.md` acceptance criteria ("Scene templates are
self-contained — each one renders without reaching outside its declared inputs").
**Type:** test
**Depends on:** P4.2.2 (serde round-trip infra).
**Files:** `src/project/scene_templates.rs` (test module).

**What:** extend the proptest harness to cover:
1. `SceneTemplate` serde round-trip: arbitrary `SceneTemplate` values
   (generated via proptest strategies) serialise and deserialise without loss.
2. Registry exhaustiveness: every ID in `scene_registry()` has a unique `id`
   field (no duplicates); every entry returns `true` from `scene_is_registered`.
3. `scene_display_label` returns `Some` for every registered ID.

**Acceptance:**
- [ ] Proptest round-trip: no serde loss for arbitrary `SceneTemplate` values.
- [ ] Registry uniqueness: no duplicate IDs.
- [ ] `scene_display_label` returns `Some` for all registered IDs.
- [ ] `make ci` clean.

---

### P4.8.2 — Proptest extension: wizard commit / cancel `ApplyProjectSnapshot` round-trip

**Source:** `src/project/CLAUDE.md` — "Snapshot Reverse" rule; proptest harness
in `project::command::tests::proptest_round_trip`.
**Type:** test
**Depends on:** P4.3.3 (commit path), P4.3.2 (cancel path).
**Files:** `src/project/command.rs` (proptest module).

**What:** extend the proptest round-trip harness to cover the wizard
commit / cancel paths:
1. Wizard cancel: `ApplyProjectSnapshot { non_undoable: true }` — applying and
   then applying the Reverse restores the original project.
2. Wizard commit: `ApplyProjectSnapshot { non_undoable: false }` — same
   round-trip invariant; additionally verify that `is_non_undoable()` returns
   `false` for the commit variant.

These exercise the same `ApplyProjectSnapshot` path as scene recall (P2.9.1)
but with wizard-generated project JSONs. Generating wizard JSONs in proptest
requires a minimal `instantiate_template` call with fixture data.

**Acceptance:**
- [ ] Proptest covers wizard-cancel `ApplyProjectSnapshot` round-trip.
- [ ] Proptest covers wizard-commit `ApplyProjectSnapshot` round-trip.
- [ ] `is_non_undoable()` returns `false` for commit, `true` for cancel.
- [ ] `make ci` clean.

---

### P4.8.3 — GPU golden: `window_reveal` template renders deterministically

**Source:** `004-phase-4.md` Acceptance criteria ("Scene templates are
self-contained — each one renders without reaching outside its declared
inputs").
**Type:** test (GPU)
**Depends on:** P4.5.1 (`window_reveal` template), P4.2.3 (instantiation);
requires `--features gpu-tests`.
**Files:** `tests/headless_gpu.rs`, `tests/golden/`.

**What:** add a GPU golden test that:
1. Instantiates the `window_reveal` template with a fixture image (same fixture
   used in existing GPU golden tests).
2. Renders one frame through `Renderer`.
3. Compares the output pixel-exactly against a recorded golden.

Mirrors the P2.9.2 GPU determinism test. Run `UPDATE_GOLDEN=1` on a Metal-
backed machine to record the baseline.

**Acceptance:**
- [ ] `window_reveal_renders_deterministically` test exists under
      `--features gpu-tests`.
- [ ] Test passes on Metal; skips when no adapter is available.
- [ ] Golden recorded in `tests/golden/window_reveal_*.png`.
- [ ] `make test-gpu` clean.

---

## Workstream 9 — Release housekeeping + acceptance smoke test

### P4.9.1 — Version bump 0.6 → 0.7

**Source:** Phase 4 is a major feature milestone (scene templates, wizard).
**Type:** housekeeping
**Depends on:** all other workstreams substantially complete.
**Files:** `Cargo.toml`.

**What:** bump `version` from `0.6.x` to `0.7.0` in `Cargo.toml`. Run
`cargo build` to verify the version propagates cleanly.

**Acceptance:**
- [ ] `Cargo.toml` version is `0.7.0`.
- [ ] `cargo build` succeeds.
- [ ] `make ci` clean.

---

### P4.9.2 — CHANGELOG body for v0.7

**Source:** `004-phase-4.md`.
**Type:** docs
**Depends on:** P4.9.1.
**Files:** `CHANGELOG.md`.

**What:** fill the `## [0.7.0]` CHANGELOG section (created as a placeholder
in P4.1.3) with the actual shipped capabilities:
- Scene Templates (eight built-ins, registry, wizard flow)
- Scene Wizard (five steps, cancel/back, commit)
- Scene-aware selected-layer card
- Capability-availability mode hints

**Acceptance:**
- [ ] CHANGELOG body matches shipped capabilities (no placeholder text).
- [ ] `make ci` clean.

---

### P4.9.3 — README — Scene Grammars section

**Source:** `004-phase-4.md`.
**Type:** docs
**Depends on:** P4.9.1.
**Files:** `README.md`.

**What:** expand the "Scene Grammars (v0.7)" stub in README into a paragraph
covering: what scene templates are, how to start the wizard, the eight built-in
templates, and a note on zone binding (requires Phase 3).

**Acceptance:**
- [ ] README "Scene Grammars" section is prose (no placeholder text).
- [ ] Eight built-in template names are listed.
- [ ] `make ci` clean.

---

### P4.9.4 — Show-day checklist: scene template validation, zone-binding audit

**Source:** `specs/roadmap.md` §"Show-day reliability"; `004-phase-4.md`
Acceptance criteria.
**Type:** docs
**Depends on:** W5 and W2 substantially complete.
**Files:** `docs/show-day-checklist.md`.

**What:** add show-day checklist items for Phase 4:
- Before go-live: run project audit and confirm no `TemplateZonesMissing`
  Warn findings (or acknowledge each one).
- Before go-live: confirm all scene template media slots are assigned
  (empty paths produce invisible layers).
- Before go-live with BPM-synced templates: confirm BPM is set and the
  clock is running.

Pattern mirrors P2.10.4.

**Acceptance:**
- [ ] Three new checklist items added under a Phase 4 heading.
- [ ] `make ci` clean.

---

### P4.9.5 — Phase 4 acceptance smoke test (manual)

**Source:** `004-phase-4.md` Acceptance criteria ("A new operator can produce
a coherent immersive scene in under five minutes starting from the launcher").
**Type:** test (manual)
**Depends on:** all W3–W7 workstreams complete; P4.9.1.

**What:** walk through the acceptance criteria as a human operator with a
stopwatch:

1. Launch rmap from the `.app` bundle (no terminal).
2. From the launcher, open a new blank project.
3. Click "New scene from template".
4. Select the `window_reveal` template.
5. Assign one image from the sample asset folder to the background slot.
6. (If Phase 3 is shipped) Tag one mask as `window` zone. (If not: skip.)
7. Click Confirm.
8. Verify a two-layer scene is live on the projector canvas.
9. Verify total time from step 3 to step 8 is under 3 minutes.
10. Press Cmd-Z; verify the canvas returns to the blank project.

Record the stopwatch result in a checklist comment in this spec.

**Acceptance:**
- [ ] Steps 1–8 complete without error or crash.
- [ ] Total wizard time under 3 minutes (spec says 5-minute overall; 3 min
      for the wizard alone is the conservative target).
- [ ] Cmd-Z restores the pre-wizard state.
- [ ] Test result recorded in this spec with the elapsed time.
