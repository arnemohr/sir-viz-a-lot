# Phase 5 — colour-space conversion decision (P5.0.3, W0)

**Status:** decision record. The conversion API and GPU readback
strategy are fixed here; W4 tasks implement them.

## Context

Phase 5 ships "color-from-pixel fixture mapping" — the engine probes
N pixels of the rendered canvas per fixture group and maps their colour
to DMX channel values. The plan lists RGB, RGBW, and HSV as
"configurable colour-space conversion" options; RGBW is explicitly
Phase 7. This document:

1. Fixes the Phase 5 colour-space conversion scope (RGB only; HSV
   as an operator-selectable mixing mode within Phase 5).
2. Defines the conversion API so Phase 7 can add RGBW without
   breaking Phase 5 callers.
3. Resolves the GPU texture readback strategy — the highest
   technical-risk element of W4, which shapes threading, latency,
   and frame-budget behaviour more than the colour math itself.

## GPU texture readback strategy

### The problem

wgpu does not support synchronous texture reads from the GPU. To read
the rendered canvas pixels on the CPU (for colour sampling), the engine
must:

1. Copy the rendered texture to a `wgpu::Buffer` (`copy_texture_to_buffer`).
2. Map the buffer (`buffer.map_async(MapMode::Read, …)`).
3. Call `device.poll(Maintain::Wait)` — this blocks the calling thread
   until the GPU finishes.

If done on the render thread, step 3 stalls the frame loop. If done on
the lighting thread, the buffer lifetime and wgpu device ownership
require careful cross-thread management.

### Readback strategy options

#### S1 — Full canvas readback per frame (rejected)

Copy the full output texture (e.g. 1920×1080 × 4 bytes = 8.3 MB) to a
staging buffer; sample N pixel coordinates on the CPU.

- **Pros:** simple — one `copy_texture_to_buffer`.
- **Cons:** 8.3 MB GPU→CPU per frame at 60 fps = 498 MB/s PCIe
  bandwidth just for lighting. On an M-series Mac with unified memory
  this is cheaper (shared VRAM) but still a large allocation per frame.
  Unnecessary: Phase 5 needs at most ~16 fixtures × a few sample pixels
  each — far less than the full canvas.

#### S2 — Per-fixture micro-readback (rejected)

One `copy_texture_to_buffer` per fixture group sampling region.

- **Pros:** minimal data transfer per fixture.
- **Cons:** each `copy_texture_to_buffer` call has a fixed GPU-command
  overhead. 16 fixtures × 16 sample points = 256 tiny copies → GPU
  command-buffer explosion. wgpu does not batch these automatically.

#### S3 — Downsample to a "lighting tap" texture once per frame (chosen)

Render an additional low-resolution texture ("lighting tap") at the end
of the frame — a simple GPU blit of the output texture downsampled to a
fixed small size (e.g. 64×36 for a 16:9 output). Copy *that* texture to
a staging buffer once per frame; sample from it on the lighting thread.

- **Pros:**
  - Single `copy_texture_to_buffer` per frame, copying ≤ 9 KB
    (64×36×4). Negligible PCIe/unified-memory traffic.
  - Sampling any fixture region is a CPU-side lookup into the small
    buffer — no per-fixture GPU work.
  - The GPU blit (downsample pass) is a trivial one-draw-call compute
    or render pass; at 64×36 it is essentially free on modern hardware.
  - `device.poll(Maintain::Wait)` is called on the *lighting thread*,
    not the render thread. The render thread queues the readback command
    and signals the lighting thread via a channel; the lighting thread
    does the poll + map + sample + DMX-send cycle at its own ~44 Hz
    tick, one frame behind the visual output. One-frame latency is
    imperceptible for light-chasing behaviour.
  - Buffer is allocated once at startup, mapped/unmapped in the lighting
    thread's loop. No per-frame allocation.
- **Cons:**
  - One frame of latency between rendered output and DMX values. Fully
    acceptable for colour-chasing; imperceptible to humans.
  - Requires a small additional render pass (downsample blit) in the
    render graph. Must be wrapped in `panic_restore` per
    `src/render/CLAUDE.md`.
  - Lighting-tap resolution (64×36) is a phase-wide constant; fine-
    grained sub-pixel sampling is not possible (not needed for Phase 5's
    fixture groups which represent broad wash areas, not pixel-accurate
    strips). Phase 7 LED-strip pixel mapping may revisit.

**S3 is chosen.** The readback decision is documented here rather than
in `004-phase-5-dmx-transport-decision.md` because it shapes the W4
colour-sampling API, not the W2 transport.

### Lighting-tap specification

