# Decision: Phase 4 wizard state machine shape

**Status:** open — must be resolved before P4.3.1 can start.
**Depends on:** none.
**Unblocks:** P4.3.1 (wizard state + routing), P4.3.2 (cancel / back navigation),
P4.3.3 (commit to Editing), P4.4.1–P4.4.5 (step UIs).

---

## Context

Phase 4 introduces a wizard-style flow for creating a new scene from a template:
media → zones → palette → mood → tempo → commit. The plan says to "reuse the
existing v3 launcher / state-machine plumbing where possible (`AppState` in
`src/app.rs`)". Two plausible approaches exist.

---

## Option A — New top-level `AppState::SceneWizard` variant

Add a sixth branch to the `AppState` enum in `src/app.rs`:

```rust
SceneWizard(SceneWizardState),
```

where `SceneWizardState` holds the wizard's step cursor, collected choices, and
a stashed `EditingState` to restore on cancel. The transition is
`Editing → SceneWizard → Editing` (commit) or `Editing → SceneWizard → Editing`
(cancel, restoring the stash).

**Pros:**
- Consistent with how the Launcher is modelled: a full `AppState` variant with
  its own `ControlFlow` (`Poll`, because the canvas is still animating behind
  the wizard overlay).
- Per-state event routing in `App::window_event` is already structured for this
  pattern. Adding a `SceneWizard(s) => handle_wizard_window_event(s, ...)` arm
  mirrors the existing `Launcher(s) => handle_launcher_window_event(s, ...)`.
- State leaks between wizard and Editing are impossible by construction: the
  borrow is on `SceneWizardState`, not `EditingState`.
- Phase 6 cue-list wizard (a similar multi-step flow) can follow the same
  pattern without retrofit.

**Cons:**
- Requires stashing `EditingState` inside `SceneWizardState` during the wizard
  session; the stash is a full clone of GPU-connected state, which is expensive
  if `EditingState` is large. Mitigation: stash only the project `serde_json::Value`
  snapshot (as `ApplyProjectSnapshot` does) rather than the GPU state; rebuild on
  cancel from the snapshot. The render pipeline keeps running throughout; only the
  project JSON needs restoring.
- Adds a variant to `AppState::is_running` and `AppState::control_flow`; one-line
  changes in both impls.

---

## Option B — Sub-state inside `Editing`: `SceneEditorState::WizardActive`

Add a `wizard: Option<SceneWizardState>` field to `EditingState`. When the field
is `Some`, the control-panel draw function renders the wizard overlay instead of
the normal editor UI. Commit clears the field and applies the generated layers.
Cancel clears the field and rolls back via `ApplyProjectSnapshot`.

**Pros:**
- No `AppState` enum change; the GPU surface and input pipeline are undisturbed.
- The canvas preview (behind the wizard modal) stays live without extra plumbing.

**Cons:**
- `EditingState` already carries ~15 fields. Adding `wizard` makes every
  `editing_mut()` call implicitly permit wizard-mode mutations, which creates
  subtle bugs if a keyboard shortcut fires during wizard flow.
- The wizard draw is guarded by a runtime `if wizard.is_some()` check rather than
  a compile-time type transition. Regression risk: new callers forget the guard.
- The state machine's invariant (a given `AppState` variant implies a specific
  rendering behaviour + `ControlFlow`) is weakened: two very different UIs share
  `AppState::Editing`.
- Phase 6 cue-wizard would force the same pattern, making `EditingState` more
  complex still.

---

## Recommendation

**Option A — new `AppState::SceneWizard` variant.**

The stash cost is real but manageable: snapshot the project `serde_json::Value`
on wizard entry (the same mechanism `ApplyProjectSnapshot` uses). Wizard cancel
dispatches a non-undoable `ApplyProjectSnapshot` to restore the pre-wizard state.
This makes the cancel path a single, well-tested code path rather than custom
rollback logic inside Option B.

The compile-time correctness gain (a new `AppState` branch that cannot be
confused with `Editing`) outweighs the stash cost for the same reasons the
Launcher was given its own variant rather than a flag on `Booting`.

---

## Implementation sketch (for P4.3.1 to expand)

```
AppState::SceneWizard(SceneWizardState {
    /// Captured project snapshot for cancel rollback.
    pre_wizard_snapshot: serde_json::Value,
    /// Collected wizard choices so far.
    choices: WizardChoices,
    /// Which step is displayed (0 = template select, 1 = media, …).
    step: WizardStep,
    /// The EditingState is MOVED OUT of AppState::Editing on entry and
    /// stored here; it is MOVED BACK on commit or cancel.
    editing: EditingState,
})
```

The move-out / move-back pattern mirrors the `GoLive` transition
(`AppState::Editing → AppState::GoLive(editing)`) already used in the codebase.

`ControlFlow` for `SceneWizard`: `Poll` (canvas animates behind the modal).

---

## Action for P4.3.1

1. Accept or reject this recommendation (record decision inline here).
2. Implement the chosen variant; mark this doc "resolved".
3. Update task spec: remove BLOCKED annotations from P4.3.1–P4.4.5.
