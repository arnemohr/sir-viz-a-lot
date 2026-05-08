# 003 — Phase 1 Tasks: Architecture Foundations

> Index: `003-tasks.md`. Plan: `003-ui-ux-overhaul-plan.md`.
> **47 tasks. ~3 engineering weeks. Critical path runs through this file.**

## Purpose

Lay the architectural rails — `AppState`, `Command`, `UndoStack`,
`ProjectAudit`, `Toast`, telemetry hooks — that every later phase
assumes. Nothing visible to a non-technical user changes. The v2
UI continues running on `main`; this work ships behind `--features
v3`.

## Scope covered

- WP-1 (AppState enum)
- WP-1.1 (decompose `init_running_app`)
- WP-2 (Command + Undo/Redo, all ~56 mutation sites)
- WP-15 (ProjectAudit + Toast)
- WP-17 (telemetry hooks)

## Relationship to overall rollout

Phase 1 unblocks every later phase. M1 is the gate.

## Entry criteria

- M0 reached: 10 plan decisions resolved (done); demo asset
  license-cleared; wireframes approved.
- T0.7 (`--features v3` Cargo feature) merged.
- Glossary v0 ≥ 8 entries (T0.1) — used in Phase 3 but copy-review
  process must be live now.

## Exit criteria

- T1.1–T1.47 acceptance criteria green.
- `cargo test --features v3` green.
- `cargo build --features v3` produces a binary that:
  - launches the v2 UI unchanged when run without `--features v3`,
  - launches with new architecture (state machine + commands +
    audit + telemetry) when run with `--features v3`,
  - allows Cmd-Z undo of every covered mutation,
  - surfaces a clear toast for the zero-scale `~/p1.rmap.json`
    fixture.
- M1 declared.

---

## Task index

| ID | Title | Owner | Scope | Depends |
|----|-------|-------|-------|---------|
| ✅ T1.1 | Define `AppState` enum | RUST | M | T0.7 |
| ✅ T1.2 | Migrate `App::resumed` to AppState | RUST | M | T1.1 |
| ✅ T1.3 | Migrate `App::window_event` to AppState | RUST | M | T1.1 |
| T1.4 | Per-state `ControlFlow` derivation | RUST | S | T1.2 |
| T1.5 | macOS resume guard for new states | RUST | S | T1.2 |
| T1.6 | Verify `--list-monitors` + `--autostart` regression-free | RUST + QA | S | T1.2, T1.3 |
| T1.7 | Extract `init_gpu()` | RUST | S | T1.1 |
| T1.8 | Extract `init_output_window()` | RUST | M | T1.7 |
| T1.9 | Extract `init_control_window()` | RUST | S | T1.7 |
| T1.10 | Extract `init_inputs()` | RUST | M | T1.7 |
| T1.11 | Extract `init_render_graph()` | RUST | M | T1.8, T1.9 |
| T1.12 | Reduce `init_running_app` to orchestrator | RUST | S | T1.7–T1.11 |
| T1.13 | Rename `ControlEvent` → `Command` | RUST | S | T1.1 |
| T1.14 | Reverse-storage type machinery | RUST | L (justified) | T1.13 |
| T1.15 | `UndoStack<C>` with `non_undoable` | RUST | M | T1.14 |
| T1.16 | Central `apply_command` function | RUST | S | T1.14 |
| T1.17 | Proptest harness on `Command::Noop` | RUST | M | T1.15, T1.16 |
| T1.18 | Migrate always-visible bindings batch | RUST | M | T1.17 |
| T1.19 | Migrate per-layer Layers tab bindings | RUST | M | T1.17 |
| T1.20 | Migrate Layers tab buttons | RUST | M | T1.17 |
| T1.21 | Migrate `show_effect` per-effect sliders | RUST | M | T1.17 |
| T1.22 | Migrate Modulator picker (whole-enum Reverse) | RUST | M | T1.17 |
| T1.23 | Migrate `modulator_slider` parameter widgets | RUST | M | T1.22 |
| T1.24 | Migrate scene_editor drag-translate | RUST | M | T1.17 |
| T1.25 | Migrate scene_editor drag-scale | RUST | S | T1.24 |
| T1.26 | Migrate scene_editor drag-rotate (effects-Vec Reverse) | RUST | M | T1.24 |
| T1.27 | Migrate mask vertex drag/insert/delete | RUST | M | T1.17 |
| T1.28 | Migrate Mapping tab buttons | RUST | M | T1.17 |
| T1.29 | Migrate Effects-tab Apply preset button | RUST | S | T1.21 |
| T1.30 | Migrate scene save/recall as `ApplyProjectSnapshot` | RUST | M | T1.17 |
| T1.31 | Migrate `DroppedFile` layer-add | RUST | S | T1.17 |
| T1.32 | Output-state toggles via `Command` for telemetry | RUST | S | T1.16 |
| T1.33 | Test: file-watcher hot-reloads excluded from undo | RUST + QA | S | T1.15, T1.31 |
| T1.34 | Define `ProjectAudit` + `AuditFinding` types | RUST | M | T1.1 |
| T1.35 | Audit: zero-scale layer | RUST | S | T1.34 |
| T1.36 | Audit: degenerate warp grid | RUST | S | T1.34 |
| T1.37 | Audit: mask polygon < 3 vertices | RUST | S | T1.34 |
| T1.38 | Audit: missing asset on disk (Critical) | RUST | M | T1.34 |
| T1.39 | Audit: out-of-range monitor index | RUST | S | T1.34 |
| T1.40 | Audit: schema_version too new (Critical) | RUST | S | T1.34 |
| T1.41 | `Toast` struct + `ToastQueue` | RUST | M | T1.34 |
| T1.42 | `toast_strip` egui primitive | RUST + DES | M | T1.41 |
| T1.43 | Wire ProjectAudit to load + AppState transitions | RUST | M | T1.34, T1.41 |
| T1.44 | Critical findings → `AppState::Failed` | RUST | M | T1.43 |
| T1.45 | Tracing spans: session_start, first_layer_added, first_warp_drag | RUST | S | T1.16 |
| T1.46 | Tracing spans: project_audit_warned, advanced_opened, undo_invoked, demo_clicked | RUST | S | T1.16, T1.43 |
| T1.47 | `ux_metrics` daily JSON sink | RUST | M | T1.45 |

---

## WP-1 — Explicit AppState machine

### Task T1.1: Define `AppState` enum (5 variants)

**Purpose**
Replace implicit `Option<RunningApp>` with explicit, typed app
states. Foundation for the launcher and Go-live transitions.

**Problem addressed**
Plan §11.1, §3.6: app states are unmodelled today; `Launcher`,
`GoLive`, `Failed` cannot exist as concepts.

**Implementation details**
- Add `AppState` enum to `app.rs` with five variants:
  `Booting`, `Launcher(LauncherState)`, `Editing(EditingState)`,
  `GoLive(EditingState)`, `Failed(FailureKind)`.
- Move existing `RunningApp` struct contents into a new
  `EditingState` struct (rename + move, no semantic changes).
- Stub `LauncherState` (empty struct for now; populated in T2.1).
- Stub `FailureKind` enum: `ProjectLoadFailed { reason: String }`,
  `RenderInitFailed`, `ProjectAuditCritical { findings:
  Vec<AuditFinding> }`. Last variant references T1.34 — gate on
  it or use a placeholder.
