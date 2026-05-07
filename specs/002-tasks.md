# Tasks: rmap v2 — Direct-Manipulation Scene Editor

> Companion to `specs/002-direct-scene-editor.md` (the *what*) and the
> existing `specs/roadmap.md`. Same conventions as `001-tasks.md`:
>
> - **ID format**: `T-<milestone>-<NN>` for milestone tasks; `D-NN` for
>   open decisions.
> - **Estimate**: S / M / L (under ½ day / ½–1 day / 1–2 days).
> - **Acceptance**: every task lists a verifiable check.
> - **Depends on**: hard dependencies. Cross-milestone dep: complete
>   the previous milestone first.
> - **Plan ref**: section in the spec that holds context.

---

## Index

| Bucket | Count | Total estimate |
|---|---:|---:|
| Decisions (D) | 2 | — |
| M8 — Image layers | 5 | 2 days |
| M9 — Live preview | 3 | 1.5 days |
| M10 — Layer manipulation | 5 | 2.5 days |
| M11 — Warp + mask + source-rect manipulation | 6 | 3 days |
| M12 — Zone palette | 3 | 1 day |
| M13 — Polish & schema migration | 4 | 1 day |
| **Total tracked v2 work** | **28** | **~11 days** |

---

## Decisions (D) — open

### D-04 — Cross-window texture sharing for the live preview

- **Spec ref**: §M9, "Live preview".
- **Question**: Register the existing `warp_rt` texture with `egui-wgpu`'s
  renderer (`Renderer::register_native_texture`), or per-frame copy
  `warp_rt` into a CPU staging buffer and re-upload as a new egui
  texture?
- **Trade-off**: Native registration shares one wgpu texture across the
  output and control windows — same Device, no copy, ~zero overhead.
  Risk: egui-wgpu cross-window-Renderer texture sharing is undocumented
  in 0.34; may require `egui-wgpu` 0.35+ or a small upstream patch.
  CPU copy is universally portable but adds one MiB-class memcpy per
  frame.
- **Recommendation**: try native registration first; if it fails after
  ½ day, fall back to CPU copy. Document either choice in the M9 PR.
- **Unblocks**: T-M9-01.

### D-05 — Mask polygon vertex add / delete gesture

- **Spec ref**: §"Mouse manipulation in the preview".
- **Question**: "double-click an edge to insert vertex / shift-click a
  vertex to delete" vs. a right-click context menu?
- **Trade-off**: Direct gestures are faster for an experienced operator
  but invisible to a first-timer; context menus are discoverable but
  slow. In MadMapper / Resolume the gesture pattern is standard.
- **Recommendation**: ship the gesture pattern; add a one-line
  "right-click for help" hint in the sidebar that pops a tooltip
  with the gesture cheatsheet.
- **Unblocks**: T-M11-04.

---

## M8 — Image layers

Goal (spec §1, §M8): a `LayerKind::Image` variant that loads JPG/PNG via
the `image` crate, uploads to a wgpu texture, and renders through the
existing compositor / effects / warp chain unchanged.

### T-M8-01 — `LayerKind` enum + schema migration v2 → v3

- **Files**: `src/project/schema.rs`, `src/project/migrate.rs`
- **Scope**: Add `pub enum LayerKind { Svg { svg_path: PathBuf },
  Image { path: PathBuf, fit: FitMode, focal: [f32; 2] } }` and
  `FitMode { Cover, Contain, Stretch }`. Default `LayerConfig.kind`
  to `Svg { svg_path }`. `CURRENT_SCHEMA_VERSION = 3`.
  Migration v2 → v3: each `layers[i]` gets `kind = Svg { svg_path }`
  synthesized from the existing `svg_path` field; the old field is
  preserved for round-trip but new code reads `kind` exclusively.
- **Acceptance**: Unit test
  `project::migrate::tests::project_v2_migrate_synthesizes_layer_kind`
  passes. Existing `project_round_trip` continues to pass.
- **Estimate**: M.

### T-M8-02 — `image::open` → wgpu texture upload helper

