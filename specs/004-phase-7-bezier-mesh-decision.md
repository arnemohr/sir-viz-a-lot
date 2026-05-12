# Phase 7 — Bezier mesh warp decision (P7.W0, W3)

**Status:** decision record. The data-model extension, shader, and
control-point hit-testing land in W3 tasks once this decision is ratified.

## Constraints

- **Backward compatibility with existing bilinear N×M mesh.** Every project
  produced by v0.3–v0.7 uses `WarpMesh { rows, cols, grid: Vec<Vec<[f32;2]>> }`
  stored at schema version 7 (`CURRENT_SCHEMA_VERSION`). The new mesh must
  either extend `WarpMesh` additively or replace it with a migration that
  produces pixel-identical output for degenerate (bilinear-equivalent) inputs.
- **M4 capability follow-on (from `004-phase-7.md`).** The spec names the
  UX explicitly: "active vertex → anchor + handles + tangents". This is
  cubic Bezier vocabulary, not B-spline or Catmull-Clark cage vocabulary.
- **N1 capability follow-on.** "Zoom-aware hit-area scaling + tangent-handle
  hit policy." The hit-test model must treat anchor and handle as distinct
  targets at interactive zoom levels.
- **I11 follow-on (UI palette scaling).** "UI palette must scale to ~5 modes
  per I11 follow-on." The selection visual gain from adding Bezier handles
  (anchor + 2 tangent handles per vertex) is the driver — but the palette
  extension is a UI task (W3.5), not a renderer task.
- **Build-time WGSL validation** (`src/render/CLAUDE.md`). Every `.wgsl`
  change passes naga `parse_str` + `Validator` at `cargo build` time. A
  broken Bezier shader fails the build, not the runtime.
- **CPU-side tessellation only.** The existing `build_warp_vertices` (in
  `src/render/warp.rs`) runs on CPU and uploads a vertex buffer; the GPU
  sees a flat vertex list. This approach must be preserved — the GPU render
  pipeline does not change topology.
- **`src/project/CLAUDE.md` mutation rules.** Any new `WarpMesh`-replacing
  `Mutation` variant must implement `ReverseStorage` (whole-enum Reverse,
  following the pattern of `ResetLayerWarpMesh` already in the codebase).

## Current mesh architecture (existing code)

`WarpMesh { rows, cols, grid: Vec<Vec<[f32;2]>> }` stores `(rows+1)×(cols+1)`
corner positions in normalised output space. `build_warp_vertices` (CPU)
tessellates each cell into `sub×sub` micro-quads via per-cell homography
(bilinear interpolation of the four corners), uploading vertices to the GPU.
The GPU shader samples the source texture using `src_uv` interpolated across
the micro-quad.

Bilinear interpolation between the four corners of each cell means:
- Straight-line edges between adjacent grid points (no curvature).
- C0 continuity at cell boundaries (positions match, tangents do not).
- Each point is a plain `[f32;2]` — no handle concept.

## Candidates evaluated

### 1. B-spline patches (rejected — control points do not lie on the surface)

A B-spline patch uses a control cage where control points generally do not
lie on the rendered surface. Operators drag control points and see the
surface move nearby, not through, the point. For live calibration (aligning
projection to a physical edge), this is disorienting: "I am dragging the
corner of the projection but nothing in the image passes through my mouse."

The B-spline basis would require a different tessellation kernel and a more
complex hit-test heuristic (snapping the click to the nearest surface point,
not the clicked control point). It also offers no meaningful quality advantage
over cubic Bezier patches for the curved-column use case.

**Verdict:** rejected. Ergonomically wrong for live operator calibration.

### 2. Catmull-Clark subdivision (rejected — wrong paradigm for flat meshes)

Catmull-Clark is a recursive subdivision scheme that produces smooth limit
surfaces from a coarse cage. It converges in the limit but has no exact
closed-form position for a given cage point — evaluating at an arbitrary
parameter value requires either many subdivision passes or a specialised
fast-evaluation algorithm (e.g. Loop + Sharp).

For a 2×2 grid (the common case), Catmull-Clark produces one smooth patch
with four corner points that lie on the limit surface only in a technical
sense. The cage-to-limit-surface displacement makes alignment to physical
architecture harder. Adding sharp edges (for a flat planar region) requires
crease tags — a substantial schema extension.

More critically, Catmull-Clark is an *adaptive* technique suited for organic
mesh smoothing, not for small-mesh warp calibration where operators manually
move individual vertices. The added complexity does not serve the "curved
column" use case better than cubic Bezier.

