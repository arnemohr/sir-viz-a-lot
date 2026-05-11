# 004 Phase 1 — task breakdown

Companion task spec for [`004-phase-1.md`](004-phase-1.md). Each task
below is sized for a single PR.

## Implementation status (2026-05-11)

**Shipped:** *(none yet — Phase 1 just opened.)*

**Not yet started:** every task below.

**Carryover from Phase 0:**

- `LayerKind::Image { path, fit, focal }` with `FitMode::{ Cover,
  Contain, Stretch }` and a `focal: [f32; 2]`. Master + per-display
  gamma / brightness / contrast overrides + per-projector RGB matrix.
- `LayerKind::Video { path, speed, loop_seamless }` with the
  AVFoundation worker (P0.4.2b), per-frame `TextureUploadQueue` drain,
  speed slider + seamless-loop checkbox in the Selected-layer panel
  (P0.4.3).
- `LayerKind::FxLayer { preset_id, params }` with one shipped preset
  (`mask_edge_ripple_wash`) — the **preset architecture is the model
  for Phase 1 treatment presets** (HashMap-keyed params, registered
  preset_id, per-preset shader pipeline, default-fill on unknown keys).
- `TextureUploadQueue` (P0.3.1) — drained per frame; today's only
  producer is the video decoder. Phase 7 NDI / Syphon-output is the
  other consumer the queue was designed for.
- Diagnostics surface (P0.3.2) — fps + panic-restored + audio drop
  count. **Texture-upload drop count is wired but not yet aggregated**
  (deferred at P0.3.2 because no real producer existed; Phase 1 closes
  this in W6).
- Frame-budget perf gate (P0.9.5) — `tests/perf_frame_budget.rs` under
  `--features gpu-tests`. Today's fixture substitutes FxLayer for
  video (no fixture mp4) and reimplements the render path locally
  (Path B) because production pipelines aren't re-exported. Phase 1
  W7.4 replaces both gaps.

**Pre-existing issues (carried from Phase 0):**

- `make lint` (clippy `--all-features`) fails on `src/project/mod.rs`
  due to a Rust 1.92 / clippy upgrade. Cleanup is orthogonal to
  Phase 1 scope; the v3,midi clippy target is clean.

**Test status:** *(placeholder; filled in by snapshot commits as
workstreams ship)*

- *N tests pass under `--features v3,midi` (baseline 523).*
- *N tests pass under default features (baseline 270).*
- *N tests pass under `--no-default-features` (baseline 254).*
- *1 test passes under `--features gpu-tests` (P0.9.5; P1.7.4
  replaces the fixture).*

---

## Operating model

- **Model:** Opus implements; **no separate review step.** Same
  operating model as Phase 0: read the spec section, read every
  CLAUDE.md the task touches, write the test alongside the
  implementation, run `make ci` before committing.
- **Pick one task at a time.** Read the source section it references
  in `004-phase-1.md` and the corresponding entry in `specs/roadmap.md`
  before starting.
- **Commit message format:** `004-P1.<workstream>.<task>: <title>` —
  e.g. `004-P1.2.1: LayerConfig.treatment schema + Mutation`.
- **Branching:** one branch per task; merge straight to `main` once
  CI is green.
- **Pre-commit hook** (`.githooks/pre-commit`, wired by `make setup`)
  runs rustfmt on staged files + `cargo check`. Heavier checks live
  in `make ci`; run that before opening a PR.
- **Tests:** every task ships with new or updated tests. For schema /
  Mutation / snapshot work, follow the v3 proptest pattern in
  `src/project/command.rs`. For render-path work, add a golden under
  `tests/golden/` (covered by `--features gpu-tests`); use
  `UPDATE_GOLDEN=1` to (re-)record the baseline. Where automation
  isn't possible (manual scrub UX, BPM-locked playback against a real
  tap stream), ship a manual smoke-test checklist instead — never
  nothing.
- **Read the right CLAUDE.md.** Tasks touching `src/project/` must
  read `src/project/CLAUDE.md` first (Mutation Reverse-storage rules,
  snapshot invariants). Tasks touching `src/render/` must read
  `src/render/CLAUDE.md` first (GPU lifecycle, panic_restore,
  build-time WGSL validation). Tasks touching `src/video_layer/`
  inherit both plus the P0.4.2b AVFoundation worker pattern.
- **Don't bundle.** If a task tempts you to also fix something
  nearby, resist — that "something nearby" probably already has its
  own task ID below.
- **GPU bring-up tasks ship golden images.** Anything that touches
  `src/render/` and renders pixels needs a `tests/golden/` baseline
  added under `--features gpu-tests`; `UPDATE_GOLDEN=1` rewrites the
  baseline.
- **Preset architecture mirrors P0.5.x.** Treatment presets use the
  same `(preset_id, params: HashMap<String, f32>)` shape and
  per-preset pipeline pattern that FxLayer established. Adding a
  preset is a four-file change (shader, pipeline constructor,
  preset-id constant, dispatch arm) — same recipe as
  `mask_edge_ripple_wash`.

## Task ID conventions

- IDs are flat-numbered within seven workstreams:
  - W1 — Setup + housekeeping (image-format extensions, image
    cache, glossary)
  - W2 — Treatment pipeline foundation (schema + Mutation + render
    integration + Selected-layer UI scaffold)
  - W3 — Treatment presets (proof-points: tone map, blur mask,
    luminance reveal, texture overlay)
  - W4 — Video operator surface (in/out points, loop modes,
    reverse, BPM-lock, thumbnail scrubbing)
  - W5 — Left rail row anatomy (I9 capability follow-on for video)
  - W6 — Diagnostics (N5 capability follow-on — texture-upload
    drop count in the aggregate)
  - W7 — Release housekeeping
- Tasks reference back to the originating section of `004-phase-1.md`
  via the **Source** field.

## Workstream summary

| WS | Theme | Tasks | Parallel-safe? | Touches |
|----|-------|-------|----------------|---------|
| 1 | Setup + housekeeping | 4 | All four parallel-safe; ship before W3/W4 hit their preset UIs | `src/image_layer.rs`, `src/app.rs` (drop dispatch), `src/windows/glossary.rs`, `src/project/audit.rs` |
| 2 | Treatment pipeline foundation | 4 | W2.1 first; W2.2 + W2.3 + W2.4 serial after | `src/project/schema.rs`, `src/project/command.rs`, new `src/render/treatments.rs`, `src/windows/advanced.rs`, `src/windows/scene_editor.rs` |
| 3 | Treatment presets | 6 | All six parallel after W2.2 lands | new `src/render/shaders/treat_*.wgsl`, `src/render/treatments.rs` |
| 4 | Video operator surface | 6 | W4.0 first (tiny); W4.1 + W4.2 next; W4.3 / W4.4 / W4.5 parallel after | `src/project/schema.rs`, `src/video_layer/worker.rs`, `src/windows/advanced.rs`, `src/windows/layer_strip.rs`, `src/app.rs` (layer spawn) |
| 5 | Left rail row anatomy | 1 | Depends on W4.1 + W4.2 | `src/windows/layer_strip.rs` |
| 6 | Diagnostics | 1 | Independent | `src/windows/diagnostics_strip.rs` (or wherever the badge lives), `src/render/texture_upload.rs` |
| 7 | Release housekeeping | 4 | Last — depends on everything else | `Cargo.toml`, `CHANGELOG.md`, `README.md`, `docs/show-day-checklist.md`, `tests/perf_frame_budget.rs`, `Makefile` |

**Suggested order for sequencing PRs:**

1. **W1.1 + W1.2 + W1.3 + W1.4 + W4.0** in parallel — ergonomics +
   glossary + defensive preprocessing + video auto-play tweak.
   None block anything heavier.
2. **W6.1** — close the P0.3.2 deferred drop-count aggregation. Tiny.
3. **W2.1** (schema + Mutation) — unblocks W2.2 + every W3 preset +
   W2.4 focal picker.
4. **W2.2** (render integration) + **W4.1** (in/out points) +
   **W4.2** (loop modes) in parallel — separate code paths.
5. **W2.3** (Selected-layer UI scaffold) once W2.2 ships; then
   **W3.1 → W3.6** in parallel (each preset is a leaf).
   **W2.4** (focal picker) parallels W2.3.
6. **W4.3 / W4.4 / W4.5** parallel after W4.1/W4.2.
7. **W5.1** once W4.1 + W4.2 + W4.5 ship (the row needs the
   thumbnails).
8. **W7** at the end (release housekeeping; W7.4 perf-gate refresh
   pulls in W7.1's fixture mp4 and the Path-A refactor of
   `render_m5_pipeline`).

**Anticipated risks (resolved at task time, called out here):**

- **Treatment vs. effect-chain ordering.** Treatments run *before*
  the existing effect chain (Color → Blur → Transform → External).
  Rationale: treatments are image-grammar primitives that may consume
  multiple textures + content-derived data (luminance histogram,
  palette); the effect chain is parametric per-pixel pixel ops.
  Locked in W2.2.
- **Reverse playback** (P1.4.3) is hard against AVFoundation's
  forward-only `AVAssetReader`. v0.4's worker contract accepts
  negative speed without implementing it; Phase 1 ships a best-effort
  reverse-via-keyframe-cache path and explicitly documents the
  fidelity tradeoff. If full real-time reverse proves infeasible,
  the task ships with the schema + UI + worker hook, and the actual
  reverse decode is deferred to Phase 7 (where the decoder may swap
  for a memory-resident keyframe-table approach).
