# 004 Phase 3 — task breakdown

Companion task spec for [`004-phase-3.md`](004-phase-3.md). Each task
below is sized for a single PR.

## Implementation status

_Implementation status will be populated as PRs land._

---

## Operating model

- **Model:** Sonnet implements; Opus reviews. Same read-the-spec-first rule
  as earlier phases: read the originating spec section, read every CLAUDE.md
  the task touches, write the test alongside the implementation, run
  `make ci` before committing.
- **Pick one task at a time.** Read the source section it references in
  `004-phase-3.md` and the corresponding entry in `specs/roadmap.md` before
  starting.
- **Commit message format:** `004-P3.<workstream>.<task>: <title>` — e.g.
  `004-P3.2.1: ZoneRole enum + WarpMesh.zone_role field`.
- **Branching:** one branch per task; merge straight to `main` once CI is
  green.
- **Pre-commit hook** (`.githooks/pre-commit`, wired by `make setup`) runs
  rustfmt on staged files + `cargo check`. Heavier checks live in `make ci`;
  run that before opening a PR.
- **Tests:** every task ships with new or updated tests. For schema / Mutation
  / snapshot work, follow the v3 proptest pattern in `src/project/command.rs`.
  For render-path work, add a golden under `tests/golden/` (covered by
  `--features gpu-tests`); use `UPDATE_GOLDEN=1` to (re-)record the baseline.
  Where automation isn't possible (manual zone-palette UX, hover tooltip),
  ship a manual smoke-test checklist — never nothing.
- **Read the right CLAUDE.md.** Tasks touching `src/project/` must read
  `src/project/CLAUDE.md` first (Mutation Reverse-storage rules, snapshot
  invariants). Tasks touching `src/render/` must read `src/render/CLAUDE.md`
  first (GPU lifecycle, panic_restore, build-time WGSL validation).
- **Don't bundle.** If a task tempts you to also fix something nearby, resist
  — that "something nearby" probably already has its own task ID below.
- **GPU bring-up tasks ship golden images.** Anything that touches
  `src/render/` and renders pixels needs a `tests/golden/` baseline added
  under `--features gpu-tests`; `UPDATE_GOLDEN=1` rewrites the baseline.
- **Preset architecture mirrors P2.5.x / P2.6.x.** Each zone-consuming FX
  preset is a four-file change: shader (`src/render/shaders/fx_*.wgsl`),
  pipeline constructor, preset-id constant in `src/render/fx_presets.rs`,
  and dispatch arm in the registry function. One PR per preset — no bundling.
- **Zone tag binding contract is BLOCKED until
  `004-phase-3-zone-tag-uniform-decision.md` is signed off.** Tasks W3.1 and
  W3.2 carry this blocker explicitly; W5 tasks wait on W3.

## Task ID conventions

- IDs are flat-numbered within seven workstreams:
  - W1 — Setup + housekeeping (glossary, perf-gate refresh, CHANGELOG/README
    placeholder)
  - W2 — Schema + Mutation foundation (`ZoneRole` enum, `WarpMesh.zone_role`
    field, schema bump v7 → v8, `SetMaskZoneRole` mutation, audit findings)
  - W3 — Zone-tag uniform + shader plumbing (`ZONE_TAG_WGSL` constant,
    bind-group contract doc, `sdf_helper.wgsl` or companion snippet)
  - W4 — Zone authoring UI (combobox/chip palette inside Mask mode, glossary
    tooltip wiring)
  - W5 — Zone-consuming FX presets (one task per preset)
  - W6 — Snapshot / proptest / golden tests
  - W7 — Release housekeeping + Phase 3 acceptance smoke test
- Tasks reference Phase 2 precedents by their task ID where the pattern is
  reused (e.g. "mirrors P2.2.4 audit shape").

## Workstream summary

| WS | Theme | Tasks | Parallel-safe? | Touches |
|----|-------|-------|----------------|---------|
| 1 | Setup + housekeeping | 3 | All three parallel-safe | `src/windows/glossary.rs`, `tests/perf_frame_budget.rs`, `CHANGELOG.md`, `README.md` |
| 2 | Schema + Mutation foundation | 5 | P3.2.1 first; P3.2.2 after; P3.2.3 after P3.2.2; P3.2.4 + P3.2.5 parallel after P3.2.3 | `src/project/schema.rs`, `src/project/migrate.rs`, `src/project/command.rs`, `src/project/audit.rs` |
| 3 | Zone-tag uniform + shader plumbing | 2 | P3.3.1 first; P3.3.2 after; BLOCKED on decision doc | `src/render/sdf.rs`, `src/render/fx_presets.rs` |
| 4 | Zone authoring UI | 2 | P3.4.1 first; P3.4.2 after P3.4.1 + P3.2.1 | `src/windows/control_panel.rs`, `src/windows/glossary.rs` |
| 5 | Zone-consuming FX presets | 3 | All three parallel after W3 lands | new `src/render/shaders/fx_zone_*.wgsl`, `src/render/fx_presets.rs` |
| 6 | Snapshot / proptest / golden tests | 3 | P3.6.1 after W2; P3.6.2 after W3 + W5; P3.6.3 after W2 | `src/project/command.rs`, `tests/headless_gpu.rs`, `tests/` |
| 7 | Release housekeeping + acceptance smoke | 4 | Last — depends on everything else | `Cargo.toml`, `CHANGELOG.md`, `README.md`, `docs/show-day-checklist.md` |

**Suggested PR sequencing:**

1. **P3.1.1 + P3.1.2 + P3.1.3** in parallel — quick independent wins.
2. **P3.2.1** (`ZoneRole` enum + `WarpMesh.zone_role`) — unblocks the rest
   of W2.
3. **P3.2.2** (schema migration v7 → v8) after P3.2.1.
4. **P3.2.3** (`SetMaskZoneRole` mutation) after P3.2.2.
5. **P3.2.4 + P3.2.5** (audit findings) parallel after P3.2.3.
6. Sign off `004-phase-3-zone-tag-uniform-decision.md`, then:
   **P3.3.1** (`ZONE_TAG_WGSL` constant + accessor) — gates W5.
7. **P3.3.2** (bind-group contract doc) after P3.3.1.
8. **P3.4.1** (zone palette in Mask mode) parallel with W3, after P3.2.1 and
   P3.1.1 land.
9. **P3.4.2** (glossary tooltip wiring) after P3.4.1 + P3.1.1.
10. **P3.5.1 + P3.5.2 + P3.5.3** (zone-consuming presets) in parallel after
    P3.3.2.
11. **P3.6.1** (proptest for `SetMaskZoneRole`) after P3.2.3.
    **P3.6.2** (zone-tag golden-image test) after P3.5.x.
    **P3.6.3** (old-project-loads-identically test) after P3.2.2.