```
Lighting-tap texture: RGBA8Unorm, 64×36 (16:9 invariant).
Updated: once per render frame, GPU blit from the composited output texture.
Readback: one copy_texture_to_buffer per frame → wgpu::Buffer (9 216 B).
Map: lighting thread calls device.poll(Maintain::Wait) on its own thread.
Latency: one render frame (≤17 ms at 60 fps).
```

## Phase 5 colour-space scope

**In scope for Phase 5:**

- **RGB direct** — scale the sampled sRGB pixel `(r, g, b)` to
  `(0..=255, 0..=255, 0..=255)` DMX values and write to the fixture's
  `Red`, `Green`, `Blue` DMX channels. No colour-space conversion;
  what the camera/display sees is what the fixture emits.
- **HSV intensity gate** — convert the sampled pixel to HSV; use `V`
  (value/brightness) to gate the fixture's overall intensity. Useful
  for fixtures that should dim when the canvas is dark. Operator-
  selectable per fixture group (`OutputStrategy::HsvIntensityGate`).
  Conversion: standard `rgb_to_hsv` (CPU, pure math, no crate needed).

**Deferred to Phase 7:**

- RGBW mixing (White channel fill). Requires `ChannelRole::White` from
  the personality model; Phase 7 adds this.
- Colour-temperature-aware mixing (warm/cool white balance).
- HSI (hue/saturation/intensity) — the additive-light-correct version
  of HSV; out of scope for Phase 5.
- Per-fixture gamma / luminance correction curves.

## Conversion API

```rust
/// The colour-space strategy an operator chooses per fixture group.
/// Phase 5 ships two variants; Phase 7 extends this without breaking
/// Phase 5 project files (serde `#[serde(default)]` on new variants).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ColorStrategy {
    /// Scale sRGB pixel (r, g, b) directly to DMX byte values 0–255.
    /// Default for Phase 5.
    #[default]
    RgbDirect,
    /// Convert pixel to HSV; use V to gate fixture intensity while
    /// keeping hue/saturation from RgbDirect. Useful for wash
    /// fixtures that should dim when the canvas darkens.
    HsvIntensityGate,
    // Phase 7: RgbwFill, ColorTemp, HsiDirect
}

/// Result of sampling one pixel and applying a ColorStrategy.
/// The `channels` slice maps directly to the fixture's DMX footprint
/// via `ChannelRole` iteration in the DMX-frame writer.
pub struct SampledColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    // Phase 7: pub w: u8, pub intensity: u8
}

/// Sample the lighting-tap texture at a normalised (u, v) coordinate
/// and apply the operator's chosen colour strategy.
///
/// Caller holds a shared reference to the mapped lighting-tap buffer.
/// Pure CPU math; no allocation.
pub fn sample_and_convert(
    tap: &LightingTapBuffer,
    uv: (f32, f32),
    strategy: ColorStrategy,
) -> SampledColor;
```

`sample_and_convert` is the only function W4's DMX-frame builder calls.
Phase 7 adds new `ColorStrategy` variants and match arms inside this
function; `SampledColor` gains `w` and/or `intensity` fields with
`#[serde(default)]` where they touch persisted data.

## Consequences

- **W4.1** (lighting-tap texture + downsample pass) is the highest-risk
  task in Phase 5: it touches the render graph, must follow
  `src/render/CLAUDE.md`'s GPU lifecycle rules, and must be wrapped in
  `panic_restore`. A GPU golden image under `tests/golden/` (recorded
  with `UPDATE_GOLDEN=1`) validates the downsample output.
- **W4.2** (lighting thread readback loop) calls `device.poll` on the
  lighting thread, not the render thread. The render thread sends a
  "readback queued" signal on the crossbeam channel; the lighting thread
  waits for the map, then samples, then sends the DMX frame. The
  implementation must not call `device.poll` from the render thread.
- Phase 7's RGBW extension contract:
  1. Add `ColorStrategy::RgbwFill` without removing existing variants.
  2. Extend `SampledColor` with `w: u8` (`#[serde(default)]` where
     applicable).
  3. Add new match arms in `sample_and_convert`.
  4. No other Phase 5 code needs to change.

## Out of scope for Phase 5

- RGBW mixing and colour-temperature-aware mixing (→ Phase 7).
- HSI (additive-light-correct model) (→ Phase 7).
- Per-fixture calibration curves (→ Phase 7).
- LED-strip per-pixel readback at display resolution (→ Phase 7; the
  64×36 lighting tap is intentionally coarse for wash fixtures).
- HDR / wide-colour-gamut canvas (beyond BT.709 / sRGB) — not in
  Phase 5's render scope.
