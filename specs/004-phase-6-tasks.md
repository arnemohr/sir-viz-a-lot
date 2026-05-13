# 004 Phase 6 — task breakdown

Companion task spec for [`004-phase-6.md`](004-phase-6.md). Each task
below is sized for a single PR.

## Implementation status

- [x] P6.1.1 c6cf423 — Glossary entries for Phase 6 domain terms (16 terms)
- [x] P6.1.2 64add7d — CHANGELOG + README Phase 6 placeholder section
- [x] P6.1.3 bc0f429 — Perf-gate stub for 6-cue transport cycle fixture
- [x] P6.2.1 09a38e3 — Cue struct + rename Project.scenes to Project.cues
- [x] P6.2.2 46b2629 — Mutation variants for cue timing edits (SetCueName, SetCueTiming, SetProjectCues)
- [x] P6.2.3 bd476ca — Schema v8 → v9 migration: scenes renamed to cues
- [x] P6.5.1 6b94e2c — TransportState struct + tick integration
- [x] P6.5.2 e1dedcc — Follow chain execution (proptest invariants)
- [x] P6.5.3 3e668e0 — BPM quantize + timecode-trigger dispatch
- [x] P6.3.1-P6.3.4 646e3d3 — Cue detail panel (timing spinners, fire mode, BPM quantize, timecode trigger)
- [x] P6.4.1 c043721 — Three-state tile renderer (idle / armed-next / live)
- [x] P6.4.2 9c1eed6 — Keyboard + MIDI navigation (CueGo, CueArmNext, CueArmPrev, CueBackStep)
- [x] P6.6.1-P6.6.2 a26377f — Transport HUD (BPM display, tap source, current cue, global quantize)
- [x] P6.9.2 cd2e600 — OSC cue-fire addresses (/rmap/cue/go|prev|next|back|N)
- [x] P6.10.1-P6.10.2 3e2b3b2 — Audio bands strip (frequency labels + collapse/expand toggle)
- [x] P6.12.1 0a1c6d9 — MTC quarter-frame decoder in MIDI bus
- [x] P6.12.2 b0de634 — MIDI-clock BPM tracking + Clock::set_bpm()
- [BLOCKED] P6.11.1 — libltc not installed (brew install libltc + cmake required); skip for v0.7
- [x] P6.13.1 — SetCueTiming + SetProjectCues proptest (shipped as part of P6.2.2)
- [x] P6.13.2 — TransportState follow-chain proptest (shipped as part of P6.5.2)
- [skipped] P6.13.3 — Manual acceptance smoke: requires hardware run (see checklist P6.14.3)
- [skipped] P6.14.1 — Version bump: Cargo.toml is already at 0.9.0 (Phase 5 bump); downgrade to 0.7.0 would be incorrect
- [x] P6.14.2-P6.14.3 57c8c72 — CHANGELOG body + show-day checklist for Phase 6

---

## Operating model

- **Model:** Sonnet implements; Opus reviews. Same read-the-spec-first
  rule as earlier phases: read the originating spec section, read every
  CLAUDE.md the task touches, write the test alongside the
  implementation, run `make ci` before committing.
- **Pick one task at a time.** Read the source section it references in
  `004-phase-6.md` and the corresponding entry in `specs/roadmap.md`
  before starting.
- **Commit message format:** `004-P6.<workstream>.<task>: <title>` —
  e.g. `004-P6.2.1: Cue struct + schema migration`.
- **Branching:** one branch per task; merge straight to `main` once CI
  is green.
- **Pre-commit hook** (`.githooks/pre-commit`, wired by `make setup`)
  runs rustfmt on staged files + `cargo check`. Heavier checks live in
  `make ci`; run that before opening a PR.
- **Tests:** every task ships with new or updated tests. For schema /
  Mutation / snapshot work, follow the v3 proptest pattern in
  `src/project/command.rs`. For render-path work, add a golden under
  `tests/golden/` (covered by `--features gpu-tests`); use
  `UPDATE_GOLDEN=1` to (re-)record the baseline. Where automation isn't
  possible (manual cuelist UX, drag-bind gesture), ship a manual
  smoke-test checklist — never nothing.
- **Read the right CLAUDE.md.** Tasks touching `src/project/` must read
  `src/project/CLAUDE.md` first (Mutation Reverse-storage rules,
  snapshot invariants). Tasks touching `src/render/` must read
  `src/render/CLAUDE.md` first (GPU lifecycle, panic_restore, build-time
  WGSL validation).
- **Don't bundle.** If a task tempts you to also fix something nearby,
  resist — that "something nearby" probably already has its own task ID
  below.
- **Decision docs gate blocked tasks.** Tasks marked BLOCKED must not
  begin until the linked decision doc is resolved. Author the decision
  doc first; mark the task unblocked once the choice is recorded.
- **No tokio.** All async wgpu calls go through `pollster::block_on`.
  LTC / timecode decoding runs in its own background thread (same
  pattern as `OscSource`), never in an async runtime.

## Baseline orientation — what v0.4 already landed

Before reading task descriptions, note the infrastructure already in
place from v0.4 and Phase 2; tasks build on it, not around it:

- `Modulator::OscBound` and `Modulator::MidiBound` are real and wired —
  the `Param::Bound` stub mentioned in earlier plans is superseded.
- `BindingSource` picker in `src/windows/components/binding_picker.rs`
  covers Fixed / Sine / Tri / Noise / BPM / Audio / OSC / MIDI.
- `src/controls/midi_learn.rs` — `arm` / `cancel` / `take_target_if_armed`
  / `poll_timeout` fully implemented with 30 s timeout.
- MIDI CC registry (`CcRegistry` in `src/controls/midi.rs`) is live.
- `src/modulators/audio.rs` — 8-band FFT provider with `bands()` bulk
  read, `NUM_BANDS = 8`.
- `src/windows/cue_strip.rs` — horizontal tile strip with crossfade
  progress ring and `pending_cue` (BPM-quantize pending) visualisation.
  Tiles carry `Scene { name, snapshot }` — no per-cue timing yet.
- `src/controls/midi.rs` Note 60 → TapTempo, 61–69 → SceneRecall,
  70 → Blackout, 71 → Freeze.

Phase 6 extends rather than replaces this surface.

## Task ID conventions

IDs are `P6.<workstream>.<task>` within fourteen workstreams:

- W1 — Setup + housekeeping (glossary, CHANGELOG/README placeholders, perf-gate)
- W2 — `Cue` struct + schema migration
- W3 — Cue editing UI (timing pickers, follow mode, BPM quantize, timecode trigger)
- W4 — Cue tile state machine (idle / armed-next / live; 3-state crossfade ring)
- W5 — Transport state machine (current/armed/next, fade-progress, follow chain)
- W6 — Transport HUD (live BPM display, tap source indicator, quantize selector)
- W7 — `Param::Bound` activation *(see baseline note; scope is audio-band
  drag-source wiring, not re-implementing what v0.4 shipped)*
- W8 — MIDI learn workflow improvements (right-click extension to cue params;
  per-row visible binding tags)
