# 004 Phase 7 — task breakdown

Companion task spec for [`004-phase-7.md`](004-phase-7.md). Each task below
is sized for a single PR.

## Implementation status

- [x] P7.0.1 8c2bfd5 — Syphon integration decision (objc2 wrapper, bundled Syphon.framework)
- [x] P7.0.2 8c2bfd5 — Bezier mesh warp decision (cubic Coons patches, CPU tessellation)
- [x] P7.0.3 8c2bfd5 — Calibration file schema decision (separate .rmap-calibration.json)
- [x] P7.0.4 8c2bfd5 — RGBW + CCT mixing decision (CCT-aware white-point subtraction)

---

## Operating model

- **Model:** Sonnet implements; Opus reviews. Same read-the-spec-first rule as
  prior phases: read the originating spec section, read every CLAUDE.md the
  task touches, write the test alongside the implementation, run `make ci`
  before committing.
- **Pick one task at a time.** Read the source section it references in
  `004-phase-7.md` and the corresponding entry in `specs/roadmap.md` before
  starting.
- **Commit message format:** `004-P7.<workstream>.<task>: <title>` — e.g.
  `004-P7.2.1: vendor Syphon.framework + build.rs linkage`.
- **Branching:** one branch per task; merge straight to `main` once CI is
  green.
- **Pre-commit hook** (`.githooks/pre-commit`, wired by `make setup`) runs
  rustfmt on staged files + `cargo check`. Heavier checks live in `make ci`;
  run that before opening a PR.
- **Tests:** every task ships with new or updated tests. For schema / Mutation /
  snapshot work, follow the v3 proptest pattern in `src/project/command.rs`.
  For render-path work, add a golden under `tests/golden/` (covered by
  `--features gpu-tests`); use `UPDATE_GOLDEN=1` to re-record the baseline.
  Where automation is not possible (manual Syphon OBS check, Bezier curved-
  column acceptance), ship a manual smoke-test checklist — never nothing.
- **Read the right CLAUDE.md.** Tasks touching `src/project/` must read
  `src/project/CLAUDE.md` first. Tasks touching `src/render/` must read
  `src/render/CLAUDE.md` first.
- **Don't bundle.** If a task tempts you to also fix something nearby, resist.
- **Decision docs are binding.** W2 (Syphon), W3 (Bezier), W7 (calibration),
  and W9 (RGBW) tasks must implement the decision recorded in their respective
  `004-phase-7-*-decision.md` file, not re-litigate the choice.

## Task ID conventions

- `P7.<workstream>.<task>`, e.g. `P7.2.1`.
- Workstreams:
  - **W0** — Decision docs
  - **W1** — Setup + housekeeping
  - **W2** — Syphon output infrastructure
  - **W3** — Bezier mesh warp
  - **W4** — `MaskGraph` schema migration + inverse mask
  - **W5** — Luma key
  - **W6** — Chroma key
  - **W7** — Calibration file schema + venue/show split
  - **W8** — Calibration verify patterns
  - **W9** — RGBW + colour-temperature mixing
  - **W10** — Scene pack export / import
  - **W11** — Show-day diagnostics refinement
  - **W12** — Snapshot / proptest / acceptance smoke
  - **W13** — Release housekeeping + acceptance smoke

## Workstream summary

| WS  | Theme                            | Tasks | Parallel-safe?                              |
|-----|----------------------------------|-------|---------------------------------------------|
| W0  | Decision docs                    | 4     | All parallel-safe (done before task work)   |
| W1  | Setup + housekeeping             | 3     | All parallel-safe                           |
| W2  | Syphon output                    | 4     | W2.1 first; W2.2 after; W2.3+W2.4 after    |
| W3  | Bezier mesh warp                 | 5     | W3.1 first; W3.2 after; W3.3+W3.4 parallel after W3.2; W3.5 last |
| W4  | MaskGraph + inverse mask         | 3     | W4.1 after W3.1 (schema bump serialised); W4.2+W4.3 parallel after W4.1 |
| W5  | Luma key                         | 2     | W5.1 first; W5.2 after                     |
| W6  | Chroma key                       | 2     | W6.1 first; W6.2 after                     |
| W7  | Calibration file schema          | 4     | W7.1 first; W7.2+W7.3 parallel; W7.4 last  |
| W8  | Calibration verify patterns      | 6     | All parallel-safe after W7.2                |
| W9  | RGBW + CCT mixing                | 3     | W9.1 first; W9.2+W9.3 parallel after       |
| W10 | Scene pack export/import         | 3     | W10.1 first; W10.2+W10.3 parallel after    |
| W11 | Show-day diagnostics refinement  | 2     | Both parallel-safe                          |
| W12 | Snapshot / proptest              | 2     | W12.1 after W3+W4+W9; W12.2 after W2       |
| W13 | Release housekeeping + smoke     | 3     | Last — depends on everything else           |

**Total leaf tasks: 42.**

**Suggested PR sequencing:**

1. **W0 decision docs** (already written — ratify before starting any W2–W13 work).
2. **W1.1 + W1.2 + W1.3** in parallel.
3. **W3.1** (schema v7→v8 migration) and **W7.1** (calibration schema) in
   parallel — both are schema-first tasks that unblock their workstreams.
   **W2.1** (vendor Syphon.framework) also parallel with these.
4. **W4.1** (MaskGraph v8→v9) strictly after W3.1 — both touch
   `CURRENT_SCHEMA_VERSION` and `migrate.rs`; running them in parallel
   causes a merge conflict. W4.1 picks up where W3.1 leaves off (v8→v9).
5. **W3.2** (Bezier tessellation) after W3.1; **W7.2 + W7.3** after W7.1;
   **W4.2 + W4.3** after W4.1; **W2.2** after W2.1.
6. **W3.3 + W3.4** in parallel after W3.2; **W5.1** and **W6.1** after W4.1.
7. **W8.1–W8.6** in parallel after W7.2; **W9.1** after Phase 5 colour-from-
   pixel is confirmed present; **W2.3 + W2.4** after W2.2.
8. **W3.5** (palette scaling UI) after W3.4; **W5.2** after W5.1; **W6.2**
   after W6.1; **W9.2 + W9.3** after W9.1; **W10.1** independent.
9. **W10.2 + W10.3** after W10.1; **W11.1 + W11.2** in parallel.
10. **W12.1 + W12.2** after their respective workstreams land.
11. **W13.1 → W13.3** last, sequentially.

---

## Workstream 0 — Decision docs

These are written before implementation begins. Mark each `[x]` when ratified
by Opus review.

### P7.0.1 — Syphon integration decision

**Source:** `004-phase-7.md` Output / Engine implications.
**Type:** decision record
**Depends on:** none
**Files:** `specs/004-phase-7-syphon-integration-decision.md`

