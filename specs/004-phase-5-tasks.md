# 004 Phase 5 — task breakdown

Companion task spec for [`004-phase-5.md`](004-phase-5.md). Each task
below is sized for a single PR.

## Implementation status (2026-05-13)

- [x] P5.0.1 — DMX transport decision (`004-phase-5-dmx-transport-decision.md`)
- [x] P5.0.2 — Fixture personality format decision (`004-phase-5-fixture-personality-decision.md`)
- [x] P5.0.3 — Colour-space + readback decision (`004-phase-5-color-space-decision.md`)

### W1 — Setup + housekeeping (complete)
- [x] P5.1.1 e5fe4f4 — glossary entries for 9 Phase 5 lighting domain terms
- [x] P5.1.2 59c31ce — lighting frame-budget stub test in `src/lighting/mod.rs`
- [x] P5.1.3 59c31ce — CHANGELOG v0.9.0 placeholder + README lighting section placeholder
- [x] P5.1.4 5f3939c — `lighting` cargo feature gate skeleton (`artnet_protocol` dep, empty `src/lighting/mod.rs`, `#[cfg(feature = "lighting")]` in lib.rs + main.rs)

### W2 — Transport layer (complete)
- [x] P5.2.1 ef38a57 — `DmxTransport` trait + `ArtNetTransport` impl + `NullTransport`
- [x] P5.2.2 ef38a57 — `DmxUniverse` newtype + `UniverseId` + `UniverseFrame`
- [x] P5.2.3 4480478 — `LightingThread` background loop at ~44 Hz
- [x] P5.2.4 ab27a0a — `LightingThread` wired into `EditingState` / `GoLive` transitions

### W3 — Fixture model (complete)
- [x] P5.3.1 6a52217 — `ChannelRole`, `FixturePersonality`, `FixtureGroup` structs + schema field
- [x] P5.3.2 6a52217 — `PixelMap` struct + `sample_uvs`
- [x] P5.3.3 7dc9199 — `Mutation::AddFixtureGroup` + reverse (`RemoveFixtureGroup`)
- [x] P5.3.4 7dc9199 — `Mutation::RemoveFixtureGroup` + reverse (`AddFixtureGroup`)
- [x] P5.3.5 7dc9199 — `Mutation::SetFixtureGroupParams` + reverse
- [x] P5.3.6 2585ba7 — DMX-frame builder (`build_universe_frame`)

### W4 — Colour-from-pixel sampling
- [ ] P5.4.1 — Lighting-tap texture + downsample render pass (64×36 `LightingTapPass`)
- [ ] P5.4.2 — Staging buffer + readback in lighting thread
- [ ] P5.4.3 — `LightingTapBuffer` + `sample_and_convert`
- [ ] P5.4.4 — Per-fixture sample budget enforcement (max 256)
- [ ] P5.4.5 — Lighting thread sampling + DMX-frame send loop

### W5 — Subscriber list for Blackout / Go-live fan-out
- [ ] P5.5.1 — `LightSubscriber` trait + subscriber list in `EditingState`
- [ ] P5.5.2 — Wire `Command::Blackout` to fan-out subscribers
- [ ] P5.5.3 — Wire `EnterGoLive` / `ExitGoLive` to fan-out subscribers

### W6 — Zone-derived fixture binding
- [ ] P5.6.1 — `FixtureSource::ZoneTag` variant + schema (already present as `String`; confirm/refactor)
- [ ] P5.6.2 — Zone-activity → DMX intensity mapping

### W7 — BPM-locked fixture chases (complete)
- [x] P5.7.1 6a52217 — `FixtureChase` data model + schema
- [x] P5.7.2 7dc9199 — `Mutation::AddFixtureChase` + reverse
- [x] P5.7.3 7dc9199 — `Mutation::RemoveFixtureChase` + reverse
- [x] P5.7.4 7dc9199 — `Mutation::SetFixtureChaseParams` + reverse
- [x] P5.7.5 6a52217 — `ChaseTicker` + `Modulator::Bpm` integration

### W8 — Output panel UI
- [ ] P5.8.1 — Output panel "Lighting" section skeleton
- [ ] P5.8.2 — Fixture-group list + add/remove in Output panel
- [ ] P5.8.3 — Fixture personality editor in the group row
- [ ] P5.8.4 — Canvas-region drag-to-assign in Output panel

### W9 — Diagnostics
- [ ] P5.9.1 — DMX universe activity LED in diagnostics chrome
- [ ] P5.9.2 — Art-Net packet-rate badge in diagnostics chrome

### W10 — Snapshot / proptest / packet-capture acceptance test
- [ ] P5.10.1 — Snapshot integration: `LightCue` in project snapshot
- [ ] P5.10.2 — Proptest extension: fixture-group Mutation round-trips
- [ ] P5.10.3 — Packet-capture acceptance test (CI Art-Net listener)

### W11 — Release housekeeping + acceptance smoke
- [ ] P5.11.1 — Phase 5 acceptance smoke test (manual)
- [ ] P5.11.2 — Version bump + CHANGELOG body for v0.9
- [ ] P5.11.3 — README — Phase 5 lighting section
- [ ] P5.11.4 — Show-day checklist: lighting pre-show checks

---

## Operating model

- **Model:** Sonnet implements; Opus reviews. Same read-the-spec-first
  rule as Phase 2: read the originating spec section, read every
  CLAUDE.md the task touches, write the test alongside the
  implementation, run `make ci` before committing.
- **Pick one task at a time.** Read the source section it references
  in `004-phase-5.md` and the corresponding entry in
  `specs/roadmap.md` before starting.
- **Commit message format:** `004-P5.<workstream>.<task>: <title>` —
  e.g. `004-P5.2.1: DmxTransport trait + ArtNetTransport impl`.