- W9 — OSC binding parity (OSC address learn; cue-fire OSC addresses)
- W10 — Audio band binding UI surface (drag-source strip, drop-target on param rows)
- W11 — LTC decoder (cargo-feature gated; BLOCKED on decision doc)
- W12 — MTC + MIDI-clock decoder (extend existing MIDI bus)
- W13 — Snapshot / proptest / acceptance smoke
- W14 — Release housekeeping + 6-cue acceptance smoke

## Workstream summary

| WS | Theme | Tasks | Parallel-safe? | Touches |
|----|-------|-------|----------------|---------|
| 1 | Setup + housekeeping | 3 | All three parallel-safe | `src/windows/glossary.rs`, `CHANGELOG.md`, `README.md` |
| 2 | `Cue` struct + schema migration | 3 | P6.2.1 first; P6.2.2 + P6.2.3 serial after | `src/project/schema.rs`, `src/project/migrate.rs`, `src/project/command.rs` |
| 3 | Cue editing UI | 4 | Serial after W2 | `src/windows/cue_strip.rs`, `src/windows/control_panel.rs` |
| 4 | Cue tile state machine | 2 | Serial after W3 | `src/windows/cue_strip.rs` |
| 5 | Transport state machine | 3 | P6.5.1 first; P6.5.2 + P6.5.3 serial after; parallel with W3/W4 at W5.1 | `src/app.rs`, new `src/transport/` |
| 6 | Transport HUD | 2 | After W5 | `src/windows/toolbar.rs` or new `src/windows/transport_hud.rs` |
| 7 | Audio-band drag-source wiring | 2 | After W10 infra; independent of W8/W9 | `src/modulators/audio.rs`, `src/windows/` |
| 8 | MIDI learn improvements | 2 | After W8.1; independent of W9/W10 | `src/controls/midi_learn.rs`, `src/windows/components/parameter_row.rs` |
| 9 | OSC binding parity | 2 | Parallel with W8 | `src/controls/osc.rs`, `src/windows/components/parameter_row.rs` |
| 10 | Audio band binding UI | 2 | P6.10.1 first; P6.10.2 after | new `src/windows/audio_bands_strip.rs` |
| 11 | LTC decoder | 3 | BLOCKED — see decision doc | new `src/sync/ltc.rs`, `Cargo.toml` |
| 12 | MTC + MIDI-clock decoder | 2 | P6.12.1 first; P6.12.2 after | `src/controls/midi.rs`, new `src/sync/mtc.rs` |
| 13 | Snapshot / proptest / acceptance smoke | 3 | After W2; W13.3 last | `src/project/command.rs`, `tests/` |
| 14 | Release housekeeping + acceptance smoke | 3 | Last — depends on everything else | `Cargo.toml`, `CHANGELOG.md`, `README.md`, `docs/show-day-checklist.md` |

**Suggested PR sequencing:**

1. **P6.1.1 + P6.1.2 + P6.1.3** in parallel — quick independent wins.
2. **P6.2.1** (`Cue` struct) — unblocks W3, W4, W5, W13.
3. **P6.2.2 + P6.2.3** (mutations + migration) serial after P6.2.1.
4. **P6.5.1** (transport state machine skeleton) — can begin after P6.2.1;
   unblocks P6.5.2 + P6.5.3.
5. **P6.3.1 → P6.3.4** (cue editing UI) serial after P6.2.2 lands.
6. **P6.4.1 + P6.4.2** (tile state machine) after P6.3.x and P6.5.x land.
7. **P6.5.2 + P6.5.3** (follow chain, fade-progress) serial after P6.5.1.
8. **P6.6.1 + P6.6.2** (transport HUD) after P6.5.x land.
9. **P6.10.1** (audio bands strip widget) — independent; can start after P6.1.x.
10. **P6.10.2** (drag-source binding) after P6.10.1.
11. **P6.7.1 + P6.7.2** (audio-band wiring to parameter rows) after P6.10.2.
12. **P6.8.1 + P6.8.2** (MIDI learn improvements) parallel with W9/W10 — isolated.
13. **P6.9.1 + P6.9.2** (OSC binding parity) parallel with W8 — isolated.
14. **P6.12.1 + P6.12.2** (MTC + MIDI-clock) — independent of W2–W6; can start after P6.1.x.
15. **P6.11.x** (LTC) — BLOCKED until decision doc resolves.
16. **P6.13.1 + P6.13.2** (proptest extensions) after W2; **P6.13.3** last before W14.
17. **P6.14.1 → P6.14.3** last; P6.14.3 runs the 6-cue acceptance smoke.

## Anticipated risks

These design decisions are locked — they were approved in the planning
phase. Each is a potential scope-creep site; call it out at task time if
implementation pressure pushes toward a different choice.

1. **`Cue` struct replaces `Scene` in the strip, not alongside it.**
   The nine tiles carry `Cue` structs after migration; `Scene` is
   retired as the tile unit. `Project.scenes: Vec<Scene>` is renamed
   `cues: Vec<Cue>` with a schema migration step. The snapshot field
   (a `serde_json::Value`) stays; timing fields are added beside it.

2. **Transport is a separate state machine, not entangled in `EditingState`.**
   `TransportState` lives in a new `src/transport/` module and holds
   `current_cue`, `armed_cue`, `fade_progress`, `follow_chain`. The
   `EditingState` carries a `TransportState`; it does not grow transport
   fields inline.

3. **Follow chain fires automatically; go-on-trigger requires Space.**
   After a cue's hold time expires, a `follow` cue fires its in-time
   immediately (no operator action needed). A `go-on-trigger` cue waits
   for Space / MIDI 60 / OSC `/rmap/tap`. This is the full semantic;
   W5 must implement both without a "maybe later" stub.

4. **LTC and MTC are cargo-feature gated exactly like `audio` and `midi`.**
   `ltc` feature gate for the libltc/equivalent binding. `midi` already
   gates MTC + MIDI-clock (they decode inside the MIDI bus). Do not
   promote either to default features.

5. **Audio bands strip is a collapsible panel above the show-day strip.**
   Per Recommendation D in `roadmap.md` §7, all new live-input surfaces
   dock into the same right-side / bottom region. The audio bands strip
   collapses to a 36-px icon strip when no audio source is active;
   it expands when an audio source is active. It does not add a new
   column to the layout.

6. **MIDI-learn right-click extension scope for Phase 6.** v0.4
   already ships `midi_learn.rs` for modulator parameters. Phase 6
   extends the right-click context to cue timing parameters (in-time,
   hold, out-time mapped to MIDI CC for live trim) and adds visible
   binding-indicator tags on every already-bound row. It does not
   re-implement the learn mechanism.

7. **Binding storage decision.** Where per-cue CC trim bindings live in
   the project schema, and whether they survive preset switches, is an
   open design question. See `specs/004-phase-6-binding-storage-decision.md`.
   W8 and W9 are BLOCKED on this decision.

8. **LTC decoder library decision.** `libltc` (FFI to C lib) vs
   pure-Rust crate vs decode-in-house is unresolved. See
   `specs/004-phase-6-ltc-decoder-decision.md`. W11 is BLOCKED.

---

## Workstream 1 — Setup + housekeeping

Quick independent wins that ship before the heavier workstreams.

### P6.1.1 — Glossary entries for Phase 6 domain terms

