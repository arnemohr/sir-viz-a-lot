# rmap

rmap is a single-machine projection-mapping tool for small live shows. Load a
still image or SVG, drag the warp corners onto your wall or screen, dial in a
mask polygon to hide the edges, and save the whole setup as a project file you
can reload at the next event. Up to two projectors are supported, with edge
blend and per-projector RGB colour calibration. The launcher opens a bundled
demo so you can explore the canvas immediately — no command-line flags required.

**Supported media:** SVG, PNG, JPG (stable); video decode — decoder selected
(AVFoundation), integration in flight.

<!-- TODO: screenshot of launcher -->

<!-- TODO: screenshot of canvas with photo layer and warp handles -->

<!-- TODO: screenshot of show-day strip -->

## Quick start

1. Build and run:

   ```bash
   cargo run --release
   ```

2. The launcher window opens. Click **Try a demo** to choose from four bundled
   demos: **window-glow** (a lit architectural still), **film-strip** (a
   multi-layer composition), **test-grid** (an alignment grid useful for
   verifying warp accuracy), and **fx-ripple-wash** (a mask-edge FX preset
   demo).

3. To use your own content, click **Open a recent show** (if you have one) or
   drag a JPG, PNG, or SVG onto the canvas once a project is open.

4. Adjust warp corners directly on the canvas, mask out surfaces you don't
   want lit, and save with **Save** in the toolbar.

## Top-chrome live readouts

The top bar of the control window shows the project name (with a `*` dirty
marker when there are unsaved changes), undo/redo buttons, and save controls.
To the right of those: the **BPM HUD** — a live tempo readout, the tap source
("Space", "MIDI", or "OSC"), and a quantize selector (Off / 1 bar / 2 bars …)
that makes cue firing wait for the next bar boundary instead of firing
immediately. At the far right, a **live thumbnail of the projector output** is
always visible; click it to bring the preview window forward.

## Layer solo / mute

Every layer row in the left rail carries two toggle buttons:

- **S (solo)** — isolates a single layer; only one layer can be soloed at a
  time across the whole project.
- **M (mute)** — drops the layer from the output without deleting it. The row
  thumbnail and label dim to roughly 50 % to show the muted state.

Both toggles survive undo and scene recall, making them safe for silently
subbing a layer in or out mid-cue before committing to a scene save.

## Multi-projector (v0.4)

The launcher's output picker lets you assign up to two projectors. An
identify-flash highlights each output so you can confirm which display is
which. The `output_targets` list in the project file stores one entry per
projector; the render loop applies passes 1–4 once and passes 5–6 (including
edge-blend and colour calibration) per output.

**Edge blend** — set an overlap width and the edge-blend WGSL applies a
multiply-blend gradient so the intensity across the seam sums to 1.0. The
edge-blend gradient test pattern in the show-day strip makes alignment
verification fast.

**Per-projector RGB matrix** — a 3×3 colour matrix in the OutputPanel corrects
white-point and colour-temperature per projector. The identity matrix is
bit-equivalent to the un-matrixed path; a non-identity state is marked in the
panel header.

## OSC and MIDI bindings (v0.4)

Right-click any parameter row to open the binding picker. Choose **OSC** to
bind to an incoming OSC address, or **MIDI** to use MIDI-learn: arm the
parameter, send a CC from your controller, and the binding is captured and
added to the undo stack. Scale and offset are derived from the CC range
automatically. Active OSC bindings are summarised in the Advanced panel's OSC
section before go-live.

## FX layers (v0.4)

Add an FX layer from the layer picker. Each FX layer holds a `preset_id` and
a parameter map. The shipped preset, `mask_edge_ripple_wash`, applies an
animated ripple wash along the edges of the layer's mask polygon. The demo
project `fx-ripple-wash` in the launcher shows the preset in action.

## FX Preset Library (v0.6)

_FX preset library — Wave / Particle / Fluid families with a built-in browser. Coming in v0.6._

## Treatment presets (v0.5)

Drop an image or video, open Advanced → Selected layer → **Treatment**, and
pick one of six presets. Each one is bit-exact identity at default
parameters, so the operator sees no change until they reach for a slider —
makes the preset list safe to scrub through on stage.

- **Tone map** — exposure / contrast / shoulder rolloff. Lifts shadows and
  rolls off highlights for video frames shot in mixed lighting.
- **Luminance reveal** — Rec. 601 luminance threshold modulates alpha;
  useful for keying bright subjects out of a dark background.
- **Blur mask** — SDF-gated separable gaussian. Feathers the mask edge into
  the background without losing centre detail.
- **Texture overlay** — composites an external image (loaded through the
  shared image cache) over the source with one of four blend modes
  (Normal / Multiply / Screen / Add).
