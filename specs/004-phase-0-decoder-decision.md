# Phase 0 — video decoder library decision (P0.4.1, W4)

**Status:** decision record. The actual integration (decoder thread,
texture-upload-queue producer, render-path wiring) lands in
follow-up commits once the chosen crate's dev environment is set up.

## Constraints

- **macOS-only target** through v0.4 (`CLAUDE.md`: "v1 is macOS-only
  by design").
- **Single-projector, event-scale** show — mp4 / H.264 is sufficient.
  Operator content is event photo/video, not pro broadcast formats.
- **No system-wide deps in `make setup`** beyond what `mise.toml`
  already pins. Anything that requires `brew install`-ing a heavy
  package needs an opt-out.
- **Bundle target:** `cargo bundle --profile release-show` produces
  the `.app`. Anything we link against must ship in the bundle or
  be a guaranteed system framework.
- **Existing precedent:** `objc2`, `objc2-foundation`, `objc2-app-kit`,
  `objc2-io-kit`, `objc2-core-foundation` already in the dependency
  graph. The pattern is "wrap Apple APIs through objc2; never
  reintroduce the deprecated `cocoa` / `objc` crates."

## Candidates evaluated

### 1. `ffmpeg-next` (rejected)

- **Pros:** industry standard; supports every codec we'd ever need
  including H.264, H.265, ProRes, NDI-friendly formats; large
  community.
- **Cons:**
  - Requires `brew install ffmpeg` (~500 MB of dependencies) on
    the dev machine. This is the operator's first-time-setup tax;
    `make setup` can prompt but a lighter alternative is preferable.
  - LGPL / GPL licensing depending on build config —
    bundle / redistribution implications need legal review.
  - Bundle size impact: dynamic linking adds the ffmpeg dylibs
    to the `.app`; static linking explodes the binary.
  - Cross-platform paths are useful for a future Linux/Windows
    port, but v0.4 is macOS-only — we get no immediate benefit
    from the cross-platform code paths.

### 2. `symphonia` + an H.264 crate (rejected)

- **Pros:** symphonia is pure Rust, no system deps.
- **Cons:** symphonia decodes audio only. Pairing it with an H.264
  decoder (e.g. `openh264-sys2` which builds Cisco's OpenH264 from
  source) introduces another C dependency and adds first-build
  complexity. Net not simpler than ffmpeg-next.

### 3. `gstreamer-rs` (rejected)

- **Pros:** mature media framework with rich Rust bindings; pipeline
  abstraction matches our texture-upload model well.
- **Cons:** GNOME stack — GStreamer + GLib + plugin packs total
  several hundred MB of system deps. Same size and licensing
  concerns as ffmpeg without the codec breadth being a meaningful
  win for our scope.

### 4. **AVFoundation via objc2** (chosen)

- **Pros:**
  - Zero system install — every supported macOS already ships
    AVFoundation as a system framework.
  - Hardware H.264 decode via VideoToolbox (free perf win on M-
    series hardware).
  - `objc2-av-foundation` lives in the same crate family already
    pinned in `Cargo.toml`; the linkage pattern is established.
  - License: nothing to redistribute — system framework.
  - Bundle size: zero added dylibs; pure linkage.
  - Operator install story: "drop an mp4, it plays."
- **Cons:**
  - macOS-only — a future Linux / Windows port would need a
    second decoder. Acceptable per `CLAUDE.md`'s explicit
    macOS-only stance through v0.4 (and likely longer).
  - Apple's API is callback-driven; we'll need an
    `AVAssetReader` running on a worker thread, pulling
    `CMSampleBuffer` frames and converting to RGBA8 (or NV12
    that the renderer learns to sample directly).
  - `objc2-av-foundation` is less battle-tested than
    `ffmpeg-next` — expect API ergonomics gaps.

## Decision

**`objc2-av-foundation` is the chosen path.**

Justifications (decision-doc form, not bullet points):

The v0.4 target is macOS-only, so "cross-platform" decoder paths
add cost without delivering value. The macOS-native
AVFoundation/VideoToolbox stack delivers hardware-decoded H.264
without any operator-facing install step — meaningful for the
event-DJ "second laptop" failover scenario where the secondary
machine may not have Homebrew at all. The objc2 family is already
in the dependency graph and the wrapping pattern is established;
adding `objc2-av-foundation` is a one-line `Cargo.toml` change
plus the wrapping work.

The trade-off is portability: the day rmap goes Linux or Windows
the decoder needs a second backend. This is a future-cost we accept
because v0.4 is explicitly macOS-only and the AVFoundation backend
is independently a strong fit for the operator-side experience.

## Architecture (for the W4.2 / W4.3 follow-up commits)

The plan, sized for two follow-up PRs:

### W4.2 — `src/video_layer/` worker

- `VideoLayerWorker::start(path: &Path) -> (JoinHandle, Sender<VideoControl>)`.
- Worker thread:
  1. Build an `AVAsset` from the file path.
  2. Configure an `AVAssetReader` with an output track that
     decodes to BGRA8 (which we re-channel to RGBA8 for upload —
     swap is a single GPU step or a CPU memcpy reorder).
  3. On every loop iteration, `copyNextSampleBuffer` → extract
     `CVPixelBuffer` → lock base address → push the bytes onto
     the `TextureUploadQueue` (P0.3.1) tagged with the layer's
     `UploadTargetId`.
  4. Sleep `1.0 / fps * 1.0/speed` between frames; `EOF`
     re-creates the reader at offset 0 (seamless loop).
  5. `Sender<VideoControl>`: `Play | Pause | SetSpeed(f32) |
     SetLoop(bool) | Stop`. Pause blocks on `recv` (no
     `thread::park` per the P0 task notes — coalescing-wake bug).

### W4.3 — render integration

- `LayerKind::Video` no longer skipped in
  `app.rs:rebuild_layers`; instead it spawns a worker (W4.2)
  and binds the resulting upload-target texture into the
  existing layer pipeline.
- The texture-upload queue's drain (P0.3.1's
  `TextureUploadQueue::drain_into`) runs at the start of each
  frame, before layer rendering, inside the
  `panic_restore::run_frame_assert_unwind_safe` boundary.
- Once a frame uploads, the existing layer pipeline (effects
  chain, warp, mask, compositor) treats it identically to an
  Image layer's textures.

### W4.4 — speed + loop UI

- Selected-layer card detects `LayerKind::Video` and renders
  speed (0.25× to 4×) + loop checkbox.
- Mutations: `SetVideoSpeed`, `SetVideoLoopSeamless`, both
  symmetric `ReverseStorage` impls. Per-mutation push a
  `VideoControl` message to the worker's control channel.

## Acceptance gates (for the integration commits)

- [ ] `objc2-av-foundation` added behind a new `video` feature
      (default-on; opt out via `--no-default-features`).
- [ ] `cargo build --no-default-features` still succeeds.
- [ ] Per-layer worker thread decodes H.264 and pushes onto the
      texture-upload queue.
- [ ] Frames render through the existing layer pipeline (warp,
      mask, effects all work identically to Image).
- [ ] Seamless loop wraps without a perceptible pause at EOF.
- [ ] Pause / play resumes within one frame interval (control
      channel, no `thread::park`).
- [ ] Missing file → audit warning (consistent with image layers).

## Out of scope for v0.4

- Codecs other than H.264 / mp4 (Phase 1 follow-up).
- Reverse playback, in/out points, thumbnail scrubbing,
  BPM-locked playback (Phase 1).
- Audio track decode (audio handled by `cpal` / FFT path; videos
  with audio play silently in v0.4).
