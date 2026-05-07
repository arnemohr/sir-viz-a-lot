# Tasks: rmap (v1.1)

> Companion to `specs/001-initial-setup.md` (the *what*) and
> `specs/001-initial-setup-plan.md` (the *how*). This file is the *who-does-what*.
>
> Each task is sized to one focused work session (½ to 1 day). Pick a task,
> read the plan section it cites, do the work, open a PR. Do not bundle
> tasks across milestone boundaries.

---

## Conventions

- **ID format**: `T-<milestone>-<NN>` for milestone tasks (`T-M2-04`),
  `X-NN` for cross-cutting refactors, `D-NN` for open decisions that need a
  human call before work can start.
- **Estimate**:
  - **S** — under ½ day
  - **M** — ½ to 1 day
  - **L** — 1 to 2 days *(if you find yourself writing one of these, look for
    a way to split it)*
- **Acceptance**: every task lists how you prove it is done. If you cannot
  state an acceptance check, the task is too vague — split it.
- **Depends on**: hard dependencies (must merge first). Implicit
  cross-milestone dependency: complete the previous milestone first.
- **Plan ref**: section in `001-initial-setup-plan.md` that holds the
  context. The plan is authoritative for design rationale; this file is
  authoritative for granularity and order.

---

## Index

| Bucket | Count | Total estimate |
|---|---:|---:|
| Cross-cutting (X) | 6 | ~1 day |
| Decisions (D, all resolved) | 3 | — |
| M1 — Hello rectangle | 6 | 2 days |
| M1.5 — Venue dry-run | 1 | ½ day calendar gate |
| M2 — Calibration tooling | 11 | 3 days |
| M3 — SVG on screen | 7 | 4 days |
| M4 — Effects + modulators | 15 | 6 days |
| M5 — Multi-layer + scenes + warp + masking | 17 | 8 days |
| M6 — Project save/load + autostart + docs | 7 | 1.5 days |
| M7 — Polish (sketch) | 7 | open |
| **Total tracked v1 work** | **71** | **~26 days** |

---

## Decisions (D) — resolved

> All v1-blocking decisions resolved on 2026-05-07. Re-open by adding a new
> D-NN entry below; do not silently change a resolved one.

### D-01 — egui: multi-window or single-window? **Resolved: multi-window**

- **Plan ref**: §3.7 item 1.
- **Decision**: multi-window. Borderless fullscreen output window on the
  projector + a separate egui control window on the primary display, sharing
  one `wgpu::Device` via `egui-wgpu`'s multiple-viewports path.
- **Fallback budget**: if multi-window setup is not working after ½ day of
  effort on T-M4-14, fall back to single-window without re-asking. Document
  the fallback in the PR.
- **Unblocks**: T-M4-14, T-M4-15.

### D-02 — `homography_round_trip_smoke`: ignore or leave green? **Resolved: `#[ignore]` now**

- **Plan ref**: §3.7 item 5.
- **Decision**: annotate the empty test with
  `#[ignore = "M5: real homography solver, see T-M5-13"]` immediately.
  Removes the false-green from CI; `cargo test` will show it as ignored
  until T-M5-13 fills the body.
- **Action**: see **X-05**.

### D-03 — Mask geometry: triangle fan or SDF? **Resolved: signed-distance texture (SDF)**