- **Thumbnail scrubbing** (P1.4.5) pre-decodes K=64 evenly-spaced
  thumbnails at layer-load. The pre-decode lives on the worker
  thread (same `AVAssetReader` re-built K times with `requested_time`
  in the output settings). Cached `Vec<wgpu::TextureView>` of low-res
  thumbnails consumed by the scrub UI.
- **BPM-locked playback** (P1.4.4) derives the worker's speed from
  `clock.bpm() * params.beats_per_loop / clip_duration_secs`. The
  worker reads BPM through the same registry the modulator path uses
  (no new pollers).
- **Image cache** (P1.1.2) is `HashMap<(PathBuf, mtime), Weak<Texture>>`
  with reference-counted ownership. Layer init upgrades to `Arc`;
  drop reduces the count; weak entry evicted lazily next look-up.

---

## Workstream 1 — Setup + housekeeping

Quick wins that ship ahead of the heavier workstreams.

### P1.1.1 — WEBP + GIF first-frame drag-and-drop support

**Source:** `004-phase-1.md` Goal ("v3 engine handles stills
(PNG/JPG/WEBP, GIF first frame) and SVG only"); roadmap §"Content
types" ("Stills: JPEG, PNG, WEBP, GIF (first frame)").
**Type:** ergonomics
**Depends on:** none
**Files:** `src/app.rs:714` (`layer_from_dropped_path`),
`src/image_layer.rs` (`upload_image_rgba8`).

**What:** the roadmap's content-types stanza claims v3 handles WEBP
and GIF; in practice only PNG/JPG/JPEG are wired through
`layer_from_dropped_path`. Add `webp`, `gif` to the accepted
extensions. The `image` crate dep already supports both formats
(WEBP via `image::ImageReader`, GIF first-frame via the same
decoder); `upload_image_rgba8` should just work.

**Steps:**
1. Verify `image::ImageReader::open(path).decode()` succeeds for a
   fixture WEBP and a fixture GIF in a manual smoke. (Drop a `.webp`
   and a `.gif` from Finder; today's "unsupported" toast appears.)
2. Extend `layer_from_dropped_path` (`src/app.rs:714`) with the two
   new extensions, routing through the existing
   `schema::layer_from_image_path(...)` helper.
3. For GIF: confirm `image` decodes the first frame deterministically.
   If the decoder returns an `AnimationDecoder` instead of the first
   frame, fall through to `image::open(path).decode()` which yields
   the first frame as a single image.
4. Update the unsupported-extension toast text in the dropped-file
   handler to mention the new formats (e.g. "Supported: SVG, PNG,
   JPG, WEBP, GIF, MP4, MOV, M4V").

**Tests:**
- Unit test (extend existing `layer_from_dropped_path` tests): assert
  `Some(_)` for `.webp` and `.gif`; `None` for `.bmp` /
  unsupported.
- Manual smoke: drop one of each format from Finder; confirm the
  layer appears and renders.

**Acceptance:**
- [ ] `.webp` and `.gif` files create Image layers via drag-and-drop.
- [ ] Unsupported-format toast lists every supported extension.
- [ ] `make ci` clean.

**Out of scope:** animated GIF (first-frame only); HEIC / AVIF
(Phase 7 if anyone asks); ICC profile handling (Phase 7).

---

### P1.1.2 — Image texture cache (Arc-shared uploads)

**Source:** `004-phase-1.md` Capability set ("cache-friendly texture
upload"); roadmap "every layer is a textured quad after upload"
invariant.
**Type:** engine
**Depends on:** none
**Files:** new `src/image_layer/cache.rs` (or inline in
`src/image_layer.rs`), `src/app.rs` (layer-init path).

**What:** today, two Image layers referencing the same file each
upload their own `wgpu::Texture` — wasteful for shows where a logo
or backplate appears on multiple layers. Add an `ImageTextureCache`
keyed by `(PathBuf, mtime)` that returns an `Arc<wgpu::Texture>`;
the second loader gets the existing texture instead of re-decoding.

**Steps:**
1. Read `src/image_layer.rs` — current path is `upload_image_rgba8`
   returns owned `(Texture, View, dims)`. The cache wraps this.
2. Define `ImageTextureCache` with a `HashMap<(PathBuf,
   SystemTime), Weak<wgpu::Texture>>`. `lookup_or_upload(...)`
   returns `Arc<wgpu::Texture>`. Use `Weak` so the cache doesn't
   prevent eviction when all layers drop.
3. Plumb the cache through `EditingState` (one instance per editor
   session). The layer-init path consults it before falling through
   to a fresh upload.
4. mtime: `fs::metadata(path).map(|m| m.modified())`; if the file
   isn't reachable, key falls back to a path-only entry (cache hit
   still works; reload-on-edit doesn't, which mirrors today's
   non-cached behaviour).
5. Document the eviction policy: weak references die when the last
   `Arc` drops (i.e. when the last `LayerState` referencing this
   texture is removed); cache entries are lazily evicted on
   next-lookup.

**Tests:**
- Unit test: `lookup_or_upload` returns the same `Arc<Texture>` on a
  second call with the same path.
- Unit test: after dropping both Arcs, the weak entry's `upgrade()`
  returns `None`; the next `lookup_or_upload` re-uploads.
- Unit test: changing the file's mtime forces a re-upload (use a
  test helper that touches mtime).
- Optional: a memory-sanity test that confirms two layers loading
  the same path don't double the GPU texture allocation (hard to
  assert without wgpu introspection — skip if friction).

**Acceptance:**
- [ ] Two Image layers pointing at the same path share a single
      `wgpu::Texture`.
- [ ] File modification invalidates the cache entry; next load
      uploads fresh bytes.
- [ ] Removing all referencing layers drops the cache entry.
- [ ] No regression in single-image-load performance.

**Out of scope:** video frame cache (videos are per-frame uploads;
not cacheable); SVG cache (SVG already caches its raster output via
the existing worker pipeline).

---

### P1.1.3 — Glossary entries for Phase 1 domain terms

**Source:** `004-phase-1.md` "named presets" convention.
**Type:** docs / UX
**Depends on:** none
**Files:** `src/windows/glossary.rs` (existing
`GlossaryTerm` enum).

**What:** add glossary entries for the Phase 1 terms operators see
in UI: *treatment*, *tone map*, *blur mask*, *luminance reveal*,
*texture overlay*, *in/out point*, *loop mode*, *BPM-locked
playback*, *reverse playback*, *thumbnail scrub*. Phase 0 added
glossary entries the same way (P0.1.4) — follow that pattern.

**Steps:**
1. Read `src/windows/glossary.rs` — extend the `GlossaryTerm` enum
   with the new variants. Each gets a short definition (~30 words)
   in the term-display match.
2. Each definition explains *what the operator sees* and *what
   pressing the control changes*, not the implementation.
3. The entries get used by `glossary_label(ui, GlossaryTerm::X)`
   calls placed by W2-W4 tasks as they ship the UI.

**Tests:**
- Unit test: the glossary enum is exhaustively matched (existing
  patterns should force this).
- Manual: hover each new label in the UI and confirm the popover
  shows the definition.

**Acceptance:**
- [ ] Each new term has a `GlossaryTerm` variant + definition.
- [ ] Existing exhaustiveness tests still pass.
- [ ] Definitions read like operator-facing copy, not engineer
      notes.

**Out of scope:** glossary entries for FX preset names (Phase 2 owns
the FX library).

---

### P1.1.4 — Safe image preprocessing (EXIF + memory bounds)

**Source:** `004-phase-1.md` Capability set ("Safe image
preprocessing: crop modes, fit/fill, focal-point selection, tone
mapping, cache-friendly texture upload").
**Type:** engine (defensive)
**Depends on:** P1.1.1 (handles the new formats this also defends).
**Files:** `src/image_layer.rs` (`upload_image_rgba8`),
`src/project/audit.rs`.

**What:** the "Safe" qualifier in the spec's capability list resolves
to two concrete defences for v0.5:

1. **EXIF orientation handling.** JPEGs from phones carry an EXIF
   `Orientation` tag (1..8). Today's `upload_image_rgba8` ignores it,
   so portraits shot vertically come out sideways. The `image` crate
   exposes EXIF metadata via `ImageReader::with_guessed_format()` →
   `decode_with_orientation()` (or similar — verify exact API at
   task time). Apply the rotation as part of the upload path so the
   GPU texture is already correctly oriented; no per-frame
   transform needed.
2. **Memory bounds.** A 12K × 8K image (96 MP) uploads to ~384 MB
   of GPU memory at RGBA8 — enough to push the renderer into
   swap on integrated GPUs. Add a hard cap (e.g. `MAX_IMAGE_DIM =
   4096` per side) above which the loader downscales (lanczos /
   bilinear via the `image` crate) to fit, and emits a `tracing::warn`
   plus a `MissingAsset`-shape audit warning so the operator sees
   "image downscaled from 12000×8000 to 4096×2730 to fit GPU
   budget".

**Scope deliberately excludes:** ICC / colour-profile normalisation
(deferred to Phase 7 — needs a colour-management library + careful
display-profile detection); defensive decode for malformed files
(today's `image` crate already returns `Result` — the existing
warn-and-skip path covers this).

**Steps:**
1. Survey `image` crate's EXIF surface — confirm the API and the
   pixel-orientation guarantee post-decode.
2. Apply EXIF rotation inside `upload_image_rgba8` BEFORE the
   `wgpu::Queue::write_texture` call. The GPU texture's `(width,
   height)` reflects the post-rotation dimensions.
3. Add `const MAX_IMAGE_DIM: u32 = 4096` (or whichever the operator
   target hardware budget permits — measure on the M-series baseline
   from P0.9.5; document the choice).
4. If `width > MAX_IMAGE_DIM || height > MAX_IMAGE_DIM`, downscale
   preserving aspect via `image::imageops::resize`.
5. Audit: emit a new `AuditKind::ImageDownscaled { layer_idx,
   original_dims, scaled_dims }` finding for any layer whose source
   image was downscaled. Severity `Info` (operator may want to
   re-encode the source to a smaller resolution upstream).
6. Toast on layer-load when a downscale happens, so the operator
   sees the warning live (in addition to the audit on next reload).

**Tests:**
- Unit test: a fixture JPEG with EXIF orientation=6 (90° rotated)
  decodes to a correctly-oriented buffer.
- Unit test: a synthesised 8K × 4K image downscales to fit the
  cap; the resulting `(width, height)` is bounded.
- Unit test: the new audit finding emits with the right shape.
- Manual smoke: drop a phone-portrait JPEG; confirm it lands
  upright on the canvas.

**Acceptance:**
- [ ] EXIF orientation respected by `upload_image_rgba8`.
- [ ] Images above the dim cap downscale with a warning toast +
      audit finding.
- [ ] No regression on already-small / already-portrait-rotated
      images (no double rotation).

**Out of scope:** ICC profile normalisation (Phase 7); HEIC / AVIF
decode (Phase 7); animated GIF (Phase 1 keeps first-frame only —
see P1.1.1).

---

## Workstream 2 — Treatment pipeline foundation

The architectural workstream. Decides the data model + render
integration that W3's presets and downstream Phase 4 scene grammars
all consume.

### P1.2.1 — `LayerConfig.treatment` schema + Mutation

**Source:** `004-phase-1.md` Engine implications ("Treatment pipeline
... applied to stills *and* video frames").
**Type:** engine (schema + Mutation)
**Depends on:** none
**Files:** `src/project/schema.rs`, `src/project/command.rs`,
`src/project/audit.rs`.

**What:** add `treatment: Option<Treatment>` to `LayerConfig`. The
type mirrors FxLayer's preset shape (P0.5.1):

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Treatment {
    pub preset_id: String,
    #[serde(default)]
    pub params: std::collections::HashMap<String, f32>,
    /// Optional second texture for presets that consume one (today:
    /// the `texture_overlay` preset; P1.3.4). HashMap<String, f32>
    /// can't carry a path, so the field lives on the struct directly.
    /// `None` for presets that don't read it.
    #[serde(default)]
    pub overlay_path: Option<std::path::PathBuf>,
    /// Image paths for presets that compose multiple sources (today:
    /// the `collage` preset; P1.3.6). Capped at 4 entries in v0.5
    /// (matches the 1×2 / 2×1 / 2×2 layouts the collage shader
    /// supports). Empty for presets that don't read it.
    #[serde(default)]
    pub collage_paths: Vec<std::path::PathBuf>,
}
```

`#[serde(default)]` on every added field keeps v7 projects loading
unchanged with `treatment == None`. No schema version bump.

**Single-treatment-per-layer is intentional.** The Phase 1 spec uses
"treatment pipeline" terminology, but v0.5 ships `Option<Treatment>`
(one preset per layer) rather than `Vec<Treatment>`. Rationale:
matches the FxLayer one-preset shape operators already learned, and
the four shipped presets aren't useful to chain (you don't stack
two tone-maps). If Phase 4 zone grammars need composition, growing
to `Vec<Treatment>` is a non-breaking serde change.

