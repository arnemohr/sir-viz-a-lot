# 003 — Phase 4 + Phase 5 Tasks: Polish, Native Integration, Release

> Index: `003-tasks.md`. Plan: `003-ui-ux-overhaul-plan.md`.
> **Phase 4: 22 tasks, ~3 weeks. Phase 5: 15 tasks, ~2 weeks.**
> **Phases combined here because both are smaller than 1–3 and
> serve as the endgame.**

## Purpose

**Phase 4 (Polish)**: ship the refinements that turn the "internal
beta" into "external beta": visual scene picker, autosave, native
macOS menu, hot-swap windowed↔fullscreen Go-live transition,
theme polish, scene transitions on the cue strip.

**Phase 5 (Validation + Release)**: dogfooding, show-day rehearsal
with panic injection, external usability test, README rewrite,
CHANGELOG, v0.3.0 GA.

## Scope covered

- WP-11 (Visual scene picker)
- WP-12 (Autosave + Save As)
- WP-14 (Theme + iPad-like motion)
- WP-16 (Native macOS menu bar)
- Hot-swap windowed↔fullscreen Go-live
- Final cleanup + release prep

## Relationship to overall rollout

Phase 4 produces M4 (external beta tag, `v0.3.0-beta`). Phase 5
produces M5 (GA, `v0.3.0`). After M5, the v2 UI is removed and
`--features v3` becomes default-on (and eventually unconditional
in v0.3.1).

## Entry criteria

- M3 reached: canvas merge live, Advanced disclosure complete,
  glossary integrated, show-day strip visible, default v3 UI on
  `main`.

## Exit criteria

- M4 reached: all P2 tasks acceptance criteria green; `v0.3.0-beta`
  tag.
- M5 reached: external usability test ≥ 80% completion; show-day
  rehearsal green; README + CHANGELOG updated; `v0.3.0` tag.

---

## Task index — Phase 4

| ID | Title | Owner | Scope | Depends |
|----|-------|-------|-------|---------|
| T4.1 | Scene-thumbnail capture at save time | RUST | M | M3 |
| T4.2 | Bottom cue strip with scene thumbnails | RUST + DES | M | T4.1 |
| T4.3 | Click thumb → recall; drag onto `+` → save | RUST | M | T4.2 |
| T4.4 | Crossfade visual indicator on cue strip | RUST + DES | S | T4.2 |
| T4.5 | Empty cue strip ("No cues yet") state | RUST + DES | S | T4.2 |
| T4.6 | Autosave timer + dirty tracking | RUST | M | M3 |
| T4.7 | "Open recent" reads named projects from prefs | RUST | M | T4.8 |
| T4.8 | `Save as…` flow with `rfd` | RUST | M | T2.13 |
| T4.9 | Project name displayed in toolbar | RUST + DES | S | T4.8 |
| T4.10 | "Unsaved changes" indicator | RUST + DES | S | T4.6 |
| T4.11 | Live monitor names also surfaced in Advanced | RUST | S | T2.7 |
| T4.12 | Per-projector identifier for project portability *(DEFERRED to v3.1)* | RUST | M | T2.7, T2.20 |
| T4.13 | Output_monitor_index migration to stable identifier *(DEFERRED to v3.1)* | RUST | M | T4.12 |
| T4.14 | Theme tokens + colour palette finalisation | RUST + DES | M | M3 |
| T4.15 | Animation tokens (handle hover, drag, transitions) | RUST + DES | M | T4.14 |
| T4.16 | Hot-swap windowed↔fullscreen at Go-live **(REWRITTEN: preview persists)** | RUST | M | M3 |
| **T4.16a** | **"Preview as projector" pre-show mode** *(NEW — practitioner-driven)* | RUST | M | M3 |
| T4.17 | Go-live emits `Command::EnterGoLive`; revert via Stop **(REWRITTEN)** | RUST | M | T4.16 |
| T4.18 | macOS keyboard accelerator audit | RUST + QA | S | T4.19 |
| T4.19 | Native macOS menu bar (File / Edit / Window / Help) *(deferrable to v3.1)* | RUST | M | M3 |
| T4.20 | Theme accent + handle colour unification | RUST + DES | S | T4.14 |
| T4.21 | Design QA pass over every screen state | DES | M | T4.20 |
| T4.22 | Performance pass: 60 fps in `Editing`, ≤ 1% CPU in `Launcher` | RUST + QA | M | T4.16 |
| **T4.23** | **Capability roadmap doc — v3 scope + v0.4 forward plan** *(NEW)* | PO + RUST | M | M3 |

## Task index — Phase 5

| ID | Title | Owner | Scope | Depends |
|----|-------|-------|-------|---------|
| T5.1 | Internal dogfooding (≥ 1 week, real project) | MIX | L (justified) | M4 |
| T5.2 | Telemetry summary report from dogfooding logs | RUST + PO | S | T5.1 |
| T5.3 | Triage and fix top dogfooding bugs | RUST | M | T5.1 |
| T5.4 | Show-day rehearsal with panic injection | RUST + QA | M | T5.1 |
| T5.5 | Stress test: large project (≥ 20 layers) | RUST + QA | S | T5.1 |
| T5.6 | External usability test (n ≥ 5) **(RESCOPED: post-GA validation cycle, not GA gate)** | PO | L (justified) | post-GA |
| **T5.16** | **Practitioner field beta (1 event-DJ + 1 AV teacher)** *(NEW — replaces T5.6 as M5 gate)* | PO + QA | L (justified) | T5.4 |
| T5.7 | Privacy review final sign-off | PO | S | T5.6 |
| T5.8 | Asset license register sign-off | PO | S | T5.6 |
| T5.9 | Section 5 metrics measurement | RUST + PO | S | T5.6 |
| T5.10 | Bug-fix cycle from external test results | RUST | M | T5.6 |
| T5.11 | README rewrite | PO + RUST | M | T5.6 |
| T5.12 | Update built-in help / "First time using rmap?" link | RUST + PO | S | T5.11 |
| T5.13 | CHANGELOG + v2→v3 migration notes | PO | M | T5.11 |
| T5.14 | Release pipeline: build `.app`, sign, notarise | RUST | M | T5.13 |
| T5.15 | GA tag, release notes, public announcement | PO | S | T5.14 |

