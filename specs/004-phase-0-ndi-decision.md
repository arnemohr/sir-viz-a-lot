# Phase 0 — NDI input binding decision (P0.6.1, W6) — deferred to v0.5

**Status:** decision record only. **W6 (NDI input) is deferred from
v0.4 to v0.5** in light of the NewTek SDK's installer +
redistribution-license friction. This document stays on file as
the binding decision that applies whenever the work resumes.

The schema placeholder `LayerKind::Ndi { source_name }` shipped by
P0.1.2 stays in v7 so v0.5 needs no migration when the receiver
lands — this decision avoids paying for NDI's adoption later.

Roadmap §1.1 classifies NDI as "transport, not primary creative
source," so deferring it matches the stated philosophy. The other
v0.4 deliverables (video, FX layer foundations, two-projector,
OSC + MIDI bindings, per-projector colour calibration) carry the
release without it.

## Constraints

- **macOS-only target** through v0.4 (`CLAUDE.md`).
- **Single-machine, single-projector** event-scale tool. NDI input
  is for the case where another machine on the same VLAN runs
  OBS-NDI / Resolume / a vision-mixer and wants its output fed
  into rmap as a layer source.
- **NDI SDK licensing:** redistribution requires NewTek's NDI
  Advanced SDK license. The free runtime is a separate concern.
  v0.4's bundle has to either ship the runtime or require the
  operator to install it; the dev machine needs the SDK headers
  + libs to build against.
- **No system-wide install in `make setup`** beyond what
  `mise.toml` pins. Anything needing a manual SDK download must
  be opt-out via `cargo build --no-default-features`.
- **Existing precedent:** the project's input modules
  (`src/controls/midi.rs`, `src/controls/osc.rs`) ship as cargo
  features. NDI follows the same pattern.

## Candidates evaluated

### 1. `ndi-rs` (rejected — not maintained)

- Last release in 2020. Bindings against an old NDI SDK version.
- API ergonomics OK but unsafe layer is exposed; build-from-source
  has been broken on recent NDI releases.

### 2. `newtek-ndi` (rejected — not maintained, deprecated)

- Older crate; same staleness story.

### 3. `ndi-sdk` (rejected — Windows-only example)

- Examples are Windows-only; macOS path untested.

### 4. **`ndi` crate (Linsmith / community fork)** (chosen)

- **Pros:**
  - Active fork with macOS support and recent NDI SDK 6
    compatibility.
  - Safe-ish wrappers around `NDIlib_*` C calls.
  - Send + Sync where it should be (the receiver handle is
    `Send` so we can move it to a worker thread).
  - Builds against the operator's installed NDI SDK via
    `pkg-config` style discovery.
- **Cons:**
  - First-build requires the NDI SDK installed at
    `/Library/NDI SDK for Apple/`. `make setup` can detect
    its presence and emit an actionable hint pointing at the
    NewTek download page.
  - License: BSD/MIT for the wrapper crate, but the underlying
    SDK is NewTek's. Bundle implications: the NDI runtime
    dylibs ship in NewTek's "NDI Tools" — operators install
    those separately; rmap doesn't redistribute the SDK.
  - `objc2-av-foundation` is the local pattern for Apple-
    framework wraps; the NDI SDK isn't a system framework so
    that pattern doesn't apply.

### 5. Direct FFI against `libndi` (rejected — too much work for v0.4)

- Hand-rolling the bindings is feasible — NDIlib has ~30
  functions for the input path — but reinventing what an
  active community crate already provides is wasted scope.
  Reconsider only if the chosen `ndi` crate becomes
  unmaintained.

## Decision

**The community-maintained `ndi` crate is the chosen path.**

The decision is conservative: pick the actively-maintained Rust
binding that wraps the official NDI SDK, accept that operators
install the SDK separately (detected at `make setup` with an
actionable hint), and ship the receiver behind a default-on
`ndi` cargo feature so default builds satisfy the v0.4 acceptance
("an NDI stream from another machine appears as a selectable
layer source").

`cargo build --no-default-features` opts out for users who can't
install the SDK (CI machines, stripped-down builds).

## Architecture (for the W6.2 / W6.3 follow-up commits)

### W6.2 — `src/ndi_layer/` receiver

- New `src/ndi_layer/mod.rs` mirroring `src/svg_layer/`'s shape:
  - `mod.rs` — public API: `start(source) -> Receiver<...>`.
  - `worker.rs` — receiver loop running on a thread.
- `NdiReceiver::start(source: NdiSourceInfo) -> JoinHandle`:
  - Open a `Recv` against the named source.
  - Loop: `recv_video(timeout=2s)` → on success, push
    `TextureFrame { target, pixels, width, height, format,
    timestamp_nanos }` onto the texture-upload queue
    (P0.3.1).
  - On error or 5 s of no-frame: drop the connection, sleep
    5 s, retry. Update the layer's runtime `connected: bool`
    on every state change (consumed by P0.6.3's audit badge).
- `LayerKind::Ndi { source_name }`: schema field already in
  place from P0.1.2; the receiver looks the source up by name
  via `ndi::find_sources` enumeration.

### W6.3 — audit + UI badge

- `AuditKind::NdiSourceUnavailable { source_name }` added to
  `src/project/audit.rs` (mirror `OutputTargetUuidNotFound`'s
  shape). Emitted at project load when the named source isn't
  enumerable.
- Left-rail layer row reads the runtime `connected: bool` and
  renders a "source unavailable" badge when false. Badge clears
  automatically on reconnect (W6.2's loop owns the reconnect
  state).

## Acceptance gates (for the integration commits)

- [ ] `ndi` crate added behind a new `ndi` cargo feature
      (default-on; opt out via `--no-default-features`).
- [ ] `cargo build --no-default-features` succeeds.
- [ ] `make setup` detects missing NDI SDK and emits an
      actionable hint pointing at the NewTek download page.
- [ ] `ndi::list_sources()` works on a real network with another
      NDI sender visible.
- [ ] Per-layer NDI receiver thread pushes frames onto the
      texture-upload queue.
- [ ] Render path draws live NDI through warp + mask + effects.
- [ ] Reconnect within ~5 s of source returning.
- [ ] `AuditKind::NdiSourceUnavailable` surfaces at project load.
- [ ] `docs/ndi-setup.md` documents the SDK install.

## Out of scope for v0.4

- NDI **output** — Phase 7 (Syphon / Spout / NDI out). Distinct
  capability requiring different render-path inversion (read
  the rendered output back, push to NDI as a sender).
- Audio-track decode from NDI streams (event scope is video-only
  visuals; audio path is `cpal` FFT modulators).
- Per-source frame-rate negotiation; v0.4 takes whatever the
  source sends.
- Multicast NDI discovery configuration; v0.4 uses the SDK
  default (subnet broadcast).