- **Branching:** one branch per task; merge straight to `main` once CI
  is green.
- **Pre-commit hook** (`.githooks/pre-commit`, wired by `make setup`)
  runs rustfmt on staged files + `cargo check`. Heavier checks live in
  `make ci`; run that before opening a PR.
- **Tests:** every task ships with new or updated tests. For schema /
  Mutation / snapshot work, follow the v3 proptest pattern in
  `src/project/command.rs`. For render-path work (W4.1), add a golden
  under `tests/golden/` covered by `--features gpu-tests`; use
  `UPDATE_GOLDEN=1` to (re-)record the baseline. Where automation isn't
  possible (fixture-group editor UX, canvas-region drag), ship a manual
  smoke-test checklist — never nothing.
- **Read the right CLAUDE.md.** Tasks touching `src/project/` must read
  `src/project/CLAUDE.md` first (Mutation Reverse-storage rules,
  snapshot invariants). Tasks touching `src/render/` must read
  `src/render/CLAUDE.md` first (GPU lifecycle, panic_restore, build-time
  WGSL validation).
- **Don't bundle.** If a task tempts you to also fix something nearby,
  resist — that "something nearby" probably has its own task ID below.
- **GPU tasks ship golden images.** Anything that touches `src/render/`
  and renders pixels needs a `tests/golden/` baseline added under
  `--features gpu-tests`; `UPDATE_GOLDEN=1` rewrites it.
- **Lighting feature gate.** All Phase 5 code compiles only under
  `--features lighting`. `cargo build --no-default-features` must
  succeed at every commit. The `lighting` feature is **off by default**
  (mirrors `audio`). Tasks that gate new code must verify this CI
  invariant.
- **Decision-doc references.** Tasks that implement a decision reference
  the doc explicitly in the `Depends on:` field. Implementers must read
  the referenced decision doc before starting.

## Task ID conventions

IDs are flat-numbered within eleven workstreams:
- W0 — Decision docs (complete; listed for traceability)
- W1 — Setup + housekeeping
- W2 — Transport layer (Art-Net; `DmxUniverse`; background thread)
- W3 — Fixture model (`FixtureGroup`, `PixelMap`; personality; Mutation)
- W4 — Colour-from-pixel sampling (lighting tap; readback; conversion)
- W5 — Subscriber list (Blackout / Go-live fan-out)
- W6 — Zone-derived fixture binding (depends on Phase 3 zones)
- W7 — BPM-locked fixture chases
- W8 — Output panel UI (fixture-group editor; colour-from-pixel mapping)
- W9 — Diagnostics (DMX activity LED; packet-rate badge)
- W10 — Snapshot / proptest / packet-capture acceptance test
- W11 — Release housekeeping + 5-minute acceptance smoke

---

## W0 — Decision docs

### P5.0.1 — DMX transport decision
**Source:** `004-phase-5.md` (Engine implications — transport)
**Type:** decision record
**Depends on:** —
**Files:** `specs/004-phase-5-dmx-transport-decision.md`
**What:** Choose Art-Net vs sACN; evaluate `artnet_protocol` and `sacn`
crates; define `DmxTransport` trait shape; specify threading model.
**Acceptance:** Doc written; recommendation justified with crate
maintenance evidence; threading model matches `OscSource` precedent.
**Status:** complete.

### P5.0.2 — Fixture personality format decision
**Source:** `004-phase-5.md` (Fixture model)
**Type:** decision record
**Depends on:** —
**Files:** `specs/004-phase-5-fixture-personality-decision.md`
**What:** Define minimal `FixturePersonality` + `ChannelRole` for Phase
5 RGB output; explain what is deferred to Phase 7 RGBW; state the
extension contract.
**Acceptance:** Doc written; `Vec<ChannelRole>` design justified;
Phase 7 extension path is additive (no migration).
**Status:** complete.

### P5.0.3 — Colour-space + readback decision
**Source:** `004-phase-5.md` (Color-from-pixel sampling)
**Type:** decision record
**Depends on:** —
**Files:** `specs/004-phase-5-color-space-decision.md`
**What:** Resolve GPU texture readback strategy (lighting-tap downsample
chosen); fix Phase 5 colour-space scope (RGB direct + HSV intensity
gate); define `sample_and_convert` API; state Phase 7 extension
contract.
**Acceptance:** Doc written; readback strategy chosen with frame-budget
justification; API shape pinned; RGBW deferral explicit.
**Status:** complete.

---

## W1 — Setup + housekeeping

### P5.1.1 — Glossary entries for Phase 5 domain terms
**Source:** `004-phase-5.md` (Capability set; Fixture model)
**Type:** housekeeping
**Depends on:** P5.0.1, P5.0.2, P5.0.3
**Files:** `src/glossary.rs` (or wherever glossary entries live),
`docs/glossary.md` if it exists
**What:** Add glossary entries for: `DmxUniverse`, `FixtureGroup`,
`PixelMap`, `DmxTransport`, `ArtNetTransport`, `ChannelRole`,
`ColorStrategy`, `LightingTap`, `OutputStrategy`. Match the existing
entry format (term + one-sentence definition + phase-introduced
annotation).
**Acceptance:** All nine terms present in the glossary; `make ci`
passes; no new warnings.

### P5.1.2 — Perf-gate refresh: lighting-frame-budget stub fixture
**Source:** `004-phase-5.md` (Acceptance criteria — frame budget)
**Type:** housekeeping / test
**Depends on:** —
**Files:** existing perf-gate test file (wherever P2.1.2 lives)
**What:** Add a stub assertion: "with up to 16 DMX universes queued and
sent per frame, render frame time stays below N ms." For Phase 5 the
test is a placeholder that succeeds trivially (lighting is not yet
wired). It documents the budget constraint and will fail if W2/W4 later
violate it.
**Acceptance:** Stub test present; `make ci` passes.