- Replace `App.state: Option<RunningApp>` with `App.state:
  AppState` (default `AppState::Booting`).
- Document `ProjectLoading(LoadProgress)` as a future variant in a
  module comment; do not implement.

**Dependencies**
T0.7.

**Can run in parallel**
No — gates everything else in Phase 1.

**Acceptance criteria**
1. `AppState` enum compiles with five variants.
2. `App.state` field is non-`Option`.
3. `cargo build --features v3` succeeds.
4. No public API breakage; `App::run` signature unchanged.
5. A unit test asserts `AppState::default() == AppState::Booting`
   (or whatever construction returns initially).

**Verification**
- `cargo test --features v3 app::state_machine`.
- Code review confirms `Option<RunningApp>` is gone.

**Risks / notes**
This is the *only* PR that lands the enum; T1.2 and T1.3 land
behaviour. Keep this PR mechanical to keep review fast.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.2: Migrate `App::resumed` to dispatch on AppState

**Purpose**
`resumed` (currently `app.rs:1196`) constructs `RunningApp`
directly. Migrate to construct an `EditingState` and place it
inside `AppState::Editing` (or `AppState::Launcher` once T2.1
lands; for now, default-into-`Editing` to preserve current
behaviour).

**Problem addressed**
Plan §11.1.

**Implementation details**
- `resumed` reads CLI flags + project path → calls
  `load_project_for_startup`.
- On success → constructs `EditingState` via the (still-monolithic
  for now) `init_running_app` and assigns
  `self.state = AppState::Editing(state)`.
- On failure → assigns `self.state = AppState::Failed(...)` and
  exits the event loop (preserves current `event_loop.exit()`
  behaviour for now; T1.44 changes Critical-finding routing).
- Preserve `--autostart` semantics.

**Dependencies**
T1.1.

**Can run in parallel**
With T1.3 only after T1.1 lands; otherwise sequential after T1.1.

**Acceptance criteria**
1. CLI launch with no project boots into `AppState::Editing`.
2. CLI launch with `*.rmap.json` + `--autostart` boots into
   `AppState::Editing` with the loaded project.
3. CLI launch with a nonexistent project file routes to
   `AppState::Failed`; the binary still exits cleanly.
4. macOS resume (suspend then wake) does not double-init (see
   T1.5).
5. Unit-testable transition function `Booting → Editing` extracted
   from inline code.

**Verification**
- Manual: `cargo run --features v3 -- assets/demo.svg --windowed
  --monitor 0` opens both windows.
- Manual: `cargo run --features v3 -- /nonexistent.rmap.json`
  exits cleanly with a logged error.

**Risks / notes**
Keep `event_loop.exit()` on failure for this task; T1.44 changes
the routing.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.3: Migrate `App::window_event` to dispatch on AppState

**Purpose**
The event handler at `app.rs:1263` does `self.state.as_mut()` and
unwraps. Migrate to a `match` on `&mut self.state`.

**Problem addressed**
Plan §11.1.

**Implementation details**
- `match self.state` → arms for each `AppState` variant.
- `Booting` and `Failed` arms ignore most events (still handle
  `CloseRequested`).
- `Editing(state)` arm runs the existing window_event body.
- `GoLive(state)` arm shares logic with `Editing` initially;
  delta is the show-day strip overlay (Phase 3).
- `Launcher(state)` arm is a stub for T2.1.

**Dependencies**
T1.1.

**Can run in parallel**
With T1.2 once T1.1 lands.

**Acceptance criteria**
1. `match` on `AppState` is exhaustive.
2. All current keyboard handlers (B/F/T/O/Esc) still work in
   `Editing`.
3. Drop-file handler still works in `Editing`.
4. Render path still runs in `Editing`.
5. `cargo clippy --features v3` is clean (no `unreachable_patterns`
   or `match_wildcard_for_single_variants` warnings).

**Verification**
- Manual smoke test: launch, drag a layer, hit B (blackout), hit T
  (test pattern), hit Esc (deselect).

**Risks / notes**
Preserve the existing comment block about layout-independent
physical-key matching.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.4: Per-state `ControlFlow` derivation

**Purpose**
Today `ControlFlow::Poll` is set globally at `app.rs:374`. After
WP-1, derive it from `AppState`.

**Problem addressed**
Plan WP-1 done-means item; battery / CPU savings in idle states.

**Implementation details**
- Inside `about_to_wait`, call
  `event_loop.set_control_flow(state.control_flow())` where
  `AppState::control_flow(&self) -> ControlFlow` returns:
  - `Poll` for `Editing`, `GoLive`.
  - `Wait` for `Launcher`, `Failed`.
  - `Wait` for `Booting`.

**Dependencies**
T1.2.

**Can run in parallel**
After T1.2.

**Acceptance criteria**
1. `Editing` state ticks at vsync (existing behaviour).
2. `Launcher` state does not consume CPU when idle (verified by
   `top` showing < 1% CPU when no input).
3. Transition from `Launcher → Editing` flips control flow without
   manual intervention.

**Verification**
Profile-watch: `top -pid <rmap-pid>` while in launcher state.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.5: macOS resume guard for new states

**Purpose**
The guard at `app.rs:1197` (`if self.state.is_some() { return; }`)
must handle every "already running" `AppState` variant, not just
`Some(RunningApp)`.

**Problem addressed**
Plan WP-1 done-means; macOS `resumed` fires more than once on
lifecycle changes; double-init must not occur.

**Implementation details**
- Replace the `is_some()` check with
  `matches!(self.state, AppState::Launcher(_) | AppState::Editing(_)
   | AppState::GoLive(_))`.
- Document why `Failed` is not in the list (Failed should re-init
  on resume to attempt recovery).

**Dependencies**
T1.2.

**Can run in parallel**
After T1.2.

**Acceptance criteria**
1. Suspend (Cmd-H) and resume rmap; no double-init log line
   appears.
2. Lock the screen and unlock; no double-init.
3. `Failed` state does re-init on resume (manual: trigger Failed,
   resume, observe re-attempt).

**Verification**
Manual macOS suspend/resume, log inspection.

**Risks / notes**
Easy to miss in the larger T1.2 PR; isolated task makes review
explicit.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.6: Verify `--list-monitors` and `--autostart` regression-free

**Purpose**
Both CLI paths bypass the normal `App::resumed` flow (the
`ListMonitorsApp` short-circuit; the `--autostart` skip). Verify
neither broke after WP-1.

**Problem addressed**
Plan WP-1 done-means.

**Implementation details**
- Smoke test: `target/release/rmap --list-monitors` outputs the
  monitor list and exits.
- Smoke test: `target/release/rmap path/to/test.rmap.json
  --autostart` boots straight into Editing.
- Add both as `tests/cli_smoke.rs` shell-harness tests.

**Dependencies**
T1.2, T1.3.

**Can run in parallel**
After T1.3.

**Acceptance criteria**
1. `--list-monitors` exits 0 and prints the expected format.
2. `--autostart` with a valid project enters Editing.
3. `--autostart` with an invalid project enters Failed and exits.
4. Tests run in CI under `--features v3`.

**Verification**
`cargo test --features v3 --test cli_smoke`.

**Suggested owner**
RUST + QA.

**Estimated scope**
S.

---

## WP-1.1 — Decompose `init_running_app`

### Task T1.7: Extract `init_gpu()`

**Purpose**
Pull GPU context construction out of `init_running_app` so the
launcher (T2.1) can use it without the rest.