**What:** Decision between Rust Syphon crate, pure IOSurface FFI, and thin
`objc2` wrapper around Syphon.framework. Recommendation: thin `objc2` wrapper
around the bundled Syphon.framework (BSD-licensed, ~800 KB, no existing Rust
crate, pure-IOSurface path skips Syphon discovery protocol).

**Acceptance:**
- [ ] Decision doc written and reviewed by Opus.
- [ ] Recommendation is clear and traceable to constraints.

---

### P7.0.2 — Bezier mesh warp decision

**Source:** `004-phase-7.md` Geometry / Engine implications; M4 follow-on.
**Type:** decision record
**Depends on:** none
**Files:** `specs/004-phase-7-bezier-mesh-decision.md`

**What:** Decision between cubic Bezier patches, B-spline patches, and
Catmull-Clark subdivision. Recommendation: cubic Bezier patches — corners lie
on the surface (calibration ergonomics), degenerate to existing bilinear mesh,
CPU tessellation only.

**Acceptance:**
- [ ] Decision doc written and reviewed by Opus.
- [ ] Migration strategy from `WarpMesh` to `BezierMesh` described.

---

### P7.0.3 — Calibration file schema decision

**Source:** `004-phase-7.md` Calibration / Engine implications; T4.12, T4.13.
**Type:** decision record
**Depends on:** none
**Files:** `specs/004-phase-7-calibration-schema-decision.md`

**What:** Locks the `.rmap-calibration.json` schema, surface-binding model
(surface-slot UUID joins to show file `OutputTarget`), mismatch behavior
(audit warning, identity fallback), and file location.

**Acceptance:**
- [ ] Decision doc written and reviewed by Opus.
- [ ] Runtime binding model (Option A vs. B) clearly decided.
- [ ] Mismatch behaviour locked (soft miss = audit warning, not hard fail).

---

### P7.0.4 — RGBW + colour-temperature mixing decision

**Source:** `004-phase-7.md` Light (refinement).
**Type:** decision record
**Depends on:** none
**Files:** `specs/004-phase-7-rgbw-cct-decision.md`

**What:** Decision between naive `w = min(r,g,b)`, measured spectral mixing,
and CCT-aware white-point subtraction. Recommendation: CCT-aware subtraction
with a per-fixture-group CCT dropdown (2700–6500K).

**Acceptance:**
- [ ] Decision doc written and reviewed by Opus.
- [ ] Schema extension for `RgbwConfig` specified.
- [ ] Backward compatibility with existing RGB-only fixture groups confirmed.

---

## Workstream 1 — Setup + housekeeping

Quick independent wins that ship before the heavier workstreams.

### P7.1.1 — Glossary entries for Phase 7 domain terms

**Source:** `004-phase-7.md` Capability set + Engine implications.
**Type:** docs / UX
**Depends on:** none
**Files:** `src/windows/glossary.rs` (existing `GlossaryTerm` enum).

**What:** Phase 7 introduces new operator-visible terms: *Syphon*, *Syphon
output*, *calibration file*, *surface slot*, *venue calibration*, *Bezier
warp*, *anchor*, *tangent handle*, *inverse mask*, *luma key*, *chroma key*,
*RGBW*, *colour temperature*, *CCT*, *scene pack*, *edge-blend gradient*,
*calibration verify*. Add `GlossaryTerm` variants and ~30-word operator-facing
definitions for each. Bump `EXPECTED_VARIANT_COUNT`. Pattern mirrors P2.1.1.

**Acceptance:**
- [ ] All Phase 7 operator-visible terms have `GlossaryTerm` variants.
- [ ] `EXPECTED_VARIANT_COUNT` bumped to match.
- [ ] Existing exhaustiveness tests pass.
- [ ] `make ci` clean.

---

### P7.1.2 — Perf-gate refresh: Bezier mesh + Syphon stubs

**Source:** `004-phase-7.md` Acceptance criteria; show-day reliability.
**Type:** engine (defensive)
**Depends on:** none (sets baseline; W2 + W3 populate real fixtures later)
**Files:** `tests/perf_frame_budget.rs`.

**What:** Add `perf_bezier_4x4_mesh_within_budget` and
`perf_syphon_publish_overhead_within_budget` test stubs. Each uses a
placeholder fixture (identity `BezierMesh` placeholder; stub Syphon publish
returning `Ok(())`). Real fixture wired by W2.3 and W3.2 respectively. Both
test functions assert p99 ≤ 16.6 ms and skip when no GPU adapter is
available.

**Acceptance:**
- [ ] Two new perf-gate stubs exist under `--features gpu-tests`.
- [ ] Both skip cleanly without a GPU adapter.
- [ ] Baseline M-series result documented in a comment.
- [ ] `make ci` clean.

---

### P7.1.3 — CHANGELOG + README v0.7 placeholders

**Source:** release workflow (mirrors P2.1.3).
**Type:** housekeeping
**Depends on:** none
**Files:** `CHANGELOG.md`, `README.md`.

**What:** Add `## [0.7.0] — unreleased` section header and bulleted
capability placeholders (Syphon output, Bezier warp, inverse/luma/chroma
mask, RGBW, calibration file, scene packs) to `CHANGELOG.md`. Add a
"Coming in v0.7" section stub to `README.md`. Actual text filled by W13.2.

**Acceptance:**
- [ ] `CHANGELOG.md` has a v0.7.0 unreleased section with capability bullets.
- [ ] `README.md` has a "Coming in v0.7" stub section.
- [ ] `make ci` clean.

---

## Workstream 2 — Syphon output infrastructure

Per `004-phase-7-syphon-integration-decision.md`: thin `objc2` wrapper around
the bundled Syphon.framework (BSD-licensed; no maintained Rust crate exists).

### P7.2.1 — Vendor Syphon.framework + build.rs linkage

**Source:** `004-phase-7-syphon-integration-decision.md` W2.1.
**Type:** infrastructure / build
**Depends on:** none
**Files:** `vendor/frameworks/Syphon.framework/` (new), `build.rs`,
`Cargo.toml` (new `syphon-out` feature), `Makefile` (setup hint).

**What:** Fetch the canonical Syphon.framework release from
`https://github.com/Syphon/Syphon-Framework/releases` and check it into
`vendor/frameworks/` (via git lfs or committed binary, whichever the project
already uses for binary assets). Add `cargo:rustc-link-search=framework` and
`cargo:rustc-link-lib=framework=Syphon` emissions to `build.rs`, gated on the
`syphon-out` feature. Add `syphon-out` as a default-on feature in `Cargo.toml`
following the `video`, `audio`, `midi` pattern. `make setup` must verify the
framework blob is present and emit an actionable hint if not.

**Acceptance:**
- [ ] `cargo build --no-default-features` succeeds (no Syphon linkage).
- [ ] `cargo build` (default features) links Syphon.framework on macOS 14+.
- [ ] `make setup` emits a hint if `vendor/frameworks/Syphon.framework/` is
      absent.
