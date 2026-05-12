# Phase 7 — Syphon output integration decision (P7.W0, W2)

**Status:** decision record. The actual integration (render-output readback,
IOSurface share, Syphon server announcement) lands in W2 tasks once this
decision is ratified.

## Constraints

- **macOS-only target** (`CLAUDE.md`: "v1 is macOS-only by design").
- **Syphon is output, not input.** Phase 0's NDI decision covers NDI input;
  this decision covers exposing rmap's rendered output to external receivers
  (OBS, VDMX, MadMapper, Resolume, capture rigs).
- **`cargo bundle --profile release-show` must produce a self-contained `.app`.**
  Any framework we use must ship inside the bundle or be a guaranteed system
  framework. The NDI precedent (separately-installed SDK, `make setup` hint)
  is acceptable only if there is no bundleable alternative.
- **Existing precedent:** `objc2`, `objc2-foundation`, `objc2-app-kit`,
  `objc2-io-kit`, `objc2-av-foundation`, `objc2-core-media`,
  `objc2-core-video` are already in the dependency graph (`Cargo.toml`
  lines 97–130). The pattern is "wrap Apple APIs via the objc2 family; do
  not reintroduce deprecated `cocoa`/`objc` crates."
- **No tokio.** Async is driven through `pollster::block_on`.
- **Cargo-feature gated.** Follows the pattern of `audio`, `midi`, `osc`, and
  `video` features — a default-on `syphon-out` feature; opt out via
  `--no-default-features`.

## Candidates evaluated

### 1. Existing `syphon` crate on crates.io (rejected — unrelated)

The crate published as `syphon` on crates.io (v0.1.0, April 2020) is a
metrics proxy tool — wholly unrelated to the macOS Syphon video-sharing
framework. No search (crates.io, lib.rs, GitHub topic search) surfaced any
maintained Rust binding for the Syphon.framework as of May 2026.

**Verdict:** no Rust Syphon crate exists.

### 2. Raw IOSurface FFI without Syphon.framework (rejected — reimplements Syphon)

`IOSurface` is macOS's inter-process image-sharing primitive; Syphon sits on
top of it and adds:

- A Mach-port-based server-announcement + client-discovery protocol so
  receivers can enumerate available Syphon sources by name.
- A Metal-backed shared-texture contract (the sender writes to a Metal texture
  backed by an IOSurface; the receiver reads from the same IOSurface without
  a copy).
- A per-frame notification mechanism (Core Foundation run-loop callbacks or
  Metal events) that tells receivers a new frame is ready.

Implementing only the IOSurface layer would mean:

- Other Syphon receivers (OBS, VDMX, Resolume) cannot discover rmap's output
  — they rely on the announcement protocol.
- The announcement and notification layers are non-trivial to reimplement
  correctly; doing so is essentially reimplementing Syphon.

The Phase 7 acceptance gate is "a Syphon receiver running OBS captures rmap
output." Without the announcement protocol that gate cannot pass.

**Verdict:** rejected. Pure IOSurface bypasses the most valuable part of
Syphon (discovery) while requiring more code.

### 3. Thin `objc2` wrapper around the bundled Syphon.framework (chosen)

