# Phase 7 — RGBW + colour-temperature mixing decision (P7.W0, W9)

**Status:** decision record. The 4-channel conversion stage and UI landing in
W9 are contingent on this decision.

## Constraints

- **Phase 5 ships RGB.** The existing colour-from-pixel sampling stage
  (Phase 5) samples an RGB value from the scene and fans it to Art-Net DMX
  channels as R, G, B. Phase 7 adds a 4-channel (RGBW) path to that existing
  stage.
- **Scope is colour-correct output, not fixture profiling.** The roadmap and
  Phase 7 plan explicitly exclude "full personality library" and "bi-directional
  console integration." The mixing math must be simple enough that an operator
  can understand it from a label, not require an engineer.
- **"Warm-stage venues read correctly."** The plan's stated problem is warm
  white (tungsten, warm LEDs, e.g. 2700–3200K) venues where the white channel
  on RGBW fixtures skews warm, causing mixed colours to drift when the W
  channel contributes significantly.
- **macOS GPU path.** Colour conversion runs on the CPU (per-channel DMX
  value computation at the existing sampling step). A GPU shader is not
  required; the conversion result is a `[u8; 4]` DMX quad, not a rendered
  texture.

## Problem statement

An RGBW fixture has four channels: R, G, B, W (white LED). Naïvely mapping
`scene_rgb → (r, g, b, w=0)` wastes the W channel. More usefully:

- Extract a white component from the sampled RGB (e.g. `w = min(r, g, b)`,
  then subtract it from r, g, b to produce a "coloured" remainder).
- Scale the resulting RGBW to the same perceived brightness.

This approach is fine for neutral-white fixtures (6500K pure white). For
warm-stage venues (fixture W channel at ~2700K), the white channel's actual
chromaticity is not `(255,255,255)` but something like `(255,210,140)` — a
warm orange. Subtracting that warm white from the scene RGB without correction
causes coloured remainder shifts.

## Candidates evaluated

### 1. Naive `w = min(r, g, b)` extraction (rejected for warm venues)

`w = min(r, g, b); r -= w; g -= w; b -= w` is the simplest approach. Correct
for neutral-white fixtures but produces visible hue errors on warm W channels
because it assumes the W channel is (1,1,1) chromatically.

Rejected as the default: it does not solve the stated problem ("warm-stage
venues read correctly"). It should remain available as a fallback mode for
neutral-white fixtures.

### 2. Measured spectral mixing (rejected — too complex for scope)

A spectral approach uses per-fixture spectral power distributions (SPDs) to
compute a metameric RGBW mix. Requires per-fixture SPD data, a CIE colour
matching function integration, and a gamut-mapping step. This is engineering-
grade fixture profiling — explicitly out of scope per the roadmap ("full
personality library permanently parked").

### 3. CCT-aware white-point subtraction (chosen)

The operator configures a colour temperature (CCT) for the fixture's W
channel — e.g. 2700 K, 3200 K, 4000 K, 6500 K. rmap maps this CCT to a
`(r_white, g_white, b_white)` value via the standard
Planckian locus / approximation (e.g. Kang et al. 2002 or a small precomputed
table). The W-extraction step then uses the fixture's actual white
chromaticity:

```
// All values in [0,1] linear.
w_extract = min(r / r_white, g / g_white, b / b_white).clamp(0,1);
r_out = r - w_extract * r_white;
g_out = g - w_extract * g_white;
b_out = b - w_extract * b_white;
w_out = w_extract;
```

This removes the warm-white contribution from the coloured channels correctly.
A 2700K white will produce a lower `w_extract` for a blue scene (correct —
blue light can't be reproduced by warm white), but a higher `w_extract` for
an amber scene (correct — amber light is mostly warm white).

The operator sets one CCT value per fixture group (not per fixture — a group
is typically all-same-spec fixtures in a zone). The UI is a dropdown:
`[2700K | 3000K | 3200K | 4000K | 5600K | 6500K | Custom]`. Custom opens a
Kelvin slider (2000–8000K). The CCT-to-RGB table is a compiled-in static
array; no runtime computation beyond a table lookup.

**Pros:**
- Correct for the stated problem (warm-stage venues, single-CCT W channels).
- One parameter per fixture group — operator-comprehensible.
- Deterministic, CPU-side, no GPU pass.
- The "neutral white" fallback is `CCT = 6500K` (or `w = min(r,g,b)` — both
  produce similar results for pure white fixtures).

**Cons:**
- Does not handle multi-temperature W channels (some RGBW+WW fixtures have
  two white channels at different CCTs). That is fixture-profiling territory,
  out of scope. The operator uses the dominant W channel's CCT.
- CCT is a physical property of the LED, not user-tunable on the fly.
  If the operator doesn't know their fixture's W channel CCT, the default
  6500K fallback is conservative (may underuse the W channel on warm
  fixtures, but won't produce wrong colours).

## Decision

**CCT-aware white-point subtraction with a per-fixture-group CCT dropdown.**

The neutral-white naive extraction (`w = min(r,g,b)`) remains available as
the `6500K` preset, providing backward compatibility with existing RGB-only
fixture groups that are upgraded to RGBW. The CCT parameter is stored in the
fixture group schema (extends Phase 5's colour-from-pixel configuration);
the default is `6500K`.

## Schema extension (for W9.1)

```
// Extends Phase 5 FixtureGroup or per-output-target colour config.
pub struct RgbwConfig {
    pub enabled: bool,              // false = RGB-only (existing behaviour)
    pub w_channel_cct_k: u16,       // 2000–8000; default 6500
    pub w_scale: f32,               // 0.0–2.0 master scale for W channel; default 1.0
}
```

`Mutation::SetRgbwConfig { fixture_group_id, new, old }` — symmetric
`ReverseStorage`.

## Acceptance gates

- [ ] RGBW fixture groups output 4 DMX channels (R, G, B, W).
- [ ] Neutral-white scenes (grey) produce high W value and near-zero coloured
      channels at 6500K CCT.
- [ ] Warm amber scene with 2700K W channel produces a high W value and low
      coloured remainder (verified against a reference colour chart).
- [ ] CCT dropdown available per fixture group; default 6500K.
- [ ] Setting `enabled: false` preserves existing RGB-only behaviour (W
      channel not emitted; no DMX regression for existing show files).
- [ ] Proptest: `RgbwConfig` round-trips through save/load; `w_channel_cct_k`
      preserves exact `u16` value.
- [ ] `make ci` clean.

## Out of scope

- Multi-W-channel fixtures (RGBWW, RGBCW+WW) — fixture-profiling territory.
- Per-fixture (not per-group) CCT configuration.
- Measured spectral mixing or full personality library.
- GPU-side colour conversion.