---

## Phase 4

### Task T4.1: Scene-thumbnail capture at save time

**Purpose**
Plan WP-11. Capture a small thumbnail of the current canvas when
a scene is saved.

**Problem addressed**
Numbered scene slots have no visual recall affordance.

**Implementation details**
- On `Command::SaveScene`, snapshot the current **post-warp,
  pre-gamma projector RT** (under v3 this was `warp_rt`; under
  v4, after Phase 3's T3.0b render-graph rewrite, it's the
  composite-of-warped-layers projector RT that the egui preview
  also samples — see plan §11.6a) into a 192 × 108 RGBA8 byte
  buffer.
- Store inline in the scene's `Scene` struct. **Schema-version
  note:** Phase 3's T3.0a already bumped `schema_version` to 4 to
  introduce per-layer warps. T4.1 adds an optional
  `thumbnail: Option<ThumbnailRgba>` field within the v4 schema —
  no further version bump; the field is `#[serde(default)]` so
  v4 projects without thumbnails (saved before this task) still
  load.
- Migration path: pre-T3.0a v3 projects migrate to v4 via T3.0a
  *and* gain `thumbnail: None` defaults via the serde default —
  one combined migration step.

**Dependencies**
M3.

**Can run in parallel**
With T4.6, T4.14, T4.16, T4.19.

**Acceptance criteria**
1. Saving a scene captures a thumbnail from the projector RT
   (post-warp, pre-gamma).
2. v3 projects load via T3.0a + serde default and gain
   `thumbnail: None`; T4.1 does not introduce its own version
   bump.
3. Thumbnails round-trip through save/load.

**Verification**
Manual + unit test on schema migration.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T4.2: Bottom cue strip with scene thumbnails

**Purpose**
Replace the numbered Scenes tab with a horizontal film strip.

**Implementation details**
- Below the canvas, height ~120 px.
- Each scene renders as a thumbnail with its index ("1", "2", …)
  in the corner.
- A `+` tile at the right end for new scenes.
- Empty cue strip: "(no cues yet)".

**Dependencies**
T4.1.

**Acceptance criteria**
1. Strip visible.
2. Thumbnails for saved scenes displayed.
3. `+` tile present.
4. Hotkeys 1–9 still work.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T4.3: Click thumb → recall; drag onto `+` → save

**Purpose**
The cue strip's interaction model.

**Implementation details**
- Click a thumbnail → emits `Command::ApplyProjectSnapshot` (the
  recall command from T1.30) with crossfade per
  `crossfade_duration_s`.
- Drag the canvas's current view onto the `+` tile (or a
  long-press on `+`) → emits `Command::SaveScene`.

**Dependencies**
T4.2.

**Acceptance criteria**
1. Click thumb recalls and crossfades correctly.
2. Drag-to-save emits a SaveScene.
3. Recall + Cmd-Z restores prior project.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T4.4: Crossfade visual indicator on cue strip

**Purpose**
While a crossfade is in progress, show progress on the target
thumbnail.

**Implementation details**
- A horizontal progress bar across the bottom of the recalling
  thumbnail.
- Hidden when no crossfade is in flight.

**Dependencies**
T4.2.

**Acceptance criteria**
1. Visible during fade.
2. Hidden after.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
S.

---

### Task T4.5: Empty cue strip state

**Purpose**
"(no cues yet)" copy + visual.

**Implementation details**
- When `project.scenes.is_empty()`, the strip shows just the `+`
  tile and a subtle hint "Save your first cue here".

**Dependencies**
T4.2.

**Acceptance criteria**
1. Empty state visible when no scenes.
2. First save populates the strip.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
S.

---

### Task T4.6: Autosave timer + dirty tracking

**Purpose**
Plan WP-12. Continuous autosave to `~/Documents/rmap/_autosave/`.

**Implementation details**
- New module `app/autosave.rs`.
- `EditingState.dirty: bool` flips to true on every applied
  `Command` that mutates `Project` (excluding `non_undoable`
  output-state ones).
- A debouncer with a 5 s grace period (configurable in
  `UserPrefs`) writes the project to
  `~/Documents/rmap/_autosave/<session-uuid>.rmap.json`.
- On clean exit, the autosave is renamed to a recovery file
  named with the session UUID + timestamp.

**Dependencies**
M3.

**Can run in parallel**
With T4.1, T4.14, T4.16.

**Acceptance criteria**
1. After any mutation + 5 s, an autosave file exists.
2. After clean exit, the file is preserved.
3. Re-launching offers to "recover" the latest autosave (T4.7).

**Verification**
Manual + filesystem inspection.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T4.7: "Open recent" reads named projects from prefs

**Purpose**
The launcher's "Open recent" already reads from
`~/Documents/rmap/` (T2.10). Update to surface autosave recovery
when applicable.

**Implementation details**
- Listed projects: explicit named projects (Save As outputs) +
  the most-recent autosave (with a "Last session (recovery)"
  label).
- Cap (Q6 / D6): default 10 entries.

**Dependencies**
T4.8.

**Acceptance criteria**
1. Launcher shows up to 10 entries.
2. Recovery autosave shown distinctly.
3. Clicking recovery loads it; user can then `Save as…` to
   commit a name.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T4.8: `Save as…` flow with `rfd`

**Purpose**
Replace the typed-`.rmap.json` path field with a native dialog.

**Implementation details**
- Toolbar (or File menu, T4.19) Save as… → calls `rfd`
  `pick_save_destination` (T2.13).
- Append `.rmap.json` if missing; copy autosave to the chosen
  path.
- Update `EditingState.project_name` and `dirty = false`.

**Dependencies**
T2.13.

