# rmap

rmap is a projection-mapping tool for small live shows — one laptop in the
booth driving one or two projectors at a club, gallery, warehouse, or event.
Drop in a photo, a video, or a logo; warp it to fit the wall; stack effects
that move with the music; save the whole setup as a project file you can
reload at the next gig.

If you've used VJ software or a media server before, the pieces will look
familiar. If not, the guide below walks you from zero to a visually striking
scene in about fifteen minutes — no coding, no shader writing.

**Media supported today:** SVG, PNG, JPG; H.264 video (mp4 / mov / m4v).

<!-- TODO: screenshot of launcher -->

<!-- TODO: screenshot of canvas with photo layer and warp handles -->

<!-- TODO: screenshot of show-day strip -->

## Getting rmap on your machine

There is no public prebuilt app yet. You'll need either:

- **The Rust toolchain installed** — `make setup` once, then `cargo run
  --release`. Most casual users skip this path.
- **A `.app` someone built for you** — if a developer hands you `rmap.app`,
  drag it into `/Applications` and launch it like any Mac app.

macOS is the only supported platform in v1 — the live-show features
(display-sleep prevention, app-nap suppression) are macOS-specific by design.

## Quick start

1. Launch rmap. The launcher window opens.
2. Click **Try a demo** for one of four built-in demos:
   - **window-glow** — a lit architectural still.
   - **film-strip** — multi-layer composition.
   - **test-grid** — alignment grid, useful for verifying the warp.
   - **fx-ripple-wash** — animated ripple along a mask edge.
3. To use your own visuals, click **Open a recent show** or drag a JPG,
   PNG, SVG, or MP4 onto the canvas after a project is open.
4. Drag the **warp corners** on the canvas until the image lines up with
   the wall or screen.
5. Tap **Save** in the toolbar — the project becomes a `.rmap.json` file
   you can reload next time.

That's the floor. Everything else is decoration.

---

## Pick a look

Tell rmap what you want; the recipe below tells you which buttons to click.

