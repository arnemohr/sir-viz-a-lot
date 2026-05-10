# 004 Phase 6 — Show control, cuelist, and live input

**Matures:** OSC parameter binding (kicked off v0.4) and the cue-tile
snapshot model (v3) into a full show-control system.
**Builds on:** Phase 5 (light cues authored in parallel to video cues).

See `specs/roadmap.md` §10.2 for why MIDI parameter binding belongs in
v0.4 alongside OSC; this phase consumes that infrastructure and matures
it into a full live-input surface.

---

## Goal

Make rmap practical for events and repeatable operation. A live operator
should trigger scenes, fade between them, and recover from mistakes
quickly without navigating deep control panes — and external transports
(MIDI, OSC, audio, timecode) should be first-class drivers, not bolted-on
extensions.

## Capability set

**Cuelist**
- Per-cue fields: in-time, hold, out-time, follow vs go-on-trigger,
  BPM-bar quantize, optional timecode trigger.
- The 9-tile snapshot strip from v3 stays; each tile carries a `Cue`
  struct, not just a `SceneIndex` (resolves UX item I6).
- Light cues authored in parallel to video cues (binds to Phase 5
  fixture model). Same scene snapshot carries both.

**Transport**
- Space = go, ←/→ = move arm without firing, Backspace = back-cue.
- Full transport HUD: live BPM, tap source (Space / MIDI 60 / OSC
  `/rmap/tap`), 1/2/4/8-bar quantize selector for cue firing.
- LTC / MTC / MIDI-clock sync.

**Live input**
- External control via OSC / MIDI parameter binding (the picker, learn
  workflow, and registry plumbing kicked off in v0.4 mature here into
  full coverage of every parameter row).
- Audio-reactive modulation as scene design — bands map to specific
  parameters via the binding picker, not free-running chaos.

## Engine implications

- `Cue` struct extends current `SceneIndex` storage. Snapshot path in
  `src/project/` gains per-cue timing fields.
- Transport state machine: holds current cue, armed-next cue,
  fade-progress, follow chain.
- LTC / MTC decoder: `libltc` (or equivalent Rust crate) for LTC,
  MIDI clock decoded inside the existing MIDI bus
  (`src/controls/midi.rs`).
- Audio FFT modulator UI surface builds on existing
  `src/modulators/audio.rs` — drag-source binding from each band to
  any parameter row.
- Parameter binding path: by the start of this phase, `Param::Bound`
  (today `#[allow(dead_code)]`) is real. Effect parameters route
  through it; learn flow extends to MIDI CC, MIDI Note, OSC address.

## UX items resolved

- **I6** — cue tiles gain idle / armed-next / live states with a
  3-state crossfade ring during transitions.
- **Recommendation J** — cuelist as the eventual home for the cue
  strip; `Cue` struct replaces `SceneIndex` storage.
- **Recommendation I follow-on** — binding picker matures from "OSC
  on a few parameter rows" (v0.4) to "every parameter row, for OSC
  + MIDI + audio bands + BPM + every other source the registry
  exposes".
- **N3 capability follow-on** — full transport HUD with timecode
  sync, MIDI clock, BPM tap surface.

## Capability lens

- **VJ lens (primary).** Music-locked, audio-reactive,
  externally-driven performance is the natural endpoint of this
  phase.
- **Light-scene-design lens (secondary).** Light cues fire on the
  same transport as video cues; light-scene blackout (Phase 5) is a
  cue type.

## Out of scope for this phase

- A/B deck pattern — two scenes loaded simultaneously with a manual
  fader. Standard VJ tool. Recommend as a v0.5+ candidate; can land
  here if it falls out of the cuelist work cleanly.
- Console interop (timecode export *to* lighting consoles is fine;
  full bi-directional MA / EOS / Hog integration is out of scope
  permanently).

## Usability rule

A live operator should be able to **trigger scenes, fade between
them, and recover from mistakes quickly** without navigating deep
control panes. Anything that requires opening the Advanced rail
during a live run-up is a design failure for this phase.

## Acceptance criteria

- An operator can build a 6-cue show with mixed timing modes (fixed
  fade, BPM-quantize, follow-on, timecode) in one editing session
  and play it back without further configuration.
- A MIDI controller knob can be bound to any effect-chain parameter
  via right-click → "Learn next MIDI CC" → twist; the binding
  survives save / reload / undo.
- Audio bands strip is visible whenever an audio source is active;
  each band is a drag-source for parameter binding.
- LTC / MTC sync drives cue firing within ±1 frame of the incoming
  timecode.
- Cue strip shows current/next/armed at all times; the operator
  never has to ask "what's about to fire?".
