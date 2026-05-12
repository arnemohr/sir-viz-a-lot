# Changelog

All notable changes to rmap are documented here.

---

## [Unreleased]

---

## v0.6.0 — 2026-05-12

v0.6 makes rmap a live FX performance tool. Operators can now pick from
14 built-in procedural presets across three families — Wave, Particle, and
Fluid — directly from an in-app browser. Every change is undoable; the
show-day frame budget is protected by hard particle-count enforcement.

### FX Preset Library

The new preset browser modal (accessible from Advanced → Selected layer →
FX Preset) lists all built-in and user-saved presets. Operators can search
by name, filter by family (Wave / Particle / Fluid / Treatment), and star
presets they use often. The three-click flow — drop a mask, open the
browser, pick a preset — is intentional: no scrubbing through a menu tree.

### Effect-Chain Reordering

The effect chain on every layer type (Image, Video, SVG, FxLayer) is now
drag-reorderable. Effects can also be added and removed with + / − buttons.
`Effect::External` is promoted to a first-class menu entry so operators can
reach custom post-processing effects without editing the project file by hand.

### Particle / Wave / Fluid Families

Fourteen built-in FX presets shipped in v0.6:

**Wave (FxLayer + Treatment)**
- `mask_edge_wave_wash` — animated wave wash along the mask polygon edge.
- `displacement_ripple` (Treatment) — time-varying per-pixel UV displacement
  for a heat-haze / ripple effect over any source layer.
- `refraction` (Treatment) — SDF-normal-based refraction that bends light
  across the mask boundary.

**Particle (FxLayer, GPU compute)**
- `mask_constrained_drift` — particles spawned inside the mask polygon,
  drifting with gentle random walk.
- `mask_edge_emission` — continuous particle emission from the mask edge,
  falling inward.
- `mask_field_flow` — flow-field driven particles; direction sampled from
  a pre-baked noise texture.
- `mask_collision_reflection` — particles bounce off the mask polygon
  boundary with configurable restitution.

**Fluid (FxLayer, GPU compute)**
- `fluid_identity` — Navier-Stokes advection with no forcing; a minimal
  identity baseline for the fluid pipeline.
- `mask_bounded_fluid` — advected velocity field constrained inside the
  mask polygon; renders as a coloured RGBA16Float velocity buffer.

### Export / Import

User-tuned presets can be exported as `.rmap-preset.json` files and
re-imported into any project. The format carries only `preset_id` and
parameter values — no media paths, no warp data — so sharing a preset
between machines requires only the single JSON file.

### Engine

- `SetFxLayerParams` mutation validates `particle_count` against each
  preset's declared `max_particle_count` and refuses to commit when
  over-budget; the UI shows an inline warning and snaps the slider back.
- `FxLayer` schema gains `seed: u64` and `t_layer_added_secs: f32` for
  deterministic particle initialisation; same seed = bit-exact pixel output
  across independent renders.
- `FxParamDescriptor` API lets presets declare parameter names, ranges, and
  defaults; the UI reads descriptors to build sliders generically.
- Project audit now emits `UnknownFxPreset` and `UnknownTreatment` findings
  for any layer whose `preset_id` is not in the registry; findings appear
  in the Diagnostics strip.
- `sample_sdf_normal` WGSL helper available to all FX shaders; returns the
  mask polygon's surface normal at any fragment coordinate.

---

## v0.5.0 — 2026-05-12

v0.5 makes rmap a real photo / video performance tool. Drop an
image or mp4, pick from six **treatment presets** (tone-map,
blur-mask, luminance-reveal, texture-overlay, palette-extract,
2×2 collage), and tune them live. Video gains in/out trim, three
loop modes, BPM-lock, and click-to-seek wiring. Every change is
undoable.

### What changed

**Treatment pipeline (W2)**

- New `TreatmentPipeline` runs *before* the effect chain on every
  layer that carries a `Treatment`. Source → treatment → effects →
  warp → compositor. Unknown preset_id falls back to the default
  blit so a half-configured layer still shows its content.