**Source:** `004-phase-6.md` Capability set (cuelist, transport, LTC,
MTC, MIDI clock, follow chain, BPM quantize, armed-next, timecode
trigger); roadmap §I, §J, §N3.
**Type:** docs / UX
**Depends on:** none
**Files:** `src/windows/glossary.rs`.

**What:** Phase 6 introduces a cluster of show-control and timecode
terms that operators will see in cue tiles, the transport HUD, and
binding pickers. Adding glossary entries before those UI surfaces ship
means W3–W12 tasks can wire `glossary_label(ui, GlossaryTerm::X)` calls
without waiting on a separate docs task. Pattern is identical to P2.1.1
— extend the `GlossaryTerm` enum with new variants and add short
(~30 word) operator-facing definitions.

**Terms to add (~16):** *cue*, *cuelist*, *armed-next*, *live cue*,
*follow chain*, *go-on-trigger*, *follow (cue mode)*, *in-time*,
*hold time*, *out-time*, *BPM quantize*, *timecode trigger*, *transport
HUD*, *LTC (Linear Timecode)*, *MTC (MIDI Timecode)*, *MIDI clock*.

**Steps:**
1. Read `src/windows/glossary.rs` — locate the `GlossaryTerm` enum and
   `EXPECTED_VARIANT_COUNT`.
2. Add one enum variant per term listed above.
3. Write a short definition (~30 words) for each in the display match arm.
4. Bump `EXPECTED_VARIANT_COUNT`.

**Acceptance:**
- [ ] All 16 terms have `GlossaryTerm` variants and operator-facing
      definitions.
- [ ] `EXPECTED_VARIANT_COUNT` bumped to match.
- [ ] Existing exhaustiveness tests still pass.
- [ ] `make ci` clean.

**Out of scope:** Phase 7 terms (bezier, NDI out, luma key).

---

### P6.1.2 — CHANGELOG + README Phase 6 placeholder section

**Source:** `004-phase-6.md` Goal.
**Type:** docs
**Depends on:** none
**Files:** `CHANGELOG.md`, `README.md`.

**What:** Drop a shell section for the Phase 6 release in both files so
W14 tasks only need to fill body text. CHANGELOG gets an `[Unreleased]`
section header. README gets stub paragraphs for cuelist, transport HUD,
and live-input binding surface. Pattern mirrors P2.1.3.

**Acceptance:**
- [ ] `CHANGELOG.md` has an `[Unreleased] — v0.7` section with
      placeholder subsections: `### Cuelist + Transport`,
      `### Live Input Surface`, `### Timecode Sync`.
- [ ] `README.md` has a stub "Show Control (v0.7)" entry.
- [ ] No version strings changed (P6.14.1 owns the bump).
- [ ] `make ci` clean.

---

### P6.1.3 — Perf-gate refresh: 6-cue transport cycle fixture

**Source:** `004-phase-6.md` Acceptance criteria (6-cue show, mixed
timing modes); `tests/perf_frame_budget.rs` existing pattern.
**Type:** engine (defensive)
**Depends on:** none (stub fixture updated by W5 tasks)
**Files:** `tests/perf_frame_budget.rs`.

**What:** The existing perf gate validates representative scenes against
a p99 frame-time target. Phase 6 needs a fixture that represents the
worst-case transport tick: crossfade in progress, follow chain active,
BPM-quantize timer running. This task adds the test function with a
stub fixture (no real `Cue` struct yet — use the current `Scene`-based
path); P6.5.3 updates the fixture to use real transport state.

**Acceptance:**
- [ ] New `perf_transport_cycle_within_budget` test exists under
      `--features gpu-tests`.
- [ ] Fixture comment notes it will be updated in P6.5.3.
- [ ] Test skips cleanly when no GPU adapter is available.
- [ ] `make ci` clean.

---

## Workstream 2 — `Cue` struct + schema migration

The architectural workstream. Every W3–W6 task depends on it.

### P6.2.1 — `Cue` struct + rename `Project.scenes` → `Project.cues`