- [ ] `cargo bundle --profile release-show` produces a `.app` with
      `Syphon.framework` embedded in `Contents/Frameworks/`.
- [ ] `make ci` clean.

---

### P7.2.2 — `src/syphon_out/` sender wrapper

**Source:** `004-phase-7-syphon-integration-decision.md` W2.2.
**Type:** engine
**Depends on:** P7.2.1
**Files:** new `src/syphon_out/mod.rs`.

**What:** Thin safe wrapper over `SyphonMetalServer` using `objc2`
`extern_class!` + `msg_send!`. Expose:
- `SyphonServer::new(name: &str, device: &wgpu::Device) -> Result<Self>`
- `SyphonServer::publish_frame(texture: &wgpu::Texture)`
- `SyphonServer::stop()`

All three methods are `#[cfg(feature = "syphon-out")]`. Extract the underlying
`MTLTexture` handle from the wgpu texture via
`wgpu::Texture::as_hal::<wgpu::Metal>()`. Error handling: any Objective-C
exception caught via `objc2::exception::catch` returns `Err(SyphonError)`.

**Acceptance:**
- [ ] `SyphonServer::new` announces the server; observable via OBS
      Syphon plugin source list.
- [ ] `SyphonServer::publish_frame` pushes a frame; OBS receives it.
- [ ] `SyphonServer::stop` removes the server from the Syphon source list.
- [ ] Wrapper compiles without `syphon-out` feature (`--no-default-features`).
- [ ] `make ci` clean.

---

### P7.2.3 — Render pipeline integration

**Source:** `004-phase-7-syphon-integration-decision.md` W2.3.
**Type:** engine / render
**Depends on:** P7.2.2
**Files:** `src/render/` (post-gamma pass), `src/app.rs` (`EditingState`),
`src/project/command.rs` (`SetSyphonOut` Mutation).

**What:** After the `GammaPipeline` pass (step 5 in `src/render/CLAUDE.md`),
if a `SyphonServer` is active, call `publish_frame(&warp_rt_texture)`. The
server lives on `EditingState`; toggled by `Mutation::SetSyphonOut { enabled:
bool }` with symmetric `ReverseStorage`. The publish call sits inside the
`panic_restore::run_frame_assert_unwind_safe` boundary. Perf-gate test stub
from P7.1.2 wired to a real fixture.

**Acceptance:**
- [ ] Enabling Syphon out: OBS receives rmap output without colour shift.
- [ ] Disabling Syphon out: OBS source goes grey / drops.
- [ ] A Syphon publish panic is caught by `panic_restore`; the next frame
      renders normally.
- [ ] `Mutation::SetSyphonOut` round-trips through undo/redo.
- [ ] Perf-gate test stub from P7.1.2 wired; p99 ≤ 16.6 ms with Syphon on.
- [ ] `make ci` clean.

---

### P7.2.4 — Output panel Syphon UI + audit

**Source:** `004-phase-7-syphon-integration-decision.md` W2.4.
**Type:** UI / UX
**Depends on:** P7.2.3
**Files:** `src/windows/output_panel.rs` (or equivalent), `src/project/audit.rs`.

**What:** Add "Syphon out" toggle + status label (advertising name
= `rmap – <project filename>`) to the Output panel. Add
`AuditKind::SyphonFrameworkMissing` raised at startup if the framework
cannot be loaded (covers corrupted bundles). Glossary wiring:
`GlossaryTerm::SyphonOutput` tooltip on the toggle label.

**Acceptance:**
- [ ] Toggle visible in Output panel; enables/disables live Syphon publishing.
- [ ] Advertising name shown when active.
- [ ] `AuditKind::SyphonFrameworkMissing` surfaces in audit panel on
      framework load failure.
- [ ] Glossary popover appears on hover.
- [ ] `make ci` clean.

---

## Workstream 3 — Bezier mesh warp

Per `004-phase-7-bezier-mesh-decision.md`: cubic Bezier patches, CPU
tessellation, degenerate-handles migration from `WarpMesh`.

### P7.3.1 — Schema v7→v8 migration + `BezierMesh` data model

**Source:** `004-phase-7-bezier-mesh-decision.md` W3.1; `src/project/CLAUDE.md`.
**Type:** schema / migration
**Depends on:** none
**Files:** `src/project/schema.rs`, `src/project/migrate.rs`,
`src/project/command.rs`.

**What:** Add `BezierMesh { rows, cols, anchors, handles, mask_polygon,
mask_feather }` to `src/project/schema.rs`. Keep `WarpMesh` deserializable
but deprecated. Add `migrate_v7_to_v8` to `migrate.rs` converting
`WarpMesh → BezierMesh` with all handles `None`. Bump
`CURRENT_SCHEMA_VERSION` to 8. Add `Mutation::ResetLayerBezierMesh` with
symmetric `ReverseStorage`. Proptest: `BezierMesh::from_warp_mesh(old)` round-
trips losslessly (vertex buffer output numerically identical for `None`-handle
meshes).

**Acceptance:**
- [ ] `CURRENT_SCHEMA_VERSION` = 8.
- [ ] `migrate_v7_to_v8` present; old v7 projects load without audit error.
- [ ] `BezierMesh` with all-`None` handles produces pixel-identical vertex
      buffer to the original `WarpMesh` bilinear calculation.
- [ ] Proptest round-trip passes.
- [ ] `make ci` clean.

---

### P7.3.2 — CPU tessellation for `BezierMesh`

**Source:** `004-phase-7-bezier-mesh-decision.md` W3.2; `src/render/CLAUDE.md`.
**Type:** engine / render
**Depends on:** P7.3.1
**Files:** `src/render/warp.rs`.

**What:** Add `build_bezier_vertices(mesh: &BezierMesh, sub: u32)` in
`warp.rs`. Evaluates a Coons patch at `sub×sub` parameter points per cell.
Each cell has four cubic Bézier edge curves (two anchors + up to two handles
per edge); the interior is Coons-blended from those edge curves. `None`
handles produce straight-line edges (bilinear-equivalent). Outputs the same
`WarpVertex` buffer format as the existing bilinear path; GPU pipeline
unchanged. Wire `WarpRenderer` to dispatch on the mesh type (bilinear for
legacy `WarpMesh`, Coons for `BezierMesh`). Golden test under
`--features gpu-tests`: all-`None`-handle `BezierMesh` vs. equivalent
`WarpMesh` — must be pixel-identical.

**Acceptance:**
- [ ] `build_bezier_vertices` present in `warp.rs`.
- [ ] GPU golden: all-`None`-handle `BezierMesh` = `WarpMesh` pixel-for-pixel.
- [ ] naga WGSL validation passes (no shader changes, but `build.rs` still
      runs on recompile — confirm no regressions).
