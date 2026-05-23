# 005 Light Trail — task breakdown

Companion task spec for [`00-prompt.md`](00-prompt.md). Each task below is sized for a single PR.

> 16 tasks across W1–W5. Read the prompt sections referenced in each task before starting.

## Purpose

Ship `Effect::LightTrail` — a glowing rainbow comet that follows the geometry of a layer's source SVG. The work splits cleanly into five workstreams: path infrastructure, schema + state, GPU pipeline + shader, control panel UI, and testing + show-day.

## Scope covered

- W1 — SVG path extraction + arc-length-parameterized polyline
- W2 — schema bump v12 → v13, `Effect::LightTrail` variant, render dispatch
- W3 — `LightTrailPipeline`, WGSL shader, head / trail / glow / palette
- W4 — look-chain control panel UI + add-effect menu
- W5 — GPU goldens, malformed-input handling, smoke, show-day

## Entry criteria

- `specs/005-light-trail/00-prompt.md` read end-to-end.
- `src/render/CLAUDE.md` and `src/project/CLAUDE.md` read.
- `make ci` clean on `main`.

## Exit criteria

- All T1.\*–T5.\* acceptance criteria green.
- `make ci` and `make test-gpu` clean.
- `make build-show` produces a `release-show` binary that includes the effect.
- Canonical GPU golden lands under `tests/golden/`.
- Manual smoke verified per T5.3.

---

## Task index

| ID | Title | Owner | Scope | Depends |
|----|-------|-------|-------|---------|
| ⏳ T1.1 | Decision: SVG path extraction strategy | RUST | S | — |
| ⏳ T1.2 | SVG path → arc-length polyline | RUST | M | T1.1 |
| ⏳ T1.3 | Polyline GPU buffer + Metal/wgpu compat check | RUST | S | T1.2 |
| ⏳ T1.4 | Polyline lookup + path-load failure tests | RUST | S | T1.2 |
| ⏳ T2.1 | Schema bump v12 → v13 + no-op migration | RUST | S | — |
| ⏳ T2.2 | `Palette` enum + `Effect::LightTrail` variant + UBO struct | RUST | M | T2.1 |
| ⏳ T2.3 | `Effect::render` dispatch arm (no-op stub) | RUST | S | T2.2 |
| ⏳ T2.4 | Proptest: `SetLayerEffects` round-trip with new variant | RUST | S | T2.2 |
| ⏳ T3.1 | Decision: SDF vs multi-pass dash render approach | RUST | S | T1.3 |
| ⏳ T3.2 | `LightTrailPipeline` skeleton + bind group layout | RUST | M | T2.3, T3.1 |
| ⏳ T3.3 | WGSL shader: head + trail core | RUST | M | T3.2 |
| ⏳ T3.4 | WGSL shader: glow halo + palette + opacity fade | RUST | M | T3.3 |
| ⏳ T3.5 | `start` / `end` clipping + `align` tangent rotation | RUST | S | T3.4 |
| ⏳ T4.1 | Look-chain match arm + per-parameter widgets | RUST | M | T2.2 |
| ⏳ T4.2 | Add-effect menu wiring | RUST | S | T4.1 |
| ⏳ T5.1 | GPU golden for canonical configuration | RUST | S | T3.5 |
| ⏳ T5.2 | Malformed-SVG no-op + `tracing` warn test | RUST | S | T1.4, T3.5 |
| ⏳ T5.3 | Manual smoke: scrub + autoplay | RUST + QA | S | T4.2 |
| ⏳ T5.4 | `release-show` build + show-day checklist update | RUST | S | T5.1, T5.3 |

---

## W1 — Path infrastructure

References prompt §3.

### T1.1 — Decision: SVG path extraction strategy

- **Files:** `specs/005-light-trail/01-path-extraction-decision.md` (new).
- **Scope:** investigate how the existing SVG layer rasterizes today via `src/image_layer.rs` (resvg) and decide where to extract parametric path data. Compare: (a) re-parse the source SVG file at effect-load time, (b) tap the existing load path to expose path nodes alongside the rasterized texture. Also decide the multi-`<path>` policy (pick first, pick longest, concatenate, expose `path_index`).
- **Output:** short markdown decision file mirroring the format of `specs/004-phase-3-zone-tag-uniform-decision.md`: context, options, chosen path, justification, follow-ups.
- **Accept:** decision file lands at `specs/005-light-trail/01-path-extraction-decision.md`; the chosen strategy explicitly names the crate (`usvg` / `kurbo` / etc.) and identifies the data structure exposed to T1.2.