- `LayerConfig.treatment: Option<Treatment>` (non-bumping serde
  addition on v7); `Treatment { preset_id, params, overlay_path,
  collage_paths }`. Two new mutations: `SetLayerTreatment` (whole-
  Option Reverse) and `SetLayerTreatmentParams` (whole-HashMap
  Reverse). Audit emits warnings for missing assets.
- Selected-layer **Treatment picker UI** in Advanced: combobox
  lists every registered preset, per-param sliders dispatch on
  drag-release (one undo entry per gesture). Preset switch
  preserves shared param keys, falling back to descriptor
  defaults.

**Six treatment presets (W3)**

- `tone_map` — S-curve (exposure / contrast / shoulder rolloff).
  Identity at defaults.
- `luminance_reveal` — Rec. 601 luma → smoothstep threshold
  modulates alpha. Useful for cutouts on bright subjects.
- `blur_mask` — three-pass SDF-gated separable gaussian. Pixels
  near the mask edge get heavy blur, centre stays sharp. Per-
  fragment radius derived from `abs(sdf) → smoothstep`.
- `texture_overlay` — composites an external image asset over
  the source with one of four blend modes (Normal/Multiply/
  Screen/Add), offset, and opacity. Loaded via the shared
  ImageTextureCache.
- `palette_extract` — bit-depth posterise with optional ordered
  Bayer dither. Reduced colour depth. (True k-means palette
  extraction is Phase 7.)
- `collage` — fixed 2×2 grid of up to four slot textures, with
  configurable seam gap. Empty slots fall back to source.

**Video operator surface (W4)**

- Auto-play on drag-drop. Drop an mp4, it plays.
- Speed slider (0.25× — 4× log scale).
- **In/out points** (`clip_in` / `clip_out` seconds). Worker
  sets AVAssetReader `timeRange` before reading; seamless loop
  seeks back to `clip_in`.
- **Loop modes** — Once (stop on EOF) / Loop (seamless, default)
  / Ping-pong (forward-only stub until Phase 7 ships the I-frame
  cache).
- **BPM-lock** — checkbox; when on, effective speed scales with
  the show's clock BPM (120 BPM = identity).
- **Seek** — `VideoControl::SeekTo(seconds)`; worker rebuilds the
  reader at the seek point. Wired end-to-end; the thumbnail-strip
  UI that triggers it on click is deferred to Phase 7 (needs
  AVAssetImageGenerator FFI + egui texture registration).
- **Reverse playback** — falls back gracefully: negative speed is
  logged + clamped to `|speed|`. True reverse needs the same
  Phase 7 keyframe cache as PingPong's second half.

**Image / Video parity (W2.4)**

- `LayerKind::Video` gains `fit` + `focal` fields, parity with
  `Image`. The per-frame render code now honours both variants
  through one shared arm.
- `SetLayerFocal` mutation handles either variant.
- Selected-layer **Source fit** section: fit-mode read-out
  (set on import) + focal-point X/Y sliders when fit == Cover.
  Click-to-set focal on a thumbnail preview is deferred to
  Phase 7 (same texture-registration infra as the thumbnail
  strip).

**Left-rail video row anatomy (W5)**

- Loop-mode glyph (∞ / → / ⇆) overlaid on the thumbnail.
- In/out trim markers along the thumbnail's bottom edge when
  the layer is trimmed (60-second reference window).

**Drag-drop + image cache (W1)**

- `.webp` and `.gif` drops route through the image-layer path
  (GIF: first-frame only).
- `ImageTextureCache` dedupes uploads keyed on `(path, mtime)`;
  wgpu's Texture is Arc-counted, so cache hits clone cheaply.
  Cache lives at session scope.
- EXIF orientation handling: phone-portrait JPEGs land upright.
- Memory bounds (MAX_DIM = 4096) emit a `tracing::warn!` on
  downscale.

**UX fixes**

- Auto-select dropped layer — the just-added layer is the
  Selected-layer panel's target immediately.