- **Files**: `src/svg_layer.rs` (rename to `layer.rs` in T-M8-04) or
  a new `src/image_layer.rs`
- **Scope**: `fn upload_image_rgba8(device, queue, path) -> Result<(Texture, TextureView, (u32, u32))>`. Reads via `image::open`,
  converts to `Rgba8UnormSrgb`, uploads via `queue.write_texture`.
  No oversampling (image is already raster); preserve native
  resolution capped at 4096×4096 to avoid GPU OOM on huge JPGs.
- **Acceptance**: Unit test `image_layer::tests::load_smoke` loads a
  small fixture PNG, asserts non-zero dimensions.
- **Depends on**: T-M8-01.
- **Estimate**: M.

### T-M8-03 — Render image layers through the existing compositor path

- **Files**: `src/app.rs` (`rebuild_layers` + `render_m5_pipeline`)
- **Scope**: Branch on `LayerConfig.kind`: SVG path keeps the worker /
  raster / upload pipeline; Image path uploads once at `rebuild_layers`
  time, no worker, no rasterizer. Both produce a `wgpu::TextureView`
  the existing `SvgLayerPipeline.render` blits onto the effect chain.
  The pipeline name is fine — it's already a textured-quad blitter.
- **Acceptance**: Manual: load a project with one Image layer (hand-
  edited JSON for now); verify it renders. T-M8-05 covers
  the drop-to-add path.
- **Depends on**: T-M8-02.
- **Estimate**: M.

### T-M8-04 — Fit mode + focal point in the texture sample

- **Files**: `src/render/shaders/textured_quad.wgsl`,
  `src/svg_layer/render.rs` (or wherever the textured-quad pipeline
  lives)
- **Scope**: Add a per-layer uniform with `fit_mode: u32`,
  `aspect_layer: f32` (texture aspect), `aspect_quad: f32` (target
  quad aspect — generally 1.0 for the pre-effects pass), and
  `focal: vec2<f32>`. Shader picks the right UV mapping per fit mode.
  Cover crops, Contain letterboxes, Stretch passes through.
- **Acceptance**: Visual on projector — a 16:9 JPG layer in `cover`
  mode crops to a square render target without horizontal stretch;
  same image in `contain` mode shows letterbox bars.
- **Depends on**: T-M8-03.
- **Estimate**: M.

### T-M8-05 — Drag-drop file → new layer

- **Files**: `src/app.rs`, `src/windows/control.rs`
- **Scope**: Wire `WindowEvent::DroppedFile(PathBuf)` on the control
  window. Inspect file extension: `.svg` → `LayerKind::Svg`; `.png`,
  `.jpg`, `.jpeg` → `LayerKind::Image` with `fit = Cover, focal = [0.5, 0.5]`.
  Push the new `LayerConfig` and trigger `rebuild_layers`.
- **Acceptance**: Manual: drop a JPG onto the control window; new
  layer appears.
- **Depends on**: T-M8-03.
- **Estimate**: S.

---

## M9 — Live preview

Goal (spec §2, §M9): the existing `warp_rt` offscreen texture surfaces
inside the control window's egui as an aspect-correct preview that
updates every frame.

### T-M9-01 — Resolve D-04 + register `warp_rt` as an egui texture

- **Files**: `src/windows/control.rs`, `src/app.rs`
- **Scope**: Per **D-04**: try `egui_renderer.register_native_texture(
  &device, &warp_rt_view, FilterMode::Linear) -> TextureId`. If
  `egui-wgpu` 0.34 doesn't expose this for cross-viewport reuse,
  fall back to per-frame `copy_texture_to_texture` into a dedicated
  egui-owned texture. Document the choice in the PR.
- **Acceptance**: A new `Scene` tab placeholder paints the egui
  texture (any color; correctness comes in T-M9-02).
- **Depends on**: —
- **Estimate**: M (½ day if native works; full day with the fallback).

### T-M9-02 — `Scene` tab with aspect-correct fitted preview