### T1.2 — SVG path → arc-length polyline

- **Files:** `src/path_geom/mod.rs` (new module, or wherever T1.1 decided), wiring into the SVG layer load path.
- **Scope:** implement the parser + polyline builder per the T1.1 decision. Output: `Polyline { points: Vec<[f32; 2]>, cumulative_arclen: Vec<f32>, total_length: f32 }` plus a `sample_at_distance(d: f32) -> ([f32; 2], [f32; 2])` returning `(point, tangent)`. Sampling resolution from `sample_resolution: u32` parameter (default 512, range 64..4096).
- **Accept:** unit tests cover (a) straight line, (b) cubic Bézier curve, (c) multi-`<path>` SVG handled per T1.1 policy. Resampling at any `d` in `[0, total_length]` is constant-time. No panics on empty path list — return `None`.

### T1.3 — Polyline GPU buffer + Metal/wgpu compat check

- **Files:** new module from T1.2; `src/effects/light_trail.rs` (new, partial).
- **Scope:** decide whether the polyline lives in a `wgpu::Buffer` (storage buffer) or a 1-D `wgpu::Texture`. Verify the chosen approach works on macOS Metal via wgpu (current `wgpu` major in `Cargo.toml`). Record the choice in a single line comment at the buffer site.
- **Accept:** a manual test program (or existing headless harness) creates the buffer / texture from a fixture polyline on the dev Mac without producing `wgpu` validation errors at debug log level (`RUST_LOG=wgpu=debug`).

### T1.4 — Polyline lookup + path-load failure tests

- **Files:** new path-geom module test file.
- **Scope:** unit tests for `sample_at_distance` at boundaries (0.0, total_length, mid-segment) plus path-load failure paths: malformed SVG, no `<path>` element, zero-length path. Each failure path returns a result the effect can convert into "no-op + `tracing::warn!`".
- **Accept:** ≥ 6 unit tests; all pass under `make test`.

---

## W2 — Schema + state + persistence

References prompt §2, §4, §8.

### T2.1 — Schema bump v12 → v13 + no-op migration

- **Files:** `src/project/schema.rs:10` (bump `CURRENT_SCHEMA_VERSION`); `src/project/migrate.rs` (add v12→v13).
- **Scope:** the migration is a no-op; the bump exists so future readers can carry the new variant. Mirror the v11→v12 entry's structure.
- **Accept:** existing project fixtures load clean; saving rewrites `schema_version: 13`. `make test` covers a round-trip load-save-load on a v12-era fixture.

### T2.2 — `Palette` enum + `Effect::LightTrail` variant + UBO struct

- **Files:** `src/effects/mod.rs` (`Effect` enum at `mod.rs:41`, new variant); `src/effects/light_trail.rs` (UBO struct + `to_wire_bytes()`).
- **Scope:**
  - Add `Palette` enum with variants `Fixed(Vec<[u8; 4]>)` and `HueShift { speed: f32 }`.
  - Add `Effect::LightTrail` variant with the full field set from prompt §4 (`progress: Modulator`, `trail_length`, `head_size`, `stroke_width`, `glow_blur`, `opacity_fade`, `palette`, `gradient_spread`, `start`, `end`, `align`, `path_index`, `sample_resolution`).
  - Add `LightTrailParams` with `#[repr(C)]` + `.to_wire_bytes()` mirroring `BlurParams` in `src/effects/blur.rs`.
  - Clamp `f32` fields at deserialize per prompt §4.
- **Accept:** `cargo build` clean; serde round-trip test for the new variant + both `Palette` variants; range clamping covered by a single test per clamped field.

### T2.3 — `Effect::render` dispatch arm (no-op stub)

- **Files:** `src/effects/mod.rs:214` (`Effect::render`).
- **Scope:** wire the new variant into the dispatch match. Returns `false` (no-op) until the pipeline lands in T3.\*. Reads the layer's `LayerConfig::Svg { svg_path }` and short-circuits with a `tracing::warn!` if the layer kind is not Svg.
- **Accept:** `cargo build` clean; a debug-log smoke (`RUST_LOG=rmap=debug`) shows the warn fires when the effect is added to a non-Svg layer.

### T2.4 — Proptest: `SetLayerEffects` round-trip with new variant

