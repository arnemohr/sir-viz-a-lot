# Implementation Plan: rmap (v1.1)

> Companion to `specs/001-initial-setup.md`. The spec is the *what*; this plan
> is the *how*, plus an audit of what the existing scaffold already does and
> where it must change.
>
> Audience: a teammate who has not seen the spec or the scaffold. Read this
> file top-to-bottom, then `make help`. You should be able to start work
> tomorrow.

---

## PHASE 1 — Audit findings

### Verification status (post-Phase-2)

| Command | Result |
|---|---|
| `cargo check --all-targets` | **pass**; `Finished dev profile … in 0.63s` |
| `cargo clippy --all-targets --all-features -- -D warnings` | **pass**; no warnings |
| `cargo test` | **pass**; `4 passed; 0 failed; 0 ignored` |
| `cargo test --features gpu-tests` | **pass**; same 4 tests; `gpu-tests` feature enables nothing yet |
| `cargo fmt --all --check` | **pass** |

The scaffold compiles and lints cleanly. There are no GPU-bound tests behind
`gpu-tests` yet — only the feature flag exists. Build wall time after a clean
checkout is dominated by the first wgpu/egui/resvg compile (~28 s on warm
crates.io cache, much longer cold).

### Drift from spec

Compared against `specs/001-initial-setup.md` line-by-line:

- `Modulator::value()` was a dead stub before Phase 2: it ignored `Clock` for
  every variant except `Static`, returning `0.0`. Spec §3 mandates that *every*
  variant reads from the central clock. **Fixed in Phase 2.**
- `waveforms` only exposed `sine` and `triangle`; `noise` and `bpm` were
  unimplemented despite being declared on the `Modulator` enum. Spec §3 lists
  all four as v1. **`noise` added in Phase 2; `bpm` dispatches via `sine` on
  the beat-period, also Phase 2.**
- `render::Renderer::render_frame()` has no `catch_unwind` and no TODO marker
  for it. Spec §6 (Show-day requirements) and §"Error handling" both call for
  per-frame panic isolation. **Not fixed in Phase 2** — exceeds the 30-line
  budget; deferred to **M2** (see milestone breakdown).
- `windows::output::OutputState` exposes `toggle_blackout` / `toggle_freeze`
  but no key handler is wired anywhere. Spec §6 names the keys `B` and `F`.
  Deferred to **M2**.
- No `tracing_appender::rolling::daily` writer. Stderr only. Spec §6 calls for
  `~/Library/Logs/rmap/`. Deferred to **M2** (TODO comment exists in
  `src/main.rs:42`).
- `svg_layer::SvgLayer::load` accepts a path and returns `Self` without
  parsing the SVG. Stub only. Spec §1. Deferred to **M3**.
- `monitors::list()` returns `Vec::new()`. Stub only. Spec §5. Deferred to
  **M1**.
- `tests/` directory is empty. Spec §"Testing" calls for headless GPU
  golden-image tests behind the `gpu-tests` feature. Deferred to **M5**
  (golden tests follow once there is something visual to pin).
- `docs/show-day-checklist.md` does not exist. Spec §M6 deliverable.
  Deferred to **M6**.

Things the spec mandates that **are** present and correct in the scaffold:

- `Effect` is a closed enum (`src/effects/mod.rs:11`). No trait objects.
- `Modulator` is a closed enum (`src/modulators/mod.rs:10`) with the
  `Audio` variant reserved as a comment (`src/modulators/mod.rs:34`).
- `Project::schema_version` field present, default 1
  (`src/project/schema.rs:62`).
- `#[serde(default)]` on every optional `Project`/`WarpMesh` field
  (`src/project/schema.rs:42-87`).
- `migrate(value) -> Result<Value>` entry point exists
  (`src/project/migrate.rs:9`).
- `Clock` is the only place that reads `Instant::now()`
  (`src/clock.rs:9, 27`); `waveforms` takes `t_s: f32` as a parameter.
- `pollster` is the async-bridge crate; no `tokio`, no async runtime.
- No rejected crates in `Cargo.toml` (no `bevy`, `nannou`, `iced`, `tokio`,
  `tauri`).
- `build.rs` performs real `naga` parse + validate on every file under
  `src/render/shaders/` (`build.rs:9-31`).
- `objc2-*` deps gated by `cfg(target_os = "macos")`
  (`Cargo.toml:64-69`).

### Quality issues independent of the spec

- `#![allow(dead_code, unused_imports)]` at the crate root
  (`src/main.rs:1-3`). Necessary for skeleton stage; spec'd in the comment to
  drop past M5. **Recommend**: tighten to per-module allows once M3 starts
  producing real consumers.
- `Default` for `Renderer` calls `Self::new().expect("renderer init")`
  (`src/render/mod.rs:43`). This is a footgun once `new()` actually does
  fallible wgpu init. **Deferred fix**: drop the `Default` impl in M1.
- `homography_round_trip_smoke` test (`src/render/warp.rs:18`) is a stub with
  a TODO body — currently passes because it asserts nothing. Risk: false
  green. **Mitigation**: M5 fills it in; CI today is not protecting against
  homography regressions.
- `tests/` and `docs/` are empty directories not tracked by git. Will silently
  vanish from a fresh clone. Add `.gitkeep` if they are intended to persist —
  or wait until they hold real content (preferred).

---

## PHASE 2 — Changes applied

Two files touched, both within the 30-line budget.

| File | Change | Lines |
|---|---|---|
| `src/modulators/waveforms.rs` | Added `noise()` (smooth value-noise via hashed samples + smoothstep) and `hash01()` helper | +18 |
| `src/modulators/mod.rs` | `Modulator::value()` now dispatches all variants to `waveforms::*` using `clock.elapsed()` and `clock.bpm()`; `Bpm` reuses `sine` over the beat period | +14, −5 |

