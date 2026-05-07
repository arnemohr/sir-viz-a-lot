# Spec: rmap — Direct-Manipulation Scene Editor (v2)

> Successor to `specs/001-initial-setup.md` (v1.1). v1 shipped a renderer-
> centric pipeline with sliders + dropdowns; v2 turns the control window
> into a **scene editor** where every graphical element — layers, warp
> corners, masks — is grabbable, draggable, and stretchable directly inside
> a live preview of the rendered scene.

## Goal

Make the rmap control window a place an operator *composes* in, not a place
where they hunt for sliders. The principal v2 surface is one panel that
shows the current rendered output and accepts mouse manipulation on every
visible element. Per the roadmap (`specs/roadmap.md`):

> The roadmap is working if the product reaches these outcomes:
> A user can create a beautiful photo-driven mapped scene in minutes, not hours.

v2 is the work that makes the first half of that statement true.

## Why this exists

v1 proves the rendering core (compositor, warp, mask SDF, effects,
modulators, scenes, project I/O). It does not yet **author beautifully**.
Today the operator picks an SVG, types a path, clicks "Add layer", and
fiddles four corner sliders to map the layer onto a wall. v2 replaces
*every* one of those moments with direct manipulation: drag the layer
to translate it, drag the corner to keystone it, drag the mask polygon
to define a window, drop a JPG to add a photo layer.

This sets up the roadmap's Phase 1 (photo-first media), Phase 2
(authored zones), and Phase 3 (scene grammars) without committing to
those scopes yet — the editor itself is the foundation.

## Non-goals (v2)

- **No new render passes**: v2 is UI/UX work over the v1 pipeline.
  Photos are an additive layer type but use the existing compositor +
  effects + warp chain.
- **No node-graph effect editor**. The roadmap explicitly defers
  "deep generic shader graph authoring."
- **No multi-projector geometry calibration**. Single-surface still.
  T-M7-02 already supports multiple warps within one projector; v2
  adds direct-mouse editing for them.
- **No lighting outputs (Art-Net / sACN)**. Roadmap Phase 4; out of v2.
- **No live audio / MIDI / OSC Param-binding UI**. The plumbing landed
  in M7; the binding UX is v2.5+.
- **No AI / facade detection / auto-mapping**.

## Core capabilities

### 1. Image (raster) layer type

A new `LayerKind::Image { path }` variant alongside the existing SVG layer.
Loaded via the `image` crate (already a dep), uploaded as RGBA8 to a wgpu
texture once at load (or on hot-reload), composited via the existing
SvgLayerPipeline (which is really a "textured-quad" pipeline; v2 renames
it). Supports JPG and PNG; HEIC and TIFF are out.

Image-specific concerns:

- **Fit modes**: `cover` (fill, crop), `contain` (fit, letterbox),
  `stretch` (no aspect lock). Stored on `LayerConfig`.
- **Focal point**: a normalized `[u, v]` per layer that the cover crop
  centers on. Default `[0.5, 0.5]`.
- **No global tone-mapping** in v2 — color/brightness/contrast already
  exist as per-layer effects.
- **Hot-reload**: same `notify`-debouncer path as SVG.

### 2. Live scene preview in the control window

A new "Scene" tab (or panel) renders the current output framebuffer as
an egui texture inside the control window, scaled to fit, at the
project aspect ratio. Updated every frame.

This requires cross-window texture readback: the output window's swap
chain texture (or, more reliably, the warp_rt_view offscreen texture)
is sampled into a CPU staging buffer or into an egui-managed texture
on the same `wgpu::Device`. Implementation choice: use the existing
warp_rt texture and register it with `egui-wgpu` so egui samples it
directly without a CPU copy.

### 3. Mouse manipulation in the preview

The preview is the scene editor. Hit-testing converts cursor pos →
normalized output-space coords; selection picks the topmost
hit-tested element; drag updates the underlying `Project` field.

**Selectable elements:**