- [ ] `make ci` clean.

---

### P7.3.3 — Bezier control-point hit-testing

**Source:** `004-phase-7-bezier-mesh-decision.md` W3.3; N1 follow-on.
**Type:** engine / UX
**Depends on:** P7.3.2
**Files:** `src/windows/scene_editor.rs`, `src/project/command.rs`.

**What:** Extend the warp-vertex picker with two-tier hit-testing: anchor-
hit radius takes priority; handle-hit radius activates only when the anchor
is selected. Add `Mutation::SetBezierHandle { layer_id, anchor_row,
anchor_col, direction, new_pos, old_pos }` and `Mutation::MoveBezierAnchor
{ layer_id, row, col, new_pos, old_pos }`, both with symmetric
`ReverseStorage`. Anchor drag propagates handles rigidly.

**Acceptance:**
- [ ] Clicking an unselected anchor selects it; clicking a handle on an
      unselected anchor does nothing.
- [ ] Dragging an anchor moves it and its handles rigidly; undo restores
      exact `old_pos`.
- [ ] Dragging a handle updates only that handle; undo restores it.
- [ ] Both Mutations round-trip through proptest.
- [ ] `make ci` clean.

---

### P7.3.4 — Handle visual overlay

**Source:** `004-phase-7-bezier-mesh-decision.md` W3.4; `src/render/CLAUDE.md`.
**Type:** UI / render
**Depends on:** P7.3.3
**Files:** `src/render/` (overlay pipeline), `tests/golden/`.

**What:** When an anchor is selected, render its two handles as hollow diamonds
connected to the anchor by thin lines in the `OverlayPipeline` pass. Handles
invisible when anchor is not selected. `O` overlay toggle hides handles with
all other editor chrome. Golden: `tests/golden/bezier_handles_overlay.png`.

**Acceptance:**
- [ ] Handles appear as hollow diamonds on selected anchor; invisible otherwise.
- [ ] Thin lines connect handle diamonds to anchor.
- [ ] `O` toggle hides handles cleanly.
- [ ] GPU golden present under `--features gpu-tests`.
- [ ] `make ci` clean.

---

### P7.3.5 — UI palette scaling (I11 follow-on)

**Source:** `004-phase-7.md` UX items / I11 follow-on; M4 follow-on.
**Type:** UI / UX
**Depends on:** P7.3.4
**Files:** `src/windows/scene_editor.rs` (warp mode pill / sub-row).

**What:** Add a sub-row to the Warp mode pill:
`[Anchor] [Handle] [Tangent]`. Active state governs whether drag moves the
anchor or a handle. "Tangent" mode: moving one handle of a C1 pair mirrors
the opposite handle symmetrically (smooth); Shift breaks symmetry (cusp).
Palette tab count rises to ~5 total; review `specs/roadmap.md` §I11 before
finalising labels.

**Acceptance:**
- [ ] Sub-row renders in the warp mode pill.
- [ ] Anchor mode drag = anchor move; Handle mode drag = handle move.
- [ ] Tangent mode: smooth mirror by default, Shift for cusp.
- [ ] Palette remains at ≤5 visible modes (no scrolling).
- [ ] `make ci` clean.

---

## Workstream 4 — `MaskGraph` schema migration + inverse mask

### P7.4.1 — `MaskGraph` data model + schema migration

**Source:** `004-phase-7.md` Mask / Engine implications; `src/project/CLAUDE.md`.
**Type:** schema / migration
**Depends on:** P7.3.1 (must land after the v7→v8 bump so this adds v8→v9,
not a conflicting v7→v8)
**Files:** `src/project/schema.rs`, `src/project/migrate.rs`,
`src/render/sdf.rs`.

**What:** Define `MaskGraph` as a new mask representation: a list of nodes,
each with a `MaskNode` kind (`Polygon { points, feather }`, `Inverse { of:
NodeId }`, `Union`, `Subtract`). The `mask_polygon` / `mask_feather` fields
on `BezierMesh` (introduced by P7.3.1) are superseded by a top-level
`MaskGraph` field on `LayerConfig`. Migration `v8→v9`: convert each layer's
`BezierMesh.mask_polygon` + `mask_feather` into a `MaskGraph` with one
`Polygon` node; remove those fields from `BezierMesh`. Old projects cascade
through v7→v8 then v8→v9 cleanly. Schema version bump to 9.
`MaskGraph::identity` = single `Polygon` node with empty points (full canvas,
no mask).

**Acceptance:**
- [ ] `CURRENT_SCHEMA_VERSION` = 9.
- [ ] `migrate_v8_to_v9` converts `mask_polygon` + `mask_feather` to a single-
      node `MaskGraph`; renders identically.
- [ ] Proptest: single-node `MaskGraph` round-trips losslessly.
- [ ] Old projects (v7 and v8) load cleanly via cascaded migration.
- [ ] `make ci` clean.

---

### P7.4.2 — `MaskGraph` SDF evaluation

**Source:** `004-phase-7.md` Mask; `src/render/sdf.rs`.
**Type:** engine / render
**Depends on:** P7.4.1
**Files:** `src/render/sdf.rs`, possibly `src/render/shaders/sdf_helper.wgsl`.

**What:** Extend the CPU SDF baker to evaluate a `MaskGraph`. For a single-
node `Polygon` graph, the output is pixel-identical to the existing
`bake_polygon_sdf`. For `Inverse`: negate the SDF (inside ↔ outside).
`Union` / `Subtract` compose two SDFs via `min` / `max` (signed). Golden
test: inverse of the existing single-node golden = pixel-inverted SDF field.

**Acceptance:**
- [ ] Single-node `MaskGraph` produces pixel-identical SDF to `bake_polygon_sdf`.
- [ ] `Inverse` node produces negated SDF.
- [ ] GPU golden for `Inverse` under `--features gpu-tests`.
- [ ] `make ci` clean.

---

### P7.4.3 — Inverse mask UI

**Source:** `004-phase-7.md` Mask / M8 capability follow-on.
**Type:** UI / UX
**Depends on:** P7.4.2
**Files:** `src/windows/scene_editor.rs` (Mask mode pill sub-row),
`src/project/command.rs`.

**What:** Add "Inverse" toggle to the Mask mode sub-row (peers to the polygon
mode). `Mutation::SetMaskInverse { layer_id, enabled, was_enabled }` with
symmetric `ReverseStorage`. When enabled, the `MaskGraph` wraps the existing
polygon node in an `Inverse` node. Display: "Inverse" pill button highlighted
when active. Accessible from the Mask mode pill sub-row, not buried in
Advanced, per the Phase 7 acceptance criteria.

**Acceptance:**
- [ ] "Inverse" toggle visible in Mask mode sub-row.
- [ ] Enabling inverse: masked area becomes unmasked and vice versa (visual
      inversion of the rendered output).