### P5.1.3 — CHANGELOG + README v0.9 placeholders
**Source:** Phase 5 plan
**Type:** housekeeping
**Depends on:** —
**Files:** `CHANGELOG.md`, `README.md`
**What:** Add `## [0.9.0] — unreleased` section to CHANGELOG; add
"Lighting output" subsection placeholder to README. Mirror P2.1.3 shape.
**Acceptance:** Placeholders present; no broken links; `make ci` passes.

### P5.1.4 — `lighting` cargo feature gate skeleton
**Source:** `CLAUDE.md` (Cargo features: `audio`, `midi`, `osc` pattern)
**Type:** setup
**Depends on:** —
**Files:** `Cargo.toml`, `src/lib.rs` (or `src/main.rs`)
**What:** Add `lighting = ["dep:artnet_protocol"]` to `[features]` in
`Cargo.toml` with `artnet_protocol` as an optional dependency (version
pinned; MIT verified per P5.0.1). Add a `#[cfg(feature = "lighting")]
pub mod lighting;` stub in `src/lib.rs` pointing at an empty
`src/lighting/mod.rs`. Verify `cargo build --no-default-features` and
`cargo build --features lighting` both succeed.
**Acceptance:** `cargo build --no-default-features` succeeds; `cargo
build --features lighting` succeeds; `make lint` clean.

---

## W2 — Transport layer

### P5.2.1 — `DmxTransport` trait + `ArtNetTransport` impl
**Source:** `004-phase-5-dmx-transport-decision.md` (Architecture)
**Type:** new module
**Depends on:** P5.1.4; `004-phase-5-dmx-transport-decision.md`
**Files:** `src/lighting/transport.rs`
**What:** Define the `DmxTransport` trait (`send_universe`, `&mut self`,
`universe: u16`, `data: &[u8; 512]`, `Result<(), LightingError>`).
Implement `ArtNetTransport`: holds a `UdpSocket` bound to 0.0.0.0:0,
sends Art-Net `ArtDmx` PDUs via `artnet_protocol::ArtCommand::Output`.
Implement `NullTransport` (no-op, for tests). Derive `LightingError`
(simple enum: `Io(std::io::Error)`).
**Acceptance:** Unit test sends to a local `UdpSocket` loopback and
decodes the packet back with `artnet_protocol`; `NullTransport` sends
silently; `make ci` passes under `--features lighting`.

### P5.2.2 — `DmxUniverse` newtype + channel type
**Source:** `004-phase-5.md` (Data structures: `DmxUniverse`)
**Type:** data model
**Depends on:** P5.2.1
**Files:** `src/lighting/universe.rs`
**What:** Define `DmxUniverse(pub [u8; 512])` with `Default`, `Clone`,
`index` helpers (`channel_mut(offset: u8) -> &mut u8`). Define
`UniverseId(pub u16)` newtype. Define `UniverseFrame { id: UniverseId,
data: DmxUniverse }` as the crossbeam channel payload type.
**Acceptance:** Unit tests: default is all-zero; channel mutation writes
to correct byte; `make ci` passes.

### P5.2.3 — `LightingThread` background loop
**Source:** `004-phase-5-dmx-transport-decision.md` (Architecture —
threading model); `src/controls/osc.rs` (precedent)
**Type:** background thread
**Depends on:** P5.2.1, P5.2.2
**Files:** `src/lighting/thread.rs`
**What:** Implement `LightingThread` mirroring `OscSource`:
- `start(transport: impl DmxTransport, dest: SocketAddr) ->
  (LightingThread, Sender<UniverseFrame>)`.
- Background thread: drains channel of all pending `UniverseFrame`s
  each tick (keeps only the latest per universe ID); sends via
  `transport.send_universe`; sleeps to maintain ~44 Hz cadence.
- `stop: Arc<AtomicBool>` checked between ticks.
- Drop impl: sets stop flag, joins thread.
- Render thread calls `tx.try_send(frame)` — if full (bounded 4),
  drops silently and continues. Frame time is never blocked.
**Acceptance:** Integration test: spawn thread with `NullTransport`;
send 100 frames rapidly; verify thread drains cleanly and join succeeds
within 200 ms; `make ci` passes.

### P5.2.4 — `LightingThread` wired into `EditingState` / `GoLive`
**Source:** `004-phase-5.md` (Engine implications — transport on
background thread; Start on Go-live)
**Type:** integration
**Depends on:** P5.2.3; `src/app.rs` (`EnterGoLive` / `ExitGoLive`
dispatch)
**Files:** `src/app.rs`, `src/lighting/mod.rs`
**What:** Add `lighting_thread: Option<LightingThread>` to
`EditingState` (behind `#[cfg(feature = "lighting")]`). On
`EnterGoLive` start the thread with the configured Art-Net destination
(from project settings, defaulting to broadcast `255.255.255.255:6454`).
On `ExitGoLive` call `lighting_thread.take()` (Drop stops it). Verify
`cargo build --no-default-features` still compiles without the field.
**Acceptance:** With `--features lighting`: thread starts on Go-live,
stops on exit; packet visible on loopback; frame budget unchanged;
`make ci` passes.

---

## W3 — Fixture model