**Problem addressed**
Plan WP-1.1; `init_running_app` is monolithic at ~130 lines.

**Implementation details**
- New function `init_gpu() -> Result<GpuContext>` in `app.rs`.
- Wraps `GpuContext::new()` with the same error mapping.
- `init_running_app` calls it.

**Dependencies**
T1.1.

**Can run in parallel**
With T1.8–T1.11 (independent extractions).

**Acceptance criteria**
1. `init_gpu` exists and returns `Result<GpuContext>`.
2. `init_running_app` calls it instead of `GpuContext::new`
   directly.
3. No behavioural change (`cargo run --features v3` works as
   before).

**Verification**
Build + smoke test.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.8: Extract `init_output_window()`

**Purpose**
Pull output-window + renderer construction out.

**Implementation details**
- Function: `init_output_window(event_loop, monitor, gpu,
  output_windowed) -> Result<(OutputWindow, Renderer)>`.
- Encapsulates `OutputWindow::new`, surface format selection,
  pipeline construction (`ColorPipeline`, `BlurPipeline`,
  `TransformPipeline`), `Renderer::new`, sleep assertion
  acquisition.

**Dependencies**
T1.7.

**Can run in parallel**
With T1.9–T1.11.

**Acceptance criteria**
1. Function exists, returns the tuple.
2. `init_running_app` calls it.
3. `SleepAssertion` is acquired inside this function.
4. Build + smoke unchanged.

**Verification**
Build, manual smoke, `cargo test`.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.9: Extract `init_control_window()`

**Purpose**
Pull control-window construction out; allow it to be optional
(matches existing fallback-on-failure pattern).

**Implementation details**
- Function: `init_control_window(event_loop, gpu) ->
  Option<ControlWindow>`.
- Logs warning + returns `None` on failure (preserves D-01
  fallback at `app.rs:521–528`).

**Dependencies**
T1.7.

**Can run in parallel**
With T1.8, T1.10, T1.11.

**Acceptance criteria**
1. Function exists, returns Option.
2. Failure path logs warning and continues.
3. Smoke unchanged.

**Verification**
Build, smoke.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.10: Extract `init_inputs()`

**Purpose**
Pull keyboard + feature-gated audio / MIDI / OSC source setup out.

**Implementation details**
- Function: `init_inputs() -> InputsBundle` where `InputsBundle`
  is a small struct with `keyboard`, optional `_audio_capture`,
  optional `midi`, optional `osc`.
- All `#[cfg(feature = ...)]` gates live inside this function.

**Dependencies**
T1.7.

**Can run in parallel**
With T1.8, T1.9, T1.11.

**Acceptance criteria**
1. Function exists; returns the bundle.
2. Audio/MIDI/OSC features still gate their respective fields.
3. Failure of any source logs and continues (preserves current
   behaviour).
4. Smoke unchanged with each feature combination
   (`--features audio`, `--features midi`, `--features osc`,
   default).

**Verification**
Build + smoke, four feature combinations.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.11: Extract `init_render_graph()`

**Purpose**
Pull compositor + warp + gamma + overlay + warp_rt + LayerState
construction out.

**Implementation details**
- Function: `init_render_graph(gpu, project, output_size,
  surface_format) -> Result<RenderGraph>` where `RenderGraph`
  bundles `Compositor`, `Vec<WarpRenderer>`, `GammaPipeline`,
  `OverlayPipeline`, `warp_rt`, `warp_rt_view`, `Vec<LayerState>`,
  `SvgLayerPipeline`.

**Dependencies**
T1.8, T1.9.

**Can run in parallel**
After T1.8 + T1.9.

**Acceptance criteria**
1. Function exists.
2. `init_running_app` calls it.
3. Render frame is identical (golden image regression test on a
   simple SVG).

**Verification**
`cargo test --features gpu-tests` for golden frames.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.12: Reduce `init_running_app` to orchestrator

**Purpose**
After T1.7–T1.11, `init_running_app` should be ~30 lines: call
the five extractors, build `EditingState`, return it.

**Dependencies**
T1.7–T1.11.

**Can run in parallel**
No — closes the WP-1.1 cluster.

**Acceptance criteria**
1. `init_running_app` ≤ 50 lines (down from ~130).
2. No behavioural change.
3. Each extractor is independently callable from a unit test that
   mocks the others where possible.

**Verification**
Line count: `wc -l` on the function. Smoke test.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## WP-2 — Command + Undo/Redo

### Task T1.13: Rename `ControlEvent` → `Command`

**Purpose**
Mechanical rename. Makes the next tasks readable; does not change
behaviour.

**Problem addressed**
Plan §11.2.

**Implementation details**
- `controls/mod.rs:20`: `ControlEvent` → `Command`.
- All call sites (`app.rs::dispatch_control_event`, the keyboard
  source, MIDI / OSC sources) renamed.
- `dispatch_control_event` → `apply_command_legacy` (T1.16
  introduces the new `apply_command`).
- Do **not** touch the `ParamSet` variant or the
  `#![allow(dead_code)]` block — preserve M7 stubs as-is.

**Dependencies**
T1.1.

**Can run in parallel**
With T1.7–T1.12.

**Acceptance criteria**
1. `cargo build --features v3` succeeds.
2. `cargo grep ControlEvent` returns zero matches in production
   code (test-only references allowed).
3. M7 stub variants (`ParamSet`, `Source::read`) untouched.
4. No behavioural change.

**Verification**
Build, smoke.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.14: Reverse-storage runtime invariant + property-test contract *(REVISED post-practitioner-review)*

**Purpose**
Encode the three Reverse-storage rules (whole-enum, effects-Vec,
project-snapshot) so naive Reverse capture is caught — but at
*runtime in test builds* and via the proptest harness, not by the
type system. Same safety property; lower contributor cost; same
proptest-property invariant.

