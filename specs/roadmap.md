# rmap roadmap

**Status:** consolidated 2026-05-10. Supersedes the prior `specs/roadmap.md`,
and folds in `specs/ui-review.md`, `specs/v3-capability-scope.md`, and
`specs/keyboard-accelerators.md`. Those three files remain on disk as
historical references but this document is the single source of truth for
product direction, scope, and post-v3 priorities.

---

## 1. Strategic framing

rmap is a **single-machine, single-projector immersive composition engine**
that treats projected visuals and physical light as parallel expressions of
one scene. v3 (Spec 003 / v0.3.0) lands the editor, scene model, show-day
controls, and persistence; everything beyond v3 is sequenced to deepen the
core idea before broadening it.

The product **does not** want to become a generic media server. Established
tools (HeavyM, LightAct, ArKaos, Modulo, Resolume, etc.) compete on
authoring UX and protocol breadth; matching their feature surface produces
a more complex but still less complete alternative. rmap competes by being
**clearer and more aesthetically coherent** for a narrower target: small
live shows where photo / video / SVG, mask-driven effects, and DMX light
combine into one scene.

### 1.1 Hard scope discipline

The following constraints are load-bearing for every phase below. They are
not "nice to have" — drop one and the product loses its differentiator.

- **One projector at a time** through v3. A two-projector edge-blend stub
  lands at v0.4; full multi-projector workflows are deliberately out of
  scope until the single-surface case is excellent.
  - PCleanup.7.6 — confirmed v1.0 ships with the 2-projector limit
    intact. Phase 7 did not lift it; 3+ projectors and per-edge
    configuration are deferred to a post-v1.0 phase. The launcher UI
    surfaces this constraint to operators when 3+ monitors are
    connected (`src/app.rs` launcher render path).
- **Authored mappings, not auto-detected.** Manual warp + mask is more
  predictable in real venues than unreliable automation; AI-based facade
  detection is permanently out of scope.
- **Photos, video, SVG, and mask-shaped effect layers** are the four
  first-class media types post-v0.4. Everything else (NDI in/out, Syphon,
  Spout) is treated as a transport, not a primary creative source.
- **Projection and light are one scene, not two systems.** Once Art-Net
  output ships (Phase 5), every show-critical event (Blackout, Go-live,
  cue fire, BPM tap) fans out to both surfaces in the same frame.

### 1.2 Two anchor capabilities post-v3 (equal priority)

The post-v3 plan has **two anchor capabilities** that drive the largest
share of perceived ceiling. They are equally important and the engine work
for one informs the other.

#### Anchor A — Video as a first-class layer

Already named in v0.4 scope. The v3 engine handles stills (PNG/JPG/WEBP,
GIF first frame) and SVG only. v0.4 adds mp4 / H.264 video decoded on a
background thread and uploaded to GPU each frame, with seamless loop and
configurable playback speed. This forces the threading + texture-upload
work that several later capabilities (Syphon-out, photo-treatment grammars
applied to video, BPM-locked playback) also need.

#### Anchor B — Mask-shaped GPU effect layers (new)

Make masks productive as more than visibility shapes. A mask becomes the
**boundary, source, and field** for a self-contained visual effect:
particles, waves, displacement, ripple, fluid-like flow.

In product language: *a real-time GPU particle system with mask-driven
wave and distortion effects.* In implementation terms it is a new **FX
layer type** alongside Image, SVG, and Video, with this capability set:

- **Mask-constrained particle systems** — GPU-driven particles whose
  spawn region, render region, or both are gated by the layer's mask
  (established term: "mask-constrained particle effects" / "emitter
  masking").
- **Field-driven particle motion** — vector fields derived from the
  mask's SDF distance + gradient drive particle velocity, so particles
  flow along, around, and away from the mask edge.
- **Mask-boundary collision and reflection** — particles collide with or
  reflect off the mask boundary, producing the "waves reflected by a
  wall" behaviour.
- **Mask-driven wave / ripple / displacement** — shader-driven undulation
  whose source map is the mask itself; pixels offset along normals to the
  mask edge produce ripple and refraction effects.
- **Fluid-like particle flow** — grid-based fluid sim with particles as
  visualisation, constrained by the mask shape.

Engine implications:

- The fixed v3 chain in `src/effects/mod.rs` (Color → Blur → Transform)
  cannot host this; FX layers need a richer pipeline with an emitter
  stage, a force-field stage, and a render stage. Effect-chain
  reordering (a recommended post-v3 lift) becomes load-bearing here.
- The mask schema today is polygon + feather (`MaskPolygon` in
  `src/render/sdf.rs`). The SDF infrastructure already exposes distance
  and gradient at every fragment — the missing piece is feeding those
  values into effect shaders as inputs, not just as alpha.
- Ships as **scene-template presets first**, not as a particle-system
  editor. Operators pick "Ripple from windows", "Particle drift behind
  portal", "Mask-edge wave wash" as named behaviours; low-level emitter
  graphs come much later, if at all.

This anchor is **why rmap is a scene engine and not a photo compositor**.
Without it, masks are useful but inert. With it, the same mask the
operator drew to hide a window can become the source of light spilling
out of that window.

---

## 2. Product principles

The roadmap preserves usability over complexity by enforcing a small set of
core objects and avoiding a control surface that exposes every internal
rendering primitive. The product model revolves around:

- **Media** — photos, SVGs, video, FX layers (mask-driven particles /
  waves / displacement), texture layers.
- **Surfaces** — projector targets and mapped output regions.
- **Zones** — predefined masks and interaction areas: windows, portals,
  voids, edges, spill, no-project regions, light-source regions.
- **Scenes** — authored visual grammars combining media, zones, effects,
  and timing.
- **Light outputs** — fixture groups, LED pixels, networked lighting
  universes (Art-Net / sACN).
- **Cues** — scene recall, transitions, tempo, control events.

The UI always favours **scene templates and semantic controls** over deep
generic parameter exposure. This is what distinguishes rmap from media
servers that ship powerful primitives but slow authoring loops.

---

## 3. Capability gap analysis

| Area | v3 today | Gap to product target | Gap to established tools |
|---|---|---|---|
| Rendering core | Strong wgpu core: warp, masks, effects, layers, scenes, hot reload. | Image-first scene treatment, then video, then mask-shaped FX layers. | Broadly aligned, less mature, less proven. |
| Creative workflow | Effect/layer oriented. | Scene templates, zone semantics, photo + video + FX-layer composition. | Commercial tools package faster authoring UX. |
| Surface interaction | v3 schema 4 makes warp + mask per-layer (each layer maps onto its own surface independently — `specs/003-ui-ux-overhaul-plan.md` §11.6a, `specs/003-tasks-phase-3.md` T3.0a–T3.0d). | Zone semantics, mask-as-input for FX layers, authored spatial behaviours. | Pro tools include stronger calibration/mapping ecosystems. |
| Lighting outputs | None in product form. | First-class DMX/Art-Net/sACN output graph; one scene drives projector + fixtures. | Mature systems support DMX, Art-Net, sACN, pixel mapping, hybrid show control. |
| Live input | MIDI bus, OSC bus, audio FFT modulator wired; **parameter-binding path is dead-coded** (`Param::Bound`, `bound_returns_zero_v1`). | Operator-facing binding UX (OSC scoped at v0.4; MIDI not yet scoped). | Established tools ship MIDI-learn / OSC-learn out of the box. |
| Operator usability | Show-day strip, Blackout, Freeze, panic-restore. | Panic-grade affordances, mode boundaries, transform UI dedup. | More workflow-optimised for operators. |

---

## 4. v3 capability scope (current state)

Canonical scope statement for the v0.3.0 release built under Spec 003. Use
this as the bar: anything below is in v3, anything above is post-v3. A
release checklist lives in `specs/003-tasks-phase-4-5.md` (Phase 5 gate
criteria).

### 4.1 v3 ships

**Content types**
- Stills: JPEG, PNG, WEBP, GIF (first frame).
- SVG layers with hot-reload (file-system watcher; re-renders on save).
- Single projector output per rmap instance.

**Canvas + warp**
- Manual warp: draggable corner-pin quad per layer.
- Mesh subdivision: configurable rows × cols for finer local deformation.
- Corner snapping to projector edges.
- Warp editing directly on the canvas (no separate Mapping tab).

**Mask**
- Per-layer mask polygon: click-to-add vertices, drag to adjust.
- Zone templates: full, left half, right half, top/bottom split, etc.
- Mask feather (soft edge up to ~0.5).

**Effects + compositing**
- Per-layer effect chain (fixed order): Transform (translate, scale),
  External JSON. Color → Blur → Transform stages live in
  `src/effects/mod.rs`.
- Per-layer blend mode: Normal, Add, Multiply, Screen.
- Per-layer opacity.
- Per-layer gamma / brightness / contrast + per-projector display
  override.

**Scenes + crossfades**
- Up to 9 scene slots with thumbnail capture.
- Visual cue strip with scene thumbnails and one-click recall.
- Configurable crossfade duration per project.
- Keyboard recall: `1`–`9`.

**Show-day strip**
- Blackout, Freeze, Test Pattern (cycle), Editor Overlay toggle.
- Go-live mode: fullscreen on the projector, separate from editing state.
- Persistent preview window (second display or floating window).

**Project management**
- Autosave every 5 minutes when dirty (configurable).
- Save in place (`Save`) and Save as… dialog (`Save as…`).
- Launcher: "Try a demo" opens a bundled sample project without CLI flags.
- Open recent: last-used projects remembered across sessions.
- Project audit on load: missing media, schema drift, multi-warp
  consolidation needed.

**Schema migration**
- v3 → v4 → v5 schema migration is automatic on load; no manual
  conversion.

**Tooling + diagnostics**
- `--list-monitors` enumerates displays with human-readable names.
- Show-day operator checklist: `docs/show-day-checklist.md`.
- Per-day UX metrics JSON sink (T1.47).
- Glossary popovers on every domain term in the Advanced panel.
- In-app Glossary window listing all terms.
- In-app help (`?`) opens the README in the default browser.

### 4.2 v3.1 catches

Capabilities deliberately deferred from v3 because they would have added
schema churn or test surface without operator-visible benefit at v0.3.0.

**Deferred audit findings**
- T1.36: static-value modulator round-trip edge case.
- T1.37: `crossfade_duration_s` Reverse-storage under undo.
- T1.39: `output_windowed` undo boundary case.
- T1.40: empty-effects-vec snapshot parity.

**Schema v5 portable monitor (T4.12, T4.13)**
- `output_monitor` becomes `OutputTarget { uuid: Option<String>,
  fallback_index: usize }`.
- On load, prefer UUID match; fall back to index; fall back to display 0
  + audit warning.
- Enables project portability across machines with different monitor
  orders.

**Compile-time Reverse-storage refactor (from T1.14)**
- Move from per-variant reverse-storage convention to a type-level
  guarantee. Reduces silent-corruption risk when adding new `Mutation`
  variants.

**Native macOS menu bar (T4.19, if not shipped in M4)**
- `File / Edit / Window / Help` via `objc2-app-kit::NSMenu`.
- Exposes `Cmd-S`, `Cmd-Shift-S`, `Cmd-O`, `Cmd-Q` as keyboard chords.
- About box: version, license, contributors. Help → rmap Help: opens
  README. Linux/Windows: no-op (egui menu suffices).

**Additional demo content**
- Film strip demo scene.
- Test grid demo scene.

**v3.1 candidates added by this consolidation**
- Layer solo / mute on the left rail (UI item I9 / Recommendation H).
- BPM HUD + tap badge in the top chrome (UI item N3 / Recommendation I).
- Persistent output preview thumbnail in the control window header
  (UI item I7 / Recommendation K).
- Audio bands strip (8-band FFT meter + drag-to-bind) when an audio
  source is active (UI item I3 / Recommendation I).

### 4.3 v0.4 will own

These require new subsystems, external dependencies, or GPU pipeline work
that would be unsafe to land in a patch release. None are operator-visible
blockers for event-scale single-projector shows, which is rmap v3's
stated target.

**Video playback (Anchor A)**
- mp4 / H.264 minimum viable path: decoded on a background thread,
  uploaded to GPU each frame as a texture.
- Seamless loop, configurable playback speed.
- Requires a decoder library (`ffmpeg` bindings or `symphonia` + a video
  codec crate) and a thread-safe texture-upload pipeline.

**NDI input layer**
- Receive an NDI stream as a layer source. Requires the NDI SDK and a
  Rust binding.

**Two-projector edge-blend stub**
- Second `OutputWindow` on a second monitor.
- Per-projector warp + mask.
- Shared blend region with configurable overlap and falloff.
- Full calibration workflow deferred further.

**OSC live parameter binding UI**
- Visual patch panel: OSC address → layer parameter mapping.
- Currently OSC is a cargo feature (`--features osc`) with no UI.
- v0.4 adds a binding editor in the Advanced panel.

**Per-projector colour calibration**
- Extends the existing per-display gamma / brightness / contrast override
  with a full RGB matrix.
- Likely requires a hardware measurement workflow or a manual adjustment
  tool beyond the current slider trio.

**v0.4 candidates added by this consolidation**
- **MIDI parameter binding + learn UX.** OSC binding ships at v0.4; the
  binding picker, learn workflow, and registry plumbing reuse cleanly
  for MIDI. Recommend extending v0.4 to include MIDI so the live-input
  surface ships once and serves both transports. Engine work: extend the
  `src/controls/midi.rs` decoder past Note On 60–71, populate the
  `InputState` source registry, route into `Param::Bound` (today
  `#[allow(dead_code)]`) or extend `Modulator`.
- **FX layer foundations (Anchor B kickoff).** Extend the layer enum to
  include an `FxLayer` variant, expose mask SDF distance + gradient to
  effect shaders, and ship one preset ("Mask-edge ripple wash") as a
  proof point. The full preset library lands in Phase 2; the engine
  prerequisites belong in v0.4 alongside the video work.

---

## 5. Release + phase plan

Per-release and per-phase specs live in their own files; this section is
the index. The 004-series covers everything from v3.1 onward.

### Release scopes (between v3.0 and Phase 1)

| Release | Spec | What it owns |
|---------|------|--------------|
| v3.1 | [`004-v3.1.md`](004-v3.1.md) | Deferred audit findings (T1.36–T1.40), schema v5 portable monitor, native macOS menu bar (`Cmd-S` / `Cmd-Shift-S` / `Cmd-O` / `Cmd-Q`), Reverse-storage refactor, demo content, four small UX wins (layer solo / mute, BPM HUD + tap, output preview thumb, audio bands strip). No new engine subsystems. |
| v0.4 (Phase 0) | [`004-phase-0.md`](004-phase-0.md) | Video playback (Anchor A kickoff), FX layer foundations (Anchor B kickoff), NDI input, two-projector edge-blend stub, OSC + **MIDI** parameter binding UI, per-projector colour calibration. The engine groundwork Phases 1–7 build on. |

### Roadmap phases (post-v0.4)

Phases are sequenced for usability and creative completeness, not feature
breadth. Each phase builds on the previous one and can ship independently.

| # | Phase | Spec | Anchor / lens |
|---|-------|------|---------------|
| 1 | Photo + video media pipeline | [`004-phase-1.md`](004-phase-1.md) | Anchor A · VJ lens primary |
| 2 | Mask-shaped GPU effect layers | [`004-phase-2.md`](004-phase-2.md) | Anchor B · all three lenses |
| 3 | Spatial zones as first-class authored objects | [`004-phase-3.md`](004-phase-3.md) | Projection-mapping lens |
| 4 | Scene grammars | [`004-phase-4.md`](004-phase-4.md) | All three lenses |
| 5 | Unified DMX / Art-Net / sACN light output | [`004-phase-5.md`](004-phase-5.md) | Light-scene-design lens |
| 6 | Show control, cuelist, and live input | [`004-phase-6.md`](004-phase-6.md) | VJ lens primary |
| 7 | Professionalisation and interoperability | [`004-phase-7.md`](004-phase-7.md) | Projection-mapping lens |

**How to use these files**

- Each release / phase file is the canonical spec for its scope: goal,
  capability set, engine implications, the specific UX items it
  resolves, and acceptance criteria.
- The cross-cutting material (v3 capability scope §4, UX punch list §6,
  recommendations A–K §7, capability synthesis §9, highest-leverage
  rankings §10, postpone list §11, keyboard accelerators in Appendix A,
  design system in Appendix B, T4.21 sign-off matrix in Appendix C)
  stays in this document because it spans phases.
- The two anchor capabilities — Anchor A (video) and Anchor B
  (mask-shaped FX layers) — carry equal post-v3 priority and are
  expected to be sequenced *together* through v0.4 → Phase 1 → Phase
  2, not strictly serialised.
- Phase 5 (unified light output) is the third leg of the immersive
  vision (alongside Anchor A and Anchor B); it can begin in parallel
  with Phase 2 once the FX layer engine prerequisites land at v0.4.
- The v3.0 fix work (UX punch list M / I / N items in §6 and
  recommendations A–K in §7) intentionally has no release file — it
  lands as part of v3.0 itself before v3.1 begins. The T4.21 sign-off
  matrix in Appendix C is the gate for that work.

---

## 6. UX punch list

Severity-ranked from a 2026-05-10 review of the rmap control window
(`specs/ui-review.md`). Items here are scoped against `specs/v3-capability-scope.md`
and T4.23 — Phase 4 sign-off (T4.21) wants the M-rank items resolved.

Source captures (Desktop screenshots, 2026-05-10):
- Launcher (idle, no recents): `Screenshot 2026-05-10 at 10.00.00.png`
- Editing, empty canvas: `Screenshot 2026-05-10 at 10.00.12.png`
- Editing, image loaded, no edit: `Screenshot 2026-05-10 at 10.00.31.png`
- Editing, Content + Advanced rail: `Screenshot 2026-05-10 at 10.02.17.png`
- Editing, Warp mode: `Screenshot 2026-05-10 at 10.02.47.png`
- Editing, Warp + Advanced (Architectural Wash): `Screenshot 2026-05-10 at 10.03.01.png`

### 6.1 Must-fix before production (M-rank)

**M1 — Blackout is not panic-grade.** Bottom strip puts Blackout at the
same visual weight as the Outlines debug toggle. Promote: red, ~1.5× tall,
leftmost, system-level shortcut. Capability follow-on (Phase 5): same `B`
also kills the lighting rig.

**M2 — Go-live has no armed/confirm affordance.** It sits between two
non-destructive items styled identically. Make it primary, accent-coloured,
hold-to-arm or one-step confirm, `Cmd+Shift+Return` shortcut, visible
armed-state ring. Capability follow-on (Phase 5): Go-live becomes a
fan-out event with subscribers (light cue, NDI/Syphon stream-on, output
failover arm).

**M3 — Mode model is unclear.** "Warp / Advanced / Preview / Go live"
mixes a tool mode, a panel toggle, a window action, and a state-machine
transition. Three peer mode pills (Warp / Mask / Content); demote Advanced
to a rail-collapse toggle; fold Preview / Go live into the right-side
action cluster. Plan the pill cluster to grow to *Warp / Mask / Content /
Output / Cue* in v0.4 / Phase 6.

**M4 — Selected element has no canvas-side highlight.** All four warp
handles look identical. Active vertex draws at 1.5× size with the warm
accent ring; siblings stay subdued; right-rail coordinate row gets a
"jump to vertex" affordance. Capability follow-on (Phase 7): extend
selection language to bezier control points + tangent handles.

**M5 — Two competing right-rail surfaces.** Advanced (~200 px) + context
panel (~150 px) consume 27% of a 1280-wide laptop. Merge corner-context
into the Advanced rail's Selected-layer section; collapse Advanced to
36 px in Warp / Mask modes. Establish a panel docking model now — every
new surface (BPM HUD, audio bands, output preview, MIDI-learn picker,
fixture group editor) docks into the same right-side region, never adds
a new column.

**M6 — Show name not in title chrome.** The window title is the literal
app name; once a project is loaded, operators identify it by filename.
Replace "Untitled show" pseudo-button with a real titlebar that flips to
the filename on save.

**M7 — Triple-redundant transform controls.** In Content mode the layer's
translate/scale/rotate/opacity appear in three places. One canonical home
— the right-rail Selected-layer card. Capability follow-on (Phase 2):
make the effect chain reorderable; promote the existing "External" effect
hook into a real plugin point.

**M8 — Two divergent entry points to Warp / Mask mode.** Top toolbar has
the pills; the far-right context panel has separate "Edit warp" / "Edit
mask" buttons. Mode pills in the toolbar are the single entry point;
remove the context-panel buttons. Capability follow-on (Phase 2 + Phase
7): expand mode pills (or a sub-row inside Mask) to carry inverse + key
peers.

### 6.2 Important (I-rank)

- **I1 — Coordinate precision is inconsistent and unitless.** Show pixel
  + percent of output, single decimal place, name corners TL/TR/BL/BR.
  Same format becomes the canonical surface in the Phase 7 calibration
  export.
- **I2 — Apply / Reload semantics on the Effect preset are opaque.**
  Rename to `Use preset` / `Revert preset`; disable based on dirty state;
  drop "Reload" entirely (undo handles it). Capability follow-on (Phase
  2): real preset library.
- **I3 — Effect chain `static` dropdown is the binding mode.** Labelled
  binding picker with antenna / jack icon. The MIDI / OSC bus exists but
  the binding *path* is dead-coded (`Param::Bound`, `bound_returns_zero_v1`).
  v0.4 ships OSC param binding; recommend extending the same release to
  MIDI binding (engine + UX work; reuses the picker / learn / registry
  plumbing).
- **I4 — `?` button and Glossary coexist at the same toolbar level.**
  Fold into a single Help menu.
- **I5 — "Advanced" appears twice with different meanings.** Demote
  toolbar Advanced to a rail-collapse toggle (covered by M3).
- **I6 — Cue strip lacks current/next/armed indicators.** Three states
  per tile: idle / armed-next / live. Crossfade ring during transitions.
  Capability follow-on (Phase 6): tiles become rows in a cuelist with
  per-cue timing fields.
- **I7 — Multi-display identity is invisible at the top level.**
  Persistent output badge top-right ("→ Output: BenQ LU935 (1920×1200)").
  Capability follow-on (v0.4 / Phase 7): badge collapses out of an Output
  panel rather than being replaced.
- **I8 — Save / Save as… inconsistency.** Once named, both should
  appear with Save as primary; surface "Saved 13 s ago" indicator.
- **I9 — Left rail "+ Add image" is the only media affordance.** For 5+
  layers it overflows without scroll, search, grouping, or
  reverse-lookup. Capability follow-on (Phase 1): thumbnail scrubbing +
  in/out points + loop mode for video.
- **I10 — Hint string ("Drag the corners…") only in Warp mode.** Same
  pattern missing in Mask, Content, Effects. First-class
  `ModeHintBanner` component; every mode has one. Capability follow-on
  (ongoing): hint banner also carries capability-availability hints
  (e.g. "Bezier handles — coming Phase 7").
- **I11 — No visual mode boundary on the canvas.** Thin 1-px
  mode-tinted border (warp = warm accent, mask = desaturated cool,
  content = neutral). Plan palette for ~5 modes.
- **I12 — Empty-canvas hint is contextually wrong.** The "Drag to move"
  hint shows when there's nothing to drag. Swap with canvas content.
- **I13 — Canvas border colour shifts meaning silently.** Image-loaded
  shows red, warp shows white/grey, empty shows dashed grey. Red border
  is likely a draw-buffer artifact — investigate and standardise per
  I11.
- **I14 — Launcher "Open a recent show" has no empty state.** Add
  caption ("No recent shows yet — try the demo to explore.").
- **I15 — Launcher's projector line is not actionable.** Clickable
  monitor selector + test-pattern affordance. Capability follow-on
  (v0.4): multi-output picker on the same row.
- **I16 — Selection status sits inside the canvas drawing area.** Move
  to chrome or render with a solid background pill.

### 6.3 Nice-to-have (N-rank)

- **N1 — Hit-target sizes for warp handles look ≤ 12 px.** Bump active
  hit area to ~24 px; keep handle visible at 12 px; add keyboard nudge
  (arrow = 1 px, ⇧arrow = 10). Capability follow-on (Phase 7):
  zoom-aware hit-area scaling + tangent-handle policy for bezier.
- **N2 — "Reset all corners" doesn't exist** (only "Reset this corner").
  Add with confirm.
- **N3 — No clock / showtime indicator.** Small clock + BPM tap badge.
  Capability follow-on (Phase 6): full transport with LTC/MTC/MIDI-clock
  sync.
- **N4 — Effect chain `value` suffix repeated on every row.** Unit/label
  belongs in the parameter row's left column.
- **N5 — Diagnostics buried at bottom of Advanced.** Persistent fps +
  panic-restored badge in the chrome. Capability follow-on (Phase 5):
  audio level meter + DMX universe LED in the same cluster.
- **N6 — Accent unification (T4.20).** Handles match warm accent; only
  red signals destructive/error.
- **N7 — Source-image stars confusable with UI markers.** Handle shape /
  halo must be unambiguous against any photo content.
- **N8 — Far-right context panel duplicates Position fields.**
  Collapses with M7 fixed.
- **N9 — Document title flips between filename and selected-layer
  name.** Keep document name fixed; selection status in its own slot.

---

## 7. Concrete UI recommendations

These pull through the M / I / N items into specific UI work. Lettering
matches the original ui-review.md.

### A. Restructure the top chrome (M2, M3, M6, I5, I7, I8, N9)

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ⌂ MyShow.rmap.json  ↶ ↷  Saved 4s    │ Warp · Mask · Content │  Output: ▣│
│                                       │                       │           │
│                                                              [ Go live ▶ ]│
└──────────────────────────────────────────────────────────────────────────┘
```

- **Left:** document identity + undo/redo + autosave indicator.
- **Center:** mode pills (Warp / Mask / Content). The active one gets
  the warm accent. Plan the cluster to grow to Warp / Mask / Content /
  Output / Cue.
- **Right:** Output badge + Go live primary button (accent-coloured,
  hold-to-arm or one-step confirm, `Cmd+Shift+Return`).
- `?` and Glossary fold into a single Help menu at the far right.

### B. Make show-critical controls panic-grade (M1)

```
[  ⏻ BLACKOUT  (B) ]   [ ❄ Freeze (F) ]   [ ▦ Test (T) ]   [ ⌗ Outlines (O) ]
   destructive red       neutral             neutral             neutral
   ~60-px tall           40-px               40-px               40-px
   always one click
```

- Blackout: red, ~1.5× tall, leftmost, never scrolls off, `B` is a
  system-level binding.
- Freeze / Test: neutral but visible state pill when active.
- Outlines: debug — group it visually away from Blackout / Freeze.
- When `GoLive` is active, the strip pins to the top of z-order.
- Capability follow-on (Phase 5): Blackout becomes `LightSceneBlackout`
  once Art-Net output ships.

### C. Selected-element feedback on the canvas (M4, I11, N1)

- Active warp / mask vertex draws at 1.5× size with warm accent ring.
- Right context panel coordinates pair with a "jump to vertex"
  affordance.
- Mode tint on canvas frame: warp = warm accent, mask = desaturated
  cool, content = neutral. Plan palette for ~5 modes.
- Hit area: 24 px logical, 12 px visual. Bezier (Phase 7) needs
  zoom-aware policy.

### D. Reclaim canvas width (M5)

1. Merge corner-context panel into the Advanced rail's Selected-layer
   section.
2. Collapse Advanced rail to a 36-px icon strip in Warp / Mask modes.
3. Default-collapse Advanced when entering Warp / Mask; default-expand
   for Content (make this policy, not incidental).
4. Establish panel docking model now — BPM HUD, audio bands, output
   preview, MIDI-learn picker, fixture group editor all dock into the
   same right-side region.

### E. Effect chain clarity (I2, I3, N4)

- Rename row dropdown from `static` to a labelled binding picker:
  `Source: static · sine/tri/noise · BPM · audio band 1–8 · MIDI CC ·
  OSC addr`. Antenna / jack icon left of the picker.
- "Apply" / "Reload" → `Use preset` / `Revert preset`. Drop Reload;
  let undo handle it.
- Move unit (`px`, `deg`, multiplier) to left of the spinner; remove
  trailing "value" label.

### F. Cue strip status (I6)

Three states per tile: `idle`, `armed/next`, `live/firing`. Keyboard:
`Space` fires armed; arrows move arm cursor without firing. Forward-
compatible to the cuelist work in J — each tile carries a `Cue` struct,
not just a `SceneIndex`.

### G. Coordinate readouts (I1)

```
Corner 4 of 4 (BR)      x  1738 / 1920 px   (90.5%)
                        y    525 / 1080 px   (48.6%)   ⊘ reset
```

Pixel + percent of output; single decimal place; name corners
(TL/TR/BL/BR) for verbal communication on a multi-person crew.

### H. Collapse the transform redundancy (M7, M8, N8)

- One canonical home: right-rail Selected-layer card.
- Below-canvas strip becomes the mode hint banner only.
- Far-right context panel disappears in Content mode; in Warp / Mask it
  shows only mode-specific data.
- Remove "Edit warp" / "Edit mask" buttons in the side panel.
- Add per-layer solo / mute to the left rail.

### I. Surface the live-input system (I3, M7 follow-on, N5 follow-on)

The single highest-ROI capability surface that doesn't touch the render
graph — but it *isn't pure UX*. The MIDI port subscription bus and OSC
UDP listener exist (`src/controls/midi.rs`, `src/controls/osc.rs`); the
parameter-binding *path* is stubbed (`Param::Bound` is `#[allow(dead_code)]`,
`bound_returns_zero_v1` confirms it always resolves to `0.0`; effect
parameters use the `Modulator` enum, not `Param<f32>`).