- **Files**: `src/windows/control_panel.rs`
- **Scope**: New `ControlTab::Scene`, defaulting to selected on first
  show. `egui::Image::new(scene_tex_id, fit_to_egui_rect)` with the
  output aspect computed from `output.config.{width, height}`. Display
  letterbox bars at the panel edges if the window aspect mismatches.
- **Acceptance**: Manual: control window shows a live mini of the
  projector at its native aspect.
- **Depends on**: T-M9-01.
- **Estimate**: S.

### T-M9-03 — Frame-rate-limit the preview to 30 fps

- **Files**: `src/windows/control.rs`
- **Scope**: The preview draws every redraw of the control window
  (currently every vsync per `about_to_wait`). For wedding-rig CPU
  budgets, halve the control-window redraw rate via a frame counter.
  Output window stays at vsync; preview at ~30 fps.
- **Acceptance**: Manual: `RUST_LOG=trace` shows preview-frame logs at
  ~30 Hz; output frames at ~60 Hz.
- **Depends on**: T-M9-01.
- **Estimate**: S.

---

## M10 — Layer manipulation

Goal (spec §3, §M10): clicking a layer in the preview selects it; drag
moves; modifier-drag scales / rotates.

### T-M10-01 — `SceneEditorState` + selection enum

- **Files**: `src/windows/scene_editor.rs` *(new)*, `src/app.rs`
- **Scope**: Add `pub struct SceneEditorState { selected: Option<Selection>,
  drag: Option<DragSession> }` and `pub enum Selection { Layer(usize),
  WarpCorner { warp: usize, r: usize, c: usize }, MaskVertex { warp:
  usize, idx: usize }, SourceRect { warp: usize, corner: SourceRectCorner } }`.
  Store on `RunningApp.scene_editor`.
- **Acceptance**: `cargo check`; consumed by T-M10-02 onward.
- **Depends on**: —
- **Estimate**: S.

### T-M10-02 — Hit-test layers in normalized output space

- **Files**: `src/windows/scene_editor.rs`
- **Scope**: `fn hit_layer(project: &Project, screen_pos_in_preview: Vec2,
  preview_rect: Rect) -> Option<usize>`. Walks `project.layers` in reverse
  draw order (top-most first), transforms preview coords to normalized
  output coords, asks the layer's static `Transform` whether the point
  is inside its post-transform unit quad.
- **Acceptance**: Unit test
  `scene_editor::tests::hit_test_centers_select_top_layer` passes.
- **Depends on**: T-M10-01.
- **Estimate**: M.

### T-M10-03 — Drag-translate selected layer

- **Files**: `src/windows/scene_editor.rs`
- **Scope**: When `Selection::Layer(idx)` is active and the operator
  drags inside the preview, accumulate `delta_normalized` from
  `start_screen` and apply to `project.layers[idx].transform.translate`.
  Use the snapshot pattern (`DragSession.start_value`) so live drag is
  "start + delta".
- **Acceptance**: Manual: select a layer, drag; projector follows.
- **Depends on**: T-M10-02.
- **Estimate**: M.

### T-M10-04 — Shift-drag scale, Alt-drag rotate

- **Files**: `src/windows/scene_editor.rs`
- **Scope**: Modifier-key gating in the drag handler. Shift = uniform
  scale around the layer's *opposite* corner from the cursor; Alt =
  rotation about the layer center. Apply to
  `project.layers[idx].transform.{scale, rotate_deg}`.
- **Acceptance**: Manual: shift-drag scales, alt-drag rotates.
- **Depends on**: T-M10-03.
- **Estimate**: M.

### T-M10-05 — Sidebar properties for selected element

- **Files**: `src/windows/control_panel.rs`
- **Scope**: When something is selected, the right-side panel shows
  numeric editors for the selected element's fields (layer:
  translate / rotate / scale / opacity / blend mode; warp corner: just
  the (x, y) of the selected grid point). ESC clears selection.
- **Acceptance**: Manual: select layer; sidebar shows + edits its
  fields. Press ESC; sidebar clears.
- **Depends on**: T-M10-01.
- **Estimate**: M.