Syphon.framework is BSD-licensed open source
(<https://github.com/Syphon/Syphon-Framework>). The framework can be
**embedded inside the `.app` bundle** (placed under
`rmap.app/Contents/Frameworks/`) — this is how VDMX, MadMapper, and other
bundled apps ship Syphon support without a separate installer step. The
operator install story is identical to the AVFoundation path: "it works."

The public Syphon Metal API surface for a sender is small:

- `SyphonMetalServer` — announce, push frame, stop. Four methods.
- Initialise with a name string and a `MTLDevice`.
- `newFrameImage:` takes a `id<MTLTexture>` backed by an IOSurface (or wraps
  one automatically on Metal, depending on the Syphon version).
- No persistent background thread required — push is synchronous from the
  caller's render thread.

The objc2 crate supports `extern_class!` + `msg_send!` macros for calling
Objective-C classes that aren't in the first-party `objc2-*` crate set. A
thin `src/syphon_out/mod.rs` can wrap the four methods in safe Rust using
the same pattern already in use for IOKit calls in `src/show_day/`.

**Build-time concern:** the linker needs to find `Syphon.framework` at build
time. The Xcode-idiomatic path is `-F$(SRCROOT)/vendor/frameworks` +
`-framework Syphon` in `build.rs`. `cargo build` emits `cargo:rustc-link-*`
to wire this. The framework is checked into `vendor/frameworks/` (binary blob
of reasonable size: Syphon.framework is ~800 KB compiled) or fetched by
`make setup` from the canonical GitHub release.

**Comparison with NDI decision:** NDI required operators to install a
multi-hundred-MB SDK themselves because NewTek's license prohibited
redistribution of the SDK dylibs. Syphon.framework is BSD-licensed and
~800 KB — bundling is both legal and lightweight. The Syphon path is
therefore cleaner than the NDI path, not more complex.

**Cons:**

- Syphon.framework must be kept at a reasonably-current version; the
  maintainers target macOS 10.15+ and follow Apple's annual SDK cycle.
  Pinning the version in `vendor/` and auditing at each annual macOS release
  is the operational cost.
- `extern_class!` + `msg_send!` is less ergonomic than using a first-party
  `objc2-*` crate. The API surface is small enough (~4 methods) that this
  is not a practical burden.
- No Rust type safety for the Objective-C boundary — but the wrapper is
  thin and test-covered at the integration level (W2.4 acceptance gate:
  OBS receives a frame).
- Syphon.framework uses a legacy `NSRunLoop` for its announcement mechanism.
  rmap's main thread already runs the winit event loop, which drives
  `NSRunLoop`; no extra thread is needed, but the interaction must be
  verified at integration time.

## Decision

**Thin `objc2` wrapper around the bundled Syphon.framework.**

Justify the choice in one paragraph: there is no maintained Rust Syphon
crate; the pure-IOSurface path silently fails the primary acceptance gate
(OBS discovery); Syphon.framework is BSD-licensed, small, and bundleable,
making it the only path that satisfies both the self-contained-bundle
requirement and the interop acceptance gate. The wrapping cost is low because
the sender API is four Objective-C methods, and the objc2 `extern_class!`
pattern is already established in the codebase.

## Architecture (for W2 follow-up tasks)

### W2.1 — Vendor Syphon.framework + build.rs linkage

- Add `vendor/frameworks/Syphon.framework/` (pinned release from
  `https://github.com/Syphon/Syphon-Framework/releases`).
- `build.rs`: emit `cargo:rustc-link-search=framework=vendor/frameworks` +
  `cargo:rustc-link-lib=framework=Syphon`.
- `cargo build --no-default-features` must succeed (framework path only
  emitted when `syphon-out` feature is active).
- `make setup` verifies the framework blob is present (via `git lfs` or a
  pre-seeded download step) and emits a hint if not.

### W2.2 — `src/syphon_out/` sender wrapper

- `SyphonServer::new(name: &str, device: &wgpu::Device) -> Result<Self>`.
  Internally calls `[SyphonMetalServer serverWithName:options:device:]`.
- `SyphonServer::publish_frame(texture: &wgpu::Texture)` — extracts the
  underlying `MTLTexture` handle from the wgpu texture via
  `wgpu::Texture::as_hal::<wgpu::Metal>()`, then calls
  `[server newFrameImage:region:time:]`.
- `SyphonServer::stop()` — calls `[server stop]`.
- Feature gated behind `#[cfg(feature = "syphon-out")]`.

### W2.3 — Render pipeline integration

- After the `GammaPipeline` pass (step 5 in `src/render/CLAUDE.md`'s
  per-frame graph), if a `SyphonServer` is active, call
  `publish_frame(&warp_rt_texture)`.
- The server lives on `EditingState`; toggled by a new `Mutation::SetSyphonOut
  { enabled: bool }` (symmetric `ReverseStorage`).
- No new GPU pass required: warp_rt is already the pixel-equivalent of
  projector output (`src/render/CLAUDE.md` §`warp_rt_view`).
- Wraps inside `panic_restore::run_frame_assert_unwind_safe` boundary —
  a Syphon publish panic must drop a frame, not crash the event loop.

### W2.4 — Output panel UI + audit

- Output panel toggle: "Syphon out" checkbox + status label (advertising
  name = `rmap – <project filename>`).
- `AuditKind::SyphonFrameworkMissing` if the framework dylib can't be
  dlopen'd at startup (compile-time linkage catches most cases; runtime
  check covers corrupted bundles).
- Acceptance gate: OBS 30.x with Syphon plugin receives rmap output,
  colour-correct, no frame stutter.

## Acceptance gates

- [ ] `vendor/frameworks/Syphon.framework/` present; `make setup` hints if
      absent.
- [ ] `cargo build --no-default-features` succeeds (framework linkage skipped).
- [ ] `cargo build` (with `syphon-out` default-on) links and runs on macOS 14+.
- [ ] OBS with Syphon plugin discovers rmap by name and receives live output.
- [ ] Colour is correct (no channel shift vs. on-screen output).
- [ ] No frame stutter at 60 fps on M-series hardware.
- [ ] Syphon publish panic is caught by `panic_restore`; next frame recovers.
- [ ] `AuditKind::SyphonFrameworkMissing` surfaces in the audit panel when the
      framework is not loadable.

## Out of scope

- NDI output (separate SDK, separate decision; NDI output is `Phase 7+`).
- Spout (Windows only; out of scope per CLAUDE.md macOS-only constraint).
- Multi-output (>2 projectors): deliberately deferred per Phase 7 plan
  ("optional only if single-surface workflow is already excellent").
- Syphon input (distinct capability; rmap already receives frames via
  the texture-upload queue from other sources).