- **Binding picker** (replaces the `static` dropdown on every parameter
  row). Antenna / jack icon left of the picker.
- **MIDI / OSC learn**: right-click any parameter row → "Learn next
  MIDI CC" / "Learn next OSC address". Listening state has a pulsing
  accent ring; ESC cancels.
- **BPM HUD**: top chrome badge — live BPM, tap source (Space / MIDI 60
  / OSC `/rmap/tap`), 1/2/4/8-bar quantize selector for cue firing.
- **Audio bands strip**: 8 FFT bands as a horizontal meter, each band
  drag-source for "bind this band to that parameter". Most direct
  binding UX possible.
- **Visible binding indicators**: small "MIDI CC 21" / "OSC
  /rmap/blur/radius" tag next to bound parameters; click → unbind /
  relearn.

OSC binding UI is **v0.4 scoped**; MIDI binding is the recommended v0.4
addition — ship the picker, learn workflow, and registry plumbing once.

### J. Cuelist as the eventual home for the cue strip (I6)

Design the strip in v3 so each tile carries a `Cue` struct (in-time,
hold, out-time, follow vs go-on-trigger, BPM-bar quantize, optional
timecode trigger), not just a `SceneIndex`. Transport: Space = go,
←/→ = move arm, Backspace = back-cue. Unblocks Phase 6 cuelist work
without re-architecting v3 cue state.