- Treatment placeholder names the actual layer kind ("this is a
  SVG layer") with warn colour instead of plain weak text.
- Left rail width 88 → 120 + zero panel inner-margin; label
  truncation via Galley + `max_width` so long filenames don't
  overflow into adjacent rows.

**Diagnostics (W6)**

- Texture-upload drop count aggregated into the diagnostics
  widget (audio + texture-upload summed, tooltip splits the
  breakdown).

**Glossary (W1.3)**

- 13 new glossary entries cover the Phase 1 vocabulary
  (Treatment, ToneMap, BlurMask, LuminanceReveal,
  TextureOverlay, PaletteExtract, Collage, FocalPoint,
  InOutPoints, LoopMode, BpmLockedPlayback, ReversePlayback,
  ThumbnailScrub).

### Deferred to Phase 7

- True reverse playback + PingPong's reverse half — needs the
  I-frame cache.
- Thumbnail strip + click-to-scrub UI — needs
  `AVAssetImageGenerator` FFI and egui texture registration for
  live video / image frames.
- Click-to-set focal on a thumbnail preview — same blocker.
- True k-means palette extraction (operator-named palettes).
- Variable-N collage (currently fixed at 2×2).

---

## v0.4.0 — 2026-05-11

v0.4 is the first multi-projector release. It adds two-output support with
edge-blend, per-projector RGB calibration, an FX layer preset system, OSC +
MIDI binding pickers with MIDI-learn, and a texture-upload queue skeleton for
the pending video integration.

### What changed

**Multi-projector (W7)**

- Launcher multi-output picker lets you assign up to two projectors before
  opening a project; an identify-flash helps confirm which display is which.
- Schema migrated v6 → v7: `output_target` field renamed to
  `output_targets: Vec<OutputTarget>`; existing projects migrate automatically
  on load.
- Second `OutputWindow` lifecycle: per-frame render loop runs passes 1–4 once
  and passes 5–6 per output; closing the second output shrinks the vec; each
  display holds its own sleep assertion.
- Edge-blend overlap rendering: `EdgeBlendConfig` + multiply-blend WGSL let
  you dial in a clean blend across the physical overlap between two projectors.
- Alignment cross and edge-blend gradient test patterns added to the show-day
  toolkit.
- Output mode pill in the toolbar (minimum-viable toggle for windowed/fullscreen
  per output).

**Per-projector colour calibration (W8)**

- `OutputPanel` scaffold surfaces per-projector controls when two outputs are
  active.
- Per-projector RGB matrix render path: a 3×3 matrix is applied in the shader
  per output for white-point and colour-temperature correction.
- RGB matrix editing UI: 3×3 spinner grid, identity reset, non-identity state
  indicator, and a "Calibrate" stub (hardware measurement workflow — Phase 7).

**FX layers (W5)**

- `LayerKind::FxLayer { preset_id, params }` with real fields and mutations;
  SDF helper WGSL is accessible to procedural shaders.
- One preset shipped: `mask_edge_ripple_wash` — applies an animated edge-ripple
  wash against the layer's mask polygon.
- Demo project `assets/demos/fx-ripple-wash.rmap.json` bundled.

**OSC + MIDI bindings (W2)**

- `Modulator::OscBound` and `Modulator::MidiBound` with per-source value
  registries.
- `BindingPicker` + `ParameterRow` UI components; the modulator slider is
  migrated to `BindingPicker`.
- Read-only OSC bindings summary in the Advanced panel.
- MIDI-learn workflow: right-click a parameter → "Learn next MIDI CC" → first
  incoming CC is captured and bound; range-derived scale/offset; action is
  undoable.

**Live-input defaults (W1)**

- `osc` and `midi` cargo features are now default-on; no feature flag required
  to use OSC or MIDI bindings at runtime.
- Schema version bumped v6 → v7 (see Multi-projector above).
- Glossary entries added for new v0.4 domain terms.

**Diagnostics (W3)**

- Thread-safe texture-upload queue skeleton; per-instance dropped-frame counter
  surfaces in the Diagnostics section of the Advanced panel.

### Deferred from v0.4

- **Video playback** — decoder technology selected (AVFoundation via objc2,
  ships with macOS, no system dependency); integration tracking in P0.4.2.
  Schema placeholder `LayerKind::Video` exists and is forward-compatible but
  renders nothing in this release.
- **NDI input** — deferred to v0.5; decision record on file (community `ndi`
  crate). No system dependency in v0.4.
- **Automated frame-budget performance gate** — deferred until the video
  integration lands so the fixture measures the full v0.4 surface; harness
  ships alongside P0.4.2.

---

## v0.3.1 — 2026-05-10

v3.1 is a stabilisation release built on top of the v3 canvas-first editor. It
closes the four deferred audit findings from v3.0, hardens cross-machine project
portability, and ships a set of small operator-facing enhancements (native menu
bar, BPM HUD, layer solo/mute, output thumbnail, audio meter) that were scoped
out of the initial v3 launch.

### What changed

**Operator-facing**

- Native macOS menu bar (`App / File / Edit / Window / Help`) with
  `Cmd-S` / `Cmd-Shift-S` (save / save as), `Cmd-O` (open), `Cmd-Q` (quit),
  `Cmd-Z` / `Cmd-Shift-Z` (undo / redo), and a standard About panel.
- Top-chrome BPM HUD shows live BPM, tap source (Space / MIDI / OSC), and tap
  age. A quantize selector (Off / 1 / 2 / 4 / 8 bars) makes cue recalls wait
  for the next bar boundary; set to Off for immediate fire (bit-identical to
  v3.0 behaviour).
- Layer rows in the left rail gain **Solo (S)** and **Mute (M)** buttons.
  Solo'd layer renders even when also muted; state survives undo/redo and scene
  recall.
- Top-right thumbnail of projector output in the control window. Click to
  focus (or open) the preview-as-projector window. No extra GPU work — reuses
  the existing render texture.
- When an audio source is active, an 8-band FFT meter strip appears above the
  cue strip. (Drag interaction reserved for parameter binding in a future
  release.)
- Two new bundled demos: **Film Strip** (4-frame horizontal photo strip) and
  **Test Grid** (SVG alignment grid + masked image corner verifier). The
  launcher demo picker now lists all three demos.
- Cross-machine project portability: saved shows now record the projector's
  display UUID (`CGDisplayCreateUUIDFromDisplayID`). On load the loader prefers
  a UUID match, falls back to index, and falls back to display 0 with an audit
  warning. A project saved on machine A loads onto the same physical display
  when opened on machine B.

**Bug fixes (deferred from v3.0)**

- Static-value modulator now round-trips bit-exact through save/load
  (T1.36 / V31.1.1).
- `crossfade_duration_s` undo now restores the correct previous value
  (T1.37 / V31.1.2).
- `output_windowed` flip is now tracked in the undo stack (T1.39 / V31.1.3).
- Empty effects-vec snapshots are now round-trip-safe in `snapshot_parity`
  tests (T1.40 / V31.1.4). All four fixes are covered by proptest harnesses.

**Internal / refactor (no operator visibility)**

- All `Mutation` variants now implement a `ReverseStorage` trait at the type
  level. Adding a new mutation variant without specifying its undo behaviour is
  a compile error. Asymmetric exceptions (`AddLayer`/`RemoveLayer` etc.) are
  documented inline. No operator-visible change.

---

## v0.3.0 — v3 UI/UX overhaul (Spec 003)

This release replaces the v2 tabbed control panel with a canvas-first editor.
Every operator-visible change is listed below. See
[`specs/v2-to-v3-migration.md`](specs/v2-to-v3-migration.md) for an
operator-facing diff if you are upgrading from v2.

### What changed

**Canvas merge**
- The separate "control panel" concept is gone. The canvas occupies the main
  window; all controls surface as contextual panels rather than fixed tabs.

**Layer thumbnail strip (left edge)**
- Layers are listed in a vertical thumbnail strip on the left. Clicking a
  thumbnail selects the layer for editing. Drag to reorder.

**Inspector (right edge, context-sensitive)**
- Appears automatically when a layer is selected. Shows fit mode, opacity, and
  the layer's asset path.

**Advanced disclosure panel (right edge, on demand)**
- Opened via the toolbar "Advanced" button or the keyboard shortcut.
- Sections: Master (gamma / brightness / contrast), Display output (per-
  projector tone override), Selected layer (effect chain, blend mode, mapping),
  Project (output mode, save/load), Diagnostics.
- All labels carry a "?" glossary popover explaining the domain term.

**Toolbar (top)**
- Project name with dirty indicator (dot prefix when unsaved).
- Undo / Redo buttons (disabled when stack is empty).
- Save and Save as… buttons.
- Warp mode toggle.
- Preview: opens a floating secondary preview window.
- Go live / Stop: transitions to fullscreen on the projector.
- Glossary: opens the in-app term browser.
- `?`: opens the README in the default browser.

**Per-layer warp, mask, and effects (schema v4/v5)**
- Warp corners are now dragged directly on the canvas (no Mapping tab).
- Mask polygon is drawn on the canvas in Mask edit mode.
- Effects are edited in the Advanced > Selected layer > Effect chain section.

**Show-day strip (bottom)**
- Blackout, Freeze, Test Pattern, and Editor Overlay toggles are always
  visible at the bottom of the control window.
- Button colours reflect active state (accent = active, default = inactive).

**Cue strip (above show-day strip)**
- Visual row of scene tiles with thumbnails and click-to-recall.
- Active scene is highlighted; a progress bar overlays the target tile during
  a crossfade.

**Autosave + dirty tracking**
- Project is autosaved to the current file every 5 minutes when dirty.
- Dirty state is indicated by a dot in the toolbar project name.
- Undo and redo are tracked per-session; autosave does not clear the undo
  stack.

**Launcher**
- First-run window with "Try a demo", "Open a recent show", and "Open…"
  actions. No command-line flags needed to get started.
- Projector dropdown populated with human-readable display names (macOS:
  `NSScreen::localizedName`; other platforms: winit fallback).

**Project audit**
- On load, the project is audited for missing media, schema drift, and
  multi-warp consolidation needs.
- Findings surface as toasts and in the Advanced > Diagnostics section.
- Missing-media layers show a relink button in the inspector.

**Undo / Redo**
- All mutations (warp corner moves, effect changes, scene saves, layer adds)
  are tracked through an undo stack.
- `Cmd-Z` (macOS) / `Ctrl-Z` (Linux/Windows) undoes; `Cmd-Shift-Z` / `Ctrl-Shift-Z`
  redoes.

**Mode-aware editing**
- Layer mode: click to select a layer; drag to move/scale.
- Warp mode: drag individual warp mesh corners.
- Mask mode: click to add polygon vertices; drag to move.
- Inspect mode: read-only; used internally by the inspector panel.

**Glossary popovers**
- Every domain term in the Advanced panel (Warp, Mask Polygon, Modulator,
  Gamma, Blend Mode, etc.) shows a "?" icon. Hovering opens a popover
  with a 1–2 sentence explanation.

**Schema migration**
- v3 and v4 project files are migrated automatically to v5 on load. No manual
  conversion is needed.

### Known deferred items (v3.1)

Four audit findings were deferred to v3.1:
T1.36, T1.37, T1.39, T1.40. See `specs/v3-capability-scope.md` for details.

---

## v0.2.x — v2 tabbed editor

Legacy release. Tabbed control panel with Scene / Effects / Layers / Scenes
tabs. Warp editing in a separate Mapping tab. Numbered scene slots
(no thumbnails). Master gamma as a top-level slider.

No formal changelog was maintained for v0.2.x. The v2 codebase is still
buildable without `--features v3`.