### P5.3.1 — `ChannelRole`, `FixturePersonality`, `FixtureGroup` structs
**Source:** `004-phase-5-fixture-personality-decision.md` (Decision);
`004-phase-5.md` (Data structures: `FixtureGroup`, `PixelMap`)
**Type:** data model + schema
**Depends on:** P5.1.4; `004-phase-5-fixture-personality-decision.md`
**Files:** `src/lighting/fixture.rs`, `src/project/schema.rs` (or
equivalent)
**What:** Define:
```rust
ChannelRole { Red, Green, Blue }  // #[non_exhaustive], serde snake_case
FixturePersonality { channels: Vec<ChannelRole>, label: String }
FixtureGroup {
    id: FixtureGroupId,           // newtype Uuid
    label: String,
    personality: FixturePersonality,
    universe_id: UniverseId,
    base_channel: u8,             // 0-indexed DMX start address
    fixture_count: u8,
    output_strategy: OutputStrategy,
    source: FixtureSource,        // CanvasRegion | ManualColor
}
FixtureSource::CanvasRegion { uv_min: (f32,f32), uv_max: (f32,f32) }
FixtureSource::ManualColor { r: u8, g: u8, b: u8 }
```
All types: `Debug, Clone, Serialize, Deserialize`. `FixtureGroup` joins
the project schema as `project.fixture_groups: Vec<FixtureGroup>`.
**Acceptance:** Roundtrip serde test (JSON → struct → JSON → struct,
assert eq); `make ci` passes; new schema field has `#[serde(default)]`
so existing projects load without it.

### P5.3.2 — `PixelMap` struct
**Source:** `004-phase-5.md` (Data structures: `PixelMap`)
**Type:** data model
**Depends on:** P5.3.1
**Files:** `src/lighting/fixture.rs`
**What:** `PixelMap` describes a grid of sample points across a canvas
region for a fixture group: `{ rows: u8, cols: u8 }`. A `FixtureGroup`
with `source: CanvasRegion` uses `PixelMap` to subdivide the region into
`rows × cols` sample coordinates. `sample_uvs(group: &FixtureGroup,
pixel_map: &PixelMap) -> Vec<(f32, f32)>` computes the UV list.
**Acceptance:** Unit test: `PixelMap { rows: 2, cols: 2 }` over a
`[0,0..1,1]` region produces four corners; `make ci` passes.

### P5.3.3 — `Mutation::AddFixtureGroup` + reverse
**Source:** `src/project/CLAUDE.md` (Mutation Reverse-storage rules)
**Type:** schema + Mutation
**Depends on:** P5.3.1
**Files:** `src/project/command.rs`, `src/project/mod.rs`
**What:** Add `Mutation::AddFixtureGroup { group: FixtureGroup }` with
reverse `RemoveFixtureGroup { id: FixtureGroupId }`. Implement
`apply_mutation` arm: push `group` into `project.fixture_groups`.
Reverse arm: remove by ID. Follow the whole-enum Reverse-storage rule.
**Acceptance:** Proptest roundtrip: add → undo → check groups unchanged;
`make ci` passes.

### P5.3.4 — `Mutation::RemoveFixtureGroup` + reverse
**Source:** `src/project/CLAUDE.md`
**Type:** Mutation
**Depends on:** P5.3.3
**Files:** `src/project/command.rs`
**What:** `RemoveFixtureGroup { id }` reverse stores the full removed
`FixtureGroup` (whole-enum rule). Apply: remove by ID. Reverse: re-
insert at original position.
**Acceptance:** Proptest: remove → undo → group back at same index;
`make ci` passes.

### P5.3.5 — `Mutation::SetFixtureGroupParams` + reverse
**Source:** `src/project/CLAUDE.md`
**Type:** Mutation
**Depends on:** P5.3.3
**Files:** `src/project/command.rs`
**What:** `SetFixtureGroupParams { id, params: FixtureGroupParams }`
where `FixtureGroupParams` is a flat struct of all mutable fields
(label, personality, universe_id, base_channel, fixture_count,
output_strategy, source). Reverse stores the old `FixtureGroupParams`.
**Acceptance:** Proptest: mutate label → undo → original label; mutate
universe_id → undo → original; `make ci` passes.

### P5.3.6 — DMX-frame builder
**Source:** `004-phase-5-fixture-personality-decision.md` (Extension
contract); `004-phase-5.md` (Engine implications)
**Type:** logic
**Depends on:** P5.3.1, P5.2.2
**Files:** `src/lighting/dmx_frame.rs`
**What:** `build_universe_frame(groups: &[FixtureGroup],
colors: &[(FixtureGroupId, SampledColor)]) ->
HashMap<UniverseId, DmxUniverse>`. Iterates groups; for each group
iterates `fixture_count` fixture slots; iterates `personality.channels`
with `enumerate`; matches `ChannelRole` to write `r`/`g`/`b` bytes at
`base_channel + fixture_offset + channel_offset`. Groups on the same
universe accumulate into the same `DmxUniverse`.
**Acceptance:** Unit tests: single RGB fixture, 3-channel personality,
verify bytes at correct offsets; two fixtures on same universe, verify
no clobbering; `make ci` passes.

---

## W4 — Colour-from-pixel sampling

### P5.4.1 — Lighting-tap texture + downsample render pass
**Source:** `004-phase-5-color-space-decision.md` (Readback strategy S3)
**Type:** render graph
**Depends on:** P5.1.4; `004-phase-5-color-space-decision.md`
**Files:** `src/render/lighting_tap.rs`, `src/render/mod.rs` (render
graph wiring), `src/render/shaders/lighting_tap.wgsl`
**What:** Add a `LightingTapPass` to the render graph, executing after
the compositor pass. The pass blits the composited output texture to a
64×36 `RGBA8Unorm` texture via a simple fullscreen blit shader. Must be
wrapped in `panic_restore`. Shader validated at build time via
`build.rs` (existing naga validation path). Read `src/render/CLAUDE.md`
before touching the render graph.
**Acceptance:** GPU golden image of the 64×36 tap texture recorded under
`tests/golden/lighting_tap_*`; `UPDATE_GOLDEN=1` records the baseline;
`--features gpu-tests` passes; `make ci` passes.