- [ ] Undo/redo of inverse toggle restores exact previous state.
- [ ] `make ci` clean.

---

## Workstream 5 — Luma key

### P7.5.1 — Luma key SDF / alpha computation

**Source:** `004-phase-7.md` Mask / M8 follow-on.
**Type:** engine / render
**Depends on:** P7.4.1 (MaskGraph infrastructure)
**Files:** `src/render/sdf.rs` or new `src/render/luma_key.rs`,
`src/project/schema.rs`.

**What:** Add a `LumaKey` mask-graph node kind: given the layer's rendered
output, compute a luminance-based alpha. Control: `threshold: f32`,
`softness: f32`. CPU SDF side produces a GPU texture from the render output's
luma channel (samples rendered pixels, not the source asset). Schema: add
`MaskNode::LumaKey { threshold, softness }`. `Mutation::SetLumaKey { ... }`
with symmetric `ReverseStorage`.

**Acceptance:**
- [ ] `LumaKey` node in `MaskGraph` compiles and produces correct alpha.
- [ ] Bright regions of a test gradient become opaque; dark regions become
      transparent at threshold 0.5.
- [ ] `Mutation::SetLumaKey` round-trips through proptest.
- [ ] `make ci` clean.

---

### P7.5.2 — Luma key UI

**Source:** `004-phase-7.md` Mask / M8 follow-on.
**Type:** UI / UX
**Depends on:** P7.5.1
**Files:** `src/windows/scene_editor.rs` (Mask mode sub-row).

**What:** Add "Luma key" button to the Mask mode sub-row (peer to Inverse).
When active, show threshold + softness sliders in the mask inspector panel.
`Mutation::SetLumaKey` dispatched on slider drag. Accessible from the Mask
mode sub-row, not buried in Advanced (Phase 7 acceptance criteria).

**Acceptance:**
- [ ] "Luma" button visible in Mask mode sub-row; activates luma key mode.
- [ ] Threshold and softness sliders appear when luma mode is active.
- [ ] Sliders dispatch `SetLumaKey`; undo/redo works.
- [ ] `make ci` clean.

---

## Workstream 6 — Chroma key

### P7.6.1 — Chroma key computation

**Source:** `004-phase-7.md` Mask / M8 follow-on.
**Type:** engine / render
**Depends on:** P7.4.1
**Files:** `src/render/` (new `chroma_key.rs` or extension of luma_key),
`src/project/schema.rs`.

**What:** Add `MaskNode::ChromaKey { hue_center_deg: f32, hue_range_deg: f32,
saturation_threshold: f32, softness: f32 }`. Converts rendered pixels to HSV;
pixels whose hue falls within `hue_center ± hue_range` and saturation exceeds
threshold become transparent (alpha → 0). Softness controls edge falloff.
`Mutation::SetChromaKey { ... }` with symmetric `ReverseStorage`. Green-screen
(120°) as the default hue center.

**Acceptance:**
- [ ] `ChromaKey` node produces correct alpha for a green test card.
- [ ] `Mutation::SetChromaKey` round-trips through proptest.
- [ ] `make ci` clean.

---

### P7.6.2 — Chroma key UI

**Source:** `004-phase-7.md` Mask / M8 follow-on.
**Type:** UI / UX
**Depends on:** P7.6.1
**Files:** `src/windows/scene_editor.rs`.

**What:** Add "Chroma" button to the Mask mode sub-row (peer to Inverse and
Luma). When active, show hue-picker + range + saturation-threshold + softness
controls in the mask inspector. Accessible from the Mask mode sub-row.

**Acceptance:**
- [ ] "Chroma" button visible in Mask mode sub-row.
- [ ] Hue picker, range, saturation threshold, softness controls appear.
- [ ] Sliders dispatch `SetChromaKey`; undo/redo works.
- [ ] `make ci` clean.

---

## Workstream 7 — Calibration file schema + venue/show split

Per `004-phase-7-calibration-schema-decision.md`.

### P7.7.1 — `CalibrationFile` schema + save/load

**Source:** `004-phase-7-calibration-schema-decision.md`.
**Type:** schema / persistence
**Depends on:** none
**Files:** new `src/calibration/mod.rs`, `src/calibration/schema.rs`.

**What:** Define `CalibrationFile { schema_version: 1, calibration_id: Uuid,
venue_name: String, created_at: DateTime, surfaces: Vec<CalibrationSurface> }`.
Each `CalibrationSurface` holds `surface_slot_id: Uuid`, `output_target:
OutputTarget`, `warp: WarpOrBezier` (an enum accepting both `WarpMesh` and
`BezierMesh` so this task can ship before or in parallel with P7.3.1 — once
P7.3.1 lands, a follow-up PR in W7.4 tightens the type to `BezierMesh`),
`gamma_matrix: [[f32;3];3]`, `brightness: f32`, `contrast: f32`. Atomic
save (temp + rename). Load returns
`Result<CalibrationFile, CalibrationLoadError>` — never panics.

**Acceptance:**
- [ ] `CalibrationFile` serialises to / deserialises from `.rmap-calibration.json`.
- [ ] Atomic save: temp file + `rename` (same pattern as `Project::save`).
- [ ] `calibration_id` is a UUID stable across saves (not regenerated on load).
- [ ] `make ci` clean.

---

### P7.7.2 — Runtime binding + audit

**Source:** `004-phase-7-calibration-schema-decision.md` Runtime binding.
**Type:** engine / audit
**Depends on:** P7.7.1
**Files:** `src/app.rs` (`EditingState`), `src/project/audit.rs`.

**What:** Add `loaded_calibration: Option<CalibrationFile>` to `EditingState`.
On "Load Calibration" action: for each `CalibrationSurface`, find the matching
`OutputTarget` by `surface_slot_id`; apply warp/mask/gamma to the runtime
state (not to the persisted show file). Emit
`AuditKind::CalibrationSurfaceUnmatched { slot_id, display_name }` for
unmatched surfaces. Loading no calibration = identity warp/mask/gamma = no
audit warning.

**Acceptance:**
- [ ] Loading a calibration applies its warp/mask/gamma to the runtime output.
- [ ] Show file on disk unchanged by loading a calibration.
- [ ] `AuditKind::CalibrationSurfaceUnmatched` appears in audit panel for
      unmatched surfaces.
- [ ] No calibration loaded = identity; no crash; no spurious audit warning.
- [ ] `make ci` clean.

---

### P7.7.3 — Same-directory auto-load offer

**Source:** `004-phase-7-calibration-schema-decision.md` File location.
**Type:** UX
**Depends on:** P7.7.1
**Files:** `src/app.rs` (project load path), `src/windows/output_panel.rs`.