**Steps:**
1. Read `src/project/CLAUDE.md` — rule 1 (whole-enum Reverse) +
   rule 2 (effects-vec Reverse) shape your Mutation design.
2. Add the `Treatment` struct (`preset_id` + `params` +
   `overlay_path` + `collage_paths`) + the `treatment:
   Option<Treatment>` field on `LayerConfig`.
3. Mutation: `SetLayerTreatment { layer_idx: usize, new:
   Option<Treatment>, old: Option<Treatment> }` with snapshot Reverse
   (whole-`Option` replacement — same shape as `SetEdgeBlend`).
   **This is the mutation that handles preset switches AND
   overlay-path edits** (the UI constructs a new Treatment with
   the same preset_id + params but a different `overlay_path` and
   dispatches this).
4. Mutation: `SetLayerTreatmentParams { layer_idx: usize, new:
   HashMap<String, f32>, old: HashMap<String, f32> }` — snapshots
   the whole map per W2.1's rule (variant-replacement loses keys
   silently otherwise; this matches `SetFxLayerParams` from P0.5.1).
   Only touches `params`, not `overlay_path`.
5. Add builder helpers on `Project`: `set_layer_treatment_mutation`,
   `set_layer_treatment_params_mutation`.
6. Extend the proptest harness in `command.rs` with both variants.
   The strategy generates `overlay_path: Option<PathBuf>` randomly
   so the round-trip exercises both populated + None.
7. Audit: a `Treatment` with an unknown `preset_id` surfaces a
   non-fatal warning. Mirror P0.5.1's unknown-FxLayer-preset audit.
   Also: a `Treatment` whose `overlay_path` points at a missing file
   surfaces a `MissingAsset` finding (mirrors the image-layer
   missing-file path).

**Tests:**
- Schema serde round-trip: a project with a populated `Treatment`
  and one with `treatment: None` both deserialise correctly.
- Old-shape compatibility: a v7 JSON without the `treatment` key
  loads with `treatment == None`.
- Mutation proptest: round-trip via `apply` for both new variants.
- Audit test: unknown preset_id → Warn finding.

**Acceptance:**
- [ ] `LayerConfig.treatment: Option<Treatment>` exists with
      `preset_id`, `params`, `overlay_path`, and `collage_paths`
      fields.
- [ ] Both Mutations implement `ReverseStorage` and pass proptest.
- [ ] Unknown preset surfaces audit warning.
- [ ] Missing overlay file (and each missing collage entry by index)
      surfaces a `MissingAsset` audit warning.
- [ ] No schema version bump (verified by existing v7-load tests).
- [ ] `make ci` clean.

**Out of scope:** any specific preset (W3); render integration
(P1.2.2); UI (P1.2.3); focal-point picker (P1.2.4).

---

### P1.2.2 — `TreatmentPipeline` render integration

