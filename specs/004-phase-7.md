# 004 Phase 7 — Professionalisation and interoperability

**Final phase.** At this point rmap competes by being **clearer and
more beautiful**, not by blindly matching every capability of
established media servers.

See `specs/roadmap.md` §11 for the permanent out-of-scope list. The
goal of Phase 7 is to close the remaining defensible gaps without
crossing those lines.

---

## Goal

Close the most important gaps to mature tools (HeavyM, LightAct, ArKaos,
Modulo, Resolume, etc.) without turning the product into a bloated
clone. Add the polish and interoperability that separates a
"differentiated v1.0" from a "permanent indie tool".

## Capability set

**Output**
- **NDI / Syphon / Spout output** (macOS-first → Syphon). Distinct
  from the v0.4 NDI *input*. Feeds media servers, capture rigs,
  stream encoders.
- Optional multi-output growth (>2 projectors) only if the
  single-surface + lighting workflow is already excellent.

**Calibration**
- **Calibration save/restore decoupled from content** — venue-scoped
  warp + mask + gamma + monitor identity travels separately from the
  show file. Extends the v3.1 schema v5 portable monitor work
  (T4.12, T4.13).
- Output panel grows calibration verify (alignment cross, dot grid,
  colour bars, edge-blend gradient, focus chart, geometry verify).

**Geometry**
- **Bezier / spline mesh warp** on top of the existing bilinear N×M
  mesh in `src/render/warp.rs` — curved walls, columns, organic
  shapes.

**Mask**
- **Inverse mask + luma / chroma key** on top of the polygon +
  feather masks in `src/render/sdf.rs`. Mode pill cluster (or a
  sub-row inside Mask) hosts inverse + key peers (extends M8's
  capability follow-on first started in Phase 2).

**Light (refinement)**
- **RGBW + colour-temperature-aware mixing.** Phase 5 ships RGB; this
  phase adds the colour-temperature stage so warm-stage venues read
  correctly.

**Project**
- **Export / import of scene packs and reusable surface templates.**
  Scene templates from Phase 4 become portable across projects and
  installations.
- **Logging, diagnostics, show-day utilities** refined from the
  current reliability work in `src/show_day/`.

## Engine implications

- NDI / Syphon / Spout output: the texture-upload pipeline from
  Phase 1 is inverted — instead of reading texture data into the
  renderer, this phase exposes the rendered output to external
  consumers. Use `IOSurface` (macOS / Syphon), DXGI
  (Windows / Spout), or the NDI SDK.
- Bezier mesh warp: extends the existing bilinear N×M warp to a
  control-point + tangent-handle model. Selection language scales:
  active vertex → anchor + handles + tangents (resolves M4's
  capability follow-on; UI palette must scale to ~5 modes per
  I11 follow-on).
- Inverse / luma / chroma masks: extend the SDF infrastructure with
  a `MaskGraph` peer to `MaskPolygon`. Schema migration: old
  `MaskPolygon` loads cleanly as a `MaskGraph` with one node.
- Calibration export: a venue file (`.rmap-calibration.json`) holds
  warp + mask + gamma + monitor identity, references abstract
  surface IDs that show files bind to.
- RGBW mixing: extends the colour-from-pixel sampling stage in
  Phase 5 with a 4-channel colour-space conversion option.

## UX items resolved

- **M4 capability follow-on** — bezier control points + tangent
  handles fully supported; selection visual scales.
- **M8 capability follow-on** — inverse mask + luma / chroma key
  land properly.
- **I7 / Recommendation K** — Output panel with calibration verify
  becomes the primary operating surface for multi-projector work.
- **N1 capability follow-on** — zoom-aware hit-area scaling +
  tangent-handle hit policy for bezier vertices.
- **I1 / Recommendation G follow-on** — calibration export uses the
  same coordinate format (px + percent + corner names) as the v3
  coordinate readouts.

## Capability lens

- **Projection-mapping lens (primary).** Bezier + calibration +
  multi-output is the install-and-repeat side of the product.
- **Light-scene-design lens (secondary).** RGBW + colour-temperature
  mixing rounds out the lighting side.

## Out of scope (permanent)

- AI-based facade detection.
- Deep generic shader graph authoring.
- Moving-light personality complexity (full personality library is
  *deliberately* not on this list — it's permanently parked).
- Bi-directional console integration (Hog / MA / EOS).

## Usability rule

By the end of Phase 7 the product **competes by clarity and
aesthetic coherence**, not by feature count. If a capability
landed in Phase 7 makes the operator UI harder to learn, it was
the wrong capability.

## Acceptance criteria

- A projection-mapping artist can calibrate a venue once, save the
  calibration as a separate file, and reuse it across multiple
  show files.
- Bezier warp produces a clean wrap on a curved column without
  visible mesh banding.
- Inverse mask + luma key are accessible from the Mask mode pill's
  sub-row, not buried in Advanced.
- A Syphon receiver running OBS captures rmap output without colour
  shift or frame stutter.
- RGBW fixtures render colour correctly under the same scene that
  renders RGB fixtures (verified against a reference colour chart).
- The show-day diagnostics surface remains terse — added Phase 7
  capabilities do not bloat it.