### P5.4.2 — Staging buffer + readback in lighting thread
**Source:** `004-phase-5-color-space-decision.md` (W4.2 note: poll on
lighting thread, not render thread)
**Type:** GPU readback
**Depends on:** P5.4.1, P5.2.3
**Files:** `src/lighting/readback.rs`
**What:** Allocate a `wgpu::Buffer` (`MAP_READ | COPY_DST`, size = 64 ×
36 × 4). Each frame: render thread calls `encoder.copy_texture_to_buffer`
(lighting-tap → staging buffer), submits, and sends a "readback queued"
signal on a one-shot channel to the lighting thread. Lighting thread
calls `device.poll(Maintain::Wait)` (on the lighting thread), maps the
buffer, extracts `[u8; 9216]`, then unmaps. Render thread never calls
`device.poll`. The wgpu `Device` must be `Arc`'d and shared safely
(verify via `Send + Sync` bounds; wgpu devices are `Send`).
**Acceptance:** Integration test: render a solid-red frame; verify
readback buffer contains non-zero R values in the expected positions;
lighting thread receives and maps cleanly; `make ci` passes.

### P5.4.3 — `LightingTapBuffer` + `sample_and_convert`
**Source:** `004-phase-5-color-space-decision.md` (Conversion API)
**Type:** logic
**Depends on:** P5.4.2
**Files:** `src/lighting/color.rs`
**What:** `LightingTapBuffer([u8; 9216])` newtype. `sample_and_convert(
tap: &LightingTapBuffer, uv: (f32, f32), strategy: ColorStrategy) ->
SampledColor` implementation:
- Clamp UV to [0,1]; compute pixel index `(u * 63) as usize + (v * 35)
  as usize * 64`; read `(r, g, b)` bytes.
- `ColorStrategy::RgbDirect` — return as-is.
- `ColorStrategy::HsvIntensityGate` — convert to HSV; scale `r, g, b`
  by `V`; return scaled values.
**Acceptance:** Unit tests: `RgbDirect` returns exact bytes;
`HsvIntensityGate` on a black pixel returns `(0, 0, 0)`; on a white
pixel returns `(255, 255, 255)`; `make ci` passes.

### P5.4.4 — Per-fixture sample budget enforcement
**Source:** `004-phase-5.md` (Usability rule: performance)
**Type:** logic + guard
**Depends on:** P5.4.3, P5.3.2
**Files:** `src/lighting/color.rs`
**What:** `budget_samples(group: &FixtureGroup, pixel_map: &PixelMap,
tap: &LightingTapBuffer, strategy: ColorStrategy) -> SampledColor`
averages the `rows × cols` UV sample results from `sample_and_convert`.
Enforce a maximum of 256 samples per fixture group per frame (a group
with more than 256 pixels in its `PixelMap` is clamped; emit a
`tracing::warn!` once). This prevents a degenerate config from
exhausting CPU time in the lighting thread.
**Acceptance:** Unit test: `PixelMap { rows: 16, cols: 16 }` = 256
samples (no clamp); `rows: 17, cols: 17` is clamped and warns; result
is averaged correctly; `make ci` passes.

### P5.4.5 — Lighting thread sampling + DMX-frame send loop
**Source:** `004-phase-5.md` (Engine implications — color-from-pixel)
**Type:** integration
**Depends on:** P5.4.4, P5.3.6, P5.2.3
**Files:** `src/lighting/thread.rs`
**What:** Extend the lighting thread loop: after readback (P5.4.2),
iterate `fixture_groups` from the shared state snapshot (sent via the
channel alongside the readback signal); call `budget_samples` per group;
collect `(FixtureGroupId, SampledColor)` list; call
`build_universe_frame`; send resulting `UniverseFrame`s via
`transport.send_universe`. The state snapshot must be cheaply cloneable
and sent on the channel (not locked behind a mutex on the render thread).
**Acceptance:** Integration test with `NullTransport` and a known-colour
tap buffer: verify correct DMX bytes in the captured output; `make ci`
passes.

---

## W5 — Subscriber list for Blackout / Go-live fan-out

### P5.5.1 — `LightSubscriber` trait + subscriber list in `EditingState`
**Source:** `004-phase-5.md` (Engine implications — fan-out subscriber
list); `src/app.rs` (`apply_command`)
**Type:** architecture
**Depends on:** P5.2.4
**Files:** `src/lighting/subscriber.rs`, `src/app.rs`
**What:** Define `trait LightSubscriber: Send { fn on_blackout(&mut
self); fn on_go_live(&mut self); fn on_exit_live(&mut self); }`. Add
`light_subscribers: Vec<Box<dyn LightSubscriber>>` to `EditingState`
(behind `#[cfg(feature = "lighting")]`). `LightingThread` implements
`LightSubscriber`: `on_blackout` sends a zero `DmxUniverse` for all
configured universes; `on_go_live` arms output; `on_exit_live` stops
the thread.
**Acceptance:** Unit test with a mock subscriber: verify call sequence;
`make ci` passes.