- **Files:** `src/project/command.rs` test module (alongside existing `SetLayerEffects` proptests).
- **Scope:** extend the existing `Vec<EffectNode>` round-trip generators to include `Effect::LightTrail` with both `Palette` variants. Verify Mutation rule 2 (whole-vec snapshot) and reverse-storage symmetry hold.
- **Accept:** proptest sequences exercise add → undo, edit → undo, remove → undo through `SetLayerEffects`; all pass under `make test`.

---

## W3 — GPU pipeline + shader

References prompt §5, §6.

### T3.1 — Decision: SDF vs multi-pass dash render approach

- **Files:** `specs/005-light-trail/02-render-approach-decision.md` (new).
- **Scope:** short decision file. Compare the two approaches from prompt §5 against T1.3's findings (storage buffer vs texture on Metal). Cover: bandwidth cost on a 4K projector, behaviour on tight curves, interaction with `panic_restore`, compositing with the existing Look chain ping-pong order.
- **Output:** chosen approach + one-paragraph justification + concrete shader plan (number of passes, bind group layout sketch, glow strategy).
- **Accept:** decision file lands; T3.2 and T3.3 implement against the chosen plan.

### T3.2 — `LightTrailPipeline` skeleton + bind group layout

- **Files:** `src/effects/light_trail.rs` (extend from T1.3 / T2.2).
- **Scope:** mirror `BlurPipeline` in `src/effects/blur.rs:38`. Owns `wgpu::RenderPipeline` (or pipelines, per T3.1), `BindGroupLayout`, sampler, uniform buffer. Wire the polyline buffer from T1.3 into the bind group. Pipeline construction follows the existing per-effect pattern in `RenderCtx`.
- **Accept:** pipeline constructs without `wgpu` validation errors at startup. The `Effect::render` arm now binds and dispatches (still producing a no-op shader output until T3.3).

### T3.3 — WGSL shader: head + trail core

- **Files:** `src/render/shaders/light_trail.wgsl` (new).
- **Scope:** first cut of the shader. Computes head position from `progress * total_length`, samples the polyline, draws a sharp head core + sharp trail core (no glow, no palette — solid white for now). Validates under `build.rs` naga.
- **Accept:** `make build` clean; manual render shows a moving white head + white trail on a fixture SVG when `progress` is scrubbed; trail represents the *already-traveled* section per prompt §1.

### T3.4 — WGSL shader: glow halo + palette + opacity fade