---

## M11 — Warp + mask + source-rect manipulation

Goal (spec §3, §M11): move T-M5-08's corner drag handles into the
preview; add mask vertex CRUD; source-rect corner editing.

### T-M11-01 — Migrate T-M5-08 corner drag onto the live preview

- **Files**: `src/windows/scene_editor.rs`,
  `src/windows/control_panel.rs`
- **Scope**: The Mapping-tab dragger keeps working as a fallback, but
  the same drag logic now also fires from inside the Scene tab. Hit-
  test priority: warp corners > mask vertices > source rect > layer body.
- **Acceptance**: Manual: drag a warp corner inside the Scene tab;
  projector follows.
- **Depends on**: T-M10-01, T-M9-02.
- **Estimate**: M.

### T-M11-02 — Mask vertex hit-test + drag

- **Files**: `src/windows/scene_editor.rs`
- **Scope**: Render mask polygon in the preview as a connected line
  loop with vertex handles. Hit-test handles; drag-translates the
  vertex in `WarpMesh.mask_polygon[idx]`.
- **Acceptance**: Manual: drag a mask vertex; SDF re-bake fires;
  projector mask follows.
- **Depends on**: T-M11-01.
- **Estimate**: M.

### T-M11-03 — Insert mask vertex by double-clicking an edge

- **Files**: `src/windows/scene_editor.rs`
- **Scope**: Per **D-05**. On double-click within `INSERT_HIT_PX` of
  any mask polygon edge, insert a new vertex at the click point in the
  correct list position, between the two endpoints.
- **Acceptance**: Manual: double-click an edge; new draggable vertex
  appears; mask updates next frame.
- **Depends on**: T-M11-02.
- **Estimate**: S.

### T-M11-04 — Delete mask vertex with shift-click

- **Files**: `src/windows/scene_editor.rs`
- **Scope**: Per **D-05**. Shift-click a vertex handle removes it from
  `mask_polygon`. Refuse if the polygon would drop below 3 vertices
  (degenerate; SDF baker treats <3 as "no mask").
- **Acceptance**: Manual: shift-click vertex; gone. Try to delete down
  to 2; refused with a tooltip.
- **Depends on**: T-M11-02.
- **Estimate**: S.

### T-M11-05 — Source rect corner drag

- **Files**: `src/windows/scene_editor.rs`
- **Scope**: Render the four source-rect corners (currently invisible)
  as a different-colored handle set inside the preview. Drag updates
  `WarpMesh.source_rect = [x, y, w, h]` in normalized composite-space.
- **Acceptance**: Manual: drag a source-rect corner; warp samples
  a different region of the composited frame.
- **Depends on**: T-M11-01.
- **Estimate**: M.

### T-M11-06 — Selection priority when handles overlap

- **Files**: `src/windows/scene_editor.rs`
- **Scope**: Codify the priority list (warp corner > mask vertex >
  source rect > layer body) in `pick_at(screen_pos) -> Option<Selection>`.
  Cover with unit tests for each priority pair.
- **Acceptance**: Unit tests pass; clicks on overlapping handles select
  the higher-priority element.
- **Depends on**: T-M11-01..05.
- **Estimate**: S.

---

## M12 — Zone palette

Goal (spec §4, §M12): a dropdown of curated mask polygon templates so
the operator can drop a starter shape and drag-edit it.

### T-M12-01 — Built-in zone templates

- **Files**: `src/project/zone_templates.rs` *(new)*
- **Scope**: Pure-Rust functions returning starter `mask_polygon`
  shapes:
  - `window_rectangle()` — a tall rectangle centered in the warp.
  - `arch_portal()` — bottom-aligned arch (rectangle + half-circle
    sampled at ~24 vertices).
  - `circle_spotlight()` — circle at warp center.
  - `void_block()` — square cutout (operator drag-edits to overlap
    the projection-undesired area).
- **Acceptance**: Unit tests verify each template returns at least
  3 vertices and is contained in `[0, 1]^2`.
- **Estimate**: S.

