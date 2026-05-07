# Spec: rmap — A Minimal Projection Mapping Tool (v1.1, Rust)

> **Note on naming**: previous revisions of this spec called the tool "PyMap" and targeted Python. v1.1 retargets to Rust and renames the binary to `rmap`. Architecture and capabilities are unchanged in spirit; the runtime, libraries, and ergonomics are not.

## Goal

A standalone Rust application that loads SVG content, applies basic visual effects, lets the user warp/map the output to physical surfaces, and projects the result fullscreen to a selected monitor (the beamer). Designed as a lightweight alternative to TouchDesigner for simple projection mapping scenarios — wedding-scale, single-projector, single-machine.

Distributed as a single static `rmap` binary (or `.app` bundle on macOS), no runtime to install on the show-day machine.

## Why this exists

Two trade-offs to be honest about, not one.

**Build vs. buy.** Commercial tools already solve wedding-scale projection mapping: MadMapper Express (~$99), HeavyM, MapMap (free, GTK-based, open source). Building this is an explicit choice. The justification needs to be one of:

- **A tool I trust because I wrote it** — show-day reliability comes from understanding every code path.
- **Customization** — bespoke effects, integrations, or content pipelines that no off-the-shelf tool exposes.
- **A learning project** — projection mapping + GPU shaders + cross-platform display handling is a meaty engineering surface.

**Rust vs. Python.** A previous revision of this spec targeted Python. Retargeting to Rust costs roughly 2× upfront velocity (compile times, ceremony, learning wgpu/WGSL if new) but pays back on the things that matter for live use:

- **Single static binary** — no runtime, no `pip`, no "wheels missing for Python 3.14" failure mode on the wedding-day laptop.
- **No GC pauses** — frame timing is predictable; nothing stutters because some background allocation triggered a major collection.
- **Memory safety in unsafe situations** — show-day, sleep-deprived, plugging/unplugging projectors, opening/closing the lid: the class of crashes Rust prevents (use-after-free in GPU resource handling, data races on hot-reload threads) is exactly the class that bites at venues.
- **`wgpu`** is arguably the cleanest cross-platform GPU abstraction available today — Metal on macOS, Vulkan on Linux, DX12 on Windows, WebGPU in browsers. One shader pipeline, four target platforms.

If the answer to "build vs. buy" becomes "I just need it to work for one wedding," buy MadMapper. If "build" stands but Rust feels heavy, the Python version of this spec is in `git log`.

## Non-goals

- Not a TouchDesigner replacement: no node-graph, no 3D scenes, no live coding environment.
- Not a multi-projector tool: no edge blending, no multi-output sync.
- Not for low-latency live VJ performance with audio-rate input.
- No advanced color calibration, no automatic projector calibration.
- No web target in v1, even though `wgpu` would allow it. Adds complexity for no wedding-day benefit.

---

## Core capabilities

### 1. SVG loading