12. **P3.7.1 → P3.7.4** last; P3.7.4 runs the acceptance smoke against the
    v0.7 release candidate.

## Anticipated risks

These design decisions are locked — they were approved in the planning
phase. Each is a potential scope-creep site; call it out at task time if
implementation pressure pushes toward a different choice.

1. **Zone tag is metadata, not geometry.** `WarpMesh.mask_polygon` and
   `mask_feather` continue unchanged. The role tag is a new field on
   `WarpMesh`, not a new type. Operators cannot define custom roles at
   runtime; adding a role is a code change (the palette is closed per the
   Usability Rule in `004-phase-3.md`).

2. **Zone tag lives on `WarpMesh`, not on `LayerConfig`.** The Phase 3 plan
   tags the mask (`WarpMesh`) rather than the layer as a whole. This is
   intentional: Phase 4 / 5 may allow a layer to carry multiple masks; the
   role lives with the geometry, not the layer. Do not "simplify" this by
   putting `zone_role` on `LayerConfig`.

3. **Zone UI is a sub-mode inside Mask, not a new top-level pill.** The mode
   pill cluster is not extended in Phase 3. The zone palette appears as a
   secondary control within the existing Mask editing surface. Any change
   that adds a pill violates the UX constraint in `004-phase-3.md`.

4. **Zone-tag uniform is Option B (new binding slot 6).** See
   `004-phase-3-zone-tag-uniform-decision.md`. Do not pack the tag into
   `FxParamsUniform`. Non-zone-aware presets do not bind slot 6; zone-aware
   presets declare it in their layout explicitly.

5. **Schema bump is v7 → v8.** Current `CURRENT_SCHEMA_VERSION = 7`
   (confirmed in `src/project/schema.rs`). The migration adds
   `zone_role: null` to all `warp` objects that lack it. Old v7 projects
   load with `ZoneRole = None` — no behaviour change.

6. **One PR per zone-consuming preset.** Same rule as P2 presets: each is a
   four-file change. Do not bundle the three zone presets into one PR.

7. **Audit findings for zone issues mirror `UnknownFxPreset` /
   `UnknownTreatment` shape.** Two new `AuditKind` variants:
   `UnknownZoneRole { layer_idx, role: String }` (a saved role string not
   in the palette — upgrade protection) and `MissingZoneTag { layer_idx,
   preset_id: String }` (a zone-consuming preset is applied to a layer that
   has `zone_role: None`).

---

## Workstream 1 — Setup + housekeeping

Quick independent wins that ship before the heavier workstreams.

### P3.1.1 — Glossary entries for Phase 3 domain terms

**Source:** `004-phase-3.md` Capability set (zone roles, zone-aware shader,
semantic palette); roadmap §"Zones".
**Type:** docs / UX
**Depends on:** none
**Files:** `src/windows/glossary.rs` (existing `GlossaryTerm` enum).

**What:** Phase 3 introduces semantic zone vocabulary that operators will
encounter in the zone palette, preset browser labels, and audit toasts.
Adding glossary entries now means W4 and W5 tasks can wire
`glossary_label(ui, GlossaryTerm::X)` calls without a separate docs task.
Pattern is identical to P2.1.1 — extend the `GlossaryTerm` enum with new
variants and short (~30 word) operator-facing definitions.

Note that `GlossaryTerm::ZoneTemplate` already exists (v0.4 mask shortcut).
The new zone role variants are distinct: `ZoneTemplate` is a geometry
shortcut; `ZoneRole` terms are semantic tags. Definitions must make this
clear to avoid operator confusion.

**Terms to add (9 total):**
- Seven role terms: `ZoneRoleWindow`, `ZoneRolePortal`, `ZoneRoleVoid`,
  `ZoneRoleSpill`, `ZoneRoleEdge`, `ZoneRoleHighlight`,
  `ZoneRoleLightSource`.
- Two cross-cutting terms: `ZoneAwareShader`, `ZoneTag`.

**Steps:**
1. Read `src/windows/glossary.rs` — locate the `GlossaryTerm` enum,
   `entry()` match, `all_terms()` list, and `EXPECTED_VARIANT_COUNT`
   (currently 63 after Phase 2).
2. Add one enum variant per term listed above (9 new variants).
3. Write a short definition (~30 words) for each in the `entry()` match arm.
   Definitions explain what the operator sees / controls, not the
   implementation. For each role, describe what kind of surface it represents
   and which FX presets respond to it.
4. Add each new variant to `all_terms()`.
5. Bump `EXPECTED_VARIANT_COUNT` from 63 to 72.

**Tests:**
- The existing exhaustiveness test (`test_all_terms_coverage`) covers all new
  variants — it will fail to compile otherwise if the pattern-match is
  exhaustive. Run `make ci` to confirm.
- Manual: hover each new label when it appears in the W4 zone palette and
  W5 preset labels; confirm the popover shows.

**Acceptance:**
- [ ] All nine new `GlossaryTerm` variants have definitions distinguishing
      role terms from the existing `ZoneTemplate` geometry shortcut.
- [ ] `EXPECTED_VARIANT_COUNT` bumped to 72.
- [ ] `all_terms()` includes all nine new variants.
- [ ] Existing exhaustiveness tests still pass.
- [ ] Definitions are operator-facing copy, not implementation notes.
- [ ] `make ci` clean.

**Out of scope:** Phase 4 / 5 zone-graph terms; fixture zone-binding terms.

---

### P3.1.2 — Show-day perf-gate refresh: zone-tagged scene fixture