### P5.5.2 — Wire `Command::Blackout` to fan-out subscribers
**Source:** `004-phase-5.md` (Wire `Command::Blackout` to fan-out
subscribers — same frame as visual change)
**Type:** integration
**Depends on:** P5.5.1
**Files:** `src/app.rs` (`apply_command`, `Command::Blackout` arm)
**What:** In the `Command::Blackout` arm (after the existing visual
blackout), iterate `editing_state.light_subscribers` and call
`on_blackout()` on each. The call happens in the same frame as the
visual state flip. The existing `OutputState::blackout` toggle is
unchanged.
**Acceptance:** Manual test: with Art-Net listener (e.g. Wireshark on
port 6454) active, press `B` — DMX zeros arrive within the same frame's
output. Unit test with mock subscriber verifies call; `make ci` passes.

### P5.5.3 — Wire `EnterGoLive` / `ExitGoLive` to fan-out subscribers
**Source:** `004-phase-5.md` (Go-live as a fan-out event)
**Type:** integration
**Depends on:** P5.5.1
**Files:** `src/app.rs` (`EnterGoLive` / `ExitGoLive` transition arms)
**What:** On `EnterGoLive` transition: after the existing visual state
flip, call `on_go_live()` on each subscriber (arms lighting output). On
`ExitGoLive`: call `on_exit_live()` (stops lighting thread, sends zeros
as a courtesy). Both fan-out calls happen in the same frame as the
state transition.
**Acceptance:** Unit test with mock subscriber; `make ci` passes.

---

## W6 — Zone-derived fixture binding