### T-M12-02 — Zone palette dropdown in the Scene tab

- **Files**: `src/windows/control_panel.rs`
- **Scope**: A dropdown above the preview: "Add zone:" → window-
  rectangle / arch-portal / circle-spotlight / void-block. On
  selection, replaces `WarpMesh.mask_polygon` with the chosen template.
- **Acceptance**: Manual: click a zone preset; mask appears; drag-edit
  it via M11 vertex handles.
- **Depends on**: T-M12-01, T-M11-02.
- **Estimate**: S.

### T-M12-03 — Mask reset / clear button

- **Files**: `src/windows/control_panel.rs`
- **Scope**: A "Clear mask" button next to the zone dropdown. Sets
  `mask_polygon = vec![]`, which the SDF dispatcher already treats
  as "no mask".
- **Acceptance**: Manual: click clear; mask gone.
- **Depends on**: —
- **Estimate**: S.

---

## M13 — Polish & schema migration

Goal: schema v3 lands cleanly; old projects still load; v1 features
still work end-to-end after the editor changes.

### T-M13-01 — `project_v2_migrate_to_v3` test

- **Files**: `src/project/migrate.rs`
- **Scope**: Construct a v2 JSON snapshot (with `schema_version: 2`,
  `layers: [{ svg_path: ... }]`); call `migrate`; assert it deserializes
  with `kind = Svg { svg_path }` and `schema_version = 3`.
- **Acceptance**: `cargo test project_v2_migrate_to_v3` passes.
- **Depends on**: T-M8-01.
- **Estimate**: S.

### T-M13-02 — Selection-state-not-saved invariant

- **Files**: `src/project/mod.rs`
- **Scope**: Document that `SceneEditorState` is runtime-only; save /
  load round-trip a project with selection set in v2 — confirm the
  loaded project has no `selected` artifact in JSON.
- **Acceptance**: Unit test asserts `serde_json::to_value(project)`
  contains no key starting with `selected`.
- **Estimate**: S.

### T-M13-03 — Update show-day checklist for drop targets

- **Files**: `docs/show-day-checklist.md`
- **Scope**: One paragraph: "drag SVG / JPG / PNG onto the control
  window's Scene tab to add a layer; drop on the projector window
  is ignored."
- **Acceptance**: Reads in 30 seconds; doesn't grow the checklist
  past one page.
- **Estimate**: S.

### T-M13-04 — End-to-end smoke: scenes + crossfade + presets after editor

- **Files**: manual / `docs/m13-smoke.md` *(new, optional)*
- **Scope**: Run through: (a) drop two photos, (b) save scene 1,
  (c) reposition both, (d) save scene 2, (e) recall scene 1 via
  hotkey with `crossfade_duration_s = 1.5`, (f) confirm fade is
  smooth. Catches any "interactive editor mutated state in a way
  that breaks scene snapshot round-trip" bug.
- **Acceptance**: A short report committed under `docs/`.
- **Estimate**: S calendar.

---

## Appendix — Estimate roll-up

| Milestone | Tasks | S | M | L | Total est. |
|---|---:|---:|---:|---:|---:|
| Decisions | 2 | — | — | — | — |
| M8 | 5 | 1 | 4 | 0 | 2 days |
| M9 | 3 | 2 | 1 | 0 | 1.5 days |
| M10 | 5 | 1 | 4 | 0 | 2.5 days |
| M11 | 6 | 3 | 3 | 0 | 3 days |
| M12 | 3 | 3 | 0 | 0 | 1 day |
| M13 | 4 | 4 | 0 | 0 | 1 day |
| **v2 total** | **28** | **14** | **12** | **0** | **~11 days** |

Cross-checks against `specs/roadmap.md` Phase 1 + Phase 2 + the
direct-manipulation goal: aligned. v2 specifically does not include
Phase 4 (lighting) or Phase 6 (broader interop).

---

*End of v2 tasks. Cross-references: spec → `002-direct-scene-editor.md`,
roadmap → `roadmap.md`, this file → who picks up which work.*
