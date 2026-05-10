# 004 Phase 3 — Spatial zones as first-class authored objects

**Builds on:** Phase 2 FX layers (FX presets consume zone semantics
directly).
**Feeds:** Phase 4 scene grammars and Phase 5 light output (zones as
shared addressing space across projector + fixtures).

See `specs/roadmap.md` §1.1 for why **manual zone authoring** is
permanent product policy: AI-based facade detection is permanently out
of scope.

---

## Goal

Make surfaces meaningful without requiring AI or live facade detection.
Operators draw geometry once and tag it with semantic roles; both FX
shaders and (later) light fixtures can address regions by name.

## Capability set

**Zones**
- Named zones with semantic roles: `window`, `portal`, `void`, `spill`,
  `edge`, `highlight`, `light-source`.
- Region-aware shaders that read zone masks and adjust behaviour by
  area. The same FX preset behaves differently when applied to a
  `window` vs a `void`.
- A lightweight zone authoring UI on top of the existing mask + warp
  system. No new geometry primitives — zones are tagged masks.

**Zone-consuming FX presets**
- FX-layer presets that consume zone semantics directly, e.g. "light
  spill from `window` zones", "ripple at `edge` zones", "particle drift
  through `portal` zones".

## Engine implications

- Schema extension: each `Mask` gains an optional `ZoneRole` tag. Old
  projects load with `ZoneRole = None` (no behaviour change). Schema
  migration is automatic on load (extends the v3 → v4 → v5 path).
- Effect shaders read zone tags from a per-fragment uniform indexed by
  layer. Tag dispatch happens shader-side, not on the CPU.
- The existing mask polygon + feather (`MaskPolygon` in
  `src/render/sdf.rs`) continues unchanged — the role tag is metadata,
  not geometry.

## UX items resolved

- **M3 capability follow-on** — mode pill cluster grows toward the
  *Output* / *Cue* peers planned for v0.4 / Phase 6. Zones land as a
  sub-mode within Mask, not a new top-level pill.
- **Recommendation H follow-on** — zone selector replaces the
  free-form "tag" the v3 mask model lacks; the small semantic palette
  is the sole entry point.

## Capability lens

- **Projection-mapping lens (primary).** Make architectural features
  productive without automation.
- **Light-scene-design lens (secondary).** Phase 5 fixtures bind to
  zone-derived signals (e.g. fixture intensity follows
  `light-source` zone activity).

## Out of scope for this phase

- AI-based facade detection (permanent).
- Inferring zones from photo content (the "Suggest mask" optional
  helper in `specs/roadmap.md` Appendix B is the closest thing — it
  proposes a polygon, not a role).
- Zone-graph layout (relationships between zones) — zones are flat,
  named, and tagged in this phase.

## Usability rule

Every zone is selectable from a **small semantic palette** rather than
built from arbitrary low-level shader graphs. The palette is closed:
adding a new role is a code change, not a runtime extensibility point.

## Acceptance criteria

- An operator can draw a polygon, tag it `window`, and pick an FX
  preset whose label says "from windows" — and see the effect bind to
  that zone without further configuration.
- Old projects without zone tags load and render identically.
- The zone palette is documented in the Glossary window (T4.11
  follow-on).
- Shader dispatch on zone tag is verified by a golden-image GPU test
  (`make test-gpu`).
