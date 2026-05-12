# Decision: LTC decoder library for Phase 6

**Status:** OPEN — must be resolved before P6.11.1 begins.
**Owner:** Arne Mohr
**Depends on:** Phase 6 plan `004-phase-6.md` §Engine implications.
**Unblocks:** P6.11.1, P6.11.2, P6.11.3.

---

## Context

Phase 6 requires LTC (Linear Timecode, SMPTE 12M) decoding so a live
timecode signal from a DAW or tape machine can drive cue firing within
±1 frame accuracy. LTC is an audio-rate bitstream carried on an analog
signal; it must be decoded from an audio input channel (via `cpal`,
already gated on `feature = "audio"`).

No LTC code exists in the codebase. Three options are available.

---

## Options

### Option A — FFI to `libltc` (C library)

`libltc` (https://github.com/x42/libltc) is the canonical LTC
implementation: LGPL-2.1, actively maintained, used by Ardour and
dozens of other professional tools. A Rust FFI binding exists as the
`ltc` crate on crates.io (thin wrapper around the `sys` crate that
vendors the C source via `cmake`).

**Pros:**
- Battle-tested across frame rates (23.976, 24, 25, 29.97DF, 30).
- ±1 frame accuracy well-documented and tested in production.
- LGPL-2.1 is compatible with rmap's macOS-only, event-day target
  (dynamic linking satisfies LGPL; static linking requires disclosure
  of the LGPL object — acceptable for an open-source project).
- Very little code to write: `LtcDecoder::new()` wraps the C struct;
  `push_samples()` feeds audio chunks; `read_timecode()` polls decoded
  frames.

**Cons:**
- FFI build dependency: requires `cmake` + a C compiler at build time.
  The existing toolchain (`mise.toml`) does not pin these; add to
  `make setup` documentation.
- C code: `cargo audit` will not see CVEs in the vendored C source.
- Linkage: the `ltc` crate vendors the C source and builds it
  statically; no dynamic-linking concern in practice, but the LGPL
  disclosure obligation must be noted in `CHANGELOG.md`.

**Build risk:** Low. The `ltc` crate builds on macOS Apple Silicon
in known projects. `cmake` is available via Homebrew / `mise`.

---

### Option B — Pure-Rust crate (`ltc-decode` or equivalent)

As of 2026, no pure-Rust LTC crate has reached production-quality
status on crates.io. The closest candidates (`ltc-decode`, `biphase`)
are experimental or unmaintained.

**Pros:**
- No C toolchain dependency.
- `cargo audit` covers the full dependency tree.

**Cons:**
- No production-validated pure-Rust crate exists as of this writing.
- Frame accuracy and edge-case handling (free-running vs jammed,
  drop-frame, speed variations) are not validated.
- Writing a conforming LTC decoder in-house (Option C) is
  non-trivially complex.

**Verdict:** Not recommended in current state. Revisit if a
well-maintained pure-Rust crate emerges.

---

### Option C — Decode in-house

Implement a biphase-mark code (BMC) decoder and LTC frame assembler
in `src/sync/ltc.rs` without external dependencies.

**Pros:**
- Zero external dependencies.
- Full control over frame accuracy.

**Cons:**
- LTC decoding requires biphase-mark code detection (edge timing),
  frame boundary detection, parity checking, and drop-frame arithmetic.
  This is 300–500 lines of careful bit-manipulation with non-obvious
  edge cases.
- Testing requires known-good audio fixtures; the `libltc` test suite
  would need to be partially replicated.
- Ongoing maintenance burden if SMPTE edge cases surface at events.
- Estimated effort: 3–5 days of focused work before the ±1 frame
  requirement is reliably met.

**Verdict:** Not recommended for Phase 6 scope. The effort exceeds the
value given that a tested C library is available under a compatible
license.

---

## Recommendation

**Option A — FFI to `libltc` via the `ltc` crate.**

Rationale:
1. The ±1 frame accuracy requirement is non-negotiable for show-day
   reliability; `libltc` is the only option with a production track
   record across all frame rates.
2. The LGPL-2.1 disclosure obligation is trivially met in an
   open-source project.
3. Build-time `cmake` dependency is acceptable: rmap already requires
   Xcode Command Line Tools on macOS for the Rust toolchain; `cmake`
   is a superset that CI and developer environments can install once.
4. The FFI surface is tiny (3–4 functions); the Rust wrapper can be
   isolated in `src/sync/ltc.rs` with a `#[cfg(feature = "ltc")]`
   gate so non-LTC builds are unaffected.

**Required before P6.11.1 begins:**

1. Verify `ltc` crate version on crates.io and audit the vendored C
   source version for known CVEs.
2. Confirm `cmake` availability in the development environment and add
   to `make setup` and `docs/setup.md` (or equivalent).
3. Add a note in `CHANGELOG.md` (Phase 6 section) that `libltc` is
   linked under LGPL-2.1.
4. Record the chosen `ltc` crate version in `Cargo.toml` with a pinned
   minimum version.

**Mark P6.11.1 as unblocked once steps 1–4 are completed.**

---

## Open questions

- Does the `ltc` crate's CMake build work correctly with the `mise`-
  managed Rust toolchain (1.92)? Verify on a fresh checkout before
  committing to the approach.
- What frame rate should rmap default to when no LTC signal is
  detected? 25 fps (PAL) and 30 fps (NTSC) are the most common for
  European and North American events. Recommend making this a project-
  level setting rather than a compile-time default.
- Should rmap write a fixture-based test using a pre-recorded LTC
  audio sample, or is a synthetic byte-sequence test sufficient for
  CI? The `libltc` source ships test vectors; use one as the fixture.