**Acceptance criteria**
1. Save as… opens native dialog.
2. Cancel returns cleanly; no file written.
3. After save, project name displays in toolbar (T4.9).

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T4.9: Project name displayed in toolbar

**Purpose**
Show the current project's name; "Untitled" until first save.

**Implementation details**
- Toolbar left side: `My first show` (or "Untitled" + "•" dirty
  indicator).

**Dependencies**
T4.8.

**Acceptance criteria**
1. Name visible.
2. Dirty marker visible when unsaved changes exist.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
S.

---

### Task T4.10: "Unsaved changes" indicator

**Purpose**
A small dot or "•" next to the project name when dirty.

**Implementation details**
- Toggle on `dirty` flag (T4.6).
- Clear after a successful Save as… or autosave commit.

**Dependencies**
T4.6.

**Acceptance criteria**
1. Indicator visible when dirty.
2. Hidden when clean.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
S.

---

### Task T4.11: Live monitor names also surfaced in Advanced

**Purpose**
The Advanced > Project section shows the human display name for
the current `output_monitor_index`.

**Dependencies**
T2.7.

**Acceptance criteria**
1. Advanced shows e.g. "Output: BenQ TH685" instead of "Output:
   monitor 1".

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T4.12: Per-projector identifier for project portability *(DEFERRED to v3.1)*

**Status post-revision:** Deferred from M4 gate to v3.1 backlog.
Practitioner review: event-scale operators rarely move projects
between machines on a per-monitor basis, and the existing T2.20
"remember last-used projector" prefs already covers the most
common case. Schema v5 churn does not justify v3 inclusion.

**Purpose**
A project saved on one machine should reasonably load on another
without locking to a specific monitor index.

**Implementation details**
- Schema v5: `output_monitor` becomes
  `OutputTarget { uuid: Option<String>, fallback_index: usize }`.
- On load, prefer the UUID match; fall back to index; fall back
  to "no monitor matched, use display 0 + emit audit warning".

**Dependencies**
T2.7, T2.20.

**Acceptance criteria**
1. Project saved on machine A loads on machine B without crashing.
2. If the UUID doesn't match, audit warns.

**Verification**
Manual cross-machine smoke (or simulated).

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T4.13: `output_monitor_index` migration to stable identifier *(DEFERRED to v3.1)*

**Status post-revision:** Deferred alongside T4.12. The schema
remains at v4 (or whatever T4.1 lands) for v3; v5 ships in v3.1
with both T4.12 and T4.13 together.

**Purpose**
Migrate v3 projects to the v5 `OutputTarget` field.

**Implementation details**
- In `project::migrate`, add a v3→v5 step (skipping v4 if v4 was
  schema-internal-only, otherwise v3→v4→v5).
- Old `output_monitor_index` becomes `OutputTarget::fallback_index`
  with `uuid: None`.

**Dependencies**
T4.12.

**Acceptance criteria**
1. v3 project loads in v5 binary.
2. `output_monitor_index` field still works for backward smoke.
3. Schema migration unit-tested.

**Verification**
`cargo test --features v3 schema_migrate_v3_to_v5`.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T4.14: Theme tokens + colour palette finalisation

**Purpose**
Plan WP-14. Centralise colour constants in
`windows/theme.rs`; one calmer dark theme; one warm accent.

**Implementation details**
- New `pub mod theme;` with named constants:
  `BG_BACKGROUND`, `BG_PANEL`, `TEXT_PRIMARY`, `TEXT_SECONDARY`,
  `ACCENT`, `WARNING`, `DESTRUCTIVE`, `HANDLE_DEFAULT`,
  `HANDLE_ACTIVE`, etc.
- Per D5 / D5-D, decide the accent (warm gold preserved, or new).
- All hardcoded `egui::Color32::from_rgb(...)` literals replaced
  with theme constants.
- Apply at app start via `egui::Visuals` override.
- WCAG AA contrast on text.

**Dependencies**
M3.

**Acceptance criteria**
1. No raw `Color32::from_rgb` literals remaining (grep clean).
2. Theme readable; design QA approves.

**Verification**
Manual + grep.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T4.15: Animation tokens

**Purpose**
Spring-eased drag, hover scale, transitions per the audit's
"iPad-like" goal.

**Implementation details**
- New `windows/anim.rs` with:
  - `HOVER_FADE_MS = 120`,
  - `DRAG_EASE_MS = 160`,
  - `TRANSITION_MS = 220`,
  - egui animation helpers (`AnimationManager`) wired in.
- Apply to: warp handle hover, layer thumbnail hover, mode
  banner cross-fade, Advanced drawer slide-in/out.

**Dependencies**
T4.14.

**Acceptance criteria**
1. Hover/drag states animate, not jump.
2. No frame drops at 60 fps (verify with profiler).

**Verification**
Manual + profiler.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T4.16: Hot-swap windowed↔fullscreen at Go-live, with persistent control-window preview *(REWRITTEN — practitioner-driven)*

**Purpose**
Plan WP-3 final piece. Eliminates "Restart rmap to apply" for the
windowed flag — *and* keeps the operator in sight of the
projector content throughout the show.

**Background**
Original task fullscreened the projector and let the control
window become a glorified parameter panel. The practitioner
review flagged this as a real-show blocker: the operator must be
able to see what the projector shows during the show, without
walking around the venue to peek at the wall.

**Problem addressed**
Plan §11.10. **Promoted from "highest-risk task in Phase 4" to
"highest-impact task in Phase 4."**

**Implementation details**
- `OutputWindow::set_fullscreen(bool, monitor: Option<MonitorHandle>)`.
- Re-creates the projector's wgpu surface bound to the existing
  winit window via `winit::window::Window::set_fullscreen(...)`.
- **Critical:** the control-window preview (`scene_texture_id`)
  is *unaffected* by the projector's surface re-creation. Under
  v4 (after Phase 3's T3.0b render-graph rewrite, see plan
  §11.6a) the preview reads from the projector RT view (post-warp,
  pre-gamma), which is an offscreen texture independent of the
  projector's swap chain. The shared v3 `warp_rt` is gone; the
  invariant ("preview source survives surface re-creation") is
  preserved by binding to the projector RT view instead. Verify
  this explicitly in code review.