### P5.6.1 — `FixtureSource::ZoneTag` variant + schema
**Source:** `004-phase-5.md` (Capability set — zone-derived accent
output; fixture group references zone tag)
**Type:** schema
**Depends on:** P5.3.1; Phase 3 zones must be shipped (external
dependency — this task is blocked until Phase 3's zone schema lands)
**Files:** `src/lighting/fixture.rs`, `src/project/schema.rs`
**What:** Add `FixtureSource::ZoneTag { tag: ZoneTag }` variant to the
`FixtureSource` enum. When this variant is active, the fixture group's
sampled colour is derived from the zone's current activity level (as
defined by Phase 3's `light-source` / `highlight` zone semantics) rather
than a canvas region. `ZoneTag` is a string newtype (mirrors Phase 3's
tag type; import from that module once it lands).
**Acceptance:** Serde roundtrip includes the new variant; existing
`CanvasRegion` fixtures unaffected; `make ci` passes. If Phase 3 is not
yet landed, this task can be stubbed with a `todo!()` in the sampling
path.

### P5.6.2 — Zone-activity → DMX intensity mapping
**Source:** `004-phase-5.md` (Zone-derived accent output: fixture
intensity follows `light-source` / `highlight` zone activity)
**Type:** logic
**Depends on:** P5.6.1; Phase 3 zone-activity API
**Files:** `src/lighting/color.rs`
**What:** `zone_activity_to_color(activity: f32, strategy:
ColorStrategy) -> SampledColor` where `activity` ∈ [0.0, 1.0] is the
zone's normalised light-source or highlight intensity. For `RgbDirect`:
returns `(255 * activity, 255 * activity, 255 * activity)` (white wash
scaled by activity). For `HsvIntensityGate`: same semantics. Plugs into
the `budget_samples` call path when the source variant is `ZoneTag`.
**Acceptance:** Unit tests: `activity = 0.0` → all zeros; `activity =
1.0` → all 255; `activity = 0.5` → 127/128; `make ci` passes.

---

## W7 — BPM-locked fixture chases

### P5.7.1 — `FixtureChase` data model + schema
**Source:** `004-phase-5.md` (BPM-locked fixture chases driven by
existing `Modulator::Bpm`)
**Type:** data model
**Depends on:** P5.3.1
**Files:** `src/lighting/chase.rs`, `src/project/schema.rs`
**What:** Define `FixtureChase { id: FixtureChaseid, label: String,
group_id: FixtureGroupId, steps: Vec<ChaseStep>, beat_divisor: u8 }`.
`ChaseStep { color: (u8, u8, u8), hold_beats: u8 }`. `beat_divisor`
divides the BPM tick (1 = one step per beat; 2 = half a beat; 4 = a
quarter). Add `project.fixture_chases: Vec<FixtureChase>` with
`#[serde(default)]`.
**Acceptance:** Serde roundtrip test; existing projects load without
`fixture_chases`; `make ci` passes.

### P5.7.2 — `Mutation::AddFixtureChase` + reverse
**Source:** `src/project/CLAUDE.md` (Mutation Reverse-storage rules)
**Type:** Mutation
**Depends on:** P5.7.1
**Files:** `src/project/command.rs`
**What:** `AddFixtureChase { chase: FixtureChase }` with reverse
`RemoveFixtureChase { id }`. Apply: push chase into
`project.fixture_chases`. Reverse: remove by ID. Whole-enum Reverse-
storage rule applies.
**Acceptance:** Proptest roundtrip: add → undo → chases unchanged;
`make ci` passes.

### P5.7.3 — `Mutation::RemoveFixtureChase` + reverse
**Source:** `src/project/CLAUDE.md`
**Type:** Mutation
**Depends on:** P5.7.2
**Files:** `src/project/command.rs`
**What:** `RemoveFixtureChase { id }` reverse stores the full removed
`FixtureChase`. Apply: remove by ID. Reverse: re-insert at original
index position.
**Acceptance:** Proptest: remove → undo → chase back at same index;
`make ci` passes.

### P5.7.4 — `Mutation::SetFixtureChaseParams` + reverse
**Source:** `src/project/CLAUDE.md`
**Type:** Mutation
**Depends on:** P5.7.2
**Files:** `src/project/command.rs`
**What:** `SetFixtureChaseParams { id, params: FixtureChaseParams }`
where `FixtureChaseParams` covers all mutable fields (label, steps,
beat_divisor). Reverse stores old params.
**Acceptance:** Proptest: mutate beat_divisor → undo → original value;
mutate steps → undo → original; `make ci` passes.

### P5.7.5 — Chase ticker: `Modulator::Bpm` integration
**Source:** `004-phase-5.md` (BPM-locked chases driven by
`Modulator::Bpm`)
**Type:** logic
**Depends on:** P5.7.1, P5.4.5; existing `Modulator::Bpm` clock
**Files:** `src/lighting/chase.rs`
**What:** `ChaseTicker::advance(bpm: f32, dt: f32) -> Option<usize>`
returns the current step index (or `None` if no BPM is set). Integrates
beat phase from `dt` and the current BPM, advances step index when the
beat-phase crosses a `beat_divisor` boundary. Plugged into the lighting
thread loop: each tick, derive the current step colour for each active
chase and override the fixture group's colour before DMX-frame building.
**Acceptance:** Unit tests: at 120 BPM with `beat_divisor = 1`, step
advances every 500 ms; `make ci` passes.

---

## W8 — Output panel UI

### P5.8.1 — Output panel "Lighting" section skeleton
**Source:** `004-phase-5.md` (Recommendation K follow-on: Output panel
grows fixture-group editor)
**Type:** UI
**Depends on:** P5.3.1, P5.2.4
**Files:** `src/ui/output_panel.rs` (or wherever the Output panel lives)
**What:** Add a collapsible "Lighting" section to the Output panel (behind
`#[cfg(feature = "lighting")]`). Initially shows: Art-Net destination IP
+ port text field (stored in project settings), "Start lighting on Go-
live" checkbox, and a placeholder "No fixture groups — add one below".
Follow the panel docking model (Recommendation D: all new surfaces dock
into the right-side region).
**Acceptance:** UI renders without warnings; settings round-trip through
the project (serde); `make ci` passes.

### P5.8.2 — Fixture-group list + add/remove in Output panel
**Source:** `004-phase-5.md` (Output panel grows fixture-group editor)
**Type:** UI
**Depends on:** P5.8.1, P5.3.3, P5.3.4
**Files:** `src/ui/output_panel.rs`
**What:** Render `project.fixture_groups` as a list with per-row:
group label (editable), universe selector (u16 spinner), base channel
(u8 spinner), fixture count (u8 spinner), and a delete button. "+ Add
fixture group" button dispatches `Command` → `Mutation::AddFixtureGroup`
with a default RGB 3-channel personality. Delete button dispatches
`Mutation::RemoveFixtureGroup`. All mutations go through the undo stack.
**Acceptance:** Add a group → appears in list; undo → disappears; redo →
re-appears; `make ci` passes.

### P5.8.3 — Fixture personality editor in the group row
**Source:** `004-phase-5-fixture-personality-decision.md` (Phase 5
personality model)
**Type:** UI
**Depends on:** P5.8.2, P5.3.5
**Files:** `src/ui/output_panel.rs`
**What:** Expand each fixture-group row to include a personality sub-
section: label field, channel count (derived from `channels.len()`),
per-channel role dropdown (Red / Green / Blue; other roles greyed with
"Phase 7" `ModeHintBanner`). Editing dispatches
`Mutation::SetFixtureGroupParams`.
**Acceptance:** Change a channel role → reflected in project JSON;
undo restores; `make ci` passes.

### P5.8.4 — Canvas-region drag-to-assign in Output panel
**Source:** `004-phase-5.md` (Usability rule: drag a region of the
canvas onto a fixture group and watch it light up)
**Type:** UI
**Depends on:** P5.8.2, P5.4.5
**Files:** `src/ui/output_panel.rs`, `src/ui/canvas.rs` (or equivalent)
**What:** Each fixture-group row shows a "Canvas region" sub-section:
two UV coordinate pairs (min/max) shown as percentage values, and a
"Select region…" button that enters a region-drawing mode on the canvas
(draw a rect → UV coords are written back). Dispatches
`Mutation::SetFixtureGroupParams` with updated `FixtureSource::
CanvasRegion`. This is the "five-minute operator story" acceptance
criterion from Phase 5.
**Acceptance:** Manual smoke test: drag a region over a coloured
canvas area → fixture colour follows; annotated in the Phase 5
acceptance smoke (P5.11.1).

---

## W9 — Diagnostics

### P5.9.1 — DMX universe activity LED in diagnostics chrome
**Source:** `004-phase-5.md` (Diagnostics: DMX universe activity LED);
roadmap N5 follow-on
**Type:** UI
**Depends on:** P5.2.3, P5.2.4
**Files:** `src/ui/diagnostics.rs` (or wherever the fps badge lives)
**What:** Add a "DMX" activity LED to the existing diagnostics strip
(next to fps + panic-restore badge per N5 Capability follow-on). The
LED is green when packets are being sent (within the last 2 seconds) and
grey otherwise. The lighting thread sets an `Arc<AtomicBool>` "active"
flag on each successful `send_universe`; the render thread reads it and
renders the badge. Behind `#[cfg(feature = "lighting")]`.
**Acceptance:** With lighting active during Go-live: badge is green.
After ExitGoLive: badge goes grey within ~2 s. `make ci` passes.

### P5.9.2 — Art-Net packet-rate badge in diagnostics chrome
**Source:** `004-phase-5.md` (Diagnostics: Art-Net packet rate badge)
**Type:** UI
**Depends on:** P5.9.1
**Files:** `src/ui/diagnostics.rs`, `src/lighting/thread.rs`
**What:** Track per-second packet count in the lighting thread (an
`Arc<AtomicU64>` incremented per send; read and reset each second by the
diagnostics render path). Display as "DMX: 44 pkt/s" next to the
activity LED.
**Acceptance:** Rate badge shows ~44 pkt/s with one universe; ~704 pkt/s
with 16 universes (44 Hz × 16); `make ci` passes.

---

## W10 — Snapshot / proptest / packet-capture acceptance test

### P5.10.1 — Snapshot integration: `LightCue` in project snapshot
**Source:** `004-phase-5.md` (Snapshot integration: light cues authored
in parallel to video cues share the same scene snapshot)
**Type:** schema + snapshot
**Depends on:** P5.3.1; `src/project/CLAUDE.md` (snapshot invariants)
**Files:** `src/project/schema.rs`, `src/project/snapshot.rs` (or
equivalent)
**What:** Extend the project snapshot to include lighting state: each
scene snapshot stores a `LightCueSnapshot { fixture_group_overrides:
Vec<(FixtureGroupId, ManualColor)> }` in addition to the existing layer
snapshot. `restore_scene` restores fixture overrides alongside layer
state. Read `src/project/CLAUDE.md` before touching `restore_scene` vs
`restore` semantics.
**Acceptance:** Proptest: snapshot with fixture overrides → restore →
overrides are back; snapshots with no lighting data deserialise cleanly
(backward compat); `snapshots_share_layer_topology` gating unchanged;
`make ci` passes.

### P5.10.2 — Proptest extension: fixture-group Mutation round-trips
**Source:** `src/project/CLAUDE.md` (proptest pattern in
`src/project/command.rs`)
**Type:** proptest
**Depends on:** P5.3.3, P5.3.4, P5.3.5, P5.7.2, P5.7.3, P5.7.4
**Files:** `src/project/command.rs` (proptest section)
**What:** Extend the existing proptest harness with strategies for
`FixtureGroup`, `FixturePersonality`, `ChannelRole`, `FixtureChase`,
`ChaseStep`. Verify: `AddFixtureGroup` → `RemoveFixtureGroup` (reverse)
is a no-op; `SetFixtureGroupParams` → undo → original; all at 1000
cases each.
**Acceptance:** All proptest cases pass; `make ci` passes.

### P5.10.3 — Packet-capture acceptance test (CI Art-Net listener)
**Source:** `004-phase-5.md` (Acceptance criteria: Blackout blacks both
projector and fixtures; verified with packet capture against an Art-Net
listener fixture in CI)
**Type:** integration test
**Depends on:** P5.5.2, P5.2.1, P5.2.3
**Files:** `tests/artnet_blackout.rs`
**What:** Integration test: spawn a loopback `UdpSocket` on port 6454;
create a `LightingThread` with `ArtNetTransport` targeting `127.0.0.1:
6454`; send a non-zero `UniverseFrame`; verify packet received with
correct `ArtDmx` opcode and payload; send a Blackout signal; verify
next packet is all-zero `DmxUniverse`; verify sequence numbers increment
monotonically. Does not require a real Art-Net node — loopback only.
**Acceptance:** Test passes on CI (no hardware required); `make ci`
passes.

---

## W11 — Release housekeeping + acceptance smoke

### P5.11.1 — Phase 5 acceptance smoke test (manual)
**Source:** `004-phase-5.md` (Acceptance criteria — all five items)
**Type:** manual smoke
**Depends on:** all W2–W9 tasks
**Files:** `docs/show-day-checklist.md` (or a new
`docs/phase-5-smoke.md`)
**What:** Manual checklist verifying all five acceptance criteria from
the Phase 5 spec:
1. Wire an Art-Net node; define a fixture group; sample a canvas region;
   fixture follows canvas colour within 5 minutes.
2. `B` (Blackout) blacks both projector and fixtures in the same frame
   (verified by watching a listener while pressing B).
3. Go-live arms light cues alongside the visual transition; both fire
   on confirm.
4. Show-day frame budget unchanged with up to 16 universes active
   (measure with the fps + DMX packet-rate badges).
5. Diagnostics badge displays DMX universe activity during a show.
**Acceptance:** All five criteria manually verified and noted in the
checklist; results committed alongside the checklist update.

### P5.11.2 — Version bump + CHANGELOG body for v0.9
**Source:** Phase 5 plan; P5.1.3 placeholders
**Type:** housekeeping
**Depends on:** P5.11.1
**Files:** `Cargo.toml`, `CHANGELOG.md`
**What:** Bump version from previous to `0.9.0`. Fill in the
`## [0.9.0]` CHANGELOG section with: transport choice (Art-Net via
`artnet_protocol`), fixture-group editor, colour-from-pixel canvas
sampling, Blackout / Go-live fan-out, BPM-locked chases, diagnostics
badges. Mirror P2.10.2 length and format.
**Acceptance:** `cargo build` succeeds with new version; CHANGELOG body
complete; `make ci` passes.

### P5.11.3 — README — Phase 5 lighting section
**Source:** Phase 5 plan; P5.1.3 placeholder
**Type:** housekeeping
**Depends on:** P5.11.2
**Files:** `README.md`
**What:** Fill in the "Lighting output" README section: Art-Net output,
fixture groups, colour-from-pixel mapping, Blackout/Go-live fan-out,
diagnostics. One paragraph + a short capability list. Mirror P2.10.3.
**Acceptance:** Section present, no broken links; `make ci` passes.

### P5.11.4 — Show-day checklist: lighting pre-show checks
**Source:** `docs/show-day-checklist.md`; Phase 5 plan
**Type:** housekeeping
**Depends on:** P5.11.1
**Files:** `docs/show-day-checklist.md`
**What:** Add a "Lighting output" section to the checklist: verify
Art-Net destination reachable; confirm DMX activity LED is green during
Go-live; verify Blackout kills both surfaces; note the 16-universe cap.
Mirror P2.10.4 format.
**Acceptance:** Section present; `make ci` passes.
