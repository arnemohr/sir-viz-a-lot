# 003 — Phase 3 Tasks: Interaction Overhaul

> Index: `003-tasks.md`. Plan: `003-ui-ux-overhaul-plan.md`.
> **32 tasks. ~5 engineering weeks. Ships internal beta (M3).**
>
> **Phase 3 entry is gated by a data-model change: T3.0a–T3.0d move
> mapping (warp + mask) from a project-level `Vec<WarpMesh>` to a
> per-layer field. Every Phase 3 interaction task that mentions
> `warp_idx` is wrong without that change first; treat T3.0a–T3.0d
> as the literal first work in Phase 3, before T3.1's canvas merge.**

## Purpose

Replace the five-tab IA with one canvas + a single Advanced
disclosure. Move mapping from a shared composite-then-warp model to
**per-layer warp + mask + effects** so each layer is mapped onto its
own surface independently. Make warp corners (now per-layer)
directly draggable on the live image. Add the in-context glossary.
Surface the four show-day controls (Blackout / Freeze / Test /
Outlines) as visible buttons.

This phase is the headline visible change of v3. After M3, `main`
runs the v3 UI by default; the v2 UI is removable code.

## Scope covered

- **WP-NEW (Per-layer warp + mask + effects architecture)** — the
  data-model + render-graph rewrite that the rest of Phase 3 depends
  on.
- WP-6 (Canvas merge: Scene + Mapping + Layers → one canvas)
- WP-7 (Advanced disclosure)
- WP-8 (In-context glossary)
- WP-10 (Show-day strip)

## Relationship to overall rollout

Phase 3 is the *visible* overhaul. M3 graduates v3 from "alpha
behind a flag" to "internal beta on `main`." Power users (Sami,
Marco) test that Advanced houses every v2 capability they used.

## Entry criteria

- M2 reached.
- WP-2 mutation surface 100% migrated (all P0+P1 tasks in Phase 1).
- All Phase 1 telemetry hooks live.
- Glossary v0 (T0.1) authored and PO-reviewed.

## Exit criteria

- Default surface contains 0 advanced controls (every v2 advanced
  control lives in the Advanced disclosure or is direct-on-canvas).
- Internal users (Eva-style + Marco-style) can complete the
  canonical 7-step flow on the new IA without consulting docs.
- Sami can complete every v2 task entirely within Advanced
  (verified by walkthrough).
- M3 declared; default UI flips on `main` (v2 UI removable).

---

## Task index

| ID | Title | Owner | Scope | Depends |
|----|-------|-------|-------|---------|
| **T3.0a** | **Schema v4: per-layer `WarpMesh` + migration from v3 `Project.warps`** | RUST | L | M2 |
| **T3.0b** | **Render graph rewrite: per-layer warp pass + composite-of-warped-layers** | RUST | L | T3.0a |
| **T3.0c** | **Mutation rename: `warp_idx` → `layer_idx` across all warp/mask variants** | RUST | M | T3.0a |
| **T3.0d** | **Audit rename + multi-warp consolidation finding** | RUST | S | T3.0a, T3.0c |
| T3.1 | Promote scene preview to full canvas | RUST | M | T3.0b |
| T3.2 | Layer thumbnail strip on left edge | RUST + DES | M | T3.1 |
| T3.3 | Selection-driven right-edge inspector | RUST + DES | M | T3.1, T3.0a |
| T3.4 | Toolbar with Warp/Advanced/Go-live buttons | RUST + DES | M | T3.1 |
| T3.5 | Wire `Selection::WarpCorner` direct manipulation | RUST | M | T3.4, T3.0c |
| T3.6 | Remove `ControlTab::Mapping` arm + checker placeholder | RUST | S | T3.5, T3.11 |
| T3.7 | `EditMode { Layer, Warp, Mask, Inspect }` enum | RUST | M | T3.1, T3.0a |
| T3.8 | `mode_banner` egui primitive (instruction strip per mode) | RUST + DES | S | T3.7 |
| T3.9 | Mode-aware cursor handling | RUST | S | T3.7 |
| T3.10 | Snap-to-edge for warp corners near framebuffer bounds | RUST | M | T3.5 |
| T3.11 | Single Advanced disclosure panel | RUST + DES | M | T3.1 |
| T3.12 | Move Master gamma/brightness/contrast into Advanced | RUST | S | T3.11 |
| T3.13 | Move Modulator picker into Advanced | RUST | S | T3.11 |
| T3.14 | Move per-effect editor into Advanced | RUST | M | T3.11 |
| T3.15 | Move mesh rows/cols and mask feather into Advanced | RUST | S | T3.11 |
| T3.16 | Move blend mode picker into Advanced | RUST | S | T3.11 |
| T3.17 | Move external-pass JSON into Advanced | RUST | S | T3.11 |
| T3.18 | Advanced disclosure "snap-back" on close | RUST | S | T3.11 |
| T3.19 | `glossary_label` egui primitive with `?` popover | RUST + DES | M | M2 |
| T3.20 | Glossary content registry | RUST + PO | S | T0.1 |
| T3.21 | Apply glossary entries to every advanced label | RUST + PO | M | T3.19, T3.20 |
| T3.22 | Compile-time check: every advanced term has a glossary entry | RUST | S | T3.20 |
| T3.23 | Show-day strip with B/F/T/O buttons | RUST + DES | M | T1.32 |
| T3.24 | Show-day strip key badges | RUST + DES | S | T3.23 |
| T3.25 | Show-day strip visible in `Editing` and `GoLive` | RUST | S | T3.23 |
| T3.26 | Phase 3 test harness additions | RUST + QA | M | T3.21, T3.23 |
| T3.27 | Remove old `ControlPanelState::tab` + tab strip rendering | RUST | S | T3.6, T3.18 |
| **T3.28** | **Per-display gamma + brightness + contrast override** *(NEW — practitioner-driven)* | RUST | S | T3.11 |

---

## WP-NEW — Per-layer warp + mask + effects architecture

### Why this is the first thing in Phase 3