### K. Output as a panel, not a badge (I7, M2)

The "Output: BenQ" line becomes an Output panel as more output channels
land. v0.4 brings two-projector edge-blend stub + per-projector colour
calibration; the panel carries per-output gamma trim, edge-blend
gradient slider, calibration verify. NDI / Syphon / Spout *output*
(Phase 7) adds a stream-on toggle. Phase 5 grows a fixture-group editor
and a colour-from-pixel mapping surface.

Design the v3 badge so it can collapse out of an Output panel rather
than be replaced.

---

## 8. Proposed "ideal" workspace layout

```
┌─ Title: MyShow.rmap.json · ↶ ↷ · Saved 4s ─┬─ Warp Mask Content ─┬─ Output: BenQ ─ Go live ▶ ─┐
├──────────────┬───────────────────────────────────────────────────────────┬─────────────────────┤
│              │                                                           │  Selected layer     │
│  Layers      │                                                           │   Layer 0 — Disco   │
│  ─────────   │                                                           │  ───────────────    │
│  ▣ Disco     │                                                           │  Blend: Normal      │
│  ▣ Backdrop  │              CANVAS  (mode-tinted border)                 │  Position           │
│  + Add img   │                                                           │   x 1738 / 90.5%    │
│              │              ●────────────●                               │   y  525 / 48.6%    │
│  Cues        │              │            │                               │  Scale / Rotate     │
│  1 ▸ live    │              │   [image]  │                               │  Effect chain       │
│  2 · armed   │              │            │                               │   Color  ▸          │
│  3 · idle    │              ●────────────●                               │   Blur   ▸          │
│  + Save cue  │                                                           │   Tform  ▸          │
│              │   "Drag corners to position the layer on the wall"        │                     │
├──────────────┴───────────────────────────────────────────────────────────┴─────────────────────┤
│  ⏻ BLACKOUT (B)        ❄ Freeze (F)     ▦ Test (T)             diagnostics: ⌗ Outlines  60fps │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Three columns with the canvas dominant. Cues integrated into the left
column (closer to layers, since they reference the same media).
Selection + editing live together on the right. Show-day strip is the
visual base of the window with Blackout outsized.

Forward-compatible additions slot in without restructure: BPM HUD +
clock in title bar, audio bands strip as a collapsible band above the
show-day strip, output preview thumbnail in the right rail (top),
MIDI-learn binding pickers as inline pills on parameter rows.

---

## 9. Capability synthesis — three lenses

The per-finding *Capability angle* notes accumulate into three
cross-cutting themes. This pulls them out so the team can see the
engine investment plan, not just the UI-fix list.

### 9.1 VJ lens — live, audio/MIDI-driven, music-locked performance

- **Video as a first-class layer type** (I9) — Anchor A. v0.4 scopes mp4
  / H.264; Phase 1 deepens.
- **Mask-shaped FX layers** (Anchor B) — engine kickoff in v0.4, full
  preset library in Phase 2. Particle drift / wave wash / fluid flow
  driven by the mask the operator already drew.
- **OSC parameter binding UI** (I3) — v0.4 scoped.
- **MIDI parameter binding + learn UX** (I3) — recommended v0.4
  addition. Highest-leverage *un-scoped* capability gap.
- **Audio-reactive UI surface** (I3, N5) — recommended v3.1 or v0.4.
- **BPM HUD + beat-locked cue firing** (N3, I6) — v3.1 (HUD), Phase 6
  (quantized cue firing).
- **Effect chain reordering + preset library** (M7, I2) — Phase 2.
- **A/B deck pattern** — two scenes loaded simultaneously with a manual
  fader. Phase 6+ candidate.
- **Layer solo / mute, groups, search, reverse-lookup** (I9) — v3.1.

### 9.2 Projection-mapping lens — install, calibrate, repeat

- **Two-projector edge-blend stub** (I7, K) — v0.4 (stub only).
- **Per-projector colour calibration** (I7, K) — v0.4.
- **NDI input layer** (I9) — v0.4.
- **Bezier / spline mesh warp** (M4, N1) — Phase 7.
- **NDI / Syphon / Spout output** (M2, I7, K) — Phase 7. Distinct from
  v0.4 NDI input.
- **Inverse mask + luma / chroma key** (M8) — Phase 7.
- **Test-pattern depth** (M3, B follow-on) — alignment cross, dot grid,
  colour bars, edge-blend gradient (slots into v0.4 stub), focus chart,
  geometry verify (concentric circles).
- **Calibration save/restore decoupled from content** (I1, G) — Phase 7.
  Schema v5 portable monitor (v3.1) is the partial step.
- **Persistent output preview thumbnail** (I7, K) — v3.1.

### 9.3 Light-scene-design lens — projection and light as one show

- **Art-Net / sACN output graph** (M1, M2, N5) — Phase 5. Anchor
  capability; everything else in this lens depends on it.
- **Fixture groups + pixel maps + colour-from-pixel** (M2) — Phase 5.
  Cheapest credible entry point.
- **Light cues authored in parallel to video cues** (I6, J) — Phase 5
  + Phase 6.
- **Light-scene blackout fired with M1** — Phase 5.
- **Light cue fired with M2 Go-live** — Phase 5.
- **BPM-locked fixture chases / pulses** (N3, I lens) — Phase 5 +
  Phase 6.
- **RGBW + colour-temperature-aware mixing** — Phase 7.

---

## 10. Highest-leverage upgrades (ranked)

### 10.1 v3 fixes — if only three ship before merging Phase 4

1. **Collapse triple-redundant transform UI (M7) and unify the two
   warp/mask entry paths (M8).** Single biggest contributor to "I edited
   the wrong thing" mistakes during a live run-up.
2. **Promote Blackout (M1) and Go-live (M2) to panic-grade affordances.**
   Without these, the rest of the polish doesn't matter at show time.
3. **Fix canvas-border colour drift (I13) and standardise the mode-tinted
   frame (I11).** Operators need to read the canvas state at a glance.

### 10.2 Post-v3 upgrades — if only three ship after v3.0 GA

1. **OSC parameter binding UI + extend to MIDI (Recommendation I).**
   OSC is *already in v0.4 scope*; MIDI parameter binding is *not yet
   scoped* anywhere. The MIDI bus exists; the routing path through
   `Param::Bound` is dead-coded today; effect parameters use the
   `Modulator` enum, not `Param<f32>`. Genuine engine + UX work, but
   bounded — doesn't touch the render graph. Strategic move: ship OSC
   binding as v0.4 promises *and* extend the same surface to MIDI in
   the same release. Picker, learn workflow, and registry plumbing
   reuse. Turns rmap from "scriptable tool" into "instrumental show
   engine".
2. **Anchor A — Video as a first-class layer (I9, VJ-lens).** v0.4
   scoped. Single biggest perceived ceiling for any operator who isn't
   doing strictly photo work. Forces the threading / texture-upload
   work that all later capabilities (Syphon-out, photo-treatment
   grammars on video frames, BPM-locked playback) need too.
3. **Anchor B — Mask-shaped FX layers (Phase 2).** Equal priority to
   video. Engine kickoff in v0.4 (FxLayer enum variant + SDF inputs
   exposed to shaders + one preset as proof point); preset library in
   Phase 2. Without this, masks remain inert; with it, the same mask
   the operator drew to hide a window can become the source of light
   spilling out.

Then in the next ring (mix of scoped and not-yet-scoped):

- **Art-Net output stub + colour-from-pixel fixture mapping** (M1
  follow-on, M2 follow-on, lighting lens) — Phase 5.
- **Two-projector edge-blend stub** — v0.4.
- **NDI input layer** — v0.4.
- **Per-projector colour calibration (RGB matrix)** — v0.4.
- **Bezier mesh warp** — Phase 7.
- **NDI / Syphon out** — Phase 7.
- **Cuelist with per-cue timing / follow / BPM-quantize** — Phase 6.
- **Audio-reactive UI surface (8-band FFT meter + drag-to-bind)** —
  v3.1 / v0.4.
- **BPM HUD + transport (clock + tap badge)** — v3.1.
- **Inverse mask + luma / chroma key** — Phase 7.
- **Output panel with calibration verify** — Phase 7.
- **Effect chain reordering + preset library** — Phase 2.
- **Layer solo / mute + groups + reverse-lookup** — v3.1.

---

## 11. What to postpone deliberately

To preserve usability, the following stay out of the critical path:

- Full AI-based facade detection.
- Deep generic shader graph authoring (FX layers ship as named presets;
  graph editor is permanently deprioritised).
- Complex multi-projector workflows (>2 projectors) until single-surface
  + two-projector stub are excellent.
- Moving-light personality complexity.
- Huge protocol surface area early on.

These are tempting but would pull the product toward complexity before
the core creative workflow is truly satisfying.

---

## 12. Success criteria

The roadmap is working if rmap reaches these outcomes:

- An operator creates a beautiful photo + video + FX-layer mapped scene
  in minutes, not hours.
- Surface interaction feels intentional through authored zones and
  masks, including masks-as-effect-sources.
- Projection and lighting outputs feel like one system, not two loosely
  connected tools.
- Operators learn the engine's edges from the UI itself — capability
  hints appear inline as "coming next phase" rather than silent
  absences.
- The operator UI remains understandable even as output capabilities
  expand.
- The system competes by clarity and aesthetic coherence, not feature
  count.

---

## 13. Strategic summary

v3 is the right entry-level product for its target user (single-projector
event/event operator with photos), and the v3-fix list lands that
strong. The post-v3 plan is the **growth path that prevents v3 from
becoming a dead-end product** — most items already have a home in this
document or the underlying spec docs; the rest are flagged as recommended
additions to the v3.1 / v0.4 buckets, justified by current capability
ceilings rather than feature-parity catch-up.

The two anchor capabilities (video and mask-shaped FX layers) carry equal
priority because they jointly raise the artistic ceiling: video adds the
temporal medium, FX layers add the spatial-expressive medium. Light
output (Phase 5) is the third leg that makes the immersive vision
concrete: one scene, three coordinated outputs (projector pixels, FX
motion, fixture intensities). Everything else sequences underneath that
core idea.

The most-important strategic point on the live-input side: the live-input
*bus* exists (MIDI port subscriptions, OSC UDP listener, audio FFT
modulators) but the parameter-binding *path* is genuinely incomplete.
v0.4 already commits to the OSC binding UX; the highest-leverage move is
to scope **MIDI parameter binding into the same release** so the binding
picker, learn workflow, and registry plumbing ship once and serve both
transports.

---

## Appendix A — Keyboard accelerators

This appendix lists every keyboard binding wired in rmap, the command it
triggers, and the file + line where the dispatch originates. Produced by
reading the source directly (T4.18 audit).

### A.1 Plain-letter bindings (no modifier required)

Dispatched from the output-window `WindowEvent::KeyboardInput` handler in
`handle_editing_window_event` (`src/app.rs:3402`). All use `physical_key`
(layout-independent key codes), so the position on the keyboard is fixed
regardless of the operator's locale.

| Key | Command | Effect |
|-----|---------|--------|
| `B` | `Command::Blackout` | Toggle projector black. Source: `src/app.rs:3404` |
| `F` | `Command::Freeze` | Hold current frame on projector. Source: `src/app.rs:3409` |
| `T` | `Command::CycleTestPattern` | Cycle through test patterns. Source: `src/app.rs:3412` |
| `O` | `Command::ToggleEditorOverlay` | Toggle warp/mask handles on projector. Source: `src/app.rs:3415` |
| `Escape` | `event_loop.exit()` | Quit (output-window focused). Source: `src/app.rs:3403` |

### A.2 Scene recall (output-window focus)

Dispatched via `KeyboardSource::push_winit_key`
(`src/controls/keyboard.rs:37`) which is polled each frame by
`InputState`. Key events are buffered and drained on the next poll cycle.

| Keys | Command | Effect |
|------|---------|--------|
| `1`–`9` | `Command::SceneRecall(0..8)` | Recall scene slot 0–8 (zero-indexed). Source: `src/controls/keyboard.rs:42–50` |
| `Space` | `Command::TapTempo` | Tap tempo for BPM-linked modulators. Source: `src/controls/keyboard.rs:39` |

### A.3 Cmd-modified accelerators (v3, output-window focus)

Wired in the same `KeyboardInput` arm as the plain-letter bindings. Only
dispatched when `state.modifiers.super_key()` (macOS Cmd) or
`state.modifiers.control_key()` (Linux/Windows Ctrl) is also held.
`state.modifiers` is updated by `WindowEvent::ModifiersChanged`
(`src/app.rs:3390`).

| Chord | Command | Effect | Source |
|-------|---------|--------|--------|
| `Cmd-Z` | `undo_stack.undo()` | Undo last mutation. | `src/app.rs:3418–3458` |
| `Cmd-Shift-Z` | `undo_stack.redo()` | Redo last undone mutation. | `src/app.rs:3418–3458` |

### A.4 Same chords from the control window (egui focus)

When the control window is focused, winit `KeyboardInput` events are
swallowed by egui. Undo/redo chords are re-detected inside egui's input
state after the `ctrl.render(…)` call using
`ui.input(|i| i.key_pressed(egui::Key::Z))`. Same semantics as the
output-window path.

| Chord | Command | Source |
|-------|---------|--------|
| `Cmd-Z` (control focused) | `undo_stack.undo()` | `src/app.rs:3143–3167` |
| `Cmd-Shift-Z` (control focused) | `undo_stack.redo()` | `src/app.rs:3143–3167` |

### A.5 Bindings not yet wired as keyboard chords

Operations exist in the UI (toolbar buttons / menu items) but have no
dedicated keyboard chord. Called out so a future native menu bar (T4.19)
has a clear gap list.

| Operation | How to invoke today | Notes |
|-----------|---------------------|-------|
| Save (in place) | Toolbar "Save" button → `ControlPanelAction::RequestSave` | No `Cmd-S` chord. |
| Save as… | Toolbar "Save as…" button → `ControlPanelAction::RequestSaveAs` | No `Cmd-Shift-S` chord. |
| Open | Launcher window; no open-in-editor chord | No `Cmd-O` chord. |
| Quit (control window) | macOS window close gesture / `Cmd-Q` via OS app menu | Not wired in our `KeyboardInput` handler. `Escape` on the output window exits; the control window's `CloseRequested` event drops the window without quitting. |
| Go-live | Toolbar "Go live" button | **Recommended chord (M2): `Cmd+Shift+Return`**, with hold-to-arm or one-step confirm. |

### A.6 Conflicts: none

`O` (EditorOverlay) and a hypothetical `Cmd-O` (Open) are **not** in
conflict because they require different modifiers. Plain `O` fires only
when no command modifier is held; a `Cmd-O` chord fires only when the
modifier is present.

The plain-letter path does not check for the absence of modifiers — the
operator pressing `Cmd-Z` with the output window focused hits the
`KeyCode::KeyZ` arm, where the modifier check then routes to undo/redo
rather than any plain-letter command. There is no `KeyZ` plain binding.

### A.7 Index: dispatch sites

| File | Line(s) | What |
|------|---------|------|
| `src/app.rs` | 3390 | `ModifiersChanged` → `state.modifiers` |
| `src/app.rs` | 3402–3458 | Output-window `KeyboardInput` arm (plain letters + Cmd-Z) |
| `src/app.rs` | 3143–3167 | Control-window egui input poll (Cmd-Z / Cmd-Shift-Z) |
| `src/controls/keyboard.rs` | 37–56 | `KeyboardSource::push_winit_key` (Space, 1–9, B, F) |

---

## Appendix B — Design system notes (groundwork for T4.20 follow-on)

- **One accent for "user-interactable handle"** — warp vertices, mask
  vertices, drag-source markers, primary buttons, mode-active pill all
  use the warm accent. Cue tiles do not use the accent unless armed.
- **One destructive colour** — red for Blackout + delete-confirms +
  validation errors. Nothing else (clear up the spurious red canvas
  border per I13).
- **One "armed/live" colour** — saturated state distinct from accent
  (e.g. amber pulse) used only when a transition is loaded but not fired
  (Go-live armed, cue armed, MIDI-learn listening).
- **Component vocabulary** — standardise `BindingPicker`, `ParameterRow`
  (label · unit · spinner · binding picker · learn-state pill),
  `ModePill`, `ModeHintBanner`, `StatusBadge`, `PanicButton`,
  `OutputBadge` (collapses out of `OutputPanel` once v0.4 multi-projector
  + per-projector colour calibration land).
- **Naming** — align "layer / effect / cue / scene / mapping / mask /
  output / fixture / cuelist / FX layer / zone" across rail, menus,
  shortcuts, glossary. The glossary window (T4.11) is the reference
  doc.

### Accessibility for dark venues

- Body text minimum 13 px, monospace numerics 14 px (current `0.83113`
  reads as ~10 px).
- Focus ring on every interactive control, keyboard-reachable via tab.
  Currently invisible on the corner handles.
- Confirm `B` / `F` / `T` / `O` work even when right rail has focus
  (show-critical). Add `Esc` = exit current mode → Content.
- Colour-blind safe palette: red blackout + green "armed" is the worst
  pairing for deuteranopia. Use amber armed + red destructive instead.
- High-contrast variant for projector booths with stage spill onto the
  laptop screen.

### Optional intelligent assistance (kept narrow, transparent, off-by-default)

Within v3 scope:

- **Auto-fit corner pin** to detected screen rectangle in a test-pattern
  photo (manual confirm before commit). Useful for first-pass corner
  placement.
- **"Suggest mask"** from edges in the source image (operator approves
  polygon).
- **Coverage-vs-projector hint** — when a layer extends beyond the
  output frame, surface a non-modal warning in the mode banner (not a
  dialog).

These never fire automatically and never run during `GoLive`. Per
`src/show_day/`, anything that can panic must be wrapped in
`panic_restore` if it touches the render path.

---

## Appendix C — T4.21 sign-off matrix

| Screen state | Severity issues found | Notes / Phase-4 fix tickets |
|---|---|---|
| Launcher · empty recents | I14, I15 | Recents empty state + projector picker |
| Launcher · with recents | (verify list affordance) | Not in current screenshots — capture before sign-off |
| Launcher · demo fired | (verify state transition) | Capture before sign-off |
| Editing · empty canvas | I12 | Hint text / action mismatch |
| Editing · image loaded, no edit | I13, M5 | Canvas border anomaly (red) |
| Editing · Content mode | M7, M8, I16, N8, N9 | Triple-redundant transforms is the headline |
| Editing · Warp mode | M3, M4, I11, N1, N7 | Mode boundary + selection feedback |
| Editing · Mask mode | (mirror Warp checks) | No screenshot yet — request capture |
| Advanced · collapsed | I5 | Naming collision with toolbar |
| Advanced · per section | I2, I3, N4 | Effect chain semantics + binding picker (I3) |
| Cue strip · empty | — | Empty state already good |
| Cue strip · populated | I6 | Live / armed / idle states missing |
| Cue strip · crossfading | (verify T4.16 ring) | Not in current screenshots |
| Show-day strip | M1 | Blackout panic-grade promotion |
| Show-day strip · GoLive | (verify) | Capture before sign-off |
| Top chrome (revised) | M2, M6, I7, I8 | Three-zone restructure |
| Toasts · info / warn / error | (verify against accent) | Capture before sign-off |
