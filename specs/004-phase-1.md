# 004 Phase 1 — Photo + video media pipeline

**Anchor:** A (Video as a first-class layer).
**Engine kickoff:** v0.4 (mp4 / H.264 + threading + texture-upload pipeline).
**Phase 1 deepens:** stills + video sharing the same treatment grammars.

See `specs/roadmap.md` for the strategic framing, scope discipline, and
phase index. See `specs/v3-capability-scope.md` (and roadmap §4.3) for the
v0.4 commitments that this phase builds on.

---

## Goal

Finish the v0.4 video work and extend the still-image pipeline into a true
media pipeline where photos, raster images, and video are first-class
inputs treated by the **same scene grammars**. The v3 engine handles
stills (PNG/JPG/WEBP, GIF first frame) and SVG only; this phase adds
video alongside, plus the shared treatment layer that both feed into.

## Capability set

**Photo / image treatments**
- Native photo/image layer treatments alongside SVG layers.
- Safe image preprocessing: crop modes, fit/fill, focal-point selection,
  tone mapping, cache-friendly texture upload.
- Treatment pipeline (applied to stills *and* video frames): blur masks,
  luminance-driven reveals, palette extraction, collage placement,
  texture overlays.

**Video layer**
- Engine prerequisites land in v0.4: mp4 / H.264 minimum viable path
  decoded on a background thread, uploaded to GPU each frame as a
  texture; seamless loop; configurable playback speed.
- Phase 1 adds the operator-facing surface: thumbnail scrubbing, in/out
  points, loop mode, rate (incl. reverse), sync-to-BPM playback.

## Engine implications

- Decoder library (`ffmpeg` bindings or `symphonia` + a video codec
  crate) plus a thread-safe texture-upload pipeline. v0.4 owns the
  initial bring-up; Phase 1 extends.
- The treatment pipeline is the first place where stills and video need
  a shared abstraction — design the per-frame texture handoff so that
  any stage downstream of "frame ready" treats stills (constant) and
  video (per-frame) identically.
- BPM-locked playback ties into the existing `Modulator::Bpm` plumbing.
  Re-uses tap-tempo (Space, MIDI Note 60, OSC `/rmap/tap`).
- The same threading + texture-upload work unblocks Phase 7 NDI /
  Syphon / Spout *output* — design the upload pipeline knowing it will
  be inverted later.

## UX items resolved

- **I9** — Left rail "+ Add image" grows row anatomy for video:
  thumbnail scrubbing, in/out points, loop mode appear inline on the
  same row that today shows a static thumbnail.
- **N5 capability follow-on** — diagnostics surface gains
  dropped-frame count alongside fps + panic-restored badge.

## Capability lens

- **VJ lens (primary).** Video as a first-class layer is the largest
  single perceived ceiling for any operator who isn't doing strictly
  photo work.
- **Projection-mapping lens (secondary).** NDI input as a layer source
  is also v0.4-scoped; treat it as a peer ingest path to the video
  pipeline.

## Out of scope for this phase

- Video output / streaming (Syphon / Spout / NDI out → Phase 7).
- Photo-treatment presets that consume zone semantics (→ Phase 3
  zones; Phase 4 scene grammars).
- Mask-shaped FX layers driven *by* media (→ Phase 2 — that's
  Anchor B).

## Usability rule

Ship a small number of tasteful image / video behaviours as **named
presets**. Do not expose dozens of controls at first. The treatment
pipeline gains depth in Phase 4 once scene grammars are the consumer.

## Acceptance criteria

- An operator can drop an mp4 into the left rail and see it play on the
  canvas with seamless loop within one click.
- Video and still layers expose the same set of treatment controls
  where applicable (crop, fit/fill, tone mapping, blend, opacity).
- BPM-locked playback follows tap-tempo without re-encoding.
- Dropped-frame count is visible in the diagnostics badge during a
  show.
