# 004 Phase 2 — Mask-shaped GPU effect layers

**Anchor:** B (Mask-shaped GPU effect layers — equal priority to video).
**Engine kickoff:** v0.4 (FxLayer enum variant + SDF inputs to shaders +
one proof-point preset).
**Phase 2 delivers:** the full preset library and effect-chain
reordering.

See `specs/roadmap.md` §1.2 for the strategic framing of why this anchor
carries equal weight with video.

---

## Goal

Promote masks from visibility shapes to **effect sources**. A mask becomes
the boundary, source, and field for a self-contained visual effect:
particles, waves, displacement, ripple, fluid-like flow.

In product language: *a real-time GPU particle system with mask-driven
wave and distortion effects.* Operators pick from a preset library the
same way they pick scene templates — the same mask drawn to hide a
window can become the source of light spilling out of it.

## Capability set

**Layer + chain**
- `FxLayer` as a first-class layer type alongside Image, SVG, and Video.
- Effect-chain reordering across all layer types (resolves UX item M7's
  capability follow-on; not just FX layers).
- Real preset library with browser, search, save / delete / star /
  export (resolves UX item I2's capability follow-on).

**FX preset families**
- **Particle** — mask-constrained drift, mask-edge emission,
  field-driven flow, collision/reflection at boundary. GPU-driven
  particles whose spawn region, render region, or both are gated by
  the layer's mask. Established term: *mask-constrained particle
  effects* / *emitter masking*.
- **Wave** — mask-edge ripple wash, mask-driven displacement,
  refraction-style distortion. Shader-driven undulation whose source
  map is the mask itself; pixels offset along normals to the mask edge
  produce ripple and refraction.
- **Fluid** — grid-based fluid sim with mask as boundary, particles as
  visualisation, constrained by the mask shape.

**SDF-aware effect inputs**
- Distance, gradient, normal, and signed distance to nearest edge are
  available to effect shaders as fragment inputs, not just as alpha.

## Engine implications

- The fixed v3 chain in `src/effects/mod.rs` (Color → Blur → Transform)
  cannot host this. FX layers need a richer pipeline with an **emitter
  stage**, a **force-field stage**, and a **render stage**.
- Effect-chain reordering becomes load-bearing — an FX preset is a
  named chain, and its stages must be recombinable.
- The mask schema today is polygon + feather (`MaskPolygon` in
  `src/render/sdf.rs`). The SDF infrastructure already exposes distance
  and gradient at every fragment — the missing piece is exposing those
  values to effect shaders as inputs.
- GPU particle simulation: compute-shader (or transform-feedback)
  approach with double-buffered particle state. Particle count budget
  per layer must be capped to keep show-day frame budget intact.
- Determinism: FX layers must be reproducible across crashes and
  scene recall. State serialisation belongs in the snapshot path
  (`src/project/`).

## UX items resolved

- **M7 follow-on** — effect chain becomes reorderable across all
  layer types.
- **I2 follow-on** — real preset library (browser, search, save,
  delete, star, export) replaces the opaque "Apply / Reload" pair.
- **M8 partial follow-on** — Mask sub-row in the mode pill cluster
  starts to host peers. (Inverse mask + luma/chroma key proper land
  in Phase 7.)

## Capability lens

- **VJ lens (primary).** Operator-driven dynamic visuals; preset
  library is what differentiates "I have a mask" from "I have a scene
  element".
- **Projection-mapping lens (secondary).** Mask-driven wave +
  displacement on architectural masks (windows, edges, voids) is a
  major aesthetic ceiling-raise.
- **Light-scene-design lens (secondary).** When Phase 5 ships, FX
  layer outputs are first-class signal sources for fixture mapping
  (e.g. "ripple intensity at this zone → DMX level").

## Out of scope for this phase

- Low-level emitter / particle graph editor (deliberately
  deprioritised; deep generic shader graph authoring is permanently
  out of scope per `specs/roadmap.md` §11).
- Inverse mask + luma / chroma key (→ Phase 7).
- Fluid sim with full Navier–Stokes pressure projection — start with a
  simple advection + dissipation scheme; richer fluid solvers are a
  later refinement.

## Usability rule

Every FX layer is created by **picking a preset and assigning a mask**.
Parameter exposure is ranked: 5 most-used controls above the fold, the
rest under "Advanced". The effect-chain editor is *not* the entry point
for new operators.

## Acceptance criteria

- An operator can drop a polygon mask, pick "mask-edge ripple wash"
  from the preset library, and see it run within three clicks.
- Particle counts per layer are enforced to keep the show-day frame
  budget; over-budget configurations refuse to commit with an inline
  warning.
- FX layer state survives scene recall and undo (proptest harness in
  `src/project/` extended to cover FX layer mutations).
- The preset library exports a single `.rmap-preset.json` per preset
  that can be shared across projects without media or warp data.
