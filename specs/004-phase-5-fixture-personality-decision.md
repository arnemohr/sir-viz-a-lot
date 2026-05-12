# Phase 5 — fixture personality format decision (P5.0.2, W0)

**Status:** decision record. The struct definitions land in W3
follow-up tasks; this document fixes the minimal scope so Phase 7's
RGBW extension is purely additive.

## Context

Phase 5's plan notes that "moving-light personality editing is
deliberately deprioritised; full personality library is a Phase 7+
candidate, if ever." The roadmap §11 lists "moving-light personality
complexity" as a permanent postpone. What Phase 5 *does* need is a
minimal fixture personality model — just enough to map canvas colours
to DMX channel offsets for RGB fixtures, the cheapest credible output
strategy.

This document:
1. Defines the minimal `FixturePersonality` struct for Phase 5.
2. Explains why it is sufficient for RGB output.
3. States what is deferred to Phase 7 and what extension hook keeps
   Phase 7's RGBW work additive (no breaking schema migration).

## Constraints

- **Phase 5 output is RGB only.** The plan is explicit: "RGB, RGBW,
  HSV" are the options; "RGBW + colour-temperature-aware mixing → Phase
  7." Phase 5 ships RGB. The personality model must not require RGBW
  fields to be non-null.
- **No personality library.** An operator should not need to browse a
  300-fixture library to wire a par-can. Phase 5 asks the operator
  only: "how many channels, and which channel is Red / Green / Blue?"
- **Schema must survive Phase 7 addition of RGBW without a migration.**
  The personality schema version is separate from the project schema
  version; a `#[serde(default)]` on new channel-role variants handles
  the extension.
- **`FixturePersonality` is operator-authored in-project, not imported
  from a library.** Phase 5 stores personalities inline in the project
  JSON (as part of `FixtureGroup`). A future personality-library import
  feature (Phase 7+ if ever) reads the same struct.
- **`Mutation` Reverse-storage rules** (`src/project/CLAUDE.md`):
  every mutation that touches the fixture model must store its full
  reverse payload. The personality struct must be cheap to clone.

## Options evaluated

### Option A — Channel count + fixed RGB offsets (rejected)

```rust
pub struct FixturePersonality {
    pub channel_count: u16,
    pub red_offset: u8,
    pub green_offset: u8,
    pub blue_offset: u8,
}
```

- **Pros:** trivial serialisation, impossible to misconfigure.
- **Cons:** a Phase 7 `white_offset: Option<u8>` field is additive but
  ugly; adding `pan_offset` / `tilt_offset` (Phase 7 moving-light, if
  ever) makes this an ever-growing flat struct. The flat layout does
  not communicate "channels beyond these three are unconstrained."

### Option B — `Vec<ChannelRole>` channel map (chosen)

```rust
#[non_exhaustive]
pub enum ChannelRole {
    Red,
    Green,
    Blue,
    // Phase 7: White, ColorTemp, Intensity, Pan, Tilt, Generic
}

pub struct FixturePersonality {
    /// Channel map: index = DMX offset within the fixture's footprint.
    /// Length == the fixture's channel count.
    pub channels: Vec<ChannelRole>,
    /// Human-readable label shown in the fixture-group editor.
    pub label: String,
}
```

- **Pros:**
  - Naturally extensible: Phase 7 adds `White`, `ColorTemp`, etc. to
    `ChannelRole`. Existing personalities that predate Phase 7 simply
    don't have those variants; deserialization is additive with
    `#[serde(default)]` on the outer `Vec`.
  - `channel_count` is implicit (`channels.len()`), eliminating the
    mismatch footgun in Option A.
  - Iterating `channels` to build the DMX frame is a single `enumerate`
    loop — the writer patches `dmx[base_offset + i]` for each role it
    recognises, leaving unrecognised roles at zero. Phase 7 adds new
    match arms without touching Phase 5 code.
  - The per-channel model is the vocabulary most fixture operators
    already know (from console-side personality editors).
- **Cons:**
  - `Vec<ChannelRole>` has a small heap allocation per fixture
    personality. Acceptable: personalities are authored once and cached;
    they are not allocated in the hot path.
  - `#[non_exhaustive]` on `ChannelRole` means match arms in the DMX
    writer must include `_ => {}` — a small ergonomic cost bought for
    forward-compatibility.

### Option C — GDTF / Open Fixture Library import (rejected)

Import personalities from the GDTF (General Device Type Format) or the
Open Fixture Library JSON format.

- **Pros:** thousands of pre-built personalities.
- **Cons:** GDTF is XML-heavy and requires a parser crate. Open Fixture
  Library requires a network fetch or bundled database. Both are Phase 7+
  scope (the plan explicitly says "full personality library is a Phase 7+
  candidate"); adding either in Phase 5 would pull the system toward
  complexity before the core creative workflow is satisfying (roadmap §11
  framing).

## Decision

**Option B — `Vec<ChannelRole>` channel map — is the chosen design.**

A `FixturePersonality` stores a human-readable `label` and a
`Vec<ChannelRole>` whose length equals the fixture's DMX footprint. The
Phase 5 fixture-group editor asks the operator: fixture label, how many
channels, and the role of each (Red / Green / Blue — other roles are
greyed out with "Phase 7" hints using the `ModeHintBanner` pattern).

The DMX-frame writer iterates `channels.iter().enumerate()` and writes
scaled byte values into the correct DMX offset for recognised roles,
leaving all others at zero. Phase 7 adds `White`, `ColorTemp`,
`Intensity`, `Pan`, `Tilt` to `ChannelRole` as new variants; the writer
grows new match arms; existing Phase 5 project files deserialise cleanly
because `Vec<ChannelRole>` deserialization is append-only.

## Minimal Phase 5 structs

```rust
/// The role of a single DMX channel within a fixture's footprint.
/// Phase 5 ships Red / Green / Blue. Phase 7 adds the remaining variants.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ChannelRole {
    Red,
    Green,
    Blue,
}

/// Minimal fixture personality: channel map + label.
/// Stored inline in `FixtureGroup`; not a separate lookup table in Phase 5.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FixturePersonality {
    /// Channel map. Index = DMX offset within this fixture's footprint.
    /// `channels.len()` is the fixture's channel count.
    pub channels: Vec<ChannelRole>,
    /// Operator-supplied label ("RGB par", "LED strip segment", …).
    pub label: String,
}
```

`FixturePersonality` is owned by `FixtureGroup` (P5.3.1). When
serialised to project JSON it is an inline object, not a library
reference, so project portability is self-contained.

## What is deferred to Phase 7

- `ChannelRole::White`, `ColorTemp`, `Intensity`, `Pan`, `Tilt`,
  `Generic(String)` — defined in Phase 7's RGBW decision.
- Personality library / GDTF / OFL import — Phase 7+ (if ever).
- Moving-head parameters (pan, tilt, gobo, iris) — permanently low
  priority (roadmap §11).
- Per-personality colour-mixing curve / gamma — Phase 7 colour-
  calibration work.
- "Quick-build" shortcuts ("3-channel RGB", "4-channel RGBW") that
  pre-fill `channels` — Phase 5 UX polish or Phase 7.

## Extension contract for Phase 7

Phase 7's RGBW decision document must:
1. Add new `ChannelRole` variants without removing existing ones.
2. Keep `#[serde(rename_all = "snake_case")]` consistent so project
   files written in Phase 5 deserialise with `#[serde(default)]`
   for any newly added variants (which will not appear in Phase 5
   personality `Vec`s).
3. Extend the DMX-frame writer's `match role` arm — no other Phase 5
   code needs to change.