**Verdict:** rejected. Conceptual mismatch; unjustified complexity for 2–16
control-point meshes.

### 3. Cubic Bezier patches (chosen)

The chosen construction is **cubic-edge Bézier warp with Coons-blended
interior** — the same approach used by MadMapper, HeavyM, and comparable
projection-mapping tools. Each cell has four cubic Bézier edges (one per
side), each defined by two shared corner anchors and two tangent handles.
The interior of the cell is blended via a bilinear Coons patch from the four
cubic edge curves. This gives a 1×1 cell 12 control points: 4 corners + 8
edge handles (2 per edge). The 4 interior-only handles of a full 4×4
bicubic tensor-product patch are absent — they contribute curvature that
operators cannot intuit, and no projection-mapping tool exposes them.

The UX result is identical to what operators expect from "Bezier warp":
drag an anchor to move the surface, drag an edge handle to bow that edge.
The Coons-blended interior means concave/convex edges compose naturally
without seams at cell boundaries.

**Advantages:**
- Corners are on-surface: operators drag corners, corners move. Calibration
  matches physical intuition.
- Tangent handles are the natural UI for "bend this edge slightly to follow
  the column curve" — exactly the M4 follow-on vocabulary ("anchor + handles
  + tangents").
- C1 continuity across cell boundaries is achievable by constraining shared-
  edge handles to be collinear and equidistant — the tessellator enforces this
  automatically; operators don't need to understand it.
- Degenerate-to-bilinear migration: old `WarpMesh` loads as a
  `BezierMesh` where all edge handles are `None` (degenerate). A Coons
  patch with all-`None` handles and straight-line edges evaluates
  identically to bilinear interpolation between the four corners.
  Pixel output is identical to the pre-migration render.
- CPU tessellation maps cleanly: evaluate the Coons patch at a `sub×sub`
  grid of parameter values `(u,v) ∈ [0,1]²`. The Coons evaluation
  requires four cubic Bézier edge curves per cell — a simple closed-form
  expression with no iterative solver. The resulting vertex list drops into
  the existing `WarpVertex` buffer without changes to the GPU pipeline or
  shader.

**Trade-offs:**
- Schema grows: each cell adds 12 inner control points (the 4 corners were
  already there). For a 4×4 mesh this is 75 inner points added (from 25
  to 100). Still trivially small for JSON storage.
- Hit-testing is now two-tier: anchor click (existing semantics, closest
  grid vertex) vs. handle click (new; targets the 2 handles adjacent to the
  selected anchor). The hit-test policy locks to: "anchor click takes
  priority when cursor is within anchor-hit-radius; handle is clickable only
  when its anchor is selected." This matches the N1 follow-on requirement.
- The tessellation function gains a `BezierMesh` variant alongside the
  existing `WarpMesh` bilinear path. Both live in `warp.rs`; the CPU cost
  at sub=8 (current default) is negligible.
- Schema migration: `WarpMesh` → `BezierMesh` bumps `CURRENT_SCHEMA_VERSION`
  to 8 and adds a `migrate_v7_to_v8` step. Old projects load cleanly via the
  bilinear-handle initialisation; the round-trip is lossless.

## Decision

**Cubic-edge Bézier warp with Coons-blended interior is the chosen path.**

The M4 follow-on in `004-phase-7.md` explicitly names "anchor + handles +
tangents" — this is the standard Bézier vocabulary, and this construction
delivers it. Corners lie on the surface; edge handles bow individual edges;
the Coons interior automatically blends without visible seams. Full 4×4
tensor-product bicubic patches are explicitly not chosen: the 4 interior-only
handles are invisible to operators and their absence has no quality impact for
the curved-wall / curved-column use case. B-spline and Catmull-Clark are
rejected because their control-point-off-surface property conflicts with live
projection calibration ergonomics.

## Schema extension

```
// New in schema v8 (replaces WarpMesh in LayerConfig.warp).
// BezierMesh is the canonical type going forward.
// WarpMesh remains deserializable for v7 migration only.
pub struct BezierMesh {
    pub rows: u32,
    pub cols: u32,
    /// (rows+1) × (cols+1) anchor positions, normalised output space.
    pub anchors: Vec<Vec<[f32; 2]>>,
    /// Edge tangent handles: per-anchor, up to 4 optional handles in
    /// [N, E, S, W] order. Each handle governs one half of one shared
    /// edge (the other half is owned by the adjacent anchor).
    /// None = degenerate straight-line edge (bilinear-equivalent).
    pub handles: Vec<Vec<[Option<[f32; 2]>; 4]>>,
    #[serde(default)]
    pub mask_polygon: Vec<[f32; 2]>,
    #[serde(default)]
    pub mask_feather: f32,
}
```