| You want… | See |
|---|---|
| A still image that breathes / pulses with the music | [Recipe 1](#recipe-1--photo-that-breathes-with-the-beat) |
| A video that smears into trails as it plays | [Recipe 2](#recipe-2--video-with-motion-trails) |
| A logo that flashes on every beat | [Recipe 3](#recipe-3--beat-strobed-logo) |
| Glowing particles drifting through a window or portal | [Recipe 4](#recipe-4--particles-on-architecture) |
| A heavy, trippy, posterised colour-shift abstract | [Recipe 5](#recipe-5--trippy-colour-shift) |
| A full immersive scene with one click | [Scene templates](#scene-templates--the-five-minute-stage-look) |

Each recipe is a short click sequence. None require coding or shader work.

---

## Vocabulary primer

Three words that matter:

- **Layer** — one image, video, SVG, or generative element on the wall.
  A project can have many; they stack like Photoshop layers.
- **Effect** — a process applied to *one* layer (Color, Blur, Transform,
  Tint, Treatment, Feedback). Each layer has its own effect stack; they
  run in order.
- **Modulator** — what makes an effect parameter move on its own (BPM
  tap, audio band, MIDI knob, internal sine LFO, …).
- **Mask** — the polygon that defines where the layer is visible on the
  wall. Outside the polygon, the layer is invisible.

Whenever a slider supports a modulator, there is a small combobox next to
it (showing "fixed value" by default). Click that combobox to change the
source — "fixed value" / "sine" / "tri" / "noise" / "bpm" / "audio" /
"osc" / "midi".

---

## Recipes

### Recipe 1 — Photo that breathes with the beat

1. Drag a PNG or JPG onto the canvas. The image becomes a layer in the
   left rail with a default effect stack (Color → Blur → Transform — all
   inert at default values).
2. Click the layer to select it. The right panel shows its effects.
3. Find the **Transform** effect. Next to the **Scale X** slider, change
   the modulator combobox from "fixed value" to **bpm**.
4. The picker reveals three numbers: **divisor**, **amp**, **offset**.
   Set `offset = 1.0`, `amp = 0.05`, `divisor = 1` — meaning one full
   breath cycle per beat, centred at scale 1.0, breathing ±5 %.
5. Repeat for **Scale Y**.
6. Tap the tempo: press **Space** four times in time with the kick. The
   BPM HUD in the top chrome shows the tempo and the tap source ("Space").
   Or feed in MIDI Clock from your DJ controller — see [Sync to
   music](#sync-to-music).

Push `amp` higher for stronger pulses; raise `divisor` to slow it down
(2 = one cycle every 2 beats).

**Watch out for:** if the image has straight edges, large `amp` will
bleed past the mask boundary visibly. Use `Cover` fit (see Source fit
below) and a tight mask polygon to hide it.

### Recipe 2 — Video with motion trails

1. Drag a `.mp4` / `.mov` / `.m4v` onto the canvas. The video auto-plays
   in a loop.
2. Select the layer. In the right panel, click **+ Add effect** →
   **Feedback**.
3. Drag the **decay** slider up — the higher the decay, the longer the
   trails. Try **0.85** to start. `offset` shifts the previous frame
   before re-blending; small non-zero offsets give directional motion
   smear.
4. Optional: change the **decay** modulator to **audio** (needs audio
   build; see below), band 0 or 1 for bass-driven smear, amp around 0.3,
   offset 0.7.

**Watch out for:** decay ≥ 0.95 never fully fades — anything you paint
stays effectively forever. Looks great for ten seconds, then you want to
clear it. Mute the layer briefly (M button in the left rail) to reset.

### Recipe 3 — Beat-strobed logo

1. Drag an SVG or transparent PNG onto the canvas.
2. Select the layer. In the right panel under **Layer**, change the
   **Opacity** slider's modulator combobox from "fixed value" to **bpm**.
3. Set `offset = 0.5`, `amp = 0.5`, `divisor = 1` — opacity rides 0 → 1
   on every beat (`0.5 ± 0.5 · sin(2π·t/beat)`). For double-time, set
   `divisor = 0.5`; for half-time, `divisor = 2`.
4. Tap tempo via **Space** or sync to MIDI Clock.

**Watch out for:** strobing above ~3 Hz can trigger photosensitive
seizures in a small fraction of the audience. Warn the room or keep
strobe rate low. The **B (Blackout)** key kills output instantly if you
need an emergency cut.

### Recipe 4 — Particles on architecture

1. Drop a photo of the wall as a reference layer. Mute it (`M` button)
   so it doesn't render — you'll see only the particles on top of black.
2. Add an FX layer: layer picker → **+ FX layer**.
3. Select the FX layer. Open **Advanced → Selected layer → FX Preset**.
   The preset browser opens.
4. In the **Particle** family, pick one of:
   - **Constrained drift** — particles drift around inside the mask.
   - **Edge emission** — particles spawn along the mask boundary.
   - **Flow field** — particles follow a curl-noise flow field.
   - **Collision reflection** — particles bounce off the mask polygon.
5. Draw a mask polygon in **Mask mode** matching the wall feature you
   want lit (window, doorway, alcove). Tag it as **Window** or **Portal**
   from the small palette in Mask mode.
6. Unmute the reference photo once you're happy with the particle
   behaviour. Keep it muted for clean particle-only renders.

**Watch out for:** there is also a basic **Particles** preset (without
"constrained" / "edge" / "flow" / "collision" in the name) that is
hardcoded to the centre 40 % of the layer regardless of mask. Use the
four mask-aware presets above. Tagging the mask polygon as **Window** /
**Portal** is what unlocks zone-aware preset behaviour.

### Recipe 5 — Trippy colour shift

1. Drag an image or video onto the canvas.
2. Select the layer. Open **Advanced → Selected layer → Treatment**.
   (Treatment is only available on Image and Video layers — SVG / FX /
   NDI layers show a greyed-out panel.)
3. From the preset combobox, pick **Palette / posterize**. Drag the
   bit-depth slider down — lower bit depths give chunkier, more posterised
   colour. Enable ordered dither for a stippled look.
4. Back in the layer's effect stack, **+ Add effect** → **Color**.
   Change the **hue** modulator combobox to **bpm** and set
   `divisor = 16, amp = 1.0, offset = 0.0` — one full hue rotation every
   16 beats.
5. Optional: stack **+ Add effect** → **Tint**, set the colour and the
   mode (Multiply / Add / Screen). Tint with `amount` modulated by
   audio band 4 or 5 for a glow that responds to mids.

**Watch out for:** Treatment is an image-grammar concept and only applies
to Image and Video layers. On SVG / FX / NDI layers, build the look from
Color + Tint + Blur effects instead.

### Scene templates — the five-minute stage look

Click **New scene…** in the toolbar. A five-step wizard opens:

1. **Template** — pick from eight built-in templates.
2. **Media** — drop photos / videos into each slot via the native file
   picker.
3. **Zones** — match your project's tagged masks to the template's
   declared zone roles. Templates instantiate even with no zone matches,
   they just look better when zones line up.
4. **Palette & Mood** — Warm / Cool / Neutral, Calm / Energetic / Ethereal.
5. **Tempo** — turn on BPM sync to lock animations to the project clock.

Press **Confirm** (or Return) and a complete scene appears in the layer
list. **Cmd-Z** undoes the entire wizard in one step.

| Template | Best for |
|---|---|
| Window Reveal | Lit interior visible through a window-mask |
| Pixel Drift | One image dispersed into drifting pixels |
| Collage Bloom | Four images blooming at the edges |
| Glow Behind Openings | Soft glow filling a portal-tagged mask |
| Fragmented Portrait | A portrait broken up by collision particles |
| Architectural Wash | Ripple wash along edges of a wall mask |
| Light Spill from Windows | Warm interior light spilling out of windows |
| Mask-Edge Ripple Wash | Pure ripple animation on the mask edge |

Templates are portable. Once you've built a look in one project you can
export it as a `.rmap-preset.json` for FX, or save the whole scene to a
`.rmap-scene-pack.zip` for sharing.

---

## Building blocks

### Layers — what goes on the wall

| Kind | Description | When to use |
|---|---|---|
| **Image** | PNG, JPG | Photos, posters, logos with raster detail |
| **SVG** | Scalable vector graphic | Logos, line-art, anything that should scale cleanly |
| **Video** | mp4 / mov / m4v (H.264) | Looping content, pre-rendered VJ footage |
| **FX layer** | Procedural shader preset | Particles, fluid simulation, mask-edge animation |
| **NDI** | Live network stream | Output from another machine (TouchDesigner, Resolume…) |

Every layer carries its own warp corners, mask polygon, blend mode,
opacity, and effect stack.

Image and SVG layers expose **Source fit** (Cover / Contain / Stretch +
focal X / Y for Cover). Video layers add **Playback speed** (0.25× – 4×,
log slider), **Loop mode** (Loop / Once / Ping-pong), and **In / Out
points** for trimming. With **BPM-lock** on, effective video speed
scales with the project clock (120 BPM = identity).

### Effects — the per-layer stack

You stack effects on a single layer with **+ Add effect**. They run in
order; drag to reorder; the × button removes one.

| Effect | What it does | Watch out for |
|---|---|---|
| **Color** | Hue / saturation / brightness / contrast | Defaults are inert — slide to see anything |
| **Blur** | Two-pass gaussian blur | Radius 0 = inert; large radii are GPU-heavy |
| **Transform** | Translate / rotate / scale the layer | Stacks with the mask, not the warp |
| **Tint** | Wash a colour over the layer (Multiply / Add / Screen) | `amount = 0` is inert |
| **Treatment** | Apply a Treatment preset — but the per-effect preset picker is not wired yet; use **Advanced → Treatment** instead | Effect-stack Treatment is currently inert (no UI to pick a preset). Use the **Advanced → Treatment** panel on Image / Video layers |
| **Feedback** | Blend the previous frame back in for trails | `decay ≥ 0.95` never decays — looks frozen |

### Modulators — what makes effects move

Whenever a slider supports modulation, a small combobox sits next to it.
The eight sources:

| Source | What it reads | Notes |
|---|---|---|
| **fixed value** | A static number | The default; not really a modulator |
| **sine** | Internal sine LFO (period, amp, offset) | Smooth oscillation; needs no music |
| **tri** | Triangle LFO | Same as sine, with triangular shape |
| **noise** | Smoothed random LFO | Drifting, organic motion |
| **bpm** | Sine tied to the project beat clock | `divisor` = cycle length in beats |
| **audio** | One of 8 FFT bands (0 = sub-bass, 7 = highs) | Needs audio build — see below |
| **osc** | An incoming OSC address | Bind via the picker |
| **midi** | A specific MIDI CC | Right-click the parameter label → "Learn next MIDI CC", then turn the knob |

All bindings are undoable. Range, scale, and offset adjust per binding.

### Masks — where the light lands

The mask polygon tells rmap where the layer is visible. Outside the
polygon, the layer renders as transparent black.

Open **Mask mode** to draw the polygon directly on the canvas: drag
vertices, click an edge to insert a new vertex, right-click a vertex to
remove. The mask is stored per-layer and survives undo.

Tag any mask with a **zone role** from the small palette in Mask mode:
**Window**, **Portal**, **Void**, **Spill**, **Edge**, **Highlight**, or
**Light Source**. Zone-aware FX presets activate only for the matching
role, outputting transparent black for everything else (no crash, no
configuration needed). Plain effects ignore the tag — it's purely a hint
for zone-aware presets and for Art-Net DMX fixtures (see Power users).

---

## Sync to music

This is the single biggest reason to use rmap for parties and live
events. Pick whichever you have.

### BPM tap tempo (always available)

Press **Space** four times in time with the kick. The BPM HUD in the top
chrome shows the current tempo and the tap source ("Space"). A
**quantize** dropdown (Off / ½ bar / 1 bar / 2 bars / 4 bars) makes
scene transitions wait for the next bar boundary instead of firing
immediately.

Once the BPM clock has a tempo, every slider with its modulator set to
**bpm** moves in time.

### MIDI controller (always available)

Plug in any USB MIDI controller. Right-click the parameter's label (the
text next to the slider) → **Learn next MIDI CC**. The label gets a
pulsing dot to show it is armed. Turn the knob you want — rmap captures
the CC and scales it to the slider's range.

MIDI Clock from a DJ controller (Pioneer, Denon, Native Instruments…)
drives the BPM clock automatically. The BPM HUD will show "MIDI" as the
tap source.

### OSC (always available)

Phone apps (TouchOSC, Hexler, Lemur), Resolume, TouchDesigner — anything
that speaks OSC. In the modulator combobox, switch to **osc**, then
enter the address (e.g. `/1/fader1`), a scale, and an offset. Active
OSC bindings are summarised in **Advanced → Live → OSC**.

### Audio-reactive (needs audio build)

When the binary was built with `--features audio`, an 8-band FFT meter
strip appears above the cue strip. In the modulator combobox, switch to
**audio** and pick the band (0 = sub-bass, 7 = highs). The slider
follows the FFT amplitude in real time.

Default builds do not include audio capture. If you switch a slider to
**audio** in such a build, the value resolves to 0.0 and rmap shows a
one-shot toast at project load explaining the binary lacks audio
support. Ask whoever built your app to rebuild with audio support if you
need this.

### Internal LFOs (always available, music-free)

The **sine**, **tri**, and **noise** modulator sources are pure
software oscillators with a `period_s` in seconds, an `amp`, and an
`offset`. No music input required — useful for slow, automatic motion
in installations or when there is no live beat to lock to.

---

## Live workflow

### Top chrome

The top bar of the control window:

- **Project name** with a `*` for unsaved changes.
- **Undo / Redo** buttons (also Cmd-Z / Cmd-Shift-Z).
- **Save** / **Save As** controls.
- **BPM HUD** — live tempo, tap source ("Space" / "MIDI" / "OSC"),
  quantize selector.
- **Live thumbnail** of the projector output, always visible. Click it
  to bring the projector preview window forward.

### Solo and mute

Every row in the left rail carries two toggle buttons:

- **S (solo)** — isolate one layer; only one solo at a time across the
  whole project.
- **M (mute)** — drop the layer from the output without deleting it.
  The row thumbnail dims to ~50 % to show muted state.

Both survive undo and scene recall, so they're safe for silently swapping
a layer in or out mid-cue before committing to a scene save.

### Blackout

Press **B** to kill the projector output (and, if `--features lighting`
is enabled, all Art-Net fixtures in the same frame). Press **B** again
to restore.

### Save and reload

**Save** writes a `.rmap.json` file with your full setup: layers, warp,
masks, scenes, BPM clock, projector outputs, gamma.

**Save Calibration…** writes a separate `.rmap-calibration.json`
containing only the venue's warp + mask + gamma. Reusable across show
files — a same-directory calibration is offered automatically when you
open a project.

---

## Treatment presets

Drop an image or video, open **Advanced → Selected layer → Treatment**,
and pick one of nine presets. Each one is bit-exact at default parameters,
so the preset list is safe to scrub through on stage:

- **Tone map** — exposure / contrast / shoulder rolloff. Lifts shadows
  and rolls off highlights for video shot in mixed lighting.
- **Luminance reveal** — Rec. 601 luminance threshold modulates alpha;
  keys bright subjects out of a dark background.
- **Blur mask** — SDF-gated separable gaussian. Feathers the mask edge
  into the background without losing centre detail.
- **Texture overlay** — composites an external image over the source
  with Normal / Multiply / Screen / Add.
- **Palette / posterize** — bit-depth quantization with optional ordered
  dither.
- **Collage (2×2)** — four-slot grid composited over the source; empty
  slots fall back to source.
- **Displacement ripple** — refracts the source through a sinusoidal
  band around the mask edge.
- **Refraction** — Snell-like UV bend near the mask boundary; glass-lens
  look at the edge.
- **Ripple lens** *(v1.1)* — concentric refraction rings keyed to mask
  distance; the source warps into bulging-bands at the edge.

Drag-release commits a single undoable mutation — accidental scrubbing
is one **Cmd-Z** away.

**Per-layer treatments (v1.1).** Treatments also run as
`Effect::Treatment(id)` inside any layer's Effect stack — so the same
shaders that ship as a global post-composition pass can now grade or
warp a single layer hard while the rest of the scene stays untouched.
Add via **+ Add effect → Treatment** and (until the picker UI lands)
JSON-edit the `id` field to switch presets.

## FX preset library

rmap ships 14 procedural presets across three families:

- **Wave** — mask-edge ripple and displacement / refraction Treatments.
- **Particle** — GPU compute presets bounded by the mask polygon:
  constrained drift, edge emission, flow field, collision reflection.
- **Fluid** — Navier-Stokes advection bounded inside the mask.

Three-click flow: draw a mask, open **Advanced → Selected layer → FX
Preset**, pick. The browser lets you search by name, filter by family,
and star presets you reach for often. **Export preset** saves a
`.rmap-preset.json` carrying only `preset_id` and parameter values — no
media paths or warp data — that you can share between projects.

---

## Multi-projector and edge blend

The launcher's output picker lets you assign up to two projectors. An
**identify-flash** highlights each output so you can confirm which
physical display is which.

- **Edge blend** — set an overlap width; the edge-blend shader applies
  a multiply-blend gradient so the intensity across the seam sums to 1.0.
  A gradient test pattern in the show-day strip makes alignment
  verification fast.
- **Per-projector RGB matrix** — a 3×3 colour matrix in the Output panel
  corrects white-point and colour temperature per projector. Identity
  matrix is bit-equivalent to the un-matrixed path; a non-identity state
  is marked in the panel header.

Project files store the projector display UUID; on load, rmap matches
the UUID first, falls back to the saved index, then falls back to
display 0 with an audit warning. A `.rmap.json` saved on machine A opens
on the same physical projector on machine B as long as the display UUID
is recognised — no `--monitor` flag required.

---

## Glossary

The in-app **Glossary** window (Help → Glossary) defines every zone role,
modulator source, and effect with examples. Searchable. Useful in the
booth when memory fails.

---

## Docs

- [Show-day operator checklist](docs/show-day-checklist.md) — pre-show
  steps, cables, verifying display-sleep prevention, two-projector /
  binding / FX checks.
- [Keyboard accelerators](specs/keyboard-accelerators.md) — every key
  binding with source locations.
- [Capability scope](specs/v3-capability-scope.md) — v3 feature scope.

---

## Power users

### CLI flags

```bash
cargo run --release -- --help
```

- `*.rmap.json` — full project (layers, warp, scenes, gamma,
  `output_targets`, optional `output_windowed`).
- `*.svg` — bootstrap one layer; warp defaults are added automatically.
- `--monitor INDEX` — output monitor. `--list-monitors` to print indices.
- `--windowed` / `--fullscreen` — windowed draws a 1280×720 decorated
  window on the chosen monitor; fullscreen is the default.
- `--autostart` — with a `.rmap.json` argument, uses the loaded
  project's monitor index when `--monitor` is omitted.

### Native macOS menu bar

| Action | Shortcut |
|---|---|
| Save | Cmd-S |
| Save As | Cmd-Shift-S |
| Open | Cmd-O |
| Quit | Cmd-Q |
| Undo | Cmd-Z |
| Redo | Cmd-Shift-Z |

The canonical key-binding list is in
[`specs/keyboard-accelerators.md`](specs/keyboard-accelerators.md).

### Cargo features

- `v3` — UI/UX overhaul (state machine, command/mutation pattern, undo).
- `midi`, `osc` — MIDI CC and OSC live input. **Default-on**.
- `audio` — 8-band FFT audio input. **Off by default** because the
  `cpal` dependency adds meaningful build-time cost. Audio-band
  modulators on any project load regardless; without this feature they
  resolve to 0.0 and a one-shot toast tells the operator the binary
  lacks audio.
- `lighting` — Art-Net DMX output. Off by default.
- `gpu-tests` — headless wgpu golden-image harness. Off by default.

```bash
cargo build --features audio                                # default + audio
cargo build --no-default-features --features audio,v3,osc,midi  # minimal
```

### Build profiles

```bash
make build          # debug
make build-release  # release
make build-show     # release-show — LTO, panic=abort, stripped, for live use
make bundle         # macOS .app via cargo-bundle
```

Logs land in `~/Library/Logs/rmap/rmap.log` (daily rolling); override
with `RUST_LOG`.

### Lighting output (DMX)

Build with `--features lighting`. Enables Art-Net DMX output so one
scene drives both projection and physical lights.

- **Art-Net transport** sends `ArtDmx` PDUs at ~44 Hz over UDP. Default
  destination is subnet broadcast (`255.255.255.255:6454`); override in
  the Output panel.
- **Fixture groups** — RGB fixtures defined by personality
  (`Vec<ChannelRole>`), universe ID, base DMX channel, and count.
- **Colour-from-pixel sampling** — each fixture group samples a UV-space
  rectangle of the rendered canvas (64×36 downsample) and the fixture
  follows the canvas colour in real time.
- **Zone-derived intensity** — fixtures can follow a zone's
  **Light Source** or **Highlight** activity level.
- **BPM-locked chases** through colour steps locked to the project BPM.
- **Blackout** — `B` kills projector and fixtures in the same frame.
  Go-live arms all lighting output alongside the visual transition.
- **Diagnostics** — DMX activity LED (green / grey) and packet-rate
  badge in the Diagnostics section.

### Tests

```bash
make test        # cargo nextest
make test-gpu    # golden-image tests, requires wgpu adapter
make ci          # fmt + clippy + tests + doctests
```