**What:** After project open, check for
`<show-file-stem>.rmap-calibration.json` in the same directory. If found,
show a non-blocking toast in the Output panel: "Venue calibration found —
load?" Two buttons: "Load" and "Dismiss". The toast does not block the editor.
Loading follows the P7.7.2 binding path.

**Acceptance:**
- [ ] Toast appears when a same-directory calibration exists.
- [ ] "Load" triggers P7.7.2 binding; "Dismiss" removes the toast.
- [ ] No toast when no matching file exists.
- [ ] Toast does not block the editor (non-modal).
- [ ] `make ci` clean.

---

### P7.7.4 — File > Save / Load Calibration menu actions

**Source:** `004-phase-7-calibration-schema-decision.md`; macOS menu bar.
**Type:** UI / UX
**Depends on:** P7.7.1, P7.7.2
**Files:** macOS menu bar module (native menu bar from v3.1 W4), output panel.

**What:** Add "File > Load Calibration…" (open panel, filter `.rmap-
calibration.json`) and "File > Save Calibration…" (save panel, default name
`<venue-name>.rmap-calibration.json`). Save: collects current warp/mask/gamma
from all `OutputTarget`s into a `CalibrationFile` and writes atomically.

**Acceptance:**
- [ ] "Load Calibration…" opens a native open dialog; loads and applies.
- [ ] "Save Calibration…" opens a native save dialog; writes atomically.
- [ ] Saved file round-trips cleanly through "Load Calibration…".
- [ ] `make ci` clean.

---

## Workstream 8 — Calibration verify patterns

Each is a separate, independently shippable verify pattern rendered as a
full-screen or partial overlay in the Output panel. All depend on P7.7.2
(calibration infrastructure). All are UI-only tasks (no new shaders unless
noted; WGSL must pass naga validation if added).

### P7.8.1 — Alignment cross verify pattern

**Source:** `004-phase-7.md` Calibration verify.
**Type:** UI / render
**Depends on:** P7.7.2
**Files:** `src/windows/output_panel.rs`, possibly a new WGSL snippet.

**What:** Full-screen overlay of a cross-hair (centre cross, four corner marks,
rule-of-thirds grid optional). Activatable from the Output panel "Verify"
section. Rendered via the `OverlayPipeline` pass (not a full-screen warp pass).
Colour configurable (white/black).

**Acceptance:**
- [ ] Alignment cross visible on the projector output when active.
- [ ] Deactivating returns to normal output instantly.
- [ ] `make ci` clean.

---

### P7.8.2 — Dot grid verify pattern

**Source:** `004-phase-7.md` Calibration verify.
**Type:** UI / render
**Depends on:** P7.7.2
**Files:** `src/windows/output_panel.rs`.

**What:** Regular dot grid overlay (configurable dot spacing in percentage of
output height, e.g. 5% / 10%). Each dot is a small filled circle; the grid
covers the full projector output. Used to verify geometric accuracy across the
entire surface.

**Acceptance:**
- [ ] Dot grid visible on projector output when active.
- [ ] Dot spacing configurable via Output panel spinner.
- [ ] `make ci` clean.

---

### P7.8.3 — Colour bars verify pattern

**Source:** `004-phase-7.md` Calibration verify.
**Type:** UI / render
**Depends on:** P7.7.2
**Files:** `src/windows/output_panel.rs`.

**What:** SMPTE-style or simplified colour bars (White, Yellow, Cyan, Green,
Magenta, Red, Blue, Black). Used to verify colour accuracy and projector
colour profile. Rendered as a set of full-height vertical rectangles.

**Acceptance:**
- [ ] Colour bars rendered on projector output when active.
- [ ] Colours match sRGB primaries + secondaries (no colour shift vs. reference).
- [ ] `make ci` clean.

---

### P7.8.4 — Edge-blend gradient verify pattern

**Source:** `004-phase-7.md` Calibration verify.
**Type:** UI / render
**Depends on:** P7.7.2
**Files:** `src/windows/output_panel.rs`.

**What:** Adjustable linear gradient overlaid on one or more screen edges.
Used to verify and adjust edge-blend overlap regions for multi-projector
setups. Configurable edge (left/right/top/bottom), blend width (0–50% of
output extent), and gamma curve (linear / gamma-2.2).

**Acceptance:**
- [ ] Edge-blend gradient visible on the configured edge.
- [ ] Edge, blend width, and gamma controls available.
- [ ] `make ci` clean.

---

### P7.8.5 — Focus chart verify pattern

**Source:** `004-phase-7.md` Calibration verify.
**Type:** UI / render
**Depends on:** P7.7.2
**Files:** `src/windows/output_panel.rs`.

**What:** Full-screen focus / sharpness chart: concentric circles, radiating
lines from corners, a Siemens star at centre, and fine text at several sizes.
Used to verify projector focus uniformly across the throw distance.

**Acceptance:**
- [ ] Focus chart rendered on projector output when active.
- [ ] All elements (circles, lines, Siemens star, text) are crisp at native
      projector resolution.
- [ ] `make ci` clean.

---

### P7.8.6 — Geometry verify pattern

**Source:** `004-phase-7.md` Calibration verify.
**Type:** UI / render
**Depends on:** P7.7.2
**Files:** `src/windows/output_panel.rs`.

**What:** Full-screen geometry check pattern: checkerboard of equal-area cells
(configurable count, e.g. 8×5), with bold outer border and cell diagonal lines
(for keystone / pillow detection). Used to verify warp accuracy on the physical
surface.

**Acceptance:**
- [ ] Geometry grid rendered on projector output when active.
- [ ] Cell count configurable.
- [ ] `make ci` clean.

---

## Workstream 9 — RGBW + colour-temperature mixing

Per `004-phase-7-rgbw-cct-decision.md`: CCT-aware white-point subtraction.

### P7.9.1 — `RgbwConfig` schema + CCT conversion table

**Source:** `004-phase-7-rgbw-cct-decision.md`.
**Type:** schema / engine
**Depends on:** none (Phase 5 colour-from-pixel must be landed)
**Files:** `src/project/schema.rs`, new `src/lighting/rgbw.rs`.

**What:** Add `RgbwConfig { enabled: bool, w_channel_cct_k: u16, w_scale:
f32 }` to the fixture-group or per-output-target colour config (whichever
Phase 5 landed). Compile-in a static CCT-to-RGB table (Planckian locus
approximation, Kang et al. 2002, sampled at 100 K steps 2000–8000 K). Add
`cct_to_rgb(k: u16) -> [f32; 3]` lookup. `Mutation::SetRgbwConfig { ... }`
with symmetric `ReverseStorage`. Proptest: `RgbwConfig` round-trips through
save/load.