Today (schema_version 3) the project carries a single
`Vec<LayerConfig>` and a separate `Vec<WarpMesh>`. The render graph
**composites every layer first** into a shared `warp_rt` texture,
**then** for each `WarpMesh` reads a `source_rect` of that composite
and remaps it to the projector. The consequence: every layer is
warped through whatever shared geometry the composite-as-a-whole
gets. Two layers cannot land on two different real-world surfaces
without authoring the composite layout to match — operator-hostile
for the canonical use case ("photo on wall A, video on wall B,
SVG overlay on the door").

The Phase 3 UX (T3.5 warp-corners-on-canvas, T3.7 EditMode) is
written assuming each layer can be mapped independently. Without
T3.0a–T3.0d the canvas merge ships with a global-warp interaction
model that contradicts the layer thumbnails on the left strip — the
operator selects layer 0, drags a warp corner, and *every* layer
deforms. That is the wrong product.

The fix is structural: each `LayerConfig` owns its own `WarpMesh`
(which carries the mask polygon). `Project.warps` is removed. The
render graph becomes per-layer warp → blend-composite onto the
projector RT. Effects continue to live in `LayerConfig.effects` and
already apply per-layer; the only architectural change is mapping.

### Task T3.0a: Schema v4 — per-layer `WarpMesh` + migration

**Purpose**
Move mapping into the layer. `LayerConfig.warp: WarpMesh`. Remove
`Project.warps`. Bump `CURRENT_SCHEMA_VERSION` from 3 to 4. Migrate
v3 projects on load by copying `warps[0]` (or a default identity
warp if the v3 project had none) onto each layer.

**Problem addressed**
WP-NEW. Without this, every Phase 3 mapping task is conceptually
broken (warp lives at the wrong granularity).

**Implementation details**

- `LayerConfig` (`src/project/schema.rs`) gains `pub warp: WarpMesh`
  with `#[serde(default = "WarpMesh::identity")]` so v3 projects
  loaded without per-layer warps still deserialise (the migration
  step then overwrites the default with the inherited geometry).
- `Project.warps` field deleted. `WarpMesh.source_rect` removed —
  each layer's warp consumes the entire layer output now; the
  source-rect concept doesn't apply once warps are per-layer.
- `CURRENT_SCHEMA_VERSION = 4`. New step in `src/project/migrate.rs`
  named `migrate_v3_to_v4`:
  - Read the project's pre-migration `warps: Vec<WarpMesh>` from
    the raw `serde_json::Value` (the field is gone from the typed
    struct, so migration runs on the JSON value before final
    deserialise).
  - For each layer in `layers`: write `layer.warp = warps.first()
    .cloned().unwrap_or_else(WarpMesh::identity)`.
  - Drop the top-level `warps` field.
  - Bump `schema_version` to 4.
  - If `warps.len() > 1`: stash a side-channel flag the audit pass
    (T3.0d) reads to emit a `MultipleWarpsConsolidated` Warn finding
    on the new project.
- `WarpMesh::identity()` constructor: 2×2 grid pinned to the unit
  square, full-canvas mask polygon, `mask_feather: 0.0`. Same
  geometry the v2 default produces today via
  `schema::default_warp_mesh()`.
- `Project::default()` adapts: layers carry identity warps; no
  top-level warps Vec to seed.

**Dependencies**
M2 — Phase 2 must be tagged before the schema bump goes onto `main`
(otherwise `--features v3` builds and `main` builds disagree on the
on-disk format mid-flight).

**Can run in parallel**
With T3.0c (mutation rename) once the schema lands; T3.0b (render
graph) is sequential.

**Acceptance criteria**

1. `Project { schema_version: 4, layers: [{warp: …}, …] }` round-trips
   through `Project::save` + `Project::load` byte-equal in
   `serde_json::Value` form.
2. A v3 fixture (e.g. a copy of `assets/demos/window-glow.rmap.json`
   pre-migration) loads under v4 with each layer carrying a copy
   of the v3 `warps[0]`.
3. v3 project with M > 1 warps and N layers: every layer ends up
   with a copy of `warps[0]`; the side-channel flag for
   `MultipleWarpsConsolidated` is set so T3.0d's audit fires once.
4. v3 project with zero `warps`: every layer ends up with
   `WarpMesh::identity()`. No audit finding.
5. `cargo nextest run --features v3` green; new test
   `migrate_v3_warps_consolidate_per_layer` covers all three
   migration cases.
6. `assets/demos/window-glow.rmap.json` is rewritten in v4 form by
   the same commit so the bundle ships canonical v4.

**Verification**
Unit tests on `migrate_v3_to_v4`. Manual smoke loading a fixture v3
project and inspecting the resulting layer warps.

**Risks / notes**

- The proptest harness (`project::command::tests::proptest_round_trip`)
  builds projects via `MutationKind::*`; the harness must be updated
  to seed per-layer warps once those mutations are renamed in
  T3.0c.
- `WarpMesh::source_rect` removal touches every `WarpRenderer`
  callsite. Land T3.0a + T3.0b in the same PR or behind a sub-flag
  (`v3-per-layer-warp`) so `main` doesn't have a half-applied
  schema during the transition.
- Existing `~/Documents/rmap/` user projects on developer machines
  migrate transparently. No user-facing migration step required.

**Suggested owner**
RUST.

**Estimated scope**
L (justified — schema bump, migration code, fixture rewrite, test
matrix all in scope).

---

### Task T3.0b: Render graph rewrite — per-layer warp + composite-of-warped-layers

**Purpose**
Replace the shared-`warp_rt` composite-then-warp model with a
per-layer warp-then-composite. Each layer is warped onto a scratch
RT in projector space, then alpha-composited onto the running
projector RT with the layer's `blend_mode` and `opacity`.

**Problem addressed**
WP-NEW + the render-graph half of T3.0a. Without this, the schema
change in T3.0a has no runtime consumer.

**Implementation details**

Today's pipeline (`src/render/CLAUDE.md`, render graph section):

```
per-layer raster ──► layer composite ─► warp_rt (shared)
                                          │
                            for each warp: warp_rt[source_rect]
                                          │
                                          ▼
                                     projector_rt (corner-pinned regions)
                                          │
                                          ▼
                                       gamma → overlay → swap
```

Target pipeline:

```
per-layer raster ──► for each layer in order:
                       layer_pre_warp ── warp pass ──► warp_scratch (1× projector size)
                                                          │
                                                          ▼
                                                     blend-composite onto projector_rt
                                                          (uses layer.blend_mode + opacity)
                                          │
                                          ▼
                                       gamma → overlay → swap
```

- New `warp_scratch: wgpu::Texture` sized to projector RT, reused
  across layers within a frame (one allocation, N writes, N reads).
  Replaces the shared `warp_rt`.
- `WarpRenderer` becomes per-layer: one instance per
  `LayerState` (the GPU sibling of `LayerConfig`). The renderer's
  per-frame call signature changes: `render(&mut self, encoder,
  source: &TextureView, target: &TextureView)` where `source` is
  the layer's pre-warp output and `target` is `warp_scratch`.
- Compositor pass (`src/render/compositor.rs` or equivalent):
  formerly composited pre-warp layer outputs into `warp_rt`. Now
  composites each layer's `warp_scratch` (post-warp, projector-
  space) onto the running projector RT. Blend-mode and opacity
  uniforms still consumed; the math is identical, the input is just
  in a different coordinate space.