Both edits are reversible. Verification re-ran clean (see "Verification
status" above). No architectural commitments made — `Modulator` is still a
closed enum with the same variant set; only the stub bodies are now real.

---

## PHASE 3 — Plan

### 3.1 Architecture overview

#### Module tree (annotated)

| Path | LoC | Phase 0 status | Notes |
|---|---:|---|---|
| `Cargo.toml` | 80 | kept-as-is | Faithful to spec crate table; see §3.3 |
| `Cargo.lock` | — | kept-as-is | Tracked; reproducible builds |
| `Makefile` | 49 | kept-as-is | All required CI targets present |
| `mise.toml` | 14 | kept-as-is | Pins Rust 1.85 (matches MSRV) |
| `build.rs` | 35 | kept-as-is | Real WGSL parse+validate via `naga` |
| `.gitignore` | 16 | kept-as-is | Standard Rust + mise overrides |
| `src/main.rs` | 61 | kept-with-edits | M2: add `tracing_appender`; M5: drop blanket allow |
| `src/app.rs` | 22 | rewrite | M1: full `ApplicationHandler` impl |
| `src/error.rs` | 21 | kept-with-edits | M2: add `RenderPanic` variant |
| `src/clock.rs` | 46 | kept-as-is | Real `Clock` + tap-tempo + BPM |
| `src/monitors.rs` | 18 | rewrite | M1: real winit-backed enumeration |
| `src/svg_layer.rs` | 28 | rewrite | M3: real load + worker + watcher |
| `src/test_patterns.rs` | 28 | kept-with-edits | M2: add per-pattern WGSL renderer |
| `src/project/mod.rs` | 38 | kept-with-edits | M6: real load/save (atomic write) |
| `src/project/schema.rs` | 105 | kept-as-is | Real serde with `default` + `schema_version` |
| `src/project/migrate.rs` | 23 | kept-as-is | Real v0→v1 migration entry |
| `src/render/mod.rs` | 48 | rewrite | M1: real wgpu init; M2: `catch_unwind` |
| `src/render/pipeline.rs` | 13 | rewrite | M4: ping-pong texture chain |
| `src/render/compositor.rs` | 13 | rewrite | M5: N-layer blend |
| `src/render/warp.rs` | 29 | rewrite | M5: mesh + mask + real homography test |
| `src/render/gamma.rs` | 19 | kept-with-edits | M5: WGSL master pass |
| `src/render/shaders/triangle.wgsl` | 26 | kept-as-is | Real, M1 hello-rectangle |
| `src/effects/mod.rs` | 33 | kept-as-is | Closed `Effect` enum per spec |
| `src/effects/color.rs` | 4 | rewrite | M4: WGSL color pass |
| `src/effects/blur.rs` | 4 | rewrite | M4: separable gaussian |
| `src/effects/transform.rs` | 6 | rewrite | M4: vertex-stage matrix push |
| `src/modulators/mod.rs` | 71 | kept-as-is (Phase 2) | Real dispatch to waveforms |
| `src/modulators/waveforms.rs` | 56 | kept-as-is (Phase 2) | sine/triangle/noise + tests |
| `src/windows/mod.rs` | 5 | kept-as-is | Re-exports |
| `src/windows/output.rs` | 22 | kept-with-edits | M1: hold Surface; M2: key handlers |
| `src/windows/control.rs` | 6 | rewrite | M4: egui-wgpu+winit integration |
| `src/controls/mod.rs` | 12 | kept-with-edits | M4: KeyboardSource; v1.5: Source trait |
| `src/show_day/mod.rs` | 7 | kept-as-is | Re-exports |
| `src/show_day/sleep_assertion.rs` | 29 | kept-with-edits | M2: real `IOPMAssertion` |

**Missing per spec** (no stub yet, all explicitly deferred to a milestone):

- `src/show_day/panic_restore.rs` — owner of the `catch_unwind` wrapper. M2.
- `src/svg_layer/worker.rs`, `src/svg_layer/watcher.rs` — off-thread worker
  + `notify` debouncer. M3.
- `src/render/shaders/{color,blur_h,blur_v,transform,compositor,warp,mask,gamma}.wgsl`
  — one shader per pass. M2 (test patterns first), M4 (effects), M5
  (compositor/warp/mask/gamma).
- `tests/golden/` — golden PNGs for headless wgpu tests. M5.
- `docs/show-day-checklist.md` — operator-facing doc. M6.

#### Cross-module boundary signatures

> Format: `// today` shows what the scaffold has; `// target` shows what the
> milestones must produce. Trait signatures only — bodies are in the
> respective module sections.

**`render::Renderer`** (`src/render/mod.rs:23`)

```rust
// today
pub struct Renderer { /* TODO M1 */ }
impl Renderer {
    pub fn new() -> Result<Self, RenderError>;
    pub fn render_frame(&mut self) -> Result<(), RenderError>;
}

// target (M1, M2)
pub struct Renderer {
    instance: wgpu::Instance,
    adapter:  wgpu::Adapter,
    device:   wgpu::Device,
    queue:    wgpu::Queue,
    pipelines: PipelineCache,
}
impl Renderer {
    pub fn new(surface: &wgpu::Surface<'_>) -> Result<Self, RenderError>;
    pub fn render_to(
        &mut self,
        target: PresentTarget<'_>,
        scene:  &SceneFrame<'_>,
        clock:  &Clock,
    ) -> Result<(), RenderError>;
}
pub enum PresentTarget<'a> {
    Surface { surface: &'a wgpu::Surface<'a>, config: &'a wgpu::SurfaceConfiguration },
    Texture { view: &'a wgpu::TextureView, format: wgpu::TextureFormat, size: (u32, u32) },
}
```

**`effects::Effect`** (`src/effects/mod.rs:11`) — closed enum stays closed:

```rust
// today + target (no migration needed)
pub enum Effect { Color { … }, Tint { … }, Blur { … }, Transform { … } }

// target additions in M4
impl Effect {
    pub fn render(
        &self,
        ctx:   &PassCtx<'_>,    // bind groups, pipelines, ping-pong textures
        clock: &Clock,
    ) -> Result<(), RenderError>;
}
```

See §3.2 for the v1.5 `ExternalPass` extension slot.

**`modulators::Modulator`** (`src/modulators/mod.rs:10`) — already real after
Phase 2; signature stable:

```rust
pub fn value(&self, clock: &Clock) -> f32;
```

**`render::warp::Warp`** (`src/render/warp.rs:5`)

```rust
// today
pub struct Warp { /* TODO M5 */ }
impl Warp { pub fn new() -> Self; }

// target (M5)
pub struct Warp {
    mesh: WarpMeshGpu,            // vertex/index buffers, derived from WarpMesh
    mask: Option<MaskTexture>,    // SDF or polygon-fan, with feather radius
}
impl Warp {
    pub fn from_config(device: &wgpu::Device, mesh: &WarpMesh) -> Self;
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        src:     &wgpu::TextureView,
        dst:     &wgpu::TextureView,
    ) -> Result<(), RenderError>;
}
```

**`render::compositor::Compositor`** (`src/render/compositor.rs:4`)

```rust
// target (M5)
impl Compositor {
    pub fn composite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        layers:  &[(&wgpu::TextureView, BlendMode, f32 /* opacity */)],
        dst:     &wgpu::TextureView,
    ) -> Result<(), RenderError>;
}
```

**`project::Project`** (`src/project/mod.rs:31`)

```rust
// today
impl Project {
    pub fn load(path: &Path) -> Result<Self, ProjectError>;  // returns Default!
    pub fn save(&self, path: &Path) -> Result<(), ProjectError>; // no-op
}

// target (M6)
impl Project {
    pub fn load(path: &Path) -> Result<Self, ProjectError>;       // real
    pub fn save(&self, path: &Path) -> Result<(), ProjectError>;  // atomic tmp+rename
    pub fn resolve_asset(&self, base: &Path, rel: &Path) -> PathBuf;
}
```

**`controls::InputState`** (`src/controls/mod.rs:6`)

```rust
// today
pub struct InputState { pub current_scene: Option<usize> }

// target (M4 keyboard, v1.5 trait)
pub trait Source {
    fn poll(&mut self) -> Vec<ControlEvent>;
}
pub enum ControlEvent {
    TapTempo,
    SceneRecall(usize),     // 0..=8
    Blackout,
    Freeze,
    ParamSet { binding: SourceRef, value: f32 },  // v1.5
}
pub struct InputState {
    pub current_scene: Option<usize>,
    sources: Vec<Box<dyn Source>>,                 // Keyboard now; Midi/Osc v1.5
}
```

**`clock::Clock`** (`src/clock.rs:7`) — already real; no signature change
expected.

#### Frame data flow (one frame)

```
┌────────────────────────────────────────────────────────────────────────┐
│ winit ApplicationHandler::about_to_wait                                │
│ window.request_redraw() — scheduled at the display refresh rate         │
└──────────────────────────────┬─────────────────────────────────────────┘
                               │
                               ▼
              ┌─────────────────────────────────┐
              │ clock::Clock::tick (no-op; just  │
              │  exposes elapsed() + bpm())      │
              └────────────────┬────────────────┘
                               │  &Clock
        ┌──────────────────────┴────────────────────────────┐
        │                                                    │
        ▼                                                    ▼
┌───────────────────┐                            ┌────────────────────┐
│ controls poll     │                            │ svg_layer worker   │
│ (Keyboard now;    │                            │ thread reports any │
│  Midi/Osc v1.5)   │                            │ ready re-rasters   │
└────────┬──────────┘                            └─────────┬──────────┘
         │ ControlEvents                                    │ texture handle swaps
         ▼                                                  ▼
┌────────────────────────────────────────────────────────────────────────┐
│ App: apply control events → mutate Project state OR Clock OR scenes    │
└──────────────────────────────┬─────────────────────────────────────────┘
                               │
                               ▼
              ┌─────────────────────────────────┐
              │ For each LayerConfig:           │
              │   modulators evaluated -> f32s  │
              │   ↓                              │
              │   effects::pipeline (ping-pong)  │
              │     transform → color → blur     │
              │   ↓                              │
              │   layer texture                  │
              └────────────────┬────────────────┘
                               │
                               ▼
              ┌─────────────────────────────────┐
              │ compositor: N layers → output    │
              │  texture (per-layer blend mode + │
              │  opacity)                        │
              └────────────────┬────────────────┘
                               │
                               ▼
              ┌─────────────────────────────────┐
              │ gamma master pass (one shader)   │
              └────────────────┬────────────────┘
                               │
                               ▼
              ┌─────────────────────────────────┐
              │ For each Warp:                   │
              │   warp.render(src, dst)          │
              │   (projective UV interp + mask)  │
              └────────────────┬────────────────┘
                               │
                               ▼
              ┌─────────────────────────────────┐
              │ if freeze: skip everything above │
              │ if blackout: clear(BLACK)        │
              │ surface.present()                │
              └─────────────────────────────────┘

Wrapper for the whole pipeline (M2):
  std::panic::catch_unwind around render_to() → on UnwindSafe panic, log,
  reset GPU surface, render error overlay on the *control* window only.
```

### 3.2 Extensibility design

#### Choice and defense

The scaffold uses **option (c): closed enum for v1, "add a variant" recipe
documented**. We keep that and design the **migration path to (a)**
(hybrid: closed enum for built-ins + `ExternalPass` trait slot) for **v1.5**.

Why (c) now, (a) later:

1. v1 ships to one operator (the dev-VJ). External pipelines are *imagined*,
   not actual; no real plugin author exists yet. Optimizing for them today is
   speculative.
2. Built-ins benefit from exhaustive `match`. Adding `Effect::Tint` *forced*
   the renderer, the UI, and serde to update. With (b) this becomes a runtime
   surprise.
3. (c) → (a) migration is mechanical and additive: introduce
   `Effect::External { id: String, params: serde_json::Value }` plus a
   registry; every existing variant is untouched.
4. (b) loses too much for a single-author renderer where the next "plugin" is
   an internal `cargo build` away.

If we revisit at v1.5 and decide to switch, the migration is one PR:

1. Add `ExternalRegistry` (`src/effects/registry.rs`).
2. Add `Effect::External { … }` variant.
3. Renderer dispatches `Effect::External` through the registry; everything
   else is untouched.
4. Effect schema in the project file gains an `external` shape; `migrate` adds
   a v1→v2 step.

#### Extension points

Each extension point names: the trait, where it is registered, and ~5 lines
of pseudocode showing how a new implementor wires in.

##### Effect / pass extension point (v1.5)

```rust
// src/effects/external.rs (new at v1.5)
pub trait ExternalPass: Send + Sync {
    fn id(&self) -> &str;
    fn init(&mut self, ctx: &PipelineCtx<'_>) -> Result<(), RenderError>;
    fn resize(&mut self, w: u32, h: u32) -> Result<(), RenderError>;
    fn render(&self, ctx: &PassCtx<'_>) -> Result<(), RenderError>;
    fn param_schema(&self) -> &'static [ParamDef];
}
```

- **Registered** at app boot in `App::run` via
  `external_registry.register::<MyPass>("user.crt-glitch")`.
- **WGSL registration**: pass author calls
  `external_registry.register_shader("user.crt-glitch", include_str!("crt.wgsl"))`;
  validation may be done in the plugin author's own `build.rs`.
- **Parameter schema**: `&'static [ParamDef { name, range, default, modulator: bool }]`
  drives the egui control panel auto-generation.
- **A new pass wires in like this**:

```rust
// in some plugin module, v1.5+
struct CrtGlitch { … }
impl ExternalPass for CrtGlitch { … }

// in App::run startup
let mut reg = ExternalRegistry::new();
reg.register("user.crt-glitch", |params: &Value| {
    Box::new(CrtGlitch::from_params(params))
});
```

##### Modulator extension point

`Modulator::Audio { band, smoothing, amp, offset }` is **already an enum slot
reserved as a comment** (`src/modulators/mod.rs:34`). Wiring in v1.5:

1. Uncomment the `Audio` variant.
2. Add `audio_provider: Option<Arc<dyn AudioProvider>>` to `Clock` (or a
   sibling `AudioState` injected into `Modulator::value`).
3. Implement `AudioProvider` for `cpal`-backed capture (M7).
4. `Modulator::value` adds one match arm calling `audio_provider.band(band)`.

No churn to existing variants. The enum stays exhaustive.

##### Input / control extension point — `Param::bind(source)`

```rust
// src/controls/param.rs (new at M4)
pub enum Param<T: Copy> {
    Static(T),
    Modulated(Modulator),     // v1
    Bound(SourceRef),         // v1.5: index into InputState::sources
}
impl Param<f32> {
    pub fn value(&self, clock: &Clock, inputs: &InputState) -> f32 { … }
}

// src/controls/mod.rs
pub trait Source {
    fn poll(&mut self) -> Vec<ControlEvent>;
    /// SourceRef → current value (for `Param::Bound` resolution).
    fn read(&self, ref_: SourceRef) -> Option<f32>;
}
```

- **Registered** at app boot:
  `inputs.register(Box::new(KeyboardSource::default()))`.
- **A new MIDI source wires in like this** (v1.5):

```rust
// src/controls/midi.rs (new at v1.5, behind `midi` cargo feature)
struct MidiSource { conn: midir::MidiInputConnection<()> , values: HashMap<u8, f32> }
impl Source for MidiSource { … }

// in App::run startup
if cfg!(feature = "midi") {
    inputs.register(Box::new(MidiSource::open()?));
}
```

The same shape covers OSC (`rosc`).

##### Output sink extension point

`PresentTarget` (introduced in §3.1 above) decouples renderer from surface:

```rust
pub enum PresentTarget<'a> {
    Surface { surface: &'a wgpu::Surface<'a>, config: &'a wgpu::SurfaceConfiguration },
    Texture { view: &'a wgpu::TextureView, format: wgpu::TextureFormat, size: (u32, u32) },
}
```

- v1: `App` only ever passes `Surface { … }` for the projector window.
- v1.5 "render to PNG" (preview, recording): same renderer, `Texture` target,
  `wgpu::Queue::copy_texture_to_buffer` for readback.
- v1.5 second projector / preview window: a second `Surface { … }` target;
  the renderer is target-agnostic so adding outputs does not touch the
  pipeline code.

Surface lifecycle (creation, configure, lost/outdated handling) stays in
`windows::output`; the renderer never reaches across that boundary.

### 3.3 Crate & workspace layout

#### Cargo.toml audit

The scaffold is **faithful to the spec's crate table**. No diff is required.
Every crate the spec recommends is present at a version that resolves on
crates.io today. Every crate the spec rejects is absent.

| Spec recommendation | Cargo.toml entry | Status |
|---|---|---|
| `winit` 0.30 | `winit = "0.30"` | ✓ |
| `wgpu` (current) | `wgpu = "29"` | ✓ |
| `egui` family | `egui`/`egui-wgpu`/`egui-winit = "0.34"` | ✓ |
| `glam` | `glam = { "0.32", features = ["serde"] }` | ✓ |
| `resvg` + `usvg` + `tiny-skia` | `0.47` + `0.47` + `0.12` | ✓ |
| `image` | `image = "0.25"` | ✓ |
| `notify` + debouncer | `notify-debouncer-full = "0.7"` (notify transitive) | ✓ |
| `crossbeam-channel` | `0.5` | ✓ |
| `clap` | `4` with `derive` | ✓ |
| `serde` + `serde_json` | `1` + `1` | ✓ |
| `tracing` + subscriber + appender | all present | ✓ |
| `thiserror` + `anyhow` | `2` + `1` | ✓ |
| `objc2` family (macOS-only) | `0.6` + `0.3` × 3 | ✓ (cfg-gated) |
| `naga` (build-dep) | `naga = "29"` with `wgsl-in` | ✓ |

Extras present that are **not** in the spec table but are defensible:

- **`pollster = "0.4"`** — single-file blocking adapter for async wgpu init.
  Spec §"Concurrency model" forbids `tokio` and any async runtime; `pollster`
  is one `block_on` function, not a runtime. Justified.

Rejected crates absent: `bevy`, `nannou`, `iced`, `tokio`, `tauri`. ✓

`[profile.release-show]`: `inherits=release, lto="fat", codegen-units=1,
panic="abort", strip=true` — matches spec §"Build & distribution".

#### Cargo features

```toml
[features]
default = []
gpu-tests = []          # exists; consumers added in M5
# Reserved (added when M7 work begins):
# audio = ["dep:cpal", "dep:rustfft"]
# midi  = ["dep:midir"]
# osc   = ["dep:rosc"]
```

Reserved features do **not** add dependencies until the corresponding M7
work begins. Naming them now keeps the migration mechanical.

#### Future workspace boundaries (post v1.5; no churn now)

When the codebase splits into a workspace, the seams are:

| Future crate | Today's modules |
|---|---|
| `rmap-core` | `error`, `clock`, `project/*`, `effects/*`, `modulators/*`, `controls/*`, `monitors`, `svg_layer`, `render/*`, `show_day/*`, `test_patterns` |
| `rmap-cli` | `main`, `app`, `windows/*` |
| `rmap-plugins` | (placeholder; `ExternalPass` impls live here) |

The current single binary keeps all modules behind `crate::*` paths. Splitting
later is a rename of `mod foo` to `pub use rmap_core::foo` plus a `Cargo.toml`
restructure. No public API changes.

### 3.4 Milestone breakdown

| ID | Name | Days | Calendar gate | New files | Touched files |
|---|---|---:|---|---:|---:|
| M1 | Hello rectangle | 2 | — | 0 | 5 |
| M1.5 | Venue dry-run | 0.5 | yes | 0 | 0 |
| M2 | Calibration tooling | 3 | — | 2 | 8 |
| M3 | SVG on screen | 4 | — | 2 | 4 |
| M4 | Effects + modulators | 6 | — | 6 | 9 |
| M5 | Multi-layer + scenes + warp + masking | 7 | — | 5 | 8 |
| M6 | Project save/load + autostart + docs | 1.5 | — | 1 | 4 |
| M7 | Polish (ongoing) | — | — | many | many |

> Estimates are **adjusted from spec** (which used Python figures): up by
> ~30 % for compile-iteration overhead and wgpu/WGSL ramp, down by ~½ day in
> M1 because the scaffold already provides the shader, the render module
> stub, the monitor stub, and the CLI parser.

---

#### M1 — Hello rectangle (2 days)

**Scope** (spec §M1). winit borderless fullscreen on a chosen monitor; wgpu
device/queue/surface; renders the gradient quad from `triangle.wgsl`; Esc
closes cleanly with no GPU validation errors.

**Starting state**.

- `src/render/shaders/triangle.wgsl` — real, validated by `build.rs`.
- `src/render/mod.rs` — `Renderer` stub; `RenderError` variants present.
- `src/app.rs`, `src/windows/output.rs`, `src/monitors.rs` — stubs.
- `src/main.rs` — `clap` CLI wired; calls `App::run`.

**File-by-file deltas**.

| File | Change |
|---|---|
| `src/app.rs` | Implement `winit::application::ApplicationHandler`; on `resumed`, pick monitor → create borderless `Window` → create `wgpu::Surface` → init `Renderer`; on `window_event` Esc → `event_loop.exit()` |
| `src/windows/output.rs` | Hold `Window`, `Surface`, `SurfaceConfiguration`; `recreate_surface()` on `SurfaceError::{Lost,Outdated}`; hide cursor with `set_cursor_visible(false)` |
| `src/render/mod.rs` | `Renderer::new(surface)` — `pollster::block_on` adapter+device; build the `triangle.wgsl` `RenderPipeline` once; cache. Drop `Default for Renderer`. |
| `src/monitors.rs` | `list(event_loop: &ActiveEventLoop) -> Vec<MonitorInfo>` |
| `src/main.rs` | Pass parsed CLI to `App::run`; convert `RmapError` to `anyhow::Error` with `with_context` |

**Acceptance**.

- `make run` opens a borderless fullscreen window on monitor 0 with the
  gradient quad.
- Esc closes; `RUST_LOG=trace` shows no wgpu validation errors.
- `cargo test` still passes (no new tests required).

**Risk + mitigation**. macOS "Displays have separate Spaces" is the project's
biggest schedule risk. Mitigated by the M1.5 gate immediately after.

---

#### M1.5 — Venue dry-run (½ day, calendar gate)

No code change. Take the M1 binary to a real HDMI projector; verify
fullscreen lands on the projector, no display sleep, no Mission Control
glitches, Esc closes cleanly. Document the result in `docs/m15-dry-run.md`
as a one-page report.

**If M1.5 fails, halt M2+ until resolved.**

---

#### M2 — Calibration tooling (3 days)

**Scope** (spec §6 + §M2). Test patterns; blackout (`B`) + freeze (`F`);
display-sleep prevention; rolling file logs; per-frame `catch_unwind`.

**Starting state**.

- `src/test_patterns.rs` — `TestPattern` enum + labels, no renderer.
- `src/windows/output.rs::OutputState` — toggle methods, no key wiring.
- `src/show_day/sleep_assertion.rs` — cfg-gated stub.
- `src/main.rs:42` — TODO for `tracing_appender`.
- No `catch_unwind` anywhere.

**File-by-file deltas**.

| File | Change |
|---|---|
| `src/test_patterns.rs` | Add `render(&self, encoder, dst)`; one WGSL per pattern in `src/render/shaders/test_*.wgsl` |
| `src/render/shaders/test_grid.wgsl` *(new)* | Procedural 50 px grid |
| `src/render/shaders/test_crosshair.wgsl` *(new)* | Crosshair + corner markers |
| `src/render/shaders/test_levels.wgsl` *(new)* | White 100/50/25 + color bars (one shader, uniform selects) |
| `src/windows/output.rs` | Key handler: B → toggle blackout, F → toggle freeze, T → cycle test pattern |
| `src/show_day/sleep_assertion.rs` | Real `IOPMAssertionCreateWithName` via `objc2-io-kit`; `Drop` releases |
| `src/show_day/panic_restore.rs` *(new)* | `pub fn run_frame<F>(f: F) where F: FnOnce() + UnwindSafe` |
| `src/render/mod.rs` | Wrap `render_to` body in `panic_restore::run_frame`; surface error overlay event on panic |
| `src/error.rs` | Add `RenderPanic { source_message: String }` variant |
| `src/main.rs` | `tracing_appender::rolling::daily("~/Library/Logs/rmap/", "rmap.log")` layered with the stderr fmt layer |

**Acceptance**.

- All 4+ test patterns render correctly; switchable via UI + `T` hotkey.
- `B` blackouts; `F` freezes (control window still editable).
- 30-min idle: projector display does not sleep; on app exit the assertion
  releases (verify via `pmset -g assertions`).
- Inject a panic in the renderer (test-only `#[cfg(test)] fn force_panic`):
  app survives, error appears in log file.
- New unit test: `render_panic_does_not_propagate`.

**Risk + mitigation**. `objc2-io-kit` API churn — keep the platform fallback
behind `SleepAssertion::acquire`'s no-op impl; revert is one cfg-gated
module.

---

#### M3 — SVG on screen (4 days)

**Scope** (spec §1 + §M3). Load + rasterize via `resvg`; display as a
textured quad; hot-reload via `notify` debouncer, off-thread.

**Starting state**.

- `src/svg_layer.rs` — stub `SvgLayer { path }` with a `load()` that does
  nothing.
- Cargo: `resvg`, `usvg`, `tiny-skia`, `notify-debouncer-full`,
  `crossbeam-channel`.

**File-by-file deltas**.

| File | Change |
|---|---|
| `src/svg_layer.rs` | Real `SvgLayer { path, tree: usvg::Tree, gpu_texture: TextureHandle, generation: u64 }`; `load()` parses `usvg::Tree`; `rasterize_to(pixmap_size)` via `resvg::render`. Texture upload via `wgpu::Queue::write_texture`. |
| `src/svg_layer/worker.rs` *(new)* | `Worker::spawn() -> (JobTx, ResultRx)`; `std::thread::spawn` consumes `RasterJob { layer_id, path, size, generation }`, sends `RasterDone { layer_id, pixmap, generation }` |
| `src/svg_layer/watcher.rs` *(new)* | `Watcher::new(paths) -> EventRx`; thin wrapper around `notify_debouncer_full::new_debouncer(Duration::from_millis(250), …)` |
| `src/render/mod.rs` | Bind the layer texture as input to a quad-render pass (groundwork for M5 compositor) |
| `src/app.rs` | Per frame: drain `EventRx` → enqueue raster jobs; drain `ResultRx` → swap texture handles; throw away results with stale `generation` |

**Acceptance**.

- Single SVG appears on the projector.
- Edit the SVG in Illustrator → output updates within ~500 ms (250 ms
  debounce + raster + upload).
- Open a 200 KB SVG; frame timing remains ≥ 60 fps during reload (off-main
  proven via tracing span).
- New unit test: `hot_reload_event_coalescing` — fakes 3 `notify` events
  within 100 ms, asserts exactly one job emitted.

**Risk + mitigation**. wgpu/`tiny-skia` premultiplied-alpha mismatch is a
classic. Mitigated by an explicit RGBA8 *unmultiplied* path in
`svg_layer.rs::upload_to_gpu` and a comment marking the choice.

---

#### M4 — Effects + modulators (6 days)

**Scope** (spec §2 + §3 + §M4). Color, blur, opacity, transform as
ping-pong WGSL passes; modulator dispatch (already real after Phase 2);
`egui` control panel; tap tempo on `Space`.

**Starting state**.

- `src/effects/mod.rs` — closed `Effect` enum.
- `src/effects/{color,blur,transform}.rs` — doc-only stubs.
- `src/modulators/mod.rs` — real `Modulator::value()` (Phase 2).
- `src/modulators/waveforms.rs` — sine/triangle/noise/(bpm via sine).
- `src/clock.rs` — real `Clock` + tap.
- `src/render/pipeline.rs` — empty stub.

**File-by-file deltas**.

| File | Change |
|---|---|
| `src/render/pipeline.rs` | `EffectPipeline { ping: TextureView, pong: TextureView, flip: bool }`; `apply(effects: &[Effect], clock, src) -> &TextureView` |
| `src/effects/color.rs` | Pipeline + bind group for color shader; reads 4 modulator values per frame |
| `src/effects/blur.rs` | Two passes (horiz, vert) with kernel size from `radius_px` modulator |
| `src/effects/transform.rs` | Vertex-stage `glam::Mat3` push; rotation/scale modulator-driven |
| `src/render/shaders/color.wgsl` *(new)* | hue/sat/bri/con |
| `src/render/shaders/blur_h.wgsl`, `blur_v.wgsl` *(new)* | separable gaussian |
| `src/render/shaders/transform.wgsl` *(new)* | textured quad, vertex matrix |
| `src/windows/control.rs` | Real egui-winit + egui-wgpu integration on its own `Window` (primary display) |
| `src/windows/control_panel.rs` *(new)* | egui sliders bound to `LayerConfig`/`Effect`/`Modulator` |
| `src/controls/keyboard.rs` *(new)* | `KeyboardSource` impl of `Source` trait |
| `src/controls/param.rs` *(new)* | `Param<T>` enum (Static / Modulated; Bound stub for v1.5) |
| `src/controls/mod.rs` | Add `Source` trait + `register(...)`; expose `poll() -> Vec<ControlEvent>` |
| `src/app.rs` | Space → `clock.tap()`; route `ControlEvent::TapTempo` |

**Acceptance**.

- Each effect renders in isolation (golden-image tests, see §3.5).
- Sliders modify parameters live (no re-init per change).
- Set `Modulator::Sine` on `Color::brightness` → projector visibly oscillates
  at the configured period.
- Tap `Space` 4 times at 0.5 s intervals → BPM converges to 120 ± 5.

**Risk + mitigation**. Multi-window egui has historically been fiddly. Use
the `multiple_viewports` example from `egui-winit` as the reference; defer
to single-window with a tabbed UI if blocked.

---

#### M5 — Multi-layer + scenes + warp + masking (7 days)

**Scope** (spec §4 + §M5). N-layer compositor; warp mesh (1×1 corner-pin);
per-warp polygon mask + feather; gamma master; scene snapshots.

**Starting state**.

- `src/project/schema.rs` — `WarpMesh`, `Scene`, `LayerConfig` real.
- `src/render/{warp,compositor,gamma}.rs` — empty stubs.
- `src/render/warp.rs::tests::homography_round_trip_smoke` — empty body.

**File-by-file deltas**.

| File | Change |
|---|---|
| `src/render/compositor.rs` | N-layer composite with `BlendMode` switch; one pipeline per blend mode, cached |
| `src/render/warp.rs` | `Warp::from_config(&WarpMesh)`; vertex buffer for `(rows+1)×(cols+1)` grid; index buffer triangle strip; `mask_texture` from polygon-fan rasterized into an SDF |
| `src/render/warp.rs::tests` | Real homography test using `glam::Mat3`: project unit-square corners through known quad; assert `< 1e-4` residual |
| `src/render/gamma.rs` | WGSL master pass: gamma + brightness + contrast |
| `src/render/shaders/{compositor,warp,mask,gamma}.wgsl` *(new)* | One per pass |
| `src/windows/control_panel.rs` | Layer reorder UI; Mapping tab (drag corners on a control-window canvas mirroring the output); Scenes tab with hotkeys 1–9 |
| `src/app.rs` | `1`–`9` keys → scene recall; `Cmd+S` → save snapshot to current scene |
| `tests/golden/{color,blur,warp}.png` *(new)* | Reference images for headless GPU tests (see §3.5) |
| `tests/headless_gpu.rs` *(new)* | Standalone `wgpu::Instance`; render each effect into an offscreen texture; readback; pixel-compare with tolerance |

**Acceptance**.

- Drag a warp corner; output follows in real time.
- Polygon mask hides a region; feather creates a soft alpha falloff.
- Save current state to scene 1; modify; recall scene 1 → instant snap-back.
- Headless tests pass under `cargo test --features gpu-tests`.

**Risk + mitigation**. Mesh subdivision math is easy to get wrong. Keep mesh
geometry as a `(rows+1)×(cols+1)` `Vec<Vec<[f32; 2]>>` even at 1×1; M7 mesh
work then only changes counts.

---

#### M6 — Project save/load + autostart + docs (1.5 days)

**Scope** (spec §8 + §M6).

**Starting state**.

- `src/project/mod.rs` — load returns `Default`, save is a no-op.
- `src/project/migrate.rs` — real v0→v1.
- `src/main.rs` — `clap` CLI parses `project: Option<PathBuf>` + `--autostart`.

**File-by-file deltas**.

| File | Change |
|---|---|
| `src/project/mod.rs` | Real `load`: read file → `serde_json::from_str::<Value>` → `migrate` → `serde_json::from_value::<Project>` → resolve `asset_root` |
| `src/project/mod.rs` | Real `save`: serialize → write to `.tmp` → `rename` (atomic) |
| `src/app.rs` | `--autostart` opens output window on saved monitor without waiting for user click |
| `docs/show-day-checklist.md` *(new)* | Operator-facing pre-show checklist (DnD, hot corners, energy saver, lock screen, projector firmware) |

**Acceptance**.

- Save → exit → reload yields byte-identical state for a non-trivial project.
- Schema-v0 file (no `schema_version` field) loads, migrates to v1, saves
  with `schema_version: 1`.
- `rmap path/to/project.rmap.json --autostart` goes straight to projector.
- New unit test: `project_round_trip` and `project_v0_migrate`.

---

#### M7 — Polish (ongoing)

Sketch only; sequenced as needed.

| Item | Touch points |
|---|---|
| Multi-cell mesh (5×5) | `WarpMesh.rows/cols` already supports it; UI drag-many-points |
| Multiple independent warps | `Project.warps: Vec<WarpMesh>` already supports it; `App::render_pass` loops |
| Audio modulator | Add `cpal` + `rustfft` behind `audio` feature; uncomment `Modulator::Audio`; new `AudioProvider` trait |
| Scene crossfade | Snapshot-interpolation over `Duration` |
| MIDI / OSC input | `midir` / `rosc` behind features; impl `Source` trait (see §3.2) |
| `ExternalPass` registry | See §3.2 migration steps |
| Effect presets | JSON bundles in `assets/presets/`; load via UI |

### 3.5 Testing strategy

#### Audit of existing test infrastructure

- `tests/` directory: **empty** (created by `mkdir -p` but never populated;
  not tracked by git).
- `build.rs`: **real** — parses every WGSL file under
  `src/render/shaders/` with `naga::front::wgsl::parse_str`, then validates
  with `naga::valid::Validator::new(ValidationFlags::all(),
  Capabilities::all())`. A broken shader fails `cargo build` before the
  binary is produced. ✓
- `gpu-tests` feature: **declared** in `Cargo.toml`, no consumers yet. M5
  introduces the first golden-image tests.

#### Unit tests by name

| Test | Invariant pinned | Status |
|---|---|---|
| `modulators::waveforms::tests::sine_zero_at_origin` | `sine(0,1,1,0,0) == 0` | exists |
| `modulators::waveforms::tests::sine_peak_at_quarter_period` | `sine(0.25,1,1,0,0) == 1` | exists |
| `modulators::waveforms::tests::triangle_extrema` | `triangle(0)=−1`, `triangle(0.5)=1` | exists |
| `render::warp::tests::homography_round_trip_smoke` | 4-pt homography re-projects unit square | **stub (no body); fill in M5** |
| `modulators::tests::dispatch_static` | `Static(v).value(any clock) == v` | missing; add in M4 |
| `modulators::tests::dispatch_bpm_at_120` | `Bpm{divisor:1,…}.value(t)` matches `sine` over 0.5 s | missing; add in M4 |
| `clock::tests::tap_tempo_converges` | 4 taps at 0.5 s → BPM in [115, 125] | missing; add in M4 |
| `project::tests::round_trip` | save → load → identical | missing; add in M6 |
| `project::tests::v0_migrate` | file lacking `schema_version` loads as v1 | missing; add in M6 |
| `svg_layer::tests::hot_reload_event_coalescing` | 3 fake events in 100 ms → 1 job | missing; add in M3 |
| `render::tests::render_panic_does_not_propagate` | `force_panic()` → app survives, error logged | missing; add in M2 |

#### Headless GPU golden-image tests (M5)

- `tests/headless_gpu.rs` builds a standalone `wgpu::Instance` with no
  surface, requests an adapter, creates an offscreen `wgpu::Texture` of
  fixed size (e.g. 256×256 RGBA8).
- For each named pass (color, blur radius=8, corner-pin warp at known
  corners, gamma=2.2), render → `Queue::copy_texture_to_buffer` → readback
  to `Vec<u8>` → load `tests/golden/<name>.png` via `image` crate →
  per-pixel max-channel diff with tolerance `≤ 2/255` for cross-driver
  wobble.
- Behind the `gpu-tests` feature so plain `cargo test` stays CPU-only.
- CI matrix: macOS Apple Silicon today; Linux Vulkan when contributors join.

### 3.6 Show-day hardening checklist

| Spec requirement (§6) | Module | Owner fn | Status today |
|---|---|---|---|
| Blackout (`B`) | `windows::output` | `OutputState::toggle_blackout` + key handler | type exists; key handler **missing** (M2) |
| Freeze (`F`) | `windows::output` | `OutputState::toggle_freeze` + key handler | type exists; key handler **missing** (M2) |
| Test patterns (grid, crosshair, white levels, color bars) | `test_patterns` | `TestPattern::render` + WGSL | enum + label exist; renderer **missing** (M2) |
| Error overlay (control window only) | `windows::control` | `error_overlay()` (egui) | **missing** (M2) |
| Logging — rotating file | `main` (`init_tracing`) | `tracing_appender::rolling::daily` | stderr only; **TODO at `src/main.rs:42`** (M2) |
| Panic restore | `show_day::panic_restore` (new) | `run_frame<F>(f)` wrapping `catch_unwind` | **missing** (M2) |
| Display-sleep prevention (macOS) | `show_day::sleep_assertion` | `SleepAssertion::acquire` | cfg-gated stub; **real `IOPMAssertion` missing** (M2) |
| App Nap suppression (macOS) | `show_day::sleep_assertion` | (separate fn) | **missing** (M2; add as part of `acquire`) |
| Cursor hidden on output | `windows::output` | `Window::set_cursor_visible(false)` | **missing** (M1) |
| Surface lost/outdated recovery | `windows::output` | `recreate_surface()` | **missing** (M1) |
| Gamma master | `render::gamma` | WGSL pass | type exists; shader **missing** (M5) |
| `--autostart` CLI | `main` + `app` | `Cli::autostart` flag | parsed; consumer **missing** (M6) |
| `docs/show-day-checklist.md` | docs/ | — | **missing** (M6) |

### 3.7 Open decisions

Each open item below names the decision, the options, and how this plan
treats it.

1. **egui multi-window vs. single-window**.
   *Spec implies multi-window* (output on projector, control on primary).
   `egui-winit` `multiple_viewports` exists but is fiddly with shared wgpu
   devices. **Plan resolves**: try multi-window first in M4; if blocked >½
   day, ship a single-window control panel with a "Send to projector" checkbox
   instead. Decision deadline: M4 day 2.
2. **Workspace split now vs. v1.5**.
   *Spec says single binary v1*. **Plan resolves**: stay single-crate. v1.5
   migration is mechanical and named in §3.3. **No user input needed.**
3. **Asset path resolution**: project file directory vs. explicit `asset_root`.
   *Spec mentions both*. **Plan resolves**: `Project.asset_root` overrides if
   `Some`; default = parent directory of the project file. Documented in
   `project::Project::resolve_asset`.
4. **Reserved features (`audio`, `midi`, `osc`)**: add stub deps now or wait?
   **Plan resolves**: wait until the corresponding M7 work begins; declare the
   feature names in `Cargo.toml` as comments today.
5. **`homography_round_trip_smoke` no-op test**: leave silently green, or
   `#[ignore]` until filled? **Plan resolves**: convert to
   `#[ignore = "M5: real homography solver"]` in Phase 2 follow-up — not done
   in Phase 2 because it crosses into M5 design (which solver, which residual
   tolerance). **Needs your call**: if you want it ignored now, say so;
   otherwise it ships as-is until M5.
6. **`Default` impl on `Renderer`** (`src/render/mod.rs:43`). Calls
   `Self::new().expect(...)`. Once `new` does real wgpu init, this becomes a
   panic-on-Default footgun. **Plan resolves**: drop the `Default` impl in
   M1 alongside the real `new`. **No user input needed.**
7. **Crate-root `#![allow(dead_code, unused_imports)]`** in `src/main.rs`.
   **Plan resolves**: tighten to per-module allows during M5; remove entirely
   at end of M5. **No user input needed.**

### 3.8 Deviations from spec

None. Every architectural choice in the scaffold matches the spec's
recommendations:

- Closed `Effect` enum (not trait objects). ✓
- Closed `Modulator` enum with `Audio` reserved as a comment. ✓
- No async runtime; `pollster` for async wgpu init. ✓
- `glam` over `nalgebra`. ✓
- Single binary crate. ✓
- `objc2` family over deprecated `cocoa`/`objc`. ✓
- `egui` (not `iced`, not `dearpygui`). ✓
- WGSL validated at build time via `naga`. ✓
- `serde(default)` on every optional `Project` field. ✓
- `schema_version` from day one with a `migrate` function. ✓

If a future revision wants to **change** the closed-enum decision (e.g. the
operator decides v1 needs user-supplied passes), the §3.2 migration recipe
is mechanical and additive.

---

*End of plan. Cross-references: spec lives at `specs/001-initial-setup.md`;
the scaffold builds with `make build`; the CI gate is `make ci`.*
