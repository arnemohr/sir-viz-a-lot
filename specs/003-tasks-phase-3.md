# 003 — Phase 3 Tasks: Interaction Overhaul

> Index: `003-tasks.md`. Plan: `003-ui-ux-overhaul-plan.md`.
> **27 tasks. ~4 engineering weeks. Ships internal beta (M3).**

## Purpose

Replace the five-tab IA with one canvas + a single Advanced
disclosure. Make warp corners directly draggable on the live image.
Add the in-context glossary. Surface the four show-day controls
(Blackout / Freeze / Test / Outlines) as visible buttons.

This phase is the headline visible change of v3. After M3, `main`
runs the v3 UI by default; the v2 UI is removable code.

## Scope covered

- WP-6 (Canvas merge: Scene + Mapping + Layers → one canvas)
- WP-7 (Advanced disclosure)
- WP-8 (In-context glossary)
- WP-10 (Show-day strip)

## Relationship to overall rollout

Phase 3 is the *visible* overhaul. M3 graduates v3 from "alpha
behind a flag" to "internal beta on `main`." Power users (Sami,
Marco) test that Advanced houses every v2 capability they used.

## Entry criteria

- M2 reached.
- WP-2 mutation surface 100% migrated (all P0+P1 tasks in Phase 1).
- All Phase 1 telemetry hooks live.
- Glossary v0 (T0.1) authored and PO-reviewed.

## Exit criteria

- Default surface contains 0 advanced controls (every v2 advanced
  control lives in the Advanced disclosure or is direct-on-canvas).
- Internal users (Eva-style + Marco-style) can complete the
  canonical 7-step flow on the new IA without consulting docs.
- Sami can complete every v2 task entirely within Advanced
  (verified by walkthrough).
- M3 declared; default UI flips on `main` (v2 UI removable).

---

## Task index

| ID | Title | Owner | Scope | Depends |
|----|-------|-------|-------|---------|
| T3.1 | Promote scene preview to full canvas | RUST | M | M2 |
| T3.2 | Layer thumbnail strip on left edge | RUST + DES | M | T3.1 |
| T3.3 | Selection-driven right-edge inspector | RUST + DES | M | T3.1 |
| T3.4 | Toolbar with Warp/Advanced/Go-live buttons | RUST + DES | M | T3.1 |
| T3.5 | Wire `Selection::WarpCorner` direct manipulation | RUST | M | T3.4 |
| T3.6 | Remove `ControlTab::Mapping` arm + checker placeholder | RUST | S | T3.5, T3.11 |
| T3.7 | `EditMode { Layer, Warp, Mask, Inspect }` enum | RUST | M | T3.1 |
| T3.8 | `mode_banner` egui primitive (instruction strip per mode) | RUST + DES | S | T3.7 |
| T3.9 | Mode-aware cursor handling | RUST | S | T3.7 |
| T3.10 | Snap-to-edge for warp corners near framebuffer bounds | RUST | M | T3.5 |
| T3.11 | Single Advanced disclosure panel | RUST + DES | M | T3.1 |
| T3.12 | Move Master gamma/brightness/contrast into Advanced | RUST | S | T3.11 |
| T3.13 | Move Modulator picker into Advanced | RUST | S | T3.11 |
| T3.14 | Move per-effect editor into Advanced | RUST | M | T3.11 |
| T3.15 | Move mesh rows/cols and mask feather into Advanced | RUST | S | T3.11 |
| T3.16 | Move blend mode picker into Advanced | RUST | S | T3.11 |
| T3.17 | Move external-pass JSON into Advanced | RUST | S | T3.11 |
| T3.18 | Advanced disclosure "snap-back" on close | RUST | S | T3.11 |
| T3.19 | `glossary_label` egui primitive with `?` popover | RUST + DES | M | M2 |
| T3.20 | Glossary content registry | RUST + PO | S | T0.1 |
| T3.21 | Apply glossary entries to every advanced label | RUST + PO | M | T3.19, T3.20 |
| T3.22 | Compile-time check: every advanced term has a glossary entry | RUST | S | T3.20 |
| T3.23 | Show-day strip with B/F/T/O buttons | RUST + DES | M | T1.32 |
| T3.24 | Show-day strip key badges | RUST + DES | S | T3.23 |
| T3.25 | Show-day strip visible in `Editing` and `GoLive` | RUST | S | T3.23 |
| T3.26 | Phase 3 test harness additions | RUST + QA | M | T3.21, T3.23 |
| T3.27 | Remove old `ControlPanelState::tab` + tab strip rendering | RUST | S | T3.6, T3.18 |
| **T3.28** | **Per-display gamma + brightness + contrast override** *(NEW — practitioner-driven)* | RUST | S | T3.11 |

