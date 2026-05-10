# 003 — Phase 2 Tasks: First-Run Experience

> Index: `003-tasks.md`. Plan: `003-ui-ux-overhaul-plan.md`.
> **22 tasks. ~3 engineering weeks. Ships alpha (M2).**

## Purpose

Replace the CLI front door with a launcher window, bundle the
"Window glow" demo so a first-time user reaches "photo on a wall"
in ≤ 120 s, and replace the dev-log empty state with a friendly
canvas drop hint. Drag-and-drop replaces typed file paths.

## Scope covered

- WP-3 (Launcher window)
- WP-4 (Bundled demo project)
- WP-5 (Empty-state hints on canvas)
- WP-9 (Drag-drop + file picker)
- WP-13 (Live monitor names + projector test)

## Relationship to overall rollout

Phase 2 is the first user-visible layer of v3. It depends on
Phase 1's architecture being complete. It produces the alpha
release (M2). The default `main` build still runs the v2 UI; the
new experience lives behind `--features v3`.

## Entry criteria

- M1 reached.
- T0.2 (demo asset license-cleared) done.
- T0.3 (launcher wireframe) approved.

## Exit criteria

- T2.1–T2.22 acceptance criteria green.
- A team member who has never used rmap reaches "photo on a wall"
  in ≤ 120 s in 3 of 3 attempts.
- M2 declared; `v3-alpha` tag pushed.

---

## Task index

| ID | Title | Owner | Scope | Depends |
|----|-------|-------|-------|---------|
| T2.1 | New `windows/launcher.rs` shell | RUST | M | M1 |
| T2.2 | `LauncherState` struct + AppState integration | RUST | M | T1.1, T2.1 |
| T2.3 | Launcher → Editing transition wiring | RUST | M | T2.2 |
| T2.4 | Launcher window paints three start buttons | RUST + DES | M | T2.1 |
| T2.5 | Projector picker dropdown with monitor list | RUST | M | T2.4, T2.7 |
| T2.6 | Launcher "Test" button fires test pattern | RUST | M | T2.5 |
| T2.7 | macOS NSScreen display-name FFI shim | RUST | M | T0.7 |
| T2.8 | Bundle "Window glow" demo project file | RUST + PO | M | T0.2, spec 002 |
| T2.9 | "Try a demo" button loads demo project | RUST | S | T2.4, T2.8 |
| T2.10 | "Open recent" reads from `~/Documents/rmap/` | RUST | M | T2.4 |
| T2.11 | `drop_target` egui primitive | RUST | M | M1 |
| T2.12 | Drop image onto canvas → AddLayer command | RUST | S | T2.11, T1.31 |
| T2.13 | Native file picker via `rfd` crate | RUST | M | M1 |
| T2.14 | "+ Add image" button on canvas | RUST | S | T2.13 |
| T2.15 | Remove typed-path field from Layers tab | RUST | S | T2.13, T2.14 |
| T2.16 | Canvas empty-state pulsing drop hint | RUST + DES | M | T2.11 |
| T2.17 | Replace `(scene preview not yet registered…)` log line | RUST | S | T2.16 |
| T2.18 | `~/Library/Preferences/rmap.toml` schema + I/O | RUST | M | M1 |
| T2.19 | `~/Documents/rmap/` directory bootstrap | RUST | S | T2.18 |
| T2.20 | Remember last-used projector across sessions | RUST | M | T2.18 |
| T2.21 | Phase 2 test harness additions | RUST + QA | M | T2.9 |
| T2.22 | Skip-to-blank-canvas path from launcher | RUST | S | T2.4 |
| **T2.23** | **Asset-portability spike: project-relative paths + embed policy** *(NEW)* | RUST + PO | M | M1 |
| **T2.24** | **Missing-media relink flow with `rfd` "Find this file"** *(NEW)* | RUST | M | T2.13, T2.23 |

---

## WP-3 — Launcher window

### Task T2.1: New `windows/launcher.rs` shell

**Purpose**
Stand up the launcher window as a peer to `OutputWindow` and
`ControlWindow`. Empty content for now; it just needs to be
visible.

