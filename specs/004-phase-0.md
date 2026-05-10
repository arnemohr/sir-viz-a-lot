# 004 Phase 0 — v0.4 release scope

**Position:** the v0.4.0 release. Sits between **v3.0 ship** (Spec 003 /
the editor, scene model, show-day controls, persistence) and **Phase 1**
(the post-v0.4 plan in `004-phase-1.md`).

**Naming:** Phase 0 is a release scope, not a roadmap phase like 1–7. It
collects every capability that requires new subsystems, external
dependencies, or GPU pipeline work too large for a v3 patch release. The
phase files 1–7 *build on* what lands here.

Sources for this scope: `specs/roadmap.md` §4.3 plus the two v0.4
additions called out in §4.3 ("v0.4 candidates added by this
consolidation").

---

## Goal

Land the engine groundwork that the post-v3 plan depends on:

1. The **temporal medium** — video as a first-class layer (Anchor A).
2. The **spatial-expressive medium** — FX layer foundations
   (Anchor B kickoff).
3. The **live-input surface** — OSC and MIDI parameter binding UI
   sharing the same picker / learn / registry plumbing.
4. The **second-surface stub** — two-projector edge-blend with
   per-projector colour calibration.

None of these are operator-visible blockers for event-scale
single-projector shows (rmap v3's stated target), but each is a
prerequisite for capabilities that *are* visible in Phases 1–7.

**NDI ingest deferred to v0.5.** The original v0.4 scope included
"receive NDI streams as a layer source"; that capability moves
to v0.5 in light of the NewTek SDK's installer + redistribution-
license friction. Roadmap §1.1 already classifies NDI input as
*transport, not primary creative source*, so the deferral matches
the stated philosophy. The `LayerKind::Ndi` schema variant added
by P0.1.2 stays in v7 as a placeholder; v0.5 fills in the receiver
without another schema bump. See
`specs/004-phase-0-ndi-decision.md` for the binding decision that
will apply when v0.5 picks this up.

## Capability set

### Video playback (Anchor A kickoff)
- mp4 / H.264 minimum viable path: decoded on a background thread,
  uploaded to GPU each frame as a texture.
- Seamless loop, configurable playback speed.
- Decoder library: `ffmpeg` bindings or `symphonia` + a video codec
  crate (decision belongs to v0.4 implementation, not roadmap).
- Thread-safe texture-upload pipeline.

### FX layer foundations (Anchor B kickoff)
- Layer enum gains an `FxLayer` variant alongside Image / SVG /
  (now) Video.
- Mask SDF distance + gradient (already present in
  `src/render/sdf.rs`) exposed to effect shaders as fragment inputs,
  not just as alpha.
- **One proof-point preset** — `Mask-edge ripple wash` —
  demonstrates the shader path end-to-end.
- The full preset library (Particle / Wave / Fluid families) is
  deliberately deferred to **Phase 2**.

### ~~NDI input layer~~ (deferred to v0.5)

The original v0.4 capability — "receive an NDI stream as a layer
source" — moves to v0.5. The decision record at
`specs/004-phase-0-ndi-decision.md` (community `ndi` crate) stays
on file and applies whenever the work resumes; the
`LayerKind::Ndi { source_name }` schema placeholder shipped by
P0.1.2 stays in v7 (no migration churn) and renders as a coloured
placeholder rectangle until v0.5 wires the receiver.

The crossed-out original capability set, retained for context:

- ~~Receive an NDI stream as a layer source.~~
- Requires the NDI SDK and a Rust binding.
- Project audit warns when a referenced NDI source is offline at
  load; a "source unavailable" badge surfaces on the layer until
  the source reconnects (same fallback shape as missing media files).
- *Distinct from* Phase 7 NDI / Syphon / Spout *output*.

### Two-projector edge-blend stub
- Second `OutputWindow` on a second monitor.
- **Single logical canvas spans both projectors.** Layer warp + mask
  remain per-layer (unchanged from schema 4 split-warp); the
  configured overlap region between the two projectors *is* the
  edge-blend region. Per-layer output assignment is deferred to
  **Phase 7**.
- Shared blend region with configurable overlap and falloff.
- Edge-blend gradient + alignment cross extend the existing `T`
  test-pattern cycle (per `specs/roadmap.md` §9.2); the gradient
  pattern verifies overlap + falloff settings without media in
  the canvas.
- Full calibration workflow deferred to Phase 7.
- The Output **panel** (rather than badge) starts here too — see
  `specs/roadmap.md` §7 Recommendation K.

### OSC live parameter binding UI
- Visual patch panel: OSC address → layer parameter mapping.
- Currently OSC is a cargo feature (`--features osc`) with no UI;
  v0.4 adds a binding editor in the Advanced panel and promotes
  `osc` to a default-on feature.
- Engine: introduce `Modulator::OscBound { addr }` parallel to
  `Modulator::Audio`; a process-wide OSC value registry keyed by
  address feeds the resolve path through a global analogous to
  `audio::PROVIDER`.
- Implements `BindingPicker` and `ParameterRow` per Appendix B
  (component vocabulary).

### MIDI parameter binding + learn UX (recommended v0.4 addition)
- Engine: extend `src/controls/midi.rs` decoder past Note On 60–71;
  add a process-wide MIDI CC registry analogous to
  `audio::PROVIDER`; introduce `Modulator::MidiBound { cc, channel }`
  parallel to `Modulator::Audio`. `Param::Bound` and `SourceRef`
  are removed in a setup PR (single dead-code cleanup; can ride
  in v3.1 or as phase 0's first commit). Promotes `midi` to a
  default-on feature.
- UX: binding picker on every parameter row, MIDI-learn workflow
  (right-click → "Learn next MIDI CC", listening state has a
  pulsing accent ring, ESC cancels).
- Visible binding indicators on parameter rows
  ("MIDI CC 21" / "OSC /rmap/blur/radius" tags).
- **Strategic point:** ship picker, learn workflow, and registry
  plumbing **once** so OSC and MIDI share the surface. Per
  `specs/roadmap.md` §10.2, this is the highest-leverage *un-scoped*
  capability gap and belongs here, not deferred.

### Per-projector colour calibration
- Extends the existing per-display gamma / brightness / contrast
  override with a full RGB matrix.
- Likely requires a hardware measurement workflow or at minimum a
  manual adjustment tool beyond the current slider trio.
- Phase 7 layers RGBW + colour-temperature mixing on top.

## Engine implications

- **Texture-upload pipeline** for video is the foundation for Phase 7
  NDI / Syphon / Spout *output* — design the pipeline knowing it
  will be inverted later. Reading rendered output back out for
  Syphon-out re-uses the same threading + texture handoff
  abstraction.
- **FxLayer enum variant** prepares the schema and render path for
  Phase 2's full preset library; the v0.4 proof point validates the
  shader-input plumbing without committing to particle / fluid
  systems yet.
- **Live bindings extend `Modulator`.** `Modulator::OscBound { addr }`,
  `Modulator::MidiBound { cc, channel }`, and the existing
  `Modulator::Audio { band, .. }` are the three external-source
  variants. New process-wide registries (one per transport) feed
  the resolve path through globals analogous to `audio::PROVIDER`.
  Reuses the existing picker dropdown, serde, undo, snapshots,
  and proptest scaffolding for free. `Param::Bound` and `SourceRef`
  are removed (single dead-code cleanup PR). The picker, learn
  workflow, and registry plumbing ship once and serve both OSC
  and MIDI.
- **Schema migrates v6 → v7.** `output_target: OutputTarget`
  becomes `output_targets: Vec<OutputTarget>` (single-canvas
  semantics, element 0 = primary projector); new layer variants
  `Video` and `FxLayer` extend the `Layer` enum. `LayerKind::Ndi`
  also lands as a placeholder so v0.5 can fill it in without
  another schema bump. Migration is automatic on load (extends
  the v2 → … → v6 path); project audit warns on missing media
  files rather than failing the load.
- **Cargo features.** `osc` and `midi` move to default-on (binding
  UI is operator-facing once v0.4 ships). `audio` stays gated
  (`cpal` build cost). The `--features osc/midi` flags remain
  for opt-out.
- **Second `OutputWindow`** exercises the multi-window state
  machine in `src/app.rs`; designing it cleanly here unblocks
  Phase 7 multi-output growth (>2 projectors) without rework.
- **NDI input deferred to v0.5.** Decision record on file
  (`specs/004-phase-0-ndi-decision.md`); the schema placeholder
  lands now so v0.5 needs no migration.
- **Glossary** gains entries + popovers for the new domain terms
  (`FxLayer`, NDI source, edge-blend region, RGB matrix,
  MIDI-learn) per the v3 invariant that every Advanced-panel
  domain term has a glossary entry. The "NDI source" entry stays
  in v0.4 — operators will see it on the (currently inert) NDI
  layer-row badge before the receiver lands.

## UX items resolved

- **M3 capability follow-on** — mode pill cluster grows toward
  *Output* / *Cue* peers; the second projector landing here makes
  *Output* the natural first new peer pill (Cue lands in Phase 6).
- **I3** — labelled binding picker (antenna / jack icon, replaces
  the bare `static` dropdown) ships as part of OSC + MIDI binding.
- **I7 capability follow-on** — persistent Output badge starts to
  collapse out of an Output *panel* as the second projector
  arrives. Phase 7 finishes the panel.
- **I15** — launcher projector line gains a multi-output picker
  on the same row.
- **N5 capability follow-on** — diagnostics surface gains
  dropped-frame count alongside fps + panic-restored badge (driven
  by video decode telemetry).

## Capability lens

- **All three lenses** equally. v0.4 is the release that lights up
  the VJ lens (video + binding), the projection-mapping lens
  (second projector + colour calibration), and the
  light-scene-design lens's prerequisite (FxLayer foundations
  let zone-aware effects exist before fixtures arrive in Phase 5).
  NDI input — originally on the projection-mapping-lens line —
  moves to v0.5 alongside Syphon-or-equivalent same-machine
  capture; rmap's projection-mapping story still ships strong on
  the second-projector + colour-calibration combo.

## Out of scope for this release

- Video features beyond seamless loop + playback speed —
  thumbnail scrubbing, in/out points, rate (incl. reverse),
  sync-to-BPM playback all land in **Phase 1**.
- FX preset library (Particle / Wave / Fluid families) → **Phase 2**.
- Effect-chain reordering across all layer types → **Phase 2**.
- Cuelist beyond v3's snapshot model → **Phase 6**.
- LTC / MTC / MIDI-clock sync → **Phase 6**.
- BPM HUD + transport (clock + tap badge) → **v3.1** (`004-v3.1.md`)
  for the badge; **Phase 6** for the full transport.
- Syphon / Spout / NDI **output** → **Phase 7**.
- **NDI input** → **v0.5** (deferred from v0.4 in light of NewTek
  SDK install + redistribution-license friction; decision record
  retained at `specs/004-phase-0-ndi-decision.md`).
- Multi-projector growth >2 → **Phase 7**.
- RGBW + colour-temperature-aware mixing → **Phase 7**.
- Art-Net / sACN output graph and any DMX work → **Phase 5**.

## Usability rule

Every v0.4 surface that an operator interacts with — the Output
panel kickoff, the binding picker, the FxLayer proof-point preset —
**ships as a finished surface for one capability**, not a parameter
explosion. The point of v0.4 is to validate the engine plumbing
that Phases 1–2 will fill out, not to expose every knob the new
subsystems unlock.

## Acceptance criteria

- An operator can drop an mp4 onto the canvas and see it play with
  seamless loop within one click.
- An operator can wire an OSC source and bind a parameter through
  the patch panel; the binding survives save / reload / undo.
- An operator can plug in a MIDI controller, right-click any
  parameter, learn a CC, and twist it to drive the parameter.
- A second projector can be connected; the launcher's projector
  picker recognises both displays; an edge-blended region renders
  between them with configurable overlap. The `T` test-pattern
  cycle includes an edge-blend gradient and an alignment cross
  reachable without media on the canvas.
- Per-projector RGB colour matrix can be saved per-display and
  reloads on next launch.
- A v6 project (saved by v3.1) loads cleanly under v0.4 with the
  schema bumped to v7; missing media files surface as audit
  warnings rather than hard failures.
- The proof-point `Mask-edge ripple wash` FX preset renders
  correctly against an existing polygon mask, demonstrating that
  SDF distance + gradient reach the effect shader.
- Show-day frame budget is unchanged with up to four video layers,
  two projectors, and active OSC + MIDI bindings. (NDI input
  budget validation moves to v0.5.)
