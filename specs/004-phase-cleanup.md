# 004 Phase Cleanup — Interconnect FX, eliminate stranded features

**Builds on:** Phases 1–7 (all shipped surface area).
**Feeds:** Phase 8+ (post-v1.0) work; closes the perception gap between "tool with infrastructure" and "tool a video pro would reach for."

This is a cross-cutting cleanup phase. It is **not** a new feature phase — every finding here is something already implemented in the codebase that has weak or no impact on the live output, either because it's a self-illuminated overlay rather than a source-modifying effect, because a slider drives nothing, because a variant declares no shader, or because a UI surface shows placeholder data. The work is structural plumbing + targeted shader-body swaps + UI wiring, not new architecture invention.

---

## Goal

Convert the bulk of the current FX/preset/treatment surface from "tech demos sitting next to each other" into "interwoven effects that meaningfully modulate the underlying image and respond to live input."

The single guiding observation is that **the codebase already has two source-reading tiers** (`Effect::*` per-layer chain — `Color`, `Blur`, `Transform`; and the global `Treatment` pass — `tone_map`, `displacement_ripple`, `refraction`). The middle tier — **FX presets** — is the only one that's purely generative. That asymmetry is the root of the "tech demo" feeling, and the structural fix is to add a `FxFamily::SourceModifier` variant alongside the existing `Fragment` and `Compute` families, plus an `Effect::Treatment(id)` variant that promotes treatments to be selectable per-layer. With those two pieces in place, every existing preset becomes a follow-up shader-body swap rather than a re-architecture.

A successful cleanup phase ends with: every FX preset that exists can either (a) modify the source image directly via a `SourceModifier` sibling, (b) feed its signal into another effect's parameter via the modulator system, or (c) is honestly labelled as a generative overlay. No silent no-ops. No sliders that do nothing. No placeholder thumbnails in the operator UI.

---

## Architectural insight — the fault line

| Tier | Examples | Reads source? | Where applied |
|---|---|---|---|
| **`Effect::*` chain** (per-layer) | `Color`, `Blur`, `Transform`, `Tint` (no-op), `External` (empty) | **Yes** | Per-layer, before composition |
| **FX presets** (per-layer overlay) | `mask_edge_wave_wash`, `mask_bounded_fluid`, `particles_*`, `zone_*`, `fluid_*`, `mask_edge_ripple_wash` | **No — all generative** | Per-layer, writes its own pixels on top of the layer |
| **Treatments** (global) | `tone_map`, `luminance_reveal`, `blur_mask`, `texture_overlay`, `palette_extract`, `collage`, `displacement_ripple`, `refraction` | **Yes** | Global, after composition (8 of 9 hidden behind `v3` flag) |

**Three structural changes** unlock every per-preset improvement in this spec:

1. **`FxFamily::SourceModifier`** — new family variant in `src/render/fx_presets.rs`. A preset declaring this family has `source_view` bound in addition to the SDF, and its shader writes `dst_view` as a modified copy of source instead of premultiplied overlay.
2. **`Effect::Treatment(id, params)`** — new `Effect` enum case in `src/effects/mod.rs` that dispatches into the existing `TreatmentPipeline`. Reuses all 9 treatments per-layer with zero new shader work.
3. **`Effect::Feedback { decay, offset }`** — new `Effect` variant. Reuses the `intermediate_view` ping-pong infrastructure already used by `Blur` (per `src/render/CLAUDE.md`). Blends previous-frame layer output into current — trails on any layer for free.

Everything else in this spec is downstream of these three.

---

## Recommended sequencing

Order is by leverage (one structural change unlocks many follow-up tasks), not by complexity.

1. **W1** — Architectural unlocks (`FxFamily::SourceModifier` + `Effect::Treatment` + `Effect::Feedback`). Ship `fluid_warp` as the proof preset for W1.
2. **W4** — Implement `Effect::Tint` properly (pure paperwork — closes a silent-data-loss class).
3. **W6** — Wire OSC parameter modulators (the modulation matrix is half-true today).
4. **W7** — Real scene thumbnails + scrubber + per-output trims (operator-facing UI fixes).
5. **W2** — Per-effect sibling source-modifying presets (parallel-safe once W1 lands).
6. **W3 / W5** — Inert sliders + schema-only variants (cleanup; can interleave).
7. **W8** — Treatments promotion / `v3` flag flip / multi-output deferred work.

---

## Workstream summary