---

## WP-6 — Canvas merge

### Task T3.1: Promote scene preview to full canvas

**Purpose**
Replace the tabbed control panel with a single canvas that *is*
the live preview. The canvas is no longer one section of one tab;
it is the whole control window's central area.

**Problem addressed**
Plan WP-6.

**Implementation details**
- `windows/control_panel.rs` is renamed `windows/canvas.rs` (or a
  new module sits alongside; `control_panel` shrinks to a thin
  shim during migration).
- The render function becomes `canvas::show(ui, project,
  state, scene_editor, inputs) -> Vec<Command>`.
- The egui top-tab strip (`Scene / Effects / Layers / Mapping /
  Scenes`) is *not yet deleted* (T3.27 deletes after migration);
  for this task, it is hidden when `--features v3` is on.
- The Scene preview's existing direct-manipulation logic (drag
  layer, drag mask vertex, etc.) survives unchanged.
- The rest of the previous tabs (Effects / Layers / Mapping /
  Scenes) still render their UI but **into the Advanced
  disclosure** (T3.11+). For Phase 3 entry, render them into a
  collapsed Advanced panel that opens via the toolbar button.

**Dependencies**
M2.

**Can run in parallel**
With T3.19, T3.23.

**Acceptance criteria**
1. With `--features v3`, the control window opens with a single
   canvas, no top tab strip visible.
2. Live preview fills the centre.
3. Drag-drop, layer drag, mask vertex drag continue to work.
4. The Advanced toolbar button (T3.4) opens a panel with the
   Effects/Layers/Mapping/Scenes content (rough placement; T3.11+
   refines).
5. Without `--features v3`, the v2 tabbed UI is unchanged.

**Verification**
Manual smoke comparing v2 and v3 builds.

**Risks / notes**
This PR is large. Split into subtasks if needed; recommend
landing the canvas + Advanced shell first, then iterating on the
Advanced contents in T3.12–T3.17.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T3.2: Layer thumbnail strip on left edge

**Purpose**
Replace the typed Layers tab with a Procreate-style left strip:
vertical list of layer thumbnails, each with a visibility toggle
+ opacity preview.

**Implementation details**
- Render at the canvas's left edge, ~80 px wide.
- Thumbnail per layer: a small (64 × 36) snapshot of the layer's
  most recent rasterised content. Use the existing per-layer
  intermediate texture if accessible; otherwise render a coloured
  placeholder bound to the layer's id hash.
- Click a thumbnail → selects the layer (`scene_editor.selected =
  Selection::Layer(idx)`).
- Drag a thumbnail vertically → reorder (emits
  `Command::SwapLayers`).
- A `+` tile at the bottom opens the file picker (T2.13).
- Visibility toggle per thumbnail emits
  `Command::SetLayerEnabled`.

**Dependencies**
T3.1.

**Can run in parallel**
With T3.3, T3.4.

**Acceptance criteria**
1. Strip visible on left.
2. Each layer has a thumbnail.
3. Click a thumbnail → selection follows.
4. Drag-reorder works; emits the right command.
5. Visibility toggle works.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.3: Selection-driven right-edge inspector