**Acceptance:**
- [ ] `RgbwConfig` serialises / deserialises correctly.
- [ ] `cct_to_rgb(6500)` = approximately `[1.0, 1.0, 1.0]` (neutral white).
- [ ] `cct_to_rgb(2700)` ≈ `[1.0, 0.82, 0.55]` (warm white).
- [ ] `Mutation::SetRgbwConfig` round-trips through proptest.
- [ ] `make ci` clean.

---

### P7.9.2 — RGBW DMX output path

**Source:** `004-phase-7-rgbw-cct-decision.md`.
**Type:** engine
**Depends on:** P7.9.1
**Files:** `src/lighting/` (Phase 5 colour-from-pixel sampling stage).

**What:** When `RgbwConfig::enabled`, apply CCT-aware white-point subtraction
to the sampled RGB before DMX channel output. Follow the formula in the
decision doc: `w_extract = min(r/r_w, g/g_w, b/b_w).clamp(0,1)` where
`[r_w, g_w, b_w] = cct_to_rgb(w_channel_cct_k)`. Scale `w_out` by
`w_scale`. Output RGBW as four DMX bytes.

**Acceptance:**
- [ ] Neutral grey scene with 6500K CCT → high W, near-zero coloured channels.
- [ ] Warm amber scene with 2700K CCT → high W, minimal coloured remainder.
- [ ] `enabled: false` → existing RGB-only output unchanged (no regression).
- [ ] `make ci` clean.

---

### P7.9.3 — RGBW UI

**Source:** `004-phase-7.md` Light (refinement).
**Type:** UI / UX
**Depends on:** P7.9.2
**Files:** `src/windows/` (fixture group / light-output inspector).

**What:** In the fixture group inspector, add "RGBW" toggle + CCT dropdown
(`[2700K | 3000K | 3200K | 4000K | 5600K | 6500K | Custom]`). Custom → Kelvin
slider (2000–8000K step 100). W scale slider (0.0–2.0, default 1.0).
`Mutation::SetRgbwConfig` dispatched on change.

**Acceptance:**
- [ ] RGBW toggle visible in fixture group inspector.
- [ ] CCT dropdown + Custom Kelvin slider functional.
- [ ] W scale slider dispatches `SetRgbwConfig`; undo/redo works.
- [ ] Glossary `GlossaryTerm::Cct` popover on the label.
- [ ] `make ci` clean.

---

## Workstream 10 — Scene pack export / import

### P7.10.1 — Scene pack schema + zip format

**Source:** `004-phase-7.md` Project / export–import.
**Type:** schema / persistence
**Depends on:** none
**Files:** new `src/scene_pack/mod.rs`, `src/scene_pack/schema.rs`.

**What:** A scene pack is a `.rmap-scene-pack.zip` containing:
- `manifest.json` with `pack_id: Uuid`, `name: String`, `author: String`,
  `created_at`, `schema_version: 1`, `templates: Vec<ScenePackTemplate>`.
- Each template is a `LayerConfig` JSON (from `src/project/schema.rs`) plus
  any referenced local assets (PNGs, SVGs, `.rmap-preset.json`).

Assets are stored relative to the manifest; paths are normalised to
forward-slash within the zip. Export: collect selected layer configs +
referenced files, write zip atomically. Import: unzip to
`~/Library/Application Support/rmap/scene-packs/<pack-id>/`, register
templates into `FxPresetRegistry` (or a new `SceneTemplateRegistry`).

**Acceptance:**
- [ ] Export produces a valid zip with `manifest.json` + assets.
- [ ] Import extracts to the correct directory; templates visible in the preset
      browser.
- [ ] Round-trip: export → import → select template → layer renders correctly.
- [ ] `make ci` clean.

---

### P7.10.2 — Scene pack export UI

**Source:** `004-phase-7.md` Project.
**Type:** UI / UX
**Depends on:** P7.10.1
**Files:** `src/windows/` (layer context menu or File menu).

**What:** Right-click a layer (or select multiple) → "Export as Scene Pack…".
Opens a save panel (default name = layer display label + `.rmap-scene-pack.zip`).
Confirmation dialog lists assets included. Export triggers P7.10.1 logic.

**Acceptance:**
- [ ] "Export as Scene Pack…" appears in layer context menu.
- [ ] Save panel opens; export writes a valid zip.
- [ ] Confirmation lists included assets.
- [ ] `make ci` clean.

---

### P7.10.3 — Scene pack import UI

**Source:** `004-phase-7.md` Project.
**Type:** UI / UX
**Depends on:** P7.10.1
**Files:** `src/windows/preset_browser.rs` (or equivalent).

**What:** File > Import Scene Pack… → open panel → extracts to local cache →
templates appear in the preset browser under a "Scene Packs" section. Duplicate
`pack_id` on re-import replaces the existing pack (same-ID update, no
duplicate). Pack management: right-click in browser → "Remove Pack" deletes
the extracted directory and unregisters templates.

**Acceptance:**
- [ ] "Import Scene Pack…" in File menu; opens dialog; templates appear in
      browser after import.
- [ ] Re-import of same `pack_id` replaces, not duplicates.
- [ ] "Remove Pack" removes templates from browser and deletes extracted files.
- [ ] `make ci` clean.

---

## Workstream 11 — Show-day diagnostics refinement

The Phase 7 acceptance criteria state: "The show-day diagnostics surface
remains terse — added Phase 7 capabilities do not bloat it."

### P7.11.1 — Show-day checklist: Phase 7 additions

**Source:** `004-phase-7.md` Acceptance criteria / `src/show_day/`.
**Type:** docs / process
**Depends on:** none
**Files:** `docs/show-day-checklist.md`.

**What:** Add one checklist section per Phase 7 major capability:
- Syphon output: "Enable Syphon out; confirm OBS sees the source."
- Calibration: "Load venue calibration; verify alignment cross on surface."
- Bezier warp: "Bezier handles not accidentally engaged (warp mode = Anchor)."
- RGBW: "Fixture group CCT matches physical fixture spec."
Keep additions terse — one line per checkpoint. Total checklist growth ≤ 8
new lines.

**Acceptance:**
- [ ] Each Phase 7 capability has ≤ 2 checklist lines.
- [ ] Total checklist length does not exceed the Phase 6 length + 8 lines.
- [ ] `make ci` clean.

---

### P7.11.2 — Diagnostics panel: Phase 7 audit kinds are terse

**Source:** `004-phase-7.md` Logging / diagnostics.
**Type:** engine / UX
**Depends on:** P7.2.4, P7.7.2 (audit kinds defined there)
**Files:** `src/windows/diagnostics.rs` (or audit display module).

**What:** Verify that the four new `AuditKind` variants from Phase 7
(`SyphonFrameworkMissing`, `CalibrationSurfaceUnmatched`,
`BezierMeshSchemaUpgraded` [informational after v7→v8 migration],
`RgbwConfigInvalid`) are displayed as single-line operator-readable messages in
the diagnostics / audit panel. No new "detailed" sub-panels. Regression:
run the full audit suite on a stock project and confirm the panel length is
unchanged.

