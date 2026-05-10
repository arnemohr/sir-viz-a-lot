# 004 Phase 4 — Scene grammars

**Builds on:** Phase 1 (media pipeline), Phase 2 (FX layers), Phase 3
(zones).
**Feeds:** Phase 5 light output (scene templates dictate fixture
behaviour) and Phase 6 cuelist (scenes are the unit of recall).

This is the phase where rmap stops feeling like a renderer with a UI
and starts feeling like a **scene engine**.

---

## Goal

Move from a renderer-centric experience to a scene-centric product.
Replace "compose layers and effects manually every time" with a small
set of strong scene grammars that the operator tunes only where needed.

## Capability set

**Scene templates**
- `window reveal`, `pixel drift`, `collage bloom`, `glow behind
  openings`, `fragmented portrait`, `architectural wash`, `mask-edge
  ripple wash`, `light-spill from windows`, and others.
- Templates are *combinations of primitives from earlier phases* —
  media (Phase 1) + zones (Phase 3) + FX presets (Phase 2) + timing.

**Scene editor flow**
- Wizard-style entry: media → zones → palette → mood → tempo, before
  offering deeper controls.
- Once committed, the scene drops into the standard Editing mode and
  the operator can adjust any underlying primitive.

**Scene behaviours**
- Authored timing: media placement, zone usage, FX preset triggers,
  output dynamics across fixtures (when Phase 5 ships).

## Engine implications

- Scene template format: portable JSON schema, lives alongside the
  per-project file but is reusable across projects.
- Scene editor state machine for the wizard-style flow. Reuse the
  existing v3 launcher / state-machine plumbing where possible
  (`AppState` in `src/app.rs`).
- The scene template engine is the natural consumer of the FX preset
  library (Phase 2) — both speak the same "named, parameterised
  recipe" model.

## UX items resolved

- **I10 capability follow-on** — mode hint banner carries
  capability-availability hints inline ("Bezier handles — coming
  Phase 7", "Fluid sim — Phase 2 preset"), so operators learn the
  engine's edges from the UI itself.
- The broader "operator UI complexity" gap noted in
  `specs/roadmap.md` §3 — scene templates close most of it.
- **Recommendation H follow-on** — the canonical Selected-layer card
  becomes a scene-aware view (template parameters above the fold,
  raw layer parameters under "Advanced").

## Capability lens

- **All three lenses equally.** Scene templates are the user-facing
  surface where VJ, projection-mapping, and light-scene-design
  workflows converge.

## Out of scope for this phase

- Deep generic shader graph authoring (permanent).
- AI-driven scene generation (permanent — operators *pick* templates,
  the system does not invent them).
- Scene packs / export-import (→ Phase 7 professionalisation).

## Usability rule

A first-time operator should create something impressive by:
1. Selecting a scene template,
2. Assigning a few media assets,
3. Mapping a handful of zones.

Anything that requires a fourth step before the scene looks intentional
is a failure of the template, not of the operator.

## Acceptance criteria

- A new operator can produce a coherent immersive scene in **under
  five minutes** starting from the launcher.
- Every scene template documents which zones it consumes, so the
  zone-mapping step is unambiguous.
- Scene templates are self-contained — each one renders without
  reaching outside its declared inputs.
- The "Architectural Wash" template (already a v3 preset name in the
  effect chain dropdown) is upgraded to a full scene template that
  consumes media + zones, not just a parameter preset.