- **Question elevated from**: T-M5-05 implementer-choice ("pick whichever
  ships first").
- **Decision**: bake the polygon into a one-channel signed-distance texture;
  sample + `smoothstep` in the shader for the feathered alpha. Future-proofs
  for non-convex masks, bezier mask boundaries, and animated mask morphing.
- **Cost**: ~+1 day on M5 vs. the triangle-fan alternative. **M5 estimate
  bumped from 7 to 8 days.**
- **Action**: T-M5-05 rewritten and bumped from estimate **M → L**;
  T-M5-06 narrowed to SDF sampling only.
- **Plan ref**: §3.4 M5 deltas (note: §3.4 mentions "polygon-fan rasterized
  into an SDF" — that earlier wording predates this decision; this
  resolution overrides the plan on the implementation choice).

---

## Cross-cutting tasks (X)

These can be picked up at any time; they do not depend on a milestone.

### X-01 — Reserve `audio`, `midi`, `osc` cargo features as comments

- **Files**: `Cargo.toml`
- **Scope**: Add commented-out `[features]` entries for `audio`, `midi`,
  `osc` with the dep wiring they will need at M7. Do **not** add the actual
  dependencies — the spec is explicit that they wait until M7 work begins.
- **Acceptance**: `Cargo.toml` has the three commented entries; `cargo check`
  unaffected.
- **Plan ref**: §3.3 "Cargo features".
- **Estimate**: S.

### X-02 — Add `.gitkeep` files to `tests/` and `docs/`

- **Files**: `tests/.gitkeep`, `docs/.gitkeep`
- **Scope**: The empty directories created by the scaffold are not tracked
  by git and will vanish from a fresh clone. Add `.gitkeep` files so the
  layout survives. (Skip if T-M2-11 lands first — that creates `tests/`
  content.)
- **Acceptance**: `git ls-files tests/ docs/` lists at least one entry.
- **Estimate**: S.

### X-03 — Replace `Default for Renderer` footgun

- **Files**: `src/render/mod.rs:43`
- **Scope**: Remove `impl Default for Renderer` — `Self::new().expect(...)`
  becomes a panic-on-`Default` once `new` does real wgpu init. Do this
  *before* T-M1-03 fills `new`, or as part of T-M1-03.
- **Acceptance**: `cargo check`, `cargo clippy -- -D warnings` still pass;
  no consumer depends on `Renderer::default()` (verify with
  `rg "Renderer::default|Renderer\s*::\s*default"`).
- **Plan ref**: §3.7 item 6.
- **Estimate**: S.

### X-04 — Tighten crate-root `#![allow]` during/after M5

- **Files**: `src/main.rs:1-3`
- **Scope**: The blanket `#![allow(dead_code, unused_imports)]` is a
  skeleton-stage convenience. As M3+ fills in real consumers, narrow to
  per-module `#[allow(...)]` and remove what is no longer needed. Goal: zero
  blanket allows by end of M5.
- **Acceptance**: `cargo clippy -- -D warnings` passes with the blanket
  removed.
- **Plan ref**: §3.7 item 7.
- **Estimate**: M (touches every module that has a stub).

### X-05 — `#[ignore]` the empty homography smoke test

- **Files**: `src/render/warp.rs:18`
- **Scope**: Implements **D-02**. Add
  `#[ignore = "M5: real homography solver, see T-M5-13"]` above the
  `homography_round_trip_smoke` test. Do nothing else; T-M5-13 still owns
  the real implementation.
- **Acceptance**: `cargo test` shows
  `homography_round_trip_smoke ... ignored` instead of `... ok`. Total test
  count drops from 4 to 3 passing + 1 ignored.
- **Estimate**: S (~5 minutes).

### X-06 — Interim monitor selection flags (unblocks T-M1.5-01)

- **Files**: `src/main.rs`, `src/app.rs`
- **Scope**: Until the egui dropdown (T-M4-15) and the saved
  `Project.output_monitor_index` (T-M6-04) land, the operator has no way
  to point the output at a specific projector — `App::resumed` hardcodes
  monitor index 0. Add two CLI flags:
  - `--list-monitors`: enumerate monitors (index, name, size@position,
    scale_factor) and exit before opening the output window.
  - `--monitor INDEX`: override the default monitor index for the output
    window. CLI takes precedence over `Project.output_monitor_index`
    (which T-M6-04 will read from the project file).
  Required to make T-M1.5-01 venue dry-run actually doable without
  source edits.
- **Acceptance**:
  - `cargo run -- --list-monitors` prints monitor info and exits 0 without
    opening a fullscreen window.
  - `cargo run -- --monitor 1` opens the output window on monitor index
    1 (or logs a clean error if that index is out of range).
  - Existing `cargo run` (no flags) keeps using monitor 0.
- **Plan ref**: precedes T-M1.5-01.
- **Estimate**: S (~½ hour).

---

## M1 — Hello rectangle

Goal (spec §M1, plan §M1): borderless fullscreen window on a chosen monitor;
wgpu device/queue/surface; renders the gradient quad from `triangle.wgsl`;
Esc closes cleanly with no GPU validation errors.

### T-M1-01 — Real monitor enumeration

- **Files**: `src/monitors.rs`
- **Scope**: Replace `list() -> Vec::new()` stub with a function taking
  `&winit::event_loop::ActiveEventLoop` and returning the live monitor list
  (index, name, size, position, scale_factor). On macOS, fall back to
  `objc2-app-kit` when `MonitorHandle::name()` returns `None`.
- **Acceptance**: A small `#[test]` (or temporary `bin/`-style example) is
  not required; visible verification is via T-M1-04 dropdown population.
- **Depends on**: —
- **Plan ref**: §3.4 M1 deltas.
- **Estimate**: M.

### T-M1-02 — Borderless fullscreen `Window` + cursor hidden

- **Files**: `src/windows/output.rs`
- **Scope**: Hold a `winit::window::Window` plus the `wgpu::Surface` and
  `SurfaceConfiguration`. `new(active_loop, monitor)` opens
  `Fullscreen::Borderless(Some(monitor))`, calls
  `set_cursor_visible(false)`, configures the surface for `Bgra8UnormSrgb`
  (or whatever the surface prefers).
- **Acceptance**: From T-M1-04 the window opens fullscreen on the chosen
  monitor with no cursor visible; resize event re-configures the surface
  without artifacts.
- **Depends on**: T-M1-01.
- **Plan ref**: §3.6 row "Cursor hidden on output".
- **Estimate**: M.

### T-M1-03 — Real `Renderer::new` + `render_to`

- **Files**: `src/render/mod.rs`, drop `impl Default for Renderer`
  (or do via X-03)
- **Scope**: Implement `Renderer::new(surface)` using
  `pollster::block_on` to request adapter + device + queue. Build the
  `triangle.wgsl` pipeline once, cache it. Expose
  `render_to(target, scene, clock) -> Result<(), RenderError>` (target is the
  `PresentTarget` from plan §3.1; for M1, only the `Surface` variant is
  used and `scene` can be a placeholder).
- **Acceptance**: Calling `render_to` from T-M1-04 produces the gradient on
  the projector. `RUST_LOG=trace cargo run` shows no wgpu validation errors.
- **Depends on**: T-M1-02.
- **Plan ref**: §3.1 "render::Renderer", §3.4 M1 deltas.
- **Estimate**: M.

### T-M1-04 — Wire `App::run` with `ApplicationHandler`

- **Files**: `src/app.rs`
- **Scope**: Implement `winit::application::ApplicationHandler` for `App`.
  On `resumed`: enumerate monitors via T-M1-01, pick the first (or the saved
  index from `Project`), open the output window via T-M1-02, init the renderer
  via T-M1-03. On `window_event`: handle `KeyboardInput` Esc → exit;
  `RedrawRequested` → `render_to`. Schedule redraws in `about_to_wait`.
- **Acceptance**: `make run` opens the gradient window; Esc closes; no
  background panics.
- **Depends on**: T-M1-01, T-M1-02, T-M1-03.
- **Plan ref**: §3.4 M1 deltas; data-flow diagram in §3.1.
- **Estimate**: M.

### T-M1-05 — Surface lost / outdated recovery

- **Files**: `src/windows/output.rs`
- **Scope**: On `wgpu::SurfaceError::{Lost, Outdated, Suboptimal}` returned
  from `Surface::get_current_texture()`, call a `recreate_surface()` method
  that calls `surface.configure()` with the current `SurfaceConfiguration`.
  Log via `tracing::warn!`.
- **Acceptance**: Manual smoke test — unplug and replug the projector while
  the app runs; the output recovers without restart.
- **Depends on**: T-M1-04.
- **Plan ref**: §3.6 row "Surface lost/outdated recovery".
- **Estimate**: S.

### T-M1-06 — CLI error context in `main`

- **Files**: `src/main.rs`
- **Scope**: Pass parsed `Cli` to `App::run`. Wrap conversion from
  `RmapError` into `anyhow::Error` with `with_context(|| format!("project={:?}",
  cli.project))` so a failed launch tells the operator which file was at
  fault. Honor `--autostart` only as a no-op for M1 (real wiring in T-M6-04).
- **Acceptance**: `RUST_LOG=info make run -- --autostart` does not panic;
  invalid `--project` prints a contextual error, not a backtrace.
- **Depends on**: T-M1-04.
- **Estimate**: S.

---

## M1.5 — Venue dry-run (calendar gate)

### T-M1.5-01 — Venue dry-run report

- **Files**: `docs/m15-dry-run.md` *(new)*
- **Scope**: Take the M1 binary to a real HDMI projector. Verify: borderless
  fullscreen lands on the projector, no display sleep within ½ hour, no
  Mission Control glitches, Esc closes cleanly, no flicker on
  reconfiguration. Document each as pass/fail with notes.
- **Acceptance**: One-page report committed under `docs/`. **If any check
  fails**, halt M2+ and open an issue.
- **Depends on**: T-M1-06.
- **Plan ref**: §3.4 M1.5.
- **Estimate**: S calendar (½ day including travel).

---

## M2 — Calibration tooling

Goal (spec §6 + plan §M2): test patterns; blackout (`B`) + freeze (`F`);
display-sleep prevention; rolling file logs; per-frame `catch_unwind`.

### T-M2-01 — Daily rolling file log

- **Files**: `src/main.rs:42`
- **Scope**: Add `tracing_appender::rolling::daily(
  "~/Library/Logs/rmap/", "rmap.log")` (resolve `~` via `dirs` crate or
  `std::env::var("HOME")`) and layer it alongside the existing stderr
  `fmt::layer()`. Use `tracing_appender::non_blocking` for the file writer
  to keep frame timing clean.
- **Acceptance**: `make run` then `ls ~/Library/Logs/rmap/` shows
  `rmap.log.YYYY-MM-DD`; tailing it shows initialization spans.
- **Depends on**: —
- **Plan ref**: §3.6 row "Logging — rotating file".
- **Estimate**: S.

### T-M2-02 — `panic_restore` module

- **Files**: `src/show_day/panic_restore.rs` *(new)*,
  `src/show_day/mod.rs`
- **Scope**: New module exposing
  `pub fn run_frame<F: FnOnce() -> Result<(), RenderError> + UnwindSafe>(f: F)
  -> Result<(), RenderError>` that wraps `f` in `std::panic::catch_unwind`,
  converts a panic payload to `RenderError::RenderPanic { message }`. Add the
  `RenderPanic` variant to `RenderError` in `src/render/mod.rs:11`.
- **Acceptance**: New unit test
  `panic_restore::tests::panic_becomes_error_not_unwind` proves panics convert
  to `RenderError::RenderPanic`.
- **Depends on**: —
- **Plan ref**: §3.6 row "Panic restore".
- **Estimate**: S.

### T-M2-03 — Wire `catch_unwind` into the render loop

- **Files**: `src/render/mod.rs`
- **Scope**: Wrap the body of `Renderer::render_to` in
  `panic_restore::run_frame(...)`. On `RenderError::RenderPanic`, log via
  `tracing::error!` and surface to the control window (overlay added in
  T-M2-04).
- **Acceptance**: Test `render::tests::render_panic_does_not_propagate`
  (added in T-M2-11) passes.
- **Depends on**: T-M2-02.
- **Plan ref**: §3.6 row "Panic restore".
- **Estimate**: S.

### T-M2-04 — Real `IOPMAssertion` via `objc2-io-kit`

- **Files**: `src/show_day/sleep_assertion.rs`
- **Scope**: Replace the macOS stub body with real
  `IOPMAssertionCreateWithName` for the assertion type
  `kIOPMAssertionTypePreventUserIdleDisplaySleep`. Hold the
  `IOPMAssertionID` in the struct; call `IOPMAssertionRelease` in `Drop`.
  Keep the non-macOS no-op as-is.
- **Acceptance**: Manual: run app for 30 min idle on macOS;
  `pmset -g assertions` shows `PreventUserIdleDisplaySleep`. Quit; the
  assertion clears within seconds.
- **Depends on**: —
- **Plan ref**: §3.6 row "Display-sleep prevention".
- **Estimate**: M.

### T-M2-05 — Test pattern: 50 px grid shader

- **Files**: `src/render/shaders/test_grid.wgsl` *(new)*
- **Scope**: Procedural shader emitting a 50 px grid (1 px lines, dim grey
  on black, brighter line every 5 cells). Validated by `build.rs`.
- **Acceptance**: `cargo build` validates the shader without errors. Visual
  proof comes via T-M2-09.
- **Depends on**: —
- **Estimate**: S.

### T-M2-06 — Test pattern: crosshair + corner markers shader

- **Files**: `src/render/shaders/test_crosshair.wgsl` *(new)*
- **Scope**: Procedural crosshair (full-screen plus + diagonals optional)
  with prominent corner markers.
- **Acceptance**: `cargo build` validates. Visual via T-M2-09.
- **Depends on**: —
- **Estimate**: S.

### T-M2-07 — Test pattern: white levels + color bars shader

- **Files**: `src/render/shaders/test_levels.wgsl` *(new)*
- **Scope**: One shader, uniform `mode: u32` selects between
  `White100 / White50 / White25 / ColorBars` (SMPTE 7-bar split). Drives
  multiple `TestPattern` enum variants from one pipeline.
- **Acceptance**: `cargo build` validates.
- **Depends on**: —
- **Estimate**: M.

### T-M2-08 — `TestPattern::render` dispatch

- **Files**: `src/test_patterns.rs`
- **Scope**: Add
  `pub fn render(&self, encoder, dst: &TextureView, device: &Device)`
  that selects the right pipeline (cached). One pipeline per shader file,
  one bind group per `levels` mode.
- **Acceptance**: T-M2-09 can switch patterns at runtime; visual matches
  expectation on the projector.
- **Depends on**: T-M2-05, T-M2-06, T-M2-07.
- **Estimate**: M.

### T-M2-09 — Key handlers: `B`, `F`, `T`

- **Files**: `src/windows/output.rs`, `src/app.rs`
- **Scope**: In the output window's keyboard handling, map:
  `B` → `OutputState::toggle_blackout`,
  `F` → `OutputState::toggle_freeze`,
  `T` → cycle `TestPattern`.
  Render order: if `blackout` → clear black; else if `freeze` → present last
  framebuffer; else if `TestPattern != None` → render via T-M2-08; else →
  normal scene.
- **Acceptance**: Manual on projector — `B` blacks out; `F` freezes (control
  window edits do not appear); `T` cycles through patterns including off.
- **Depends on**: T-M2-08.
- **Plan ref**: §3.6 rows "Blackout", "Freeze", "Test patterns".
- **Estimate**: M.

### T-M2-10 — Error-overlay scaffold (control window)

- **Files**: `src/windows/control.rs`
- **Scope**: Add an `error_overlay(msg: &str)` API stub that pushes a sticky
  panel to the control window. Real wiring (egui integration) lands in
  T-M4-14; for M2 the function exists and is called from T-M2-03 panic
  handling, even if rendering is deferred.
- **Acceptance**: `cargo check` passes; calls compile.
- **Depends on**: T-M2-03.
- **Plan ref**: §3.6 row "Error overlay".
- **Estimate**: S.

### T-M2-11 — Test: `render_panic_does_not_propagate`

- **Files**: `tests/render_panic.rs` *(new)* or
  `src/render/mod.rs::tests`
- **Scope**: Test gates renderer with a `force_panic: bool` toggle (cfg-test
  only). Call `render_to`; assert `RenderError::RenderPanic` returned, no
  panic propagation.
- **Acceptance**: `cargo test render_panic_does_not_propagate` passes.
- **Depends on**: T-M2-03.
- **Estimate**: S.

---

## M3 — SVG on screen

Goal (spec §1 + plan §M3): real SVG load via `resvg`; off-thread raster;
hot-reload via `notify`-debouncer.

### T-M3-01 — `SvgLayer::load` real implementation

- **Files**: `src/svg_layer.rs`
- **Scope**: Replace stub with real loader: read file → `usvg::Tree::from_str`
  with default `Options`. Store the tree in `SvgLayer`. Add a `bbox()` accessor.
- **Acceptance**: `cargo test svg_layer::tests::load_smoke` passes (loads a
  tiny `tests/fixtures/circle.svg`, asserts non-empty bbox).
- **Depends on**: —
- **Estimate**: S.

### T-M3-02 — Rasterize via `resvg` + `tiny-skia`

- **Files**: `src/svg_layer.rs`
- **Scope**: Add
  `rasterize(&self, size: (u32, u32)) -> tiny_skia::Pixmap` using
  `resvg::render`. Apply 2× oversampling internally; downsample with
  `image::imageops::resize` for the upload. Cache the last `(size, generation)`.
- **Acceptance**: `cargo test svg_layer::tests::rasterize_dimensions` passes
  (rasters at requested size).
- **Depends on**: T-M3-01.
- **Plan ref**: §3.4 M3 deltas; M3 risk note on premultiplied alpha.
- **Estimate**: M.

### T-M3-03 — GPU texture upload (RGBA8 unmultiplied)

- **Files**: `src/svg_layer.rs`
- **Scope**: Add
  `upload(&mut self, queue: &Queue, device: &Device, pixmap: &Pixmap)`.
  Format `Rgba8UnormSrgb` if downstream sampling expects sRGB; document the
  *unmultiplied* alpha choice in a one-line comment per the M3 risk note.
- **Acceptance**: T-M3-06 can render the layer to a textured quad; visible
  on projector.
- **Depends on**: T-M3-02.
- **Estimate**: M.

### T-M3-04 — Off-thread raster worker

- **Files**: `src/svg_layer/worker.rs` *(new)*,
  `src/svg_layer.rs` (re-export module)
- **Scope**: `Worker::spawn() -> (Sender<RasterJob>, Receiver<RasterDone>)`
  using `crossbeam-channel` and one `std::thread::spawn`. Job carries
  `{ layer_id, path, size, generation }`; result carries `{ layer_id, pixmap,
  generation }`. Stale `generation` results are dropped.
- **Acceptance**: Unit test
  `worker::tests::stale_generation_dropped` enqueues two jobs for the same
  layer at gens 1 and 2; asserts only gen-2 result is delivered.
- **Depends on**: T-M3-02.
- **Estimate**: M.

### T-M3-05 — `notify` debouncer wrapper

- **Files**: `src/svg_layer/watcher.rs` *(new)*
- **Scope**: Thin wrapper around
  `notify_debouncer_full::new_debouncer(Duration::from_millis(250), ...)`.
  Expose `Watcher::new(paths: &[PathBuf]) -> Receiver<WatchEvent>`; events
  carry the affected layer's path.
- **Acceptance**: T-M3-07 covers behaviour.
- **Depends on**: —
- **Estimate**: M.

### T-M3-06 — App-level event drain + texture handle swap

- **Files**: `src/app.rs`
- **Scope**: Per frame:
  1. Drain `WatchEvent`s; for each affected layer, enqueue a `RasterJob`
     with an incremented generation.
  2. Drain `RasterDone`s; if generation matches the layer's latest, upload
     to GPU and swap the texture handle.
  3. Issue redraw.
- **Acceptance**: Manual on projector — edit an SVG, output updates within
  ~500 ms; no frame stutter (verify with a 200 KB SVG).
- **Depends on**: T-M3-03, T-M3-04, T-M3-05.
- **Plan ref**: data-flow diagram in §3.1.
- **Estimate**: M.

### T-M3-07 — Test: `hot_reload_event_coalescing`

- **Files**: `tests/hot_reload.rs` *(new)* or `svg_layer/watcher.rs::tests`
- **Scope**: Inject 3 `notify` events for the same path within 100 ms;
  assert exactly one `WatchEvent` is emitted by the debouncer wrapper.
- **Acceptance**: `cargo test hot_reload_event_coalescing` passes.
- **Depends on**: T-M3-05.
- **Estimate**: S.

---

## M4 — Effects + modulators

Goal (spec §2 + §3 + plan §M4). Effects as ping-pong WGSL passes; modulator
dispatch (already real after Phase 2); `egui` control panel; tap tempo.

### T-M4-01 — `EffectPipeline` ping-pong textures

- **Files**: `src/render/pipeline.rs`
- **Scope**: `EffectPipeline { ping: TextureView, pong: TextureView,
  flip: bool }` allocated once at layer-resize. `apply(effects, clock,
  src_view) -> &TextureView` runs each effect in turn, alternating ping/pong;
  returns the final view.
- **Acceptance**: `cargo check`; consumed by T-M4-02 onward.
- **Depends on**: —
- **Estimate**: M.

### T-M4-02 — Color shader + pipeline

- **Files**: `src/effects/color.rs`, `src/render/shaders/color.wgsl` *(new)*
- **Scope**: WGSL pass for hue/sat/bri/con. Bind group has 4 `f32` uniforms
  (read from `Modulator::value(clock)`). Cache `RenderPipeline`.
- **Acceptance**: Golden-image test in T-M5-15 (deferred until headless GPU
  infra exists). Visual check via control panel slider in T-M4-15.
- **Depends on**: T-M4-01.
- **Estimate**: M.

### T-M4-03 — Blur horizontal pass

- **Files**: `src/render/shaders/blur_h.wgsl` *(new)*
- **Scope**: Separable gaussian, horizontal direction. Kernel size derived
  from a `radius_px` uniform (clamp ≤ 32 for v1).
- **Acceptance**: `cargo build` validates.
- **Depends on**: —
- **Estimate**: S.

### T-M4-04 — Blur vertical pass + `effects::blur` orchestration

- **Files**: `src/effects/blur.rs`, `src/render/shaders/blur_v.wgsl` *(new)*
- **Scope**: Vertical pass shader; `effects::blur::apply` runs horizontal
  then vertical into ping-pong views.
- **Acceptance**: Golden-image test in T-M5-16. Visual via T-M4-15.
- **Depends on**: T-M4-01, T-M4-03.
- **Estimate**: M.

### T-M4-05 — Transform vertex push

- **Files**: `src/effects/transform.rs`,
  `src/render/shaders/transform.wgsl` *(new)*
- **Scope**: Vertex stage takes a `glam::Mat3` push constant (or uniform);
  rotates/scales the textured quad about anchor. Translation is part of the
  matrix.
- **Acceptance**: Sliders in T-M4-15 visibly transform the layer.
- **Depends on**: T-M4-01.
- **Estimate**: M.

### T-M4-06 — `Effect::render` dispatch in `EffectPipeline`

- **Files**: `src/effects/mod.rs`
- **Scope**: Add `Effect::render(&self, ctx, clock)` matching on the enum,
  delegating to `effects::color::apply`, `effects::blur::apply`, etc.
  Closed-enum keeps this exhaustive.
- **Acceptance**: `cargo clippy -- -D warnings` passes (no
  `match` non-exhaustive warning).
- **Depends on**: T-M4-02, T-M4-04, T-M4-05.
- **Estimate**: S.

### T-M4-07 — `Param<T>` type

- **Files**: `src/controls/param.rs` *(new)*
- **Scope**: `pub enum Param<T: Copy> { Static(T), Modulated(Modulator),
  Bound(SourceRef) }` with `Param<f32>::value(clock, inputs) -> f32`.
  `Bound` arm returns `0.0` for v1; v1.5 fills it in.
- **Acceptance**: `cargo test controls::param::tests::static_passthrough`
  and `modulated_dispatches_to_modulator` pass.
- **Depends on**: —
- **Plan ref**: §3.2 input/control extension point.
- **Estimate**: S.

### T-M4-08 — `Source` trait + `ControlEvent` enum

- **Files**: `src/controls/mod.rs`
- **Scope**: Add `pub trait Source { fn poll(&mut self) -> Vec<ControlEvent>;
  fn read(&self, _: SourceRef) -> Option<f32> { None } }` and the
  `ControlEvent` enum (`TapTempo`, `SceneRecall(usize)`, `Blackout`, `Freeze`,
  `ParamSet { binding, value }`). Add `InputState::register(Box<dyn Source>)`
  and `poll() -> Vec<ControlEvent>` that drains all registered sources.
- **Acceptance**: `cargo check`; consumed by T-M4-09.
- **Depends on**: —
- **Plan ref**: §3.2.
- **Estimate**: S.

### T-M4-09 — `KeyboardSource` impl

- **Files**: `src/controls/keyboard.rs` *(new)*
- **Scope**: A `Source` impl that translates buffered winit keyboard events
  into `ControlEvent`s: Space → `TapTempo`, 1–9 → `SceneRecall(0..=8)`,
  B → `Blackout`, F → `Freeze`, T → cycle test pattern (separate event or
  reuse the M2 inline handling).
- **Acceptance**: `cargo test controls::keyboard::tests::space_emits_tap`
  passes (with a fake event injection helper).
- **Depends on**: T-M4-08.
- **Estimate**: M.

### T-M4-10 — Tap-tempo wiring

- **Files**: `src/app.rs`
- **Scope**: On `ControlEvent::TapTempo`, call `clock.tap()`. Display
  current BPM in the control window (egui label, see T-M4-15).
- **Acceptance**: T-M4-13 covers the math.
- **Depends on**: T-M4-09.
- **Estimate**: S.

### T-M4-11 — Test: `Modulator` dispatch invariants

- **Files**: `src/modulators/mod.rs::tests`
- **Scope**: Add tests:
  - `dispatch_static`: `Static(0.5).value(any) == 0.5`
  - `dispatch_sine_quarter_period`: `Sine{period_s:1, amp:1, ..}.value(clock@0.25s) ~ 1.0`
  - `dispatch_bpm_at_120`: `Bpm{divisor:1, ..}.value(clock@0.25s, bpm=120) ~ 1.0`
- **Acceptance**: `cargo test modulators::` passes 7+ tests.
- **Depends on**: —
- **Estimate**: S.

### T-M4-12 — Test: `tap_tempo_converges`

- **Files**: `src/clock.rs::tests`
- **Scope**: Test feeds 4 taps at 0.5 s intervals (using a mockable clock or
  by injecting `Instant`s through a test-only constructor). Asserts BPM ∈
  [115, 125].
- **Acceptance**: `cargo test clock::tests::tap_tempo_converges` passes.
- **Depends on**: —
- **Estimate**: M (requires injecting time, currently `Instant::now()`).

### T-M4-13 — Test: `bound_returns_zero_v1`

- **Files**: `src/controls/param.rs::tests`
- **Scope**: Document the v1 behaviour by test: `Param::Bound(...)` returns
  `0.0`. Pinning prevents accidental v1.5 partial-implementation in v1.
- **Acceptance**: `cargo test bound_returns_zero_v1` passes.
- **Depends on**: T-M4-07.
- **Estimate**: S.

### T-M4-14 — `egui-wgpu` + `egui-winit` multi-window integration

- **Files**: `src/windows/control.rs`
- **Scope**: Per **D-01** (multi-window): open a second `winit::Window` on
  the primary display via `egui-winit`'s multiple-viewports API. Share the
  `wgpu::Device` already created by `Renderer`. Render the egui frame each
  redraw cycle into the control window's own surface.
- **Acceptance**: Empty control window opens on the primary display while
  the output window stays fullscreen on the projector. Closing the control
  window does not close the output.
- **Fallback**: if multi-window setup is not converging after ½ day of
  honest effort, fall back to single-window (one winit window with an egui
  side-panel and the rendered scene in a viewport region) per the D-01
  fallback budget. Document the fallback in the PR.
- **Plan ref**: §3.7 item 1.
- **Estimate**: L (1 day for multi-window; ½ day for single-window
  fallback).

### T-M4-15 — Control panel: layer + effect + modulator sliders

- **Files**: `src/windows/control_panel.rs` *(new)*
- **Scope**: egui UI with collapsible per-layer panels containing:
  enable toggle, blend mode picker, opacity slider, and per-effect sliders
  (color: hue/sat/bri/con; blur: radius; transform: tx/ty/rot/scale). Each
  numeric slider has a context menu "→ Modulator" that swaps `Param::Static`
  for `Param::Modulated(...)`.
- **Acceptance**: Manual: change a slider → projector reflects the change
  within one frame. Set a sine modulator → projector animates.
- **Depends on**: T-M4-14, T-M4-06, T-M4-07.
- **Estimate**: L (1.5 days; the bulk of M4's wall time).

---

## M5 — Multi-layer + scenes + warp + masking

Goal (spec §4 + plan §M5).

### T-M5-01 — `Compositor::composite` N-layer blend

- **Files**: `src/render/compositor.rs`
- **Scope**: Composite a slice of `(view, blend_mode, opacity)` into a
  destination view. One `RenderPipeline` per `BlendMode`, cached.
- **Acceptance**: Visual via T-M5-15 + T-M5-16 golden tests; for two layers
  with `Add`, output is the per-channel sum clamped.
- **Depends on**: —
- **Plan ref**: §3.1 `compositor` signature.
- **Estimate**: M.

### T-M5-02 — Compositor WGSL

- **Files**: `src/render/shaders/compositor.wgsl` *(new)*
- **Scope**: One shader file with switchable blend mode via specialization
  constant or per-pipeline variant. Keep simple: 4 pipelines total (Normal /
  Add / Multiply / Screen).
- **Acceptance**: `cargo build` validates.
- **Depends on**: —
- **Estimate**: M.

### T-M5-03 — `Warp::from_config` (vertex + index buffers)

- **Files**: `src/render/warp.rs`
- **Scope**: Build vertex buffer from `WarpMesh.grid` `(rows+1)×(cols+1)`
  control points; build index buffer for a triangle strip across the grid.
  At 1×1 (corner-pin), this is 4 vertices + 2 triangles.
- **Acceptance**: T-M5-08 can edit corners; output follows.
- **Depends on**: —
- **Plan ref**: §3.4 M5 deltas.
- **Estimate**: M.

### T-M5-04 — Warp WGSL with projective UVs

- **Files**: `src/render/shaders/warp.wgsl` *(new)*
- **Scope**: Vertex stage emits `vec3<f32>` UV with homogeneous coordinate;
  fragment stage samples `texture / w` for perspective-correct projection.
  This is the spec-recommended approach (avoids CPU homography bugs).
- **Acceptance**: Cornering a 4-pt warp produces a perspective-correct
  texture (no affine "stretch" artefact).
- **Depends on**: T-M5-03.
- **Plan ref**: §3.4 M5 risk note.
- **Estimate**: M.

### T-M5-05 — Bake polygon mask → signed-distance texture

- **Files**: `src/render/warp.rs`, `src/render/sdf.rs` *(new)*
- **Scope**: Per **D-03** (SDF, not triangle fan). Three pieces in one
  task; if it grows past ~1.5 days, split via sub-letters (5a/5b/5c):
  1. **CPU baker**: from `WarpMesh.mask_polygon: Vec<[f32; 2]>`, produce a
     one-channel `f32` texture at 256×256 where each texel stores the
     signed distance to the nearest polygon edge (negative inside, positive
     outside). Use a brute-force "min distance to each edge" pass — at this
     resolution, ~65 K texels × N edges is fine on CPU. Sign via a
     point-in-polygon ray cast.
  2. **GPU upload + lifecycle**: hold the SDF as a `wgpu::Texture` of format
     `R32Float` per `Warp`. Re-bake (off the render thread, via
     `crossbeam-channel` like the SVG worker in M3) whenever the polygon
     changes; swap texture handles on completion. Fall back to "no mask"
     while a re-bake is in flight.
  3. **Unit tests** (CPU only, no GPU):
     - `sdf_inside_is_negative`: a point in a known polygon's interior has
       distance < 0.
     - `sdf_on_edge_is_zero`: a point exactly on an edge has |distance|
       < texel_size.
     - `sdf_outside_is_positive`: a point well outside has distance > 0.
- **Acceptance**: All three unit tests pass. T-M5-06 renders the mask
  correctly with smooth feathering.
- **Depends on**: T-M5-03.
- **Plan ref**: D-03; §3.4 M5 deltas.
- **Estimate**: L.

### T-M5-06 — Mask WGSL: sample SDF and feather

- **Files**: `src/render/shaders/mask.wgsl` *(new)*
- **Scope**: Per **D-03**. Shader samples the per-warp SDF texture
  (linear filter), then computes alpha as
  `smoothstep(0.0, feather_px, distance)` (inverted so inside-the-mask is
  opaque). Feather radius is a uniform from `WarpMesh.mask_feather_px`.
  Multiply this alpha into the warp output's alpha channel.
- **Acceptance**: Visual on projector — masked region invisible; edges
  fall off smoothly over the feather distance; setting `mask_polygon = []`
  yields no masking (full output).
- **Depends on**: T-M5-05.
- **Estimate**: M.

### T-M5-07 — Gamma master pass

- **Files**: `src/render/gamma.rs`,
  `src/render/shaders/gamma.wgsl` *(new)*
- **Scope**: Final shader pass applies
  `pow(rgb, 1/gamma) * contrast + brightness`. Driven by `Project.gamma /
  brightness / contrast` (currently in schema).
- **Acceptance**: Sliders in control panel adjust output without reload.
- **Depends on**: —
- **Estimate**: S.

### T-M5-08 — Mapping tab UI (drag corners)

- **Files**: `src/windows/control_panel.rs`
- **Scope**: New egui tab. Show a thumbnail of the output framebuffer with
  draggable handles at each `WarpMesh.grid` control point. Mouse drag
  updates the grid in-place; renderer picks it up next frame.
- **Acceptance**: Drag a corner; projector follows in real time.
- **Depends on**: T-M5-03, T-M4-14.
- **Estimate**: L.

### T-M5-09 — Layer reorder UI

- **Files**: `src/windows/control_panel.rs`
- **Scope**: Drag-to-reorder list of layers. Reordering mutates
  `Project.layers`; compositor reads in order.
- **Acceptance**: Drag layer 2 above layer 1; blend visibly changes.
- **Depends on**: T-M4-15.
- **Estimate**: M.

### T-M5-10 — Scenes tab UI

- **Files**: `src/windows/control_panel.rs`
- **Scope**: List of scenes, slot per slot 1–9. "Save current" button on
  each slot serializes the current `Project` state into `Scene.snapshot`.
  Recall button restores it.
- **Acceptance**: Modify a slider; save scene 1; modify again; recall scene 1
  → state snaps back.
- **Depends on**: T-M5-11.
- **Estimate**: M.

### T-M5-11 — Scene save / recall logic

- **Files**: `src/app.rs`, `src/project/schema.rs`
- **Scope**: Helper:
  `fn snapshot(project: &Project) -> serde_json::Value` and
  `fn restore(project: &mut Project, snap: &serde_json::Value)`.
  Snap must round-trip through `serde_json` losslessly.
- **Acceptance**: Unit test
  `project::tests::scene_snapshot_round_trip` passes.
- **Depends on**: —
- **Estimate**: S.

### T-M5-12 — Hotkeys 1–9 → scene recall

- **Files**: `src/app.rs`
- **Scope**: Wire `ControlEvent::SceneRecall(i)` from `KeyboardSource` to
  `restore(project, &project.scenes[i].snapshot)`.
- **Acceptance**: Keyboard 1–9 swaps scenes instantly. No-op if scene index
  unbound.
- **Depends on**: T-M5-11, T-M4-09.
- **Estimate**: S.

### T-M5-13 — Real homography round-trip test

- **Files**: `src/render/warp.rs::tests`
- **Scope**: Implement a `solve_homography(src: [Vec2;4], dst: [Vec2;4]) ->
  Mat3` (DLT method via `glam::Mat3` plus a 4×4 linear solve, or pull in a
  small helper). Test projects src corners through the matrix; asserts
  per-corner residual `< 1e-4`. Remove the `#[ignore]` annotation added by
  X-05.
- **Acceptance**: `cargo test homography_round_trip` passes with a real
  assertion (no `#[ignore]`).
- **Depends on**: X-05 (annotation).
- **Plan ref**: §3.5 unit-test table.
- **Estimate**: M.

### T-M5-14 — Headless wgpu test infrastructure

- **Files**: `tests/headless_gpu.rs` *(new)*
- **Scope**: Common helper that builds a no-surface `wgpu::Instance`,
  requests an adapter, creates an offscreen RGBA8 texture, runs a closure
  over a `(Device, Queue, TextureView)`, then `copy_texture_to_buffer` and
  returns the bytes. Pixel-compare helper:
  `fn assert_image_matches(got: &[u8], path: &str, tolerance: u8)` reads the
  golden via `image::open`, max-channel diff, asserts ≤ tolerance.
- **Acceptance**: Shipped helper compiles under `--features gpu-tests`;
  consumed by T-M5-15+.
- **Depends on**: —
- **Plan ref**: §3.5.
- **Estimate**: M.

### T-M5-15 — Golden image: color pass

- **Files**: `tests/headless_gpu.rs`,
  `tests/golden/color.png` *(new, generated then committed)*
- **Scope**: Render a known SVG fixture through the color effect with fixed
  parameters (hue=+30°, sat=1.5); compare against `color.png`. Tolerance
  ≤ 2/255.
- **Acceptance**: `cargo test --features gpu-tests color_pass_golden` passes.
- **Depends on**: T-M5-14, T-M4-02.
- **Estimate**: S.

### T-M5-16 — Golden image: blur pass

- **Files**: `tests/headless_gpu.rs`, `tests/golden/blur.png` *(new)*
- **Scope**: Same shape as T-M5-15 but for blur radius=8.
- **Acceptance**: `cargo test --features gpu-tests blur_pass_golden` passes.
- **Depends on**: T-M5-14, T-M4-04.
- **Estimate**: S.

### T-M5-17 — Golden image: corner-pin warp

- **Files**: `tests/headless_gpu.rs`, `tests/golden/warp.png` *(new)*
- **Scope**: 4 corners pinned to a known trapezoid; render a checkerboard
  test pattern; compare.
- **Acceptance**: `cargo test --features gpu-tests warp_pass_golden` passes.
- **Depends on**: T-M5-14, T-M5-04.
- **Estimate**: S.

---

## M6 — Project save/load + autostart + docs

Goal (spec §8 + plan §M6).

### T-M6-01 — `Project::load` real implementation

- **Files**: `src/project/mod.rs`
- **Scope**: Read file → `serde_json::from_str::<Value>` → `migrate(value)?`
  → `serde_json::from_value::<Project>(...)`. On `Io`, return
  `ProjectError::Io { path, source }`.
- **Acceptance**: T-M6-05 passes.
- **Depends on**: —
- **Estimate**: S.

### T-M6-02 — `Project::save` atomic write

- **Files**: `src/project/mod.rs`
- **Scope**: Serialize via `serde_json::to_string_pretty` → write to
  `path.tmp` → `std::fs::rename(tmp, path)`. Atomic on POSIX.
- **Acceptance**: T-M6-05 passes.
- **Depends on**: —
- **Estimate**: S.

### T-M6-03 — `Project::resolve_asset`

- **Files**: `src/project/mod.rs`
- **Scope**: `resolve_asset(&self, project_path: &Path, rel: &Path) ->
  PathBuf`: prefer `self.asset_root` if `Some`; else use
  `project_path.parent()`. Joins with `rel`.
- **Acceptance**: Unit test
  `project::tests::resolve_asset_default_to_project_dir` and
  `resolve_asset_honors_explicit_root` pass.
- **Depends on**: —
- **Estimate**: S.

### T-M6-04 — `--autostart` wiring

- **Files**: `src/app.rs`
- **Scope**: If `cli.autostart && cli.project.is_some()`: load project, open
  output window on `project.output_monitor_index` immediately, skip the
  "press a button to start" gate (if any).
- **Acceptance**: `rmap path/to/proj.rmap.json --autostart` lands on the
  saved monitor without user click.
- **Depends on**: T-M6-01.
- **Estimate**: S.

### T-M6-05 — Test: `project_round_trip`

- **Files**: `src/project/mod.rs::tests` or `tests/project.rs` *(new)*
- **Scope**: Build a non-trivial `Project` (one layer, one warp, one scene),
  save to a temp file, load back, assert equal (via `PartialEq` derive or
  `serde_json::to_value` equality).
- **Acceptance**: `cargo test project_round_trip` passes.
- **Depends on**: T-M6-01, T-M6-02.
- **Estimate**: S.

### T-M6-06 — Test: `project_v0_migrate`

- **Files**: `src/project/migrate.rs::tests`
- **Scope**: Construct a JSON `Value` lacking `schema_version`; call
  `migrate`; assert result deserializes as `Project` with
  `schema_version == 1`.
- **Acceptance**: `cargo test project_v0_migrate` passes.
- **Depends on**: —
- **Estimate**: S.

### T-M6-07 — Show-day operator checklist

- **Files**: `docs/show-day-checklist.md` *(new)*
- **Scope**: Operator-facing pre-show checklist. Include: enable Do Not
  Disturb, disable Hot Corners, disable Mission Control gestures, disable
  Energy Saver display sleep on the show machine, lock-screen behaviour
  during long ceremonies, projector firmware/input verification, USB-C/HDMI
  adapter sanity, the `pmset -g assertions` recipe to verify display-sleep
  prevention is actually held.
- **Acceptance**: One page or less; reads top to bottom in 5 minutes.
- **Depends on**: —
- **Plan ref**: §3.6 row "`docs/show-day-checklist.md`".
- **Estimate**: S.

---

## M7 — Polish (sketch)

Open-ended; sequence as needed. Each item below is a future task header, not
a contract.

### T-M7-01 — Multi-cell mesh warp

- **Files**: `src/render/warp.rs`,
  `src/windows/control_panel.rs`
- **Scope**: Allow `WarpMesh.rows / cols > 1`; UI grows draggable handles
  per interior point. Geometry is already a `Vec<Vec<[f32;2]>>`; only counts
  change. Plan ref §3.4 M7.

### T-M7-02 — Multiple independent warps

- **Files**: `src/app.rs`, `src/render/warp.rs`
- **Scope**: Loop over `Project.warps` in the warp pass; each gets a slice
  of the composited texture per `WarpMesh.source_rect`.

### T-M7-03 — Audio modulator (cpal + rustfft)

- **Files**: `src/modulators/mod.rs`, new `src/modulators/audio.rs`
- **Scope**: Behind `audio` cargo feature. Uncomment `Modulator::Audio`;
  add `AudioProvider` trait + `cpal` impl; FFT band extraction via
  `rustfft`. See plan §3.2 modulator extension point.

### T-M7-04 — Crossfade between scenes

- **Files**: `src/app.rs`, `src/project/schema.rs`
- **Scope**: Interpolate snapshots over a configurable duration.
  Snapshot diff strategy: per-numeric-field linear interp; categorical
  (blend modes) snap at midpoint.

### T-M7-05 — MIDI input

- **Files**: `src/controls/midi.rs` *(new)*
- **Scope**: Behind `midi` feature. `MidiSource` impl of `Source` trait
  using `midir`. Plan §3.2.

### T-M7-06 — OSC input

- **Files**: `src/controls/osc.rs` *(new)*
- **Scope**: Behind `osc` feature. `OscSource` impl using `rosc`.

### T-M7-07 — `ExternalPass` registry

- **Files**: `src/effects/mod.rs`, `src/effects/registry.rs` *(new)*
- **Scope**: Add `Effect::External { id, params }` variant; introduce
  registry per plan §3.2 migration steps. v2 schema bump in
  `src/project/migrate.rs`.

### T-M7-08 — Effect preset bundles

- **Files**: `assets/presets/*.json` *(new)*,
  `src/windows/control_panel.rs`
- **Scope**: Curated parameter bundles ("candle flicker", "soft pulse").
  UI loads from `assets/presets/`.

---

## Appendix A — Dependency graph (text)

Top-of-tree dependencies (pre-conditions). Within a milestone, sub-tasks
follow the order listed above unless the body says otherwise.

```
M1   ── T-M1-01 ─┐
                  ├─→ T-M1-02 ─→ T-M1-03 ─→ T-M1-04 ─┬─→ T-M1-05
                  │                                    └─→ T-M1-06
                  │
M1.5 ─────────────┴─→ T-M1.5-01    (gates everything below)
                                    │
M2   ── T-M2-01 (independent)       │
        T-M2-02 ─→ T-M2-03 ─→ T-M2-10 ─→ T-M2-11
        T-M2-04 (independent, macOS)
        T-M2-05, T-M2-06, T-M2-07 ─→ T-M2-08 ─→ T-M2-09
                                    │
M3   ── T-M3-01 ─→ T-M3-02 ─→ T-M3-03 ─┐
        T-M3-04 ─────────────────────────┤
        T-M3-05 ─→ T-M3-07               ├─→ T-M3-06
                                          │
M4   ── T-M4-14 (multi-window per D-01)  │
        T-M4-01 ─→ T-M4-02, T-M4-04, T-M4-05 ─→ T-M4-06
        T-M4-07 ─→ T-M4-13
        T-M4-08 ─→ T-M4-09 ─→ T-M4-10
        T-M4-11, T-M4-12 (independent)
        T-M4-14 + T-M4-06 + T-M4-07 ─→ T-M4-15
                                          │
M5   ── T-M5-01 + T-M5-02 (parallel)     │
        T-M5-03 ─→ T-M5-04, T-M5-05 ─→ T-M5-06
        T-M5-07 (independent)
        T-M5-11 ─→ T-M5-10 ─→ T-M5-12
        T-M5-03 + T-M4-14 ─→ T-M5-08
        T-M4-15 ─→ T-M5-09
        T-M5-13 (independent)
        T-M5-14 ─→ T-M5-15, T-M5-16, T-M5-17
                                          │
M6   ── T-M6-01 ─→ T-M6-04, T-M6-05      │
        T-M6-02 ─→ T-M6-05
        T-M6-03 (independent)
        T-M6-06, T-M6-07 (independent)
                                          │
M7   ── any of T-M7-01..08, ordered by current need
```

---

## Appendix B — Estimate roll-up

| Milestone | Tasks | S | M | L | Total est. |
|---|---:|---:|---:|---:|---:|
| Cross-cutting | 6 | 5 | 1 | 0 | ~1 day |
| M1 | 6 | 2 | 4 | 0 | 2 days |
| M1.5 | 1 | 1 | 0 | 0 | ½ day cal. |
| M2 | 11 | 6 | 5 | 0 | 3 days |
| M3 | 7 | 1 | 6 | 0 | 4 days |
| M4 | 15 | 6 | 7 | 2 | 6 days |
| M5 | 17 | 5 | 8 | 4 | 8 days |
| M6 | 7 | 7 | 0 | 0 | 1.5 days |
| **v1 total (excluding M7)** | **69** | **32** | **31** | **6** | **~26 days** |
| M7 sketch | 8 | — | — | — | open |

Cross-checks against plan §3.4 milestone table: aligned within ½ day per
milestone, except **M5 is +1 day** vs the plan due to **D-03** (SDF mask
geometry). Variance otherwise comes from cross-cutting (X) tasks not
attributed to a milestone in the plan.

---

*End of tasks. Cross-references: spec → `001-initial-setup.md`,
plan → `001-initial-setup-plan.md`, this file → who picks up which work.*