- **Layer body** (anywhere inside its current quad after Transform).
  Drag → `Transform.translate`. Shift-drag → uniform scale around
  pinned corner. Alt-drag → rotate about layer center.
- **Warp corners** (per-warp `WarpMesh.grid` control points). Drag →
  update grid in place (T-M5-08 logic; same code, new viewport).
- **Mask polygon vertices** (each `WarpMesh.mask_polygon[i]`). Drag a
  vertex; shift-click to delete; double-click an edge to insert a
  new vertex.
- **Layer source rect** (per-warp `WarpMesh.source_rect`). The
  "what part of the composited frame this warp shows" — currently
  edited only via JSON. Adds a second handle pair on the preview
  for source-rect corners.

**Selection model:**

- One element selected at a time (for v2; multi-select is future).
- Sidebar shows the selected element's properties for fine-grained
  numeric editing.
- ESC clears selection.

### 4. Predefined mask zones (semantic palette)

Per roadmap Phase 2 ("authored spatial behaviors"), a small dropdown of
named mask polygon templates: `window-rectangle`, `arch-portal`,
`circle-spotlight`, `void-block`. Each one drops a starter polygon into
`WarpMesh.mask_polygon` that the operator then drag-edits.

This is *not* a generic shape library — it's a curated set of four
to six wedding-relevant zones. The roadmap's "small semantic palette"
rule constrains the count.

### 5. Asset drop targets

Drop an SVG / JPG / PNG file onto the Scene preview. Adds a new layer
of the appropriate kind, sized to its native aspect, centered, with
the default effect chain. No type field selection — file extension
decides.

## Technical design

### Render-pipeline integration

The v1 pipeline (`render_m5_pipeline`) is unchanged. v2 adds:

- A second `Compositor`-style pass for the editor's "selection overlay"
  (handles, edges, text labels). Drawn into a separate offscreen
  texture by egui itself, then composited over the preview by egui's
  layer system.
- The preview texture is one `wgpu::TextureView` of the existing
  `warp_rt_view`. Egui's `Renderer::register_native_texture` (already
  used internally for fonts) wraps it as an `egui::TextureId`.

### Data model deltas

- `LayerConfig.kind: LayerKind` enum, default `Svg { svg_path: PathBuf }`.
- `LayerKind::Image { path: PathBuf, fit: FitMode, focal: [f32; 2] }`.
- `FitMode` enum: `Cover` (default for Image), `Contain`, `Stretch`.
- Schema version bumps to 3. Migration v2 → v3: synthesize `kind`
  from old `svg_path` field for any v2 layer.

### Direct-manipulation backend

Selection state lives on `RunningApp.scene_editor: SceneEditorState`:

```rust
pub struct SceneEditorState {
    pub selected: Option<Selection>,
    pub drag: Option<DragSession>,
}
pub enum Selection {
    Layer(usize),
    WarpCorner { warp: usize, r: usize, c: usize },
    MaskVertex { warp: usize, idx: usize },
    SourceRect { warp: usize, corner: SourceRectCorner },
}
pub struct DragSession {
    pub start_screen: Vec2,
    pub start_value: serde_json::Value,
}
```

Drag stores the **starting value** as a snapshot so the live drag is
"start + delta" (cumulative drag math is the source of float drift bugs
in interactive editors). On drag end, fire `ControlPanelAction` to
clear the session.

### Hit-testing

Hit-testing transforms screen coords → preview-space → normalized
output-space → project-space. For each element type:

- **Layer body**: hit-test the post-`Transform` quad in normalized space.
  Transform is from `LayerConfig.transform` + the per-effect Transform
  at evaluation time. Selection uses the *static* `LayerConfig.transform`
  + the *static* Transform effect anchor (modulators not factored —
  selection is operator-edit-time, not animation-time).
- **Warp corners**: hit-test a circle of radius `HANDLE_HIT_RADIUS_PX`
  around each `WarpMesh.grid[r][c]` in preview space.