**Source:** `004-phase-3.md` Acceptance criteria ("Shader dispatch on zone
tag is verified").
**Type:** engine (defensive)
**Depends on:** none
**Files:** `tests/perf_frame_budget.rs`.

**What:** The existing perf gate validates representative scenes against a
p99 frame-time target. Phase 3 needs a new fixture: a scene with a
zone-tagged `FxLayer` at maximum budget to verify that zone-tag dispatch
does not regress the frame budget. This task adds the test function with a
stub fixture (ripple-wash layer with `zone_role = None`) that will be
updated in P3.5.x once real zone-consuming presets land. Same pattern as
P2.1.2.

**Steps:**
1. Read `tests/perf_frame_budget.rs` — understand existing structure and
   skip conditions.
2. Add `perf_zone_tagged_fx_layer_within_budget` that builds a single-layer
   scene with one `FxLayer` (ripple-wash, max amplitude) and asserts p99 ≤
   16.6 ms.
3. Mark with `#[cfg(feature = "gpu-tests")]` and the existing skip condition.
4. Document in a comment that the fixture will be updated in P3.5.x to use
   a zone-consuming preset.
5. Record the M-series baseline in a comment.

**Tests:**
- GPU test (`--features gpu-tests`); skipped when no adapter available.

**Acceptance:**
- [ ] New `perf_zone_tagged_fx_layer_within_budget` test exists under
      `--features gpu-tests`.
- [ ] Baseline M-series result documented in a comment.
- [ ] Test skips cleanly when no GPU adapter is available.
- [ ] `make ci` clean.

**Out of scope:** zone-consuming preset fixture update (P3.5.x).

---

### P3.1.3 — CHANGELOG + README Phase 3 placeholder section

**Source:** `004-phase-3.md` Goal.
**Type:** docs
**Depends on:** none
**Files:** `CHANGELOG.md`, `README.md`.

**What:** Drop a shell section for v0.7 in both files so W7 tasks only fill
body text rather than establish document structure. No version bump yet
(that's P3.7.1). CHANGELOG gets an `[Unreleased] — v0.7` section above the
v0.6 entry. README gets a stub paragraph for spatial zones. Mirrors P2.1.3.

**Steps:**
1. In `CHANGELOG.md`, add an `## [Unreleased] — v0.7` section above v0.6
   with three placeholder subsections: `### Spatial Zones`,
   `### Zone-Consuming FX Presets`, `### Zone Authoring UI`.
2. In `README.md`, add a "Spatial Zones (v0.7)" subsection under the
   Features list with a one-sentence placeholder.
3. Do not change any version strings.

**Tests:**
- No automated tests for documentation files.
- Manual: verify both files render correctly with a Markdown previewer.

**Acceptance:**
- [ ] `CHANGELOG.md` has an `[Unreleased] — v0.7` header with placeholder
      subsections.
- [ ] `README.md` has a stub Spatial Zones entry.
- [ ] No version strings changed.
- [ ] `make ci` clean.

**Out of scope:** filling the CHANGELOG body (P3.7.2); README prose
(P3.7.3); version bump (P3.7.1).

---

## Workstream 2 — Schema + Mutation foundation

Introduces the `ZoneRole` type, extends `WarpMesh`, bumps the schema
version, and adds the `SetMaskZoneRole` mutation with audit findings. Every
subsequent workstream depends on the `ZoneRole` type defined here.

### P3.2.1 — `ZoneRole` enum + `WarpMesh.zone_role` field

**Source:** `004-phase-3.md` Engine implications ("each `Mask` gains an
optional `ZoneRole` tag"); Capability set (seven named roles).
**Type:** schema
**Depends on:** none
**Files:** `src/project/schema.rs`.

**What:** introduce the `ZoneRole` enum with seven variants matching the
closed palette (`Window`, `Portal`, `Void`, `Spill`, `Edge`, `Highlight`,
`LightSource`) and add `zone_role: Option<ZoneRole>` to `WarpMesh` with a
serde default of `None`. Old projects load transparently — the serde default
means any warp object without a `zone_role` key deserialises to `None` with
no migration step. The schema version bump and migration step come in P3.2.2
(because a new *optional* field with a `None` default is technically a
non-breaking serde change, but the project pattern is to bump on any schema
extension so audit tooling can track which version introduced a field).

The `ZoneRole` enum must be `#[derive(Debug, Clone, Copy, PartialEq, Eq,
Serialize, Deserialize)]` with `#[serde(rename_all = "kebab-case")]` so the
saved string matches the plan's identifiers (`"window"`, `"light-source"`,
etc.).

Read `src/project/CLAUDE.md` §"Schema additions" before starting: the serde
default for `zone_role` must round-trip to `None`, not some non-identity
value.

**Steps:**
1. Read `src/project/CLAUDE.md` in full — particularly the serde-default
   rule.
2. Add `pub enum ZoneRole` with seven variants above `WarpMesh` in
   `schema.rs`.
3. Add `#[serde(default)] pub zone_role: Option<ZoneRole>` to `WarpMesh`.
4. Update `WarpMesh::identity()` and `WarpMesh::default_placement()` to
   include `zone_role: None`.

**Tests:**
- Unit test: `WarpMesh::identity()` round-trips through serde with
  `zone_role = None`.
- Unit test: a `WarpMesh` JSON object without a `zone_role` key deserialises
  to `zone_role = None` (regression guard for old projects).
- Unit test: each `ZoneRole` variant serialises to the expected kebab-case
  string (`ZoneRole::LightSource` → `"light-source"`).

**Acceptance:**
- [ ] `ZoneRole` enum exists with all seven variants.
- [ ] `WarpMesh.zone_role: Option<ZoneRole>` with `#[serde(default)]`.
- [ ] `WarpMesh` JSON without `zone_role` key deserialises to `None`.
- [ ] `ZoneRole` serialises to kebab-case strings.
- [ ] `WarpMesh::identity()` and `WarpMesh::default_placement()` updated.
- [ ] `make ci` clean.

**Out of scope:** schema version bump (P3.2.2); `SetMaskZoneRole` mutation
(P3.2.3).

---

### P3.2.2 — Schema migration v7 → v8

**Source:** `004-phase-3.md` Engine implications ("schema migration is
automatic on load").
**Type:** schema
**Depends on:** P3.2.1.
**Files:** `src/project/schema.rs`, `src/project/migrate.rs`.

**What:** bump `CURRENT_SCHEMA_VERSION` from 7 to 8. Add a
`migrate_v7_to_v8_zone_role` step in `migrate.rs` that walks every layer's
`warp` object in the JSON and ensures `zone_role` is present (setting it to
`null` if absent). This step is technically a no-op for well-formed v7
projects (serde already defaults to `None`), but the explicit migration step
means the audit log can report "migrated from v7" and schema tooling in
future phases can reason about when `zone_role` first appeared.

Read `src/project/migrate.rs` — the existing `migrate_v6_to_v7_output_targets`
function is the structural template.

**Steps:**
1. Bump `CURRENT_SCHEMA_VERSION` to 8 in `schema.rs`.
2. Add `migrate_v7_to_v8_zone_role(value: &mut Value)` in `migrate.rs` that
   iterates `value["layers"]` → each layer's `warp` object and inserts
   `zone_role: null` if the key is absent.
3. Wire the new function into the `match schema_version` dispatch in
   `migrate::migrate` (add a `7 => { migrate_v7_to_v8_zone_role(&mut value) }` arm
   then fall-through to the version bump).

**Tests:**
- Unit test: a v7 project JSON loads and migrates cleanly to v8 with
  `zone_role: null` on every warp.
- Unit test: a v8 project JSON (with explicit `zone_role: "window"` on one
  layer) round-trips through `migrate()` unchanged.
- Unit test: `CURRENT_SCHEMA_VERSION == 8` assertion.

**Acceptance:**
- [ ] `CURRENT_SCHEMA_VERSION == 8`.
- [ ] `migrate_v7_to_v8_zone_role` adds `zone_role: null` to all warp objects
      that lack it.
- [ ] Existing v7 golden fixtures in `migrate.rs` tests still load cleanly.
- [ ] `make ci` clean.

**Out of scope:** `SetMaskZoneRole` mutation (P3.2.3).

---

### P3.2.3 — `SetMaskZoneRole` mutation

**Source:** `004-phase-3.md` Engine implications ("New `SetMaskZoneRole`
mutation").
**Type:** schema / mutation
**Depends on:** P3.2.2.
**Files:** `src/project/command.rs`.

**What:** add `Mutation::SetMaskZoneRole` following the exact v3
Reverse-storage pattern. The payload struct `SetMaskZoneRole { layer_idx:
usize, new: Option<ZoneRole>, old: Option<ZoneRole> }` implements
`ReverseStorage`. Read `src/project/CLAUDE.md` §"Whole-enum Reverse" — even
though `Option<ZoneRole>` is small, the Reverse stores the full old
`Option<ZoneRole>` value. The `debug_assert!` in `apply` verifies the
carried `old` matches the project's current `warp.zone_role`.

Add `Project::set_mask_zone_role_mutation(layer_idx, new_role)` constructor
on `Project` (mirroring `set_layer_treatment_params_mutation`) to auto-
capture the pre-mutation state.

**Steps:**
1. Read `src/project/CLAUDE.md` in full.
2. Define `pub struct SetMaskZoneRole { layer_idx, new, old }` and
   `impl ReverseStorage for SetMaskZoneRole`.
3. Add `Mutation::SetMaskZoneRole(SetMaskZoneRole)` to the `Mutation` enum.
4. Add the `apply` arm in `Mutation::apply`: set
   `project.layers[layer_idx].warp.zone_role = new`; `debug_assert!` that
   `old` matches current state.
5. Add `Project::set_mask_zone_role_mutation`.
6. Add the variant to the proptest `Mutation::arbitrary()` distribution (or
   note that P3.6.1 covers this separately if the proptest harness requires
   schema fixtures to be pre-built).

**Tests:**
- Unit test: `SetMaskZoneRole` apply sets the role; reverse restores the
  previous value.
- Unit test: `debug_assert!` fires (in test/debug builds) when `old` is
  stale.

**Acceptance:**
- [ ] `Mutation::SetMaskZoneRole` exists with `ReverseStorage` impl.
- [ ] Apply sets `warp.zone_role`; reverse restores it.
- [ ] `Project::set_mask_zone_role_mutation` captures pre-mutation state.
- [ ] `debug_assert!` on stale Reverse.
- [ ] `make ci` clean.

**Out of scope:** UI dispatch site (P3.4.x); proptest round-trip (P3.6.1).

---

### P3.2.4 — Audit finding: `UnknownZoneRole`

**Source:** `004-phase-3.md` Acceptance ("old projects load identically");
roadmap §"Audit".
**Type:** schema / audit
**Depends on:** P3.2.3.
**Files:** `src/project/audit.rs`.

**What:** add `AuditKind::UnknownZoneRole { layer_idx: usize, role: String }`
to handle the case where a saved project carries a zone role string that is
not in the current `ZoneRole` palette (forward-compat / hand-edited file
protection). Mirrors the `UnknownFxPreset` / `UnknownTreatment` shape in
`audit.rs`. This finding is `Severity::Warn` — the layer renders as if
`zone_role = None`.

The audit walker already iterates layers; add a check after existing mask
checks: deserialise `warp.zone_role` from the raw JSON (not the typed enum)
and emit the finding if the string is non-null and unknown.

**Steps:**
1. Add `UnknownZoneRole { layer_idx: usize, role: String }` to `AuditKind`.
2. In the audit walk, after deserialising `LayerConfig`, inspect the raw
   `zone_role` value: if it is a string that does not map to any `ZoneRole`
   variant, emit the finding.
3. Wire the finding's display message in `AuditFinding::message()` (or
   equivalent display path) — include the `role` string and the `layer_idx`.
4. Add the new variant to the `AuditKind` `Display` match (or message fn).

**Tests:**
- Unit test: a project with `"zone_role": "sky-bridge"` on a layer produces
  exactly one `UnknownZoneRole` finding at the correct `layer_idx`.
- Unit test: a project with `"zone_role": "window"` produces no `UnknownZoneRole`
  finding.

**Acceptance:**
- [ ] `AuditKind::UnknownZoneRole` variant with `Severity::Warn`.
- [ ] Finding fires for unrecognised role strings, not for `None`.
- [ ] Finding message includes the bad role string and layer index.
- [ ] `make ci` clean.

**Out of scope:** `MissingZoneTag` finding (P3.2.5).

---

### P3.2.5 — Audit finding: `MissingZoneTag`

**Source:** `004-phase-3.md` "audit findings: missing zone tag where preset
expects one".
**Type:** schema / audit
**Depends on:** P3.2.3.
**Files:** `src/project/audit.rs`, `src/render/fx_presets.rs`.

**What:** add `AuditKind::MissingZoneTag { layer_idx: usize, preset_id:
String }` for the case where a layer uses a zone-consuming FX preset but its
`warp.zone_role` is `None`. This finding is `Severity::Info` — the layer
will render in its no-zone fallback path. Requires a way to query "is this
preset zone-consuming?" — add `pub fn fx_requires_zone(preset_id: &str) ->
bool` in `src/render/fx_presets.rs` (returns `true` for the three W5
presets; `false` for all others).

**Steps:**
1. Add `pub fn fx_requires_zone(preset_id: &str) -> bool` to
   `src/render/fx_presets.rs` (initially returns `false` for everything;
   W5 tasks will flip their preset to `true` as they land — or this task
   can pre-populate for the three planned preset IDs).
2. Add `AuditKind::MissingZoneTag { layer_idx: usize, preset_id: String }`
   to `AuditKind` in `audit.rs`.
3. In the audit walk, for each `FxLayer`, if `fx_requires_zone(preset_id)`
   and `warp.zone_role.is_none()`, emit the finding.
4. Wire display message.

**Tests:**
- Unit test: a project with a zone-consuming FX preset and `zone_role = None`
  produces exactly one `MissingZoneTag` finding at the correct `layer_idx`.
- Unit test: a project with a zone-consuming preset and `zone_role =
  Some(ZoneRole::Window)` produces no `MissingZoneTag` finding.
- Unit test: `fx_requires_zone("mask_edge_ripple_wash")` returns `false`.

**Acceptance:**
- [ ] `AuditKind::MissingZoneTag` variant with `Severity::Info`.
- [ ] `fx_requires_zone()` exists in `src/render/fx_presets.rs`.
- [ ] Finding fires when zone-consuming preset + no zone tag.
- [ ] `make ci` clean.

**Out of scope:** zone-consuming preset implementation (W5); zone-tag uniform
binding (W3).

---

## Workstream 3 — Zone-tag uniform + shader plumbing

**BLOCKED on `004-phase-3-zone-tag-uniform-decision.md` sign-off.** Both
tasks in this workstream implement Option B from that decision doc. Read it
before starting.

### P3.3.1 — `ZONE_TAG_WGSL` constant + zone-tag accessor

**Source:** `004-phase-3.md` Engine implications ("effect shaders read zone
tags from a per-fragment uniform indexed by layer. Tag dispatch happens
shader-side").
**Type:** render / shader
**Depends on:** P3.2.1. BLOCKED on `004-phase-3-zone-tag-uniform-decision.md`.
**Files:** `src/render/sdf.rs`, new `src/render/shaders/zone_tag_helper.wgsl`
(or appended to `sdf_helper.wgsl` — implementer's choice; document the
choice in a comment).

**What:** add a `ZONE_TAG_WGSL: &str` constant (analogous to `SDF_HELPER_WGSL`)
containing:
1. WGSL constant definitions for each zone role value: `const ZONE_NONE: u32
   = 0u;`, `const ZONE_WINDOW: u32 = 1u;`, …, `const ZONE_LIGHT_SOURCE: u32
   = 7u;`. The u32 values must match the `ZoneRole` enum's discriminant
   order established in P3.2.1 — document this coupling with a comment.
2. A `struct ZoneTagUniform { zone_tag: u32, _pad0: u32, _pad1: u32, _pad2:
   u32 }` (16-byte aligned, wgpu min-binding-size safe).
3. A `@group(0) @binding(6) var<uniform> u_zone: ZoneTagUniform;` declaration
   — **this must only be injected into zone-aware preset shaders** (see W5);
   not into `sdf_helper.wgsl` directly, since non-zone-aware presets do not
   bind slot 6.

The `ZONE_TAG_WGSL` string is prepended to zone-aware preset shaders at
pipeline build time in `fx_presets.rs`, exactly as `SDF_HELPER_WGSL` is
prepended today. Build-time WGSL validation (`build.rs`) will catch
declaration errors.

Also add a Rust enum→u32 conversion: `impl From<ZoneRole> for u32` and
`impl From<Option<ZoneRole>> for u32` (None maps to 0).

**Steps:**
1. Read `src/render/CLAUDE.md` in full — particularly build-time WGSL
   validation.
2. Decide whether `ZONE_TAG_WGSL` lives as a separate
   `zone_tag_helper.wgsl` file (preferred — consistent with how
   `sdf_helper.wgsl` works) or is appended to `sdf_helper.wgsl` (simpler
   but muddies separation). Document in a `// P3.3.1` comment.
3. Write the WGSL snippet with constants, struct, and binding declaration.
4. Add `pub const ZONE_TAG_WGSL: &str = include_str!("shaders/zone_tag_helper.wgsl");`
   to `src/render/sdf.rs`.
5. Add `impl From<ZoneRole> for u32` and `impl From<Option<ZoneRole>> for u32`
   in `src/project/schema.rs` (or in a small `src/render/zone.rs` adapter —
   document the choice).
6. Verify `build.rs` picks up the new `.wgsl` file via `rerun-if-changed`
   (it should already glob `src/render/shaders/*.wgsl`; confirm).

**Tests:**
- Unit test: `u32::from(ZoneRole::Window) == 1` (and the rest of the
  mapping).
- Unit test: `u32::from(None::<ZoneRole>) == 0`.
- Build test: `cargo build` succeeds (naga validates the new WGSL).

**Acceptance:**
- [ ] `ZONE_TAG_WGSL` constant exists and includes zone constants, struct,
      and binding declaration.
- [ ] `From<ZoneRole> for u32` and `From<Option<ZoneRole>> for u32` impls
      exist and are tested.
- [ ] `cargo build` clean (naga validates new shader snippet).
- [ ] `make ci` clean.

**Out of scope:** bind-group contract doc update (P3.3.2); preset shaders
(W5).

---

### P3.3.2 — Bind-group contract: document slot 6 + per-frame write path

**Source:** `004-phase-3.md` Engine implications; `fx_presets.rs` canonical
bind-group slot table.
**Type:** render / engine
**Depends on:** P3.3.1.
**Files:** `src/render/fx_presets.rs`, `src/app.rs` (or wherever per-frame
uniform writes happen).

**What:** update the canonical bind-group slot table in the `fx_presets.rs`
module-level doc to document slot 6 (`ZoneTagUniform`, zone-aware presets
only). Then implement the per-frame write path: for each zone-aware FX layer,
read `layer.warp.zone_role`, convert to `u32`, write the 16-byte
`ZoneTagUniform` buffer via `queue.write_buffer` before the draw call.

Add a wgpu `Buffer` (16 bytes, `UNIFORM | COPY_DST`) per zone-aware FX layer
to `FxPresetPipeline` (or a dedicated `ZoneTagBuffer` wrapper — document the
choice). The buffer is created at layer-add time and updated each frame. If
a non-zone-aware preset is used on the layer, the buffer is `None` and slot
6 is omitted from the bind group.

**Steps:**
1. Read `src/render/CLAUDE.md` — GPU lifecycle and per-frame render-graph
   order.
2. Update the bind-group slot table comment in `fx_presets.rs` to add slot 6.
3. Add `zone_tag_buffer: Option<wgpu::Buffer>` to `FxPresetPipeline` (or
   the appropriate pipeline struct).
4. Initialise the buffer in the pipeline constructor when `fx_requires_zone`
   returns `true` for the preset.
5. In the per-frame render path, write `ZoneTagUniform { zone_tag:
   u32::from(layer.warp.zone_role), … }` into the buffer before the
   draw call.
6. Update `FxFamily` or add a `zone_aware: bool` field on `FxPresetEntry` in
   the registry to drive this branch.

**Tests:**
- Unit test: `FxPresetEntry` for zone-consuming presets (W5) has
  `zone_aware = true` once those presets land; non-zone presets have
  `zone_aware = false`.
- Manual: run a zone-tagged ripple-wash layer in the editor; confirm no
  GPU validation errors in the console (no missing-binding errors).

**Acceptance:**
- [ ] Bind-group slot 6 documented in `fx_presets.rs` module doc.
- [ ] `zone_tag_buffer` created and updated per-frame for zone-aware layers.
- [ ] Non-zone-aware preset layers do not bind slot 6.
- [ ] No `wgpu` validation errors at runtime for zone-tagged layers.
- [ ] `make ci` clean.

**Out of scope:** zone-consuming preset shaders (W5); golden test (P3.6.2).

---

## Workstream 4 — Zone authoring UI

### P3.4.1 — Zone role palette in Mask mode

**Source:** `004-phase-3.md` UX items resolved ("zone selector replaces the
free-form 'tag' the v3 mask model lacks; the small semantic palette is the
sole entry point"); Capability set ("lightweight zone authoring UI on top of
the existing mask + warp system").
**Type:** UI
**Depends on:** P3.2.3 (`SetMaskZoneRole` mutation), P3.1.1 (glossary
terms).
**Files:** `src/windows/control_panel.rs` (Mask mode section).

**What:** add a small zone role palette within the existing Mask editing
surface. The palette is a combobox or a chip cluster of seven role buttons
plus a "None" option. Selecting a role dispatches
`Project::set_mask_zone_role_mutation` via the existing mutation dispatch
path (matching how other Mask controls, such as `SetLayerMaskFeather`,
are dispatched).

The palette must be a sub-mode inside Mask — not a new top-level pill. The
zone controls appear below the existing mask feather slider, before the
polygon vertex list. Every role button wraps its label in a
`glossary_label(ui, GlossaryTerm::ZoneRoleX)` call so the operator sees a
`?` hover popover.

UI constraint: the palette is closed — the UI offers only the seven roles
plus None. There is no free-text input.

**Steps:**
1. Read `src/windows/control_panel.rs` — find the Mask mode section and the
   existing `SetLayerMaskFeather` dispatch pattern.
2. Add a `ui.horizontal` row (or chip cluster) below `mask_feather` with
   an `egui::ComboBox` (or segmented button cluster) for the seven roles +
   None.
3. On selection change, call `ctx.dispatch_mutation(
   project.set_mask_zone_role_mutation(layer_idx, selected_role))`.
4. Wrap each role label with `glossary_label(ui, GlossaryTerm::ZoneRoleX)`.
5. Read the current `zone_role` from `layer.warp.zone_role` to initialise
   the selection state.

**Tests:**
- No automated UI tests (egui UX is manual-only for this task).
- Manual smoke test checklist:
  - [ ] Zone palette renders below the feather slider in Mask mode.
  - [ ] Selecting "Window" updates the displayed selection immediately.
  - [ ] Undo (Cmd-Z) reverts the zone role change.
  - [ ] Redo re-applies it.
  - [ ] Selecting "None" clears the zone role.
  - [ ] Hovering a role chip shows the glossary popover.
  - [ ] No new top-level pill appears.

**Acceptance:**
- [ ] Zone role palette renders as a sub-mode inside Mask (not a new pill).
- [ ] Selecting a role dispatches `SetMaskZoneRole` mutation.
- [ ] Undo / redo works.
- [ ] All seven roles + None are selectable.
- [ ] Each label wraps `glossary_label`.
- [ ] `make ci` clean.

**Out of scope:** zone-consuming preset wiring (W5); glossary tooltip
definitions (P3.1.1 owns those).

---

### P3.4.2 — Zone role reflected in layer list + preset browser label

**Source:** `004-phase-3.md` Acceptance ("An operator can draw a polygon,
tag it `window`, and pick an FX preset whose label says 'from windows'").
**Type:** UI
**Depends on:** P3.4.1, P3.1.1.
**Files:** `src/windows/control_panel.rs` (layer list), `src/windows/preset_browser.rs`.

**What:** two small surface updates that close the authoring loop:

1. In the layer list, append the zone role badge to the layer row when
   `zone_role` is `Some(_)` — e.g. "[window]" in a muted colour. This lets
   the operator see at a glance which layers are zone-tagged without entering
   Mask mode.
2. In the preset browser (P2.8.1), zone-consuming presets display a
   supplemental label "— requires zone tag" in a muted colour so the
   operator knows to tag a mask before applying. Driven by `fx_requires_zone(preset_id)`.

Neither change affects layout beyond the row width; muted colour is the
existing `ui.visuals().weak_text_color()`.

**Steps:**
1. In the layer list row, check `layer.warp.zone_role` and append a short
   badge when `Some`.
2. In `preset_browser.rs`, for each preset entry call `fx_requires_zone` and
   append the supplemental label when `true`.

**Tests:**
- Manual smoke test checklist:
  - [ ] A window-tagged layer shows "[window]" badge in the layer list.
  - [ ] An untagged layer shows no badge.
  - [ ] Zone-consuming presets show "— requires zone tag" in the browser.
  - [ ] Non-zone presets do not show the label.

**Acceptance:**
- [ ] Zone role badge visible in layer list for tagged layers.
- [ ] Preset browser shows zone-requirement label for zone-consuming presets.
- [ ] `make ci` clean.

**Out of scope:** zone-consuming preset behavior (W5).

---

## Workstream 5 — Zone-consuming FX presets

**BLOCKED on P3.3.2 (bind-group contract + per-frame write path).** Each
preset is a four-file change: shader, pipeline constructor, preset-id
constant, dispatch arm — mirroring P2.5.x / P2.6.x exactly. One PR per
preset.

For each preset shader, prepend both `SDF_HELPER_WGSL` and `ZONE_TAG_WGSL`
at pipeline build time. The shader reads `u_zone.zone_tag`, compares to the
role constants from `ZONE_TAG_WGSL`, and adjusts behaviour accordingly. When
`zone_tag == ZONE_NONE`, the preset renders a neutral / identity-ish output
(documented fallback) rather than crashing or producing corruption.

Each preset must set `fx_requires_zone(preset_id)` to return `true` — update
`P3.2.5`'s `fx_requires_zone` fn after the preset's constant is defined.

---

### P3.5.1 — "Light spill from `window` zones" FX preset

**Source:** `004-phase-3.md` Zone-consuming FX presets ("light spill from
`window` zones").
**Type:** render / shader
**Depends on:** P3.3.2, P3.2.5.
**Files:** new `src/render/shaders/fx_zone_light_spill.wgsl`,
`src/render/fx_presets.rs`.

**What:** a fragment-family FX preset that samples the SDF, checks
`u_zone.zone_tag == ZONE_WINDOW`, and renders a warm-glow spill gradient
emanating inward from the mask edge only when the zone tag matches. When
`zone_tag != ZONE_WINDOW` (including `ZONE_NONE`), the shader outputs
transparent black (no-op fallback). Parameters: spill radius (normalised
units), colour temperature (warm/cool bias), intensity.

Register as `"fx_zone_light_spill"` in `fx_registry()` with
`family: FxFamily::Fragment`. Update `fx_requires_zone` to return `true`
for this preset ID.

**Steps:**
1. Read `src/render/CLAUDE.md` — build-time WGSL validation, bind-group
   contract.
2. Write `fx_zone_light_spill.wgsl`: prepend `SDF_HELPER_WGSL` + `ZONE_TAG_WGSL`;
   declare slot 6 (`u_zone`); branch on `u_zone.zone_tag`.
3. Add `LIGHT_SPILL_PRESET_ID: &str = "fx_zone_light_spill"` constant.
4. Add pipeline constructor `FxPresetPipeline::new_light_spill`.
5. Register in `fx_registry()` and `fx_param_descriptors()`.
6. Add dispatch arm in the render path.
7. Flip `fx_requires_zone(LIGHT_SPILL_PRESET_ID)` to `true`.

**Tests:**
- Unit test: `fx_is_registered(LIGHT_SPILL_PRESET_ID)` returns `true`.
- Unit test: `fx_requires_zone(LIGHT_SPILL_PRESET_ID)` returns `true`.
- Unit test: descriptors non-empty; each `min < max`; defaults in range.
- Build test: `cargo build` clean (naga validates new shader).
- Manual: apply preset to a window-tagged layer; confirm glow renders.
  Apply to untagged layer; confirm transparent output (no crash).

**Acceptance:**
- [ ] Preset registered; descriptor table populated.
- [ ] Shader outputs glow for `ZONE_WINDOW` tag; transparent for other tags.
- [ ] `fx_requires_zone` returns `true`.
- [ ] `cargo build` clean.
- [ ] `make ci` clean.

**Out of scope:** golden test (P3.6.2 covers zone-tag dispatch verification).

---

### P3.5.2 — "Ripple at `edge` zones" FX preset

**Source:** `004-phase-3.md` Zone-consuming FX presets ("ripple at `edge`
zones").
**Type:** render / shader
**Depends on:** P3.3.2, P3.2.5.
**Files:** new `src/render/shaders/fx_zone_edge_ripple.wgsl`,
`src/render/fx_presets.rs`.

**What:** a fragment-family FX preset that amplifies the existing
ripple-wash behaviour specifically at `ZONE_EDGE`-tagged regions. When
`zone_tag == ZONE_EDGE`, renders a tighter, higher-frequency ripple
originating from the mask edge. When `zone_tag != ZONE_EDGE` (including
`ZONE_NONE`), outputs transparent black. Parameters: wave frequency,
speed, amplitude.

Register as `"fx_zone_edge_ripple"` with `family: FxFamily::Fragment`.

**Steps:**
1. Write `fx_zone_edge_ripple.wgsl` following the `fx_ripple_wash.wgsl`
   structure; add `ZONE_TAG_WGSL` prefix and `zone_tag == ZONE_EDGE` guard.
2. Add preset constant, pipeline constructor, registry entry, descriptor
   table, dispatch arm.
3. Flip `fx_requires_zone` to `true`.

**Tests:**
- Same structure as P3.5.1 tests with `ZONE_EDGE` semantics.
- Manual: apply to edge-tagged layer; confirm ripple renders. Apply to
  untagged layer; confirm transparent.

**Acceptance:**
- [ ] Preset registered; descriptor table populated.
- [ ] Shader active for `ZONE_EDGE`; transparent otherwise.
- [ ] `fx_requires_zone` returns `true`.
- [ ] `cargo build` clean.
- [ ] `make ci` clean.

---

### P3.5.3 — "Particle drift through `portal` zones" FX preset

**Source:** `004-phase-3.md` Zone-consuming FX presets ("particle drift
through `portal` zones").
**Type:** render / shader (compute-particle family)
**Depends on:** P3.3.2, P3.2.5.
**Files:** new `src/render/shaders/fx_zone_portal_drift.wgsl` (compute),
new `src/render/shaders/fx_zone_portal_drift_fragment.wgsl` (optional
render pass), `src/render/fx_presets.rs`.

**What:** a compute-particle family FX preset (mirrors P2.5.2–P2.5.5 shape)
that constrains particle drift to the `ZONE_PORTAL`-tagged SDF region.
Particles spawn at the mask interior edge and drift through the polygon.
When `zone_tag != ZONE_PORTAL` (including `ZONE_NONE`), the compute shader
emits zero particles (no-op; no visible output). Parameters: particle count
(max-budget-capped), drift speed, spread.

Register as `"fx_zone_portal_drift"` with `family: FxFamily::ComputeParticle`.
Max particle count follows the P2.5.6 budget-enforcement pattern.

**Steps:**
1. Read the Phase 2 particle compute infrastructure in `src/render/fx_compute.rs`
   — internalise the emitter/compute/render pattern.
2. Write the compute shader (`fx_zone_portal_drift.wgsl`): read
   `u_zone.zone_tag`; early-exit with zero velocity if `!= ZONE_PORTAL`.
3. Write (or reuse) the particle render pass fragment shader.
4. Add preset constant, `FxComputePipeline` constructor, registry entry
   (including `max_particle_count`), dispatch arm.
5. Flip `fx_requires_zone` to `true`.
6. Update `perf_frame_budget.rs` stub fixture (P3.1.2) to use this preset.

**Tests:**
- Unit test: `fx_is_registered("fx_zone_portal_drift")` returns `true`.
- Unit test: `fx_requires_zone("fx_zone_portal_drift")` returns `true`.
- Unit test: `fx_param_descriptors` includes a budget-capped `particle_count`
  descriptor.
- Manual: apply to portal-tagged layer; confirm drift particles render.
  Apply to untagged layer; confirm no particles.

**Acceptance:**
- [ ] Preset registered; descriptor includes budget-capped `particle_count`.
- [ ] Compute shader emits no particles for non-portal zone tags.
- [ ] `fx_requires_zone` returns `true`.
- [ ] Perf fixture in P3.1.2 updated.
- [ ] `cargo build` clean.
- [ ] `make ci` clean.

---

## Workstream 6 — Snapshot / proptest / golden tests

### P3.6.1 — Proptest extension: `SetMaskZoneRole` round-trip

**Source:** `004-phase-3.md` Acceptance; `src/project/CLAUDE.md`
§"proptest_round_trip".
**Type:** test / mutation
**Depends on:** P3.2.3.
**Files:** `src/project/command.rs` (proptest harness section).

**What:** extend the existing `proptest_round_trip` harness to generate
`Mutation::SetMaskZoneRole` inputs and verify that apply → reverse returns
the project to its exact prior state. Mirrors how P2.9.1 extended the
harness for `SetFxLayerParams`.

Also verify the `debug_assert!` path: in a non-proptest unit test, construct
a `SetMaskZoneRole` with a deliberately stale `old` value and confirm it
panics in debug mode (matching the existing pattern in `command.rs` tests).

**Steps:**
1. Read the existing proptest strategy in `command.rs` — find
   `Mutation::arbitrary()` or the strategy builder.
2. Add a `SetMaskZoneRole` strategy that picks a random `layer_idx` from
   a generated project and a random `Option<ZoneRole>`.
3. Verify apply → reverse is identity.
4. Add a separate `#[test] fn set_mask_zone_role_stale_reverse_panics`
   (using `#[should_panic]`).

**Tests:**
- Proptest: apply → reverse round-trip for 256 cases.
- Unit test: stale Reverse panics in debug mode.

**Acceptance:**
- [ ] `proptest_round_trip` covers `SetMaskZoneRole`.
- [ ] Stale-Reverse panic test passes.
- [ ] `make ci` clean.

---

### P3.6.2 — GPU golden: zone-tag dispatch verification

**Source:** `004-phase-3.md` Acceptance ("Shader dispatch on zone tag is
verified by a golden-image GPU test (`make test-gpu`)").
**Type:** test / GPU
**Depends on:** P3.5.1, P3.5.2, P3.5.3, P3.3.2.
**Files:** `tests/headless_gpu.rs`, `tests/golden/` (new baseline PNGs).

**What:** add three golden-image GPU tests under `--features gpu-tests`:

1. A layer with `zone_role = Some(ZoneRole::Window)` + light-spill preset →
   golden PNG shows the glow; `zone_role = None` on the same layer → golden
   PNG is transparent black (bit-exact).
2. A layer with `zone_role = Some(ZoneRole::Edge)` + edge-ripple preset →
   golden PNG shows ripple.
3. Verify same-seed particle drift on a portal-tagged layer matches the golden
   (determinism check, mirrors P2.9.2's pattern).

Record baselines with `UPDATE_GOLDEN=1 cargo nextest run --features gpu-tests`.

**Steps:**
1. Read `tests/headless_gpu.rs` — internalise test harness, golden-compare
   utility, `UPDATE_GOLDEN` env-var pattern.
2. Write three test functions following the existing golden structure.
3. Record baseline PNGs with `UPDATE_GOLDEN=1`.

**Tests:**
- GPU tests (`--features gpu-tests`): skip when no adapter available.
- Each test is the deliverable; secondary tests not needed.

**Acceptance:**
- [ ] Three new golden-image tests exist under `--features gpu-tests`.
- [ ] `zone_role = None` on a zone-consuming preset produces transparent
      black output (bit-exact to golden).
- [ ] Particle drift is seed-deterministic (bit-exact to golden).
- [ ] All three tests pass on Metal (M-series baseline).
- [ ] `make test-gpu` clean.

---

### P3.6.3 — "Old project loads identically" regression test

**Source:** `004-phase-3.md` Acceptance ("Old projects without zone tags
load and render identically").
**Type:** test / schema
**Depends on:** P3.2.2.
**Files:** `src/project/migrate.rs` (test section), `tests/` (optional
fixture file).

**What:** add an explicit regression test that a v7 project JSON (without
any `zone_role` key) migrates to v8 and renders all layers identically to
the pre-migration output — specifically: `zone_role = None` on every layer,
`CURRENT_SCHEMA_VERSION == 8` after migration, and no audit findings
attributable to zone roles. This is a CPU-only test (no GPU) — it validates
schema migration and audit pass, not pixel output.

**Steps:**
1. Construct a minimal v7 project JSON fixture (one image layer, one FX
   layer, no `zone_role` keys) in a `const` string or inline JSON.
2. Call `migrate(value)` and assert:
   - `schema_version == 8`.
   - Every `warp.zone_role` is `null`.
   - `AuditRunner::run` produces zero `UnknownZoneRole` or
     `MissingZoneTag` findings for a non-zone-consuming FX layer.

**Tests:**
- Unit test (CPU-only); no GPU required.

**Acceptance:**
- [ ] v7 fixture migrates to v8 with `zone_role: null` on all layers.
- [ ] Audit produces no zone-related findings for non-zone-consuming presets.
- [ ] `make ci` clean.

---

## Workstream 7 — Release housekeeping + Phase 3 acceptance smoke

### P3.7.1 — Version bump 0.6 → 0.7

**Source:** standard release practice.
**Type:** release
**Depends on:** all W1–W6 tasks.
**Files:** `Cargo.toml`.

**What:** bump the `version` field in `Cargo.toml` from `0.6.x` to `0.7.0`.
Verify `cargo build` still passes. Mirrors P2.10.1.

**Steps:**
1. Edit `[package] version` in `Cargo.toml` to `0.7.0`.
2. Run `make build` to confirm.

**Tests:**
- No automated tests; `make build` is the gate.

**Acceptance:**
- [ ] `Cargo.toml` version is `0.7.0`.
- [ ] `make build` clean.

**Out of scope:** CHANGELOG body (P3.7.2); README prose (P3.7.3).

---

### P3.7.2 — CHANGELOG body for v0.7

**Source:** `004-phase-3.md` Goal; Capability set.
**Type:** docs
**Depends on:** P3.7.1.
**Files:** `CHANGELOG.md`.

**What:** fill the `[Unreleased] — v0.7` placeholder (created in P3.1.3)
with operator-facing release notes for Phase 3: spatial zones overview,
seven role descriptions, three new presets, zone palette UI, schema
migration note. Mirrors P2.10.2. Replace the `[Unreleased]` header with
the release date.

**Acceptance:**
- [ ] `CHANGELOG.md` v0.7 section has prose under all three placeholder
      subsections.
- [ ] `[Unreleased]` replaced with `[0.7.0] — YYYY-MM-DD`.
- [ ] `make ci` clean.

---

### P3.7.3 — README — Spatial Zones section

**Source:** `004-phase-3.md` Goal.
**Type:** docs
**Depends on:** P3.7.1.
**Files:** `README.md`.

**What:** fill the stub Spatial Zones entry (P3.1.3) with a brief
operator-facing summary: what zone roles are, how to tag a mask, which FX
presets respond to zone tags. Mirrors P2.10.3.

**Acceptance:**
- [ ] README Spatial Zones entry has prose (2–4 sentences).
- [ ] `make ci` clean.

---

### P3.7.4 — Phase 3 acceptance smoke test (manual)

**Source:** `004-phase-3.md` Acceptance criteria (all four bullet points).
**Type:** test / release
**Depends on:** all other P3 tasks.
**Files:** `docs/show-day-checklist.md`.

**What:** run the four Phase 3 acceptance criteria as a manual smoke test
against the v0.7 release candidate. Document the result in
`docs/show-day-checklist.md` and in the commit message. Mirrors P2.10.5.

**Steps:**
1. Build `make build-release`.
2. Draw a polygon, tag it `window`, apply "light spill from window zones"
   preset — confirm effect binds to the zone without further configuration.
3. Load a v6 project (pre-zone-tags) — confirm it loads and renders
   identically.
4. Verify zone palette is documented in the Glossary window (open Glossary,
   search "window" / "zone").
5. Run `make test-gpu` — confirm zone-tag dispatch golden tests pass.
6. Add a checklist item to `docs/show-day-checklist.md` for zone-tag
   verification: "confirm zone palette visible in Mask mode; apply one
   zone-consuming preset; verify render."

**Tests:**
- Manual — all four acceptance criteria from `004-phase-3.md`.

**Acceptance:**
- [ ] All four acceptance criteria in `004-phase-3.md` verified manually.
- [ ] Show-day checklist updated with zone-tag item.
- [ ] `make test-gpu` passes.
- [ ] `make ci` clean.