- **Files:** `src/render/shaders/light_trail.wgsl`.
- **Scope:** add the glow halo (Gaussian per T3.1's plan), the palette (Fixed array + HueShift), and the head→tail `opacity_fade` falloff. Apply hue before alpha to avoid desaturating on fade (prompt §10). Layer order per prompt §6: base → soft glow → trail core → head halo → head core.
- **Accept:** manual render produces a rainbow comet on the fixture SVG, glow visibly halos the head and the leading section of the trail. No banding artifacts at default `gradient_spread=1.0`.

### T3.5 — `start` / `end` clipping + `align` tangent rotation

- **Files:** `src/render/shaders/light_trail.wgsl`, `src/effects/light_trail.rs`.
- **Scope:** clip the visible trail to `[start * total_length, end * total_length]`. When `align == true`, rotate the head sprite to the polyline tangent at the head's arc-length position. Handle near-zero tangent vectors at curve seams (clamp to last valid tangent rather than `NaN`).
- **Accept:** scrubbing with `start=0.2, end=0.8` clips correctly; `align=true` rotates a non-circular head (test with a deliberately asymmetric head shape — can revert to circular for ship). No `NaN` artifacts at curve seams under fuzzed `progress` values.

---

## W4 — Control panel UI

References prompt §7.

### T4.1 — Look-chain match arm + per-parameter widgets

- **Files:** `src/windows/look_chain.rs:116` (`show_effect_full_params`); `src/windows/control_panel.rs` (use `modulator_slider` at `control_panel.rs:2329`).
- **Scope:** add the variant match arm. Widgets per prompt §7 table — modulator slider for `progress`; plain sliders / drag values for the rest; nested editor for `Palette`; paired range slider for `start`/`end` with `start <= end` enforced. Stage edits as a single `SetLayerEffects` per the existing flush pattern.
- **Accept:** all parameters are editable from the look-chain panel; edits produce one `SetLayerEffects` per gesture (matches existing video-speed pattern); Cmd-Z undoes each gesture cleanly.

### T4.2 — Add-effect menu wiring

- **Files:** `src/windows/look_chain.rs:902` (`add_node_to_chain` helper); the add-effect menu sites.
- **Scope:** add `LightTrail` to the add-effect menu with a sensible default config (HueShift palette, trail_length 0.2, etc., per prompt §4 defaults). Add menu only shows the entry when the selected layer's kind is `Svg`; for other layer kinds, gray out or hide.
- **Accept:** adding the effect to an SVG layer produces a working comet; menu hides / disables for non-Svg layers.

---

## W5 — Testing + show-day

References prompt §9.

### T5.1 — GPU golden for canonical configuration

- **Files:** `tests/headless_gpu.rs`; `tests/golden/light_trail_canonical.png` (new); a fixture SVG under `tests/fixtures/`.
- **Scope:** one canonical configuration — Fixed palette, mid-curve `progress=0.5`, default `trail_length` / `glow_blur` / `head_size`. Captured with `UPDATE_GOLDEN=1 make test-gpu` per the existing convention.
- **Accept:** `make test-gpu` is green on the dev Mac; the golden image is committed.

### T5.2 — Malformed-SVG no-op + `tracing` warn test

- **Files:** unit + integration test alongside `src/effects/light_trail.rs`.
- **Scope:** feed the effect (a) an SVG with no `<path>`, (b) a malformed SVG, (c) a path of zero length. Each case: `Effect::render` returns `false`, a `tracing::warn!` is emitted (capture via `tracing-test` or equivalent), no panic, no `wgpu` validation error.
- **Accept:** ≥ 3 tests; all pass under `make test`. `panic_restore` is not exercised — the effect must handle the bad input gracefully without raising.

### T5.3 — Manual smoke: scrub + autoplay

- **Files:** none (manual checklist appended to `docs/show-day-checklist.md` per T5.4).
- **Scope:** on dev Mac with a fixture SVG layer: (a) drag `progress` slider end-to-end — head moves smoothly, no jitter; (b) set `progress` to `Modulator::Time { period_secs: 4.0 }` — autoplay loops smoothly at vsync; (c) toggle `align`, `palette` variants, `start`/`end` — verify visually.
- **Accept:** observed visually on dev Mac. Record fps and any artifacts in the PR description.

### T5.4 — `release-show` build + show-day checklist update

- **Files:** `docs/show-day-checklist.md` (extend); CHANGELOG.
- **Scope:** confirm `make build-show` succeeds with the effect enabled. Add a show-day checklist line for the effect (e.g., "verify LightTrail effects render at expected fps before going live"). Update CHANGELOG with one line under the next version.
- **Accept:** `make build-show` is green; checklist + CHANGELOG land in the same PR.

---

## Operating model

- **Model:** Sonnet implements; Opus reviews. Read the prompt section referenced in the task header before starting.
- **Commit message format:** `005-T<N>.<M>: <title>` — e.g. `005-T1.2: SVG path → arc-length polyline`.
- **Branching:** one branch per task; merge straight to `main` once `make ci` is green.
- **Pre-commit hook** (wired by `make setup`) runs rustfmt + `cargo check` on staged files. Heavier checks live in `make ci`.
- **Tests with the implementation.** Schema / Mutation work follows the proptest pattern in `src/project/command.rs`. Render-path work adds a `tests/golden/` baseline under `--features gpu-tests`; use `UPDATE_GOLDEN=1` to (re-)record.
- **Read the right CLAUDE.md.** W2 + T2.\* tasks → `src/project/CLAUDE.md` (Mutation Reverse-storage rules). W3 tasks → `src/render/CLAUDE.md` (GPU lifecycle, `panic_restore`, build-time WGSL validation).
- **Don't bundle.** Resist the urge to fix nearby things; if it's worth doing, it already has a task ID or deserves one.
- **GPU bring-up tasks ship golden images.** T3.3 onward should at minimum update a local golden during dev; T5.1 commits the canonical one.
- **Decision tasks ship a decision file.** T1.1 and T3.1 each produce a markdown file under `specs/005-light-trail/` mirroring `004-phase-3-zone-tag-uniform-decision.md`'s shape.
- **Stop on red.** If `make ci` fails on `main`, pause and triage before opening a new branch — the same rule used in Phase 1/2.