**Background**
Original task required compile-error-on-missing-Reverse. The
practitioner / contributor friction cost was judged too high
("I just want to add a slider, why does adding it break the
build?"). Compile-time enforcement deferred to a v3.1 refactor;
v3 ships with runtime + proptest enforcement.

**Problem addressed**
Plan §11.2 + Risk R11.

**Implementation details**
- New trait `Reversible: Sized` with associated type `type Reverse:
  Sized;` and method `fn apply(self, project: &mut Project) ->
  Self::Reverse;`.
- For each command category, define the canonical struct shape:
  - **Field replacements** (gamma, opacity): `SetGamma { new: f32,
    old: f32 }`.
  - **Enum-variant replacements** (Modulator, BlendMode, Effect):
    `SetModulator { layer, effect_idx, field, new: Modulator, old:
    Modulator }` where `old` is the *full* enum value.
  - **Effects-Vec replacements** (drag transform): `SetLayerEffects
    { layer, new: Vec<Effect>, old: Vec<Effect> }` where `old` is
    the *full* effects Vec.
  - **Project-snapshot replacements** (scene recall): `ApplyProject
    Snapshot { new: serde_json::Value, old: serde_json::Value,
    non_undoable: bool }`.
- Provide helper constructors that *read the current state at
  construction time* so call sites cannot forget:
  - `Project::current_modulator(layer, effect_idx, field) ->
    Modulator`
  - `Project::current_layer_effects(layer) -> Vec<Effect>`
  - `Project::current_snapshot() -> serde_json::Value`
  - Convenience: `Command::set_gamma(project, new)` returns
    `Command::SetGamma { new, old: project.gamma }`.
- Doc-comment the three rules at the `Command` enum head;
  `#[deny(missing_docs)]` on the `Command` module enforces an
  explanation per variant.
- **Runtime safety via `debug_assert!`:** in
  `Reversible::apply`, before mutating, assert that the stored
  `old` matches `Project::current_*` for the relevant field. Catches
  contributor errors in test builds without burdening release builds.
- **Real safety via property test (T1.17):** any sequence of
  commands + matching undos returns the project to byte-equal
  serde_json. This is the contract.

**Dependencies**
T1.13.

**Can run in parallel**
No — gates all migration tasks.

**Acceptance criteria**
1. `Command` enum has the four representative variants covering
   each Reverse pattern.
2. `Reversible::apply` returns `Self::Reverse` and is total (no
   `unreachable!()`).
3. The three Reverse-storage rules are documented at the
   `Command` module head verbatim from §11.2.
4. `debug_assert!`s fire in test builds when a hand-constructed
   command has stale `old`.
5. Helper constructors exist (`set_gamma`, `set_modulator`,
   `set_layer_effects`, `apply_snapshot`) and read the old state
   internally.
6. The crate compiles with `#[deny(missing_docs)]` on the
   `Command` module.

**Verification**
- `cargo build --features v3` green.
- `cargo test --features v3 reverse_storage_debug_assert` — a
  hand-constructed stale command panics in test build.
- `cargo doc --features v3 --no-deps` produces docs for every
  variant.

**Practitioner relevance**
A future contributor (PO, design, an external pull request)
adding a new slider does not have to pass a compile-time gate;
they get clear runtime guidance from `debug_assert!`s and the
proptest harness catches any subtle bugs they miss. Same
real-world safety; no architectural pricing tag for the next
contributor.

**Risks / notes**
- Compile-time enforcement is **explicitly v3.1 backlog**;
  recorded in the v3.1 deferral list inside `T4.23` capability
  roadmap doc.
- The proptest harness must include strategies for every Command
  variant; tasks T1.18+ extend the strategy as variants ship.

**Suggested owner**
RUST.

**Estimated scope**
M — was L before the soft revision; ~1 day saved.

---

### Task T1.15: `UndoStack<C>` with `non_undoable` flag

**Purpose**
Implement the undo / redo stack that consumes `Command` Reverse
values.

**Problem addressed**
Plan WP-2.

**Implementation details**
- New type `UndoStack` in a new module `app/undo.rs`.
- Internal: two `VecDeque<Command::Reverse>` for undo and redo.
- Soft cap (200) with FIFO trim on overflow.
- Method `push(cmd: Command, non_undoable: bool)`: applies the
  command, pushes the Reverse onto the undo deque (skipped if
  `non_undoable`), clears the redo deque.
- Method `undo(project) -> bool`: pops one Reverse from undo,
  applies it, pushes the resulting Reverse onto redo. Returns
  whether something was undone.
- Method `redo(project) -> bool`: symmetric.
- Method `len()` and `redo_len()` for tests.

**Dependencies**
T1.14.

**Can run in parallel**
With T1.16, T1.17.

**Acceptance criteria**
1. Pushing N commands grows undo by N; `redo_len() == 0`.
2. Calling `undo` once shrinks undo by 1, grows redo by 1.
3. Pushing a `non_undoable: true` command grows undo by 0.
4. Pushing > 200 commands FIFO-trims the oldest.
5. After `undo` then `push`, redo is cleared.

**Verification**
`cargo test --features v3 app::undo`.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.16: Central `apply_command` function

**Purpose**
Replace `dispatch_control_event` with `apply_command(state: &mut
EditingState, cmd: Command)` that:
1. Routes to `Reversible::apply` (or to `LegacyApply` for the M7
   `ParamSet` stub),
2. Pushes onto `state.undo_stack`,
3. Records a `tracing` span for telemetry (T1.45+).

**Problem addressed**
Plan §11.2.

**Implementation details**
- Inside `EditingState`, add field `undo_stack: UndoStack`.
- New free function `apply_command(state, cmd)`.
- The legacy `apply_command_legacy` (renamed in T1.13) becomes
  `apply_command`'s sole caller for the M7 `ParamSet` variant,
  which is `non_undoable`.
- Side-effects (e.g., `RebuildLayers` after `AddLayer`) are
  returned as a `SideEffect` enum, applied by the event loop.

**Dependencies**
T1.14.

**Can run in parallel**
With T1.15, T1.17.

**Acceptance criteria**
1. `apply_command` exists.
2. Every call to `apply_command_legacy` in `app.rs` is replaced by
   `apply_command`.
3. `SideEffect` enum exists with at least
   `None`, `RebuildLayers`, `RegisterScenePreview` variants.
4. The `SceneRecall` arm routes through
   `Command::ApplyProjectSnapshot`.

**Verification**
- Build + smoke.
- A unit test that `apply_command(state, Command::Noop)` does not
  push to the undo stack (assuming `Noop` is `non_undoable`).

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.17: Proptest harness on `Command::Noop`

**Purpose**
Build the property test that any sequence of commands +
matching undos returns the project to a byte-equal
`serde_json::Value`. Stub on `Command::Noop` first; later
migration tasks add their command variants to the harness.

**Problem addressed**
Plan WP-2 done-means; R11 mitigation.

**Implementation details**
- Add `proptest` as a `dev-dependency`.
- New test file `tests/command_undo_proptest.rs`.
- Strategy: generate a `Vec<Command>` of length 0..50; apply all,
  then undo all; assert
  `serde_json::to_value(&project_before) ==
   serde_json::to_value(&project_after)`.
- Initially the only generator is `Command::Noop`; later tasks
  extend the strategy.
- Add a corresponding "redo round-trip" test: apply N, undo all,
  redo all, assert equality with post-apply state.

**Dependencies**
T1.15, T1.16.

**Can run in parallel**
After T1.15, T1.16.

**Acceptance criteria**
1. `cargo test --features v3 --test command_undo_proptest` runs at
   least 256 cases (proptest default).
2. All cases pass.
3. The test file is set up so adding a new `Command` variant only
   requires extending the strategy enum.
4. CI runs the proptest on every PR labelled `v3-foundation`.

**Verification**
CI green.

**Risks / notes**
Proptest generation must be deterministic-on-failure; use
`PROPTEST_CASES=1024` in CI for robustness.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.18: Migrate always-visible bindings batch

**Purpose**
Migrate the ~10 always-visible egui bindings to commands:
- `project.gamma`, `project.brightness`, `project.contrast`
- `project.crossfade_duration_s`, `project.output_windowed`
- `WarpMesh.mask_feather`, `WarpMesh.rows`, `WarpMesh.cols`

**Problem addressed**
Plan WP-2 mutation surface table, row 1.

**Implementation details**
- Build the `command_slider(ui, label, value, range, |new|
  Command::SetGamma { ... })` egui primitive.
- Build `command_checkbox(ui, label, value, |new|
  Command::SetOutputWindowed { ... })`.
- Replace each `egui::Slider::new(&mut project.X, ...)` site in
  `windows/control_panel.rs` with `command_slider` calls.
- Each command emits via `apply_command`.
- Sliders use the *live preview* pattern: draw against a simulated
  value on `dragged()`; emit the `Command` on `drag_stopped()`.

**Dependencies**
T1.17.

**Can run in parallel**
With T1.19, T1.20, T1.21, T1.24, T1.27, T1.28, T1.30, T1.31.

**Acceptance criteria**
1. All 10 always-visible bindings emit commands; none bind `&mut
   project.X` directly.
2. Slider drag still feels live (no stutter); the command fires
   only on drag end.
3. Cmd-Z reverses each binding's last edit.
4. Proptest harness is extended to include all 10 variants.
5. `grep -n "&mut project\." windows/control_panel.rs` returns
   zero matches for fields touched in this task.

**Verification**
Manual smoke + extended proptest run.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.19: Migrate per-layer Layers tab bindings

**Purpose**
Migrate the per-layer fields touched in the Layers tab:
- `layer.opacity` (slider)
- `layer.enabled` (checkbox)
- `layer.blend_mode` (combobox — enum-variant Reverse rule
  applies)

**Implementation details**
- Add `Command::SetLayerOpacity { layer_idx, new, old }`.
- Add `Command::SetLayerEnabled { layer_idx, new, old }`.
- Add `Command::SetLayerBlendMode { layer_idx, new: BlendMode, old:
  BlendMode }` — whole-enum Reverse.
- Replace direct bindings at `control_panel.rs:525, 547` with
  command-emitting widgets.

**Dependencies**
T1.17.

**Can run in parallel**
With T1.18, T1.20+.

**Acceptance criteria**
1. Three commands added.
2. Layer enable toggle reversible by Cmd-Z.
3. Blend mode picker reversible (whole-enum Reverse smoke test).
4. Proptest covers all three.

**Verification**
Manual + proptest.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.20: Migrate Layers tab buttons

**Purpose**
Migrate the buttons in the Layers tab:
- "Add layer" (path input + button at `control_panel.rs:482`)
- ↑ / ↓ reorder buttons at `control_panel.rs:550, 553`

**Implementation details**
- `Command::AddLayer { kind: LayerKind, position: usize }`. Reverse
  is `Command::RemoveLayer { idx }`.
- `Command::SwapLayers { i, j }`. Self-reverse.
- Both emit `SideEffect::RebuildLayers`.

**Dependencies**
T1.17.

**Can run in parallel**
With T1.18, T1.19, T1.21+.

**Acceptance criteria**
1. Two new commands.
2. Add → Cmd-Z removes the layer.
3. Reorder → Cmd-Z restores order.
4. `selected_layer` state survives undo correctly (does not
   point past `len()`).

**Verification**
Manual + proptest.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.21: Migrate `show_effect` per-effect sliders

**Purpose**
Migrate the ~9 sliders inside `show_effect` (`control_panel.rs:848`):
- `Effect::Color::hue/saturation/brightness/contrast` (when
  `Modulator::Static`) — though these go through `modulator_slider`,
  see T1.23.
- `Effect::Transform::translate[0]/translate[1]` (direct sliders).
- `Effect::Blur::radius_px` (modulator-wrapped — T1.23).
- `Effect::Transform::rotate_deg/scale_x/scale_y` (modulator-
  wrapped — T1.23).

This task focuses on the *non-modulator* paths
(`Effect::Transform.translate`).

**Implementation details**
- `Command::SetEffectTransformTranslate { layer, effect_idx, new,
  old }` — but per the effects-Vec rule, the Reverse stores the
  whole `Vec<Effect>`. So actually:
- `Command::SetLayerEffects { layer, new: Vec<Effect>, old:
  Vec<Effect> }` is the universal command for any effect-chain
  mutation. Effect-parameter sliders all funnel into it.

**Dependencies**
T1.17.

**Can run in parallel**
With other T1.18+ migrations.

**Acceptance criteria**
1. Translate sliders for `Effect::Transform` emit
   `SetLayerEffects` with whole-Vec Reverse.
2. Cmd-Z restores prior translate.
3. Proptest covers `SetLayerEffects`.

**Verification**
Manual + proptest.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.22: Migrate Modulator picker (whole-enum Reverse smoke test)

**Purpose**
The combobox at `control_panel.rs:907–922` swaps `Modulator`
variants wholesale. This is the canonical whole-enum Reverse case.

**Problem addressed**
Plan §11.2 rule 1.

**Implementation details**
- `Command::SetModulator { layer, effect_idx, field: ModulatorField,
  new: Modulator, old: Modulator }` — both old and new are full
  enum values.
- `ModulatorField` is a small enum identifying *which* Modulator
  inside the effect (e.g., `ColorHue`, `BlurRadius`).
- Migrate the combobox.
- **Smoke test (mandatory):** unit test that constructs a
  `Modulator::Sine { period_s: 2.0, amp: 0.3, phase: 1.0, offset:
  0.5 }`, applies a `Command::SetModulator` to flip it to
  `Modulator::Static(0.7)`, then undoes — asserts every field of
  the original Sine is restored byte-equal.

**Dependencies**
T1.17.

**Can run in parallel**
With other migrations.

**Acceptance criteria**
1. Variant switch is reversible.
2. The smoke test passes.
3. Proptest extended with `Modulator` strategies covering all six
   variants.

**Verification**
`cargo test --features v3 modulator_whole_enum_reverse`.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.23: Migrate `modulator_slider` parameter widgets

**Purpose**
The 15 instances of `modulator_slider` (`control_panel.rs:890`)
that mutate fields *inside* a Modulator variant (e.g.,
`Modulator::Sine.period_s`).

**Implementation details**
- These all reduce to `Command::SetLayerEffects` (effects-Vec
  rule) because changing a field inside a `Modulator::Sine`
  inside an `Effect::Color` mutates the effects Vec.
- Or: a finer-grained `Command::SetModulatorParameter { ..., new:
  Modulator, old: Modulator }` (still whole-enum Reverse). Choose
  one approach; recommend the finer-grained command for cleaner
  telemetry (one event per parameter tweak).
- Update `modulator_slider` helper in `control_panel.rs` to take a
  `dispatcher` callback that emits commands.

**Dependencies**
T1.22.

**Can run in parallel**
With other migrations.

**Acceptance criteria**
1. All 15 `modulator_slider` call sites emit commands.
2. Tweaking a Sine `period_s` is reversible via Cmd-Z.
3. Proptest covers Sine / Triangle / Noise / Bpm / Audio
   variants.

**Verification**
Manual + proptest.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.24: Migrate scene_editor drag-translate

**Purpose**
Migrate the drag-translate path in
`windows/scene_editor.rs::handle_scene_input`
(`scene_editor.rs:412–419`).

**Implementation details**
- Drag *start* captures the layer's current `Vec<Effect>` as
  `old`.
- Drag *end* (`response.drag_stopped()`) emits
  `Command::SetLayerEffects { layer, new, old }`.
- During the drag, mutate the layer in-place for live feedback;
  the final command captures the cumulative delta.
- This is the *effects-Vec Reverse rule* in action.

**Dependencies**
T1.17.

**Can run in parallel**
With T1.25, T1.26 (after T1.24 lands the pattern).

**Acceptance criteria**
1. Dragging a layer emits exactly one command on drag end.
2. Mid-drag motion is visually live.
3. Cmd-Z restores prior position byte-equal.
4. Proptest covers a "drag-then-undo" sequence.

**Verification**
Manual + proptest.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.25: Migrate scene_editor drag-scale

**Purpose**
Same pattern as T1.24 for shift-drag scale.

**Dependencies**
T1.24 (pattern).

**Can run in parallel**
With T1.26.

**Acceptance criteria**
1. Shift-drag emits one command on drag end.
2. Cmd-Z restores prior scale.

**Verification**
Manual + proptest.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.26: Migrate scene_editor drag-rotate (effects-Vec Reverse smoke test)

**Purpose**
Same pattern as T1.24 for alt-drag rotate. **This is the canonical
effects-Vec Reverse smoke test** because `mutate_transform_effect`
(`scene_editor.rs:147`) appends a default `Effect::Transform` if
the layer's effects chain doesn't have one.

**Problem addressed**
Plan §11.2 rule 2.

**Implementation details**
- The smoke test (mandatory): start with a layer whose `effects`
  Vec does *not* contain a Transform effect. Alt-drag to rotate.
  Cmd-Z. Assert the layer's `effects` Vec is back to the original
  length and contents — no stray Transform effect left.

**Dependencies**
T1.24.

**Can run in parallel**
With T1.25.

**Acceptance criteria**
1. Alt-drag emits one command on drag end.
2. The "no stray Transform after undo" smoke test passes.
3. Proptest covers the create-or-update path.

**Verification**
`cargo test --features v3 effects_vec_reverse_no_stray`.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.27: Migrate mask vertex drag/insert/delete

**Purpose**
Three mutation paths: drag (`scene_editor.rs:451`), insert
(`control_panel.rs:277`), delete (`control_panel.rs:289`).

**Implementation details**
- `Command::SetMaskVertex { warp, idx, new, old }`.
- `Command::AddMaskVertex { warp, after_idx, point }`. Reverse:
  `Command::RemoveMaskVertex { warp, idx }`.
- `Command::RemoveMaskVertex { warp, idx, removed_point }`.
  Reverse: `Command::AddMaskVertex { warp, after_idx, point:
  removed_point }`.

**Dependencies**
T1.17.

**Can run in parallel**
With other migrations.

**Acceptance criteria**
1. Three commands.
2. Each is reversible via Cmd-Z.
3. Proptest covers all three.
4. The "≥ 3 vertices" guard at `control_panel.rs:290` is preserved
   on the command path.

**Verification**
Manual + proptest.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.28: Migrate Mapping tab buttons

**Purpose**
- "Reset to identity" at `control_panel.rs:767`.
- "clear mask" at `control_panel.rs:789`.
- Zone-template buttons at `control_panel.rs:783–787`.

**Implementation details**
- `Command::ResetWarpToCornerPin { warp_idx, old: WarpMesh }`.
- `Command::ClearMaskPolygon { warp_idx, old_polygon: Vec<[f32;
  2]> }`.
- `Command::ApplyZoneTemplate { warp_idx, template_name: &'static
  str, old_polygon: Vec<[f32; 2]> }`.

**Dependencies**
T1.17.

**Can run in parallel**
With other migrations.

**Acceptance criteria**
1. Three commands.
2. Reset is reversible (full WarpMesh restored).
3. Proptest covers all three.

**Verification**
Manual + proptest.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.29: Migrate Effects-tab Apply preset button

**Purpose**
The "Apply" button at `control_panel.rs:435` replaces a layer's
entire `effects` chain.

**Implementation details**
- Routes through `Command::SetLayerEffects` (already added in
  T1.21).
- Just wire the click handler to emit it; trivial after T1.21.

**Dependencies**
T1.21.

**Can run in parallel**
After T1.21.

**Acceptance criteria**
1. Apply is reversible.
2. Confirms preset application + undo round-trips byte-equal.

**Verification**
Manual + proptest.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.30: Migrate scene save/recall as `ApplyProjectSnapshot` (project-snapshot Reverse smoke test)

**Purpose**
The Scenes-tab save/recall buttons at `control_panel.rs:818, 830`
and the keyboard-driven recall in `dispatch_control_event`. These
are the canonical project-snapshot Reverse case.

**Problem addressed**
Plan §11.2 rule 3.

**Implementation details**
- `Command::SaveScene { slot }`. Reverse: previous scene at that
  slot (or "didn't exist").
- `Command::ApplyProjectSnapshot { new: Value, old: Value,
  non_undoable: bool }`. Scene recall calls it with `non_undoable:
  false`; crossfade-tick calls it with `non_undoable: true`.
- Crossfade tick at `app.rs:1508` switches from direct
  `restore_scene` to `apply_command(Command::ApplyProjectSnapshot
  { non_undoable: true, ... })`.
- **Smoke test (mandatory):** save scene to slot 1, modify project,
  recall slot 1, undo — assert project byte-equal to pre-recall
  state.
- **Crossfade test (mandatory):** start a 2 s crossfade, let it
  run to completion, assert `UndoStack::len()` did not grow by
  ~120 (the number of frames during the crossfade).

**Dependencies**
T1.17.

**Can run in parallel**
With other migrations.

**Acceptance criteria**
1. Save / recall reversible via Cmd-Z.
2. Crossfade tick does not pollute the undo stack.
3. Both smoke tests pass.

**Verification**
`cargo test --features v3 snapshot_reverse_smoke crossfade_undo_excluded`.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.31: Migrate `DroppedFile` layer-add

**Purpose**
The drop handler at `app.rs:1287` mutates `state.project.layers`
directly. Migrate to a command.

**Implementation details**
- Routes through `Command::AddLayer` from T1.20.
- Just wire `WindowEvent::DroppedFile` to call
  `apply_command(Command::AddLayer { ... })`.

**Dependencies**
T1.20.

**Can run in parallel**
After T1.20.

**Acceptance criteria**
1. Drop emits a command.
2. Cmd-Z removes the dropped layer.

**Verification**
Manual smoke.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.32: Output-state toggles via `Command` for telemetry only

**Purpose**
B/F/T/O hotkeys mutate `state.output.state` directly. Route through
commands (`non_undoable: true`) so telemetry catches them.

**Implementation details**
- `Command::ToggleBlackout`, `ToggleFreeze`, `CycleTestPattern`,
  `ToggleEditorOverlay` — all `non_undoable`.
- The keyboard handler at `app.rs:1371–1392` and the
  `ControlEvent::Blackout/Freeze` dispatch all funnel through
  `apply_command`.
- Telemetry span fires for each.

**Dependencies**
T1.16.

**Can run in parallel**
With other migrations.

**Acceptance criteria**
1. B/F/T/O still work via keyboard.
2. They emit commands but do **not** enter the undo stack.
3. Tracing logs show `tracing::info!` events for each.

**Verification**
`cargo test --features v3 output_state_non_undoable`.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.33: Test — file-watcher hot-reloads excluded from undo

**Purpose**
The plan's rule "file-watcher reloads do *not* enter the undo
stack" is easy to violate accidentally. Lock it in with a test.

**Implementation details**
- Test fixture: a project with one SVG layer.
- Apply zero user commands; `UndoStack::len() == 0`.
- Touch the SVG file 5 times (file-watcher fires).
- Wait until reload commands have settled.
- Assert `UndoStack::len() == 0` still.

**Dependencies**
T1.15, T1.31.

**Can run in parallel**
After both deps land.

**Acceptance criteria**
1. Test exists and passes.
2. Test runs in CI.

**Verification**
`cargo test --features v3 hot_reload_excluded_from_undo`.

**Risks / notes**
File-watcher events are async; may need a deterministic
synchronisation point (e.g., a debouncer's flush method).

**Suggested owner**
RUST + QA.

**Estimated scope**
S.

---

## WP-15 — ProjectAudit + Toast

### Task T1.34: Define `ProjectAudit` + `AuditFinding` types

**Purpose**
Foundation for all audit findings.

**Implementation details**
- New module `project/audit.rs`.
- `pub struct ProjectAudit;`
- `pub struct AuditFinding { kind: AuditKind, severity: Severity,
  message: String, autofix: Option<Command> }`.
- `pub enum Severity { Info, Warn, Critical }`.
- `pub enum AuditKind { ZeroScale { layer_idx }, DegenerateWarp {
  warp_idx }, MaskTooFew { warp_idx, vertex_count }, MissingAsset
  { layer_idx, path }, MonitorOutOfRange { requested, available },
  SchemaTooNew { project_version, max_supported }, EmptyProject }`
  — start with this set; add more as findings ship.
- `ProjectAudit::run(project: &Project, env: &AuditEnv) ->
  Vec<AuditFinding>` where `AuditEnv` carries available monitor
  count and any other env state.

**Dependencies**
T1.1.

**Can run in parallel**
With T1.7+ once T1.1 lands.

**Acceptance criteria**
1. Types compile.
2. `ProjectAudit::run` returns an empty `Vec` for a project with
   no issues.
3. Module documentation lists all `AuditKind` variants.

**Verification**
`cargo test --features v3 project::audit`.

**Suggested owner**
RUST + PO (copy review on `message` strings).

**Estimated scope**
M.

---

### Task T1.35: Audit — zero-scale layer

**Purpose**
The headline failure mode caught during the original audit:
`transform.scale = [0, 0]` in `~/p1.rmap.json`.

**Implementation details**
- For each layer, check if `transform.scale == [0.0, 0.0]` (or
  either component < 1e-6).
- Severity: `Warn`.
- Message: `"Layer {id} has zero scale (invisible). [Auto-fix]"`.
- Autofix: `Command::SetLayerEffects { ... new effects with
  Transform.scale = [1.0, 1.0] ... }`.
- Unit-test against the fixture file `~/p1.rmap.json`.

**Dependencies**
T1.34.

**Can run in parallel**
With T1.36–T1.40.

**Acceptance criteria**
1. Loading `~/p1.rmap.json` emits one `ZeroScale` finding for
   each layer with zero scale.
2. Auto-fix command restores `[1.0, 1.0]`.
3. Test passes.

**Verification**
`cargo test --features v3 audit_zero_scale`.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.36: Audit — degenerate warp grid *(REPRIORITISED — P1, ship-if-slack)*

**Priority post-revision:** Drop from P0-must-ship-for-M1 to P1
within Phase 1. If Phase 1 has slack (calendar or engineering),
ship; otherwise defer to v3.1.

**Background**
Practitioner review: the wedding-scale audience rarely produces a
degenerate warp grid through normal use. Two P0 audit findings
(zero-scale, missing-asset) cover the headline failures. This and
the next three findings are useful but not ship-blocking.

**Implementation details**
- A warp grid is "degenerate" if rows < 2, cols < 2, or any row
  has different `len()`. Plan §11.4.
- Severity: `Warn`.
- Autofix: `Command::ResetWarpToCornerPin { warp_idx, old }`.

**Dependencies**
T1.34.

**Can run in parallel**
With T1.35–T1.40.

**Acceptance criteria**
1. A 1×2 grid triggers the finding.
2. A non-rectangular grid (rows of differing length) triggers it.
3. A 2×2 healthy grid does not.

**Verification**
`cargo test --features v3 audit_degenerate_warp`.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.37: Audit — mask polygon < 3 vertices *(REPRIORITISED — P1, ship-if-slack)*

**Priority post-revision:** P1 within Phase 1; v3.1 fallback. See
T1.36 background.

**Implementation details**
- Severity: `Info`.
- Autofix: `Command::ClearMaskPolygon { warp_idx, old_polygon }`.

**Dependencies**
T1.34.

**Acceptance criteria**
1. 0/1/2-vertex masks trigger the finding.
2. ≥ 3-vertex masks do not.

**Verification**
`cargo test --features v3 audit_mask_too_few`.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.38: Audit — missing asset on disk (Critical) *(EXTENDED — relink autofix added)*

**Priority post-revision:** P0-must-ship. Practitioner-elevated.

**Background**
Original task offered "remove this layer" as the only autofix.
Practitioner review: cross-machine project portability is a real
wedding-DJ workflow ("laptop A's path doesn't exist on laptop B").
The autofix needs a **relink** option, not just a remove option.
Implementation links to T2.24 (missing-media relink flow).

**Implementation details**
- For each layer, stat `kind.asset_path()` (resolving relative
  paths per T2.23 portability convention).
- Severity: `Critical`.
- Autofix metadata exposes **two actions**:
  1. **Find this file…** — opens the relink flow (T2.24).
  2. **Remove this layer** — confirmation-required.
- New command `Command::RelinkAssetPath { layer_idx, new_path,
  old_path }` for the relink action; Reverse stores the previous
  `LayerKind` (path or content).
- Critical-severity routes to `AppState::Failed` only when the
  user does not relink — the toast in `AppState::Editing` is the
  first attempt; on confirmation of "Remove this layer" or
  refusal to relink, the project enters Failed.

**Dependencies**
T1.34, **T2.24** (relink flow lives in Phase 2; T1.38 extension
is feature-flagged behind T2.24 readiness).

**Acceptance criteria**
1. Project referencing a nonexistent SVG triggers Critical.
2. Toast offers both **Find this file…** and **Remove this
   layer**.
3. Successful relink → `Command::RelinkAssetPath` → audit re-runs
   → no more findings.
4. Refused relink → routes to `AppState::Failed`.
5. Healthy project: no finding.

**Verification**
- `cargo test --features v3 audit_missing_asset_relink_path` —
  unit test on the autofix command.
- Manual: move an asset on disk after saving the project; reopen.

**Practitioner relevance**
The wedding-DJ "second laptop" failover scenario specifically
needs this. Without it, every cross-machine project load is a
wall.

**Suggested owner**
RUST.

**Estimated scope**
M (was M; extended scope absorbed via T2.24 doing the heavy UI
lift).

---

### Task T1.39: Audit — out-of-range monitor index *(REPRIORITISED — P1, ship-if-slack)*

**Priority post-revision:** P1 within Phase 1; v3.1 fallback.

**Implementation details**
- Severity: `Warn`.
- Autofix: `Command::SetOutputMonitorIndex { new: 0, old }`.

**Dependencies**
T1.34.

**Acceptance criteria**
1. Project with `output_monitor_index: 99` on a single-monitor
   system triggers it.

**Verification**
Unit test with a mocked `AuditEnv`.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.40: Audit — schema_version too new (Critical) *(REPRIORITISED — P1, ship-if-slack)*

**Priority post-revision:** P1 within Phase 1; v3.1 fallback. The
existing migration system handles forward-compatible cases
already; this is the strict-fail case for *future* incompatible
versions, low-frequency in practice.

**Implementation details**
- If the loaded project's `schema_version` exceeds the binary's
  max supported, refuse to load.
- Severity: `Critical`.
- No autofix (suggest binary upgrade).
- Routes to `AppState::Failed`.

**Dependencies**
T1.34.

**Acceptance criteria**
1. Project with `schema_version: 99` triggers Critical.
2. Project with current `schema_version` (3) does not.

**Verification**
Unit test.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.41: `Toast` struct + `ToastQueue`

**Purpose**
Plan §11.5. A small in-process notification queue.

**Implementation details**
- New module `windows/toast.rs`.
- `pub struct Toast { kind: ToastKind, message: String, action:
  Option<ToastAction>, ttl: Duration }`.
- `pub enum ToastKind { Info, Warn, Error }`.
- `pub struct ToastAction { label: String, command: Command }`.
- `pub struct ToastQueue { ... }` with `push`, `drain_expired`,
  `iter_visible(max: usize)`.
- Open question D4 (max visible / duration): default 3 visible,
  4 s for Info, 6 s for Warn, sticky for Error.

**Dependencies**
T1.34 (uses `Command` from T1.14).

**Can run in parallel**
With T1.35–T1.40.

**Acceptance criteria**
1. `ToastQueue::push` adds a toast.
2. `drain_expired` removes toasts whose TTL elapsed.
3. `iter_visible(3)` returns at most 3 toasts.
4. Sticky toasts (Error) never expire automatically.

**Verification**
Unit tests.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.42: `toast_strip` egui primitive

**Purpose**
Render the toast queue in the canvas top-right.

**Implementation details**
- Function `toast_strip(ui: &mut Ui, queue: &mut ToastQueue) ->
  Option<Command>` returning a command if the user clicked an
  action button.
- Visual treatment: see `T0.5` Advanced wireframe (or a separate
  toast wireframe).
- One soft-eased fade-in on push; fade-out on TTL.

**Dependencies**
T1.41.

**Can run in parallel**
After T1.41.

**Acceptance criteria**
1. Toasts visible in the top-right of the editor.
2. Clicking the action button emits the carried `Command`.
3. Toasts fade out cleanly.

**Verification**
Manual smoke + a snapshot test if `egui_kittest` is mature
enough.

**Suggested owner**
RUST + DES (design review).

**Estimated scope**
M.

---

### Task T1.43: Wire ProjectAudit into load + AppState transitions

**Purpose**
Run `ProjectAudit::run` after every project load; push findings to
the toast queue.

**Implementation details**
- In `load_project_for_startup` and any future load path, call
  `ProjectAudit::run(&project, &env)`.
- Findings of severity `Info` or `Warn` push to the toast queue.
- Findings of `Critical` route to `AppState::Failed` (T1.44).
- `AppState::Editing` only entered if no Critical findings.

**Dependencies**
T1.34, T1.41.

**Can run in parallel**
After both deps.

**Acceptance criteria**
1. Loading `~/p1.rmap.json` results in a Warn toast for
   zero-scale.
2. Auto-fix click restores layer scale to `[1.0, 1.0]`.
3. Loading a project referencing a missing SVG enters Failed.
4. Loading a healthy project: no toasts.

**Verification**
Manual + unit tests with fixtures.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T1.44: Critical findings → `AppState::Failed`

**Purpose**
Critical audit findings (missing asset, schema-too-new) must
route to `AppState::Failed` rather than entering Editing with
broken state.

**Implementation details**
- `FailureKind::ProjectAuditCritical { findings }` (already stubbed
  in T1.1).
- Editing state transition guards on
  `findings.iter().all(|f| f.severity != Severity::Critical)`.
- `AppState::Failed` shows a screen with the findings + a "Try
  another project" / "Quit" pair of buttons.
- A simple `Failed` screen suffices for v3 (no fancy recovery
  UI); the launcher (T2.*) provides the "Try another project"
  flow.

**Dependencies**
T1.43.

**Acceptance criteria**
1. Loading a project with Critical findings enters Failed, not
   Editing.
2. The Failed screen shows each finding's `message`.
3. Quit button exits cleanly.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

## WP-17 — Telemetry hooks

### Task T1.45: Tracing spans — session_start, first_layer_added, first_warp_drag

**Purpose**
First three of the Section-5-metric spans.

**Implementation details**
- `session_start`: emitted at `App::resumed` after `init_running_app`
  succeeds.
- `first_layer_added`: emitted at the first `Command::AddLayer`
  application within a session.
- `first_warp_drag`: emitted at the first `Command::SetWarpCorner`
  application within a session.
- Each span uses `tracing::info!` at level INFO with structured
  fields (timestamps as `Instant::now()` deltas).

**Dependencies**
T1.16.

**Can run in parallel**
With T1.46.

**Acceptance criteria**
1. All three spans fire exactly once per session.
2. `RUST_LOG=rmap=info` shows them.
3. They do not contain user payload (filenames, paths).

**Verification**
Manual log inspection.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.46: Tracing spans — project_audit_warned, advanced_opened, undo_invoked, demo_clicked

**Implementation details**
- `project_audit_warned`: emitted by `T1.43` for each non-Critical
  finding.
- `advanced_opened`: counted in Phase 3 once Advanced exists; stub
  here.
- `undo_invoked`: emitted on every `Cmd-Z`.
- `demo_clicked`: counted in Phase 2 once launcher exists; stub
  here.

**Dependencies**
T1.16, T1.43.

**Can run in parallel**
With T1.45.

**Acceptance criteria**
1. Spans exist (even if some fire only after later phases).
2. No payload leakage.

**Verification**
Manual log inspection.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T1.47: `ux_metrics` daily JSON sink

**Purpose**
Plan §11.12. A daily-rolling JSON sink in
`~/Library/Logs/rmap/ux_metrics_<date>.json`.

**Implementation details**
- New module `app/telemetry.rs`.
- Subscribes to a tracing `Layer` filtered to UX events (a custom
  target, e.g., `target = "rmap::ux"`).
- Writes JSON line per event.
- Daily rotation matches the existing log rotation.

**Dependencies**
T1.45.

**Can run in parallel**
After T1.45.

**Acceptance criteria**
1. Daily file appears.
2. Each event is a valid JSON object.
3. No project content (no filenames, paths, layer ids beyond
   numeric index).
4. Privacy review (T0.6) checklist passes.

**Verification**
Manual: run rmap for a session, inspect the JSON file.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

## Phase 1 closeout — M1 readiness *(updated post-revision)*

**M1 P0 set** (must ship): WP-1, WP-1.1, WP-2 (with rewritten
T1.14), WP-15 *with only T1.35 (zero-scale) + T1.38 (missing-
asset, extended)*, WP-17 hooks. T1.36, T1.37, T1.39, T1.40 are
P1-within-Phase-1 (ship if slack; otherwise v3.1).

Before declaring M1, verify:

- [ ] All T1.* acceptance criteria green.
- [ ] `cargo test --features v3` passes including proptest with
      `PROPTEST_CASES=1024`.
- [ ] `cargo run --features v3 -- ~/p1.rmap.json` shows the
      zero-scale toast and auto-fix works.
- [ ] `cargo run --features v3 -- /nonexistent.rmap.json` enters
      Failed.
- [ ] Cmd-Z reverses every covered mutation (smoke test on each).
- [ ] `RUST_LOG=rmap=info cargo run --features v3` produces
      session_start span.
- [ ] `~/Library/Logs/rmap/ux_metrics_*.json` file appears.
- [ ] `cargo run` *without* `--features v3` runs the v2 UI
      unchanged.
- [ ] No telemetry payload leaks (privacy checklist green).
- [ ] WP-2 mutation surface coverage: `grep -n "&mut project\."
      windows/control_panel.rs` and `windows/scene_editor.rs`
      return zero matches in production code.

Once all items check, M1 is declared. Open Phase 2 task file
(`003-tasks-phase-2.md`).