**Source:** `004-phase-6.md` Engine implications ("Cue struct extends
current SceneIndex storage"); `src/project/CLAUDE.md` schema additions.
**Type:** engine / schema
**Depends on:** none
**Files:** `src/project/schema.rs`, `src/project/mod.rs`,
`src/windows/cue_strip.rs`, `src/app.rs` (call sites).

**What:** Define `Cue` as the tile unit, extending `Scene` with per-cue
timing fields. The current `Scene { name: String, snapshot:
serde_json::Value, thumbnail: ThumbnailRgba }` becomes:

```rust
pub struct Cue {
    pub name: String,
    pub snapshot: serde_json::Value,
    pub thumbnail: ThumbnailRgba,
    // --- Phase 6 additions ---
    pub in_time_s: f32,    // default 0.0 (snap)
    pub hold_time_s: Option<f32>, // None = hold until triggered
    pub out_time_s: f32,   // default 0.0 (snap)
    pub fire_mode: CueFireMode, // Follow | GoOnTrigger
    pub bpm_quantize: BpmQuantize, // Off | Bars(1|2|4|8)
    pub timecode_trigger: Option<TimecodePosition>, // HH:MM:SS:FF
}

pub enum CueFireMode { Follow, GoOnTrigger }

pub enum BpmQuantize { Off, Bars(u8) } // u8 in {1, 2, 4, 8}

pub struct TimecodePosition { pub hh: u8, pub mm: u8, pub ss: u8, pub ff: u8 }
```

Rename `Project.scenes: Vec<Scene>` → `Project.cues: Vec<Cue>`.
Update all call sites in `src/app.rs`, `src/windows/cue_strip.rs`,
`src/project/mod.rs` (particularly `restore_scene` which references
`project.scenes` directly). The schema migration step lands in P6.2.3.

Read `src/project/CLAUDE.md` before editing — `restore_scene` vs
`restore` distinction, serde default rules (non-zero identity values
need explicit `#[serde(default = "...")]`), `CURRENT_SCHEMA_VERSION`
bump lives in P6.2.3.

**Acceptance:**
- [ ] `Cue` struct compiles with all fields and serde defaults.
- [ ] All new timing fields default to identity values that round-trip
      to the same behaviour as a v7 `Scene` with no timing.
- [ ] `CueFireMode::GoOnTrigger` is the default (preserves existing
      trigger semantics).
- [ ] `restore_scene` tests (`recall_preserves_other_slots`,
      `restore_scene_preserves_crossfade_duration`) still pass.
- [ ] `make ci` clean (all existing scene-related tests pass with
      renamed field).

---

### P6.2.2 — `Mutation` variants for cue timing edits

**Source:** `004-phase-6.md` Engine implications; `src/project/CLAUDE.md`
Mutation Reverse-storage rules.
**Type:** engine / mutations
**Depends on:** P6.2.1
**Files:** `src/project/command.rs`.

**What:** Add typed `Mutation` variants for every per-cue field edit so
the undo stack records reversible entries. Pattern mirrors
`SetLayerTreatmentParams` (whole-struct snapshot Reverse).

New variants:
- `SetCueName { cue_idx: usize, new: String, old: String }`
- `SetCueTiming { cue_idx: usize, new: CueTimingSnapshot, old: CueTimingSnapshot }`
  where `CueTimingSnapshot` captures all timing fields in one struct.
- `SetProjectCues { new: Vec<Cue>, old: Vec<Cue> }` — replaces
  `SetProjectScenes`; reorder / save / delete all go through this.

Apply the three Reverse-storage rules from `src/project/CLAUDE.md`:
1. `SetCueName` stores the full old `String`.
2. `SetCueTiming` stores the whole `CueTimingSnapshot` (not just the
   changed field) — future fields added to `Cue` won't silently corrupt
   on undo.
3. `SetProjectCues` mirrors `SetProjectScenes`'s whole-vec snapshot.

Write `debug_assert!` guards in each `apply` arm. Add proptest
round-trip cases in the existing `proptest_round_trip` harness.

**Acceptance:**
- [ ] All three variants compile with `ReverseStorage` impls.
- [ ] `debug_assert!` guards verify old == current before apply.
- [ ] Proptest round-trip exercises all three variants.
- [ ] `make ci` clean.

---

### P6.2.3 — Schema v7 → v8 migration: `scenes` → `cues`

**Source:** `src/project/CLAUDE.md` schema additions ("Bump
`CURRENT_SCHEMA_VERSION` and add a step to `migrate.rs`").
**Type:** engine / migration
**Depends on:** P6.2.1
**Files:** `src/project/migrate.rs`, `src/project/schema.rs`.

**What:** Bump `CURRENT_SCHEMA_VERSION` from 7 to 8. Add a migration
step that renames the `scenes` key to `cues` in the serialised JSON
and injects identity defaults for all new timing fields. Old projects
(schema v7 and below) must load without error and display existing cue
names / snapshots unchanged.

Write a migration unit test that round-trips a v7 fixture JSON through
`migrate` and asserts `cues` is present and `scenes` is absent; timing
fields default correctly.

**Acceptance:**
- [ ] `CURRENT_SCHEMA_VERSION` is 8.
- [ ] Migration step is present in `migrate.rs` at the v7→v8 position.
- [ ] v7 fixture round-trips: cue name preserved, timing fields default.
- [ ] Save → reload round-trip test passes (schema v8 fixture).
- [ ] `make ci` clean.

---

## Workstream 3 — Cue editing UI

### P6.3.1 — In-time / hold / out-time spinners in cue detail panel

**Source:** `004-phase-6.md` Capability set (per-cue timing fields);
UX item I6.
**Type:** UI
**Depends on:** P6.2.2
**Files:** `src/windows/cue_strip.rs` or new
`src/windows/cue_detail_panel.rs`, `src/app.rs`.

**What:** Clicking a cue tile expands a detail area (or opens a side
panel) showing the three timing spinners: in-time (seconds, 0.0..=60.0),
hold (seconds or "∞" for indefinite), out-time (seconds, 0.0..=60.0).
Each spinner dispatches `Mutation::SetCueTiming` through the undo stack
on change. Use the `ParameterRow` component pattern from
`src/windows/components/parameter_row.rs` so the rows inherit binding
picker and glossary popover wiring.

**Acceptance:**
- [ ] Three spinner rows visible when a cue tile is selected.
- [ ] Changing any spinner dispatches `SetCueTiming`; undo reverses it.
- [ ] Spinners clamp to valid ranges; out-of-range input is silently
      clamped, not rejected.
- [ ] Glossary tooltips wire to `GlossaryTerm::InTime`,
      `GlossaryTerm::HoldTime`, `GlossaryTerm::OutTime`.
- [ ] Manual smoke: change in-time, undo, verify revert. `make ci` clean.

---

### P6.3.2 — Fire mode picker (Follow / GoOnTrigger)

**Source:** `004-phase-6.md` Capability set (follow vs go-on-trigger).
**Type:** UI
**Depends on:** P6.3.1
**Files:** same as P6.3.1.

**What:** Add a two-state toggle (radio buttons or segmented control) for
`CueFireMode::Follow` | `CueFireMode::GoOnTrigger` to the cue detail
panel. Dispatches `Mutation::SetCueTiming` on change. Wire glossary
popover for `GlossaryTerm::FollowChain` and `GlossaryTerm::GoOnTrigger`.

**Acceptance:**
- [ ] Toggle present and dispatches `SetCueTiming`; undo reverses it.
- [ ] Glossary popovers show correct definitions.
- [ ] `make ci` clean.

---

### P6.3.3 — BPM quantize selector

**Source:** `004-phase-6.md` Capability set (1/2/4/8-bar quantize);
roadmap §J.
**Type:** UI
**Depends on:** P6.3.1
**Files:** same as P6.3.1.

**What:** Add a drop-down or segmented control for `BpmQuantize`: Off /
1 / 2 / 4 / 8 bars. `BpmQuantize::Off` means cue fires immediately on
Space; `Bars(n)` means the transport snaps the fire to the next n-bar
boundary at the current BPM. Dispatches `SetCueTiming`. Wire glossary
popover for `GlossaryTerm::BpmQuantize`.

**Acceptance:**
- [ ] Picker present and dispatches `SetCueTiming`; undo reverses it.
- [ ] `BpmQuantize::Off` selection fires immediately (verified via W5
      integration after P6.5.2 lands).
- [ ] `make ci` clean.

---

### P6.3.4 — Timecode trigger field

**Source:** `004-phase-6.md` Capability set (optional timecode trigger);
Engine implications.
**Type:** UI
**Depends on:** P6.3.1, P6.12.1 (MTC decoder must exist for the field
to be meaningful, but the UI field can ship before MTC is wired)
**Files:** same as P6.3.1.

**What:** Add an optional timecode trigger field (HH:MM:SS:FF) to the
cue detail panel. A checkbox enables the field; when disabled the field
greys out. When enabled, the transport fires the cue automatically when
incoming timecode reaches the specified position (wired in P6.5.3).
Dispatches `SetCueTiming`. Wire glossary popover for
`GlossaryTerm::TimecodePosition`.

**Acceptance:**
- [ ] Field present; checkbox enables/disables it.
- [ ] Four sub-spinners (HH, MM, SS, FF) with clamped ranges.
- [ ] `SetCueTiming` dispatched on any sub-field change; undo reverses.
- [ ] `make ci` clean.

---

## Workstream 4 — Cue tile state machine

### P6.4.1 — Three-state tile renderer (idle / armed-next / live)

**Source:** `004-phase-6.md` UX item I6 ("cue tiles gain idle /
armed-next / live states with a 3-state crossfade ring").
**Type:** UI
**Depends on:** P6.2.1, P6.5.1 (transport state exposes
`current_cue_idx` and `armed_cue_idx`)
**Files:** `src/windows/cue_strip.rs`.

**What:** Extend `scene_tile` to accept a `TileState` discriminant
(`Idle | ArmedNext | Live`) and render accordingly:

- **Idle:** existing muted appearance.
- **ArmedNext:** warm accent ring on the tile border (amber pulse per
  Appendix B design system — distinct from the crossfade-target ring).
- **Live:** solid accent fill on a slim bottom bar + "LIVE" badge
  superimposed on the thumbnail.

Crossfade-in-progress adds a progress ring to the `Live` tile (carried
over from the existing `crossfade_progress` field; ensure the two visual
layers compose correctly).

The `TileState` discriminants are computed from `TransportState` fields
(current / armed) and passed into `show` from the call site in
`src/app.rs` or `src/windows/control_panel.rs`.

**Acceptance:**
- [ ] All three states render without egui warnings.
- [ ] Amber armed ring is visually distinct from the crossfade accent ring.
- [ ] "LIVE" badge is legible against the thumbnail gradient.
- [ ] Manual smoke: arm two cues in sequence; verify visual transitions.
- [ ] `make ci` clean.

---

### P6.4.2 — Keyboard + MIDI navigation (←/→ arm, Space go, Backspace back-cue)

**Source:** `004-phase-6.md` Capability set (Space = go, ←/→ = move
arm, Backspace = back-cue); roadmap Appendix A.
**Type:** engine / UX
**Depends on:** P6.4.1, P6.5.1
**Files:** `src/controls/keyboard.rs`, `src/controls/midi.rs`,
`src/app.rs` (`apply_command`).

**What:** Wire cue navigation into the `Command` enum and `apply_command`
dispatch:

- `Command::CueGo` — fires the armed cue (Space key, MIDI Note 60
  already wired to `TapTempo`; reuse or add a secondary dispatch
  for `CueGo` on Note 60 when transport is armed).
- `Command::CueArmNext` / `Command::CueArmPrev` — move the armed pointer
  without firing (←/→ keys).
- `Command::CueBackStep` — step back one cue and re-arm (Backspace).

Do not conflict with existing Note 60 → `TapTempo` mapping; `CueGo`
fires on Note 60 only when a cue is armed. Document the dual-role in
a code comment.

**Acceptance:**
- [ ] All four commands dispatch from keyboard (verify with manual smoke).
- [ ] MIDI Note 60 dual-role documented; TapTempo still fires when no
      cue is armed.
- [ ] `make ci` clean.

---

## Workstream 5 — Transport state machine

### P6.5.1 — `TransportState` struct + tick integration

**Source:** `004-phase-6.md` Engine implications ("Transport state
machine: holds current cue, armed-next cue, fade-progress, follow
chain").
**Type:** engine
**Depends on:** P6.2.1
**Files:** new `src/transport/mod.rs`, `src/app.rs`
(`EditingState`), `src/project/schema.rs`.

**What:** Create `src/transport/` module with:

```rust
pub struct TransportState {
    pub current_cue: Option<usize>,
    pub armed_cue: Option<usize>,
    pub fade_progress: f32,   // 0.0..=1.0
    pub follow_chain: Vec<usize>, // indices of pending follow-mode cues
}
```

Wire `TransportState` into `EditingState` as `pub transport:
TransportState`. Per-frame tick method `TransportState::tick(delta_s,
bpm, cues)` advances `fade_progress` based on the current cue's
`in_time_s`, checks hold expiry, and triggers follow-mode chaining.
The tick is called once per frame from the `EditingState` render path
*outside* the mutable borrow of UI state (mirrors `SideEffect` pattern
in `apply_command`).

`TransportState` is not serialised to the project file — it is
session-only state, like `crossfade_progress` today.

**Acceptance:**
- [ ] `TransportState` compiles and is wired into `EditingState`.
- [ ] Per-frame tick advances `fade_progress` for a synthetic cue
      (unit test with a stub `Vec<Cue>`).
- [ ] `make ci` clean.

---

### P6.5.2 — Follow chain execution

**Source:** `004-phase-6.md` Capability set (follow vs go-on-trigger).
**Type:** engine
**Depends on:** P6.5.1, P6.2.1
**Files:** `src/transport/mod.rs`.

**What:** Implement the follow chain: after `current_cue`'s hold expires,
if `fire_mode == Follow`, the transport immediately loads the next cue
into `current_cue` and begins its `in_time_s` fade. No operator action.
If `fire_mode == GoOnTrigger`, `fade_progress` freezes at 1.0 until
`Command::CueGo` arrives. Chain ends when the last cue in the list is
`GoOnTrigger` or when `cues` is exhausted.

Unit test: build a 3-cue vec with `Follow` / `Follow` / `GoOnTrigger`;
verify the chain auto-advances through the first two then halts at the
third.

**Acceptance:**
- [ ] Follow chain auto-advances without operator input (unit test).
- [ ] GoOnTrigger halts the chain (unit test).
- [ ] Chain exhaustion (last cue fired) does not panic.
- [ ] `make ci` clean.

---

### P6.5.3 — BPM quantize + timecode-trigger dispatch

**Source:** `004-phase-6.md` Capability set (BPM-bar quantize, timecode
trigger).
**Type:** engine
**Depends on:** P6.5.2, P6.3.3, P6.3.4
**Files:** `src/transport/mod.rs`, `src/app.rs`.

**What:** Extend `TransportState::tick` to:

1. **BPM quantize:** when `Command::CueGo` arrives and the armed cue has
   `BpmQuantize::Bars(n)`, defer the fire until the next n-bar boundary
   at the current BPM. The armed cue index is stored as `armed_cue`
   during the wait; the tile renders with the `ArmedNext` state. At the
   boundary, fire normally. `BpmQuantize::Off` fires immediately.

2. **Timecode trigger:** each tick checks whether any armed cue has a
   `timecode_trigger` position that matches the incoming timecode
   (sourced from `TransportState::last_timecode_position` set by
   W12/W11 decoders). On match, dispatch `CueGo` internally.

Update the perf-gate fixture stub added in P6.1.3 to use real
`Cue` + `TransportState`.

**Acceptance:**
- [ ] BPM quantize defers fire to bar boundary (unit test with synthetic
      BPM and tick loop).
- [ ] Timecode trigger fires on matching position (unit test with
      injected timecode sequence).
- [ ] Perf-gate fixture updated to real transport state.
- [ ] `make ci` clean.

---

## Workstream 6 — Transport HUD

### P6.6.1 — Live BPM display + tap source indicator

**Source:** `004-phase-6.md` Capability set (Transport HUD: live BPM,
tap source); roadmap §I, N3.
**Type:** UI
**Depends on:** P6.5.1
**Files:** `src/windows/toolbar.rs` or new
`src/windows/transport_hud.rs`.

**What:** Add a transport HUD widget to the top chrome (or a collapsible
band above the show-day strip — per panel-docking rule in roadmap §D).
The HUD shows:
- Live BPM value (f32, one decimal place) sourced from `EditingState`'s
  existing BPM clock.
- Tap source indicator: "Space", "MIDI 60", or "OSC /rmap/tap" — the
  source of the most recent `TapTempo` event.
- Current cue name + index (e.g. "3 / 6 — Arch Wash").

The HUD is always visible when `EditingState` or `GoLive` is active; it
is hidden in `Launcher` / `Failed` states.

**Acceptance:**
- [ ] BPM value updates each frame.
- [ ] Tap source updates on each `TapTempo` command (from any source).
- [ ] Current cue name shows "—" when no cue is active.
- [ ] `make ci` clean.

---

### P6.6.2 — Quantize selector in HUD

**Source:** `004-phase-6.md` Capability set (1/2/4/8-bar quantize
selector in transport HUD).
**Type:** UI
**Depends on:** P6.6.1, P6.5.3
**Files:** same as P6.6.1.

**What:** Add a 1 / 2 / 4 / 8 / Off segmented selector to the transport
HUD that overrides the per-cue `BpmQuantize` setting for the current
session (a "global quantize override" for the operator). When set, all
cues fire on the global quantize boundary regardless of their individual
setting. Setting it to "Off" falls back to per-cue settings. The value
is session-only (not saved to the project file).

Wire `GlossaryTerm::BpmQuantize` popover on the selector label.

**Acceptance:**
- [ ] Segmented selector renders and responds to click.
- [ ] Global quantize overrides per-cue setting when set.
- [ ] Falls back to per-cue setting when Off.
- [ ] Value not serialised to project file (unit test: save/reload,
      selector resets to Off).
- [ ] `make ci` clean.

---

## Workstream 7 — Audio-band drag-source wiring

*Note: W7 depends on W10 (the audio bands strip widget) providing
drag-source handles. W7 wires those handles to the parameter-row
drop-targets.*

### P6.7.1 — Drag-source IDs on FFT band tiles

**Source:** `004-phase-6.md` Capability set ("Audio FFT modulator UI
surface: drag-source binding from each band"); `src/modulators/audio.rs`.
**Type:** UI / engine
**Depends on:** P6.10.1 (audio bands strip widget)
**Files:** `src/windows/audio_bands_strip.rs`,
`src/modulators/audio.rs`.

**What:** Each of the 8 FFT band tiles in the audio bands strip becomes
an egui drag-source. The drag payload is a `BindingSource::Audio`
with band index. This task adds the drag-source registration; the
drop-target wiring lives in P6.7.2.

Use egui's `DragAndDrop` API introduced in recent egui versions.
The drag visual is a copy of the band tile at reduced opacity. Do not
invent a custom drag system — use the egui-native API.

**Acceptance:**
- [ ] Each of 8 band tiles can be dragged (egui `DragAndDrop` detected).
- [ ] Drag payload correctly encodes band index.
- [ ] No visual regression on the band strip when not dragging.
- [ ] `make ci` clean.

---

### P6.7.2 — Drop-target on parameter rows + binding commit

**Source:** `004-phase-6.md` Capability set (drag-source binding from
each band to any parameter row).
**Type:** UI / engine
**Depends on:** P6.7.1
**Files:** `src/windows/components/parameter_row.rs`,
`src/windows/control_panel.rs`.

**What:** Each `ParameterRow` becomes a drop-target. When a band drag is
released over a row, the row dispatches `Mutation::SetModulator` with
`Modulator::Audio { band: <idx>, scale, offset }` — reusing the same
constructor that the `BindingSource::Audio` picker already uses
(`modulator_for_source` in `control_panel.rs`). A brief visual flash
confirms the binding.

**Acceptance:**
- [ ] Dropping a band tile on a parameter row binds the band.
- [ ] Binding persists across undo (the mutation is on the undo stack).
- [ ] Binding survives save/reload (already covered by the
      `Modulator::Audio` serde path — verify with a round-trip test).
- [ ] Visual flash appears on successful drop.
- [ ] `make ci` clean.

---

## Workstream 8 — MIDI learn improvements

### P6.8.1 — Visible binding-indicator tags on bound parameter rows

**Source:** `004-phase-6.md` Capability set ("right-click → unbind /
relearn"); roadmap Recommendation I.
**Type:** UI
**Depends on:** none (the binding infrastructure is already in place)
**Files:** `src/windows/components/parameter_row.rs`.

**What:** When a parameter row's `Modulator` is `MidiBound` or
`OscBound`, render a small tag beside the spinner showing the binding
address (e.g. "MIDI CC 21 ch 1" or "OSC /rmap/blur/radius"). The tag
is clickable: left-click opens a context menu with "Unbind" and
"Re-learn" options. "Unbind" dispatches `Mutation::SetModulator` with
`Modulator::Static`. "Re-learn" re-arms the existing learn mechanism
(`midi_learn::arm` or the OSC equivalent from P6.9.1).

**Acceptance:**
- [ ] Tag visible on every bound row (MIDI and OSC).
- [ ] "Unbind" dispatches correct mutation; undo reverses it.
- [ ] "Re-learn" re-arms the learn state; pulsing ring appears.
- [ ] `make ci` clean.

---

### P6.8.2 — Right-click "Learn next MIDI CC" on all parameter rows

**Source:** `004-phase-6.md` Capability set ("Right-click on parameter
row → 'Learn next MIDI CC'").
**Type:** UI
**Depends on:** P6.8.1
**Files:** `src/windows/components/parameter_row.rs`,
`src/controls/midi_learn.rs`.

**What:** Every `ParameterRow` should expose the right-click "Learn next
MIDI CC" context menu item, not just the modulator rows that had it in
v0.4. The v0.4 path in `src/windows/control_panel.rs`'s `modulator_slider`
already calls `midi_learn::arm`; this task generalises the right-click
menu to all `ParameterRow` instances (including effect parameters, cue
timing fields added in P6.3.1, and any new parameters added in W3).

`midi_learn.rs` is not modified — the existing `arm` / `cancel` /
`take_target_if_armed` surface is sufficient.

**Acceptance:**
- [ ] Right-click context menu appears on all `ParameterRow` instances.
- [ ] "Learn next MIDI CC" arms `midi_learn::arm` for the row's
      `LearnTarget`.
- [ ] Pulsing accent ring appears on the armed row's binding picker.
- [ ] Twisting a CC captures the binding; the ring disappears.
- [ ] Binding survives save/reload (undo-stack mutation path).
- [ ] `make ci` clean.

---

## Workstream 9 — OSC binding parity

*W9 is BLOCKED until `specs/004-phase-6-binding-storage-decision.md`
is resolved.*

### P6.9.1 — OSC address learn (process-wide learn state)

**Source:** `004-phase-6.md` Capability set (learn flow extends to
OSC address); roadmap Recommendation I.
**Type:** engine / UI
**Depends on:** P6.8.1; decision doc `004-phase-6-binding-storage-decision.md`
**Files:** new `src/controls/osc_learn.rs`, `src/controls/osc.rs`,
`src/windows/components/parameter_row.rs`.

**What:** Implement an OSC address learn module mirroring
`midi_learn.rs`. When armed, the `OscSource` receive loop captures
the *next* OSC message with any unknown address and emits
`Command::OscLearnCapture { addr, target }`. The `apply_command` arm
dispatches `SetModulator(OscBound { addr, scale, offset })`.

The 30 s timeout pattern from `midi_learn.rs` carries over.

**Acceptance:**
- [ ] `osc_learn::arm` / `cancel` / `take_target_if_armed` /
      `poll_timeout` implemented with unit tests matching the
      `midi_learn.rs` test structure.
- [ ] Right-click "Learn next OSC address" appears on `ParameterRow`.
- [ ] Incoming OSC message captures the address; binding committed.
- [ ] `make ci` clean.

---

### P6.9.2 — OSC cue-fire addresses

**Source:** `004-phase-6.md` Capability set (OSC `/rmap/tap` is an
existing tap source; extend to cue control).
**Type:** engine
**Depends on:** P6.5.1; decision doc `004-phase-6-binding-storage-decision.md`
**Files:** `src/controls/osc.rs`.

**What:** Extend the OSC address decode table in `osc.rs` to cover the
new transport commands:

- `/rmap/cue/go` → `Command::CueGo`
- `/rmap/cue/prev` → `Command::CueArmPrev`
- `/rmap/cue/next` → `Command::CueArmNext`
- `/rmap/cue/back` → `Command::CueBackStep`
- `/rmap/cue/N` (N = 1..=9) → `Command::SceneRecall(N-1)` (fire cue N)

Document the new addresses in a code comment block at the top of
`osc.rs` alongside the existing address table.

**Acceptance:**
- [ ] All five address families decode to correct commands.
- [ ] Unit tests cover each address (mirrors existing decode tests).
- [ ] Existing address tests unchanged.
- [ ] `make ci` clean.

---

## Workstream 10 — Audio band binding UI

### P6.10.1 — Audio bands strip widget

**Source:** `004-phase-6.md` Capability set ("Audio bands strip is
visible whenever an audio source is active; each band is a drag-source
for parameter binding"); roadmap §D, Recommendation I.
**Type:** UI
**Depends on:** none (the audio provider is already in place)
**Files:** new `src/windows/audio_bands_strip.rs`,
`src/windows/control_panel.rs` (wiring).

**What:** Create an `audio_bands_strip::show(ui, bands: &[f32; 8])`
widget that renders 8 vertical bar meters (logarithmic scale, 0..=1)
in a horizontal row, each labelled with an approximate frequency range
(e.g. "Sub", "Bass", "Low mid", "Mid", "High mid", "Pres", "Brill",
"Air"). The strip is a fixed height (36 px when collapsed, 80 px
expanded).

Visibility rule: the strip renders when the audio feature is active
and an audio provider is installed (`audio::PROVIDER` is Some). It is
hidden (or collapsed to an 8-px hint bar) when no audio source is
active.

Wire `current_bands_snapshot` from `audio.rs` to update the meter
values each frame. This is a read-only display in P6.10.1; drag-source
behaviour lands in P6.7.1.

**Acceptance:**
- [ ] Widget renders 8 labelled bars.
- [ ] Bars update each frame from the live audio provider.
- [ ] Strip hidden when no audio provider is active.
- [ ] `make ci` clean.

---

### P6.10.2 — Collapsed / expanded state + docking

**Source:** Roadmap §D ("all new live-input surfaces dock into the same
right-side / bottom region").
**Type:** UI
**Depends on:** P6.10.1
**Files:** `src/windows/audio_bands_strip.rs`,
`src/windows/control_panel.rs`.

**What:** Add a toggle button (chevron icon) that collapses the strip to
a 36-px icon bar and expands it to the full 80-px meter view.
Collapsed/expanded state is stored in `ControlPanelState` (session-only,
not saved to project). The strip slots into the panel-docking region
established by the existing show-day strip at the bottom of the
control panel — no new layout column.

**Acceptance:**
- [ ] Toggle button collapses / expands the strip.
- [ ] Collapsed height is 36 px; expanded height is 80 px.
- [ ] Strip sits in the same bottom docking region as the show-day strip.
- [ ] Expanded/collapsed state survives window resize.
- [ ] `make ci` clean.

---

## Workstream 11 — LTC decoder

*W11 is BLOCKED until `specs/004-phase-6-ltc-decoder-decision.md` is
resolved. Do not begin any P6.11.x task before the decision doc is
approved.*

### P6.11.1 — LTC cargo feature + audio input thread

**Source:** `004-phase-6.md` Engine implications (LTC via `libltc` or
equivalent; cargo-feature gated).
**Type:** engine
**Depends on:** BLOCKED — decision doc `004-phase-6-ltc-decoder-decision.md`
**Files:** `Cargo.toml`, new `src/sync/ltc.rs`.

**What:** Add a `ltc` cargo feature. Add the chosen LTC dependency
(per the decision doc). Create `src/sync/ltc.rs` with an `LtcDecoder`
struct that subscribes to the audio input stream (via `cpal`, already
gated on `feature = "audio"`) and decodes LTC frames from the selected
channel. The decoded timecode position is stored in an
`Arc<Mutex<Option<TimecodePosition>>>` readable by the transport tick.

The `ltc` feature implies `audio`; `Cargo.toml` must enforce this.

**Acceptance:**
- [ ] `ltc` feature gates the dependency; `--features ltc` builds clean.
- [ ] `LtcDecoder::start()` returns `anyhow::Result<Self>`.
- [ ] Decoded timecode is readable from the shared position slot.
- [ ] Unit test with a canned LTC sample buffer verifies decode
      (fixture-driven, no hardware required).
- [ ] `make ci` clean (both with and without `--features ltc`).

---

### P6.11.2 — LTC position wired to `TransportState`

**Source:** `004-phase-6.md` Acceptance criteria (LTC sync drives cue
firing within ±1 frame).
**Type:** engine
**Depends on:** P6.11.1, P6.5.3
**Files:** `src/transport/mod.rs`, `src/app.rs`.

**What:** On each transport tick, read the LTC decoder's shared position
slot (if `feature = "ltc"` and decoder is running) and update
`TransportState::last_timecode_position`. The timecode-trigger dispatch
in P6.5.3 then fires the matching cue.

**Acceptance:**
- [ ] LTC position flows to `TransportState` each tick when enabled.
- [ ] Unit test: inject a synthetic timecode sequence that matches a
      cue trigger; verify `CueGo` is dispatched within ±1 frame
      (fixture-driven).
- [ ] Transport is unaffected when `ltc` feature is absent.
- [ ] `make ci` clean.

---

### P6.11.3 — LTC source indicator in transport HUD

**Source:** `004-phase-6.md` Transport HUD.
**Type:** UI
**Depends on:** P6.11.2, P6.6.1
**Files:** `src/windows/transport_hud.rs` (or toolbar).

**What:** When `feature = "ltc"` is active and an LTC signal is
detected, the transport HUD shows "LTC" as the timecode source alongside
the current timecode position (HH:MM:SS:FF). When no signal is detected
(decoder returns `None`), show "LTC (no signal)" in a muted colour.
When the feature is absent, the timecode source row is hidden.

**Acceptance:**
- [ ] "LTC HH:MM:SS:FF" appears in HUD when signal present.
- [ ] "LTC (no signal)" in muted colour when signal absent.
- [ ] HUD row hidden when `ltc` feature absent.
- [ ] `make ci` clean.

---

## Workstream 12 — MTC + MIDI-clock decoder

### P6.12.1 — MTC quarter-frame decoder in the MIDI bus

**Source:** `004-phase-6.md` Engine implications ("MIDI clock decoded
inside existing MIDI bus"); `src/controls/midi.rs`.
**Type:** engine
**Depends on:** none (extends existing MIDI bus)
**Files:** `src/controls/midi.rs`, new `src/sync/mtc.rs`.

**What:** MTC (MIDI Timecode) is sent as quarter-frame messages (status
byte 0xF1). Add a `MtcDecoder` in `src/sync/mtc.rs` that assembles
8 quarter-frame messages into a full `TimecodePosition`. The MIDI
callback in `midi.rs` recognises 0xF1 messages and passes them to the
decoder; on full-frame assembly, the position is stored in
`Arc<Mutex<Option<TimecodePosition>>>` (same interface as the LTC
decoder in W11, so `TransportState::last_timecode_position` can read
from either).

Feature gate: MTC decoding is part of `--features midi`; no new feature
gate needed.

**Acceptance:**
- [ ] `MtcDecoder` assembles 8 quarter-frame messages correctly.
- [ ] Unit test: feed 8 synthetic quarter-frame bytes; verify decoded
      `TimecodePosition`.
- [ ] MIDI callback passes 0xF1 messages to decoder without affecting
      the existing Note On / CC decode path.
- [ ] `make ci` clean.

---

### P6.12.2 — MIDI-clock BPM tracking + transport HUD wiring

**Source:** `004-phase-6.md` Capability set (MIDI-clock sync).
**Type:** engine / UI
**Depends on:** P6.12.1, P6.6.1
**Files:** `src/controls/midi.rs`, `src/transport/mod.rs`,
`src/windows/transport_hud.rs`.

**What:** MIDI clock sends 24 pulses per quarter note (status byte 0xF8).
Add a `MidiClockTracker` that timestamps incoming 0xF8 messages and
computes a rolling BPM average (last 24 pulses = one quarter note).
The derived BPM is stored in `Arc<RwLock<Option<f32>>>` and exposed
as the tap-source "MIDI Clock" in the BPM clock. The transport HUD
shows "MIDI Clock" as the tap source when this path is active.

The BPM clock existing in `EditingState` already accepts `TapTempo` from
keyboard / MIDI note / OSC; this task adds MIDI Clock as a fourth source
that updates the BPM directly rather than through individual taps.

**Acceptance:**
- [ ] MIDI Clock BPM derived from 0xF8 pulse timing (unit test with
      synthetic pulse timestamps).
- [ ] Transport HUD shows "MIDI Clock" when this source is active.
- [ ] BPM quantize (W5/W6) uses MIDI Clock BPM when it is active.
- [ ] `make ci` clean.

---

## Workstream 13 — Snapshot / proptest / acceptance smoke

### P6.13.1 — Proptest extension: `Cue` timing round-trip

**Source:** `src/project/CLAUDE.md` (proptest pattern); `004-phase-6.md`
acceptance criteria (binding survives save/reload/undo).
**Type:** testing
**Depends on:** P6.2.2
**Files:** `src/project/command.rs` proptest module.

**What:** Extend the existing `proptest_round_trip` harness to cover:
- `Mutation::SetCueTiming` — arbitrary timing values, round-trip through
  apply → reverse → state matches original.
- `Mutation::SetProjectCues` — arbitrary cue vec, round-trip.
- Schema v8 fixture: save a project with two cues carrying non-default
  timing fields; reload from JSON; verify timing fields preserved.

Pattern mirrors the `SetFxLayerParams` proptest in Phase 2 (P2.9.1).

**Acceptance:**
- [ ] Both mutation variants have proptest round-trip tests.
- [ ] Schema v8 fixture save/reload test passes.
- [ ] `make ci` clean.

---

### P6.13.2 — Proptest extension: `TransportState` follow-chain invariants

**Source:** `004-phase-6.md` Engine implications (follow chain).
**Type:** testing
**Depends on:** P6.5.2
**Files:** `src/transport/mod.rs` test module.

**What:** Property-based test that generates an arbitrary sequence of
`CueFireMode` values and verifies:
- Follow chains always eventually terminate (no infinite loop).
- GoOnTrigger always halts the chain.
- `fade_progress` is always in `[0.0, 1.0]` after any number of ticks.

**Acceptance:**
- [ ] Proptest exercises at least 1000 random fire-mode sequences.
- [ ] All three invariants hold.
- [ ] `make ci` clean.

---

### P6.13.3 — Manual acceptance smoke: 6-cue mixed-timing show

**Source:** `004-phase-6.md` Acceptance criteria (6-cue show, mixed
timing modes).
**Type:** testing / acceptance
**Depends on:** all W2–W12 tasks complete
**Files:** `docs/show-day-checklist.md`.

**What:** Manually run a 6-cue show:

1. Build a project with 6 cues covering: snap in/out (cue 1), 2 s
   fade (cue 2), BPM-quantize 4-bar (cue 3), follow-on (cue 4),
   timecode-triggered (cue 5, fixture-injected via synthetic MTC),
   Go-on-trigger (cue 6).
2. Verify all cues fire and transition correctly.
3. Verify a MIDI CC binding on one effect parameter survives save /
   reload / undo.
4. Verify the audio bands strip is visible and band-to-parameter
   drag binding works.
5. Add findings as a checklist section to `docs/show-day-checklist.md`.

**Acceptance:**
- [ ] All 6 cues run through correctly in one session.
- [ ] MIDI CC binding survives save/reload/undo.
- [ ] Audio bands drag binding works.
- [ ] Checklist entry added to `docs/show-day-checklist.md`.
- [ ] `make ci` clean.

---

## Workstream 14 — Release housekeeping

### P6.14.1 — Version bump to v0.7.0

**Source:** Roadmap phase plan.
**Type:** release
**Depends on:** all W1–W13 tasks complete
**Files:** `Cargo.toml`.

**What:** Bump the package version from the current value to `0.7.0`.
Follow the same single-file pattern as P2.10.1.

**Acceptance:**
- [ ] `Cargo.toml` version is `0.7.0`.
- [ ] `cargo metadata` shows consistent version.
- [ ] `make ci` clean.

---

### P6.14.2 — CHANGELOG body + README for v0.7

**Source:** `004-phase-6.md` Goal.
**Type:** docs / release
**Depends on:** P6.14.1
**Files:** `CHANGELOG.md`, `README.md`.

**What:** Fill the `[Unreleased] — v0.7` placeholder from P6.1.2.
CHANGELOG body covers: cuelist + per-cue timing, transport state
machine, transport HUD, audio band binding, MIDI/OSC learn improvements,
MTC + MIDI-clock sync, LTC (if shipped). README adds a "Show Control"
feature entry under the Features list.

**Acceptance:**
- [ ] CHANGELOG body is operator-facing copy, not implementation notes.
- [ ] README "Show Control (v0.7)" entry is present.
- [ ] `make ci` clean.

---

### P6.14.3 — Show-day checklist update for Phase 6

**Source:** `004-phase-6.md` Usability rule.
**Type:** docs / release
**Depends on:** P6.13.3
**Files:** `docs/show-day-checklist.md`.

**What:** Add Phase 6 show-day checklist items: verify MIDI controller
binding before going live; confirm audio input active if using audio
binding; verify timecode source (LTC / MTC / MIDI Clock) locked if
using timecode cues; run a cue-strip dry-run (arm + fire each cue once)
before audience enters. Also add recovery steps: what to do if MIDI
learn times out, if LTC signal is lost mid-show.

**Acceptance:**
- [ ] Phase 6 section present in `docs/show-day-checklist.md`.
- [ ] At least 6 checklist items covering binding verification,
      timecode lock, dry-run, and recovery.
- [ ] `make ci` clean.