- During `GoLive`, the control window:
  - keeps showing the live preview (same FPS as `Editing`),
  - keeps the show-day strip visible,
  - keeps the layer thumbnail strip visible,
  - hides Advanced disclosure by default (collapsed but still
    available via the toolbar button — operator may need to tweak
    a slider mid-show).
- Wrap the surface re-creation in `catch_unwind` (matches v1's
  panic-recovery pattern at `app.rs:1572–1591`).
- Failure path: log + toast, route to `AppState::Failed` with a
  "Couldn't switch to fullscreen" message and a "Try again"
  button.

**Dependencies**
M3.

**Acceptance criteria**
1. `Editing → GoLive` switches the **projector** to fullscreen at
   runtime, no restart needed.
2. The **control-window preview keeps rendering** through the
   transition; preview FPS during `GoLive` is within 20% of
   preview FPS during `Editing`.
3. The show-day strip + layer strip + cue strip remain visible
   and interactive in the control window during `GoLive`.
4. `GoLive → Editing` (Stop button) returns the projector to
   windowed; control window unchanged.
5. A simulated panic during surface re-creation routes to Failed
   without crashing the binary; the control-window preview
   continues until the panic is observed.
6. Display-sleep assertion remains held throughout.

**Verification**
- Manual: launch demo, click Go live, observe both windows. The
  control window's preview must remain live throughout. Click
  Stop. Repeat ≥ 5 times.
- Profile: capture preview-window FPS during `Editing` and
  `GoLive`; assert within 20%.
- Forced-panic unit test for the surface re-creation path.

**Practitioner relevance**
This is the show-day-blocking gap the practitioner review
surfaced. Without a persistent preview during the show, the
operator must walk to the projector to verify content state —
disqualifying the tool for any paid use.

**Risks / notes**
- Under v4 the projector RT view is sampled by both the egui
  control-window texture binding AND the projector's gamma pass.
  Any surface re-creation must not invalidate the egui binding.
  Test on Apple Silicon and Intel; macOS is the primary target.
- Reuse v1's panic recovery; do not invent a new path.
- Document in code: "the preview's `TextureId` is bound to the
  projector RT view (post-warp, pre-gamma; T3.0b), which survives
  `OutputWindow::set_fullscreen` because it is an offscreen
  texture independent of the projector's swap chain."

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T4.16a: "Preview as projector" pre-show mode *(NEW — practitioner-driven)*

**Purpose**
Allow the operator to dry-run a show *before* connecting a
projector — useful for offsite planning, content rehearsal, and
pre-event checks.

**Background**
Practitioner review noted "no way to test the show offsite before
the venue." The new toolbar button **Preview** opens a small
extra window on the laptop sized to the projector's aspect ratio
(no projector connection required).