**Acceptance:**
- [ ] Each new `AuditKind` renders as a single-line message.
- [ ] No new sub-panels or expanded detail sections added.
- [ ] Audit panel length on a stock project unchanged.
- [ ] `make ci` clean.

---

## Workstream 12 — Snapshot / proptest / acceptance smoke

### P7.12.1 — Proptest extension: W3 + W4 + W9 Mutations

**Source:** `src/project/CLAUDE.md` proptest rules.
**Type:** test
**Depends on:** P7.3.3, P7.4.3, P7.9.1 (all new Mutations defined)
**Files:** `src/project/command.rs` (proptest module).

**What:** Extend `proptest_round_trip` to cover all new Phase 7 `Mutation`
variants: `MoveBezierAnchor`, `SetBezierHandle`, `SetMaskInverse`,
`SetLumaKey`, `SetChromaKey`, `SetRgbwConfig`, `ResetLayerBezierMesh`.
Each variant must satisfy the three `ReverseStorage` rules from
`src/project/CLAUDE.md`.

**Acceptance:**
- [ ] All 7 new Mutation variants included in `proptest_round_trip`.
- [ ] All pass 1000 proptest iterations without shrink.
- [ ] No regression in existing Mutation variants.
- [ ] `make ci` clean.

---

### P7.12.2 — GPU golden: Syphon frame fidelity

**Source:** Phase 7 acceptance criteria ("OBS receives rmap output without
colour shift").
**Type:** test (GPU)
**Depends on:** P7.2.3
**Files:** `tests/headless_gpu.rs`, `tests/golden/`.

**What:** Headless GPU test: render a known scene, publish via `SyphonServer`
in loopback mode (if the framework supports it, or mock the `publish_frame`
call and compare the texture data directly). Assert pixel values match
within ε = 2/255 (accounts for sRGB rounding). This is the closest automation
gets to the manual "no colour shift" acceptance gate.

**Acceptance:**
- [ ] Test passes under `--features gpu-tests syphon-out`.
- [ ] Pixel delta ≤ 2/255 between the scene render and the Syphon-published
      texture.
- [ ] Golden image present in `tests/golden/syphon_frame_fidelity.png`.
- [ ] `make ci` clean (test skipped without GPU adapter).

---

## Workstream 13 — Release housekeeping + acceptance smoke

### P7.13.1 — Version bump 0.6 → 0.7.0

**Source:** release workflow.
**Type:** housekeeping
**Depends on:** all W1–W12 tasks green
**Files:** `Cargo.toml`.

**What:** Bump `version = "0.7.0"` in `Cargo.toml`. Run `cargo build` to
confirm no lockfile conflicts. `make ci` clean.

**Acceptance:**
- [ ] `cargo metadata --no-deps | jq .packages[0].version` = `"0.7.0"`.
- [ ] `make ci` clean.

---

### P7.13.2 — CHANGELOG + README body for v0.7

**Source:** release workflow.
**Type:** housekeeping
**Depends on:** P7.13.1
**Files:** `CHANGELOG.md`, `README.md`.

**What:** Replace the `## [0.7.0] — unreleased` placeholder (from P7.1.3) with
a release date and full capability description. Update `README.md` "Coming in
v0.7" stub with the shipped feature set (Syphon output, Bezier warp, inverse /
luma / chroma mask, RGBW, calibration file, scene packs).

**Acceptance:**
- [ ] CHANGELOG v0.7.0 section has date + all shipped capabilities listed.
- [ ] README updated.
- [ ] `make ci` clean.

---

### P7.13.3 — Phase 7 acceptance smoke test (manual)

**Source:** `004-phase-7.md` Acceptance criteria.
**Type:** acceptance smoke
**Depends on:** P7.13.2
**Files:** none (manual checklist appended to this task as a comment).

**What:** Manual walkthrough against every item in `004-phase-7.md`
Acceptance criteria:
1. Calibrate a venue once, save `.rmap-calibration.json`, open a second show
   file, load the calibration — confirm warp/mask/gamma applied.
2. Bezier warp on a curved column: no visible mesh banding at the seam.
3. Inverse mask + luma key accessible from the Mask mode pill sub-row without
   entering Advanced.
4. Syphon receiver in OBS captures rmap output; no colour shift; no stutter
   at 60 fps.
5. RGBW fixtures under the same scene as RGB fixtures: verify with reference
   colour chart (warm amber test, primary-colour test).
6. Show-day diagnostics panel: count lines before and after Phase 7 enable
   — confirm no growth beyond the checklist expectation.

**Acceptance:**
- [ ] All 6 acceptance items pass (operator sign-off required).
- [ ] Sign-off recorded as a comment on the PR.

---

## Anticipated risks

These design choices are locked — they were ratified in the W0 decision docs.
Each is a potential scope-creep site; call it out at task time if
implementation pressure pushes toward a different choice.

1. **Multi-output (>2 projectors) is OUT OF SCOPE for Phase 7.** The plan
   conditions multi-output on "single-surface workflow is already excellent."
   If implementation pressure raises the question, defer to a Phase 8 decision.
   The calibration schema supports multiple surfaces structurally, but the Phase
   7 UI and verify tooling only need to handle 1–2 surfaces. Do not add multi-
   output routing, edge-blend calculation, or multi-projector layout UI.

2. **Syphon.framework redistribution.** The framework is BSD-licensed and
   bundleable, but the pinned version in `vendor/` must be audited at each
   annual macOS release. If a future macOS breaks binary compatibility, the fix
   is to update the vendored framework — not to switch to a different
   integration strategy mid-release.

3. **Bezier UI palette scaling (I11) scope.** W3.5 adds a warp sub-row; the
   spec says ≤5 visible modes. If the palette needs to expand beyond 5, open a
   new decision doc before implementing — do not silently add a scrollable
   palette or a hidden mode.

4. **Calibration runtime binding is session-only.** The show file does not
   change when a calibration is loaded. If a task implementation is tempted to
   persist the binding in the show file, re-read the calibration decision doc
   — that is Option A, which was rejected.

5. **RGBW CCT scope.** W9 implements one W-channel CCT per fixture group.
   Multi-W-channel fixtures (RGBWW) and per-fixture CCT variation are out of
   scope. If a fixture requires more than one CCT value, defer to a follow-on
   decision.

6. **MaskGraph Union / Subtract nodes.** W4.1 defines the node types but
   Phase 7 only ships the `Polygon`, `Inverse`, `LumaKey`, and `ChromaKey`
   nodes. `Union` and `Subtract` are schema scaffolding only (no UI, no CPU
   evaluation path). Do not implement them in Phase 7; they exist so the
   schema is forward-compatible with a future composable mask editor.