- Per-frame order:
  1. Clear projector RT (single clear, before the loop).
  2. For each enabled layer in `Project.layers`:
     a. Render `LayerKind` content + per-layer `effects` chain into
        the layer's pre-warp texture (existing path).
     b. Apply the layer's `WarpMesh` + mask SDF, sampling the pre-
        warp texture, writing into `warp_scratch` with `LoadOp::Clear`.
     c. Composite `warp_scratch` onto projector RT with `LoadOp::Load`
        and the layer's `blend_mode` + `opacity`.
  3. `gamma` pass over projector RT.
  4. `overlay` pass (editor handles).
  5. Present.
- `scene_texture_id` (egui-registered native texture used for the
  control-window preview) re-binds to the projector-RT view, **not**
  the dead `warp_rt_view`. The preview now shows post-warp,
  pre-gamma content — semantically the same surface the operator
  sees on the projector.
- `OverlayPipeline` and `panic_restore` integration unchanged.

**Dependencies**
T3.0a (schema must allow per-layer warps before the renderer reads
them).

**Can run in parallel**
T3.0c (mutation rename) is sequential after T3.0b's compile lands;
T3.0d (audit rename) is parallel.

**Acceptance criteria**

1. Single-layer project: visual output is bit-equivalent to v2 within
   the existing golden-image tolerance (`tests/golden/warp.png`
   regenerated; diff ≤ 2/255 per channel).
2. Multi-layer project (≥ 3 layers, mixed `Image` and `Svg`, each
   with a distinct corner-pin): each layer renders with its own
   warp; verified by a new headless GPU test
   `per_layer_warp_distinct_corners` under `--features gpu-tests`.
3. Blend modes (`Normal`, `Add`, `Multiply`, `Screen`) produce the
   same composite as v2 *modulo* the warp coordinate-space change.
   Existing blend-mode tests pass.
4. Memory: a 1920×1080 projector RT + a single 1920×1080
   `warp_scratch` (≈ 16 MB total). The shared `warp_rt` is gone;
   net memory change is roughly zero.
5. Per-frame perf for N = 10 layers: within 15% of the v2 single-
   warp pipeline at the same framebuffer size. Stress-tested via
   `cargo bench` or a manual frame-time read.
6. `cargo nextest run --features v3 gpu-tests` green.

**Verification**

- New golden-image test for the multi-layer-distinct-corners case.
- Frame-time read on the v2 fixture with 5 layers vs. the v4 build
  on the same fixture migrated.

**Risks / notes**

- Per-frame allocation budget: we **must** keep `warp_scratch` as
  one persistent texture, not a per-layer fresh allocation. The
  `LoadOp::Clear` at the start of each layer's warp pass is what
  makes reuse safe.
- The `panic_restore::run_frame_assert_unwind_safe` wrapper sees a
  longer per-frame call chain. Verify panic injection still routes
  to `RenderError::RenderPanic` cleanly — add a panic-injection
  test in the multi-layer fixture.
- The control-window preview registration changes its source view.
  Verify `register_native_texture` re-runs after `T3.0b` rebuilds
  the projector-RT view (the existing resize hook covers this; just
  audit it).
- This is the largest single render-graph change in the v3
  initiative. Recommend landing in a feature-flagged branch
  (`--features v3-per-layer-warp`) first; flip the default once
  T3.0a + T3.0b + T3.0c + T3.0d all green.

**Suggested owner**
RUST.

**Estimated scope**
L (justified — render graph, golden tests, perf check).

---

### Task T3.0c: Mutation rename — `warp_idx` → `layer_idx`

**Purpose**
Every Phase-1 mutation that targeted a project-level warp now
targets a layer's owned warp. Rename mechanically; the apply/Reverse
logic stays structurally identical.

**Problem addressed**
WP-NEW + Phase 1 retrofit debt. Without this, the v3 mutation
language still talks about `Project.warps[i]` which no longer
exists.

**Implementation details**

Variants to rename (in `src/project/command.rs`):

| v3 variant | v4 variant |
|---|---|
| `SetWarpDimensions { warp_idx, … }` | `SetLayerWarpDimensions { layer_idx, … }` |
| `SetMaskPolygon { warp_idx, … }` | `SetLayerMaskPolygon { layer_idx, … }` |
| `AddMaskVertex { warp_idx, … }` | `AddLayerMaskVertex { layer_idx, … }` |
| `RemoveMaskVertex { warp_idx, … }` | `RemoveLayerMaskVertex { layer_idx, … }` |
| `SetMaskVertex { warp_idx, … }` | `SetLayerMaskVertex { layer_idx, … }` |
| `ResetWarpMesh { warp_idx, … }` | `ResetLayerWarpMesh { layer_idx, … }` |
| `SetWarpMaskFeather { warp_idx, … }` | `SetLayerMaskFeather { layer_idx, … }` |

Plus a new variant introduced for T3.5's per-layer corner drag:

- `SetLayerWarpCorner { layer_idx, r: usize, c: usize, new: [f32; 2], old: [f32; 2] }`
  with `needs_layer_rebuild() = false` (warp grid edits don't
  invalidate `LayerState` GPU resources, only re-baked SDFs).

Helper constructors update: `Project::set_layer_warp_dimensions_mutation(layer_idx, …)`
etc., reading the current value from `project.layers[layer_idx].warp`.

The proptest harness (`MutationKind` enum + `to_mutation` match)
gains the renamed variants. Existing test assertions on the v3
names update mechanically.

`is_non_undoable()` and `needs_layer_rebuild()` branches: every
renamed variant stays `false` for `is_non_undoable` (warp + mask
edits are user-driven and Cmd-Z reversible) and `false` for
`needs_layer_rebuild` (the renderer rebakes SDF / warp grid
buffers off the project state without recreating `LayerState`).

**Dependencies**
T3.0a (schema gives the field), T3.0b (renderer reads it).

**Can run in parallel**
With T3.0d (audit) after this lands.

**Acceptance criteria**

1. All seven renamed variants compile + tests pass.
2. `SetLayerWarpCorner` lands with apply / Reverse / proptest
   coverage.
3. Helper constructors: `Project::set_layer_*_mutation` exist for
   every renamed variant; UI sites in T3.5 / T3.7 / T3.15 consume
   them.
4. The proptest round-trip generates per-layer warp / mask
   mutations and asserts byte-equal Reverse.
5. `cargo nextest run --features v3 -E 'package(rmap) and test(/proptest_round_trip/)'`
   green with ≥ 1024 cases.

**Verification**
Unit tests + proptest. No manual smoke needed; the rename is
structural.

**Risks / notes**

- The Cmd-Z keyboard handler in `app.rs` (Phase-1 code) does not
  inspect variant names, so the rename is invisible there.