**Implementation details**
- New toolbar button **Preview** sits next to **Go live**.
- Click → opens a child `OutputWindow` on the primary display,
  windowed, sized to a configured "target projector aspect" (the
  user's last-used projector aspect, or 16:9 if none).
- The child window renders the same render graph as a real
  projector would (`render_m5_pipeline` end-to-end).
- No display-sleep assertion held during preview mode.
- Closing the preview window returns the app to `Editing`
  cleanly (the child output's surface is dropped; the existing
  `OutputWindow` is unaffected because preview uses a separate
  short-lived window).
- *Not* a new `AppState` variant — preview is a transient sub-
  mode of `Editing`; modelled as `EditingState.preview_window:
  Option<PreviewWindow>`.

**Dependencies**
M3.

**Parallelization**
With T4.16 (different surfaces).

**Acceptance criteria**
1. Click Preview → child window appears on the laptop.
2. The child window renders the same content the projector would.
3. Resizing the child window preserves projector aspect ratio
   (letterbox if needed).
4. Closing the child window returns to plain `Editing`.
5. Sleep assertion is *not* held while preview is the only
   windowed output.

**Verification**
- Manual.
- Unit test on the `EditingState.preview_window` lifecycle.

**Practitioner relevance**
Cheap addition; meaningful for offsite content rehearsal. A
event DJ practising at home before the gig now has a real
preview tool. This is the kind of feature MadMapper / Resolume
have but rarely match in indie tools.

**Risks / notes**
- Resource cost: preview adds one extra surface + one extra render
  graph evaluation per frame. Acceptable on any modern Mac.
- iCloud-synced preview window position should not persist
  across sessions (preview is transient).

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T4.17: Go-live emits `Command::EnterGoLive`; revert via Stop *(REWRITTEN — preview-persistence acceptance added)*

**Purpose**
Wire the toolbar's Go-live button (stub from T3.4) to T4.16.

**Background**
Original task focused on the projector-side fullscreen swap.
Practitioner-driven amendment: explicitly verify that the
control-window preview persists across the transition.

**Implementation details**
- `Command::EnterGoLive` and `Command::ExitGoLive` (both
  `non_undoable: true`).
- Apply transitions `Editing ↔ GoLive`.
- `EnterGoLive` triggers T4.16 hot-swap to fullscreen on the
  projector and *preserves* the control-window egui texture
  binding to the projector RT view (the post-warp, pre-gamma
  composite under schema v4 — see T3.0b).
- `ExitGoLive` reverses on the projector side; the control
  window is unaffected throughout.
- A telemetry span `go_live_clicked` (already stubbed in T1.46)
  fires.

**Dependencies**
T4.16.

**Acceptance criteria**
1. Click Go live → fullscreen on chosen projector + show-day
   strip + sleep assertion.
2. **Control-window preview FPS within 20% of `Editing`**
   immediately after the transition (verified via T4.22 perf
   pass).
3. Click Stop → revert.
4. Section 5 Q10 acceptance — all three (fullscreen, strip,
   sleep) visible *and* the operator can see the projector
   content from the control window without leaving their seat.

**Verification**
Manual + Q10 acceptance check + frame-rate assertion.

**Practitioner relevance**
Q10 was originally "fullscreen + strip + sleep assertion." The
revised gate adds **operator visibility**: the show is not
acceptable unless the operator can see what the audience is
seeing.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T4.18: macOS keyboard accelerator audit

**Purpose**
Cmd-Z, Cmd-S, Cmd-O, Cmd-Shift-Z, Cmd-Q must not conflict with
existing single-letter hotkeys (B/F/T/O, 1–9).

**Implementation details**
- Audit: list every hotkey + accelerator path.
- Confirm Cmd-modifiers route to the new menu (T4.19); plain
  letters route to legacy keyboard handlers.
- Ensure `Cmd-O` doesn't conflict with the `O` editor-overlay
  toggle (it doesn't — different modifier — but verify).

**Dependencies**
T4.19.

**Acceptance criteria**
1. Audit document committed to `specs/` listing every binding.
2. No conflicts.
3. QA can hit each accelerator and observe the right behaviour.

**Verification**
Manual walkthrough.

**Suggested owner**
RUST + QA.

**Estimated scope**
S.

---

### Task T4.19: Native macOS menu bar *(deferrable to v3.1 if Phase 4 slips)*

**Status post-revision:** Schedule-resilience marker. If Phase 4
hits its calendar by M4 without the menu, ship without and add
in v3.1. The menu adds OS-native polish but is not a
practitioner-flagged blocker.

**Purpose**
Plan WP-16. File / Edit / Window / Help with standard items.

**Implementation details**
- Wire via `objc2-app-kit::NSMenu`.
- Menu structure (final layout per D3):
  - **File**: New, Open…, Open Recent, Save, Save As…,
    separator, Close, Quit
  - **Edit**: Undo, Redo, separator, Cut, Copy, Paste
  - **Window**: Minimize, Zoom, separator, control window /
    output window
  - **Help**: rmap Help (links to README), Glossary (opens a
    summary of all glossary entries)
- Each item emits a `Command` (already exists from Phase 1).
- About box: "rmap — version, license, contributors".

**Dependencies**
M3.

**Acceptance criteria**
1. Menu visible on macOS.
2. Cmd-Z / Cmd-S / Cmd-O / Cmd-Q all work.
3. About box shows version + license.
4. Help → rmap Help opens repo README in the default browser.
5. On Linux/Windows, the menu is a no-op (egui menu suffices).

**Verification**
Manual on macOS.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T4.20: Theme accent + handle colour unification

**Purpose**
The audit called out `(180, 160, 70)` mustard handles vs.
`(120, 165, 220)` blue mesh vs. `(220, 120, 100)` red errors as
visually noisy.

**Implementation details**
- Pick one warm accent (per D5).
- Apply to all interactive handles (warp, mask vertex, drag-source).
- Errors use `WARNING` / `DESTRUCTIVE` distinct from accent.

**Dependencies**
T4.14.

**Acceptance criteria**
1. Warp handles, mask vertices, drag-source markers share the
   accent.
2. Errors visually distinct.

**Verification**
Manual + design QA.

**Suggested owner**
RUST + DES.

**Estimated scope**
S.

---

### Task T4.21: Design QA pass over every screen state

**Purpose**
DES walks every screen state in the new UI against the wireframes.

**Implementation details**
- Screens covered: Launcher (3 start-button states + dropdown +
  test fired), Canvas (empty, layered, warp-mode, mask-mode),
  Advanced (collapsed + each section expanded), Cue strip (empty
  + populated + crossfading), Show-day strip (each toggle
  state), Toasts (info / warn / error + with action).
- Output: per-screen sign-off in a checklist.

**Dependencies**
T4.20.

**Acceptance criteria**
1. Every screen state signed off.
2. Issues flagged → tracked as Phase-4 fix tickets.

**Verification**
Sign-off doc.

**Suggested owner**
DES.

**Estimated scope**
M.

---

### Task T4.22: Performance pass

**Purpose**
60 fps in `Editing` and `GoLive`; ≤ 1% CPU in `Launcher`.

**Implementation details**
- Profile with `cargo flamegraph` on a laden project.
- Identify hotspots; fix or document.
- Confirm `Launcher`'s `ControlFlow::Wait` keeps idle CPU low.

**Dependencies**
T4.16.

**Acceptance criteria**
1. 60 fps during a 5-layer + animation crossfade.
2. ≤ 1% CPU when launcher is open and idle.
3. No new render-path allocation per frame (validated with
   `tracing::field` instrumentation).

**Verification**
Profiler + manual.

**Suggested owner**
RUST + QA.

**Estimated scope**
M.

---

### Task T4.23: Capability roadmap doc — v3 scope + v0.4 forward plan *(NEW — practitioner-driven)*

**Purpose**
Publish an explicit, practitioner-honest statement of what v3
ships, what v3.1 catches up, and what v0.4 will own.

**Background**
Practitioner review: the "iPad-like projection mapping tool"
framing oversells the product to people whose paid work has needs
v3 cannot meet. An honest forward-roadmap reduces user
disappointment, sets contributor expectations, and unlocks the
README rewrite (T5.11).

**Implementation details**
- New file `specs/v3-capability-scope.md`.
- Three sections:
  1. **v3 ships:** still images + SVG, single projector, manual
     warp + corner pin, mask polygons + zone templates, scenes
     + crossfades, autosave + Save As, launcher + bundled demo,
     show-day strip + Go-live + persistent preview, project
     audit + missing-media relink.
  2. **v3.1 catches:** the four deferred audit findings (T1.36,
     T1.37, T1.39, T1.40), schema v5 portable monitor (T4.12,
     T4.13), compile-time Reverse-storage refactor (from T1.14),
     native menu bar if dropped from M4 (T4.19), additional
     demos (film strip, test grid).
  3. **v0.4 will own:** video layer (mp4/H.264 minimum, with
     decoded thread + GPU upload pipeline), NDI input layer
     (one new layer kind subscribing to an NDI sender),
     two-projector edge-blend stub (soft-edge alpha mask
     between adjacent warps), OSC live parameter binding UI
     (revival of M7 stubs), per-projector colour calibration
     beyond per-display gamma override.
- Each item in the v0.4 list links back to the relevant
  decision-task in `003-tasks-revision.md` Section 7 (D11–D14).
- Document is the source of truth for the README rewrite (T5.11)
  and the GA announcement.

**Dependencies**
M3.

**Parallelization**
Yes — writing task; runs in parallel with T4.1–T4.22.

**Acceptance criteria**
1. `specs/v3-capability-scope.md` committed.
2. The three sections (v3 / v3.1 / v0.4) each list at least the
   items above.
3. Decision-tasks D11–D14 reference the same forward plan.
4. PO + practitioner-reviewer sign-off on the document before
   T5.11 README rewrite.

**Verification**
Doc review.

**Practitioner relevance**
The most asked-for thing in the practitioner review was
"reposition the product." The most defensible answer is **publish
the roadmap, not change the spec**. This task does that.

**Risks / notes**
- The roadmap is a *scope* statement, not a *date* commitment.
  Word it accordingly: "v0.4 will own video" not "v0.4 ships in
  Q3."
- A draft can be reviewed in Sprint 1; final version blocks
  T5.11.

**Suggested owner**
PO + RUST.

**Estimated scope**
M.

---

## Phase 5

### Task T5.1: Internal dogfooding (≥ 1 week, real project)

**Purpose**
Use rmap on a real (or simulated-real) event end-to-end before
external users see it.

**Implementation details**
- The team uses rmap to plan a real or fake show: import 3+
  photos, draw a window cutout, save 3+ scenes, run a 30-minute
  show in fullscreen with crossfades.
- Track every UX surprise, performance issue, missing feature
  in a dogfooding bug list.

**Dependencies**
M4.

**Acceptance criteria**
1. ≥ 1 week of cumulative use.
2. Bug list with severity + reproducibility.
3. Telemetry log captured for the period.

**Verification**
Bug list shared.

**Suggested owner**
MIX.

**Estimated scope**
**L** — justified because it's calendar time, not engineering time.

---

### Task T5.2: Telemetry summary report from dogfooding logs

**Purpose**
Aggregate the daily JSON metrics into Section 5 numbers.

**Implementation details**
- Parse all `~/Library/Logs/rmap/ux_metrics_*.json` files from the
  dogfooding period.
- Compute: time-to-first-pixel, time-to-first-photo-on-wall,
  undo-invoked counts, advanced-opened counts.
- Write a markdown summary into `specs/telemetry-dogfood-report.md`.

**Dependencies**
T5.1.

**Acceptance criteria**
1. Report committed.
2. Section 5 metrics measured against baseline.

**Verification**
Report review.

**Suggested owner**
RUST + PO.

**Estimated scope**
S.

---

### Task T5.3: Triage and fix top dogfooding bugs

**Purpose**
Address the highest-severity issues from T5.1.

**Implementation details**
- Triage: severity × frequency.
- Fix top 5 (or more if cheap) before external testing.

**Dependencies**
T5.1.

**Acceptance criteria**
1. Top 5 fixed.
2. Each fix has a regression test or explicit decision not to.

**Verification**
PR review + test runs.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T5.4: Show-day rehearsal with panic injection

**Purpose**
Plan §15.2 show-day reliability gate.

**Implementation details**
- Run a full simulated show: launcher → demo → drag/warp/save → Go
  live → run 30 min with crossfades + blackout/freeze toggles.
- During the show, inject a deliberate panic (e.g., via a
  feature-gated `--inject-panic` CLI flag) and verify recovery.
- Verify all four show-day buttons (B/F/T/O) work both as
  buttons and as hotkeys.

**Dependencies**
T5.1.

**Acceptance criteria**
1. The 30-minute show completes without operator intervention
   beyond intended cues.
2. Injected panic recovers; show continues.
3. Display-sleep assertion held throughout.
4. Logs reviewed: no unhandled errors.

**Verification**
Recorded demo + log review.

**Suggested owner**
RUST + QA.

**Estimated scope**
M.

---

### Task T5.5: Stress test — large project

**Purpose**
Validate performance on a project with ≥ 20 layers + multiple
warps + 9 scenes.

**Implementation details**
- Construct a stress-test fixture: 20 image layers, 4 warps, 9
  scenes with crossfades.
- Run on the test machine; measure FPS, memory, GPU usage.
- Acceptable: 30+ FPS sustained; memory growth < 50 MB over 10
  min.

**Dependencies**
T5.1.

**Acceptance criteria**
1. Stress fixture committed.
2. Performance numbers captured.
3. If targets missed, fix or document before T5.6.

**Verification**
Profiler.

**Suggested owner**
RUST + QA.

**Estimated scope**
S.

---

### Task T5.6: External usability test (n ≥ 5) *(RESCOPED — post-GA validation cycle, not GA gate)*

**Status post-revision:** Moved out of the M5 gate. Recruitment
starts the day *after* `v0.3.0` ships; results inform v3.1
priorities. The M5 gate is now T5.4 (show-day rehearsal) +
T5.16 (practitioner field beta).

**Background**
Original task held GA hostage to a 5-tester recruitment pipeline.
The practitioner review correctly flagged this as calendar
risk: a single recruitment delay would block GA without changing
release quality. The revised gate (T5.4 + T5.16) is more
practitioner-grounded and tighter on calendar.

**Purpose**
Validate the released product against a broader sample of users
than the field beta covered, and feed v3.1 prioritisation.

**Implementation details**
- Recruit 5 testers (per Q8 / D8). Mix of Eva-style (event
  volunteer) + Marco-style (visual operator).
- Each session: 30 min, follow the canonical 7-step flow; record
  successes, failures, hesitations.
- Goal: ≥ 80% complete unaided. *(Same target; not a release
  gate.)*
- Findings feed `specs/v3.1-priorities.md` (a new doc).

**Dependencies**
M5 reached (post-GA).

**Acceptance criteria**
1. 5 sessions recorded within 4 weeks of `v0.3.0` tag.
2. Completion rate measured.
3. Findings documented in `specs/v3.1-priorities.md`.
4. Top 3 findings reflected in the v3.1 backlog opening.

**Verification**
Session recordings + report.

**Suggested owner**
PO.

**Estimated scope**
**L** — calendar; not a release gate.

---

### Task T5.7: Privacy review final sign-off

**Purpose**
R10 mitigation. Confirm telemetry contains no payload.

**Implementation details**
- Parse the latest dogfood + external-test ux_metrics JSON files.
- Verify no filenames, paths, project names, asset names appear.
- Sign off in `specs/telemetry-privacy-review.md`.

**Dependencies**
T5.6.

**Acceptance criteria**
1. Sign-off doc committed.
2. CI grep guardrail added: PRs that introduce a
   `ux_metrics`-payload string fail.

**Verification**
CI rule + manual review.

**Suggested owner**
PO.

**Estimated scope**
S.

---

### Task T5.8: Asset license register sign-off

**Purpose**
Confirm `assets/demos/*` licenses are clear.

**Dependencies**
T5.6.

**Acceptance criteria**
1. `assets/LICENSES.md` listing each asset + license + source URL.
2. PO sign-off.

**Verification**
Doc review.

**Suggested owner**
PO.

**Estimated scope**
S.

---

### Task T5.9: Section 5 metrics measurement

**Purpose**
Final measurement of every Section 5 metric vs. target.

**Implementation details**
- Combine T5.2 (dogfood metrics) + T5.6 (external test metrics).
- Document each metric, baseline, target, observed.
- For misses, decide: defer (with explicit justification),
  hotfix, or accept.

**Dependencies**
T5.6.

**Acceptance criteria**
1. Metrics document committed.
2. Each metric has a status (met / missed-deferred /
   missed-fix-required).

**Verification**
Doc review.

**Suggested owner**
RUST + PO.

**Estimated scope**
S.

---

### Task T5.10: Bug-fix cycle from external test results

**Purpose**
Fix any blocker bugs before GA.

**Implementation details**
- Triage T5.6 findings.
- Fix all blockers + the highest-severity non-blockers.

**Dependencies**
T5.6.

**Acceptance criteria**
1. Zero blocker bugs open.
2. Each fix has a regression test (or documented justification).

**Verification**
Issue tracker + tests.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T5.11: README rewrite

**Purpose**
The README's first paragraph today describes flags. Rewrite it to
describe the launcher.

**Implementation details**
- New top-of-README:
  - One-paragraph product description.
  - Quick-start: download, double-click, click Try a demo.
  - Screenshots: launcher, canvas with photo, show-day strip.
  - CLI section moves *below*, marked "Power users".
- Preserve all existing content under the new structure.

**Dependencies**
T5.6.

**Acceptance criteria**
1. README rewrite committed.
2. Quick-start path matches the actual app behaviour.
3. CLI section preserved.

**Verification**
Doc review.

**Suggested owner**
PO + RUST.

**Estimated scope**
M.

---

### Task T5.12: Update built-in help / "First time using rmap?" link

**Purpose**
The Help menu (T4.19) links somewhere. Make sure it points to the
new README + (eventually) a glossary page.

**Implementation details**
- Help → rmap Help opens the README on GitHub.
- Help → Glossary opens a summary list of glossary entries (could
  be inline or link to README#glossary).

**Dependencies**
T5.11.

**Acceptance criteria**
1. Both Help items work.
2. Links point to canonical URLs (no localhost / dead links).

**Verification**
Manual.

**Suggested owner**
RUST + PO.

**Estimated scope**
S.

---

### Task T5.13: CHANGELOG + v2→v3 migration notes

**Purpose**
A user upgrading from v2 needs a one-page summary of what changed.

**Implementation details**
- New `CHANGELOG.md` entry for v0.3.0 listing every breaking IA
  change.
- `specs/v2-to-v3-migration.md` (or section in CHANGELOG) noting:
  - "Mapping tab" gone; warp corners now on canvas.
  - Layer typed-path field gone; drag-drop or "+ Add image".
  - Numbered scene slots replaced by visual cue strip.
  - Master gamma + modulators + blend modes → Advanced.
  - Project files compatible (schema migration is automatic).

**Dependencies**
T5.11.

**Acceptance criteria**
1. CHANGELOG entry committed.
2. Migration notes committed.

**Verification**
Doc review.

**Suggested owner**
PO.

**Estimated scope**
M.

---

### Task T5.14: Release pipeline

**Purpose**
Build, sign, and notarise a macOS `.app` for distribution.

**Implementation details**
- `Makefile` (or `xtask` Rust binary) target `make release-mac`:
  - `cargo build --release-show` (existing profile in
    `Cargo.toml`).
  - Bundle into `rmap.app` with `assets/` included.
  - Sign with the team's Apple Developer cert.
  - Notarise via `notarytool`.
  - Output: `target/dist/rmap-0.3.0.dmg`.
- Linux/Windows: best-effort tarball / zip; not blocking GA.

**Dependencies**
T5.13.

**Acceptance criteria**
1. `make release-mac` produces a notarised `.dmg`.
2. The `.dmg` opens, drags to Applications, double-click runs the
   launcher.
3. Asset paths resolve inside the bundle.

**Verification**
Manual install on a clean Mac.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T5.16: Practitioner field beta *(NEW — replaces T5.6 as M5 gate)*

**Purpose**
Validate the GA candidate with two practitioners doing real
single-event use *before* the public tag.

**Background**
Practitioner review: a 5-tester academic usability lab is the
wrong gate signal for a tool whose real test is "did the event
DJ get through the night?" Field-validation by two operators
running a real (or simulated-real) event is a tighter, more
honest signal.

**Implementation details**
- Recruit two practitioners:
  - One **event DJ-style** operator (small ceremony, photos +
    SVG, single projector, 1–2 hours of show).
  - One **AV teacher-style** operator (school assembly, gallery
    opening, classroom demo).
- Each runs the tool through one event, real or simulated-real
  (a friend's gathering, a school rehearsal, a gallery preview).
- Sessions are observed (in person or remotely); the tool is
  *not* changed mid-session.
- After both sessions, triage findings into:
  - **Blocker for GA:** crash, data loss, show-day-stopping
    bug. Must be fixed before tag.
  - **Post-GA fix:** feeds T5.10 bug-fix cycle.
  - **v3.1 backlog:** feeds the v3.1 priorities doc.

**Dependencies**
T5.4 (show-day rehearsal complete with no panic recovery).

**Parallelization**
Sequential after T5.4. T5.10 (bug-fix cycle) can begin in
parallel with the field-beta sessions for non-blocking issues.

**Acceptance criteria**
1. Two practitioner sessions completed (or scheduled before GA
   tag, with M5 gating on the *blocker* class only).
2. Findings triaged into the three classes.
3. Blockers fixed and verified before tag.
4. Notes from both sessions committed to
   `specs/field-beta-notes.md`.

**Verification**
Session notes review; bug-fix verification on each blocker.

**Practitioner relevance**
This is the GA gate the practitioner review explicitly asked for.
Without it, the team is shipping based on engineering confidence,
not field confidence.

**Risks / notes**
- Recruitment is the hardest part. Two volunteer operators is
  achievable in 1–2 weeks; budget calendar.
- If only one operator session can be arranged, document why and
  proceed with M5; don't hold the tag indefinitely.

**Suggested owner**
PO + QA.

**Estimated scope**
**L** — calendar (~1–2 weeks recruitment + sessions).

---

### Task T5.15: GA tag, release notes, public announcement

**Purpose**
Tag `v0.3.0`; publish release notes; close the overhaul.

**Implementation details**
- `git tag v0.3.0`; push.
- GitHub Release with the notarised `.dmg` attached.
- Release notes: copy from CHANGELOG + screenshots.
- Internal announcement (or external if relevant).

**Dependencies**
T5.14.

**Acceptance criteria**
1. Tag exists.
2. Release published.
3. `.dmg` attached and downloadable.

**Verification**
GitHub UI inspection.

**Suggested owner**
PO.

**Estimated scope**
S.

---

## M5 readiness — GA checklist *(REVISED post-practitioner-review)*

Before tagging `v0.3.0`:

- [ ] All P0 work-package acceptance criteria green; P1 (within-
      Phase-1 deferrable findings, T1.36/T1.37/T1.39/T1.40)
      green or v3.1-tracked.
- [ ] Section 5 metrics measured: each target met or deferred with
      explicit justification.
- [ ] Property test for undo/redo: green.
- [ ] Project audit: ≥ 2 P0 finding kinds covered with auto-fixes
      (zero-scale + missing-asset relink). Additional kinds
      shipped if Phase 1 had slack.
- [ ] Cross-machine portability smoke (T2.23): save → copy folder
      → load → render.
- [ ] Missing-media relink (T2.24): rename-on-disk → reopen → relink
      via toast.
- [ ] Headless command-driven harness: covers the canonical 7-step
      flow.
- [ ] **Show-day rehearsal: complete with panic injection** (T5.4).
- [ ] **Field beta: 2 practitioner sessions; blockers fixed**
      (T5.16). *(Replaces the original "n ≥ 5 external usability"
      gate.)*
- [ ] Capability roadmap doc published (T4.23) and signed off by
      PO + practitioner reviewer.
- [ ] README rewrite (T5.11) reflects T4.23 positioning.
- [ ] CHANGELOG + v2→v3 migration notes committed.
- [ ] Glossary v1: every advanced term has a polished entry.
- [ ] Telemetry privacy review signed off (R10 / T5.7).
- [ ] Asset license register signed off (T5.8).
- [ ] `--features v3` is now default-on (or `v3` is the only
      build); v2 UI removable in v0.3.1.
- [ ] Notarised `.dmg` distributed; install + launch verified on
      a clean Mac.
- [ ] `v0.3.0` tag pushed; release notes published.

**Post-GA cycle (NOT a tag blocker):**
- [ ] T5.6 (n ≥ 5 external usability) recruitment within 4 weeks
      of tag; results feed `specs/v3.1-priorities.md`.

GA declared. Overhaul complete.

---

## Post-GA backlog (out of scope for v0.3.0; tracked here)

**v3.1 backlog** (next minor; addresses deferred-during-revision
items):

- T1.36 (degenerate warp audit), T1.37 (mask <3 vertices audit),
  T1.39 (out-of-range monitor audit), T1.40 (schema-too-new
  audit) — if not shipped during Phase 1 slack.
- T4.12 (per-projector UUID portability) + T4.13 (schema v5
  migration) — deferred to v3.1.
- T4.19 (native macOS menu bar) — if dropped from M4.
- Compile-time Reverse-storage enforcement (refactor of T1.14
  from runtime to compile-time gate).
- T5.6 external usability test results → v3.1 priorities.
- Two more demos: "Slow film strip", "Test grid" (Q5
  fast-follow).

**v0.4 scope** (per T4.23 capability roadmap; major release):

- Video layer (mp4/H.264 minimum) — D11.
- NDI input layer — D12.
- Two-projector edge-blend stub — D13.
- OSC live parameter binding UI (revival of M7 stubs) — D14.
- Per-projector colour calibration beyond per-display gamma
  override.

**Roadmap-deferred** (no spec yet):

- Linux / Windows GA parity (Q9 deferred).
- Lighting outputs (Art-Net / sACN) — original roadmap Phase 4.
- AR-based calibration — original roadmap Phase 5+.
- iPad / iOS port — never planned in this overhaul.
- v0.5+: remove the `--features v3` feature flag entirely; remove
  the v2 UI code paths.

These are listed so the team has a clean post-GA backlog to draw
from; none block `v0.3.0`.