**Problem addressed**
Plan WP-3.

**Implementation details**
- New module `windows/launcher.rs`.
- `pub struct LauncherWindow { window, surface, config, egui_ctx,
  egui_state, egui_renderer, ... }` — mirror the `ControlWindow`
  scaffolding (same wgpu device, same egui setup).
- Default size 600 × 400, centred on the primary display.
- Title: "rmap" (the macOS app menu still says "rmap"; the launcher
  is the user's first window).
- Fields are private; expose `id()`, `on_window_event`, `resize`,
  `render`.
- Render closure body for now: a single `egui::CentralPanel` with
  the placeholder text "Launcher coming soon."

**Dependencies**
M1.

**Can run in parallel**
With T2.7, T2.11, T2.13, T2.18 (all independent surfaces).

**Acceptance criteria**
1. Module compiles under `--features v3`.
2. A new winit `Window` opens at the configured size when
   `LauncherWindow::new` is called.
3. egui renders inside it.
4. CloseRequested on the launcher window exits the app cleanly.

**Verification**
Manual smoke launching with `--features v3 --launcher-only`
(temporary CLI flag for testing).

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T2.2: `LauncherState` struct + AppState integration

**Purpose**
Populate the empty `LauncherState` stub from T1.1.

**Implementation details**
- `pub struct LauncherState { launcher: LauncherWindow, gpu:
  GpuContext, inputs: InputsBundle, prefs: UserPrefs, recent:
  Vec<RecentProject> }`.
- `gpu` and `inputs` come from T1.7 / T1.10 (the launcher
  reuses GPU and keyboard / MIDI / OSC sources before the editor
  needs them).
- `prefs` is loaded via T2.18.
- `recent` is loaded from `~/Documents/rmap/` per T2.10.

**Dependencies**
T1.1, T2.1.

**Can run in parallel**
With T2.7+.

**Acceptance criteria**
1. `LauncherState` compiles.
2. `AppState::Launcher(LauncherState)` is the new default for a
   first launch (no `--autostart`, no project arg).
3. `--autostart project.rmap.json` still skips the launcher and
   goes straight to Editing.

**Verification**
Manual smoke for both paths.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T2.3: Launcher → Editing transition wiring

**Purpose**
A click on any of the three launcher buttons triggers a
transition `AppState::Launcher → AppState::Editing`.

**Implementation details**
- New `Command::Launch { project: ProjectSource, monitor: usize,
  windowed: bool }` where `ProjectSource` is `Empty | RecentPath
  (PathBuf) | Demo("window-glow")`.
- The `apply_command` handler for `Launch` calls
  `init_output_window`, `init_control_window`,
  `init_render_graph` (composing the T1.7–T1.11 helpers) and
  produces an `EditingState`.
- The launcher window can either close (drop) or stay alive (kept
  hidden) until next session — close it on transition for
  simplicity.

**Dependencies**
T2.2.

**Can run in parallel**
With T2.4–T2.6.

**Acceptance criteria**
1. Clicking any launcher start button transitions to Editing.
2. The launcher window closes; the output and control windows
   open.
3. `Command::Launch` is recorded in telemetry but is `non_undoable:
   true` (you cannot Cmd-Z back to the launcher).
4. Transition errors (e.g., GPU surface failure) route to
   `AppState::Failed`.

**Verification**
Manual smoke + a unit test on the `Launch` command application
that asserts the `EditingState` is constructed.

**Risks / notes**
This is the largest behavioural change in Phase 2. Test
suspend/resume with the launcher open + transitioning.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T2.4: Launcher paints three start buttons

**Purpose**
Render the three primary launcher options.

**Implementation details**
- Three big rounded buttons stacked vertically:
  - **Start a new show** — emits `Command::Launch { project:
    Empty, ... }`.
  - **Open a recent show** *(disabled if recent.is_empty())* —
    opens a sub-list popover; clicking a recent emits
    `Command::Launch { project: RecentPath, ... }`.
  - **Try a demo** *(highlighted with "Recommended" badge)* —
    emits `Command::Launch { project: Demo("window-glow"), ... }`.
- Layout per `T0.3` wireframe.
- Each button is a `command_button` (Phase 1 primitive).

**Dependencies**
T2.1.

**Can run in parallel**
With T2.5–T2.6.

**Acceptance criteria**
1. Three buttons visible.
2. Hover state on each (subtle scale or brightness change).
3. "Recommended" badge on the demo button on first launch
   (suppressed if user has launched before — T2.18 prefs flag).
4. Disabled "Open recent" if no recent projects.

**Verification**
Manual + design QA against wireframe.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T2.5: Projector picker dropdown with monitor list

**Purpose**
Below the start buttons, a "Projector:" dropdown lists attached
displays by name.

**Implementation details**
- `egui::ComboBox` populated from `crate::monitors::list(event_loop)`
  augmented with display names from T2.7.
- Default selection: the most recently used projector (T2.20),
  else the non-primary display, else display 0.
- Live update on display hot-plug (best-effort; winit emits
  `MonitorAttached`/`MonitorRemoved` on macOS via NSScreen
  notifications — T2.7 surfaces these).

**Dependencies**
T2.4, T2.7.

**Can run in parallel**
With T2.6.

**Acceptance criteria**
1. Dropdown shows human names ("Built-in Display", "BenQ TH685
   — Living Room Wall", etc.).
2. Selection persists in `LauncherState` and is passed to
   `Command::Launch.monitor`.
3. If only one display is attached, dropdown is shown but
   single-choice (or replaced by a static label).
4. Hot-plug a display → dropdown updates within 1 frame.

**Verification**
Manual on a multi-monitor setup.

**Risks / notes**
Hot-plug detection is fragile on macOS; if it proves flaky, drop
to "live update on next launcher render" instead of event-driven.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T2.6: Launcher "Test" button fires test pattern

**Purpose**
Next to the projector dropdown, a small "Test" button fires a
5-second test pattern on the selected projector to confirm the
cable is working.

**Implementation details**
- Click → opens a temporary windowed `OutputWindow` on the chosen
  monitor at 1280 × 720.
- Renders the existing `TestPatternRenderer::Crosshatch` (or
  similar) for 5 seconds.
- Closes the output window; returns to launcher state.
- Failure (GPU surface error) routes to a toast in the launcher,
  not a transition.

**Dependencies**
T2.5.

**Can run in parallel**
With T2.4 once T2.5 lands.

**Acceptance criteria**
1. Click triggers the 5 s pattern.
2. Pattern visible on the chosen monitor.
3. After 5 s, the output window closes and the launcher remains
   visible.
4. A failure case (e.g., monitor unplugged mid-test) surfaces a
   toast.

**Verification**
Manual on real hardware.

**Risks / notes**
Surface creation in the middle of a launcher session is the same
code path as the eventual Editing transition, so hardening here
helps T2.3.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T2.7: macOS NSScreen display-name FFI shim

**Purpose**
winit returns `MonitorHandle::name()` as `"Monitor #41052"` on
macOS. We need the actual display name like `"BenQ TH685"`.

**Problem addressed**
Plan WP-13. High-risk task per Section 8 of `003-tasks.md`.

**Implementation details**
- New module `monitors/macos.rs` (gated `#[cfg(target_os =
  "macos")]`).
- Use `objc2-app-kit::NSScreen` to enumerate screens, calling
  `localizedName()` on each.
- Match to winit's monitor list by frame/position.
- Fall back to "Display N" for screens that fail to match or fail
  to return a name.
- Stable identifier: `NSScreenNumber` (a `CGDirectDisplayID`)
  used for last-used-projector persistence (T2.20).
- Linux/Windows: stub function returning `format!("Display {n}")`.

**Dependencies**
T0.7.

**Can run in parallel**
With T2.1–T2.6.

**Acceptance criteria**
1. On macOS, displays show their human names.
2. On display reconfiguration (unplug + replug), names update.
3. On Linux/Windows, the fallback is used (no panics).
4. Unit test with a mocked NSScreen list.

**Verification**
Manual on multiple-monitor mac.

**Risks / notes**
`objc2-app-kit` API surface is large; isolate into a small
function with clear inputs/outputs.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

## WP-4 — Bundled demo project

### Task T2.8: Bundle "Window glow" demo project file

**Purpose**
Author the `assets/demos/window-glow.rmap.json` project plus the
demo photo asset.

**Problem addressed**
Plan WP-4. Highest first-impression-impact single task.

**Implementation details**
- Acquire (T0.2) license-cleared photo: portrait orientation, soft
  lighting, ideally a single subject. CC0 from Unsplash is
  acceptable per Q5/D2.
- Place at `assets/demos/window-glow/photo.jpg`.
- Hand-author `assets/demos/window-glow.rmap.json`:
  - One `LayerKind::Image { path: "../assets/demos/window-glow/photo.jpg",
    fit: FitMode::Cover, ... }` (path resolution must be relative
    to the project file, not CWD).
  - One warp with a 2×2 corner-pin grid pre-positioned to a
    pleasant rectangle (~80% of the framebuffer).
  - One mask polygon using `zone_templates::window_rectangle()`
    centred in the warped output.
  - `transform.scale: [1.0, 1.0]` (NOT [0, 0] — explicit
    safeguard).
  - `output_windowed: true` (demo opens windowed for safety).
- Add `LICENSE-NOTES.md` next to the asset documenting the
  source.
- Add demo file path resolution: when `Command::Launch.project ==
  Demo("window-glow")`, resolve to `assets/demos/window-glow.rmap.json`
  relative to the binary (or to a packaged `.app` bundle path on
  macOS).

**Dependencies**
T0.2 (asset cleared), spec 002 image-layer support.

**Can run in parallel**
With T2.1–T2.7.

**Acceptance criteria**
1. Project file exists, well-formed, validates against schema 3.
2. Photo asset exists in repo with license note.
3. Loading the demo through `ProjectAudit::run` produces zero
   findings.
4. The image renders as a visible photo on the projector (not
   just outlines).
5. Property test: `cargo test --features v3 demo_loads_clean`
   asserts the demo loads, audits clean, and produces ≥ 1 visible
   pixel through the render pipeline (golden image test).

**Verification**
Property test + manual smoke.

**Risks / notes**
Path resolution differs between `cargo run` (CWD = repo root) and
a packaged `.app` (CWD = arbitrary). Test both.

**Suggested owner**
RUST + PO.

**Estimated scope**
M.

---

### Task T2.9: "Try a demo" button loads demo project

**Purpose**
Wire the launcher's demo button to the bundled project.

**Implementation details**
- Click handler emits `Command::Launch { project:
  Demo("window-glow"), ... }`.
- The launch command's load step calls `Project::load` on the
  resolved demo path.
- ProjectAudit runs as usual; the demo should produce zero
  findings, but if any do appear, they show as toasts in the new
  Editing state.

**Dependencies**
T2.4, T2.8.

**Can run in parallel**
After both deps.

**Acceptance criteria**
1. Click → ≤ 30 s to projected pixel on the chosen projector
   (cold start).
2. Demo opens windowed (per the project's `output_windowed:
   true`).
3. Telemetry `demo_clicked` span fires.

**Verification**
Manual stopwatch test on real hardware.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T2.10: "Open recent" reads from `~/Documents/rmap/`

**Purpose**
The recent-projects list scans `~/Documents/rmap/` for `*.rmap.json`
files and presents them with their filename + last-modified date.

**Implementation details**
- On launcher mount, scan the directory (created by T2.19 if
  missing).
- Filter to files ending `.rmap.json`, ignore the `_autosave/`
  subdirectory.
- Sort by mtime descending.
- Display the top 10 (per Q6 / D6 — confirm 10 is the chosen
  cap).
- Each entry: filename + relative date ("2 hours ago", "yesterday",
  "Mar 4").

**Dependencies**
T2.4 (button), T2.19 (directory), **T2.24 (missing-media relink
flow — for projects whose assets are gone)**.

**Can run in parallel**
With T2.5–T2.9 (without T2.24's missing-media handling, which is
a Phase 2 late addition).

**Acceptance criteria**
1. List shows up to 10 files sorted by mtime desc.
2. Empty state ("no recent projects yet") if directory is empty.
3. Click a file → `Command::Launch { project:
   RecentPath(path), ... }`.
4. Stale entries (file deleted between scan and click) surface a
   toast, not a panic.
5. **Project loads with missing assets** → routes through T2.24
   relink flow before entering Editing; user can find or remove
   the missing assets without leaving the launcher path.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

## WP-9 — Drag-drop + file picker

### Task T2.11: `drop_target` egui primitive

**Purpose**
A reusable visual treatment for the canvas drop zone: subtle
dashed border that pulses on `is_anything_being_dragged` from
egui.

**Implementation details**
- Function `drop_target(ui: &mut Ui, rect: Rect, label: &str)`.
- Paints the dashed border + centred label *only* when egui
  reports a drag in progress, OR when the canvas is empty (T2.16).
- Returns the egui Response for hit-testing.

**Dependencies**
M1.

**Can run in parallel**
With T2.1–T2.10.

**Acceptance criteria**
1. Primitive exists in a `windows/primitives.rs` module (new).
2. Drag a file into rmap from Finder → border pulses.
3. Drop the file → border returns to inactive state.

**Verification**
Manual smoke.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T2.12: Drop image onto canvas → AddLayer command

**Purpose**
Today's drop handler at `app.rs:1287` already supports SVG / JPG /
PNG. Wire it to the new `Command::AddLayer` (already migrated in
T1.31) and expose visual feedback via `drop_target`.

**Dependencies**
T2.11, T1.31.

**Acceptance criteria**
1. Drop a JPG/PNG/SVG → layer added; toast confirms.
2. Cmd-Z removes the dropped layer.
3. Unsupported file type → toast: "That file type isn't
   supported yet. Try a JPG, PNG, or SVG."
4. The drop zone covers the whole canvas (matching the new
   layout in Phase 3, but functional in Phase 2).

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T2.13: Native file picker via `rfd` crate

**Purpose**
Add the `rfd` (Rust File Dialog) crate; wire it for "+ Add image"
and "Save as…" flows.

**Implementation details**
- Add `rfd = "0.13"` (or current) to `Cargo.toml`.
- New helper `windows/file_dialogs.rs` with:
  - `pick_image_to_add() -> Option<PathBuf>` — filters to JPG/PNG/SVG.
  - `pick_save_destination(default_name: &str) -> Option<PathBuf>`
    — filters to `.rmap.json`, suggests filename, appends extension.
  - `pick_open_project() -> Option<PathBuf>` — for the launcher's
    "Open recent" alternative path.

**Dependencies**
M1.

**Can run in parallel**
With T2.1–T2.12.

**Acceptance criteria**
1. `rfd` added.
2. Three helper functions exist and work on macOS.
3. Cancel button in the dialog returns `None` cleanly (no panic).
4. Linux/Windows produce a working dialog (`rfd` handles this).

**Verification**
Manual on macOS.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T2.14: "+ Add image" button on canvas

**Purpose**
A small `+` tile at the bottom of the (forthcoming) layer strip;
in Phase 2 it lives next to the existing Layers tab.

**Implementation details**
- A button in the Layers tab that calls
  `pick_image_to_add()` then emits `Command::AddLayer`.
- Replaces the typed-path text field at `control_panel.rs:475`.
- For Phase 2: keep the old text field as well, behind a
  `--features v3-keep-typed-path` for one cycle of testing; remove
  in T2.15.

**Dependencies**
T2.13.

**Acceptance criteria**
1. Click → file picker.
2. Pick a file → layer added.
3. Cancel → no change.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T2.15: Remove typed-path field from Layers tab

**Purpose**
The typed-path field at `control_panel.rs:475` is the
audit's biggest workflow-friction item. Now that drag-drop
(T2.12) and the "+ Add image" button (T2.14) exist, remove it.

**Implementation details**
- Delete the `egui::TextEdit` and "Add layer" button block.
- Delete `ControlPanelState::new_layer_path_input` and
  `add_layer_error` fields.
- Clean up the Layers tab heading text accordingly.

**Dependencies**
T2.13, T2.14.

**Acceptance criteria**
1. Typed-path UI gone from the Layers tab.
2. Build clean (no unused imports).
3. The "Path does not exist" / "File must have extension .svg"
   error strings no longer surface to users.

**Verification**
Manual + grep for the removed strings.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## WP-5 — Empty-state hints on canvas

### Task T2.16: Canvas empty-state pulsing drop hint

**Purpose**
When the canvas has zero layers, paint a soft pulsing dashed
rectangle with copy *"Drop a photo or SVG here to begin."*

**Implementation details**
- Inside `show_scene_tab` in `windows/control_panel.rs`, when
  `project.layers.is_empty()`, render the empty-state instead of
  (or on top of) the existing black rect.
- Use `drop_target` (T2.11) for the visual.
- Empty state dismisses the moment the first layer is added.

**Dependencies**
T2.11.

**Acceptance criteria**
1. Empty project → drop hint visible.
2. Drop a file → hint disappears in the next frame.
3. Hint pulses smoothly (no jank).

**Verification**
Manual + design QA against wireframe.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T2.17: Replace `(scene preview not yet registered…)` log line

**Purpose**
The `scene_texture` is `None` when the output window is still
initialising. Today this surfaces as the developer-log line at
`control_panel.rs:243`. Replace with a friendly transition.

**Implementation details**
- New copy: *"Connecting to projector…"* (animated dots).
- After 5 seconds without success, escalate to a toast: *"Couldn't
  reach the projector. [Pick a different one]"* — the action
  button reopens a small projector picker (the launcher widget,
  reused).
- Track time-since-launch in `EditingState`; expire the
  "Connecting…" state and switch to error after 5 s.

**Dependencies**
T2.16.

**Acceptance criteria**
1. The dev-log string is removed from user-visible UI.
2. Connecting state visible for the brief init window.
3. Forced failure (e.g., `--monitor 99`) escalates to the error
   toast within 5 s.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## Preferences and recents

### Task T2.18: `~/Library/Preferences/rmap.toml` schema + I/O

**Purpose**
Per Q4 / D2.18, persist user preferences. Phase 2 needs at least:
- `last_used_projector_uuid: Option<String>`
- `first_launch_completed: bool`

**Implementation details**
- New module `app/prefs.rs`.
- `pub struct UserPrefs { ... }` with Serde derive (TOML).
- `UserPrefs::load() -> UserPrefs` (returns default on missing
  file or parse error; logs a warning).
- `UserPrefs::save(&self)` (atomic write via tempfile + rename).
- Loaded once per session; saved on relevant mutations.

**Dependencies**
M1.

**Can run in parallel**
With T2.1+.

**Acceptance criteria**
1. File appears at `~/Library/Preferences/rmap.toml` after first
   modification.
2. Corrupt file → fallback to default; rmap launches.
3. Schema is forward-compatible (unknown keys ignored).

**Verification**
Manual + unit test.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T2.19: `~/Documents/rmap/` directory bootstrap

**Purpose**
Create the directory on first launch so "Open recent" and Save As…
work without a user mkdir.

**Implementation details**
- On launcher mount, ensure both `~/Documents/rmap/` and
  `~/Documents/rmap/_autosave/` exist (`fs::create_dir_all`).
- Failure (permissions) → toast: "Couldn't create rmap's projects
  folder. Save will still work, but you may need to pick a
  location each time."

**Dependencies**
T2.18.

**Acceptance criteria**
1. Directories created on first launch.
2. Idempotent — second launch does not re-create or fail.
3. Permission failure handled gracefully.

**Verification**
Manual on a fresh user account; chmod test for permission failure.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T2.20: Remember last-used projector across sessions

**Purpose**
Per Q4: launcher remembers the last-used projector.

**Implementation details**
- On `Command::Launch`, record the chosen projector's stable
  identifier (NSScreenNumber on macOS, T2.7) into
  `prefs.last_used_projector_uuid`.
- On launcher mount, prefill the dropdown to that projector if
  still attached; otherwise fall back to non-primary.
- A "stable identifier" beats display name because names can
  collide.

**Dependencies**
T2.18, T2.7.

**Acceptance criteria**
1. After choosing a projector, quitting, and relaunching, the
   dropdown defaults to that projector.
2. If that projector is unplugged, dropdown falls back gracefully
   without a stale-id warning to the user.

**Verification**
Manual on multi-monitor machine.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T2.21: Phase 2 test harness additions

**Purpose**
Extend the headless command-driven test harness (T0.8) with the
new Phase 2 commands and the canonical first-run flow.

**Implementation details**
- Add a test "first_run_canonical" that:
  1. Loads the demo project.
  2. Asserts `ProjectAudit` produces zero findings.
  3. Asserts the render pipeline produces ≥ 1 non-black pixel
     (golden-image test, threshold-based).
- Add a test "launcher_recents_listing" with a mocked filesystem
  that the launcher picks up the right files and sorts them.

**Dependencies**
T2.9.

**Can run in parallel**
After T2.9.

**Acceptance criteria**
1. Both tests added and passing.
2. CI runs them under `--features v3 gpu-tests`.

**Verification**
CI green.

**Suggested owner**
RUST + QA.

**Estimated scope**
M.

---

### Task T2.22: Skip-to-blank-canvas path from launcher

**Purpose**
Per Q2 (skippable, default-recommended) — "Start a new show"
opens a blank canvas without forcing the user through any tour.

**Implementation details**
- The "Start a new show" button emits `Command::Launch { project:
  Empty, ... }`.
- The empty-state drop hint (T2.16) handles the rest.

**Dependencies**
T2.4, T2.16.

**Acceptance criteria**
1. Click → blank canvas with drop hint.
2. No additional dialogs or steps.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## Asset portability *(NEW — practitioner-driven)*

### Task T2.23: Asset-portability spike — project-relative paths + embed policy

**Purpose**
Today projects store absolute asset paths (verifiable in
`~/p1.rmap.json` fixture). The event-DJ "second laptop" failover
scenario fails on absolute paths. Spike + implement a portability
convention that survives cross-machine moves.

**Background**
Practitioner-flagged. F2 in revision triage.

**Implementation details**
- **Spike phase** (~0.5 day):
  - Audit how `LayerKind::Svg.path` and `LayerKind::Image.path`
    are read across the codebase. Currently consumed via
    `kind.asset_path()` returning a `PathBuf`.
  - Decide policy: paths stored in the project file are relative
    to the project file's directory whenever possible
    (asset is in the same folder, or in a `media/` subfolder, or
    in a `~/Documents/rmap/` shared folder). Otherwise absolute.
  - Resolution: `Project::resolve_asset(layer_idx, project_dir)
    -> PathBuf` does relative→absolute resolution at load time.
- **Implementation phase** (~1.5 day):
  - On save, attempt to write paths as relative to the project
    file's parent dir; fall back to absolute with a `tracing::info!`
    note.
  - On load, resolve relatives against the project file's
    parent dir.
  - One-time migration toast for existing absolute-path projects
    on first save: *"Save As… to convert paths to relative form
    and make this project portable."*
  - The migration is **opt-in** (Save As… triggers it); existing
    projects continue to work absolute-path-only without forcing.

**Dependencies**
M1.

**Parallelization**
Yes — runs in parallel with T2.1 (launcher shell). Different
module surface.

**Acceptance criteria**
1. A new project saved with an asset in the same folder writes a
   relative path.
2. A project moved to a different machine (asset path co-moved)
   loads cleanly.
3. Existing absolute-path projects load unchanged.
4. The opt-in migration toast appears on existing absolute-path
   projects exactly once per session.
5. Round-trip test: save a relative-path project, reload, verify
   asset resolves and renders.

**Verification**
- Unit tests on `Project::resolve_asset` with both relative and
  absolute inputs.
- Manual cross-machine test (or simulated via different working
  directories).

**Practitioner relevance**
This is the *single biggest cross-machine workflow gap* the
practitioner review surfaced. Without it, sharing a project file
between two laptops requires manually editing JSON. With it,
operators copy a folder and it works.

**Risks / notes**
- The macOS `.app` bundle's working directory differs from
  `cargo run`. Test both.
- iCloud-Drive-synced `~/Documents/rmap/` folders may surface
  symlinks; resolve through canonicalization at load time.
- Asset *embedding* (vs. referencing) is **out of scope** for
  T2.23; flagged for v3.1 if user demand surfaces.

**Suggested owner**
RUST + PO (policy decision review).

**Estimated scope**
M.

---

### Task T2.24: Missing-media relink flow

**Purpose**
When a project loads with missing assets (the T1.38 audit
finding), the toast offers a "Find this file…" option that opens
a native file picker and rebinds the layer.

**Background**
Practitioner-flagged. F2 + F16 in revision triage. Cross-link to
T1.38 (audit) and T2.10 (Open Recent listing).

**Implementation details**
- New `Command::RelinkAssetPath { layer_idx, new_path: PathBuf,
  old_path: PathBuf }` (added in T1.38; the implementation lives
  here).
- Toast action **Find this file…** opens `rfd::FileDialog` with:
  - Title: "Find {basename}"
  - Suggested directory: the project file's parent dir (because
    most missing-media cases are "asset moved with project").
  - File filter: matches the original asset's extension.
- Successful pick → emit `Command::RelinkAssetPath`; re-run
  `ProjectAudit` to surface any *other* missing assets.
- Bulk-relink heuristic: after a successful relink, scan
  remaining `MissingAsset` findings; if ≥ 2 share the same
  *old* parent directory and the new file is in the same *new*
  parent directory, offer **Relink {N} more from the same
  folder?** as a single bulk action.
- Cancelled file picker → toast remains; no command emitted.

**Dependencies**
T2.13 (rfd), T2.23 (path-resolution semantics; the relink
records absolute new path then T2.23 may convert to relative on
next Save As).

**Parallelization**
After T2.23.

**Acceptance criteria**
1. Open a project with one missing asset → toast with both
   actions visible.
2. Click **Find this file…** → file picker appears with the
   correct filter.
3. Pick a valid file → layer renders; toast disappears.
4. Cancel the picker → toast remains; no project mutation.
5. Bulk-relink offered when ≥ 2 missing assets share a parent
   directory; one click relinks all that match.
6. `Command::RelinkAssetPath` is undoable via Cmd-Z.

**Verification**
- Integration test: project with 3 missing assets sharing a
  parent dir → bulk relink → all rendered.
- Manual: move an asset on disk, reopen the project, relink.

**Practitioner relevance**
The event-DJ second-laptop failover relies on this. Beyond
that, every operator who reorganises their `~/Documents/`
benefits — and every collaborator who shares a project folder.

**Risks / notes**
- `rfd` blocks the egui frame on dialog open (modal). Acceptable
  for a relink action; document.
- The bulk-relink heuristic's "same parent dir" check should be
  conservative — only suggest, never apply silently.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

## Phase 2 closeout — M2 readiness (alpha)

Before declaring M2:

- [ ] All T2.* acceptance criteria green (including new T2.23 +
      T2.24).
- [ ] **Cross-machine portability smoke test:** save a project on
      one filesystem layout, copy folder to another, open
      cleanly.
- [ ] **Missing-media relink smoke test:** rename an asset on
      disk; relaunch; relink via the toast.
- [ ] **Internal usability test:** a team member who has never
      used rmap reaches "photo on a wall" in ≤ 120 s in **3 of 3
      attempts.**
- [ ] Demo loads cleanly through `ProjectAudit` (zero findings).
- [ ] Launcher remembers last-used projector after a quit/restart
      cycle.
- [ ] Drag-drop and "+ Add image" both work; the typed-path field
      is gone.
- [ ] Empty-state drop hint appears on a blank canvas; replaces
      the dev-log line.
- [ ] On macOS, displays show their human names in the launcher.
- [ ] Test pattern button works on the chosen projector.
- [ ] `cargo test --features v3 gpu-tests` green.
- [ ] No regressions on the v2 default build (`cargo run`).
- [ ] Tag `v0.3.0-alpha` pushed.

Once all items check, M2 declared. Open `003-tasks-phase-3.md`.