- ControlPanel's old Mapping-tab UI (still rendered until T3.6
  deletes it) calls `Project::set_warp_dimensions_mutation(0, …)`.
  T3.0c keeps a compatibility alias **for one phase** so the v2
  Mapping tab still works during T3.6's deletion window. The alias
  is removed in T3.6.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T3.0d: Audit rename + multi-warp consolidation finding

**Purpose**
The `ProjectAudit` findings emitted by Phase-1 audits (`DegenerateWarp`,
`MaskTooFew`) target `warp_idx`. Under v4 they target `layer_idx`.
Rename + add a new `MultipleWarpsConsolidated` finding that fires
exactly once per session for v3 projects whose migration consolidated
> 1 warps onto layers (T3.0a's side-channel signal).

**Problem addressed**
WP-NEW + audit consistency.

**Implementation details**

`AuditKind` (`src/project/audit.rs`) variants renamed:

| v3 variant | v4 variant |
|---|---|
| `DegenerateWarp { warp_idx }` | `DegenerateLayerWarp { layer_idx }` |
| `MaskTooFew { warp_idx, vertex_count }` | `LayerMaskTooFew { layer_idx, vertex_count }` |

New variant:

- `MultipleWarpsConsolidated { previous_warp_count: usize, layer_count: usize }`
  with `Severity::Warn`, message:
  *"Project had {previous_warp_count} warps but rmap now maps each
   layer individually. Each layer was given a copy of the first warp;
   verify layer mapping looks right."*
  Autofix: `None` (the operator must re-map per layer; no automated
  best-effort restoration).

The audit pass walks `project.layers` (not `project.warps`); each
layer's owned warp produces its own `DegenerateLayerWarp` /
`LayerMaskTooFew` finding when applicable. The audit no longer emits
findings against a non-existent `Project.warps` — that field is gone.

The `MultipleWarpsConsolidated` finding is fired once on first audit
of the migrated project. T3.0a stashes the previous warp count in a
project-side-channel field (`Project.transient_audit_signals`,
`#[serde(skip)]`) which the audit consumes and clears.

**Dependencies**
T3.0a, T3.0c.

**Can run in parallel**
With downstream Phase 3 UX tasks once it compiles.

**Acceptance criteria**

1. `DegenerateLayerWarp` / `LayerMaskTooFew` produced for the same
   shapes the v3 variants produced (proptest reused with renamed
   matchers).
2. `MultipleWarpsConsolidated` fires exactly once per session for
   a v3 project with > 1 original warps; never fires for v3 projects
   with ≤ 1 warp; never fires for v4-native projects.
3. Existing audit toasts still surface the renamed findings via
   `apply_launch_command` (T-003-T1.43) without code changes — the
   match arms inside the toast push are exhaustive over `AuditKind`,
   so the renamed variants surface naturally.
4. `cargo nextest run --features v3` green.

**Verification**
Unit tests in `src/project/audit.rs::tests`.

**Risks / notes**

- The `MultipleWarpsConsolidated` text is the operator's only
  signal that mapping may need fix-up. Phase 3 design QA (T4.21
  later) should review the copy.
- Removing the warp-level audit findings frees up the `warp_idx`
  parameter in toast actions. T2.24's `Command::OpenRelinkPicker`
  is unaffected (it targets `layer_idx` already).

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## WP-6 — Canvas merge

### Task T3.1: Promote scene preview to full canvas

**Purpose**
Replace the tabbed control panel with a single canvas that *is*
the live preview. The canvas is no longer one section of one tab;
it is the whole control window's central area.

**Problem addressed**
Plan WP-6.

**Implementation details**
- `windows/control_panel.rs` is renamed `windows/canvas.rs` (or a
  new module sits alongside; `control_panel` shrinks to a thin
  shim during migration).
- The render function becomes `canvas::show(ui, project,
  state, scene_editor, inputs) -> Vec<Command>`.
- The egui top-tab strip (`Scene / Effects / Layers / Mapping /
  Scenes`) is *not yet deleted* (T3.27 deletes after migration);
  for this task, it is hidden when `--features v3` is on.
- The Scene preview's existing direct-manipulation logic (drag
  layer, drag mask vertex, etc.) survives unchanged.
- The rest of the previous tabs (Effects / Layers / Mapping /
  Scenes) still render their UI but **into the Advanced
  disclosure** (T3.11+). For Phase 3 entry, render them into a
  collapsed Advanced panel that opens via the toolbar button.

**Dependencies**
M2.

**Can run in parallel**
With T3.19, T3.23.

**Acceptance criteria**
1. With `--features v3`, the control window opens with a single
   canvas, no top tab strip visible.
2. Live preview fills the centre.
3. Drag-drop, layer drag, mask vertex drag continue to work.
4. The Advanced toolbar button (T3.4) opens a panel with the
   Effects/Layers/Mapping/Scenes content (rough placement; T3.11+
   refines).
5. Without `--features v3`, the v2 tabbed UI is unchanged.

**Verification**
Manual smoke comparing v2 and v3 builds.

**Risks / notes**
This PR is large. Split into subtasks if needed; recommend
landing the canvas + Advanced shell first, then iterating on the
Advanced contents in T3.12–T3.17.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T3.2: Layer thumbnail strip on left edge

**Purpose**
Replace the typed Layers tab with a Procreate-style left strip:
vertical list of layer thumbnails, each with a visibility toggle
+ opacity preview.

**Implementation details**
- Render at the canvas's left edge, ~80 px wide.
- Thumbnail per layer: a small (64 × 36) snapshot of the layer's
  most recent rasterised content. Use the existing per-layer
  intermediate texture if accessible; otherwise render a coloured
  placeholder bound to the layer's id hash.
- Click a thumbnail → selects the layer (`scene_editor.selected =
  Selection::Layer(idx)`).
- Drag a thumbnail vertically → reorder (emits
  `Command::SwapLayers`).
- A `+` tile at the bottom opens the file picker (T2.13).
- Visibility toggle per thumbnail emits
  `Command::SetLayerEnabled`.

**Dependencies**
T3.1.

**Can run in parallel**
With T3.3, T3.4.

**Acceptance criteria**
1. Strip visible on left.
2. Each layer has a thumbnail.
3. Click a thumbnail → selection follows.
4. Drag-reorder works; emits the right command.
5. Visibility toggle works.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.3: Selection-driven right-edge inspector

**Purpose**
When a layer / warp corner / mask vertex is selected, a small
right-edge inspector shows its properties (move/scale/rotate +
opacity) plus a "More…" link to Advanced.

**Implementation details**
- ~280 px wide, slides in from right when `scene_editor.selected
  != None`.