**Purpose**
When a layer / warp corner / mask vertex is selected, a small
right-edge inspector shows its properties (move/scale/rotate +
opacity) plus a "More…" link to Advanced.

**Implementation details**
- ~280 px wide, slides in from right when `scene_editor.selected
  != None`.
- Default content:
  - `Selection::Layer(idx)` → translate / scale / rotate / opacity
    sliders (already exists in v2's Scene tab, just reposition).
  - `Selection::WarpCorner` → numeric x/y readouts and a "Reset
    this corner" button.
  - `Selection::MaskVertex` → numeric x/y readouts.
- "More…" link opens Advanced.
- Esc / clicking empty canvas → inspector hides.

**Dependencies**
T3.1.

**Can run in parallel**
With T3.2, T3.4.

**Acceptance criteria**
1. Inspector appears on selection.
2. Properties update live as the user drags.
3. Inspector hides on Esc or deselect.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.4: Toolbar with Warp / Advanced / Go-live buttons

**Purpose**
Top-of-canvas toolbar with primary controls.

**Implementation details**
- Left side: project name (auto-saved indicator — Phase 4
  refines), Undo / Redo buttons.
- Right side: **Warp** (mode toggle), **Advanced** (disclosure
  toggle), **Go live** (Phase 4 transitions to fullscreen; for
  Phase 3 it's a stub button).
- Each button uses `command_button` for telemetry consistency
  (clicks emit non-undoable `Command::OpenAdvanced` etc.).

**Dependencies**
T3.1.

**Can run in parallel**
With T3.2, T3.3.

**Acceptance criteria**
1. Toolbar visible on top of canvas.
2. Undo / Redo buttons work and reflect undo-stack state
   (disabled when empty).
3. Warp button toggles `EditMode::Warp` (T3.7).
4. Advanced button toggles the Advanced panel.
5. Go live button is visible but doesn't yet transition (Phase 4
   T4.16).

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.5: Wire `Selection::WarpCorner` direct manipulation

**Purpose**
The `Selection::WarpCorner` arm is `#[allow(dead_code)]` in
`scene_editor.rs:42` today. The canvas merge needs it live.

**Problem addressed**
Plan WP-6 acceptance: warp corners draggable on the live image.

**Implementation details**
- Hit-test priority (per `scene_editor.rs:11`): warp corners
  first, then mask vertices, then source rect, then layer body.
- Hit testing only fires when `EditMode::Warp` is active (T3.7).
- Drag emits `Command::SetWarpCorner { warp_idx, r, c, new, old }`.
- Visual: the warp grid is painted on the canvas as a faint mesh
  with handle dots at every grid intersection (matching the
  existing Mapping-tab visualisation, but on the live image).

**Dependencies**
T3.4.

**Can run in parallel**
With T3.6, T3.7.

**Acceptance criteria**
1. Toggle Warp mode → grid visible on canvas.
2. Drag a corner → live image deforms; command emits on drag end.
3. Cmd-Z reverses corner.
4. Snap-to-edge (T3.10) is a no-op until that task lands.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T3.6: Remove `ControlTab::Mapping` arm + checker placeholder

**Purpose**
The Mapping tab's 480×270 checker-pattern canvas is the audit's
single most-derided UI element. Delete it.

**Implementation details**
- Delete `ControlTab::Mapping` from the enum.
- Delete `show_mapping_tab` function (`control_panel.rs:591`).
- Delete the checker-pattern rendering code.
- Mesh rows/cols and mask feather move to Advanced (T3.15).
- Zone-template buttons move to Advanced or to the warp corner
  inspector (T3.3 inspector when a `Selection::WarpCorner` is
  active — design call).

**Dependencies**
T3.5 (warp editing on canvas), T3.11 (Advanced has destinations).

**Can run in parallel**
After both deps.

**Acceptance criteria**
1. `ControlTab::Mapping` gone.
2. `show_mapping_tab` gone.
3. Checker-pattern code gone.
4. All previous Mapping-tab capabilities still reachable: warp
   corners on canvas, mesh rows/cols in Advanced, zone templates
   somewhere visible (Advanced).
5. `cargo build --features v3` succeeds without unused-import
   warnings.

**Verification**
Manual + `grep ControlTab::Mapping`.

**Risks / notes**
Critical timing: do not delete before T3.11 lands its
destinations. See `003-tasks.md` Section 2 sequencing-mistake R3.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.7: `EditMode { Layer, Warp, Mask, Inspect }` enum

**Purpose**
The canvas has interaction modes; encode them.

**Implementation details**
- New enum `EditMode` in `windows/scene_editor.rs`:
  - `Layer` (default; current v2 behaviour).
  - `Warp` (warp corner editing, grid visible).
  - `Mask` (mask vertex editing on the selected warp).
  - `Inspect` (selection only, no drag).
- `SceneEditorState.mode: EditMode`.
- Mode toggled via the Warp button on the toolbar (T3.4).
- `Mask` mode entered automatically when a mask vertex is
  selected; deselection returns to `Layer` mode (or current
  mode).
- `handle_scene_input` dispatches by mode.

**Dependencies**
T3.1.

**Can run in parallel**
With T3.5, T3.10.

**Acceptance criteria**
1. Enum exists; default is `Layer`.
2. Warp button toggles `Layer ↔ Warp`.
3. Selecting a mask vertex switches to `Mask` mode.
4. `Inspect` mode is reachable (e.g., via a future "lock" toggle —
   stub for now).

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T3.8: `mode_banner` egui primitive

**Purpose**
A thin instruction strip at the top of the canvas that updates
copy per `EditMode`.

**Implementation details**
- `mode_banner(ui, mode)` renders a single line of guidance:
  - `Layer` → *"Drag to move. Shift-drag to scale. Alt-drag to
    rotate."*
  - `Warp` → *"Drag the corners to fit the wall."*
  - `Mask` → *"Drag a vertex. Double-click an edge to insert.
    Shift-click to delete."*
  - `Inspect` → *"Click anything to inspect."*
- Visual: small, low-contrast, no border.

**Dependencies**
T3.7.

**Can run in parallel**
With T3.9.

**Acceptance criteria**
1. Banner visible at top of canvas.
2. Copy updates when mode changes.
3. Copy is concise (matches plan §H — "four sentences, eight
   verbs").

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
S.

---

### Task T3.9: Mode-aware cursor handling

**Purpose**
The cursor should reflect the current mode so the user understands
what their next click will do.

**Implementation details**
- `Layer` → default arrow.
- `Warp` → crosshair.
- `Mask` → cell.
- `Inspect` → arrow.
- egui exposes `ui.output().cursor_icon`; set per mode.

**Dependencies**
T3.7.

**Can run in parallel**
With T3.8.

**Acceptance criteria**
1. Cursor changes when mode changes.
2. Cursor reverts on mouse-leave from the canvas area.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.10: Snap-to-edge for warp corners near framebuffer bounds

**Purpose**
Plan §15.1 (D15) and §D6: ease-in snap on warp corners released
near the canvas edge.

**Implementation details**
- During `Command::SetWarpCorner` drag end, if the released
  position is within ~10 px (in canvas-screen space) of `[0.0,
  0.0]`, `[1.0, 0.0]`, `[0.0, 1.0]`, or `[1.0, 1.0]` (the four
  framebuffer corners), snap to that corner exactly.
- During the drag, paint a faint magnetic-zone indicator when the
  cursor is in range.

**Dependencies**
T3.5.

**Can run in parallel**
With T3.6+.

**Acceptance criteria**
1. Releasing a corner within 10 px of a framebuffer corner snaps
   to exact integer coords.
2. Snap is a single `Command::SetWarpCorner` with the snapped
   value, not the cursor's pixel-precise value.
3. Snap can be bypassed by holding Shift on release.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

## WP-7 — Advanced disclosure

### Task T3.11: Single Advanced disclosure panel

**Purpose**
One labelled door for all advanced controls.

**Implementation details**
- New module `windows/advanced.rs`.
- A right-edge drawer that slides in when `state.advanced_open ==
  true`.
- Width ~360 px.
- Contains sections (collapsible accordion-style, but not nested
  more than one level):
  - **Master** (gamma, brightness, contrast)
  - **Selected layer** (effect chain editor, blend mode,
    modulator pickers — only visible when a layer is selected)
  - **Selected warp** (mesh rows/cols, mask feather, source rect,
    zone templates — only visible when a warp is selected; T3.5
    introduces warp corner selection)
  - **Project** (autostart flag, output_windowed, project file
    info)
  - **Diagnostics** (audit findings re-runnable; telemetry summary
    if it grows usefully later)
- Telemetry: `advanced_opened` span (T1.46).

**Dependencies**
T3.1.

**Can run in parallel**
With T3.2–T3.10.

**Acceptance criteria**
1. Click Advanced toolbar button → panel slides in.
2. Click again or Esc → slides out.
3. Sections render in the listed order.
4. Default-collapsed Master section, default-open Selected layer
   section.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.12: Move Master gamma/brightness/contrast into Advanced

**Purpose**
The plan wants gamma off the default surface.

**Implementation details**
- The three sliders move from the always-visible
  `CollapsingHeader::new("Master (gamma)")` block in
  `control_panel.rs:206–213` into the Advanced "Master" section.
- They use the same `command_slider` helpers wired in T1.18.
- Each slider gets a `glossary_label` (`?` popover) for its term.

**Dependencies**
T3.11.

**Acceptance criteria**
1. Sliders no longer appear on the default canvas surface.
2. They appear in Advanced > Master.
3. Cmd-Z still reverses each.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.13: Move Modulator picker into Advanced

**Purpose**
The combobox at `control_panel.rs:907` moves into Advanced > Selected
layer > Effect chain.

**Implementation details**
- Render as part of each effect's parameter list.
- Now lives only when `state.advanced_open && a layer is selected
  && that layer has effects`.

**Dependencies**
T3.11.

**Acceptance criteria**
1. Modulator picker only visible inside Advanced.
2. Modulator type changes still emit commands; Cmd-Z still
   reverses.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.14: Move per-effect editor into Advanced

**Purpose**
The full effect-chain editor (`show_effect`, `show_effects_tab`)
moves to Advanced.

**Implementation details**
- Show effects only for the currently-selected layer.
- The "Apply preset" combobox (T1.29 already migrated) lives at
  the top of the effects section.
- The "Effect chain" heading is preserved.

**Dependencies**
T3.11.

**Acceptance criteria**
1. Effect chain editor only inside Advanced.
2. Effect parameter sliders + Modulator pickers + presets all
   still work.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
M.

---

### Task T3.15: Move mesh rows/cols and mask feather into Advanced

**Purpose**
Plan §H: "Mesh rows / cols → Grid detail (Advanced)"; mask feather
slider → Advanced > Selected warp.

**Implementation details**
- Mesh rows/cols at `control_panel.rs:609` move into
  `Advanced > Selected warp > Grid detail`.
- Mask feather at `control_panel.rs:776` moves into
  `Advanced > Selected warp`.

**Dependencies**
T3.11.

**Acceptance criteria**
1. Both controls only inside Advanced.
2. Resampling on row/col change still preserves the operator's
   customisation (existing `resample_grid` logic).

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.16: Move blend mode picker into Advanced

**Purpose**
Per-layer blend mode (`control_panel.rs:530`) moves to Advanced >
Selected layer.

**Dependencies**
T3.11.

**Acceptance criteria**
1. Blend picker only in Advanced.
2. Cmd-Z reverses (whole-enum Reverse already in T1.19).

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.17: Move external-pass JSON into Advanced

**Purpose**
The `Effect::External` block at `control_panel.rs:879` shows raw
JSON. Hide unless Advanced is open.

**Implementation details**
- Effects of variant `External` render their JSON only when
  Advanced is open AND the layer has at least one External effect.
- Otherwise show a small placeholder: *"This effect is configured
  in the project file."*

**Dependencies**
T3.11.

**Acceptance criteria**
1. JSON only visible inside Advanced.
2. Placeholder visible outside Advanced.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.18: Advanced disclosure "snap-back" on close

**Purpose**
When the user closes Advanced, transient state inside it (e.g.,
which sub-section was open) persists; selection state is honoured.

**Implementation details**
- Persist sub-section open/closed state in `ControlPanelState` for
  this session.
- Re-opening Advanced restores the same scroll position and
  open sub-sections.

**Dependencies**
T3.11.

**Acceptance criteria**
1. Open Advanced, scroll, expand "Selected layer", close.
2. Re-open: scroll position and "Selected layer" expansion
   preserved.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## WP-8 — In-context glossary

### Task T3.19: `glossary_label` egui primitive

**Purpose**
A label paired with a `?` icon. Hover (or click) → popover with
the term's glossary entry.

**Implementation details**
- Function `glossary_label(ui: &mut Ui, term: GlossaryTerm) ->
  Response`.
- `GlossaryTerm` is a typed enum (T3.20), not a string — so a
  typo is a compile error.
- Layout: term text + small `?` to the right.
- Hover for ≥ 250 ms → popover slides in.
- Popover content: term + 1–2 sentence body + optional "Learn
  more" link to a future docs URL (deferred placeholder for now).

**Dependencies**
M2.

**Can run in parallel**
With T3.1–T3.18, T3.23.

**Acceptance criteria**
1. Primitive renders label + `?` + popover.
2. Hover delay tuned so transient cursor passes don't trigger.
3. Popover dismisses on cursor exit.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.20: Glossary content registry

**Purpose**
Compile-time-checked storage for all glossary entries.

**Implementation details**
- New module `windows/glossary.rs`.
- `pub enum GlossaryTerm { Warp, MaskPolygon, Modulator, Gamma,
  Brightness, Contrast, BlendMode, Crossfade, Scene, SourceRect,
  ZoneTemplate, Blackout, Freeze, TestPattern, EditorOverlay,
  Effect, FitMode, ... }` — one variant per term.
- `pub fn entry(t: GlossaryTerm) -> GlossaryEntry`. `GlossaryEntry
  { headline, body }`.
- Body content from T0.1.
- Exhaustive match in `entry()` ensures every variant has content.

**Dependencies**
T0.1.

**Can run in parallel**
With T3.19.

**Acceptance criteria**
1. Enum and `entry` function exist.
2. Every variant has a non-empty body.
3. Compile-time exhaustive match (no `_ => …` arm).

**Verification**
`cargo build --features v3`.

**Suggested owner**
RUST + PO.

**Estimated scope**
S.

---

### Task T3.21: Apply glossary entries to every advanced label

**Purpose**
Every label inside Advanced gets a `glossary_label` rather than
a plain `ui.label`.

**Implementation details**
- Audit the Advanced panel: every parameter label, every section
  heading, every dropdown that names a domain term.
- Replace plain labels with `glossary_label(ui,
  GlossaryTerm::*)`.
- A non-domain label (e.g., "value", "amp", "phase") may stay
  plain.

**Dependencies**
T3.19, T3.20.

**Can run in parallel**
After both deps.

**Acceptance criteria**
1. Every advanced section has at least one `glossary_label`.
2. Every domain term's first appearance in a section uses the
   glossary primitive.
3. Hovering over any `?` icon shows a friendly popover.

**Verification**
Manual walkthrough of every Advanced section.

**Suggested owner**
RUST + PO.

**Estimated scope**
M.

---

### Task T3.22: Compile-time check — every advanced term has a glossary entry

**Purpose**
Plan R9: prevent content debt where new terms are added without
glossary entries.

**Implementation details**
- The `GlossaryTerm` enum (T3.20) is the only valid input to
  `glossary_label`. New terms require a new enum variant +
  matching `entry()` arm.
- A `lint_terms_have_entries` test iterates over every
  `GlossaryTerm` variant and asserts the body is non-empty.

**Dependencies**
T3.20.

**Acceptance criteria**
1. Adding a new term without a body fails the test.
2. Test runs in CI.

**Verification**
`cargo test --features v3 lint_terms_have_entries`.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## WP-10 — Show-day strip

### Task T3.23: Show-day strip with B/F/T/O buttons

**Purpose**
Four large always-visible buttons mirror the keyboard hotkeys.

**Implementation details**
- New egui strip at the bottom of the canvas, visible in both
  `Editing` and `GoLive`.
- Four buttons: **Blackout**, **Freeze**, **Test**, **Outlines**.
- Each emits the corresponding `Command` from T1.32.
- Visual state reflects current `OutputState`: active (blackout
  on) → button highlighted; inactive → muted.
- Test button cycles through patterns matching the `T` key.

**Dependencies**
T1.32 (commands exist), T3.4 (toolbar / canvas layout).

**Can run in parallel**
With T3.1–T3.22.

**Acceptance criteria**
1. Strip visible in `Editing` and `GoLive`.
2. Click each button → output state changes match keyboard.
3. Active state is visually distinct.

**Verification**
Manual.

**Suggested owner**
RUST + DES.

**Estimated scope**
M.

---

### Task T3.24: Show-day strip key badges

**Purpose**
Each button shows its keyboard accelerator in a small badge.

**Implementation details**
- Render a small "(B)", "(F)", "(T)", "(O)" badge on each
  button.
- Badge style: low-contrast, small font.

**Dependencies**
T3.23.

**Acceptance criteria**
1. Badges visible.
2. Layout doesn't shift on hover.

**Verification**
Manual + design QA.

**Suggested owner**
RUST + DES.

**Estimated scope**
S.

---

### Task T3.25: Show-day strip visible in `Editing` and `GoLive`

**Purpose**
Confirm the strip survives the Go-live transition (Phase 4 lands
the transition itself; T3.25 makes sure the strip is part of
both states).

**Implementation details**
- Both `AppState::Editing` and `AppState::GoLive` arms render the
  strip.
- A future "Hide UI" mode (out of v3 scope) could hide it; not in
  this task.

**Dependencies**
T3.23.

**Acceptance criteria**
1. Strip visible in both Editing and a stubbed GoLive.

**Verification**
Manual.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

### Task T3.26: Phase 3 test harness additions

**Purpose**
Extend the headless harness with the canonical 7-step flow.

**Implementation details**
- New end-to-end test "canonical_first_session":
  1. Boot launcher.
  2. Pick a (mocked) projector.
  3. Click "Try a demo".
  4. Drag a warp corner via `Command::SetWarpCorner`.
  5. Drop a (mocked) image.
  6. Save scene to slot 1.
  7. Click Go live (stubbed).
  8. Assert end state has all expected mutations + sane render
     output.
- Test the canvas-merge replacement: assert that no
  `ControlTab::*` arms (other than maybe a stub) remain.

**Dependencies**
T3.21, T3.23.

**Acceptance criteria**
1. Canonical test added and passing.
2. CI runs it.

**Verification**
CI green.

**Suggested owner**
RUST + QA.

**Estimated scope**
M.

---

### Task T3.27: Remove old `ControlPanelState::tab` + tab strip rendering

**Purpose**
Final cleanup: the v2 tab system is deletable.

**Implementation details**
- Delete `enum ControlTab` (`control_panel.rs:71`).
- Delete `ControlPanelState::tab` field.
- Delete the top tab strip rendering at
  `control_panel.rs:139–149`.
- Old `show_scene_tab`, `show_effects_tab`, `show_layers_tab`,
  `show_scenes_tab` either:
  - Renamed and adapted to the new canvas / Advanced model, or
  - Deleted entirely if their content has fully migrated.

**Dependencies**
T3.6, T3.18 (Advanced contents migrated).

**Can run in parallel**
After both deps.

**Acceptance criteria**
1. `cargo grep ControlTab` returns zero matches.
2. No unused imports.
3. v3 UI unchanged after the cleanup.

**Verification**
Build + smoke.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## Per-display tone override *(NEW — practitioner-driven)*

### Task T3.28: Per-display gamma + brightness + contrast override

**Purpose**
Even single-projector setups benefit from per-output tone
override because the laptop monitor and the projector live in
different colour spaces. The current `Master (gamma)` panel
applies globally; an operator who tunes gamma for the projector
makes the control-window preview look wrong, and vice versa.

**Background**
Practitioner-flagged. F4 in revision triage. Cheap real-world
fix; high practitioner value.

**Implementation details**
- New section in Advanced > Selected output: per-output
  `gamma_override`, `brightness_override`, `contrast_override`,
  each defaulting to `None` (inherit from master).
- Storage: per-output, on the existing `WarpMesh` struct
  (single-projector v3 has one warp per output region; multi-
  projector v0.4 will have an explicit `OutputTarget`).
- Render path: in the gamma pass, if any override is `Some`, use
  it instead of the master. Single-projector means one set wins;
  no conflict.
- The control window's *preview* uses the master values; the
  projector's *fullscreen* output uses the override values when
  present. This is the entire point — the operator sees their
  laptop-correct preview while the projector renders projector-
  correct.
- Glossary popovers (T3.21) for each override term.

**Dependencies**
T3.11 (Advanced disclosure exists).

**Parallelization**
After T3.11. Independent of T3.12–T3.17.

**Acceptance criteria**
1. Advanced > Selected output has three override sliders +
   "inherit" toggles.
2. Setting an override changes the projector but not the
   control-window preview.
3. Clearing the override (returning to "inherit") restores
   master-driven values.
4. Cmd-Z reverses each override change.
5. Project save/load round-trips override values.

**Verification**
- Manual: open the demo with a real projector, observe colour
  shift; tune the override; verify preview vs. projector
  divergence.
- Unit test on the gamma pass uniform: override absent → master
  value; present → override value.

**Practitioner relevance**
This is the highest-value real-world tweak in v3 for a working
operator. Without it, gamma tuning is a binary choice between
"laptop looks right" and "projector looks right." With it, both
look right.

**Risks / notes**
- Schema change: `WarpMesh` gains three optional fields.
  Migration: add as `serde::default` so existing projects load
  without explicit override.
- Multi-projector (v0.4) will reorganise this onto an
  `OutputTarget`; the v3 schema decision is deliberately
  forward-compatible.

**Suggested owner**
RUST.

**Estimated scope**
S.

---

## Phase 3 closeout — M3 readiness (internal beta)

Before declaring M3:

- [ ] All T3.* acceptance criteria green.
- [ ] **Default surface contains 0 advanced controls** (verified
      by manual walkthrough).
- [ ] Canonical 7-step flow completes on the new IA without docs
      (verified by an Eva-style team member).
- [ ] Sami completes every v2 task entirely within Advanced
      (verified by walkthrough).
- [ ] Old `ControlTab::Mapping` arm and checker placeholder are
      gone.
- [ ] Glossary popovers exist on every advanced label.
- [ ] Show-day strip visible and functional.
- [ ] `cargo run` *without* `--features v3` still runs the v2 UI
      (deferred removal to Phase 5).
- [ ] CI green including the canonical-flow harness.
- [ ] Default `--features v3` → flip on `main` for internal team
      use (per Q9 / D9 — confirm timing with PO).
- [ ] Tag `v0.3.0-beta` candidate prepared (final tag in M4).

Once all items check, M3 declared. Open
`003-tasks-phase-4-5.md`.