| WS | Theme | Tasks (count) | Touches |
|----|-------|---------------|---------|
| W1 | Architectural unlocks | 4 | `src/render/fx_presets.rs`, `src/effects/mod.rs`, `src/render/treatments.rs` |
| W2 | Source-modifying FX preset siblings | 12 | `src/render/shaders/fx_*.wgsl`, `src/render/fx_presets.rs` |
| W3 | Inert sliders / dead parameters | 4 | `src/render/fx_presets.rs`, `src/render/shaders/`, `src/app.rs` |
| W4 | No-op `Effect` variants | 2 | `src/effects/mod.rs`, new `src/effects/tint.rs`, `src/effects/registry.rs` |
| W5 | Schema variants without renderers | 3 | `src/project/schema.rs`, `src/video_layer/worker.rs`, `src/project/mod.rs` |
| W6 | Inputs & automation gaps | 4 | `src/modulators/osc.rs`, `src/app.rs`, `Cargo.toml`, `src/controls/osc.rs` |
| W7 | UI surface gaps | 6 | `src/windows/cue_strip.rs`, `src/windows/layer_strip.rs`, `src/windows/output_panel.rs`, `src/windows/output.rs`, `src/app.rs`, `src/project/schema.rs` |
| W8 | Treatments per-layer + `v3` flag | 3 | `src/render/treatments.rs`, `Cargo.toml`, `src/effects/mod.rs` |

Total: **38 findings**, each sized to fit a single PR.

---

# Findings

Each finding includes: current state with file:line, fix sketch, acceptance criteria, dependencies, test plan, and effort estimate (S = <1 day, M = 1–3 days, L = 1+ week). Numbering is provisional — a follow-up `004-phase-cleanup-tasks.md` will assign canonical IDs.

---

## W1 — Architectural unlocks

### W1.1 — Add `FxFamily::SourceModifier` variant