The degenerate case (all handles `None`) produces straight-line edges whose
Coons-blended interior evaluates identically to bilinear interpolation of
the four corners. When a handle is `Some`, the tessellator uses the explicit
handle position to bow the edge. There are no interior-only control points —
the interior is entirely determined by the four edge curves.

## Architecture (for W3 follow-up tasks)

### W3.1 — Schema migration v7→v8 + `BezierMesh` data model

- Add `BezierMesh` to `src/project/schema.rs`.
- `WarpMesh` is kept (serde-deserializable) but marked `#[deprecated]`.
- `migrate_v7_to_v8` converts `WarpMesh` → `BezierMesh` with all handles `None`.
- Bump `CURRENT_SCHEMA_VERSION` to 8.
- Proptest: `BezierMesh::from_warp_mesh(old).to_warp_mesh()` round-trips
  pixel-identically (compare vertex buffer output numerically, not
  visually).

### W3.2 — CPU tessellation for `BezierMesh`

- New `build_bezier_vertices(mesh: &BezierMesh, sub: u32)` in `warp.rs`.
- Evaluates a Coons patch at `sub×sub` parameter points per cell. Each cell's
  four edges are cubic Bézier curves; the interior is Coons-blended from those
  edge curves. If a handle is `None`, the corresponding edge is a straight line
  (linear parameter, identical to the bilinear corner interpolation).
- Drops into the existing `WarpVertex` buffer + GPU pipeline unchanged.
- Golden test under `--features gpu-tests`: all-`None`-handle `BezierMesh`
  renders pixel-identically to the old `WarpMesh` golden.

### W3.3 — Bezier control-point hit-testing

- Extend `src/windows/scene_editor.rs` warp-vertex picker.
- Two-tier model: anchor-hit radius takes priority; handle-hit radius
  activates only when the anchor is selected.
- `Mutation::SetBezierHandle { layer_id, anchor_row, anchor_col, direction,
  new_pos, old_pos }` — symmetric `ReverseStorage`.
- `Mutation::MoveBezierAnchor { layer_id, row, col, delta, old_pos }` —
  symmetric `ReverseStorage`; anchors dragging propagates handles rigidly
  (no re-computation of curvature).

### W3.4 — Handle visual (egui overlay)

- Anchor = filled circle (existing warp-point visual).
- Handle = hollow diamond connected to anchor by a thin line (editor-chrome
  overlay drawn in the `OverlayPipeline` pass).
- Handles visible only when the anchor is selected.
- `O` overlay toggle hides handles with the rest of editor chrome.
- Golden: `tests/golden/bezier_handles_overlay.png`.

### W3.5 — UI palette scaling (I11 follow-on)

- Warp mode pill in the editor gains a sub-row: `[Anchor] [Handle] [Tangent]`
  (active state determines whether a drag moves the anchor or a handle).
- "Tangent" mode: moving one handle of a C1 pair mirrors the opposite handle
  symmetrically (smooth). Holding Shift breaks symmetry (cusp).
- Palette tab count rises to ~5; read `specs/roadmap.md` §I11 guidance before
  finalising the label set.

## Acceptance gates

- [ ] Old v7 `WarpMesh` projects load as `BezierMesh` with all handles `None`;
      render output is pixel-identical to pre-migration.
- [ ] `CURRENT_SCHEMA_VERSION` bumped to 8; `migrate_v7_to_v8` present in
      `src/project/migrate.rs`.
- [ ] Proptest: bilinear-equivalent `BezierMesh` round-trips losslessly.
- [ ] `build_bezier_vertices` passes naga WGSL validation at `cargo build`
      (no shader changes needed; CPU tessellation only).
- [ ] Bezier warp on a curved column produces a clean wrap without visible
      mesh banding (manual acceptance: photograph the test surface).
- [ ] Undo (`Cmd-Z`) of an anchor move and a handle move both restore the
      previous position exactly.
- [ ] Handle visual renders correctly in the overlay; `O` toggle hides it.

## Out of scope

- Catmull-Clark or B-spline as alternative basis (rejected above).
- Per-patch crease/sharp-edge tagging (deferred; adds schema complexity
  not needed for the curved-column use case).
- GPU-side tessellation (deferred; CPU tessellation at sub=8 is fast enough
  for interactive use on M-series hardware).
- More than 4×4 mesh resolution as the default (still N×M; Bezier is an
  extension of the existing model, not a replacement for large-mesh work).
