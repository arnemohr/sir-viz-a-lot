# Decision: binding storage for Phase 6 (where bindings live in the project schema)

**Status:** OPEN — must be resolved before P6.9.1 and P6.8.2 (the W8/W9
tasks that *write* new binding types) begin.
**Owner:** Arne Mohr
**Depends on:** existing `Modulator` enum in `src/modulators/mod.rs`;
v0.4 MIDI + OSC binding infrastructure.
**Unblocks:** P6.9.1, P6.9.2.

---

## Context

v0.4 landed `Modulator::MidiBound { channel, cc, scale, offset }` and
`Modulator::OscBound { addr, scale, offset }` as the binding storage
for *effect parameters*. These live on `LayerConfig.effects[i].params`
inside the project's `Vec<Layer>` and are serialised with the project.
The undo stack records `Mutation::SetModulator` changes; they survive
save/reload and undo.

Phase 6 adds new categories of bindable surface:

1. **Per-cue timing fields** (in-time, hold, out-time) — operators may
   want to trim these live from a MIDI controller during rehearsal.
2. **Cue-fire commands** — a MIDI note or OSC address firing a specific
   cue (already covered partially by the Note 61–69 → SceneRecall
   table in `midi.rs`; Phase 6 may extend or formalise this).
3. **Transport-level controls** — BPM tap, quantize level — already
   routed through `TapTempo` / `Command::*`; these do not need a new
   binding storage layer.

The core question is: **should per-cue timing bindings live in the
`Cue` struct (alongside the timing fields they bind to), or in a
separate process-level binding registry that is not serialised?**

---

## Options

### Option A — Bindings live in `Cue` struct (schema-serialised)

Extend the `Cue` struct (P6.2.1) with optional binding fields:

```rust
pub struct Cue {
    // ... timing fields ...
    pub in_time_binding: Option<CcBinding>,
    pub hold_binding: Option<CcBinding>,
    pub out_time_binding: Option<CcBinding>,
}

pub struct CcBinding { pub channel: u8, pub cc: u8, pub scale: f32, pub offset: f32 }
```

Bindings are serialised with the project (schema v8), survive
save/reload, and are covered by the existing `SetCueTiming` mutation
(or a new `SetCueBinding` variant).

**Pros:**
- Bindings travel with the project file — opening a show on a
  different machine retains the binding *address* (the operator
  must verify the controller is connected, but the mapping is
  preserved).
- Consistent with how `Modulator::MidiBound` works on effect params.
- The proptest round-trip harness (P6.13.1) covers binding fields
  without special-casing.
- Undo works naturally: `SetCueTiming` (or `SetCueBinding`) is on the
  undo stack.

**Cons:**
- Schema churn: adds fields to `Cue` (which is already being
  added in P6.2.1 — this is an additive change, not a new migration).
- The binding is an address that may not match the operator's
  controller on a different machine. This is the same situation as
  `Modulator::MidiBound`; the established answer is "the operator
  re-learns on the new rig."

**Verdict:** Recommended. Consistent with existing binding model;
no new storage concept required.

---

### Option B — Bindings in a process-level registry (not serialised)

A `BindingRegistry` struct holds all active bindings at runtime. It is
not serialised to the project file; the operator re-learns at each
session start.

**Pros:**
- No schema change for bindings specifically.
- "Clean slate" at session start — stale bindings never confuse a
  new rig setup.

**Cons:**
- The Phase 6 acceptance criterion explicitly states: "A MIDI
  controller knob can be bound to any effect-chain parameter via
  right-click → 'Learn next MIDI CC' → twist; the binding survives
  save / reload / undo." A process-level registry cannot satisfy
  "survives save / reload."
- Diverges from the v0.4 `Modulator::MidiBound` precedent; two
  separate binding storage models in the same codebase.
- Undo is non-trivial: the registry is mutable session state, not
  on the project mutation stack.

**Verdict:** Does not satisfy the acceptance criterion. Rejected.

---

### Option C — Bindings in a sidecar file (separate from project JSON)

Store bindings in `~/Library/Application Support/rmap/<project-name>.bindings.json`
alongside the `.rmap.json` file.

**Pros:**
- Project JSON stays clean.
- The sidecar can be updated without incrementing `CURRENT_SCHEMA_VERSION`.

**Cons:**
- Two-file save/load atomicity is harder than a single atomic rename.
  The existing save path (`save_portable` + atomic rename) does not
  generalise to two files without more complexity.
- "Survives reload" requires the sidecar to be found alongside the
  project; renaming or moving the project file orphans the bindings.
- Adds a new file format to document and maintain.

**Verdict:** Adds complexity without meaningful benefit over Option A.
Rejected.

---

## Recommendation

**Option A — Bindings live in `Cue` struct, serialised in the project
schema.**

**Schema design:**

Add an optional `CcBinding` (MIDI) or `OscBinding` (OSC) to each
timing field that supports live trim. Keep the binding fields optional
(`Option<CcBinding>`) so existing projects that don't use this feature
are unaffected. The serde default is `None` (no binding), which
round-trips to the same behaviour as a pre-Phase-6 `Cue`.

The `SetCueTiming` mutation (P6.2.2) includes the binding fields in its
`CueTimingSnapshot` so bindings are captured atomically with timing
values in the undo stack. No separate `SetCueBinding` mutation is
needed.

**Scope for Phase 6:**

Only expose bindings for the three per-cue timing fields that an
operator might trim live (in-time, hold, out-time). Cue-fire-command
bindings (Note 61–69 → SceneRecall) are already handled by the
hard-wired MIDI decoder table in `midi.rs`; do not extend the `Cue`
schema for those in Phase 6. If dynamic cue-fire mappings are needed,
that is a Phase 7 feature.

**Preset switches:**

`Cue` has no "preset" concept (unlike `FxLayer`). When the operator
saves a cue with a binding and later recalls the same cue slot, the
binding is part of the saved snapshot and is restored. No special
handling is needed.

**Required before P6.9.1 and P6.8.2 begin:**

1. Add `CcBinding` and `OscBinding` structs to `src/project/schema.rs`
   with serde defaults.
2. Extend `CueTimingSnapshot` in P6.2.2 to include binding fields.
3. Verify the migration step in P6.2.3 injects `None` for all new
   binding fields in v7→v8 migration.
4. Mark P6.9.1 and P6.8.2 as unblocked once steps 1–3 are
   completed in P6.2.1/P6.2.2/P6.2.3.

---

## Open questions

- Should `OscBinding` store just an address string (`addr: String`) or
  a full `{ addr, scale, offset }` triple? The existing
  `Modulator::OscBound` uses the triple; recommend consistency.
- Should binding fields on cue timing rows be exposed in the per-cue
  detail panel (W3) from day one, or shipped as a follow-up task?
  Recommendation: ship in W8/W9 tasks (after the per-cue panel
  exists from P6.3.1) rather than adding it to P6.3.1's scope.
- Should the MIDI learn right-click menu on timing spinners respect
  the same 30 s timeout as parameter-row learn? Yes — use the
  existing `midi_learn` module unchanged.