- Default content:
  - `Selection::Layer(idx)` → translate / scale / rotate / opacity
    sliders (already exists in v2's Scene tab, just reposition);
    plus a small "Mapping" sub-section showing the layer's warp
    grid dimensions (rows × cols) and mask vertex count, with
    affordances to enter Warp / Mask edit mode for *this* layer
    (T3.0a — every layer owns its own warp + mask).
  - `Selection::WarpCorner { layer_idx, r, c }` → numeric x/y
    readouts and a "Reset this corner" button. The selection
    carries `layer_idx` because corners belong to layers (T3.0c).
  - `Selection::MaskVertex { layer_idx, idx }` → numeric x/y
    readouts; `layer_idx` for the same reason.
- "More…" link opens Advanced.
- Esc / clicking empty canvas → inspector hides.

**Per-layer note (T3.0a follow-up)**
The Inspector's Mapping sub-section is the operator's primary
entry point into per-layer warp editing — clicking "Edit warp"
drops into Warp mode (T3.7) scoped to the selected layer.

**Dependencies**
T3.1.

**Can run in parallel**
With T3.2, T3.4.

**Acceptance criteria**
1. Inspector appears on selection.
2. Properties update live as the user drags.
3. Inspector hides on Esc or deselect.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.4: Toolbar with Warp / Advanced / Go-live buttons

**Purpose**
Top-of-canvas toolbar with primary controls.

**Implementation details**
- Left side: project name (auto-saved indicator — Phase 4
  refines), Undo / Redo buttons.
- Right side: **Warp** (mode toggle), **Advanced** (disclosure
  toggle), **Go live** (Phase 4 transitions to fullscreen; for
  Phase 3 it's a stub button).
- Each button uses `command_button` for telemetry consistency
  (clicks emit non-undoable `Command::OpenAdvanced` etc.).

**Dependencies**
T3.1.

**Can run in parallel**
With T3.2, T3.3.

**Acceptance criteria**
1. Toolbar visible on top of canvas.
2. Undo / Redo buttons work and reflect undo-stack state
   (disabled when empty).
3. Warp button toggles `EditMode::Warp` (T3.7).
4. Advanced button toggles the Advanced panel.
5. Go live button is visible but doesn't yet transition (Phase 4
   T4.16).

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.5: Wire `Selection::WarpCorner` direct manipulation

**Purpose**
The `Selection::WarpCorner` arm is `#[allow(dead_code)]` in
`scene_editor.rs:42` today. The canvas merge needs it live —
**scoped to the selected layer's warp** (T3.0a).

**Problem addressed**
Plan WP-6 acceptance: warp corners draggable on the live image.
Plus WP-NEW: per-layer mapping — only the selected layer's warp
corners are interactive.

**Implementation details**
- `Selection::WarpCorner` carries `{ layer_idx, r, c }` (T3.0c
  rename).
- Hit-test priority (per `scene_editor.rs:11`): warp corners of the
  *selected layer* first, then mask vertices of the selected layer,
  then layer body. Other layers' warp corners are not painted and
  not hit-testable while in Warp mode — the canvas would otherwise
  show N×4 corner handles for N layers, defeating the
  per-layer-clarity goal.
- Hit testing only fires when `EditMode::Warp` is active (T3.7).
- Drag emits `Command::SetLayerWarpCorner { layer_idx, r, c, new,
  old }` (T3.0c new variant).
- Visual: the *selected layer's* warp grid is painted on the canvas
  as a faint mesh with handle dots at every grid intersection.
  The selected-layer outline (already painted in v3 Layer mode) is
  preserved so the operator sees which layer they're warping.

**Dependencies**
T3.4, T3.0c (the renamed mutation must exist).

**Can run in parallel**
With T3.6, T3.7.

**Acceptance criteria**
1. Toggle Warp mode while a layer is selected → that layer's grid
   is visible on canvas; no other layer's grid is.
2. Drag a corner → that layer's image deforms; command emits on
   drag end.
3. Cmd-Z reverses corner.
4. Snap-to-edge (T3.10) is a no-op until that task lands.
5. Toggle Warp mode without any layer selected → grid is hidden;
   a banner instructs the operator to select a layer first
   (T3.8 mode-banner copy update — covered in T3.0a-derived edits).
6. Switching the selected layer while in Warp mode swaps the
   visible grid in one frame.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T3.6: Remove `ControlTab::Mapping` arm + checker placeholder

**Purpose**
The Mapping tab's 480×270 checker-pattern canvas is the audit's
single most-derided UI element. Delete it.

**Implementation details**
- Delete `ControlTab::Mapping` from the enum.
- Delete `show_mapping_tab` function (`control_panel.rs:591`).
- Delete the checker-pattern rendering code.
- Per-layer mesh rows/cols and mask feather move to Advanced
  (T3.15) — surfaced **only when a layer is selected**, since each
  layer owns its own values (T3.0a).
- Zone-template buttons move to Advanced or to the warp corner
  inspector (T3.3 inspector when a `Selection::WarpCorner` is
  active — design call). Zone templates apply to the **selected
  layer's** mask polygon, not to a project-level mask.
- Drop the T3.0c compatibility alias for the old project-level
  `set_warp_dimensions_mutation(0, …)` callsite — once T3.6 deletes
  the v2 Mapping tab, no caller remains.

**Dependencies**
T3.5 (warp editing on canvas), T3.11 (Advanced has destinations),
T3.0a (per-layer warp/mask data model).

**Can run in parallel**
After both deps.

**Acceptance criteria**
1. `ControlTab::Mapping` gone.
2. `show_mapping_tab` gone.
3. Checker-pattern code gone.
4. All previous Mapping-tab capabilities still reachable per layer:
   warp corners on canvas (T3.5), per-layer mesh rows/cols in
   Advanced (T3.15), zone templates applying to the selected
   layer's mask.
5. `cargo build --features v3` succeeds without unused-import
   warnings.
6. T3.0c's compatibility alias is removed; `grep -r
   set_warp_dimensions_mutation` returns no hits outside the
   mutation definition itself.

**Verification**
Manual + `grep ControlTab::Mapping`.

**Risks / notes**
Critical timing: do not delete before T3.11 lands its
destinations. See `003-tasks.md` Section 2 sequencing-mistake R3.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.7: `EditMode { Layer, Warp, Mask, Inspect }` enum

**Purpose**
The canvas has interaction modes; encode them. **Each non-Inspect
mode is implicitly scoped to the selected layer** (T3.0a) — Warp
mode edits the selected layer's warp, Mask mode edits the selected
layer's mask polygon. There is no "global" warp or mask in v4.

**Implementation details**
- New enum `EditMode` in `windows/scene_editor.rs`:
  - `Layer` (default; current v2 behaviour — drag/scale/rotate the
    selected layer's body).
  - `Warp` (warp corner editing on the selected layer's grid; grid
    visible only for that layer).
  - `Mask` (mask vertex editing on the selected layer's polygon;
    polygon visible only for that layer).
  - `Inspect` (selection only, no drag).
- `SceneEditorState.mode: EditMode`.
- Mode toggled via the Warp button on the toolbar (T3.4).
- `Mask` mode entered automatically when a mask vertex is
  selected; deselection returns to `Layer` mode (or current
  mode).
- Entering `Warp` or `Mask` with no layer selected: the mode is
  still set, but the canvas paints nothing interactive and the
  mode banner (T3.8) reads *"Select a layer first."* This is
  intentional — modes track the user's *intent* even when there's
  nothing to act on yet.
- `handle_scene_input` dispatches by mode and reads the selected
  layer's `WarpMesh` / mask polygon directly off
  `project.layers[selected].warp`.

**Dependencies**
T3.1, T3.0a (the EditMode behaviour reads per-layer warp data
that only exists post-migration).

**Can run in parallel**
With T3.5, T3.10.

**Acceptance criteria**
1. Enum exists; default is `Layer`.
2. Warp button toggles `Layer ↔ Warp`.
3. Selecting a mask vertex switches to `Mask` mode.
4. `Inspect` mode is reachable (e.g., via a future "lock" toggle —
   stub for now).
5. `Warp` / `Mask` modes operate only on the selected layer; other
   layers' warp grids and mask polygons are not painted and not
   interactive while in those modes.
6. Switching the selected layer mid-Warp-mode (e.g. via the
   thumbnail strip) swaps the visible grid to the new layer in
   one frame.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T3.8: `mode_banner` egui primitive

**Purpose**
A thin instruction strip at the top of the canvas that updates
copy per `EditMode`.

**Implementation details**
- `mode_banner(ui, mode)` renders a single line of guidance:
  - `Layer` → *"Drag to move. Shift-drag to scale. Alt-drag to
    rotate."*
  - `Warp` → *"Drag the corners to fit the wall."*
  - `Mask` → *"Drag a vertex. Double-click an edge to insert.
    Shift-click to delete."*
  - `Inspect` → *"Click anything to inspect."*
- Visual: small, low-contrast, no border.

**Dependencies**
T3.7.

**Can run in parallel**
With T3.9.

**Acceptance criteria**
1. Banner visible at top of canvas.
2. Copy updates when mode changes.
3. Copy is concise (matches plan §H — "four sentences, eight
   verbs").

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
S.

---

### Task T3.9: Mode-aware cursor handling

**Purpose**
The cursor should reflect the current mode so the user understands
what their next click will do.

**Implementation details**
- `Layer` → default arrow.
- `Warp` → crosshair.
- `Mask` → cell.
- `Inspect` → arrow.
- egui exposes `ui.output().cursor_icon`; set per mode.

**Dependencies**
T3.7.

**Can run in parallel**
With T3.8.

**Acceptance criteria**
1. Cursor changes when mode changes.
2. Cursor reverts on mouse-leave from the canvas area.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.10: Snap-to-edge for warp corners near framebuffer bounds

**Purpose**
Plan §15.1 (D15) and §D6: ease-in snap on warp corners released
near the canvas edge.

**Implementation details**
- During `Command::SetWarpCorner` drag end, if the released
  position is within ~10 px (in canvas-screen space) of `[0.0,
  0.0]`, `[1.0, 0.0]`, `[0.0, 1.0]`, or `[1.0, 1.0]` (the four
  framebuffer corners), snap to that corner exactly.
- During the drag, paint a faint magnetic-zone indicator when the
  cursor is in range.

**Dependencies**
T3.5.

**Can run in parallel**
With T3.6+.

**Acceptance criteria**
1. Releasing a corner within 10 px of a framebuffer corner snaps
   to exact integer coords.
2. Snap is a single `Command::SetWarpCorner` with the snapped
   value, not the cursor's pixel-precise value.
3. Snap can be bypassed by holding Shift on release.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

## WP-7 — Advanced disclosure

### Task T3.11: Single Advanced disclosure panel

**Purpose**
One labelled door for all advanced controls.

**Implementation details**
- New module `windows/advanced.rs`.
- A right-edge drawer that slides in when `state.advanced_open ==
  true`.
- Width ~360 px.
- Contains sections (collapsible accordion-style, but not nested
  more than one level):
  - **Master** (gamma, brightness, contrast)
  - **Selected layer** (effect chain editor, blend mode,
    modulator pickers — only visible when a layer is selected)
  - **Selected warp** (mesh rows/cols, mask feather, source rect,
    zone templates — only visible when a warp is selected; T3.5
    introduces warp corner selection)
  - **Project** (autostart flag, output_windowed, project file
    info)
  - **Diagnostics** (audit findings re-runnable; telemetry summary
    if it grows usefully later)
- Telemetry: `advanced_opened` span (T1.46).

**Dependencies**
T3.1.

**Can run in parallel**
With T3.2–T3.10.

**Acceptance criteria**
1. Click Advanced toolbar button → panel slides in.
2. Click again or Esc → slides out.
3. Sections render in the listed order.
4. Default-collapsed Master section, default-open Selected layer
   section.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.12: Move Master gamma/brightness/contrast into Advanced

**Purpose**
The plan wants gamma off the default surface.

**Implementation details**
- The three sliders move from the always-visible
  `CollapsingHeader::new("Master (gamma)")` block in
  `control_panel.rs:206–213` into the Advanced "Master" section.
- They use the same `command_slider` helpers wired in T1.18.
- Each slider gets a `glossary_label` (`?` popover) for its term.

**Dependencies**
T3.11.

**Acceptance criteria**
1. Sliders no longer appear on the default canvas surface.
2. They appear in Advanced > Master.
3. Cmd-Z still reverses each.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.13: Move Modulator picker into Advanced

**Purpose**
The combobox at `control_panel.rs:907` moves into Advanced > Selected
layer > Effect chain.

**Implementation details**
- Render as part of each effect's parameter list.
- Now lives only when `state.advanced_open && a layer is selected
  && that layer has effects`.

**Dependencies**
T3.11.

**Acceptance criteria**
1. Modulator picker only visible inside Advanced.
2. Modulator type changes still emit commands; Cmd-Z still
   reverses.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.14: Move per-effect editor into Advanced

**Purpose**
The full effect-chain editor (`show_effect`, `show_effects_tab`)
moves to Advanced.

**Implementation details**
- Show effects only for the currently-selected layer.
- The "Apply preset" combobox (T1.29 already migrated) lives at
  the top of the effects section.
- The "Effect chain" heading is preserved.

**Dependencies**
T3.11.

**Acceptance criteria**
1. Effect chain editor only inside Advanced.
2. Effect parameter sliders + Modulator pickers + presets all
   still work.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T3.15: Move per-layer mesh rows/cols and mask feather into Advanced

**Purpose**
Plan §H: "Mesh rows / cols → Grid detail (Advanced)"; mask feather
slider → Advanced. Under v4 these are **per-layer** controls
(T3.0a) — surfaced in `Advanced > Selected layer > Mapping`, not
under a global "Selected warp" heading.

**Implementation details**
- Mesh rows/cols (currently at `control_panel.rs:609` over the
  v3-deleted Mapping tab; in v4 read from
  `project.layers[selected].warp.rows` / `.cols`) move into
  `Advanced > Selected layer > Mapping > Grid detail`. Sliders
  emit `Mutation::SetLayerWarpDimensions { layer_idx, rows, cols }`
  (T3.0c).
- Mask feather (currently `control_panel.rs:776`; in v4
  `project.layers[selected].warp.mask_feather`) moves into
  `Advanced > Selected layer > Mapping > Mask feather`. Slider
  emits `Mutation::SetLayerMaskFeather { layer_idx, … }`.
- The Advanced section is hidden (or shows an empty-state
  "Select a layer to see mapping controls") when
  `scene_editor.selected != Selection::Layer(_)`.

**Dependencies**
T3.11, T3.0a (per-layer fields), T3.0c (per-layer mutations).

**Acceptance criteria**
1. Both controls only inside Advanced > Selected layer > Mapping.
2. Resampling on row/col change still preserves the operator's
   customisation (existing `resample_grid` logic, now invoked
   per-layer).
3. With no layer selected, the controls are not visible.
4. Switching the selected layer updates the slider values to that
   layer's warp on the next frame.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.16: Move blend mode picker into Advanced

**Purpose**
Per-layer blend mode (`control_panel.rs:530`) moves to Advanced >
Selected layer.

**Dependencies**
T3.11.

**Acceptance criteria**
1. Blend picker only in Advanced.
2. Cmd-Z reverses (whole-enum Reverse already in T1.19).

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.17: Move external-pass JSON into Advanced

**Purpose**
The `Effect::External` block at `control_panel.rs:879` shows raw
JSON. Hide unless Advanced is open.

**Implementation details**
- Effects of variant `External` render their JSON only when
  Advanced is open AND the layer has at least one External effect.
- Otherwise show a small placeholder: *"This effect is configured
  in the project file."*

**Dependencies**
T3.11.

**Acceptance criteria**
1. JSON only visible inside Advanced.
2. Placeholder visible outside Advanced.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.18: Advanced disclosure "snap-back" on close

**Purpose**
When the user closes Advanced, transient state inside it (e.g.,
which sub-section was open) persists; selection state is honoured.

**Implementation details**
- Persist sub-section open/closed state in `ControlPanelState` for
  this session.
- Re-opening Advanced restores the same scroll position and
  open sub-sections.

**Dependencies**
T3.11.

**Acceptance criteria**
1. Open Advanced, scroll, expand "Selected layer", close.
2. Re-open: scroll position and "Selected layer" expansion
   preserved.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## WP-8 — In-context glossary

### Task T3.19: `glossary_label` egui primitive

**Purpose**
A label paired with a `?` icon. Hover (or click) → popover with
the term's glossary entry.

**Implementation details**
- Function `glossary_label(ui: &mut Ui, term: GlossaryTerm) ->
  Response`.
- `GlossaryTerm` is a typed enum (T3.20), not a string — so a
  typo is a compile error.
- Layout: term text + small `?` to the right.
- Hover for ≥ 250 ms → popover slides in.
- Popover content: term + 1–2 sentence body + optional "Learn
  more" link to a future docs URL (deferred placeholder for now).

**Dependencies**
M2.

**Can run in parallel**
With T3.1–T3.18, T3.23.

**Acceptance criteria**
1. Primitive renders label + `?` + popover.
2. Hover delay tuned so transient cursor passes don't trigger.
3. Popover dismisses on cursor exit.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.20: Glossary content registry

**Purpose**
Compile-time-checked storage for all glossary entries.

**Implementation details**
- New module `windows/glossary.rs`.
- `pub enum GlossaryTerm { Warp, MaskPolygon, Modulator, Gamma,
  Brightness, Contrast, BlendMode, Crossfade, Scene, SourceRect,
  ZoneTemplate, Blackout, Freeze, TestPattern, EditorOverlay,
  Effect, FitMode, ... }` — one variant per term.
- `pub fn entry(t: GlossaryTerm) -> GlossaryEntry`. `GlossaryEntry
  { headline, body }`.
- Body content from T0.1.
- Exhaustive match in `entry()` ensures every variant has content.

**Dependencies**
T0.1.

**Can run in parallel**
With T3.19.

**Acceptance criteria**
1. Enum and `entry` function exist.
2. Every variant has a non-empty body.
3. Compile-time exhaustive match (no `_ => …` arm).

**Verification**
`cargo build --features v3`.

**Suggested owner**
RUST + PO.

**Estimated scope**
S.

---

### Task T3.21: Apply glossary entries to every advanced label

**Purpose**
Every label inside Advanced gets a `glossary_label` rather than
a plain `ui.label`.

**Implementation details**
- Audit the Advanced panel: every parameter label, every section
  heading, every dropdown that names a domain term.
- Replace plain labels with `glossary_label(ui,
  GlossaryTerm::*)`.
- A non-domain label (e.g., "value", "amp", "phase") may stay
  plain.

**Dependencies**
T3.19, T3.20.

**Can run in parallel**
After both deps.

**Acceptance criteria**
1. Every advanced section has at least one `glossary_label`.
2. Every domain term's first appearance in a section uses the
   glossary primitive.
3. Hovering over any `?` icon shows a friendly popover.

**Verification**
Manual walkthrough of every Advanced section.

**Suggested owner**
RUST + PO.

**Estimated scope**
M.

---

### Task T3.22: Compile-time check — every advanced term has a glossary entry

**Purpose**
Plan R9: prevent content debt where new terms are added without
glossary entries.

**Implementation details**
- The `GlossaryTerm` enum (T3.20) is the only valid input to
  `glossary_label`. New terms require a new enum variant +
  matching `entry()` arm.
- A `lint_terms_have_entries` test iterates over every
  `GlossaryTerm` variant and asserts the body is non-empty.

**Dependencies**
T3.20.

**Acceptance criteria**
1. Adding a new term without a body fails the test.
2. Test runs in CI.

**Verification**
`cargo test --features v3 lint_terms_have_entries`.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## WP-10 — Show-day strip

### Task T3.23: Show-day strip with B/F/T/O buttons

**Purpose**
Four large always-visible buttons mirror the keyboard hotkeys.

**Implementation details**
- New egui strip at the bottom of the canvas, visible in both
  `Editing` and `GoLive`.
- Four buttons: **Blackout**, **Freeze**, **Test**, **Outlines**.
- Each emits the corresponding `Command` from T1.32.
- Visual state reflects current `OutputState`: active (blackout
  on) → button highlighted; inactive → muted.
- Test button cycles through patterns matching the `T` key.

**Dependencies**
T1.32 (commands exist), T3.4 (toolbar / canvas layout).

**Can run in parallel**
With T3.1–T3.22.

**Acceptance criteria**
1. Strip visible in `Editing` and `GoLive`.
2. Click each button → output state changes match keyboard.
3. Active state is visually distinct.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.24: Show-day strip key badges

**Purpose**
Each button shows its keyboard accelerator in a small badge.

**Implementation details**
- Render a small "(B)", "(F)", "(T)", "(O)" badge on each
  button.
- Badge style: low-contrast, small font.

**Dependencies**
T3.23.

**Acceptance criteria**
1. Badges visible.
2. Layout doesn't shift on hover.

**Verification**
Manual + design QA.

**Suggested owner**
RUST + DES.

**Estimated scope**
S.

---

### Task T3.25: Show-day strip visible in `Editing` and `GoLive`

**Purpose**
Confirm the strip survives the Go-live transition (Phase 4 lands
the transition itself; T3.25 makes sure the strip is part of
both states).

**Implementation details**
- Both `AppState::Editing` and `AppState::GoLive` arms render the
  strip.
- A future "Hide UI" mode (out of v3 scope) could hide it; not in
  this task.

**Dependencies**
T3.23.

**Acceptance criteria**
1. Strip visible in both Editing and a stubbed GoLive.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.26: Phase 3 test harness additions

**Purpose**
Extend the headless harness with the canonical 7-step flow.

**Implementation details**
- New end-to-end test "canonical_first_session":
  1. Boot launcher.
  2. Pick a (mocked) projector.
  3. Click "Try a demo".
  4. Drag a warp corner via `Command::SetWarpCorner`.
  5. Drop a (mocked) image.
  6. Save scene to slot 1.
  7. Click Go live (stubbed).
  8. Assert end state has all expected mutations + sane render
     output.
- Test the canvas-merge replacement: assert that no
  `ControlTab::*` arms (other than maybe a stub) remain.

**Dependencies**
T3.21, T3.23.

**Acceptance criteria**
1. Canonical test added and passing.
2. CI runs it.

**Verification**
CI green.

**Suggested owner**
RUST + QA.

**Estimated scope**
M.

---

### Task T3.27: Remove old `ControlPanelState::tab` + tab strip rendering

**Purpose**
Final cleanup: the v2 tab system is deletable.

**Implementation details**
- Delete `enum ControlTab` (`control_panel.rs:71`).
- Delete `ControlPanelState::tab` field.
- Delete the top tab strip rendering at
  `control_panel.rs:139–149`.
- Old `show_scene_tab`, `show_effects_tab`, `show_layers_tab`,
  `show_scenes_tab` either:
  - Renamed and adapted to the new canvas / Advanced model, or
  - Deleted entirely if their content has fully migrated.

**Dependencies**
T3.6, T3.18 (Advanced contents migrated).

**Can run in parallel**
After both deps.

**Acceptance criteria**
1. `cargo grep ControlTab` returns zero matches.
2. No unused imports.
3. v3 UI unchanged after the cleanup.

**Verification**
Build + smoke.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## Per-display tone override *(NEW — practitioner-driven)*

### Task T3.28: Per-display gamma + brightness + contrast override

**Purpose**
Even single-projector setups benefit from per-output tone
override because the laptop monitor and the projector live in
different colour spaces. The current `Master (gamma)` panel
applies globally; an operator who tunes gamma for the projector
makes the control-window preview look wrong, and vice versa.

**Background**
Practitioner-flagged. F4 in revision triage. Cheap real-world
fix; high practitioner value.

**Implementation details**
- New section in Advanced > Selected output: per-output
  `gamma_override`, `brightness_override`, `contrast_override`,
  each defaulting to `None` (inherit from master).
- Storage: per-output, on the existing `WarpMesh` struct
  (single-projector v3 has one warp per output region; multi-
  projector v0.4 will have an explicit `OutputTarget`).
- Render path: in the gamma pass, if any override is `Some`, use
  it instead of the master. Single-projector means one set wins;
  no conflict.
- The control window's *preview* uses the master values; the
  projector's *fullscreen* output uses the override values when
  present. This is the entire point — the operator sees their
  laptop-correct preview while the projector renders projector-
  correct.
- Glossary popovers (T3.21) for each override term.

**Dependencies**
T3.11 (Advanced disclosure exists).

**Parallelization**
After T3.11. Independent of T3.12–T3.17.

**Acceptance criteria**
1. Advanced > Selected output has three override sliders +
   "inherit" toggles.
2. Setting an override changes the projector but not the
   control-window preview.
3. Clearing the override (returning to "inherit") restores
   master-driven values.
4. Cmd-Z reverses each override change.
5. Project save/load round-trips override values.

**Verification**
- Manual: open the demo with a real projector, observe colour
  shift; tune the override; verify preview vs. projector
  divergence.
- Unit test on the gamma pass uniform: override absent → master
  value; present → override value.

**Practitioner relevance**
This is the highest-value real-world tweak in v3 for a working
operator. Without it, gamma tuning is a binary choice between
"laptop looks right" and "projector looks right." With it, both
look right.

**Risks / notes**
- Schema change: `WarpMesh` gains three optional fields.
  Migration: add as `serde::default` so existing projects load
  without explicit override.
- Multi-projector (v0.4) will reorganise this onto an
  `OutputTarget`; the v3 schema decision is deliberately
  forward-compatible.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## Phase 3 closeout — M3 readiness (internal beta)

Before declaring M3:

- [ ] All T3.* acceptance criteria green.
- [ ] **Default surface contains 0 advanced controls** (verified
      by manual walkthrough).
- [ ] Canonical 7-step flow completes on the new IA without docs
      (verified by an Eva-style team member).
- [ ] Sami completes every v2 task entirely within Advanced
      (verified by walkthrough).
- [ ] Old `ControlTab::Mapping` arm and checker placeholder are
      gone.
- [ ] Glossary popovers exist on every advanced label.
- [ ] Show-day strip visible and functional.
- [ ] `cargo run` *without* `--features v3` still runs the v2 UI
      (deferred removal to Phase 5).
- [ ] CI green including the canonical-flow harness.
- [ ] Default `--features v3` → flip on `main` for internal team
      use (per Q9 / D9 — confirm timing with PO).
- [ ] Tag `v0.3.0-beta` candidate prepared (final tag in M4).

Once all items check, M3 declared. Open
`003-tasks-phase-4-5.md`.