- **Current state:** `FxFamily` (in `src/render/fx_presets.rs`, near `fx_registry()`) has `Fragment` and `Compute` variants. Both families produce premultiplied-alpha output that the compositor adds on top of the layer. There is no path for an FX preset to *read* the layer's source texture; the source view is available in `RenderCtx { source_view, dst_view, intermediate_view }` (per `src/render/CLAUDE.md`) but no FX-preset dispatch arm uses it.
- **Fix sketch:** Add a `SourceModifier` variant. Its bind-group layout includes binding 4 = `t_source` (filterable, R+G+B+A from the layer's prior chain output). Dispatch reads source, writes modified output to `dst_view` (LoadOp::Clear). Pipeline construction follows the existing `Fragment` pattern in `fx_presets.rs`.
- **Acceptance criteria:**
  - `FxFamily::SourceModifier` variant exists and is matched exhaustively in the dispatch arm.
  - A new preset (W1.2 / `fluid_warp`) registers under this family and renders.
  - The compiler enforces exhaustive matching (no `dyn` dispatch).
- **Dependencies:** None.
- **Test plan:** Unit test that the registry contains the new family. GPU golden under `--features gpu-tests` for W1.2 once it lands.
- **Effort:** S.

### W1.2 — Ship `fluid_warp` as the proof preset for `SourceModifier`

- **Current state:** `mask_bounded_fluid` runs a real 2D velocity-field sim on the GPU but the draw fragment shader (`src/render/shaders/fx_fluid_identity.wgsl:47-51`, reused by bounded) outputs `(vx*0.5+0.5, vy*0.5+0.5, 0, 0.5)` — the olive-yellow placeholder. The shader's own comment at line 11 calls it "proof-of-contract." `src/render/shaders/fx_fluid_bounded.wgsl:27-30` admits "particle visualisation is deferred to a follow-up."
- **Fix sketch:** New shader `fx_fluid_warp.wgsl` reusing the existing advect compute pass; the draw fragment shader samples `t_source` at `uv - velocity * amplitude`. Add `amplitude` to `FxParamsUniform` aliasing (probably re-using `wavelength` since this preset has unused fields). New registry entry `fluid_warp` under `FxFamily::SourceModifier`.
- **Acceptance criteria:**
  - Operator can apply `fluid_warp` to a layer with any source image.
  - Source pixels visibly flow according to the velocity field.
  - Inside a masked layer, the warp respects the mask edge (no-slip boundary already in place from `fx_fluid_bounded.wgsl`).
  - `amplitude` slider varies the visible warp strength from 0 (passthrough) to a strong distortion.
- **Dependencies:** W1.1.
- **Test plan:** GPU golden image with a vortex sim at clock=5 against a checkerboard source. Manual smoke test against a real photo layer.
- **Effort:** S.

### W1.3 — Add `Effect::Treatment(id, params)` variant

- **Current state:** `src/render/treatments.rs` ships 9 fully-built treatment shaders (`identity`, `tone_map`, `luminance_reveal`, `blur_mask`, `texture_overlay`, `palette_extract`, `collage`, `displacement_ripple`, `refraction`). They run only as a single global pass after composition. The agent inventory confirms: "Per-layer post-processing is deferred (W2.x roadmap)." `displacement_ripple` and `refraction` are precisely the source-modifying effects the FX tier is missing — they exist but are locked to the global tier and hidden behind the `v3` cargo flag.
- **Fix sketch:** New `Effect::Treatment { id: String, params: HashMap<String, f32> }` variant. The render dispatch in `src/effects/mod.rs` looks up the treatment in the existing `TreatmentPipeline` registry and dispatches it into `dst_view`, reusing the per-layer `intermediate_view` for multi-pass treatments (`blur_mask` already does H+V). Preserves the enum (no dyn) per `src/render/CLAUDE.md`.
- **Acceptance criteria:**
  - Operator can stack a treatment ID inside a layer's effect chain.
  - `displacement_ripple` and `refraction` immediately become per-layer effects.
  - The global treatment pass continues to work; per-layer and global are independent.
  - Unknown treatment IDs warn-and-skip (matching `Effect::External` policy).
- **Dependencies:** None (parallel to W1.1).
- **Test plan:** Proptest for serde round-trip of `Effect::Treatment` (follow `src/project/command.rs` pattern). GPU golden for `displacement_ripple` applied to a single layer while another layer renders untreated.
- **Effort:** M.

### W1.4 — Add `Effect::Feedback { decay, offset }` variant

- **Current state:** No feedback/echo/trails effect exists. The renderer has `intermediate_view` per-layer for ping-pong (used by `Blur`), so the infrastructure is in place but no leaf uses it for temporal feedback.
- **Fix sketch:** New `Effect::Feedback` variant. The pipeline keeps a per-layer "history" texture (re-using `intermediate_view` or adding a `history_view` to `RenderCtx` if needed). Fragment shader: `mix(sample(t_source, uv), sample(t_history, uv + offset), decay)`. Write result to `dst_view` AND copy back to history. Both `decay` and `offset` are `Modulator`-driven so they can pulse with audio/MIDI.
- **Acceptance criteria:**
  - Applying `Feedback` to a layer produces visible trails.
  - `decay` slider varies trail length (0 = no trail, 1 = infinite hold).
  - `offset` slider allows directional motion-trail (e.g. wind-blown trails).
  - `Modulator`-driven `decay` responds to audio bands when audio feature is enabled.
- **Dependencies:** May need a small extension to `RenderCtx` to carry a per-layer history texture (decide during implementation; reusing `intermediate_view` is preferable if feasible).
- **Test plan:** GPU golden: still source + Feedback effect over 10 frames; verify the diff between frame 0 and frame 9 matches the expected exponential blend. Manual smoke on a moving layer.
- **Effort:** M.

---

## W2 — Source-modifying FX preset siblings

Each task below is a new preset registered under `FxFamily::SourceModifier` (depends on W1.1). They reuse the existing compute / fragment infrastructure of their generative siblings — most are shader-body swaps. Sized S each unless noted. All parallel-safe after W1.1 lands.

### W2.1 — `ripple_lens` (sibling of `mask_edge_ripple_wash`)

- **Current state:** `src/render/shaders/fx_ripple_wash.wgsl` emits concentric coloured rings from the mask edge. Does not read source.
- **Fix sketch:** New shader `fx_ripple_lens.wgsl`. Same SDF + sine-phase math, but `sample(t_source, uv + normal * sin(phase) * amp)` instead of writing colour. Optionally sample R/G/B at slightly different amplitudes for chromatic aberration.
- **Acceptance criteria:** Rings become refraction lenses bulging the underlying image. `amplitude` and `chromatic_offset` sliders work.
- **Effort:** S.

### W2.2 — `edge_lens` (sibling of `mask_edge_wave_wash`)

- **Current state:** `src/render/shaders/fx_edge_wave_wash.wgsl:12` — "no source texture (binding 4) is read." 4 self-illuminated crests orbit the boundary.
- **Fix sketch:** Same phase function, but the 4 crests displace `t_source` UV in the normal direction. Image distorts at each crest, recovers between them.
- **Acceptance criteria:** 4 traveling refraction bumps around the mask edge. `wave_speed` and `amplitude` both visible.
- **Effort:** S.

### W2.3 — `fluid_warp_full` (sibling of `fluid_identity`)

- **Current state:** `fx_fluid_identity.wgsl:11` — "proof-of-contract."
- **Fix sketch:** Same as W1.2 but without the SDF gating; full-layer fluid warp.
- **Acceptance criteria:** Source image flows according to the unbounded velocity field across the whole layer.
- **Dependencies:** W1.2 (proves the pattern).
- **Effort:** S.

### W2.4 — `spotlights` (sibling of `particles_identity`)

- **Current state:** `src/render/shaders/fx_particles_fragment.wgsl:14-19` returns solid white. Particles are visualised as 2×2 px white dots.
- **Fix sketch:** Each particle becomes a soft Gaussian *luminance brightener* that lifts source-pixel brightness in its radius. Additive blend.
- **Acceptance criteria:** Source image visible everywhere; particles lift brightness around themselves. `particle_size`, `brightness_gain` sliders work.
- **Effort:** S.

### W2.5 — `drift_pinholes` / `drift_brushstrokes` (sibling of `mask_constrained_drift`)

- **Current state:** Particles wander inside mask as white dots. See W2.4 inventory.
- **Fix sketch:** Two variants. (a) `drift_pinholes` — only source pixels under particles visible, rest goes dark. Layer becomes a moving stencil. (b) `drift_brushstrokes` — each particle is a small motion-blurred smear of the source colour, leaving a short trail.
- **Acceptance criteria:** Underlying photo bleeds through particles. Choose one variant for v1; defer the other.
- **Effort:** S.

### W2.6 — `edge_sparks` (sibling of `mask_edge_emission`)

- **Current state:** Particles fly outward from mask edge as white dots.
- **Fix sketch:** Each particle additively lifts the underlying source's luminance in a soft radius (no opaque dot). Sparks "light up" the image instead of overlaying.
- **Acceptance criteria:** Sparks brighten the photo where they pass; underlying detail still visible.
- **Effort:** S.

### W2.7 — `field_advect_source` (sibling of `mask_field_flow`)

- **Current state:** Particles follow SDF gradient (`sample_sdf_gradient`).
- **Fix sketch:** Drop the particle visualisation; use the gradient field to advect `t_source` directly. `sample(t_source, uv - gradient(uv) * flow_speed * clock)`. The photo flows along the mask normals over time.
- **Acceptance criteria:** Photo visibly drifts along the mask gradient. `flow_speed` works smoothly.
- **Effort:** S.

### W2.8 — `collision_ripples` (sibling of `mask_collision_reflection`)

- **Current state:** Particles bounce inside mask off SDF boundary.
- **Fix sketch:** Each collision event injects a small ripple into a per-layer displacement field (CPU-side ring buffer of recent collisions, GPU shader sums their contributions). Source displaced accordingly — water-drops on the photo.
- **Acceptance criteria:** Each particle bounce produces a visible ripple in the source image at the bounce location.
- **Effort:** M (needs collision event readback or a parallel CPU sim).

### W2.9 — `zone_brighten` (sibling of `fx_zone_light_spill`)

- **Current state:** `fx_zone_light_spill.wgsl` adds a warm colour overlay in the zone region.
- **Fix sketch:** Replace additive colour with luminance multiplication: pixels in the spill region get their brightness boosted (1.0 + spill_radius_falloff * gain). Same falloff curve, different blend math.
- **Acceptance criteria:** Source pixels in the zone visibly brighten without colour shift.
- **Effort:** S.

### W2.10 — `zone_lens` (sibling of `fx_zone_edge_ripple`)

- **Current state:** Cool-blue ripples at zone edges. Does not read source.
- **Fix sketch:** Same ripple phase, but displaces source UV in a band around the zone edge.
- **Acceptance criteria:** Source warps in a thin band at the zone perimeter; rest untouched.
- **Effort:** S.

### W2.11 — `portal_warp` (sibling of `fx_zone_portal_drift`)

- **Current state:** `fx_zone_portal_drift.wgsl:6-13` — fragment-only implementation; compute-particle architecture for zones is "deferred to Phase 4." Particles drift through portal zones.
- **Fix sketch:** Particles displace source pixels they pass over (small lensing region per particle).
- **Acceptance criteria:** Ghost-through-the-room effect on a photo of a room.
- **Effort:** M (closes the Phase 4 deferral simultaneously).

### W2.12 — Deprecate self-illuminated overlays where SourceModifier equivalents ship

- **Current state:** Once W2.1–W2.11 ship, every generative overlay has a SourceModifier sibling.
- **Fix sketch:** Audit the FX picker UI: surface the SourceModifier variants prominently; demote the pure-overlay variants to an "Overlays (generative)" subgroup. Don't delete — they're useful for layered compositing (e.g. overlay on a black layer to compose an additive "lights only" pass).
- **Acceptance criteria:** Operator's first FX-picker view shows source-modifying presets; generative overlays are reachable but de-emphasised.
- **Effort:** S (UI grouping; small label additions to `fx_param_descriptors`).

---

## W3 — Inert sliders / dead parameters

### W3.1 — `mask_bounded_fluid.particle_count` is inert

- **Current state:** Descriptor at `src/render/fx_presets.rs:486-488` admits "the current implementation does not maintain a particle SSBO; particle visualisation is deferred." Slider works in the UI but has no visible effect.
- **Fix sketch:** Two options. (a) Remove `particle_count` from the descriptor until the particle SSBO pass lands. (b) Implement the SSBO + particle draw pass alongside the velocity field (M-sized work that complements W1.2 / `fluid_warp`).
- **Acceptance criteria:** Either slider is gone, or moving it produces visible particles.
- **Effort:** S (remove) or M (implement).

### W3.2 — `mask_edge_wave_wash` unused uniform fields

- **Current state:** `fx_edge_wave_wash.wgsl:42-50` — `wavelength`, `base_g`, `base_b`, `_pad0`, `_pad1` all hardcoded to 0. Shared `FxParamsUniform` carries them but this preset doesn't use them.
- **Fix sketch:** Either (a) expose `wavelength` as `N_WAVES` slider (currently `const N_WAVES = 4.0`), or (b) document the field aliasing in the descriptor so it's clear they're intentionally inert. (a) is more interesting; (b) is correct hygiene if (a) isn't desired.
- **Acceptance criteria:** Either an `N_WAVES` slider works (1–8 range), or the descriptor includes a doc comment naming the unused fields.
- **Effort:** S.

### W3.3 — `fx_zone_light_spill.speed` parameter unused

- **Current state:** `src/render/shaders/fx_zone_light_spill.wgsl:18` — `speed → unused`. The descriptor exposes `speed`; the shader stores it in `_unused: f32` and never reads it.
- **Fix sketch:** Either animate the spill radius or colour intensity with `clock_secs * speed` (small pulse, breathing effect), or drop the descriptor entry.
- **Acceptance criteria:** Either slider produces a visible animation, or it's removed.
- **Effort:** S.

### W3.4 — Frozen-frame cue hold-time bindings are inert

- **Current state:** `src/project/schema.rs` declares `Cue.in_time_binding`, `hold_binding`, `out_time_binding`. `process_pending_cue` in `src/app.rs` (around line 972 — same vicinity as the bar-phase TODO) does not look them up. Operator can set them in JSON; they're ignored at cue-fire time.
- **Fix sketch:** At cue-fire time, call `lookup_modulator(binding).unwrap_or(default)` for each timing field and use the resolved value.
- **Acceptance criteria:** Setting `hold_binding` to an OSC address and sending an OSC message changes the cue's hold duration.
- **Test plan:** Integration test: bind hold to a constant modulator (`Modulator::Constant(2.0)`), recall the cue, verify the hold duration is 2 seconds.
- **Effort:** S.

---

## W4 — No-op `Effect` variants

### W4.1 — Implement `Effect::Tint`

- **Current state:** `src/effects/mod.rs:21,82-84,112-115`. Variant deserialises from JSON, but `render()` logs `warn!` and returns `false`. Projects with Tint effects silently skip them.
- **Fix sketch:** New file `src/effects/tint.rs` and new WGSL `src/render/shaders/tint.wgsl`. Three-mode tint: multiply (proper tint), additive (wash), screen. Reads source, mixes with `rgba` colour by `amount`. ~30 lines WGSL + ~80 lines of pipeline boilerplate matching the `Effect::Color` pattern.
- **Acceptance criteria:**
  - Adding `Effect::Tint` to a layer produces a visible tint.
  - All three modes work distinctly.
  - `amount` is `Modulator`-driven.
  - No more `warn!` log when Tint effects exist.
- **Dependencies:** None.
- **Test plan:** Unit test that the pipeline renders; GPU golden for each of the three modes against a known source. Verify the warn-log is removed.
- **Effort:** S.

### W4.2 — Hide `Effect::External` from UI until M7 plugins land

- **Current state:** `src/effects/registry.rs:58-62` — "v1 ships no built-in External passes." Registry is always empty by default. Variant deserialises but the picker UI may show an entry that does nothing.
- **Fix sketch:** Either (a) hide the External variant from the picker UI in default builds, or (b) ship 1–2 built-in passes through this API as a proof: a LUT lookup, a chromatic-aberration / RGB-shift pass. (b) validates the protocol AND gives operators VJ staples.
- **Acceptance criteria:** Either External is hidden, or at least one built-in pass is registered and selectable.
- **Effort:** S (hide) or M (ship sample passes).

---

## W5 — Schema variants without renderers

### W5.1 — `MaskNode::Union` and `MaskNode::Subtract` are scaffolding

- **Current state:** `src/project/schema.rs:605-636` — "Union and Subtract are schema scaffolding only." Project JSON can serialise these but neither the CPU evaluator (in `src/project/mod.rs` interpolate path) nor the SDF baker handles them. Saved projects with these variants render as black or invisible.
- **Fix sketch:** Add CPU-side SDF combine in the bake step: `union(a, b) = min(a, b)`; `subtract(a, b) = max(-a, b)`. SDF baker calls the new combine where the project tree has Union/Subtract nodes. UI to author them is out of scope here (deferred to a separate mask-editor task).
- **Acceptance criteria:**
  - Project JSON with `MaskNode::Union { children: [a, b] }` renders as the union of the two SDFs.
  - Same for `Subtract`.
  - Hand-edited JSON fixtures verify both cases.
- **Dependencies:** None.
- **Test plan:** Add fixture under `tests/` with Union and Subtract mask trees; GPU golden for the resulting SDF.
- **Effort:** M.

### W5.2 — `LoopMode::PingPong` falls back to forward

- **Current state:** `src/project/schema.rs:177-195` defines `PingPong` but `src/video_layer/worker.rs:595` admits "reverse playback not yet implemented; falling back to forward at |speed|. Phase 7 will add the I-frame cache." Operator picks PingPong, gets Loop silently.
- **Fix sketch:** Phase 7 I-frame cache is L-sized work. For the cleanup phase, the honest short-term fix is to warn in the UI when PingPong is selected (or grey out the option) until the I-frame cache lands. The warning ships ~3 lines of egui.
- **Acceptance criteria:** Selecting PingPong shows a "(forward fallback until Phase 7)" hint in the loop-mode picker. Or: PingPong is hidden until reverse decode lands.
- **Dependencies:** Phase 7 I-frame cache is the real fix; this is a stopgap.
- **Effort:** S.

### W5.3 — Audit other schema variants with no renderer

- **Current state:** Beyond W5.1 and W5.2, there may be other enum variants in `src/project/schema.rs` that serialise but have no render path. Worth a sweep.
- **Fix sketch:** `rg --type rust 'scaffolding|not yet|deferred|placeholder' src/project/schema.rs` and cross-reference each match against actual dispatch arms.
- **Acceptance criteria:** Audit report (committed as a comment in the spec or a follow-up task list).
- **Effort:** S.

---

## W6 — Inputs & automation gaps

### W6.1 — Wire OSC parameter modulators

- **Current state:** `src/modulators/osc.rs:25-45,36` — the `PROVIDER` registry exists, the install function is marked `#[allow(dead_code)]` ("W2.1 follow-up"). `controls::OscSource` receives OSC datagrams and dispatches *commands* (TapTempo, SceneRecall, Freeze, Blackout) but **never populates the modulator provider**. Result: `Modulator::OscBound { addr, scale, offset }` always reads `0.0`.
- **Fix sketch:** Wire `OscSource::poll_into(&mut ProviderRegistry)` in the per-frame source loop. The install path at `src/modulators/osc.rs:36-38` is the missing wiring.
- **Acceptance criteria:**
  - Sending an OSC message to a `Modulator::OscBound` address updates the bound parameter visibly.
  - A test fixture or integration test verifies the path end-to-end.
- **Dependencies:** OSC feature flag is on by default.
- **Test plan:** Integration test sending a UDP packet to the local OSC port and asserting that a bound modulator reads the expected value within one frame.
- **Effort:** S (~30 lines).

### W6.2 — Bar-phase re-anchor on tap-tempo

- **Current state:** `src/app.rs:972-989` — `// TODO: re-anchor bar phase on tap-tempo?`. Tap updates BPM but not `started`, so quantised cues fire off-beat after a tap.
- **Fix sketch:** On tap, snap `started` so the next bar boundary aligns with the latest tap. Single-line behavioural change, but needs a UX decision: does the tap represent beat 1 of the bar, or the nearest beat? Pick one and document.
- **Acceptance criteria:**
  - Tapping tempo while a quantised cue is queued causes the cue to fire on the next bar boundary aligned with the tap.
  - Unit test for `Clock::tap` updating both BPM and `started`.
- **Dependencies:** `v3` feature flag.
- **Effort:** S.

### W6.3 — Document audio-feature opt-in

- **Current state:** `Cargo.toml` — `audio` is opt-in (CPAL build cost, per inline comment near line 179). Operators building from `main` without `--features audio` get silent zero-band reads from audio-bound modulators.
- **Fix sketch:** Two parts. (a) Add a runtime check: if a project has audio-bound modulators but the `audio` feature is compiled out, show a one-time UI hint at load time. (b) Document the opt-in in `README.md` and the show-day checklist.
- **Acceptance criteria:**
  - Loading a project with audio-bound modulators without the `audio` feature produces a UI hint.
  - README has a "Building with audio support" section.
- **Effort:** S.

### W6.4 — Audit modulator coverage across effect parameters

- **Current state:** Some effect parameters are `Modulator` (live-updatable), others are plain `f32`. Inconsistent surface area.
- **Fix sketch:** `rg 'pub [a-z_]+: f32' src/effects/ src/render/fx_presets.rs` and decide for each: should this be `Modulator`? At minimum, every animated parameter (speed, amplitude, frequency, brightness) should be `Modulator`-driven.
- **Acceptance criteria:** A doc comment in the spec listing which parameters changed from `f32` → `Modulator`, and which were kept as plain `f32` (with reasons).
- **Effort:** M (scope work).

---

## W7 — UI surface gaps

### W7.1 — Real scene thumbnails in the cue strip

- **Current state:** `src/windows/cue_strip.rs:51-58` — `placeholder_thumbnail_for_name()` returns a 192×108 muted gradient. Real GPU readback of `warp_rt` per scene is deferred (T4.1 follow-up). The cue strip looks like a wireframe.
- **Fix sketch:** Reuse the existing `register_scene_preview` path (per `src/render/CLAUDE.md` — `warp_rt_view` is already registered with the egui renderer using `FilterMode::Linear`, the "single source of truth" pattern). At cue recall time, snapshot the registered texture ID, cache the downsampled view per scene. No new render targets needed.
- **Acceptance criteria:**
  - The cue strip shows actual scene contents, not gradients.
  - Thumbnails update when the scene's content changes.
  - Resize-safe: re-registers after `resize_m5_gpu`.
- **Dependencies:** None.
- **Test plan:** Manual smoke: create three scenes with distinct content, verify the cue strip shows three distinct thumbnails.
- **Effort:** M.

### W7.2 — Layer-strip timeline scrubber

- **Current state:** `src/windows/layer_strip.rs:233-235` — "hover thumbnails + click-to-seek … deferred (P1.4.5's deferred half)."
- **Fix sketch:** Add an egui drag-detection on the strip rect; emit a `SeekVideoLayer(layer_id, t_secs)` command. The video worker already supports seek (`src/video_layer/worker.rs`). Hover thumbnails can be a follow-up.
- **Acceptance criteria:**
  - Click-to-seek on the layer strip moves the playhead.
  - Drag-to-scrub works smoothly without dropping frames.
- **Effort:** M.

### W7.3 — Per-output gamma / brightness / contrast trims

- **Current state:** `src/windows/output_panel.rs:21,163,194` — TODO markers for P0.8.1 stubs. UI sliders exist; the render chain does not apply per-output corrections.
- **Fix sketch:** Extend `GammaPipeline` (`src/render/gamma.rs`) to accept a per-output uniform block. The master gamma pass already supports a 64-byte uniform (tone + 3 matrix rows); per-output is one more bind point. Wire the UI sliders to update the uniform.
- **Acceptance criteria:**
  - Moving the per-output gamma slider visibly changes the output.
  - Two outputs can have independent settings.
- **Dependencies:** None.
- **Test plan:** GPU golden with two outputs, distinct gamma values, verify the diff.
- **Effort:** M.

### W7.4 — Preview-as-projector output window

- **Current state:** `src/windows/output.rs:14-25` — T4.16a stub. Creates window + surface but the blit path "deferred as a follow-up."
- **Fix sketch:** Same one-source-of-truth pattern as W7.1: hook the preview window into `warp_rt_view`. egui can blit at any size with the registered texture ID.
- **Acceptance criteria:** Opening the preview window shows live projector output at a configurable size.
- **Dependencies:** Existing `warp_rt_view` registration (already wired for the cue strip / scene panel).
- **Effort:** S–M.

### W7.5 — `AppState::Launcher → Failed` arm + `GoLive` keybind

- **Current state (a):** `src/app.rs:133-136` — `Launcher → Failed` transition arm partial. Critical audit findings exit the app instead of routing to `Failed`. Operator gets a hard exit instead of a meaningful error.
- **Current state (b):** `src/app.rs:6186-6348` — GoLive state + UI button + window-fullscreen all wired. No dedicated hotkey documented.
- **Fix sketch (a):** Return `AppState::Failed(FailureKind::ProjectAudit)` from the launcher's load-project handler instead of `process::exit`.
- **Fix sketch (b):** Add a single hotkey (suggest `Shift+Enter` — `F` is taken if it conflicts with anything else in the keymap; verify against `specs/keyboard-accelerators.md`). Wire in `src/app.rs` keymap.
- **Acceptance criteria:**
  - (a) Loading a project with critical audit findings routes to the Failed screen with the findings visible.
  - (b) Pressing the GoLive hotkey toggles GoLive state.
- **Effort:** S each.

### W7.6 — Multi-output beyond two projectors

- **Current state:** `src/project/schema.rs:775-802` — `EdgeBlendConfig` is hardcoded for two projectors. 3+ outputs and per-edge settings require JSON hand-editing (Phase 7 deferred per `specs/roadmap.md`).
- **Fix sketch:** This is intentionally deferred per the roadmap ("Roadmap defers true multi-output until single-surface UX is mature"). The cleanup-phase action is to **document this explicitly** in the spec and the launcher UI so operators don't expect it. Real fix is Phase 7+.
- **Acceptance criteria:**
  - Launcher UI shows a "(2 projectors max in v1; 3+ in a future phase)" hint when selecting outputs.
  - Roadmap explicitly tracks this as a v1 limitation.
- **Effort:** S.

---

## W8 — Treatments per-layer + `v3` flag

### W8.1 — Flip `v3` flag to default at M3

- **Current state:** `Cargo.toml` — the `v3` feature gates Spec 003's UI/UX overhaul (state machine, command/mutation pattern, undo, launcher, project audit). Eight treatment presets (`tone_map`, `luminance_reveal`, `blur_mask`, `texture_overlay`, `palette_extract`, `collage`, `displacement_ripple`, `refraction`) are visible in the picker only under `v3`. Code is complete; users in default builds can't reach them.
- **Fix sketch:** When the milestone gate is met (per CLAUDE.md: "planned to flip to default at M3"), flip `v3` to a default cargo feature. Audit all `#[cfg(feature = "v3")]` blocks first for any unfinished work.
- **Acceptance criteria:**
  - Default `cargo build` includes v3 features.
  - All v3-gated tests pass without `--features v3`.
- **Dependencies:** M3 milestone gate.
- **Effort:** S (the flip itself) — but pre-flip audit may surface follow-ups.

### W8.2 — Promote treatments to per-layer

- **Current state:** Covered by W1.3 — adding `Effect::Treatment(id, params)`.
- **Acceptance criteria:** Once W1.3 lands, this workstream is effectively complete; the picker shows treatments in the per-layer effect menu.
- **Effort:** Subsumed by W1.3.

### W8.3 — Treatment-specific reimagining (optional follow-up)

- **Current state:** Some treatments are weak in their current form (e.g. `collage` is a 4-slot grid).
- **Fix sketch (optional, post-W1.3):** Per the per-effect reimagining table:
  - `palette_extract` — make zone-aware (different posterise inside vs. outside a mask).
  - `collage` — add kaleidoscope mode (mirror tiles) and mosaic mode (per-tile region sampling).
  - `blur_mask` — distance-from-mask-driven radius (genuinely different from `Effect::Blur`).
- **Effort:** M each, optional.

---

## Acceptance criteria for the phase

1. **No silent no-ops.** Every `Effect` variant either renders something or is hidden from the UI. Every FX preset slider produces a visible change. Every `MaskNode` variant either renders correctly or is hidden.
2. **Source-modifying parity.** At least 6 of the 12 FX presets have a SourceModifier sibling. Operators can choose to modify the underlying image rather than overlay.
3. **OSC modulators work end-to-end.** Sending an OSC message to a bound parameter visibly updates the rendered output.
4. **Cue strip shows real thumbnails.** Operators can recognise their scenes at a glance.
5. **All known stranded features are either implemented, removed, or explicitly documented as deferred.** No silent footguns.
6. **Per-layer treatments.** `displacement_ripple` and `refraction` are selectable inside any layer's effect chain.

## Out of scope

- New FX presets that don't have a stranded sibling (creative additions belong in their own phase).
- The Phase 7 H.264 I-frame cache for PingPong (referenced; not implemented here).
- M7 plugin system / `Effect::External` registration of third-party passes (forward-compat hook).
- 3+ projector multi-output (deferred to a post-v1.0 phase per `specs/roadmap.md`).
- AI-generated content / scene authoring (permanent — per `specs/004-phase-4.md`).

## Critical files

- `src/render/fx_presets.rs` — preset registry, `FxFamily` enum, parameter descriptors (W1.1, W2.\*, W3.\*).
- `src/render/shaders/fx_*.wgsl` — leaf shaders; sibling SourceModifier shaders land here (W2.\*).
- `src/render/treatments.rs` — 9 treatment pipelines (W1.3, W8.\*).
- `src/effects/mod.rs` — `Effect` enum, dispatch (W1.3, W1.4, W4.\*).
- `src/effects/registry.rs` — `ExternalRegistry` (W4.2).
- `src/project/schema.rs` — `MaskNode`, `LoopMode`, `Cue`, `EdgeBlendConfig` (W3.4, W5.\*, W7.6).
- `src/modulators/osc.rs`, `src/controls/osc.rs` — OSC parameter modulators (W6.1).
- `src/app.rs` — `AppState`, `process_pending_cue`, keymap (W3.4, W6.2, W7.5).
- `src/windows/cue_strip.rs`, `src/windows/layer_strip.rs`, `src/windows/output_panel.rs`, `src/windows/output.rs` — UI placeholders (W7.\*).
- `src/video_layer/worker.rs` — PingPong fallback (W5.2).
- `Cargo.toml` — `v3`, `audio` feature flags (W6.3, W8.1).
- `src/render/CLAUDE.md`, `src/project/CLAUDE.md` — load-bearing invariants; update if the new variants change them.

## Anticipated risks

1. **`FxFamily::SourceModifier` doubles the bind-group layouts.** Some pipelines may need rework if the bind-group cache assumes a fixed layout. Decide whether to share the bind-group between `Fragment` and `SourceModifier` (with binding 4 = source always present, sometimes ignored) or to maintain two layouts. **Recommendation:** share — simpler cache, marginal extra bind cost.

2. **`Effect::Treatment(id)` schema impact.** Adding the variant requires a Mutation Reverse-storage rule (per `src/project/CLAUDE.md` v3 invariants). Follow the existing `Effect::Color` pattern for whole-enum reverse.

3. **`Effect::Feedback` history texture lifetime.** Per-layer history must persist across frames. Lifetime is tied to the layer; if the layer is removed the history must be released. Verify against `EditingState`'s layer-removal path.

4. **`v3` flag flip (W8.1)** is gated on M3 milestone, not on this phase. Don't bundle the flip into a cleanup PR if M3 hasn't shipped.

5. **OSC modulator wiring (W6.1)** must not block the OSC command path that already works. Add the registry update as an additional consumer of the OSC datagram stream, not a replacement.

## References

- `specs/roadmap.md` — product framing (photo-driven scene composition, deliberate scope constraint).
- `src/render/CLAUDE.md` — GPU lifecycle, per-frame render-graph order, `RenderCtx` semantics.
- `src/project/CLAUDE.md` — Mutation Reverse-storage, snapshot invariants, `Command` vs `Mutation` separation.
- `specs/004-phase-2-tasks.md` — Phase 2 FX preset four-file pattern (template for new SourceModifier presets in W2).
- `specs/004-phase-4.md` — Scene grammars (consumers of the per-layer effect chain).

---

*Companion task spec to follow: `004-phase-cleanup-tasks.md` will assign canonical task IDs and a workstream-by-workstream ordering. This document is the **what** and **why**; the task spec is the **how** and **when**.*