- Load one or more SVG files from disk.
- Parse vector content (paths, shapes, fills, strokes) and rasterize on demand at the output resolution via `resvg`.
- Hot-reload: if the SVG file changes on disk, re-render automatically (debounced — designers' save operations fire multiple FS events).
- Multiple SVG layers, toggleable, reorderable, individually transformable.
- SVG paths stored relative to the project file so `wedding.rmap.json + assets/` is portable on a USB stick.

### 2. Effects pipeline

A small, composable chain of effects applied per layer or globally:

- **Color**: hue shift, saturation, brightness, contrast.
- **Tint/recolor**: replace fill color or apply a global color overlay.
- **Blur**: gaussian blur with adjustable radius (separable: horizontal pass + vertical pass).
- **Rotation / scale / translate**: per-layer 2D transform.
- **Opacity / blend mode**: normal, add, multiply, screen.
- **Background color**: solid or transparent.
- **Gamma master** (global, post-warp): projectors run dark; venues run dark; SVG colors look washed out without a gamma/brightness/contrast pass on the final composite.

Effects are applied in a fixed order (transform → color → blur → blend) for v1. Per-layer parameters live in a config struct and are live-editable. Implementation uses **ping-pong textures** (two output textures swapped per pass) — *not* one texture per effect — to keep VRAM bounded.

Effects are modeled as a Rust `enum`, not trait objects. Exhaustiveness-checking on `match` catches "you added a variant but forgot the renderer" at compile time, which a `Box<dyn Effect>` design wouldn't.

### 3. Modulators

Any numeric effect parameter can optionally be driven by a modulator instead of held static:

```rust
pub enum Modulator {
    Static(f32),
    Sine    { period_s: f32, amp: f32, phase: f32, offset: f32 },
    Triangle{ period_s: f32, amp: f32, offset: f32 },
    Noise   { period_s: f32, amp: f32, offset: f32 },
    Bpm     { divisor: f32, amp: f32, offset: f32 },
    // Reserved for v1.5: Audio { band: u8, smoothing: f32, amp: f32, offset: f32 }
}

impl Modulator {
    pub fn value(&self, clock: &Clock) -> f32 { /* ... */ }
}
```

Modulators are a v1 architecture component, not a v2 addition. Without them, the tool is a screensaver. With them, every parameter is alive.

### 4. Projection mapping (warp)

- The composited output is mapped onto the projector via one or more **warp meshes**.
- v1 default: a single 1×1 mesh (i.e. a corner-pin quad with four draggable corners).
- v1.5: multi-cell meshes (e.g. 5×5) for curved walls, archways, tablecloths.
- v1.5: multiple independent warps, each receiving a slice of the source content.
- **Per-warp polygon mask** with edge feathering (Gaussian falloff, 0–N px) — even single-projector shows need to mask off ceilings, foregrounds, and edges.
- Mapping data persisted to the project JSON.
- Calibration UI: drag corners (or grid points) with the mouse on the control window while the projector shows the live result.

**Implementation**: render the warp mesh as a triangle strip in `wgpu`, with per-vertex source UVs supplied as `vec3<f32>` carrying the homogeneous coordinate. The rasterizer does the perspective division for free. Identical visual result to a CPU-side homography, fewer matrix-convention bugs, and extends naturally to mesh subdivision. A unit test still validates a 4-point homography on the math path because `glam::Mat3` is the fallback for any geometry that needs a pre-computed matrix.

### 5. Output to selected monitor

- Enumerate connected displays via `winit::event_loop::ActiveEventLoop::available_monitors()`.
- User picks the projector display from a list (by index, name, or position).
- Open a borderless fullscreen window on that display (`Fullscreen::Borderless(Some(monitor))`) showing the final mapped output.
- A separate `egui` control window stays on the primary display for editing.
- Toggle output on/off without quitting (drop and recreate the wgpu surface).
- **Cursor hidden** on the output window at all times (`Window::set_cursor_visible(false)`).

### 6. Show-day requirements

These are required for v1, not "polish".

- **Blackout (`B` key)**: instantly outputs solid black, keeps state and clock running. Standard VJ practice.
- **Freeze (`F` key)**: locks the current frame on the projector while you edit safely on the control window.
- **Test patterns**: built-in calibration sources, independent of any SVG layer:
  - 50-px grid
  - Crosshair + corner markers
  - White at 100% / 50% / 25%
  - Color bars
- **Error overlay on the *control* window** (never the output) when an SVG fails to load. Don't crash; render a placeholder.
- **Logging** via `tracing` + `tracing-appender`, daily-rotated to `~/Library/Logs/rmap/`. Capture monitor selection, GPU adapter, surface configuration, shader compile diagnostics, hot-reload events, and all errors. When something breaks at a venue you have minutes, not hours.
- **Panic restore**: catch panics on the render thread (`std::panic::catch_unwind`), log them, recreate the surface and continue. Don't take the show down because of a malformed SVG.
- **Display-sleep prevention**: hold an `IOPMAssertionCreateWithName` assertion via `objc2-io-kit` for the lifetime of the output window. Document the System Settings checklist as a fallback.

### 7. Live input

v1 in-scope:

- **Tap tempo (`Space`)**: feeds a global BPM that `Modulator::Bpm` syncs to.
- **Scene recall hotkeys (`1`–`9`)**: instantly switches to one of N saved snapshots of the current parameter state. A scene is a serialized snapshot of the layer/effect/warp/modulator config — no animation, just instant state swap. (Crossfade is M7 polish.)

v1 out-of-scope but architecturally reserved:

- MIDI/OSC input. Reserve a `controls` module and a `Param::bind(source)` API so adding a USB foot pedal or Launchpad later is a 1-day job, not a refactor. Likely crates: `midir` for MIDI, `rosc` for OSC.

### 8. Configuration & persistence

- Project state stored as a single JSON file: SVG paths (relative to project root), layer settings, effect parameters, modulators, scenes, mapping mesh + masks, output monitor choice.
- `serde` derive on every config struct. Schema is **versioned from day one** (`schema_version: u32`) with a `migrate(json: Value) -> Result<Project>` function — even if v1 is the only version, the migration entry point exists.
- Save / Load / "Save As" from the control window.
- CLI flag (`clap`) to launch directly into a saved project — `rmap wedding.rmap.json --autostart` is the show-day entry point.

---

## Technical design

### Language and runtime

- **Rust 2024 edition**, MSRV pinned to a recent stable (recommend `rust-toolchain.toml` with the latest stable + 1).
- Cross-platform target: macOS first (matches the dev machine), Linux secondary (Wayland + X11 via wgpu/Vulkan), Windows best-effort.
- `cargo` for everything; `cargo-watch` for dev iteration to dampen the compile-time hit.
- `rustfmt` + `clippy -- -D warnings` in CI from day one.

### Recommended crates

| Concern | Choice | Rationale |
|---|---|---|
| Windowing & input | **`winit`** (≥ 0.30 with `ApplicationHandler` API) | The de facto cross-platform window/input crate. Integrates natively with wgpu. Owns the event loop |
| GPU API | **`wgpu`** | Modern cross-platform GPU abstraction (Metal/Vulkan/DX12/WebGPU). The same stack Bevy and many production tools ship on. WGSL shaders, validated at compile time when possible |
| Shading language | **WGSL** | Native to wgpu, validated, no GLSL→SPIR-V translation hop. Use `naga` for offline shader validation in tests |
| SVG rasterization | **`resvg`** (with `usvg` parser, `tiny-skia` backend) | Gold-standard pure-Rust SVG2 renderer. Handles gradients, filters, masks. No native deps. Fast |
| Image handling | **`image`** + raw `Vec<u8>` for GPU uploads | `image` for golden-image tests; raw buffers for the fast path |
| Control UI | **`egui`** + **`egui-wgpu`** + **`egui-winit`** | Immediate-mode UI native to Rust. Renders into the same wgpu device. Best-in-class ergonomics for graphics tools. (Equivalent of Dear ImGui in the Python spec.) |
| Math | **`glam`** | The standard for game/graphics math in Rust — `Vec2`, `Mat3`, `Mat4`, SIMD-friendly. Lighter than `nalgebra`, sufficient for our needs |
| Hot reload | **`notify`** (≥ 6) with a **250 ms debouncer** (`notify-debouncer-full`) | Standard file-watching crate; debouncing handles Illustrator's multi-event saves |
| File watching threading | `std::thread` + `std::sync::mpsc` channel | No async runtime needed; one watcher thread feeds events to the renderer |
| Background SVG rasterization | `std::thread::spawn` + `crossbeam-channel` | Off-main-thread rasterization; texture handle swap when ready. Avoids any async runtime |
| CLI | **`clap`** with derive feature | Universal |
| Logging | **`tracing`** + **`tracing-subscriber`** + **`tracing-appender`** (daily rolling to `~/Library/Logs/rmap/`) | Modern Rust observability. Structured spans help diagnose "what was the renderer doing when X happened" |
| Errors | **`thiserror`** for typed errors at module boundaries; **`anyhow`** at the application top level | Standard Rust error split. Keep `Result<T, RmapError>` in libraries, `Result<T>` (anyhow alias) in `main` |
| Serialization | **`serde`** + **`serde_json`** for the project file; consider **`serde_json::Value`** for forward-compatible field reads in the migrator | Standard |
| Display-sleep prevention (macOS) | **`objc2-io-kit`** to call `IOPMAssertionCreateWithName`; fallback shell out to `caffeinate -d` | The `objc2` family is the modern way to call Apple APIs from Rust; older `cocoa`/`objc` crates are deprecated |
| Monitor metadata (macOS extras) | **`objc2-app-kit`** for display name resolution if `winit`'s `MonitorHandle::name()` is insufficient | macOS sometimes returns `None` for monitor names depending on the configuration |
| macOS app bundling | **`cargo-bundle`** or **`cargo-packager`** | Produces a `.app` for show-day distribution. `cargo-packager` is newer and supports notarization workflows |

Crates explicitly **rejected**:

- **`bevy`** — full ECS game engine. Massive, slow to compile, dictates architecture. Overkill for a single-window tool. (We borrow ideas from its renderer; we don't depend on it.)
- **`nannou`** — creative-coding framework. Lovely for sketches but its abstraction layer fights you when you need direct wgpu control of the pipeline.
- **`iced`** — beautiful Elm-style UI but retained-mode and async-first; egui is the right tool for a graphics-editor sidebar.
- **`tokio`** — no async I/O of consequence here. Adds a runtime for nothing.
- **`tauri`** — webview-based UI; hostile to native GPU compositing and an order of magnitude more dependency surface than egui.

### Architecture

Single binary crate (no workspace in v1; a workspace can be split out at v1.5 if `core` and `cli` diverge).

```
src/
├── main.rs                  # entry, clap, tracing setup, App::run()
├── app.rs                   # winit ApplicationHandler, ties output + control windows
├── project/
│   ├── mod.rs               # Project struct, save/load, migration entry
│   ├── schema.rs            # All serde-derived config structs (versioned)
│   └── migrate.rs           # schema_version migration registry
├── render/
│   ├── mod.rs               # wgpu device/queue/surface lifecycle
│   ├── pipeline.rs          # ping-pong FBO chain for one layer's effect chain
│   ├── compositor.rs        # blends N layer textures into a single output texture
│   ├── warp.rs              # mesh warp (1×1 = corner-pin), per-warp masking, render(src, dst)
│   ├── gamma.rs             # final master pass
│   └── shaders/             # WGSL files, included via include_str!
├── effects/
│   ├── mod.rs               # Effect enum + EffectKind discriminator
│   ├── color.rs
│   ├── blur.rs              # separable gaussian
│   └── transform.rs
├── modulators/
│   ├── mod.rs               # Modulator enum + value(clock) -> f32
│   └── waveforms.rs         # sine/triangle/noise/bpm impls
├── svg_layer.rs             # resvg integration, off-thread rasterization, hot reload
├── windows/
│   ├── output.rs            # borderless fullscreen, cursor hidden, blackout/freeze, panic restore
│   └── control.rs           # egui UI for layers/effects/modulators/mapping/scenes
├── monitors.rs              # winit-based enumeration + macOS name fallback
├── controls/                # reserved for v1.5 MIDI/OSC; v1 holds tap-tempo + scene hotkeys
│   └── mod.rs
├── clock.rs                 # owns Instant-based time, BPM, tap-tempo state
├── show_day/
│   ├── mod.rs               # blackout/freeze state, panic restore
│   ├── sleep_assertion.rs   # objc2-io-kit IOPMAssertion
│   └── checklist.md         # render this in the docs/ output
├── test_patterns.rs         # grid, crosshair, white levels, color bars
└── error.rs                 # thiserror RmapError
```

### Render pipeline (per frame)

1. `clock.tick()` — emits `t: Duration`, current BPM, tap-tempo phase. All modulators read from `clock`; nothing else reads wall time. (Makes deterministic testing possible.)
2. For each enabled SVG layer, get the cached rasterized texture. Re-rasterize *off-thread* only if the source SVG changed on disk OR effective on-screen size crossed an oversampling threshold. Swap the texture handle when the worker thread reports ready.
3. Apply per-layer effects via `render::pipeline` using two ping-pong textures.
4. Composite all layer textures into a single output texture using each layer's blend mode and opacity.
5. Apply gamma master pass.
6. Apply each warp: render the output texture onto the user-defined mesh into the final surface texture; multiply by the per-warp mask in the same fragment shader.
7. If `freeze` is active, skip steps 1–6 (present the previous final texture).
8. If `blackout` is active, clear the surface to black before present.
9. `surface.present()` on the output window.
10. `egui` paints the control window in its own pass on its own surface.

Target: 60 fps at 1920×1080 on a modern laptop GPU. wgpu's frame timing is more predictable than OpenGL's, and Rust's lack of GC means no surprise pauses.

### Data model

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Transform2D {
    pub translate: [f32; 2],
    pub rotate_deg: f32,
    pub scale: [f32; 2],
    pub anchor: [f32; 2],
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Add,
    Multiply,
    Screen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConfig {
    pub id: String,
    pub svg_path: PathBuf,                  // relative to Project::asset_root
    pub enabled: bool,
    pub transform: Transform2D,
    pub effects: Vec<Effect>,
    pub blend_mode: BlendMode,
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpMesh {
    pub rows: u32,                          // 1 = corner-pin quad
    pub cols: u32,
    pub grid: Vec<Vec<[f32; 2]>>,           // (rows+1) × (cols+1) control points in output px
    pub source_rect: [f32; 4],              // x, y, w, h in composited texture
    #[serde(default)]
    pub mask_polygon: Vec<[f32; 2]>,        // empty = no mask
    #[serde(default)]
    pub mask_feather_px: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub snapshot: serde_json::Value,        // opaque snapshot of mutable runtime state
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,                // start at 1
    #[serde(default)]
    pub layers: Vec<LayerConfig>,
    #[serde(default)]
    pub warps: Vec<WarpMesh>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub output_monitor_index: usize,
    #[serde(default)]
    pub output_resolution: Option<(u32, u32)>, // None = native; overriding is a footgun
    #[serde(default = "default_bg")]
    pub background_color: [f32; 4],
    #[serde(default)]
    pub asset_root: Option<PathBuf>,
    #[serde(default = "default_one")]  pub gamma: f32,
    #[serde(default)]                  pub brightness: f32,
    #[serde(default = "default_one")]  pub contrast: f32,
}

fn default_bg() -> [f32; 4] { [0.0, 0.0, 0.0, 1.0] }
fn default_one() -> f32 { 1.0 }
```

`#[serde(default)]` on every optional field is deliberate: a v1.0 project file that lacks `mask_feather_px` should still load, not error.

### Show-day environment hardening (macOS)

Beyond the "Displays have separate Spaces" issue, macOS has additional traps:

- **Display sleep / Energy Saver**: hold an `IOPMAssertion` for the lifetime of the output window via `objc2-io-kit`; document the System Settings fallback.
- **App Nap**: file an `NSProcessInfo` activity assertion via `objc2-foundation`.
- **Hot Corners / Mission Control gestures** can yank the app off the projector mid-ceremony — disable on the show-day machine.
- **Notification Center** on the primary display will route to the projector if accidentally dragged over. Use Do Not Disturb on show day.

A short `docs/show-day-checklist.md` is part of M6.

### Concurrency model

- **Main thread** owns winit's event loop, the wgpu surface, and rendering. Single-threaded by design (winit on macOS *requires* this).
- **One file-watcher thread** owned by `notify`, sending debounced FS events into a `std::sync::mpsc` channel.
- **A small `std::thread` pool** (or `std::thread::spawn` per job) for SVG rasterization; results sent over `crossbeam-channel`.
- **No async runtime.** No `tokio`. Adding one buys nothing here and adds compile time + dependency mass.
- Shared mutable state lives in `App` on the main thread; threads communicate by message-passing, not by `Arc<Mutex<...>>`.

### Error handling

- `thiserror` derives at module boundaries: `RmapError`, `ProjectError`, `RenderError`, etc., each with structured variants.
- `anyhow::Result` only at the `main()` boundary, where context (file path, monitor index) is added before logging.
- **Renderer panics are caught** with `std::panic::catch_unwind` around the per-frame render call; on panic, log + show error overlay on control window + reset the GPU surface. Do not propagate.

---

## User flow

1. Launch `rmap`. Control window opens on primary display.
2. User picks the projector from a "Display" dropdown. Output window opens borderless fullscreen on that monitor; cursor hidden.
3. User toggles the **calibration test pattern** (grid + crosshair) and adjusts the warp corners until the grid aligns with the physical wall feature.
4. User adds an SVG layer via "Add Layer" → file picker.
5. SVG appears on the projector. User adjusts transform, effects, and modulators in the control window; output updates live.
6. User saves snapshots as **scenes** for the ceremony, dinner, and party — recallable via `1`/`2`/`3`.
7. User saves the project as `wedding.rmap.json`. Assets stored next to it.
8. On the day: `rmap wedding.rmap.json --autostart` opens directly into the saved state, on the saved monitor, with display-sleep prevention active.

---

## Milestones

### M1 — Hello rectangle (2–3 days, ~1 day longer than Python equivalent due to wgpu setup)

- `winit` borderless fullscreen window opens on a chosen monitor.
- `wgpu` device/queue/surface initialized; renders a solid colored quad via a trivial WGSL shader.
- Esc closes cleanly; surface released, no GPU validation errors on shutdown.
- **Keep this minimal.** On macOS with separate Spaces, *just this* is the project's biggest schedule risk. Don't pile features on.

### M1.5 — Venue dry-run (½ day, calendar-blocking)

- Take the M1 binary to the actual wedding venue (or any room with a real HDMI projector).
- Verify: borderless fullscreen lands on the projector, no display sleep, no Mission Control glitches, Esc closes cleanly, no flicker on extended-display reconfiguration.
- **If M1.5 fails, the entire project is at risk.** Find out in week 1, not week 6.

### M2 — Calibration tooling (2–3 days)

- Test patterns (grid, crosshair, white levels, color bars) selectable from control window + hotkey.
- Blackout (`B`) and Freeze (`F`).
- Display-sleep prevention via `objc2-io-kit`.
- `tracing` logging to file.
- Panic restore around the renderer.

### M3 — SVG on screen (3–4 days)

- Load an SVG, rasterize via `resvg`.
- Display as a textured quad on the output.
- Hot-reload via `notify` with 250 ms debounce; rasterization on a worker thread; texture-handle swap on completion.

### M4 — Effects + modulators (5–7 days)

- Color, blur, opacity, transform as ping-pong WGSL passes.
- Modulator enum (`Static`, `Sine`, `Triangle`, `Noise`, `Bpm`).
- Tap tempo (`Space`) feeding the `Bpm` modulator.
- `egui` control panel with sliders bound to layer/effect/modulator parameters.

### M5 — Multiple layers + scenes + warp + masking (6–8 days)

- Multi-layer compositor with blend modes.
- Layer reordering in UI.
- Single warp mesh (default 1×1 = corner-pin) with drag-to-edit corners.
- Per-warp polygon mask with feathering.
- Gamma master pass.
- Scene snapshots with hotkeys `1`–`9`.
- Save/load mapping + scenes to project JSON.

### M6 — Project save/load + autostart CLI + show-day docs (1–2 days)

- Full Project JSON round-trip with `schema_version` and `asset_root`.
- `--autostart` CLI flag via `clap`.
- `docs/show-day-checklist.md`.

### M7 — Polish (ongoing)

- Multi-cell warp mesh (5×5 etc.) for curved surfaces.
- Multiple independent warps.
- Audio-reactivity modulator (`Modulator::Audio { ... }`) using `cpal` for capture.
- Crossfade between scenes.
- MIDI/OSC input via `controls/` module (`midir`, `rosc`).
- Better error handling for malformed SVGs.
- Preset effect bundles ("candle flicker", "soft pulse").

> Estimates run ~30–50% longer than the Python version of the same milestones, accounting for compile-iteration overhead and wgpu/WGSL ramp time. The Python spec's M1 was 1–2 days; the Rust M1 is 2–3.

---

## Build & distribution

- **Development**: `cargo run`. Use `cargo-watch -x "run -- wedding.rmap.json"` for hot-restart on save during M3+.
- **Show-day build profile**: a custom `[profile.release-show]` inheriting `release` with `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`. Slower compile, faster cold-start, smaller binary, no panic-unwind machinery (panic still goes through `catch_unwind` for renderer recovery before the abort path).
- **macOS .app bundle** via `cargo-bundle` or `cargo-packager`. Embeds the binary, an `Info.plist` with high-DPI hints, and an icon.
- **Code signing** is not required for personal use, but document the macOS Gatekeeper "right-click → Open" workaround for first launch. If wider distribution is ever wanted, `cargo-packager` supports the notarization flow.
- The wedding-day machine runs the bundled `.app` (or the static binary directly), not a `cargo run` against a dev checkout.
- **Cross-compilation**: not in v1. Build natively on the show-day machine architecture (Apple Silicon).

---

## Testing

- **Unit tests** (`cargo test`):
  - 4-point homography solve via `glam::Mat3` (input/output point pairs → 3×3 → re-project corners; assert near-zero residual).
  - Modulator value() at known `t` for each variant (sine peaks at quarter-period, etc.).
  - Project save/load round-trip with `schema_version` migration.
  - Hot-reload event coalescing (fake `notify` events through the debouncer).
- **Headless GPU tests** (`cargo test --features gpu-tests`):
  - Standalone `wgpu::Instance` + offscreen texture; render the blur kernel, color shift, and corner-pin warp; copy texture back to `Vec<u8>`; pixel-compare against golden PNGs in `tests/golden/` using the `image` crate. Tolerance per channel for cross-driver wobble.
- **WGSL validation** in `build.rs`: every shader compiled offline with `naga` so build fails if a shader is broken (faster signal than runtime).
- **Clippy + rustfmt** enforced in CI as compile errors, not warnings.

One day of testing setup; saves ten hours of mystery debugging at the venue.

---

## Open questions

These need answers before or during M3 — they don't block M1/M1.5/M2:

1. **Build vs. buy** is settled by the "Why this exists" section above — but worth re-confirming honestly: is the answer still "build", or has scope drift made MadMapper Express the better call?
2. **Rust vs. Python** is settled, but if the compile-time hit slows iteration enough to threaten the wedding date, the Python branch is in `git log` and is shorter. Re-evaluate at end of M3 if velocity feels wrong.
3. **One projector, definitely?** Multi-projector requires a different output architecture (sync, edge blending). v1 is single-output; switching later is a near-rewrite.
4. **SVG-only?** Or PNG / MP4 / HAP video too? Video adds `ffmpeg-next` (FFI to libav) and uploads to GPU textures via `wgpu::Queue::write_texture`. Doable but a real chunk of work.
5. **Audio reactivity, really never?** A microphone-driven brightness modulator is a 1–2 day add (`cpal` for capture, FFT via `rustfft`, feed into `Modulator::Audio`). Dramatically lifts the result. Hooks already in v1.
6. **Designer hand-off**: will anyone else (a designer friend) produce SVGs you'll trust? `resvg` already covers this — it has the best SVG2 support of anything in any language.
7. **Show-day machine**: same MacBook as dev, or a separate one? If separate, the .app bundle target architecture must match — Apple Silicon vs Intel are not interchangeable without explicit cross or universal builds.

---

## Risk notes

- **macOS fullscreen on a second monitor** is historically finicky (the "Displays have separate Spaces" issue we hit with TouchDesigner). M1.5 exists *because* of this. `winit` ≥ 0.30 handles this better than older Rust windowing crates but has its own quirks.
- **wgpu surface re-creation** when the projector is unplugged/replugged is a real failure mode and needs explicit handling — surface lost / outdated / suboptimal events have to be caught and the surface reconfigured. Bake this into M2.
- **WGSL compile errors at runtime** crash an inexperienced renderer. The `build.rs` `naga` validation catches them at build time; missing that step is a footgun.
- **SVG rasterization at high res** is slow; cache aggressively and rasterize off-thread. Re-rasterize only when the source file changes or effective on-screen size crosses an oversampling threshold (not every frame).
- **Corner-pin / homography math** is easy to get wrong (matrix conventions, row-major vs column-major, source vs destination). Prefer wgpu's perspective-correct rasterization; validate the matrix path with a unit test.
- **Compile-time tax** on iteration: incremental builds should stay sub-5s; if a new dependency (looking at you, `bevy`) blows that budget, drop it. `cargo-watch` and a small `Cargo.toml` are the levers.
- **Modulator clock drift**: all time-based modulators must read from the central `clock` module, not `Instant::now()` directly, or freeze/blackout/scene-recall will desync them.
- **macOS objc2 bindings** are verbose but stable; the older `cocoa`/`objc` crates are deprecated and should not be added "just because the example online uses them".
