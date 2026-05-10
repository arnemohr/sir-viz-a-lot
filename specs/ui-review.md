# UI/UX + capability review — rmap control window

**Date:** 2026-05-10
**Reviewer:** staff UI/UX (heuristic + workflow-based) + creative-pro lens (VJ / projection-mapping artist / light-scene designer)
**Phase:** 4 — feeds T4.21 (Design QA pass over every screen state); also informs post-v3 capability planning
**Scope:** v3 capability scope per `specs/v3-capability-scope.md` and T4.23 (single-projector, photo + SVG, manual warp + corner pin, masks, scenes/crossfades, show-day strip, Go-live), plus forward-looking findings against `specs/roadmap.md` Phases 1–6.

This document combines a **workflow-based heuristic UX evaluation** (scored against live-show realism, not feature parity with multi-projector media servers) with a **forward-looking capability review** through the lens of a working VJ, projection-mapping artist, and light-scene designer. Findings are ranked by severity for the v3 deliverable (T4.21), and each carries — where one exists — a paired *Capability angle* showing the engine ceiling beneath the symptom and a *Roadmap home* (v3 polish, v3.1, v0.4, or `specs/roadmap.md` Phase 1–6).

> **How to read this:** every finding has a **UX symptom** (what an operator hits today). Most also carry a **Capability angle** (what's missing in the *engine*, not just the UI) and a **Roadmap home**. Findings that are purely UX (e.g. window title chrome, save/save-as labelling) intentionally omit the capability angle rather than fabricate one. The "Concrete recommendations" section ends with three new subsections (I, J, K) that grow out of capability angles, and the closing "Highest-leverage upgrades" section is paired: ranked v3 fixes (unchanged from the original review) followed by ranked post-v3 upgrades.

Source captures (Desktop screenshots, 2026-05-10):

- `Screenshot 2026-05-10 at 10.00.00.png` — Launcher (idle, no recents)
- `Screenshot 2026-05-10 at 10.00.12.png` — Editing, empty canvas
- `Screenshot 2026-05-10 at 10.00.31.png` — Editing, image loaded, no edit
- `Screenshot 2026-05-10 at 10.02.17.png` — Editing, Content mode, Advanced rail open
- `Screenshot 2026-05-10 at 10.02.47.png` — Editing, Warp mode (corner selected)
- `Screenshot 2026-05-10 at 10.03.01.png` — Editing, Warp mode + Advanced rail (Architectural Wash preset)

---

## Severity-ranked summary

### Must-fix before production (blocks live use or causes show-time errors)

**M1 — Blackout is not panic-grade.** *Where: bottom strip*
- *UX symptom:* sits in tiny dim text at the bottom-left, visually equal to "Outlines (O)" (a debug toggle). Under stress, in a dark venue, the operator must hit this in <1 s and never accidentally hit it.
- *Capability angle:* Blackout today is **visual-only** — it suppresses render output but cannot kill the lighting rig because no DMX / Art-Net / sACN output exists in the engine (`src/show_day/`, `src/controls/`). For a unified projection-and-light show it is half a panic button.
- *Fix in v3:* red, ~1.5× tall, leftmost, system-level shortcut (see Recommendation B).
- *Capability follow-on (roadmap Phase 4):* extend Blackout to a `LightSceneBlackout` once Art-Net output ships — same key, both surfaces dark in the same frame.

**M2 — "Go live" is a one-click state change with no armed/confirm affordance** *Where: top-right toolbar*
- *UX symptom:* wedged between two non-destructive items (Preview, document title) and styled identically to "Advanced". This is the single most consequential transition in the app.
- *Capability angle:* Go-live fires only the visual transition. In a unified show this same instant should fire a parallel light cue (roadmap Phase 4) and arm any NDI / Syphon / Spout outputs (output streaming is *not yet scoped* — v0.4 covers NDI **input** only). Today the transition is a single render-graph state flip with no fan-out hook.
- *Fix in v3:* primary button, accent-coloured, hold-to-arm or one-step confirm, `Cmd+Shift+Return` shortcut, visible armed-state ring (see Recommendation A).
- *Capability follow-on (roadmap Phase 4 + post-v3):* design the Go-live transition as an event with subscribers (light cue, NDI/Syphon stream-on, output failover arm) rather than a UI-only state flip.

**M3 — Mode model is unclear.** *Where: top toolbar*
- *UX symptom:* "Warp / Advanced / Preview / Go live" mixes a tool mode (Warp), a panel toggle (Advanced), a window action (Preview), and a state-machine transition (Go live). Three different verbs in one strip, all styled identically.
- *Capability angle:* the v3 mode set ("Warp / Mask / Content") is also incomplete — real installs need an *Output / Calibration* peer mode (multi-projector edge-blend, per-output gamma trim, calibration save/restore) and a *Cue / Show* peer mode (cuelist authoring, transport, BPM HUD). Designing the toolbar today as if those don't exist guarantees the redesign happens twice.
- *Fix in v3:* three peer mode pills (Warp / Mask / Content); demote Advanced to a rail-collapse toggle; fold Preview / Go live into the right-side action cluster (see Recommendation A).
- *Capability follow-on (v0.4 + roadmap Phase 5):* expand pill set to *Warp / Mask / Content / Output / Cue* so future modes don't break the muscle memory established in v3.

**M4 — Selected element has no canvas-side highlight.** *Where: canvas + right rail*
- *UX symptom:* the right rail says "Layer 0, corner (0, 1)" but all four warp handles look identical in the canvas. The operator can't tell which one a coordinate edit or arrow-key nudge will affect.
- *Capability angle:* the underlying warp is a bilinear N×M mesh (`src/render/warp.rs`), not bezier — so "selected vertex" is the only authoring primitive. If bezier / spline control points are added (not yet scoped — recommend for v0.4 or later), the visual selection language has to scale to *anchor + handles + tangents*. Designing the highlight today for "one of four corners" only is a one-version solution.
- *Fix in v3:* active vertex draws at 1.5× size with the warm accent ring; siblings stay subdued; right-rail coordinate row gets a "🎯 jump to vertex" affordance (see Recommendation C).
- *Capability follow-on (post-v3):* extend the selection language to bezier control points + tangent handles when the warp grows past bilinear.

**M5 — Two competing right-rail surfaces.** *Where: layout*
- *UX symptom:* a ~200 px "Advanced" panel *and* a separate context panel ("Layer 0, corner…") consume the right third of the window — pushing the canvas (the actual workspace) into a small middle column. On a 13" laptop the canvas is too small to warp accurately.
- *Capability angle:* the chrome budget will only get tighter post-v3. Surfaces still queued for the right side: cuelist (replaces 9 tiles → per-cue timing fields), BPM HUD, audio FFT bands strip, output preview thumbnail (per-output once multi-projector lands), MIDI-learn binding picker, light-fixture group editor. Designing for current panels alone underestimates the chrome load by ~2×.
- *Fix in v3:* merge the corner-context panel into the Advanced rail's Selected-layer section; collapse Advanced to a 36 px icon strip in Warp / Mask modes (see Recommendation D).
- *Capability follow-on (v3.1+):* establish a panel docking model now — every new surface (BPM HUD, audio bands, output preview) docks into the same right-side region with deterministic priority, never adds a new column.

**M6 — No "show name" in the title chrome.** *Where: title bar*
- *UX symptom:* the window title is the literal app name "rmap control" and "Untitled show" is masquerading as a toolbar button. Once a project is loaded, operators identify their show by filename — it must be in the title bar.
- *(Pure UX — no engine ceiling beneath this.)*

**M7 — Triple-redundant transform controls.** *Where: Content mode*
- *UX symptom:* in Content mode the layer's translate/scale/rotate/opacity appear in **three** places simultaneously: (1) the strip below the canvas, (2) the Advanced rail → Selected layer → Transform section, (3) the far-right "discoball_complete" panel with `Position / Scale / Rotate / Opacity`. Three sources of truth, three different layouts, same data.
- *Capability angle:* the Transform stage is also baked into the per-layer effect chain (`src/effects/mod.rs` — fixed order Color → Blur → Transform), and is *not reorderable*. Collapsing the UI to one canonical home forces the question of whether the engine should keep a fixed chain at all. Most established tools ship a reorderable chain or a small graph.
- *Fix in v3:* one canonical home — the right-rail Selected-layer card; the below-canvas strip becomes the mode-hint banner; the far-right context panel disappears in Content mode (see Recommendation H).
- *Capability follow-on (post-v3 — not yet scoped):* make the effect chain reorderable; promote the existing "External" effect hook into a real plugin point so the Tint stub is not the only example of an extensible stage.

**M8 — Two divergent entry points to Warp / Mask mode.** *Where: Content-mode side panel + top toolbar*
- *UX symptom:* the top toolbar has the "Warp" / "Advanced" pills; the far-right context panel has separate **"Edit warp"** and **"Edit mask"** buttons under "Placement / Warp" → "1×1 grid · mask vertices: 0". Same destinations, different labels, different locations.
- *Capability angle:* the mask system today is polygon-only with feather (`src/render/sdf.rs`, `MaskPolygon` in schema). The mode pill cluster is the right home for *all* surface-shaping tools, including peers that don't exist yet: inverse mask, luma key, chroma key, soft-edge feather painting. Design the entry once with growth in mind.
- *Fix in v3:* mode pills in the toolbar are the single entry point; remove the context-panel buttons (see Recommendation H).
- *Capability follow-on (post-v3 — not yet scoped):* expand mode pills (or a sub-row inside Mask) to carry inverse + key peers; let the mask schema grow from `MaskPolygon` to `MaskGraph` without breaking the entry point.

### Important (degrades workflow, learnability, or recovery)

**I1 — Coordinate precision is inconsistent and unitless.** *Where: right context panel*
- *UX symptom:* `x: 0.83113`, `y: 0.2735` — five vs four decimals, and 0–1 normalised space exposed raw. Operators think in pixels, % of output, or "corner 4 of 4".
- *Capability angle:* a calibration-file workflow (venue-scoped warp + mask + gamma + monitor identity, decoupled from content — roadmap Phase 6) needs canonical pixel-and-percent readouts anyway, so fixing the coordinate display in v3 also seeds the calibration export format.
- *Fix in v3:* show pixel + percent of output, single decimal place, name corners TL/TR/BL/BR (see Recommendation G).
- *Capability follow-on (Phase 6):* same coordinate format becomes the canonical surface in the calibration export.

**I2 — Apply / Reload semantics on the Effect preset are opaque.** *Where: Effect chain header*
- *UX symptom:* two adjacent buttons next to "Architectural Wash" with no visible state difference — does Apply commit the dropdown choice? Does Reload re-fetch from disk or revert the chain?
- *Capability angle:* the buttons conflate two missing things — a **preset library** (no browser, no save / delete / star / export) and **chain dirty-state** (already tracked by the v3 Mutation system, see `src/project/CLAUDE.md`). The current pair of buttons hides both gaps.
- *Fix in v3:* `Use preset` (commits dropdown to chain) + `Revert preset` (rolls back), each disabled based on dirty state. Drop "Reload" entirely and let undo handle it (see Recommendation E).
- *Capability follow-on (post-v3 — not yet scoped):* ship a real preset library with browser, search, save / delete / star / export.

**I3 — The Effect chain `static` dropdown is the binding mode** *Where: Effect chain rows*
- *UX symptom:* the dropdown lists `static / sine / tri / noise / bpm / audio` (per `src/modulators/mod.rs`) but is presented as a generic combo-box with no affordance suggesting "this is a binding source".
- *Capability angle:* this dropdown is the **single operator-facing surface for the entire live-input system**, and the live-input system itself is partly stubbed. The MIDI and OSC **buses** exist (`src/controls/midi.rs` decodes Note On 60–71 → `Command`; OSC listens on a UDP port) — but the **parameter-binding path** does not: `Param::Bound` in `src/controls/param.rs` is `#[allow(dead_code)]` and its `bound_returns_zero_v1` test confirms it always resolves to `0.0`; effect parameters today are driven by the `Modulator` enum, not `Param<f32>`. So a CC twist literally cannot move a slider yet. There's also no MIDI learn UX, no per-CC mapping, no OSC address binding from the parameter row.
- *Fix in v3:* labelled binding picker with antenna / jack icon; rename to surface intent; make the dropdown the canonical place to switch sources (see Recommendation E and Recommendation I).
- *Capability follow-on (v0.4 — OSC; not yet scoped — MIDI):* per `specs/v3-capability-scope.md`, **OSC live parameter binding UI is v0.4 scope** (a "Visual patch panel: OSC address → layer parameter mapping"). MIDI parameter binding (CC / PC / pitch routing) is *not* in v3.1 or v0.4 scope — it is a roadmap gap, not a scheduled milestone, and shipping it requires both engine work (extend the MIDI decoder, populate the source registry, route into `Param::Bound` or extend `Modulator`) and UX work.

**I4 — `?` button and "Glossary" coexist at the same toolbar level.** *Where: top toolbar*
- *UX symptom:* two help affordances, both top-left, both small text. Pick one.
- *(Pure UX — fold into a single Help menu.)*

**I5 — "Advanced" appears twice with different meanings:** *Where: toolbar + right rail*
- *UX symptom:* a top-toolbar item *and* a right-rail header. If they're related (toggle), make that obvious. If not, rename one.
- *(Pure UX — covered by demoting toolbar Advanced to a rail-collapse toggle in M3.)*

**I6 — Cue strip lacks current/next/armed indicators.** *Where: bottom cue strip*
- *UX symptom:* three identical-looking cue tiles, no marker for "currently fired" or "next on Space". Show operators need this to recover from any reordering.
- *Capability angle:* the cue model is **snapshot-only with linear crossfade**, gated on layer-topology match (`src/project/mod.rs`). There is no per-cue *in-time / hold / out-time / follow / BPM-quantize / timecode-trigger*, no chain-of-cues, no go/back transport. The 9 tiles are placeholders for a real cuelist; designing tile state today should leave room to grow.
- *Fix in v3:* idle / armed-next / live tile states + crossfade ring during transitions (see Recommendation F).
- *Capability follow-on (roadmap Phase 5):* tiles become rows in a cuelist with per-cue timing fields; tile UI stays forward-compatible if each tile's data model is a `Cue` struct, not just a `SceneIndex` (see Recommendation J).

**I7 — Multi-display identity is invisible at the top level.** *Where: top chrome*
- *UX symptom:* "Display output" is buried in the Advanced rail. Operators need a persistent "→ Output: BenQ LU935 (1920×1200)" badge so they know what they're about to fullscreen *before* hitting Go live.
- *Capability angle:* a single-line label is correct for v3 (single projector) but becomes a panel once multi-projector + edge-blend (v0.4 stub — scoped) lands; NDI / Syphon / Spout *output* (not yet scoped — v0.4 covers NDI input only) would add a stream-on toggle. At that point the badge needs to expand into per-output gamma trim + edge-blend gradient + calibration verify + stream-on toggle.
- *Fix in v3:* persistent output badge at top-right (see Recommendation A).
- *Capability follow-on (v0.4 / Phase 6):* badge collapses out of an Output panel rather than being replaced (see Recommendation K).

**I8 — Save / Save as… inconsistency.** *Where: top toolbar*
- *UX symptom:* only "Save as…" is shown — fine while untitled, but once named, both Save and Save as… should appear with Save as primary. Autosave (per spec) should also surface a "Saved 13 s ago" indicator.
- *(Pure UX — already plumbed via T4.6 autosave.)*

**I9 — Left rail "+ Add image" is the only affordance for media** *Where: left rail*
- *UX symptom:* for a 5-layer show it will overflow without scroll, search, or grouping. Plus there's no visible "where is this image actually used?" reverse lookup.
- *Capability angle:* media is **stills (PNG/JPG/WEBP, GIF first frame) + SVG only** (`src/svg_layer.rs`, `src/image_layer.rs`). Once video lands (Phase 1 / v0.4) the left rail needs thumbnail scrubbing, in/out points, loop mode — the same row that today shows a static thumbnail. Plan the row layout for video, not just stills.
- *Fix in v3:* scroll, search, layer solo/mute (see Recommendation H follow-on / Capability synthesis).
- *Capability follow-on (Phase 1 / v0.4):* thumbnail scrubbing + in/out points + loop mode for video layers.

**I10 — Warp mode shows a hint string ("Drag the corners…")** *Where: below canvas*
- *UX symptom:* excellent in Warp mode, but the same pattern is missing in Mask mode, Content mode, Effects editing.
- *Capability angle:* the hint banner should also carry a *capability hint* when the operator is reaching for something the engine doesn't yet support (e.g. "Bezier handles — coming post-v3", "Inverse mask — coming post-v3"). Better than silent absence.
- *Fix in v3:* first-class `ModeHintBanner` component, every mode has one (see Recommendation H).
- *Capability follow-on (ongoing):* surface capability availability inline so operators learn the engine's edges from the UI, not from docs.

**I11 — No visual mode boundary on the canvas itself.** *Where: canvas frame*
- *UX symptom:* the canvas frame is the same regardless of Warp / Mask / Content — only the toolbar pill changes. A subtle border tint or corner badge would prevent the classic "I edited the warp when I meant the mask" mistake.
- *Capability angle:* once Output / Cue peer modes land (M3 follow-on) the canvas tinting needs more colour codes — design the palette today for ~5 modes, not 3.
- *Fix in v3:* thin 1-px mode-tinted border (warp = warm accent, mask = desaturated cool, content = neutral) (see Recommendation C).

**I12 — Empty-canvas hint is contextually wrong.** *Where: empty Editing*
- *UX symptom:* with no layer present, the toolbar already shows "Drag to move. Shift-drag to scale. Alt-drag to rotate." — but there's nothing to drag. The hint should swap with the canvas content.
- *(Pure UX — see I10 ModeHintBanner work.)*

**I13 — Canvas border colour shifts meaning silently.** *Where: canvas frame*
- *UX symptom:* the image-loaded shot shows a **red** canvas frame; the warp shot shows **white/grey**; the empty shot shows **dashed grey**. Three different colours, no legend. The red border is likely an unintended draw-buffer artifact.
- *(Pure UX / rendering bug — investigate and standardise per I11.)*

**I14 — Launcher "Open a recent show" has no empty state.** *Where: launcher*
- *UX symptom:* when dimmed there's no caption like "No recent shows yet — try the demo to explore." Newcomers will think the app is broken.
- *(Pure UX — also note the launcher is currently a skeleton per Phase-2 task scaffolding; capture before sign-off.)*

**I15 — Launcher's projector line is not actionable.** *Where: launcher*
- *UX symptom:* "Projector: Built-in Retina Display" looks like a label, not a control. With multiple monitors connected, operators need to confirm and choose **before** starting a new show.
- *Capability angle:* in v0.4 (multi-projector) this becomes a multi-select with per-output assignment. The launcher is the right place to commit to a layout before the show file even loads.
- *Fix in v3:* clickable monitor selector + test-pattern affordance.
- *Capability follow-on (v0.4):* multi-output picker on the same row.

**I16 — Selection status appears inside the canvas drawing area** *Where: Content mode*
- *UX symptom:* "selected: layer 0 (discoball_complete)" sits on top of the canvas. It overlaps the artwork at high zoom and is invisible against bright content.
- *(Pure UX — move into chrome or render with a solid background pill.)*

### Nice-to-have (polish, accessibility, scale)

**N1 — Hit-target sizes for warp handles look ≤ 12 px** *Where: canvas*
- *UX symptom:* fine on a desk, painful on a trackpad in low light.
- *Capability angle:* once the mesh grows past 4-corner (16×16 mesh detail today is a ceiling; bezier control points are not yet scoped) hit-area policy needs to scale per zoom level and per vertex density. Design the policy now so it doesn't break at high mesh detail.
- *Fix in v3:* bump active hit area to ~24 px while keeping the visible handle small; add keyboard nudge (arrow = 1 px, ⇧arrow = 10).
- *Capability follow-on (post-v3):* zoom-aware hit-area scaling + tangent-handle hit policy for bezier vertices.

**N2 — "Reset this corner" exists; "Reset all corners" doesn't.** *Where: right context*
- *(Pure UX — add with confirm.)*

**N3 — No clock / showtime indicator.** *Where: top chrome*
- *UX symptom:* some operators run cues against external timecode in their head; even a simple wall clock helps. (Out of v3 scope per T4.23 but cheap to add.)
- *Capability angle:* this slot is the eventual home for a **transport HUD**: timecode (LTC / MTC) sync, MIDI clock, and the BPM tap surface. *Not currently in v3.1 or v0.4 scope.*
- *Recommended fix (suggest for v3.1):* small clock + BPM tap badge.
- *Capability follow-on (roadmap Phase 5):* full transport with LTC/MTC/MIDI-clock sync.

**N4 — Effect chain `value` suffix is repeated on every row** *Where: effect chain*
- *(Pure UX — unit/label belongs in the parameter row's left column.)*

**N5 — Diagnostics is hidden at the bottom of Advanced.** *Where: right rail / chrome*
- *UX symptom:* for show-day, surface CPU/GPU/fps and "panic-restored frames" as a small always-visible badge per `src/render/CLAUDE.md` `panic_restore`.
- *Capability angle:* the diagnostics surface should also report dropped-frame count, audio-input level (when audio FFT modulators are active), and — once Phase 4 lands — DMX universe activity (Art-Net packet rate, fixture group counts).
- *Fix in v3:* persistent fps + panic-restored badge.
- *Capability follow-on (Phase 4+):* audio level meter + DMX universe LED in the same badge cluster.

**N6 — Accent unification (T4.20).** *Where: theme*
- *UX symptom:* yellow-ish corner handles vs. blue cue tiles vs. white text. After T4.20 the handles should match the chosen warm accent and only red should signal destructive/error states.
- *(Pure UX — covered by T4.20.)*

**N7 — Source-image stars are confusable with UI markers.** *Where: warp mode*
- *(Pure UX — handle shape/halo must be unambiguous against any photo content.)*

**N8 — Far-right context panel duplicates Position fields.** *Where: Content mode*
- *(Pure UX — collapses with M7 fixed.)*

**N9 — Document title flips between "Untitled show" and selected-layer name** *Where: top chrome*
- *(Pure UX — keep document name fixed; selection status in its own slot.)*

---

## Workflow walk-through (what a tech actually does)

The IA roughly matches the v3 workflows but has rough edges at every transition.

1. **Open → load show.** Launcher is fine; once in Editing, the window title doesn't reaffirm which show. **(M6)**
2. **Add image / SVG.** Left rail "+ Add image" works for one. Beyond ~4 layers it will need scroll + per-layer visibility/solo. Once video lands the row's anatomy changes meaningfully. **(I9)**
3. **Surface / mapping.** The "Warp" toolbar pill is the mode entry point; corner-pin works as expected. Switching to Mask requires going to Advanced → Mapping (inferred). Mask should be a peer toolbar pill with Warp, not buried. **(M3, I11)**
4. **Projector setup.** Single projector v3, but "which display the projector lives on" should be a top-chrome badge, not buried. Multi-projector is the obvious v0.4 extension. **(I7, I15)**
5. **Content assignment.** Layer ordering, blend, effects all live in the Selected-layer panel. Clear. The Effect chain UI is dense but legible. The Apply/Reload pair undermines confidence. The triple-redundant transform UI undermines it further. The fixed effect ordering caps creative range; reordering is a post-v3 lift, not yet scoped. **(I2, I3, M7, N4)**
6. **Cues / scenes.** Cue tiles render but lack state. Save Cue tile is good. The 9-tile snapshot model is forward-compatible to a real cuelist if planned now. **(I6)**
7. **Rehearsal / Go-live / panic.** Both the Go-live transition and the Blackout response are under-affordanced for the moments that matter most — and visual-only on the lighting side. **(M1, M2)**

Where users will get lost: the **mode boundary** (Warp vs Mask vs Content), the **Apply lifecycle** (preset → chain → save), the **transform redundancy** (which of three sliders is canonical?), and — for any operator who's worked with media servers before — the **invisible live-input system** (where do I learn a MIDI knob? where's the BPM display? where do I see the audio bands?).

---

## What the screenshots already get right

Flag these so they don't regress in the T4.21 sign-off:

- **Mode-aware rail collapse**: Warp mode hides the Advanced rail and shows only the corner-context column. Content mode opens both. This is a strong pattern — make it explicit policy rather than incidental.
- **Empty-state copy on the canvas**: "Drop a photo or SVG here to begin." is short, correct, friendly. Use the same voice elsewhere (cue strip already does: "Save your first cue").
- **Show-day strip is visible in every editing state** (Blackout / Freeze / Test / Outlines on the bottom row, every screenshot). That continuity is exactly right — fix only the visual hierarchy (M1), don't relocate.
- **Per-mode contextual hint line** is excellent. Codify as a `ModeHintBanner` component, then ensure every mode has one.
- **Launcher hierarchy** is broadly correct: primary action (Start new), secondary (Open recent), tertiary (Try demo), with an output-target footer. Keep the structure; fix the affordances (I14, I15).
- **Snapshot + crossfade scene model** with topology gating (`snapshots_share_layer_topology` in `src/project/mod.rs`) is a solid foundation; the cue-tile UI grows on top of it without changing the data model.
- **Modulator system architecture** (`src/modulators/`) cleanly separates source from parameter, so MIDI-learn / OSC-learn UX can be added without re-architecting effects.

---

## Concrete recommendations

### A. Restructure the top chrome (M2, M3, M6, I5, I7, I8, N9)

Three zones, three different visual weights:

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ⌂ MyShow.rmap.json  ↶ ↷  Saved 4s    │ Warp · Mask · Content │  Output: ▣│
│                                       │                       │           │
│                                                              [ Go live ▶ ]│
└──────────────────────────────────────────────────────────────────────────┘
```

- **Left:** document identity + undo/redo + autosave indicator. Replace "Untitled show" pseudo-button with a real titlebar that flips to the filename on save. Keep "Save as…" under a menu.
- **Center:** **mode pills** — these are tools, not panels. Three peers in v3: Warp / Mask / Content. The currently active one gets the warm accent (T4.20). "Advanced" is **not** a mode; demote it to a right-rail collapse toggle. Plan the pill cluster to grow to *Warp / Mask / Content / Output / Cue* in v0.4 / Phase 5.
- **Right:** **Output badge** (which display, resolution, identity name) and a **Go live** primary button — bigger, accent-coloured, with a one-step confirm or hold-to-arm. Pair with `Cmd+Shift+Return` and a visible armed-state ring.

`?` and "Glossary" → fold into a single Help menu at the far right.

### B. Make show-critical controls panic-grade (M1)

Promote the bottom strip from a status bar to a **show-day toolbar** with explicit visual hierarchy:

```
[  ⏻ BLACKOUT  (B) ]   [ ❄ Freeze (F) ]   [ ▦ Test (T) ]   [ ⌗ Outlines (O) ]
   destructive red       neutral             neutral             neutral
   60-px tall            40-px              40-px              40-px
   always one click
```

- Blackout: red, ~1.5× tall, leftmost, never scrolls off, single-letter shortcut already there — keep `B` as a system-level binding.
- Freeze / Test: neutral but visible state pill when active.
- Outlines: this is debug — group it visually away from Blackout / Freeze (e.g. right side or under a "Diagnostics overlays" cluster).

When `GoLive` is active, the strip should be even more prominent and pinned to the top of z-order regardless of other panels.

**Capability follow-on (Phase 4):** Blackout becomes `LightSceneBlackout` once Art-Net output ships — the same `B` key kills both projector frames and DMX channels in the same instant.

### C. Selected-element feedback on the canvas (M4, I11, N1)

- Active warp / mask vertex draws at 1.5× size with the warm accent ring; siblings stay subdued.
- The right context panel's coordinates always pair with a "🎯 jump to this vertex on canvas" affordance.
- Mode tint on canvas frame: thin 1-px border in mode-colour (warp = warm accent, mask = a desaturated cool, content = neutral). Plan the palette for ~5 modes (Output peer mode lands with v0.4 multi-projector edge-blend; Cue peer mode at roadmap Phase 5).
- Hit area: 24 px logical, 12 px visual. If bezier handles are added (post-v3), hit policy must scale per zoom + per vertex density.

### D. Reclaim canvas width (M5)

Current right-side allocation is roughly: Advanced rail (~200 px) + Context panel (~150 px) = 350 px on a 1280-wide laptop = 27% of the window. Options:

1. **Merge** the corner-context panel into the Advanced rail's "Selected layer" section, since context is *always* the selection.
2. Make the Advanced rail collapsible to a 36-px icon strip (T4.20-style) so during warping you get the full canvas.
3. Default-collapse Advanced when entering Warp / Mask modes; default-expand for Content. (This pattern is partially in place — make it policy.)

Establish a **panel docking model** now: every new right-side surface (BPM HUD, audio bands strip, output preview thumbnail, MIDI-learn picker, light-fixture group editor) docks into the same region with deterministic priority. No new columns post-v3.

### E. Effect chain clarity (I2, I3, N4)

- Rename the row dropdown from `static` to a labelled **binding picker**: `Source: static · sine/tri/noise · BPM · audio band 1–8 · MIDI CC · OSC addr`. Use a small antenna / jack icon. (See Recommendation I for the full surface.)
- "Apply" / "Reload" → `Use preset` (commits dropdown to chain) and `Revert preset` (rolls chain back). Disable each based on dirty state. Even better: drop "Reload" entirely and let undo handle it (per `Mutation` in `src/project/CLAUDE.md`).
- Move the unit (`px`, `deg`, multiplier) to the left of the spinner; remove the trailing "value" label everywhere.

### F. Cue strip status (I6)

Three states per tile: `idle`, `armed/next`, `live/firing` (with a 3-state crossfade ring during transitions per spec T4.16). Keyboard: `Space` fires armed; arrows move arm cursor without firing. Forward-compatible to the cuelist work in Recommendation J — each tile carries a `Cue` struct, not just a `SceneIndex`.

### G. Coordinate readouts (I1)

```
Corner 4 of 4 (BR)      x  1738 / 1920 px   (90.5%)
                        y    525 / 1080 px   (48.6%)   ⊘ reset
```

Show pixel + percent of output; pick a single decimal place; name corners (TL/TR/BL/BR) for verbal communication on a multi-person crew. Same format becomes the canonical surface in the calibration export (Phase 6).

### H. Collapse the transform redundancy (M7, M8, N8)

- One canonical home for layer transforms: the right-rail Selected-layer card.
- The below-canvas strip becomes the **mode hint banner** only.
- The far-right context panel disappears in Content mode; in Warp / Mask it shows only mode-specific data (selected vertex coords, vertex count).
- "Edit warp" / "Edit mask" buttons in the side panel are removed — mode pills in the toolbar are the single entry point.
- Add per-layer **solo / mute** to the left rail (zero-cost UX win, hard ceiling on usability without it past ~5 layers).

### I. Surface the live-input system (I3, M7 follow-on, N5 follow-on)

This is the **single highest-ROI capability surface** that doesn't touch the render graph — but it isn't pure UX. The MIDI port subscription bus and OSC UDP listener are real (`src/controls/midi.rs`, `src/controls/osc.rs`), and the audio FFT modulator is wired (`src/modulators/audio.rs`); but the *parameter-binding path* is stubbed: `Param::Bound` is dead-coded and effect parameters use the `Modulator` enum, not `Param<f32>`. So the work below is engine-plumbing-plus-UX, not UX-only — call it honestly. v0.4 already scopes the OSC half; recommend extending the same release to MIDI parameter binding (the binding picker, learn workflow, and registry plumbing ship once and serve both transports).

- **Binding picker** (replaces the `static` dropdown on every parameter row): `static · sine / tri / noise · BPM · audio band 1–8 · MIDI CC · OSC addr`. Antenna / jack icon left of the picker so it reads as "input source", not generic combo.
- **MIDI / OSC learn**: right-click any parameter row → "Learn next MIDI CC" or "Learn next OSC address". Listening state has a clear visible cue (pulsing accent ring on the row, ESC to cancel). Once received, the binding is editable in the same row.
- **BPM HUD**: small badge in the top chrome (next to the wall clock per N3) showing live BPM, tap source (Space / MIDI 60 / OSC `/rmap/tap`), and a 1/2/4/8-bar quantize selector for cue firing. *Not currently in v3.1 or v0.4 scope; recommend for v3.1.*
- **Audio bands strip**: when an audio source is active, expose the 8 FFT bands as a small horizontal meter (in Diagnostics, or as a collapsible footer above the show-day strip). Each band is drag-source for "bind this band to that parameter" — the most direct binding UX possible. *Not currently in v3.1 or v0.4 scope; recommend for v3.1 or v0.4.*
- **Visible binding indicators on parameter rows**: a small "MIDI CC 21" or "OSC /rmap/blur/radius" tag next to bound parameters; click → unbind / relearn.

### J. Cuelist as the eventual home for the cue strip (I6)

Design the strip in v3 so it can grow per-cue fields (in-time, hold, out-time, follow vs go-on-trigger, BPM-bar quantize, optional timecode trigger) without breaking the visual model. The 9-slot snapshot is forward-compatible if each tile carries a `Cue` struct with fade fields, rather than just a `SceneIndex`. The transport (Space = go, ←/→ = move arm, Backspace = back-cue) is roughly the standard everyone has muscle memory for.

This unblocks the roadmap Phase 5 cuelist work without re-architecting v3 cue state.

### K. Output as a panel, not a badge (I7, M2)

The "Output: BenQ" line becomes an **Output panel** as more output channels land. v0.4 scopes a two-projector edge-blend stub and per-projector colour calibration (RGB matrix); the panel carries per-output gamma trim, edge-blend gradient slider, and calibration verify. NDI / Syphon / Spout *output* is **not yet scoped** (v0.4 covers NDI **input** only); when added, a stream-on toggle joins the panel. When Art-Net / sACN output ships (roadmap Phase 4), the panel grows a fixture-group editor and a color-from-pixel mapping surface — same panel, more rows.

Design the v3 badge so it can collapse out of an Output panel rather than be replaced by one.

---

## Proposed "ideal" workspace layout

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

Three columns with the canvas dominant; cues integrated into the left column (closer to layers, since they reference the same media); selection and editing live together on the right; show-day strip is the visual base of the window with Blackout outsized.

Forward-compatible additions slot in without restructure: BPM HUD + clock in the title bar, audio bands strip as a collapsible band above the show-day strip, output preview thumbnail in the right rail (top), MIDI-learn binding pickers as inline pills on parameter rows.

---

## Capability synthesis — three lenses

The per-finding *Capability angle* notes above accumulate into three cross-cutting themes. This synthesis pulls them out so the team can see the engine investment plan, not just the UI-fix list.

### VJ lens — live, audio/MIDI-driven, music-locked performance

What an operator running a club / festival / event-driven show needs, beyond the v3 baseline:

- **Video as a first-class layer type** *(I9)* — loops, scrub, rate (incl. reverse), in/out points, sync-to-BPM playback. Largest single capability ceiling today (engine is stills + SVG only). **Roadmap home: v0.4 ("Video playback" — mp4/H.264 decoded on a background thread per `specs/v3-capability-scope.md`); deepens through roadmap Phase 1 (photo/video treatment grammars).**
- **OSC parameter binding UI** *(I3)* — operator-facing patch panel mapping OSC addresses to layer parameters; named in v0.4 scope as "Visual patch panel: OSC address → layer parameter mapping" but UX still TBD. **Roadmap home: v0.4 (scoped).**
- **MIDI parameter binding + learn UX** *(I3)* — *not* in v3.1 or v0.4 scope today. Bus exists (Note On 60–71); CC / PC / pitch routing into parameters is engine + UX work (extend the decoder, populate the `InputState` source registry, route into `Param::Bound` or extend `Modulator`, then build Learn). **Roadmap home: not yet scoped — flag as a recommended addition to v0.4 or a v0.5 candidate; treat as the highest-leverage *un-scoped* capability gap.**
- **Audio-reactive UI surface** *(I3, N5)* — 8-band FFT plumbing exists in `src/modulators/audio.rs`; operator-facing meter + drag-to-bind is the missing UX. **Roadmap home: not yet scoped — recommend for v3.1 or v0.4.**
- **BPM HUD + beat-locked cue firing** *(N3, I6)* — `Modulator::Bpm` exists, tap plumbed via Space and MIDI Note 60; no HUD, no quantize. **Roadmap home: HUD is not yet scoped (recommend v3.1); quantized cue firing belongs to roadmap Phase 5 (cue list / timeline-lite).**
- **Effect chain reordering + preset library** *(M7, I2)* — chain is fixed-order today (Color → Blur → Transform per `src/effects/mod.rs`); preset library doesn't exist, only "Apply / Reload" of an opaque preset. **Roadmap home: not yet scoped — recommend as v0.4 / v0.5 work.**
- **A/B deck pattern** — two scenes loaded simultaneously with a manual fader between them; standard VJ tool (Resolume, vMix). Could re-use `Project.crossfade_duration_s` plumbing driven manually instead of by recall. **Roadmap home: not yet scoped — recommend as v0.5+.**
- **Layer solo / mute, groups, search, reverse-lookup** *(I9, H follow-on)* — visibility toggle only today; missing solo is painful past ~5 layers. **Roadmap home: not yet scoped — recommend for v3.1.**

### Projection-mapping lens — install, calibrate, repeat

What a projection-mapping artist setting up on architecture needs, beyond the v3 baseline:

- **Two-projector edge-blend stub** *(I7, K)* — second `OutputWindow` on a second monitor, per-projector warp + mask, shared blend region with configurable overlap and falloff. Explicitly named in v0.4 scope; full calibration workflow deferred further. **Roadmap home: v0.4 (scoped — stub only).**
- **Per-projector colour calibration** *(I7, K)* — extends the existing per-display gamma / brightness / contrast override with a full RGB matrix. Named in v0.4 scope. **Roadmap home: v0.4 (scoped).**
- **NDI input layer** *(I9)* — receive an NDI stream as a layer source. Named in v0.4 scope. **Roadmap home: v0.4 (scoped).**
- **Bezier / spline mesh warp** *(M4, N1)* — on top of the existing bilinear N×M mesh in `src/render/warp.rs`. Curved walls, columns, organic shapes. **Roadmap home: not yet scoped — recommend for v0.4 or v0.5.**
- **NDI / Syphon / Spout output** *(M2, I7, K)* — feed a media server, capture rig, or stream encoder. macOS-first → Syphon. *Note: v0.4 scopes NDI **input** but not output — these are different capabilities.* **Roadmap home: not yet scoped — recommend for v0.5.**
- **Inverse mask + luma / chroma key** *(M8)* — mask is polygon + feather only today (`src/render/sdf.rs`). **Roadmap home: not yet scoped — recommend for v0.4 or v0.5.**
- **Test-pattern depth** *(M3, B follow-on)* — single test toggle today; need alignment cross, dot grid, color bars, edge-blend gradient, focus chart, geometry verify (concentric circles). **Roadmap home: not yet scoped (depth); edge-blend gradient slots into v0.4 edge-blend stub.**
- **Calibration save/restore decoupled from content** *(I1, G)* — venue-scoped warp + mask + gamma + monitor identity travels separately from the show file. *Note: schema v5 portable monitor (T4.12 / T4.13) in v3.1 partly addresses monitor-identity portability, but the full calibration-file split is larger work.* **Roadmap home: roadmap Phase 6.**
- **Persistent output preview thumbnail** *(I7, K)* — surfaced in the control window header at all times, not just inside the preview-as-projector window. **Roadmap home: not yet scoped — recommend for v3.1.**

### Light-scene-design lens — projection and light as one show

What a light-scene designer authoring a unified video + light show needs, beyond the v3 baseline:

- **Art-Net / sACN output graph** *(M1, M2, N5)* — anchor capability; everything else in this lens depends on it. **Roadmap home: roadmap Phase 4.**
- **Fixture groups + pixel maps + color-from-pixel** *(M2)* — sample N pixels of the canvas → DMX channels. Cheapest credible entry point and produces strong results from day one. **Roadmap home: roadmap Phase 4.**
- **Light cues authored in parallel to video cues** *(I6, J)* — the same scene snapshot carries both; cuelist (Recommendation J) extends to fixtures. **Roadmap home: roadmap Phase 4 + Phase 5.**
- **Light-scene blackout fired with M1** — same `B` key, both surfaces dark. **Roadmap home: roadmap Phase 4.**
- **Light cue fired with M2 Go-live** — Go-live is a single "show start" event with subscribers, not a UI-only state flip. **Roadmap home: roadmap Phase 4.**
- **BPM-locked fixture chases / pulses** *(N3, I lens)* — same `Modulator::Bpm` plumbing drives DMX values. **Roadmap home: roadmap Phase 4 + Phase 5.**
- **RGBW + color-temperature-aware mixing** — out of scope for early Phase 4. **Roadmap home: roadmap Phase 6.**

---

## Consistency / design-system notes (groundwork for T4.20 follow-on)

- **One accent for "user-interactable handle"**: warp vertices, mask vertices, drag-source markers, primary buttons, mode-active pill — all warm accent. Cue tiles should *not* use the accent unless armed.
- **One destructive colour**: red for Blackout + delete-confirms + validation errors. Nothing else. (See I13 — clear up the spurious red canvas border.)
- **One "armed/live" colour**: a saturated state distinct from accent (e.g. amber pulse) used only when a transition is loaded but not fired (Go-live armed, cue armed, MIDI-learn listening).
- **Component vocabulary**: standardise a `BindingPicker` (the I3 surface), `ParameterRow` (label · unit · spinner · binding picker · learn-state pill), `ModePill`, `ModeHintBanner`, `StatusBadge`, `PanicButton`, `OutputBadge` (collapses out of `OutputPanel` once v0.4 multi-projector + per-projector colour calibration land). Pull every panel through these.
- **Naming**: align "layer / effect / cue / scene / mapping / mask / output / fixture / cuelist" across rail, menus, shortcuts, and glossary. The glossary window (per T4.11) is the reference doc for these terms — every label in the UI should match it.

---

## Accessibility for dark venues

- Body text minimum 13 px, monospace numerics 14 px (current `0.83113` reads as ~10 px).
- Focus ring on every interactive control, keyboard-reachable via tab. Currently invisible on the corner handles.
- Keyboard shortcuts: confirm B / F / T / O work even when right rail has focus (they're show-critical). Add `Esc` = exit current mode → Content.
- Colour-blind safe palette: red blackout + green "armed" is the worst pairing for deuteranopia; amber armed + red destructive avoids it.
- High-contrast variant for projector booths with stage spill onto the laptop screen.

---

## Optional intelligent assistance (kept narrow, transparent, off-by-default)

Within v3 scope:

- **Auto-fit corner pin to detected screen rectangle** in a test-pattern photo (manual confirm before commit). Useful for first-pass corner placement.
- **"Suggest mask"** from edges in the source image (operator approves polygon).
- **Coverage-vs-projector hint**: when a layer extends beyond the output frame, surface a non-modal warning in the mode banner (not a dialog).

These should never fire automatically and never run during `GoLive`. Per `src/show_day/`, anything that can panic must be wrapped in `panic_restore` if it touches the render path.

---

## T4.21 sign-off matrix

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

---

## Highest-leverage upgrades

### Highest-leverage v3 fixes (ranked)

If only three things ship before merging Phase-4:

1. **Collapse the triple-redundant transform UI (M7) and unify the two mode-entry paths (M8).** Single biggest contributor to "I edited the wrong thing" mistakes during a live run-up.
2. **Promote Blackout (M1) and Go-live (M2) to panic-grade affordances.** Without these, the rest of the polish doesn't matter at show time.
3. **Fix the canvas-border colour drift (I13) and standardise the mode-tinted frame (I11).** Operators need to read the canvas state at a glance — the current accidental red border is actively misleading.

The Mask-mode, "Open recent" / "demo fired" launcher states, cue-crossfade state, GoLive show-day strip, and toast variants are not represented in the current captures — request these before T4.21 sign-off.

### Highest-leverage post-v3 upgrades (ranked)

If only three things ship after v3.0 hits GA, in priority order:

1. **OSC parameter binding UI + extend with MIDI binding (Recommendation I).** OSC is *already in v0.4 scope* per `specs/v3-capability-scope.md` ("Visual patch panel: OSC address → layer parameter mapping"); MIDI parameter binding is *not yet scoped* anywhere. The MIDI bus exists (`src/controls/midi.rs` decodes Note On only) but the routing path through `Param::Bound` is dead-coded today (`#[allow(dead_code)]`, the test confirms `bound_returns_zero_v1`), and effect parameters use the `Modulator` enum, not `Param<f32>` — so this is genuinely engine + UX work, not pure UX. The lift is real but bounded — it doesn't touch the render graph. The strategic move is to ship the OSC binding UI as v0.4 already promises *and* extend the same surface to MIDI in the same release; the binding picker, learn workflow, and registry plumbing reuse. Turns rmap from "scriptable tool" into "instrumental show engine". **Roadmap home: v0.4 (OSC scoped, MIDI recommended addition).**
2. **Video as a first-class layer type (I9, VJ-lens).** Single biggest perceived ceiling for any operator who isn't doing strictly photo work. v0.4 already commits to "mp4 / H.264 minimum viable path: decoded on a background thread, uploaded to GPU each frame." Forces the threading / texture-upload work that all later capabilities (Syphon-out, photo-treatment grammars applied to video frames, BPM-locked playback) need too. **Roadmap home: v0.4 (scoped); deepens through roadmap Phase 1 (photo/video treatment grammars).**
3. **Art-Net output stub + color-from-pixel fixture mapping (M1 follow-on, M2 follow-on, lighting-lens).** Not yet scoped in v3.1 or v0.4; lives in roadmap Phase 4 ("Add unified lighting outputs"). Closes the immersive-show loop and stakes the differentiating positioning the roadmap already names ("photos + projection + light as one scene"). Color-from-pixel sampling is the cheapest credible entry point and produces strong results from day one. **Roadmap home: roadmap Phase 4.**

Then in the next ring (unranked, mix of scoped and not-yet-scoped — all material to "top-notch" positioning):

- **Two-projector edge-blend stub** (M4 follow-on, I7, K) — *v0.4 (scoped — stub only)*
- **NDI input layer** (I9) — *v0.4 (scoped)*
- **Per-projector colour calibration (RGB matrix)** (I7, K) — *v0.4 (scoped)*
- **Bezier mesh warp** (M4, N1) — *not yet scoped, recommend v0.4 or v0.5*
- **NDI / Syphon out** (M2, I7, K) — *not yet scoped, recommend v0.5 — note v0.4 covers NDI **input** but not output*
- **Cuelist with per-cue timing / follow / BPM-quantize** (I6, J) — *roadmap Phase 5*
- **Audio-reactive UI surface (8-band FFT meter + drag-to-bind)** (I3, N5) — *not yet scoped, recommend v3.1 or v0.4*
- **BPM HUD + transport (clock + tap badge)** (N3) — *not yet scoped, recommend v3.1*
- **Inverse mask + luma / chroma key** (M8) — *not yet scoped, recommend v0.4 or v0.5*
- **Output panel with calibration verify** (I7, K) — *roadmap Phase 6*
- **Effect chain reordering + preset library** (M7 follow-on, I2) — *not yet scoped*
- **Layer solo / mute + groups + reverse-lookup** (I9, H) — *not yet scoped, recommend v3.1*

### Strategic framing

The honest framing is that **v3 is the right entry-level product** for its target user (single-projector event/event operator with photos), and the v3-fix list above lands that strong. The post-v3 upgrade list is the **growth path that prevents v3 from becoming a dead-end product** — most items already have a home in `specs/roadmap.md` (Phases 1–6) or `specs/v3-capability-scope.md` (v3.1, v0.4); the rest are flagged as recommended additions to those buckets, justified by a current capability ceiling rather than feature-parity catch-up.

The single-most-important strategic point is item 1 above. The framing here matters: the live-input *bus* exists (MIDI port subscriptions, OSC UDP listener, audio FFT modulators) but the parameter-binding *path* is genuinely incomplete — `Param::Bound` is dead-coded, effect parameters use the `Modulator` enum and have no MIDI/OSC variants, and the `InputState` source registry is unpopulated. v0.4 already commits to the OSC binding UX; the highest-leverage move is to scope **MIDI parameter binding into the same release** so the binding picker, learn workflow, and registry plumbing ship once and serve both transports. *That single addition* moves rmap from "scriptable visual tool" to "instrumental show engine" without any new render-graph work — and it is currently the only capability gap of this size that lives in nobody's bucket.
