# Decision: SVG path extraction strategy for Effect::LightTrail

**Status:** Decided — T1.2 implements against this plan.
**Affects:** T1.2 (polyline builder), T1.3 (GPU buffer), T1.4 (failure tests), T2.3 (render dispatch).

---

## Background

`Effect::LightTrail` needs a parametric representation of the source SVG's path geometry
to drive arc-length sampling, comet-head positioning, and tangent computation. rmap has
no existing parametric path infrastructure.

The prompt names `src/image_layer.rs` as "where SVG layers are loaded today." That file
handles raster images (PNG/JPG/WEBP/GIF) only. The actual SVG load and rasterization path
is `src/svg_layer/worker.rs::rasterize_one` — `usvg::Tree::from_str` at line 148. This
decision corrects that reference for downstream tasks.

---

## Decision 1 — Where to extract path data from the SVG

### Options

**Option A — Re-parse the source SVG at effect-load time (chosen)**

Call `usvg::Tree::from_str(text, &usvg::Options::default())` on the same file the SVG
layer loads, then walk the usvg tree to collect path geometry. This is independent of the
rasterization job and runs on the calling thread at effect-load time (not per-frame).

**Option B — Tap the existing SVG layer load path**

Extend `SvgLayer` (or the off-thread worker) to emit parsed path data alongside the
`tiny_skia::Pixmap`. This would require threading the `usvg::Tree` out of the worker, or
adding a second result type to `RasterDone`.

### Chosen path: Option A

**Justification.** The off-thread worker (`svg_layer/worker.rs`) is scoped to a single
concern: rasterize as fast as possible and return a `Pixmap`. Adding path-extraction there
couples two unrelated concerns, forces a new channel type, and creates an ordering hazard —
the effect could be added to a layer whose worker job hasn't returned yet. Re-parsing at
effect-load time is decoupled, happens once, and the source file is already on disk.
`usvg = "0.47"` is a direct dependency (verified in `Cargo.toml`), so no new crates are
needed. Re-parse is the approach explicitly preferred by the prompt (§3: "Re-parse the
source SVG file at effect-load time is preferred").

### Exact API call chain (verified against `usvg-0.47.0` source)

```
usvg::Tree::from_str(text: &str, &usvg::Options::default())
    -> Ok(tree)

tree.root()                          // -> &usvg::Group
    .children()                      // -> &[usvg::Node]
    // depth-first walk of the full node tree:
    // match each Node::Path(p) ->
    //   p.abs_transform()           // usvg::Transform — path local → SVG user-space
    //   p.data()                    // -> &tiny_skia_path::Path
    //   p.data().segments()         // -> PathSegmentsIter<'_>
    //   yields tiny_skia_path::PathSegment::{MoveTo, LineTo, QuadTo, CubicTo, Close}
```

Coordinates from `p.data()` are in path-local space. Apply `p.abs_transform()` before
accumulating so all segments land in SVG user-space — the same space
`tree.root().abs_bounding_box()` reports and that `raster_uniform_fit_transform` operates
on. This is the alignment required for the trail to match the rasterized layer image.

`tiny_skia_path` is re-exported by `usvg` at `usvg::tiny_skia_path` (verified:
`usvg-0.47.0/src/tree/mod.rs:13 pub use tiny_skia_path;`). No `kurbo` dependency.

---

## Decision 2 — Multi-`<path>` SVG handling

### Options

- **Pick first path encountered in DFS walk** — ignores `path_index`.
- **Pick longest path by arc-length** — non-deterministic ordering; expensive to compute
  before we have a polyline.
- **Concatenate all paths in document order** — the comet teleports at subpath boundaries
  where `MoveTo` re-positions the cursor without an intervening `Close`.
- **Expose `path_index: u32` parameter (chosen)** — prompt §4 already requires this field
  with default 0. This is therefore the constrained answer, not a free choice.

### Chosen path: path_index parameter

**Walk order:** DFS through the usvg node tree, collecting only `Node::Path` nodes (skip
`Node::Group`, `Node::Image`, `Node::Text`). The index in the resulting list maps to
`path_index`. `<use>` elements are resolved by usvg's parser pipeline before the tree is
returned, so resolved path copies appear as ordinary `Node::Path` nodes — no special
handling needed.

**Out-of-range handling:** if `path_index >= paths.len()`, clamp to the last valid index
and emit `tracing::warn!(…, "path_index out of range, clamped to last path")`. Do not
treat it as a no-op; the operator most likely typed the wrong number rather than wanting
silence. A zero-path SVG (empty node tree, or a file with no `Node::Path` at all) is
distinct: log `tracing::warn!` and return `None`, which becomes a no-op render per §3.

**Concatenating is rejected:** a multi-subpath concatenation causes the comet to teleport
at `MoveTo` boundaries because arc-length distance between the two subpaths is undefined.
Visible behavior is a jump to an unrelated screen position.

---

## Data structure handed to T1.2

```rust
/// Path segments in absolute SVG user-space, ready for arc-length sampling.
/// Produced by the extraction function; consumed by T1.2's polyline builder.
pub struct ExtractedPath {
    /// Segments with coordinates already in SVG user-space (abs_transform applied).
    pub segments: Vec<tiny_skia_path::PathSegment>,
    /// Total count of Node::Path elements found in the SVG (for range-clamping
    /// path_index and surfacing in tracing::warn).
    pub path_count: usize,
}
```

T1.2 converts `ExtractedPath::segments` into the arc-length-parameterized polyline
(`Polyline { points, cumulative_arclen, total_length }` per T1.2's spec). T1.1 does not
build the polyline.

---

## Module home

New module: `src/path_geom/mod.rs`.

Rationale: path extraction and polyline math are a new concern, distinct from SVG
rasterization (`src/svg_layer/`) and raster-image loading (`src/image_layer.rs`).
Putting it in `src/svg_layer.rs` would conflate geometry extraction with rasterization;
putting it in `src/effects/light_trail.rs` would bury reusable infrastructure inside
a single effect. A `src/path_geom/` module signals that this infrastructure is
available to future effects (e.g. a motion-path effect).

---

## Follow-up risks for T1.2 and T3.x

**Risk 1 — Text-converted-to-path and font loading.**
`usvg::Options::default()` does not load system fonts. SVG files where `<text>` elements
are not pre-converted to `<path>` (i.e., the SVG contains actual `<text>` tags) will
parse successfully but produce zero `Node::Path` hits for those elements. The node tree
will hold `Node::Text` nodes instead; these are invisible to the path extractor.
Operators must ensure their source SVGs use outlined/converted paths for any text they
want the trail to follow. Document in field doc-comment on `path_index`.

**Risk 2 — SVG user-space → render-target-space mapping.**
The extracted path is in SVG user-space coordinates. The rasterizer maps SVG user-space
into the oversample pixmap via `raster_uniform_fit_transform` (a uniform scale + centering
letterbox). The GPU shader in T3.x must apply the same mapping to the polyline points so
the trail aligns with the rendered layer image. T1.2 should emit the polyline in
SVG user-space (not normalized 0..1 UV), and T1.3 / T3.x must document the coordinate
space explicitly. Failure to apply the transform produces a trail that is correctly shaped
but offset or scaled relative to the visible SVG.