**Source:** `004-phase-1.md` Engine implications ("design the
per-frame texture handoff so that any stage downstream of 'frame
ready' treats stills (constant) and video (per-frame) identically").
**Type:** render
**Depends on:** P1.2.1.
**Files:** new `src/render/treatments.rs`, `src/app.rs` (per-frame
layer loop), `src/render/pipeline.rs` (effect-chain entry point).

**What:** introduce a `TreatmentPipeline` that runs **before** the
existing effect chain. Per-frame layer order becomes: raster source
→ **treatment** → effects → warp → compositor. The treatment stage
operates on the rasterised/decoded source texture and writes into the
effect chain's first ping-pong texture, replacing the current
`svg_pipeline.render` blit.

**Steps:**
1. Read `src/render/CLAUDE.md` for the per-frame render-graph order
   and the existing `EffectPipeline`'s ping-pong contract.
2. Locate the per-frame layer loop in `src/app.rs` (the FxLayer +
   Video branches from P0.4.2a / P0.5.3 are the existing parallel
   examples).
3. Define `TreatmentPipeline` as a trait-objects-free enum of
   per-preset render pipelines (mirror `FxPresetPipeline` from
   P0.5.3). The dispatch arm selects by `preset_id`.
4. When `layer.treatment.is_some()`, the per-frame loop:
   - rasters/decodes into the source texture (today's path);
   - invokes `treatments::dispatch(preset_id, params, ...)` which
     renders into the effect chain's `src_view`;
   - the rest of the chain runs unchanged.
5. When `treatment.is_none()`, the existing `svg_pipeline.render`
   blit runs as today (bit-exact identical path).
6. Document the treatment's bind-group contract (texture inputs:
   source, optional second texture for overlays; uniforms: params +
   clock).

**Tests:**
- Compile-time: new module is reachable from `src/render/mod.rs`.
- Golden test (`--features gpu-tests`): an Image layer with
  `treatment: None` renders bit-exact identical to its pre-P1.2.2
  baseline (no behavioural change on the default path).
- Unit test: `dispatch` for an unknown preset is a no-op (the audit
  W1 caught it; the renderer skips silently).
- Integration: a placeholder/identity treatment that just blits
  source→dst exercises the full path without changing pixels.

**Acceptance:**
- [ ] `TreatmentPipeline` module exists with dispatch by preset_id.
- [ ] Default path (no treatment) is bit-exact unchanged.
- [ ] Identity / no-op treatment proves the bind-group contract
      end to end.
- [ ] Per-frame loop integration documented + tested.
- [ ] `make ci` clean.

**Out of scope:** any specific real preset (W3); UI (P1.2.3).

---

### P1.2.3 — Selected-layer UI scaffold + treatment picker

**Source:** `004-phase-1.md` Usability rule ("Ship a small number of
tasteful image / video behaviours as named presets").
**Type:** UI
**Depends on:** P1.2.2.
**Files:** `src/windows/advanced.rs` (Selected-layer section).

**What:** in the Selected-layer panel, add a "Treatment" collapsing
section that renders when the layer is `LayerKind::Image` or
`LayerKind::Video`. Inside: a preset picker (combobox listing every
registered preset + "None"), and — when a preset is active — a
per-key slider for each `params` entry (using the param's documented
range; defaults pulled from the preset's `for_<id>(...)` builder).

**Steps:**
1. Read `src/windows/advanced.rs:175-244` for the existing
   Selected-layer pattern (Blend mode / Effect chain / Mapping
   sections are the templates).
2. Place the Treatment section between "Blend mode" and "Effect
   chain" so the operator's mental model is "treat then effect".
3. Combobox sources its options from a `treatments::registry()`
   helper that lists `(preset_id, display_label)` pairs. Changing
   the selection dispatches a `SetLayerTreatment` mutation; param
   sliders dispatch `SetLayerTreatmentParams` on drag-release.
4. Param range metadata: each preset documents its param ranges in
   a `for_<id>_params()` -> Vec<ParamDescriptor> helper. Sliders
   render the descriptor's label + range.
5. Visibility: only show the section for Image / Video layers. SVG
   and FxLayer get a placeholder label ("Treatments apply to image
   and video layers; FX layers use their own preset library").

**Tests:**
- Manual smoke: pick a preset on an Image layer; sliders appear;
  edits push mutations through the undo stack.
- Unit test: the param descriptor metadata exhaustively matches the
  registered preset list (one descriptor table per preset).

**Acceptance:**
- [ ] Treatment section renders for Image / Video layers.
- [ ] Preset picker + per-param sliders work + dispatch mutations.
- [ ] Undo reverses both preset-switch and param edits.
- [ ] SVG / FxLayer get the explanatory placeholder.

**Out of scope:** real-time param scrubbing (mutations dispatch on
drag-release); per-frame param interpolation (Phase 4 scene
grammars); focal-point picker (P1.2.4).

---

### P1.2.4 — Focal-point picker for Image / Video layers

**Source:** `004-phase-1.md` Capability set ("focal-point selection"
in the Photo / image treatments stanza).
**Type:** UI
**Depends on:** P1.2.3 (parallel-safe — both edit the Selected-layer
section but in different sub-sections).
**Files:** `src/windows/scene_editor.rs` (the canvas-side click-to-
set affordance; check whether the existing scene editor handles this
or if it lives in `advanced.rs`), `src/project/command.rs`
(Mutation), `src/project/schema.rs` (no field change — `focal:
[f32; 2]` already exists on `LayerKind::Image`; need a matching
field on `LayerKind::Video` if we want focal there too — see
scoping).

**What:** the schema's existing `LayerKind::Image { focal: [f32;
2] }` has had a focal point since v3, but no in-app editor —
operators have to hand-edit JSON. Phase 1 closes this with a
click-to-set affordance on the layer preview / canvas. Operator
clicks (or drags) the focal anchor; the layer's `Cover` crop
re-centres there.

**Scoping decision (write this into the task before starting):**
- `LayerKind::Video` doesn't carry a `focal` field today. Either
  (a) add `focal: [f32; 2]` to `Video` for parity with Image, or
  (b) keep focal Image-only this phase and document Video focal as
  Phase 7 work. **Choose (a)** — it's a non-breaking serde
  addition and parity is what the Phase 1 acceptance criterion
  ("video and still expose the same controls") asks for.

**Steps:**
1. Add `focal: [f32; 2]` with `#[serde(default = "default_focal")]`
   to `LayerKind::Video` (shape mirrors `Image`).
2. Mutation: `SetLayerFocal { layer_idx, new: [f32; 2], old: [f32;
   2] }` with `ReverseStorage` impl. Apply matches both `Image` and
   `Video` arms (whole-enum Reverse rule applies — `Image` and
   `Video` carry different other fields, so the apply panics on
   any other variant).
3. UI: in the Selected-layer panel, when the layer is Image or
   Video with `fit == Cover`, render a 16:9 preview thumbnail
   (re-uses the cached video thumbnail from W4.5 for Video, or the
   image cache from W1.2 for Image). Click on the thumbnail sets
   `focal`; the click position is mapped to normalised UV and
   dispatched via `SetLayerFocal`.
4. Visual feedback: small crosshair overlaid on the thumbnail at
   the current focal position; draggable.
5. The picker only renders when `fit == Cover` (focal is ignored
   for Contain / Stretch).

**Tests:**
- Mutation proptest extends with `SetLayerFocal`.
- Schema serde round-trip: a v7 Video JSON without `focal` loads
  with the default `[0.5, 0.5]`.
- Manual smoke: click on a portrait crop, confirm the head ends up
  centred (focal moved to where the head is in the texture).

**Acceptance:**
- [ ] Click-to-set focal affordance exists for Image + Video layers
      with `fit == Cover`.
- [ ] Mutation is undoable; proptest covers it.
- [ ] Video gains `focal` field (non-breaking serde).
- [ ] Focal picker hidden for Contain / Stretch.

**Out of scope:** focal-point animation across scenes (Phase 4); per-
beat focal cycling (Phase 6).

---

## Workstream 3 — Treatment presets

Phase 1 ships four proof-points. **The fifth treatment family named
in the Phase 1 spec (palette extraction + collage placement) is
NOT shipped here.** They are not currently homed in a later phase
either — Phase 4's "collage bloom" scene template + "palette → mood"
wizard step are different constructs that don't require these
primitives. See the [Cross-workstream notes](#cross-workstream-notes)
for the open question this leaves; resolution either re-homes them
or adds them to W3.

### P1.3.1 — `tone_map` preset

**Source:** `004-phase-1.md` Capability set ("tone mapping").
**Type:** shader + render + content
**Depends on:** P1.2.2.
**Files:** new `src/render/shaders/treat_tone_map.wgsl`,
`src/render/treatments.rs` (pipeline constructor + params builder +
descriptor table).

**What:** an S-curve tone-mapping treatment that lifts shadows
and rolls off highlights — useful for video frames shot in mixed
lighting where the master gamma slider is too blunt. Three params:
`exposure: f32` (stops, -2..=+2), `contrast: f32` (0.5..=1.5),
`shoulder: f32` (highlight-rolloff strength, 0..=1).

**Steps:**
1. WGSL: standard 3-channel gain → contrast pivot → shoulder
   rolloff. Use ACES-like curve (look up canonical references).
2. Pipeline: mirror `FxPresetPipeline::new_ripple_wash` shape from
   P0.5.3. Bind group: source texture, sampler, params uniform.
3. Param defaults: `exposure: 0.0, contrast: 1.0, shoulder: 0.7`
   — identity-ish at default.
4. Document the param ranges + descriptors for P1.2.3's UI to read.

**Tests:**
- Golden test: identity defaults produce ~bit-exact source output
  (within 1 LSB tolerance for floating-point rounding).
- Golden test: exposure=+1.0 doubles luminance roughly.
- Unit test: `for_tone_map(HashMap::new())` returns defaults.

**Acceptance:**
- [ ] WGSL parses + validates via build.rs naga.
- [ ] Pipeline + params builder + descriptor table all wired.
- [ ] Identity defaults are visually transparent.
- [ ] Reachable from the P1.2.3 picker.

**Out of scope:** per-channel tone curves (Phase 7); 1D LUT
import (Phase 7).

---

### P1.3.2 — `blur_mask` preset

**Source:** `004-phase-1.md` Capability set ("blur masks").
**Type:** shader + render + content
**Depends on:** P1.2.2; reuses P0.5.2's SDF helper (the layer's
mask polygon is sampled to gate blur strength).
**Files:** new `src/render/shaders/treat_blur_mask.wgsl`,
`src/render/treatments.rs`.

**What:** a gaussian blur whose strength is gated by the SDF
distance: pixels near the mask edge get heavy blur, pixels deep
inside get less. The operator effect is "feather the photo's edge
into the background without losing detail in the centre". Three
params: `max_radius_px: f32` (0..=64), `edge_band_px: f32`
(0..=128), `falloff: f32` (0..=1, 0=hard cutoff).

**Steps:**
1. WGSL: two-pass separable gaussian (mirror `BlurPipeline`'s
   approach). Use the SDF helper (build.rs prefix table includes
   `treat_` after P1.2.2 lands; ensure that's wired or add it
   alongside the new shader).
2. Per-fragment radius: `r = max_radius * smoothstep(0, edge_band,
   abs(sdf_distance)) * falloff_curve`.
3. Two-pipeline setup (horizontal + vertical). Cache the intermediate
   texture per-layer (same shape as `EffectPipeline`'s
   intermediate_view).

**Tests:**
- Golden test: a fixture polygon mask + this treatment produces a
  baseline blur ramp at the edge.
- Unit test: defaults from empty HashMap → no blur (`max_radius_px
  = 0`).

**Acceptance:**
- [ ] WGSL parses; SDF helper accessible.
- [ ] Edge feathered, centre sharp at default params.
- [ ] Reachable from P1.2.3.

**Out of scope:** non-gaussian kernels (box, motion-blur) — Phase 7.

---

### P1.3.3 — `luminance_reveal` preset

**Source:** `004-phase-1.md` Capability set ("luminance-driven
reveals").
**Type:** shader + render + content
**Depends on:** P1.2.2.
**Files:** new `src/render/shaders/treat_luminance_reveal.wgsl`,
`src/render/treatments.rs`.

**What:** the layer's alpha is a threshold on its own luminance: only
pixels brighter than `threshold` show; everything else is
transparent. A soft `softness` band smooths the threshold so the
operator doesn't see jagged-edge clipping. Three params: `threshold:
f32` (0..=1), `softness: f32` (0..=0.5), `invert: f32` (0 or 1; a
toggle expressed as float for the HashMap-keyed param shape).

**Steps:**
1. WGSL: `luma = dot(rgb, vec3(0.299, 0.587, 0.114))` (Rec. 601
   weights; document the choice). Alpha output: `smoothstep(threshold
   - softness, threshold + softness, luma)`, optionally inverted.
2. RGB pass-through; treatment only affects alpha.
3. The compositor blends the post-treatment output normally —
   downstream warp + mask still apply.

**Tests:**
- Golden test: 50%-threshold against a fixture grayscale ramp
  produces the expected half-transparent split.
- Unit test: defaults give a non-clipping output (`threshold = 0.5,
  softness = 0.1, invert = 0`).

**Acceptance:**
- [ ] WGSL parses.
- [ ] Reveal works on both stills and video frames (same path).
- [ ] Reachable from P1.2.3.

**Out of scope:** chroma-key (Phase 7 — `004-phase-7.md` calls out
"luma/chroma key proper"); colour-range keys (Phase 7).

---

### P1.3.4 — `texture_overlay` preset

**Source:** `004-phase-1.md` Capability set ("texture overlays").
**Type:** shader + render + content
**Depends on:** P1.2.2 (which carries the `Treatment.overlay_path`
field that this preset consumes).
**Files:** new `src/render/shaders/treat_texture_overlay.wgsl`,
`src/render/treatments.rs`.

**What:** a second texture multiplies into the source — paper grain,
noise pattern, film texture, sky gradient. Reads
`Treatment.overlay_path` (already shipped by P1.2.1) for the overlay
texture path. Two HashMap params: `opacity: f32` (0..=1), `tint:
f32` (0..=1; lerp between greyscale and full-colour multiply).

**Steps:**
1. WGSL: sample the source + sample the overlay; `result =
   source * mix(grayscale(overlay), overlay, tint) * opacity`.
2. Overlay texture: upload via the P1.1.2 image cache (cache hit
   when multiple layers share the same overlay).
3. UI: P1.2.3's slider grid grows a "Pick overlay…" button when
   the active preset is `texture_overlay`. Uses the existing
   `rfd::FileDialog` filtered to image extensions. The pick
   constructs a new `Treatment` with the same preset_id + params
   but the new `overlay_path`, then dispatches
   `SetLayerTreatment` (the whole-Option mutation P1.2.1 wired).

**Tests:**
- Golden test: a fixture overlay (8×8 noise pattern) multiplied
  against a constant-colour source produces the expected pattern.
- Unit test: a `Treatment` with `overlay_path = None` and
  preset_id = "texture_overlay" renders without panic (skip
  pipeline; treatment is a no-op).

**Acceptance:**
- [ ] WGSL parses + validates.
- [ ] Reachable from P1.2.3 with file-picker for the overlay path.
- [ ] Missing overlay file → audit warning (from P1.2.1) + no-op
      render.
- [ ] Overlay-path edit is undoable (via `SetLayerTreatment`).

**Out of scope:** procedural overlays (Phase 2 FX); animated overlays
(out of scope until Phase 4 scene grammars).

---

### P1.3.5 — `palette_extract` preset

**Source:** `004-phase-1.md` Capability set ("palette extraction").
**Type:** shader + render + content + CPU preprocessing
**Depends on:** P1.2.2.
**Files:** new `src/render/shaders/treat_palette_extract.wgsl`,
`src/render/treatments.rs`, possibly a small CPU extraction helper
in `src/image_layer.rs`.

**What:** posterise the source image down to N derived colours. CPU
extracts a palette of K colours from the source image (median-cut
algorithm — well-known, deterministic, ~10 ms for a 1080p input);
result is stored as a 1×K LUT texture. Shader samples the LUT,
maps each source pixel to its nearest palette colour. Three params:
`palette_size: f32` (treated as `u8 = max(2, value as u8)`, range
2..=16), `dither: f32` (0..=1, ordered-dither blend for soft
transitions), `vibrance: f32` (0..=2, exaggerates saturation of the
extracted palette).

**Video caveat.** Per-frame palette extraction is expensive
(~10 ms × 60 fps = 600 ms/s wasted). v0.5 uses the **first
decoded frame** as the palette source and reuses it for the rest
of the clip; document this in the commit body. Phase 4 may extend
to per-scene-boundary palette refresh once scene grammars expose
the trigger.

**Steps:**
1. CPU helper: `extract_palette_median_cut(pixels: &[u8], width:
   u32, height: u32, k: u8) -> Vec<[u8; 4]>`. Document the
   median-cut algorithm in a header comment.
2. LUT upload: build a 1×K `wgpu::Texture` (`Rgba8UnormSrgb`),
   pack the extracted palette colours into rows.
3. WGSL: per-fragment, compute distance from source pixel to each
   palette entry (loop K times — K ≤ 16, cheap), pick the nearest.
   Apply ordered-dither (Bayer matrix at fragment_pos) for soft
   transitions when `dither > 0`.
4. Image path: palette extracted once at layer-load (cached
   alongside the texture in the P1.1.2 image cache).
5. Video path: worker pushes the first decoded frame's pixel
   buffer onto a one-shot "palette probe" channel that the render
   thread drains, runs `extract_palette_median_cut`, and stores
   the LUT in the layer's state. Subsequent frames reuse this LUT.
6. The "first-frame palette" approach is the v0.5 simplification.
   Document the limitation; a future task can wire periodic
   refresh.

**Tests:**
- Unit test: `extract_palette_median_cut` produces K colours that
  span the input's chromatic range (sanity test against a synthetic
  3-colour input).
- Unit test: defaults (`palette_size = 8, dither = 0.0, vibrance =
  1.0`) round-trip through the `for_palette_extract(...)` builder.
- Golden test: a fixture image with K=4 produces the documented
  posterised output.

**Acceptance:**
- [ ] CPU palette extraction works for stills + first-frame video.
- [ ] WGSL palette LUT sampling produces expected posterisation.
- [ ] Dither parameter smooths transitions without colour shift.
- [ ] Reachable from P1.2.3.

**Out of scope:** per-scene palette refresh (Phase 4); animated
palette transitions (Phase 4); palette EDIT (operator overrides
extracted colours) — Phase 7 colour-grading work.

---

### P1.3.6 — `collage` preset

**Source:** `004-phase-1.md` Capability set ("collage placement").
**Type:** shader + render + content + schema
**Depends on:** P1.2.2; consumes `Treatment.collage_paths` (added
to the foundation in P1.2.1 — see below).
**Files:** new `src/render/shaders/treat_collage.wgsl`,
`src/render/treatments.rs`.

**What:** composite multiple images onto a single layer in a grid
layout. The layer's source texture acts as the "base canvas";
collage images tile/grid on top. v0.5 caps at 4 images (a 2×2 grid).
Phase 4 generalises to operator-authored layouts. Three HashMap
params: `rows: f32` (treated as u8 in [1, 2]), `cols: f32` (u8 in
[1, 2]), `spacing: f32` (0..=0.1, normalised gap between cells).
Image paths live in a new `Treatment.collage_paths: Vec<PathBuf>`
(empty by default).

**Schema dependency.** P1.2.1 lands `Treatment.collage_paths:
Vec<PathBuf>` alongside `overlay_path` (both are non-`f32` fields
that the HashMap can't carry). Update P1.2.1's struct definition
accordingly; this task only consumes the field.

**Steps:**
1. WGSL: discard the source texture (this preset writes a fresh
   composition over it). For each fragment, compute (row, col) from
   `(uv * vec2(rows, cols))`; sample the matching image from the
   collage-paths array using `textureLoad` on the bound array
   texture. Apply `spacing` as a gap-mask between cells (background
   shows through).
2. Bind group: a `texture_2d_array<f32>` carrying up to 4 collage
   images (each uploaded via the P1.1.2 cache). The array's
   layer count is the actual `collage_paths.len()`, capped at 4.
3. UI: P1.2.3's slider grid grows a "Pick collage images…" button
   that opens `rfd::FileDialog::pick_files()` (multi-select) when
   the active preset is `collage`. Picked files populate
   `collage_paths`; dispatches `SetLayerTreatment`.
4. Missing-file audit: each entry in `collage_paths` is checked;
   missing files surface as `MissingAsset` findings with the index.

**Tests:**
- Unit test: a `Treatment` with `collage_paths.len() == 0` is a
  no-op (skip pipeline; render source unchanged).
- Unit test: a `Treatment` with `collage_paths.len() == 4` and a
  2×2 grid configures the texture array correctly.
- Golden test: a fixture 4-image collage matches a baseline.

**Acceptance:**
- [ ] WGSL parses + validates.
- [ ] Up to 4 images compose in a 1×2 / 2×1 / 2×2 grid.
- [ ] Spacing parameter creates visible gaps.
- [ ] Missing collage images surface audit warnings.
- [ ] Reachable from P1.2.3 with multi-file picker.

**Out of scope:** more than 4 images (Phase 4 — needs richer layout
authoring); arbitrary grid shapes (Phase 4 scene grammars);
per-image rotation / scale (Phase 4).

---

## Workstream 4 — Video operator surface

The VJ-lens completions. Each task adds one operator-visible video
control on top of the v0.4 schema-and-worker base.

### P1.4.0 — Video worker default state (auto-play on drag-drop)

**Source:** `004-phase-1.md` Acceptance criteria ("An operator can
drop an mp4 into the left rail and see it play on the canvas with
seamless loop **within one click**").
**Type:** engine (tiny)
**Depends on:** none
**Files:** `src/video_layer/worker.rs`, possibly `src/app.rs`
(drag-drop dispatch + layer-spawn path).

**What:** the v0.4 worker (P0.4.2b) starts in a state where the
decode loop only runs after `VideoControl::Play` arrives. If a
freshly-spawned worker doesn't get an explicit `Play` from the
spawn path, today's behaviour is "drop file → static layer; click
Play → playback starts" — two operator clicks. The Phase 1
acceptance criterion is one click (the drop itself).

**Steps:**
1. Read the v0.4 worker's state-machine bootstrap. Either:
   - **(a)** the worker already auto-plays on spawn (Play is the
     default state) — no code change needed; verify with a manual
     smoke + write a unit test asserting the default; OR
   - **(b)** the worker starts paused — add a `Play` send right
     after `crate::video_layer::spawn(...)` returns the control
     sender, in the layer-init path that constructs a Video
     LayerState (today's site in `app.rs`); OR
   - **(c)** change the worker's default state to Playing — choose
     this if (b) feels like a workaround.
2. Pick (a) / (b) / (c) based on what the code shows. Document the
   choice in the commit body.
3. If shipping a code change: add a unit test that confirms a
   newly-spawned worker pushes at least one frame within ~200 ms
   (use the test harness's TextureFrameSender stub).

**Tests:**
- Unit / integration: a freshly-spawned video worker produces a
  frame without any explicit Play message.
- Manual smoke: drag an mp4 onto the canvas; it animates without
  additional clicks.

**Acceptance:**
- [ ] Drop-an-mp4 → see-it-play within one operator action.
- [ ] No regression on explicit Play / Pause behaviour.

**Out of scope:** auto-pause on focus-loss (Phase 6 show-control
work); per-layer initial-state policy (Phase 4 scene grammars).

---

### P1.4.1 — In/out points

**Source:** `004-phase-1.md` Capability set ("in/out points");
roadmap I9 ("for 5+ layers it overflows ... in/out points").
**Type:** schema + worker + UI
**Depends on:** none
**Files:** `src/project/schema.rs` (`LayerKind::Video`),
`src/video_layer/worker.rs`, `src/project/command.rs`,
`src/windows/advanced.rs`.

**What:** extend `LayerKind::Video` with `clip_in: f32` and
`clip_out: f32` (seconds, 0.0..=duration). The worker starts decode
at `clip_in`, stops at `clip_out`, and the seamless-loop path seeks
back to `clip_in` (not 0).

**Steps:**
1. Schema: extend `LayerKind::Video` with `#[serde(default)]
   clip_in: f32` (default 0.0) and `clip_out: f32` (default
   `f32::INFINITY`, sentinel for "end of clip"). Non-breaking;
   existing v7 projects load with full-clip playback.
2. Worker: respect `clip_in` on initial seek and on seamless-loop
   reset. Skip frames past `clip_out`. AVAssetReader supports a
   `timeRange` on the output settings — use that to bound the
   reader rather than per-frame timestamp comparison.
3. Mutation: `SetVideoClipRange { layer_idx, new_in, new_out,
   old_in, old_out }` (per-field; both fields snapshot together
   so undo restores the pair atomically).
4. UI: two number inputs (`clip in` / `clip out`) in the
   Selected-layer Video section. Validation: clamp `clip_in <
   clip_out`; if the operator drags one past the other the mutation
   is rejected with a toast.
5. `VideoControl` channel: add `SetClipRange { clip_in, clip_out }`
   — worker rebuilds the reader on receipt.

**Tests:**
- Schema serde round-trip: a v7 JSON without the new fields loads
  with defaults.
- Mutation proptest: round-trip via apply.
- Manual smoke: trim a 10-second clip to seconds 2-7; loop confirms
  decoder seeks to 2.0 on EOF, not 0.0.

**Acceptance:**
- [ ] `clip_in` + `clip_out` fields exist; non-breaking on v7.
- [ ] Worker respects the range; seamless loop seeks to `clip_in`.
- [ ] UI exposes both with clamping validation.
- [ ] Mutation is undoable.

**Out of scope:** thumbnail scrubbing for setting in/out (P1.4.5);
keyboard shortcuts for "set in / out at current position" (Phase 6).

---

### P1.4.2 — Loop modes

**Source:** `004-phase-1.md` Capability set ("loop mode").
**Type:** schema + worker + UI
**Depends on:** none
**Files:** `src/project/schema.rs`, `src/video_layer/worker.rs`,
`src/project/command.rs`, `src/windows/advanced.rs`.

**What:** the existing `loop_seamless: bool` becomes `loop_mode:
LoopMode` with three variants: `Once`, `Loop`, `PingPong`. Migration
maps `loop_seamless: true → Loop` and `false → Once`.

**Steps:**
1. Schema: introduce `pub enum LoopMode { Once, Loop, PingPong }`
   with `#[serde(default = ...)]` defaulting to `Loop`. Replace the
   `loop_seamless: bool` field with `loop_mode: LoopMode`. **Add
   a migration** that maps old saves' `loop_seamless` value to
   the matching enum variant (v7 still — non-breaking via a custom
   deserializer or a serde-helper that accepts both old and new
   shapes).
2. Worker state machine: on EOF in `Once` mode, transition to
   `Paused`. In `Loop`, seek to `clip_in` (or 0 if W4.1 not yet
   landed). In `PingPong`, flip the rate sign and decode the reverse
   pass (this couples with W4.3 reverse; document the dependency).
3. Mutation: `SetVideoLoopMode { layer_idx, new, old }`. Replaces
   `SetVideoLoopSeamless` from P0.4.3; deprecate the old one with
   a doc comment + keep it as a no-op wrapper for one release cycle
   (it just maps `bool` → `LoopMode` and emits the new mutation),
   or delete outright if no projects depend on it. **Delete
   outright** — P0.4.3's mutation hasn't been depended on
   elsewhere.
4. UI: replace the seamless-loop checkbox in the Selected-layer
   Video section with a combobox listing the three modes.
5. `VideoControl::SetLoopMode(LoopMode)` replaces `SetLoop(bool)`.

**PingPong dependency.** True PingPong requires the reverse-decode
path from P1.4.3. **P1.4.2 ships PingPong as a forward-only stub**:
on hitting `clip_out` the worker resets to `clip_in` (same as Loop).
This is correct for the Once and Loop modes; for PingPong it's a
"functionally-Loop" stub until P1.4.3 wires the real reverse pass,
at which point PingPong's second half plays the reverse direction.
Document the stub in the commit; P1.4.3's acceptance closes the
gap.

**Tests:**
- Schema migration: a v0.4 JSON with `loop_seamless: true` /
  `loop_seamless: false` loads to `Loop` / `Once`.
- Mutation proptest: all three variants round-trip.
- Manual smoke: `Once` stops on EOF; `Loop` wraps; `PingPong`
  behaves as Loop until P1.4.3 lands.

**Acceptance:**
- [ ] `LoopMode` enum exists; serde compatible with old saves.
- [ ] Worker implements Once + Loop fully; PingPong is a
      forward-only stub (closed by P1.4.3).
- [ ] UI combobox replaces the checkbox.
- [ ] Mutation is undoable.

**Out of scope:** custom loop region beyond clip_in / clip_out
(use P1.4.1 for that); true reverse direction inside PingPong
(P1.4.3 closes this).

---

### P1.4.3 — Reverse playback (best-effort)

**Source:** `004-phase-1.md` Capability set ("rate (incl. reverse)").
**Type:** worker engine
**Depends on:** P1.4.2 (the LoopMode enum exists with PingPong;
this task wires the reverse direction into PingPong's second half
in addition to direct negative-rate playback).
**Files:** `src/video_layer/worker.rs`.

**What:** negative `speed` causes the worker to decode the clip in
reverse. AVAssetReader is forward-only, so this requires either
(a) pre-decoding the clip into a sparse keyframe cache, or (b)
re-creating the reader at each backward step. (a) is faster, (b)
is memory-light. Phase 1 ships (a) with a **clip-length cap**:
clips ≤ 30 seconds get the full keyframe cache; longer clips log a
warning and fall back to forward playback at `abs(speed)`.

**Steps:**
1. Read the P0.4.2b worker for the existing forward-decode loop.
2. At layer load, if the asset duration ≤ 30 s, pre-decode every
   I-frame into RAM (an `Arc<[u8]>` cache indexed by timestamp).
   Memory: an HD frame is ~6 MB; 30 s at 1 keyframe/s is ~180 MB
   worst-case, acceptable for v0.4 hardware. Document the cost
   tradeoff.
3. Reverse playback consumes the keyframe cache: iterate timestamps
   in reverse, push each cached frame onto the TextureUploadQueue.
   Inter-frame interpolation is **not** in scope — Phase 1 reverse
   is "I-frame staccato"; document this in the commit body.
4. PingPong (P1.4.2 stub) now flips to the reverse path on each
   `clip_out` hit; on `clip_in` it flips back to forward. Same
   keyframe cache, alternating direction.
5. For clips > 30 s, log `tracing::warn!` and clamp `speed >= 0.05`
   (and PingPong falls back to forward-only Loop). The
   Selected-layer UI shows a "Reverse not available for clips
   > 30 s" hint when the operator drags speed below zero OR picks
   PingPong on a long clip.

**Tests:**
- Unit test: keyframe-cache build for a fixture mp4 produces ≥1
  cached frame (skipped if no fixture mp4 — pending P1.7.4).
- Manual smoke: a short clip at speed = -1.0 plays backward.
- Manual smoke: a short clip with `PingPong` alternates forward /
  reverse across clip boundaries.
- Manual smoke: a > 30 s clip with negative speed warns + clamps.

**Acceptance:**
- [ ] Reverse playback works for clips ≤ 30 s.
- [ ] PingPong second-half plays reverse on clips ≤ 30 s.
- [ ] Clips > 30 s fall back to forward at `abs(speed)` + Loop for
      PingPong, with a log + UI hint.
- [ ] Memory cap documented (~180 MB worst case).

**Out of scope:** smooth reverse (Phase 7); real-time reverse for
long clips (Phase 7 needs a different decoder strategy — likely a
keyframe-table approach with on-demand re-decode of the GOP).

---

### P1.4.4 — BPM-locked playback

**Source:** `004-phase-1.md` Capability set ("sync-to-BPM playback");
acceptance criterion ("BPM-locked playback follows tap-tempo without
re-encoding").
**Type:** schema + worker + UI
**Depends on:** P1.4.1 (clip duration is bounded by clip_in /
clip_out for the lock calculation).
**Files:** `src/project/schema.rs`, `src/video_layer/worker.rs`,
`src/windows/advanced.rs`, `src/clock.rs` (BPM read API; existing).

**What:** when `bpm_lock` is set on a Video layer, the worker's
playback rate is derived from the current BPM such that the clip's
in→out range plays exactly N beats. The operator picks N (default 4
beats = 1 bar at 4/4).

**Steps:**
1. Schema: extend `LayerKind::Video` with `#[serde(default)]
   bpm_lock: Option<u8>` (None = free-rate; Some(beats) = locked).
2. Worker: on `Play`, if `bpm_lock` is `Some(beats)`, compute
   `clip_secs = clip_out - clip_in`, `target_beats = beats as f32`,
   `target_secs = target_beats * 60.0 / clock.bpm()`,
   `effective_speed = clip_secs / target_secs`. Re-poll `clock.bpm()`
   each frame loop (no new poller — read the existing global).
3. Mutation: `SetVideoBpmLock { layer_idx, new: Option<u8>, old:
   Option<u8> }`.
4. UI: a "Lock to BPM" checkbox + a "beats" number input (1..=32).
   When the checkbox is unchecked, the speed slider re-enables;
   when checked, the slider is disabled (the worker is driving
   rate from BPM).
5. The speed slider's value is still the schema's `speed` field —
   when `bpm_lock` is set, `speed` is ignored at decode time.

**Tests:**
- Schema serde round-trip.
- Unit test: at BPM=120, a 2-second clip locked to 4 beats →
  effective_speed = 1.0 (4 beats at 120 BPM = 2 seconds).
- Unit test: BPM change midstream → next frame uses the new rate.
- Manual smoke: tap tempo, confirm the clip plays exactly one full
  iteration per bar.

**Acceptance:**
- [ ] BPM-locked playback follows tap-tempo (acceptance criterion
      from `004-phase-1.md`).
- [ ] No re-encode; rate change is at decode-time only.
- [ ] UI clearly indicates locked vs free state.

**Out of scope:** per-beat phase alignment (Phase 6 cuelist work);
quarter-note / eighth-note subdivisions (Phase 6).

---

### P1.4.5 — Thumbnail scrubbing

**Source:** `004-phase-1.md` Capability set ("thumbnail scrubbing");
roadmap I9 ("thumbnail scrubbing + in/out points + loop mode for
video").
**Type:** worker + UI
**Depends on:** P1.4.1 (in/out points define the scrub range).
**Files:** `src/video_layer/worker.rs` (thumbnail probe API),
`src/windows/advanced.rs` (scrub bar UI), `src/windows/layer_strip.rs`
(W5.1 reads the cached thumbnails for the left-rail inline scrub).

**What:** at layer load, the worker pre-decodes K=64 evenly-spaced
thumbnails across the clip's [0, duration] range, downscaled to
~96×54 (16:9 thumbnail size). The scrub UI shows the nearest
thumbnail under the cursor as the operator hovers a timeline bar.

**Steps:**
1. Worker startup: in addition to the main decode loop, kick off a
   separate "thumbnail probe" pass that opens a second AVAssetReader
   with `requested_time` settings at K evenly-spaced points. Pull
   the pixel buffer at each, CPU-downscale to 96×54 (bilinear or
   nearest — picking-quality is fine), store as `Vec<Vec<u8>>` in
   the `LayerState`'s `video_thumbnails` field. (Or as
   `Arc<wgpu::Texture>` per thumbnail — both work; pick the simpler
   storage for the scrub UI's needs.)
2. UI: a horizontal scrub bar in the Selected-layer Video section.
   Hover position → display the nearest thumbnail above the bar.
   Click → emit `Command::VideoSeekTo(layer_idx, target_secs)` which
   dispatches `VideoControl::SeekTo(target_secs)` to the worker.
3. Worker `SeekTo` handler: re-create the reader at the target time
   and resume playing from there.
4. UI: also a draggable "set in" / "set out" affordance — drag the
   left edge of the scrub bar to set `clip_in`; right edge for
   `clip_out`. Dispatches `SetVideoClipRange` (P1.4.1's mutation).

**Tests:**
- Unit test (no GPU): the thumbnail probe API stub returns K
  thumbnails for a fixture clip.
- Manual smoke: hover the scrub bar — thumbnails update; click
  somewhere mid-clip — playback resumes from there.

**Acceptance:**
- [ ] K thumbnails decoded at layer-load (off-render-thread).
- [ ] Scrub bar shows thumbnail-under-cursor.
- [ ] Click → seek.
- [ ] Drag-to-set in/out works.

**Out of scope:** real-time scrub-with-frame-update (frame-accurate
seek is Phase 7); audio waveform under the scrub bar (no audio in
v0.4/v0.5).

---

## Workstream 5 — Left rail row anatomy (I9)

### P1.5.1 — Video row grows inline scrub + in/out markers + loop

**Source:** `004-phase-1.md` UX items resolved ("I9 — Left rail
'+ Add image' grows row anatomy for video: thumbnail scrubbing,
in/out points, loop mode appear inline on the same row that today
shows a static thumbnail").
**Type:** UI
**Depends on:** P1.4.1 (in/out fields), P1.4.2 (loop modes),
P1.4.5 (thumbnails). The scrub bar in the left-rail row is a
compressed version of the Selected-layer scrub bar — same data
source, smaller render.
**Files:** `src/windows/layer_strip.rs`.

**What:** the left-rail layer strip currently renders each row as
`[thumbnail | eye-toggle | label | up/down arrows]`. For Video
layers, grow the row to also show:
- A miniature scrub bar with the cached thumbnails (the operator
  can scrub from the left rail without opening the Selected-layer
  panel).
- A loop-mode glyph (∞ for Loop, → for Once, ⇆ for PingPong).
- In/out markers as small triangles on the scrub bar.

Non-Video rows are unchanged.

**Steps:**
1. Read `src/windows/layer_strip.rs` for the existing row layout
   (lines 213+).
2. Add a Video branch: when `lc.kind` is `Video`, render the extra
   widgets in a second row underneath the thumbnail or to the right
   (operator's screen real estate is tight — pick the layout that
   stays within the 88px-wide left rail).
3. Inline scrub: re-use the cached thumbnails from P1.4.5. Hovering
   the strip shows the thumbnail-under-cursor as a popover; click
   seeks (same `Command::VideoSeekTo` dispatch).
4. Loop glyph: a single character or icon-font glyph; tooltip
   "Loop mode: Loop / Once / PingPong".
5. In/out markers: two 4px-tall triangles on the scrub bar.

**Tests:**
- Manual smoke: Video row shows the new widgets; Image / SVG /
  FxLayer rows unchanged.
- Unit test: the row-height calculation accommodates the new
  widgets (the strip's vertical scroll behaviour stays correct).

**Acceptance:**
- [ ] Video rows show inline scrub + in/out + loop glyph.
- [ ] Click on scrub strip seeks.
- [ ] Non-Video rows unchanged.

**Out of scope:** keyboard shortcuts for scrub (Phase 6); audio
waveform (no audio).

---

## Workstream 6 — Diagnostics

### P1.6.1 — Texture-upload drop count in the diagnostics aggregate

**Source:** `004-phase-0-tasks.md` P0.3.2 ("Deferred: texture-upload
queue counter joins the aggregate when W4.2 wires the drain");
`004-phase-1.md` UX items resolved (N5 capability follow-on).
**Type:** UI
**Depends on:** none (the producer — video — already exists)
**Files:** `src/windows/show_day_strip.rs` (or wherever the
diagnostics badge lives — search for `dropped_audio_count`),
`src/render/texture_upload.rs`.

**What:** P0.3.2 wired audio drops into the diagnostics aggregate
but deferred texture-upload drops because no producer existed.
Now the video worker is a producer. Wire
`TextureUploadQueue::dropped_count()` into the diagnostics aggregate
so the badge shows combined drops.

**Steps:**
1. Locate the diagnostics aggregate render. Today it reads the
   audio counter directly.
2. Plumb `EditingState.texture_upload_queue.dropped_count()` into
   the inputs the diagnostics widget receives (extend
   `ControlPanelInputs` or whatever the existing path is).
3. The aggregate text becomes e.g. `dropped: 0/s` (sum of audio +
   texture upload) — or two side-by-side counters if visually
   clearer. Pick whichever P0.3.2 chose (probably aggregated).
4. The texture-upload counter is process-cumulative; the diagnostics
   widget compares against a per-second snapshot to display rate.
   Mirror the audio counter's snapshot logic.

**Tests:**
- Manual smoke: force texture-upload overflow (stress hook in
  P0.3.1's tests) and confirm the badge increments.
- Unit test: the rate calculation handles a zero-elapsed snapshot
  without dividing by zero.

**Acceptance:**
- [ ] Diagnostics badge aggregates audio + texture-upload drops.
- [ ] Counter fades to subdued when zero (per P0.3.2's design).

**Out of scope:** dropped-frame visualisation as a graph (Phase 6
show-control timeline); per-layer drop tracking (Phase 7
profiling).

---

## Workstream 7 — Release housekeeping

Ship v0.5.0.

### P1.7.1 — Version bump + `release-show` profile validation

**Source:** v0.5 release framing.
**Type:** release
**Depends on:** every other Phase 1 workstream.
**Files:** `Cargo.toml`, `Cargo.lock`.

**What:** bump from `0.4.0` to `0.5.0`. Verify `make build-show` +
`make bundle` produce clean artefacts.

**Steps:**
1. Bump `version` in `Cargo.toml`.
2. `make build-show` — confirm clean build.
3. `make bundle` — confirm `.app` produces.
4. Manual smoke: launch the bundle, drop a photo + a video + an FX
   layer + a treatment, run for 10 minutes, confirm no panics, no
   leaks.

**Acceptance:**
- [ ] Version bumped to 0.5.0.
- [ ] `make build-show` clean.
- [ ] `make bundle` clean.
- [ ] 10-minute soak passes.

**Out of scope:** signing / notarisation — separate concern.

---

### P1.7.2 — CHANGELOG + README updates

**Source:** v0.5 release framing.
**Type:** docs
**Depends on:** every other Phase 1 workstream.
**Files:** `CHANGELOG.md`, `README.md`.

**What:** write the v0.5.0 changelog entry covering every Phase 1
capability; refresh README to reflect the new feature set.

**Steps:**
1. Add a `## v0.5.0` section at the top of `CHANGELOG.md`. Organise
   by capability headline (image treatments, video operator surface,
   left-rail anatomy, diagnostics). One terse sentence per entry.
2. README updates: feature list mentions treatment presets, in/out
   points, BPM-locked video, reverse playback (best-effort),
   thumbnail scrubbing, image format support (WEBP / GIF added).
3. Cross-link the in-app `?` button still resolves to README.

**Acceptance:**
- [ ] CHANGELOG covers every shipped Phase 1 capability.
- [ ] README accurately describes v0.5.
- [ ] Deferred items called out (Phase 2 FX library; Phase 7
      chroma-key, frame-accurate seek).

**Out of scope:** marketing copy.

---

### P1.7.3 — Show-day checklist refresh

**Source:** `docs/show-day-checklist.md` (lives in the repo).
**Type:** docs
**Depends on:** every other Phase 1 workstream.
**Files:** `docs/show-day-checklist.md`.

**What:** add Phase 1 surfaces to the show-day checklist.

**Steps:**
1. Add sections for:
   - **Treatment presets**: each shipped preset gets a "what to
     check" line (e.g. tone_map: identity defaults look identical
     to no-treatment).
   - **Video operator surface**: in/out points correct; loop mode
     selected (Once / Loop / PingPong); BPM lock matches the
     show's BPM; thumbnail scrub works on representative clips.
   - **Image cache**: re-load the project after editing an image
     externally; confirm the new version appears (mtime cache
     invalidation works).
2. Update the "External dependencies" section: still zero Homebrew
   deps (Phase 1 added no native libraries).

**Acceptance:**
- [ ] Checklist covers every v0.5 surface that has a show-day
      failure mode.
- [ ] Manual walkthrough passed.

**Out of scope:** Phase 5 light-rig checks; Phase 7 NDI output checks.

---

### P1.7.4 — Perf gate refresh: real video fixture + Path-A refactor

**Source:** `004-phase-0-tasks.md` P0.9.5 ("substitutions documented
in commit: video layers → FxLayer; ... `TODO(P0.9.5-path-a)` marks
the refactor for a follow-up"); `004-phase-1.md` Acceptance
("Dropped-frame count is visible in the diagnostics badge during a
show").
**Type:** test + refactor
**Depends on:** P1.4.x (real video integration), P1.6.1.
**Files:** `tests/perf_frame_budget.rs`, `tests/fixtures/` (new
fixture mp4), `src/app.rs` (`render_m5_pipeline` Path-A refactor +
`pub(crate)` exposure for tests).

**What:** the v0.4 perf gate substituted FxLayer for video and
reimplemented the render path locally (Path B). Phase 1 closes both
gaps:

1. **Fixture mp4**: add a small (~500KB, ~3 s, 256×144 H.264)
   public-domain video to `tests/fixtures/test.mp4`. Document the
   provenance in `tests/fixtures/README.md`. (If a clip can't be
   added cleanly to git, use a test-time generator that encodes
   one via the AVFoundation writer APIs — bigger lift, but
   self-contained.)
2. **Path-A refactor**: extract a `render_m5_pipeline_to_views(
   renderer, views: &[(TextureView, u32, u32, TextureFormat)], ...)`
   helper in `src/app.rs` (or move to `src/render/` so tests can
   import it from the lib crate — this requires re-exporting from
   `src/lib.rs`). `render_m5_pipeline` becomes a thin wrapper that
   extracts the views from `&[OutputWindow]` and calls the new
   helper. The test plugs in offscreen views directly.
3. **Fixture composition**: use the real path — 4 Video layers
   pointing at `tests/fixtures/test.mp4`, edge-blend across 2
   simulated outputs, OSC + MIDI bindings on effect params with
   stub providers feeding values.
4. **Assertions**: keep the loose p99 < 100 ms CI guard; record the
   new baseline in `docs/show-day-checklist.md`. Compare against
   the v0.4 baseline (M-series ~5.87 ms) to confirm Phase 1 didn't
   regress.

**Tests:**
- The perf gate itself runs under `make test-gpu`.
- The Path-A refactor doesn't break the production render path
  (existing tests stay green).

**Acceptance:**
- [ ] Real video fixture in the perf gate.
- [ ] Path-A refactor lands; `TODO(P0.9.5-path-a)` cleared.
- [ ] Baseline recorded; no regression vs. v0.4.
- [ ] `make test-gpu` passes on macOS.

**Out of scope:** non-macOS CI configurations; long-duration soak
testing (operator's pre-tag work).

---

## Cross-workstream notes

- **Schema bumps.** Phase 1 adds non-bumping fields only.
  `Treatment` (W2.1, including `overlay_path` and `collage_paths`),
  `LayerKind::Video.focal` (W2.4 for parity with `Image.focal`),
  `clip_in` / `clip_out` (W4.1), `loop_mode` (W4.2 — replaces
  `loop_seamless` via a custom deserializer that accepts both
  shapes), `bpm_lock` (W4.4), `AuditKind::ImageDownscaled` (W1.4)
  all land on the existing v7 schema with `#[serde(default)]`
  fallbacks. **No v7 → v8 bump.**
- **Cargo features.** No new features in Phase 1. `video` stays
  default-on. The treatment pipeline is unconditional (the four
  shipped presets are pure WGSL + Rust; no system deps).
- **Reverse-storage rules.** Every Mutation in Phase 1 follows
  `src/project/CLAUDE.md`'s three rules. The new mutations:
  - `SetLayerTreatment` (whole-Option replacement — snapshot Reverse;
    **also handles overlay-path edits** since `overlay_path` lives
    inside the `Treatment` struct)
  - `SetLayerTreatmentParams` (whole-HashMap — effects-vec analog;
    per-key Reverse would lose unrelated keys silently)
  - `SetLayerFocal` (per-field `[f32; 2]` — applies to Image AND
    Video variants; the `apply` impl matches both arms)
  - `SetVideoClipRange` (both clip_in + clip_out snapshot together)
  - `SetVideoLoopMode` (whole-enum Reverse — replaces
    `SetVideoLoopSeamless` from P0.4.3, which is deleted)
  - `SetVideoBpmLock` (Option<u8> — snapshot Reverse)
- **Glossary attachment.** P1.1.3 lands the data; downstream tasks
  (P1.2.3, P1.2.4, P1.3.x, P1.4.x, P1.5.1) attach
  `glossary_label(...)` calls to the UI surfaces they introduce.
- **Preset registry expansion.** W3 grows the treatments registry
  from zero (no treatment presets in v0.4) to six (`tone_map`,
  `blur_mask`, `luminance_reveal`, `texture_overlay`,
  `palette_extract`, `collage`). The registry's `(preset_id,
  display_label, param_descriptors)` shape is locked in P1.2.2;
  W3 tasks just add rows.
- **Worker contract.** W4 extends `VideoControl` with `SetClipRange`,
  `SetLoopMode`, `SetBpmLock`, `SeekTo`. P1.4.0 may also formalise
  the default-state-is-Playing semantics. The worker state machine
  grows correspondingly; document the new states in
  `src/video_layer/worker.rs`'s header comment.
- **All five spec treatment families ship in Phase 1.** P1.3.5
  (`palette_extract`) and P1.3.6 (`collage`) close the gap between
  W3's initial four-preset draft and the Phase 1 spec's promised
  five families (blur masks ✓, luminance-driven reveals ✓, palette
  extraction ✓, collage placement ✓, texture overlays ✓). Palette
  extraction uses first-frame-of-video as the palette source —
  per-scene-boundary refresh is Phase 4 work. Collage caps at 4
  images in a 2×2 grid; richer composition is Phase 4.
- **Acceptance gate for shipping v0.5.0.** Every workstream
  acceptance box checked + P1.7.4 perf gate green + 10-minute soak
  under `make build-show` (P1.7.1) + show-day checklist walkthrough
  on real hardware (P1.7.3).
