# Prompt — Effect::LightTrail (rainbow comet along an SVG path)

Paste this prompt to a coding agent working in this repo. It briefs the work and leaves explicit decision points open for the implementer to justify. It is **not** a task list — the `T1.*`-style breakdown lands in a sibling `tasks.md` later.

## 1. Goal

Implement a new `Effect::LightTrail` that draws a glowing rainbow comet head plus a fading tail along the geometry of the layer's source SVG. The quality bar is "premium glowing comet on an arbitrary curve," not "a ball moving on a line." The trail must hug the actual path geometry, the head must be luminous, the tail must fade smoothly from head to end, and color must distribute as a rainbow palette or a time-cycling hue. The effect runs on macOS via wgpu, must compose with the existing Look chain, and must not panic inside the per-frame `panic_restore` wrapper (see `src/show_day/panic_restore.rs` and `src/render/CLAUDE.md`).

The light trail effect belongs on a layer whose `kind` is already `LayerConfig::Svg { svg_path }` (`src/project/schema.rs`). It consumes that layer's existing source SVG as path geometry — it does **not** carry its own `d` attribute string and does **not** introduce a new layer kind.

## 2. Where this slots into rmap

Read `src/render/CLAUDE.md` and `src/project/CLAUDE.md` first — both apply.

Files to touch:

- **`src/effects/mod.rs`** — add `LightTrail { … }` variant to `Effect` (the enum starts at `mod.rs:41`; currently 7 variants: `Color`, `Tint`, `Blur`, `Transform`, `Feedback`, `Treatment`, `External`). Extend `Effect::render(ctx, clock) -> bool` dispatch at `mod.rs:214`. Define `LightTrailParams` with `#[repr(C)]` + `.to_wire_bytes()` mirroring the existing `BlurParams` / `ColorParams` pattern.
- **`src/effects/light_trail.rs`** (new) — `LightTrailPipeline` owning the `wgpu::RenderPipeline`, `BindGroupLayout`, sampler, and uniform buffer. Mirror `BlurPipeline` in `src/effects/blur.rs:38`.
- **`src/render/shaders/light_trail.wgsl`** (new) — gets naga-validated at compile time by `build.rs`. If you need helper functions, use the existing prepend mechanism in `build.rs` (don't `include_str!` from outside the shaders dir).
- **`src/project/schema.rs:10`** — bump `CURRENT_SCHEMA_VERSION` from 12 to 13.
- **`src/project/migrate.rs`** — add a v12→v13 migration. It is a no-op for projects without the variant; the migration exists so the version bump is honored. Follow the v11→v12 pattern already there.
- **`src/project/command.rs`** — no new `Mutation` variant. `SetLayerEffects` (struct at `command.rs:745`, `ReverseStorage` impl at `command.rs:754`) already covers add / edit / remove (whole-vec snapshot per Mutation rule 2 in `src/project/CLAUDE.md`). Confirm by reading that section.
- **`src/windows/look_chain.rs`** — add the variant to `show_effect_full_params` (`look_chain.rs:116`) and to the add-effect menu (the `add_node_to_chain` helper at `look_chain.rs:902`).
- **`src/windows/control_panel.rs`** — use `modulator_slider` (`control_panel.rs:2329`) for modulator-typed fields; plain egui widgets for the rest.

No edits to `src/app.rs` should be necessary — the effect plugs into the existing Look chain dispatch.

## 3. Path data model

You consume the existing `LayerConfig::Svg { svg_path: PathBuf }`. rmap has **no parametric path infrastructure today** (no `getTotalLength`, no Bézier or polyline tooling). You must build it.

Required investigation, to be summarized in your PR description:

- How does the SVG layer rasterize today? (resvg in `src/image_layer.rs`.) Decide where to extract the parametric path subset. **Re-parse the source SVG at effect-load time is preferred** — extracting from the rasterized representation is lossy.
- Decide how multi-`<path>` SVGs are handled. Options: pick first path, pick longest, concatenate in document order, or expose a `path_index` parameter. **Pick one and justify it.** If you expose `path_index`, default to 0.
- Build a CPU-side polyline approximation (uniform arc-length sampling) at load time so the GPU can do constant-time `point-at-distance` lookup. Sampling resolution is a parameter (default 512 samples). Cache the polyline; do not recompute per frame.
- Decide where the polyline lives: a `LightTrailPipeline`-owned `wgpu::Buffer` (storage or 1-D texture) is the natural fit. State your choice and confirm macOS Metal compatibility through wgpu.

If re-parsing fails (malformed SVG, no path elements), the effect must render as a no-op (`Effect::render` returns `false`) and log via `tracing` at `warn` level — do not panic.

## 4. Required parameters

Use rmap idiom: `Modulator` for time-varying fields, plain scalars for static config. The `Modulator` type lets the user drive a value with time, MIDI, OSC, etc. — so `duration` / `speed` / `easing` / `loop` / `repeat` from the original browser-flavored prompt are **not new fields**: they are already expressible via a `Modulator::Time { … }` driving `progress`. State this in the user-facing param docs so the surface stays small.

Required fields on `Effect::LightTrail { … }`:

| Field | Type | Range / units | Default | Notes |
|---|---|---|---|---|
| `progress` | `Modulator` | 0..1 normalized path position | `Modulator::Static(0.0)` | Externally drivable. Drag in the UI = manual scrub; `Modulator::Time` = autoplay. |
| `trail_length` | `f32` | 0..1 (normalized along path) | 0.2 | **Explicitly normalized**, not pixels. Document this in the field doc-comment. |
| `head_size` | `f32` | pixels in render-target space | 12.0 | Radius of bright head core. |
| `stroke_width` | `f32` | pixels | 3.0 | Trail core thickness. |
| `glow_blur` | `f32` | pixels (Gaussian stdDev-equivalent) | 8.0 | Halo softness. |
| `opacity_fade` | `f32` | 0..1 | 0.7 | Falloff exponent from head (1.0) to tail end (0.0). Higher = faster decay. |
| `palette` | `Palette` enum | — | `Palette::HueShift { speed: 0.2 }` | Variants: `Fixed(Vec<[u8; 4]>)` and `HueShift { speed: f32 }`. |
| `gradient_spread` | `f32` | 0..1 | 1.0 | Fraction of visible trail over which colors are distributed. |
| `start` | `f32` | 0..1 | 0.0 | Lower bound of the path subrange the effect animates within. |
| `end` | `f32` | 0..1 | 1.0 | Upper bound. `end <= start` ⇒ no-op. |
| `align` | `bool` | — | `false` | Rotate head to tangent. Default false because a circular head doesn't need it. |
| `path_index` | `u32` | — | 0 | Which `<path>` in the SVG to follow if you exposed this knob. |
| `sample_resolution` | `u32` | 64..4096 | 512 | Polyline samples for the arc-length approximation. |

All `f32` fields are clamped at deserialization to their stated ranges; values outside the range are not an error but get clamped silently (mirrors `BlurParams::radius_px` behavior — verify).

## 5. Render model

The shader produces the comet in a single full-screen pass over the layer's render target, matching the existing `RenderCtx { source_view, dst_view, intermediate_view }` ping-pong pattern (see `src/effects/mod.rs` and `src/render/CLAUDE.md`). Two practical approaches — **pick one and justify in the PR description**:

- **A. Distance-field (recommended).** Bind the precomputed arc-length-parameterized polyline as a storage buffer (or 1-D texture if storage buffers are not viable on macOS Metal via wgpu — verify before committing). For each fragment, compute distance to the visible trail segment between `head_dist - trail_length * total_length` and `head_dist`. Color/opacity derive from arc-length position. Handles glow and fade in-shader, no multi-pass overdraw, no dash-offset hacks. Most resilient to sharp curves.
- **B. Multi-pass dash.** Rasterize the polyline as a stroked primitive with `stroke-dashoffset`-equivalent uniforms, then run a separable Gaussian for the glow. Closer to the original prompt's framing; uses more bandwidth and risks visible facet seams at low `sample_resolution`.

Either way, the effect must compose with the existing Look chain order (`src/render/CLAUDE.md`) and not violate the per-frame render-graph invariants.

## 6. Rendering order inside the effect's pass

In-shader composition order, back to front:

1. base layer (`source_view`, unmodified — pass through anywhere the trail doesn't cover)
2. soft trail glow (wide Gaussian, low opacity)
3. sharper trail core
4. head halo
5. bright head core

Write to `dst_view`. Return `true` from `Effect::render` to flip ping-pong.

## 7. Control panel UI

Add a match arm in `show_effect_full_params` (`src/windows/look_chain.rs:116`) for the new variant. Per-field widgets:

- `progress` — `modulator_slider`, range 0.0..1.0. (Modulator-eligible.)
- `trail_length` — plain slider 0.0..1.0, label "Trail length (path %)".
- `head_size` — plain slider 1.0..64.0 px.
- `stroke_width` — plain slider 0.5..16.0 px.
- `glow_blur` — plain slider 0.0..64.0 px.
- `opacity_fade` — plain slider 0.0..1.0.
- `palette` — radio between `Fixed` and `HueShift`; nested editor for whichever is active (color-array editor for `Fixed`, single-speed slider for `HueShift`).
- `gradient_spread` — plain slider 0.0..1.0.
- `start`, `end` — paired range slider 0.0..1.0; enforce `start <= end` in the UI.
- `align` — checkbox.
- `path_index` — drag-value `u32`.
- `sample_resolution` — drag-value `u32`, range 64..4096, with a tooltip noting that changing this rebuilds the polyline (not free).

Stage edits as a single `SetLayerEffects` per the flush pattern already used in `look_chain.rs`.

Add the effect to the add-effect menu via the `add_node_to_chain` helper (`look_chain.rs:902-912`).

## 8. Persistence + undo

`Effect::LightTrail { … }` serializes via the existing tagged-enum mechanism on `Effect` — no custom serde plumbing needed. Verify by reading `src/effects/mod.rs:41-127` and the JSON examples in the existing variants.

Add / remove / edit all go through `SetLayerEffects` at `src/project/command.rs:745-770`. Per the whole-vec snapshot rule in `src/project/CLAUDE.md`, every edit produces a new `SetLayerEffects` carrying the full `Vec<EffectNode>` before and after. Undo replays the inverse.

Schema migration: bump `CURRENT_SCHEMA_VERSION` to 13 in `src/project/schema.rs:10`, add a v12→v13 entry in `src/project/migrate.rs`. The migration is a no-op (projects saved at v12 simply lack the variant); the version bump exists so future readers know the schema can carry it.

## 9. Acceptance criteria

Mirroring the style in `specs/004-phase-1-tasks.md`:

- `make ci` is clean (fmt, clippy `-D warnings`, nextest, doctests).
- `build.rs` naga validation passes for the new WGSL.
- Round-trip: save a project with a `LightTrail` effect at default params, reload, render is identical (golden image stable across save/load).
- Adding the effect via the look-chain menu produces a `SetLayerEffects` mutation that undoes cleanly back to the previous chain.
- Manual scrub: dragging `progress` in the control panel moves the head along the path without visible jitter at default `sample_resolution`.
- Autoplay: a `Modulator::Time { period_secs: 4.0, … }` driving `progress` produces smooth motion at vsync on the dev machine.
- At least one GPU golden test (`make test-gpu`, `tests/golden/`) covers a canonical configuration. Pattern: see existing `tests/headless_gpu.rs` and adjacent goldens.
- No panics inside `panic_restore::run_frame_assert_unwind_safe` for the configurations covered by golden tests. Verify by deliberately feeding the effect a malformed SVG path and confirming it logs + returns `false` rather than unwinding.
- Show-day reliability: the effect compiles cleanly under the `release-show` profile (`make build-show`).

## 10. Pitfalls

- **Lossy path data** if extracted from the rasterized SVG instead of re-parsed. Re-parse the source file.
- **Sampling resolution too low** ⇒ visible faceting on tight curves. The default 512 is fine for moderate curves; document the knob.
- **Tail extending past `start` / `end` bounds** ⇒ either clip the visible trail to `[start, end]` (preferred) or wrap (avoid; visually confusing on partial ranges). Clip.
- **Glow blur radius vs render-target resolution.** `glow_blur` is in pixels of the render target, not path-space units. State this in the doc-comment; otherwise the halo size shifts when the projector resolution changes.
- **HueShift × opacity_fade order.** Compute hue first, premultiply alpha last, so fading doesn't desaturate the color.
- **Storage-buffer support on macOS Metal via wgpu.** Verify before committing to the SDF approach. If unavailable, use a 1-D texture for the polyline; the SDF approach still works.
- **Modulator evaluation on `progress`.** The `Modulator` is evaluated once per frame on the CPU side and pushed into the UBO — do **not** put `progress` evaluation inside the shader. (Mirrors how `BlurParams::radius_px` is evaluated; verify.)

## 11. Open decisions — justify in the PR description

The prompt deliberately does not pre-decide these. Pick one of each and write a one-paragraph justification in the PR body:

- SDF render approach vs multi-pass dash.
- Multi-`<path>` SVG handling strategy (and whether `path_index` is exposed).
- Whether path extraction is shared with the existing SVG layer load path (`src/image_layer.rs`) or kept independent.
- Whether `palette` accepts variants beyond `Fixed` / `HueShift` (e.g. a `Modulator::Time` driving palette index). Defer unless trivially in-scope.

## 12. Out of scope

- Inline `d` attribute strings on the effect itself — path must come from the layer's source SVG.
- New `Mutation` variants — reuse `SetLayerEffects`.
- New layer kinds.
- Tasks-list breakdown (`tasks.md`) — that's a follow-on file.
- Cross-platform support — macOS only, per `CLAUDE.md`.