- **Mask vertices / source rect**: same circle hit-test.

### Z-order / topmost selection

Picking returns the topmost match in this priority:
1. Warp corners (small, precise — pick first if hit).
2. Mask vertices.
3. Source rect corners.
4. Layer bodies (largest hit area, picked last).

### File-drop integration

`winit::WindowEvent::DroppedFile(PathBuf)` already fires when a file is
dropped on a `winit` window. Wire on the **control window** (so the
operator drops onto the preview, not the projector).

## User flow

1. Operator launches `rmap path/to/show.rmap.json`.
2. Output window fills the projector; control window opens on the
   primary display showing **Scene** tab by default with a live preview.
3. Operator drags a JPG onto the preview. New `Image` layer appears in
   the center, default fit `cover`, default effect chain.
4. Operator clicks the layer; it highlights with a selection box and
   a sidebar shows its properties.
5. Operator drags the layer body to position it; shift-drags a corner
   to scale; alt-drags to rotate.
6. Operator clicks the **window-rectangle** zone preset; a starter
   mask polygon appears over the warp; operator drag-edits each vertex
   to align with the actual window edge in the projection.
7. Operator presses `Cmd-S` (or the existing **Save** button); the
   project file gets the new layer + mask + warp state.

## Milestones

### M8 — Image layers

`LayerKind::Image` + texture upload + fit/focal modes. Hot-reload via
the existing notify pipeline. SVG layers continue working unchanged via
schema migration. Drag-drop adds a layer.

### M9 — Live preview

Scene tab with `warp_rt` registered as an egui texture. Aspect-ratio
preserved scaling. Updated every frame at ≤60 fps.

### M10 — Layer manipulation

Layer-body hit-testing + drag-translate + shift-drag scale + alt-drag
rotate. Selection state + sidebar properties. Esc clears selection.

### M11 — Warp + mask + source-rect manipulation

Move T-M5-08's drag handles onto the live preview (same code; new
canvas). Mask vertex add/move/delete via mouse. Source-rect corners.

### M12 — Zone palette

Curated `window-rectangle`, `arch-portal`, `circle-spotlight`,
`void-block` mask templates accessible from a dropdown on the Scene
tab. Each template populates `mask_polygon` with a starter shape
centered on the layer.

### M13 — Polish & schema migration

Schema v3 migration test. Drop-target docs in `docs/show-day-checklist.md`.
Selection-state save/restore round-trip. Verify all M5–M7 features still
work end-to-end after the editor lands.

## Decisions deferred to plan/tasks

- **D-04 — Cross-window texture sharing**: register the existing
  `warp_rt` texture with `egui-wgpu`'s renderer, or copy each frame
  into a CPU staging buffer and re-upload as an egui texture? Both
  are workable; the plan picks one before any code lands.
- **D-05 — Mask polygon vertex add/delete**: is "double-click an edge
  inserts a vertex / shift-click deletes" the right gesture, or
  should it be a context menu? Resolve in plan.

## Roadmap alignment

| Roadmap phase | v2 contribution |
|---|---|
| Phase 1 — Photos as first-class media | M8 Image layers |
| Phase 2 — Spatial zones | M12 zone palette + M11 mask manipulation |
| Phase 3 — Scene grammars | M9 + M10 + M11 (the editor itself is the substrate) |
| Phase 4 — Lighting outputs | Out of v2 |
| Phase 5 — Show control / cueing | M7 (already shipped); v2 surfaces nothing new |
| Phase 6 — Professionalization | Out of v2 |

## Verification

v2 is "working" when:

- A first-time operator can drop three photos, drag them into a
  rough collage, define a mask zone for the projection wall, and
  save — in under five minutes — without opening any JSON file.
- Existing v1 `*.rmap.json` projects load unchanged after the schema
  v2 → v3 migration.
- Crossfade, scenes, presets, MIDI, OSC, audio modulator continue
  to function exactly as in v1.