- **Palette / posterize** — bit-depth quantization with optional ordered
  dither.
- **Collage (2×2)** — fixed four-slot grid composited over the source.
  Pick up to four images; empty slots fall back to source.

The combobox plus per-param sliders dispatch undoable mutations on
drag-release, so accidental scrubbing is one Cmd-Z away.

## Video grammar (v0.5)

Drag-drop an mp4 / mov / m4v; the layer auto-plays. Selected-layer →
**Video** exposes:

- **Playback speed** (0.25× — 4×, log slider).
- **Loop mode** — *Loop* (seamless, default), *Once* (stop on EOF), or
  *Ping-pong* (forward-only stub in v0.5; true reverse lands with the
  Phase 7 keyframe cache).
- **In / Out points** — number inputs in seconds. Default `Out` is the
  sentinel "end" (no trim). The worker sets the AVAssetReader's
  `timeRange` before reading, so trimming is decoder-bounded rather than
  per-frame filtered.
- **BPM-lock** — when on, effective speed scales with the show clock's
  BPM (120 = identity). Pair with the BPM tap-tempo in the top chrome.

Video layers also expose the **Source fit** section (Cover / Contain /
Stretch + focal X/Y when fit == Cover), parity with image layers.

A small loop-mode glyph (∞ / → / ⇆) and in/out markers appear on the
video layer's thumbnail in the left rail so the operator can read
playback state at a glance without opening Advanced.

## Docs

- [Show-day operator checklist](docs/show-day-checklist.md) — macOS-focused
  pre-show steps, cables, verifying display-sleep prevention, and v0.4
  two-projector / binding / FX checks.
- [Keyboard accelerators](specs/keyboard-accelerators.md) — every key binding
  with its source location.
- [Capability scope](specs/v3-capability-scope.md) — v3 feature scope and
  historical v3.1 deferred-item list.

## Tests

```bash
make test
```

GPU golden tests (optional feature, requires a working wgpu adapter):

```bash
make test-gpu
```

Full CI (fmt + clippy + tests + doctests):

```bash
make ci
```

---

## Power users

### CLI flags

```bash
cargo run --release -- --help
```

- **`*.rmap.json`** — full project (layers, warp, scenes, gamma,
  `output_targets` list, optional `output_windowed`). Each `OutputTarget`
  records the projector display's UUID; on load, rmap matches the saved UUID
  first, falls back to the saved index, then falls back to display 0 with an
  audit warning. This means a `.rmap.json` saved on machine A opens onto the
  same physical projector on machine B as long as the display UUID is recognised
  — no `--monitor` flag required. Projects from schema v6 and earlier migrate
  automatically.
- **`*.svg`** — bootstrap one layer; warp defaults are added automatically.
- **`--monitor INDEX`** — output monitor (overrides the value saved in the
  project file). Use `--list-monitors` to print indices.
- **`--windowed`** / **`--fullscreen`** — windowed draws a 1280×720 decorated
  window on the chosen monitor; fullscreen is the default and can be forced to
  override a saved `output_windowed` flag. The two flags are mutually
  exclusive.
- **`--autostart`** — with a `.rmap.json` argument, logs startup intent and
  uses the loaded project's monitor index when `--monitor` is omitted (no
  extra click gate in this build).

### Native macOS menu bar

Standard macOS keyboard shortcuts work via the native menu bar:

| Action | Shortcut |
|--------|----------|
| Save | Cmd-S |
| Save As | Cmd-Shift-S |
| Open | Cmd-O |
| Quit | Cmd-Q |
| Undo | Cmd-Z |
| Redo | Cmd-Shift-Z |

The `rmap > About rmap` menu item shows the running version. The canonical
list of all key bindings is in
[`specs/keyboard-accelerators.md`](specs/keyboard-accelerators.md).

### Cargo features

- `v3` — Spec 003 UI/UX overhaul (state machine, command/mutation pattern,
  undo, launcher, project audit). Currently behind the flag while v3 ships
  incrementally; planned to flip to default at M3.
- `gpu-tests` — headless wgpu golden-image harness. Off by default.
- `midi`, `osc` — MIDI CC and OSC live input sources. **Default-on** as of
  v0.4; binding pickers and MIDI-learn are available out of the box.
- `audio` — 8-band FFT audio input source. Off by default; when enabled, a
  meter strip appears above the cue strip. Do not promote to default.

### Build profiles

```bash
make build          # debug
make build-release  # release
make build-show     # release-show (LTO, panic=abort, stripped) — for live use
make bundle         # macOS .app via cargo-bundle
```

Logs land in `~/Library/Logs/rmap/rmap.log` (daily rolling); override with
`RUST_LOG`.
