# 004 Phase 5 — Unified DMX / Art-Net / sACN light output

**Closes the loop:** projection + light as one show, the third leg of
the immersive vision (alongside video in Phase 1 and FX layers in
Phase 2).
**Anchor capability** for the Light-scene-design lens — everything in
that lens depends on this phase landing.

See `specs/roadmap.md` §1.1 ("projection and light are one scene, not
two systems"). This phase is what makes that statement load-bearing
rather than aspirational.

---

## Goal

Extend the visual pipeline so one scene can drive both projection and
physical lights. Every show-critical event (Blackout, Go-live, cue
fire, BPM tap) fans out to both surfaces in the same frame.

## Capability set

**Output graph**
- Lighting output graph as a first-class part of the engine.
- Art-Net and/or sACN as the primary lighting transport (common for
  networked DMX and pixel-mapped event systems).

**Fixture model**
- Fixture groups + pixel maps that sample colours / intensities from
  scene outputs.
- Output strategies in order of value (cheapest, most credible
  first):
  1. **Color-from-pixel fixture mapping** — sample N pixels of the
     canvas → DMX channels. Cheapest credible entry point; produces
     strong results from day one.
  2. Scene-wide colour wash.
  3. Zone-derived accent output (binds to Phase 3 zones; fixture
     intensity follows `light-source` / `highlight` zone activity).
  4. Pixel-mapped LED strips and fixture groups.
  5. Trigger / cue outputs for external lighting systems.

**Fan-out events**
- **Light-scene blackout** — same `B` key, both surfaces dark in the
  same frame (resolves UX item M1's capability follow-on).
- **Go-live as a fan-out event** — arms parallel light cues and any
  output streams (NDI / Syphon out, when those land in Phase 7), not
  just the visual transition (resolves UX item M2's capability
  follow-on).

**Modulation**
- BPM-locked fixture chases / pulses driven by the existing
  `Modulator::Bpm`.

## Engine implications

- New module: `src/lighting/` (or similar) for Art-Net / sACN
  transport. Network output runs on a background thread; show-day
  frame budget must remain unaffected.
- Data structures: `FixtureGroup`, `PixelMap`, `DmxUniverse`. Each
  fixture group references either a canvas region (color-from-pixel),
  a zone tag (zone-derived), or a direct DMX value (manual control).
- Wire `Command::Blackout` and the Go-live transition to fan-out
  subscribers. The existing v3 path is a single render-graph state
  flip; this phase introduces a small subscriber list that runs in
  the same frame as the visual change.
- Color-from-pixel sampling: probe N output pixels per fixture, map
  to DMX channels via a configurable colour-space conversion (RGB,
  RGBW, HSV).
- Snapshot integration: light cues authored in parallel to video cues
  share the same scene snapshot — extends the snapshot path in
  `src/project/`.

## UX items resolved

- **M1 capability follow-on** — Blackout becomes
  `LightSceneBlackout`; same key kills both surfaces.
- **M2 capability follow-on** — Go-live designed as an event with
  subscribers (light cue, NDI/Syphon stream-on once Phase 7 ships,
  output failover arm).
- **N5 capability follow-on** — diagnostics gains DMX universe
  activity LED + Art-Net packet rate badge.
- **Recommendation K follow-on** — Output panel grows a
  fixture-group editor and a colour-from-pixel mapping surface
  (same panel, more rows).

## Capability lens

- **Light-scene-design lens (primary).** This is the phase where
  rmap becomes the unified projection-and-light tool the strategic
  framing claims it is.

## Out of scope for this phase

- Moving-light personality editing (deliberately deprioritised; full
  personality library is a Phase 7+ candidate, if ever).
- RGBW + colour-temperature-aware mixing (→ Phase 7).
- LTC / MTC / MIDI-clock sync (→ Phase 6 transport HUD).
- Console interop (Hog, MA, EOS) — out of scope; export trigger
  values is sufficient.

## Usability rule

Start with simple **fixture groups, RGB/RGBW output, and pixel-mapped
LED workflows**. The first thing an operator does after wiring an
Art-Net node is drag a region of the canvas onto a fixture group and
watch it light up — not configure a personality.

## Acceptance criteria

- An operator can wire an Art-Net node, define a fixture group, sample
  a canvas region, and see the fixture follow the canvas colour
  within five minutes.
- `B` (Blackout) blacks both projector and fixtures in the same frame
  (verified with packet capture against an Art-Net listener fixture
  in CI).
- Go-live arms light cues alongside the visual transition; both fire
  on confirm.
- Show-day frame budget is unchanged with up to 16 universes of DMX
  output active.
- Diagnostics badge displays DMX universe activity during a show.
