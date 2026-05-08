# 003 — rmap UI/UX Overhaul Plan

> Implementation-ready execution plan derived from the UX audit at
> `specs/003-ui-ux-overhaull.md`. Owns the conversion of vision into
> sequenced epics, work packages, acceptance criteria, and Rust
> architecture decisions.
>
> **Audience:** product owner, design lead, Rust engineering team.
> **Status:** draft, pending Phase 0 alignment review.
> **Source-of-truth UX critique:** `specs/003-ui-ux-overhaull.md`.
> **Predecessor specs:** `001-initial-setup.md` (renderer foundation),
> `002-direct-scene-editor.md` (live-preview direct manipulation).

---

## 1. Title and objective

**Title:** rmap UI/UX Overhaul — from technically capable to
operator-obvious.

**Why this overhaul exists.** rmap's renderer, project format, and
direct-manipulation editor are sound. Its first-use experience is
not. A non-technical operator cannot launch the app without a
terminal, cannot find their way through five tabs, cannot
trivially get a photo onto a wall, and can silently load a project
that renders nothing visible (`transform.scale = [0, 0]`, no
warning). This overhaul restructures the surface area, the
defaults, the launch flow, the failure modes, and the editing
canvas so a first-time operator reaches *photo on a wall* in under
two minutes.

**Product outcomes this overhaul must achieve.**

| # | Outcome | Operationalised as |
|---|---------|--------------------|
| O1 | A first-time operator launches rmap by double-clicking an icon, never a CLI. | A bundled `.app` (macOS) launches into a launcher window. CLI flags persist for power users. |
| O2 | A first-time operator gets a photo onto the projector in under 2 minutes without reading documentation. | Bundled demo project + drag-drop on canvas + warp handles directly on the live preview. |
| O3 | The default surface contains only the controls a beginner needs. | Gamma, modulators, blend modes, mesh detail, source rect, external passes all live behind a single Advanced disclosure. |
| O4 | A loaded project can never silently render nothing. | First-frame project audit emits actionable warnings; zero-scale and degenerate-warp cases auto-repair or clearly flag. |
| O5 | Domain terminology (warp, mask polygon, modulator, gamma, blend mode, crossfade, scene) is preserved and *taught*, not renamed. | Each advanced label has a one-line in-context glossary popover. |
| O6 | Every edit is undoable. | Project mutations route through a `Command` abstraction with reversible application. Cmd-Z everywhere. |
| O7 | Show-day controls are visible, not memorised. | The four B/F/T/O actions surface as four large always-visible buttons; hotkeys still work. |

---

## 2. Executive summary

**For users.** The CLI gate is replaced by a launcher window with a
named projector picker, three start options (New / Open recent /
Try a demo), and a one-click test pattern to confirm the cable is
working. The five-tab control window is replaced by one canvas
that *is* the live preview, with layer thumbnails on the left, a
warp-mode toggle on the toolbar, scene thumbnails ("cues") on the
bottom, and four show-day buttons (Blackout / Freeze / Test
pattern / Outlines) always visible. Everything advanced — gamma,
modulators, blend modes, mesh detail, effect chain, source rect,
external passes, project autostart — moves behind one labelled
**Advanced** disclosure. Domain terminology stays exactly as it
is; every advanced label gets a `?` icon that opens a one-line
glossary popover. Drag-and-drop replaces typed file paths. Saved
projects autosave continuously; *Save As…* names them. Every edit
is undoable. A project that loads to nothing visible surfaces a
toast: *"3 layers have zero scale. [Auto-fix]"*.

**For the codebase.** The implicit `Option<RunningApp>` state
machine becomes an explicit `AppState` enum (`Booting`,
`Launcher`, `Editing`, `GoLive`, `Failed`) with typed transitions.
Every mutation of `Project` routes through a `Command` enum
(extension of the existing `ControlEvent`) that is loggable,
serialisable, and reversible — the foundation for undo/redo,
telemetry, and command-line replay. The egui control panel
collapses from five tab-rendering functions into one canvas
function plus an `Advanced` disclosure. Project loading gains a
`ProjectAudit` pass that runs after migration and before render
runtime construction; its warnings are surfaced through a new
`Toast` system. The launcher is a new top-level egui window that
shares the wgpu context but is not coupled to the renderer.
Telemetry is added as `tracing` spans on a small set of UX
events (`session_start`, `first_layer_added`, `first_warp_drag`,
`go_live_clicked`, `project_audit_warned`).

**For product structure.** rmap stops being shipped-as-a-CLI and
starts being shipped-as-an-app. Demo projects join the asset
bundle. The README's first paragraph stops describing flags and
starts describing the launcher.

---

## 3. Problem statement

The audit identified problems across six layers. Root causes are
called out separately from symptoms.

### 3.1 UX discoverability problems (symptoms)

- Five flat tabs of equal weight; no indication where to start.
- "Live preview" instructions for mouse gestures appear before
  any content has been added.
- "Master (gamma)" expanded by default in empty state.
- No empty-state guidance on the canvas; users see a black
  rectangle and a dev-log line.
- B/F/T/O keyboard shortcuts undocumented in the UI.
- Saved scenes are 1–9 numeric slots with no thumbnails.

### 3.2 Workflow problems (symptoms)

- Layer addition is a typed file-path field; drag-and-drop is
  hinted only in tiny grey text.
- Mapping happens in a tab disconnected from the live image — a
  480×270 checker placeholder labelled "output area
  (placeholder thumbnail)".
- Project save requires typing `*.rmap.json` as a filename.
- Switching projector requires quitting and relaunching with a
  different `--monitor` flag.
- Windowed-vs-fullscreen requires a project-file edit and an app
  restart.

### 3.3 Information architecture problems (root cause)

- The control window's IA mirrors the rendering pipeline (Scene /
  Effects / Layers / Mapping / Scenes), not the user's task model
  (Pick projector → Add content → Fit to wall → Save → Go live).
- Advanced controls share the default surface with beginner
  controls. There is no progressive disclosure.

### 3.4 Interaction-design problems (root cause)

- Direct manipulation lives only on the Scene tab's preview;
  warp corner editing happens on a separate placeholder canvas.
- Undo does not exist. Every edit is destructive in the moment.
- No snapping, locking, or guidance for warp corners.

### 3.5 Terminology and language problems (symptoms, not root cause)

- Help text is dense (5-sentence blocks of mouse-gesture
  instructions).
- Error messages leak implementation ("Filename should end with
  .rmap.json", "(none — assets/presets/*.json not found)").
- Empty-state strings are dev-log lines ("(scene preview not yet
  registered — output window not initialized)").
- **Note:** terminology itself (warp, mask polygon, modulator,
  gamma) is *not* the problem. The problem is that terms appear
  before context exists. Renames are explicitly out of scope; the
  fix is in-context glossaries and progressive disclosure.

### 3.6 Technical / architectural constraint problems (root cause)

- `App` holds `Option<RunningApp>` — a hidden state machine. There
  is no explicit place to model `Launcher`, `Editing`, `Live`,
  `Failed` as distinct, gated states.
- `Project` is mutated directly by control-panel render functions
  (e.g., slider widgets bind `&mut project.gamma`). Every such
  mutation site is a missed undo opportunity and a telemetry
  blind spot.
- `Project::load` runs schema migration (`project/migrate.rs`) but
  has no post-migration sanity audit — projects with `scale =
  [0, 0]`, degenerate warp grids, or invalid mask polygons load
  silently and render nothing.
- `ControlEvent` already exists as the abstraction over input
  sources (keyboard / MIDI / OSC) but mutations via the egui
  control panel bypass it. This asymmetry is the architectural
  hole that blocks undo/redo and full telemetry.

### 3.7 First-use catastrophic failure mode (caught during audit)

Loading the existing `~/p1.rmap.json` produces zero visible
content because `transform.scale` was serialised as `[0, 0]`. The
editor overlay (pink/blue layer bounds) is the only thing
projected. A non-technical operator would conclude "the app
doesn't work" and uninstall. **This bug is fixable in
architecture (project audit), not in microcopy.**

---

## 4. Product goals

### 4.1 Goals (in scope)

| Goal | What it means concretely |
|------|--------------------------|
| **First-use clarity** | A user opening rmap for the first time identifies the next action within 5 seconds, every time, without reading documentation. |
| **Reduction of cognitive load** | Default surface ≤ 12 visible interactive controls (target measured on the canvas + toolbar; Advanced disclosure not counted). |
| **Simpler onboarding** | A bundled demo gets the user to "photo on a wall" in ≤ 120 seconds wall-clock. |
| **Clearer workflow sequencing** | The 7-step canonical flow (Section 7.3) is supported left-to-right, top-to-bottom in the UI; users do not have to navigate tabs to follow it. |
| **Safer editing interactions** | Every project mutation is undoable (Cmd-Z); destructive operations (delete layer, reset warp, clear mask) require confirmation or are reversible for ≥ 30 seconds. |
| **Beginner / advanced separation** | Single Advanced disclosure houses all power-user controls; default surface contains zero controls a beginner cannot use safely. |
| **Failure-mode visibility** | A loaded project that produces no visible content surfaces a one-line warning + auto-fix action within 1 frame of load. |
| **Domain-vocabulary preservation** | Warp, mask polygon, modulator, gamma, blend mode, crossfade, scene remain. Each gets a 1–2 sentence in-context glossary on first encounter. |

### 4.2 Non-goals (out of scope for this overhaul)

- Renaming any domain term (warp → "fit to wall" is **explicitly
  out**; see audit Section D5).
- Multi-projector / multi-output workflows.
- Lighting outputs (Art-Net, sACN) — defer to roadmap Phase 4.
- Audio-reactive UX beyond what M7 already plumbs.
- AR-based or camera-based calibration.
- iPad / iOS port (the interface should *feel* iPad-like; we are
  not shipping an iPad app).
- A new render pipeline. v1/v2's compositor + warp + mask SDF +
  effects + gamma chain stays.
- A node-graph effect editor.
- Cross-platform parity beyond what is already there. macOS
  remains the primary target; Linux/Windows continue as best-
  effort.

---

## 5. Success metrics

Measured at GA against the current `main` baseline.

### 5.1 Product metrics

| Metric | Baseline (today) | Target |
|--------|------------------|--------|
| Time-to-first-projected-pixel (cold start, no project, single demo click) | ∞ (not possible without docs) | ≤ 30s |
| Time-to-first-photo-on-wall (cold start, user-supplied photo, warp fitted) | ≥ 15 min (estimated, no usability study yet) | ≤ 120s |
| First-launch completion rate (operator launches → reaches "Go live") | not measured | ≥ 70% |
| Sessions where a project loads to zero visible content with no warning | currently possible | 0 |

### 5.2 UX metrics

| Metric | Baseline | Target |
|--------|----------|--------|
| Visible interactive controls on first launch (empty state) | ~14 (5 tabs + 3 sliders + Save row + Project file row + window checkbox + ?) | ≤ 5 (launcher: 3 buttons + projector picker + test) |
| Default-surface controls in editing mode | ~25–30 across all five tabs | ≤ 12 on the canvas + toolbar |
| Distinct typed inputs required for full first-mapping flow | ≥ 2 (file path + project filename) | 0 |
| Average time to identify "where to start" in user testing | not measured | < 5 s, n ≥ 5 sessions |

### 5.3 Engineering metrics

| Metric | Baseline | Target |
|--------|----------|--------|
| Project mutations not routed through `Command` | ~all egui binding sites | 0 (all UI mutations go through `Command::apply`) |
| Lines in `app.rs` | ~1400+ | ≤ 1000 (state machine + dispatch only; canvas / launcher / advanced split out) |
| Modules with bidirectional coupling between UI panel and render runtime | several (control_panel directly mutates Project, app rebuilds layers from project) | clear unidirectional flow: UI → Command → Project → Runtime rebuild |
| Project load failure modes covered by audit warnings | 0 | ≥ 6 (zero scale, degenerate warp, mask < 3 vertices, missing asset, monitor-out-of-range, unsupported schema_version) |
| Undo stack hit rate in self-testing | n/a | ≥ 95% of mutations reversible |

---

## 6. Guiding principles

These govern every implementation decision in this overhaul.
Conflict between principles is resolved by ordering: earlier
principles win.

1. **Direct manipulation over control panels.** Anything that has
   a visual representation on the canvas is edited on the canvas.
   Sliders are a fallback, not the primary interface.
2. **Progressive disclosure over upfront complexity.** A beginner
   never sees a control they don't yet need. Advanced disclosure
   is a single, clearly-labelled door.
3. **Obvious next step at all times.** Every screen, every state,
   shows the user what to do next. Empty states do not happen
   silently.
4. **Safe defaults over expert-first exposure.** Defaults are
   *guaranteed-to-work-out-of-the-box* values, not industry-
   standard expert values. Gamma defaults to 1.0; warp defaults
   to corner-pin; scale defaults to identity (never zero).
5. **Teach the vocabulary, do not rename it.** Domain terms stay.
   Each gets a 1–2 sentence in-context glossary the first time it
   appears.
6. **Architecture supports extension without leaking complexity to
   the UI.** New effects, new modulators, new zone templates can
   be added without restructuring the control panel.
7. **Every mutation is reversible.** Undo is a feature, an
   architecture, and a confidence guarantee.
8. **Single mutation pathway.** Every change to `Project` flows
   through `Command::apply`. No widget binds `&mut project.field`
   directly. This is the price of admission for undo, telemetry,
   replay, and migration.
9. **No silent failures.** A project that produces zero visible
   pixels surfaces a warning, in plain language, with an action.
10. **Show-day reliability is non-negotiable.** Nothing in this
    overhaul reduces the existing operator-safety guarantees
    (blackout, freeze, panic recovery, display-sleep prevention,
    daily log rolling).

---

## 7. User journeys and task model

### 7.1 Primary user types

| Persona | Role | Skill profile | Primary goal |
|---------|------|---------------|--------------|
| **Eva — Event volunteer** | Friend-of-couple at a wedding, AV teacher prepping a school play, lighting volunteer at a small venue. | Non-technical. Uses macOS daily; never opens Terminal. Has a projector and a laptop. | Project a few photos and graphics on a wall during one event. |
| **Marco — Visual operator** | Self-taught VJ, indie performer, event company freelancer. | Comfortable with creative tools (Resolume, MadMapper, OBS). Has used a CLI but doesn't enjoy it. | Build a small repeating show with scenes, transitions, and live reactivity. |
| **Sami — rmap power user / contributor** | The current author and contributors. | Rust-fluent, knows the codebase, knows projection mapping. | Push the limits of effects, modulators, advanced warp, custom presets. |

The overhaul is optimised for **Eva** without breaking **Marco**
or **Sami**. The Advanced disclosure exists for Marco and Sami;
the default surface is Eva's.

### 7.2 Jobs to be done

| # | Job | Who |
|---|-----|-----|
| J1 | Confirm my projector is working before I commit. | All |
| J2 | Get my image on the wall, fitted, in minutes. | Eva, Marco |
| J3 | Save what I just made so I can recall it tomorrow. | All |
| J4 | Build a sequence of looks I can flip between live. | Marco, Sami |
| J5 | Adjust colour, brightness, animation — when needed. | Marco, Sami |
| J6 | Run a show without surprises (blackout, freeze, recovery). | All |

J1 and J2 are the overhaul's headline jobs; today both are
hostile.

### 7.3 The canonical first successful session (target state)

```
0.  Double-click rmap.app                                [today: terminal command]
1.  Launcher opens; projector auto-selected by name      [today: --monitor INDEX]
2.  Click "Try a demo" → pick "Window glow"              [today: no equivalent]
3.  Canvas opens; demo loaded; output windowed on wall   [today: fullscreen by default]
4.  Drag four corner handles to match the wall           [today: separate Mapping tab]
5.  Drop your own photo onto the canvas                  [today: typed file path]
6.  Click "Save as…" → name "My first show"              [today: type *.rmap.json]
7.  Click "Go live" → fullscreen; show-day strip appears [today: silent fullscreen]

Total wall-clock: ≤ 120 s.
```

### 7.4 Where today's product breaks

| Step | Today's failure | Fix in this plan |
|------|-----------------|------------------|
| 0 | CLI required | WP-3 Launcher window |
| 1 | Numeric monitor index, no preview | WP-13 Live monitor names + projector test |
| 2 | No demos | WP-4 Bundled demo project |
| 3 | Fullscreen by default takes over the projector | WP-3 Launcher chooses windowed for setup |
| 4 | Mapping in a separate tab on a placeholder canvas | WP-6 Canvas merge |
| 5 | Typed path required | WP-9 Drag-drop + file picker |
| 6 | Filename extension knowledge required | WP-12 Autosave + Save As |
| 7 | No transition; no show-day controls visible | WP-3 Go-live transition + WP-10 Show-day strip |

---

## 8. Scope breakdown

Eleven workstreams. Each is scoped, dependency-mapped, and
risk-flagged so it can be turned into one or more epics.

### W1. Onboarding and first-run experience

- **Objective.** A first-time user reaches "photo on a wall" in
  under 2 minutes without docs.
- **User problem.** No path from app launch to first success.
- **In scope.** Launcher window, three start options, projector
  picker with named displays + test pattern, bundled demo project,
  guided banner during first session.
- **Out of scope.** A multi-step wizard. The launcher is a
  one-screen choice, not a 5-step tour.
- **Dependencies.** WP-1 (state machine), WP-13 (monitor names).
- **Risks.** Overdesigning the launcher into a full tour. Demo
  asset licensing.
- **Expected impact.** Highest single-change impact in the audit.

### W2. Information architecture and navigation

- **Objective.** Replace the five-tab structure with a single
  canvas + Advanced disclosure that mirrors the user's task model.
- **User problem.** Users don't know where to start.
- **In scope.** Canvas as default editing surface; Advanced
  disclosure; toolbar; left layer strip; bottom cue strip; right
  inspector (selection-driven).
- **Out of scope.** A new docking / panel system. We are
  collapsing IA, not generalising it.
- **Dependencies.** W3 (canvas interactions), W6 (Advanced
  contents).
- **Risks.** Power-user regression if Advanced contents are
  incomplete.
- **Expected impact.** Largest in-app clarity win.

### W3. Core mapping workflow redesign

- **Objective.** Make the live preview the only place mapping
  happens.
- **User problem.** Mapping today happens on a placeholder
  thumbnail disconnected from the user's image.
- **In scope.** Promote v2's Scene-tab preview to the full canvas;
  warp handles directly on the live image; "Warp" mode toggle on
  the toolbar.
- **Out of scope.** Multi-warp authoring UI improvements beyond
  what v2 ships. (Multi-warp data model already exists.)
- **Dependencies.** W2 (canvas placement).
- **Risks.** Warp-corner hit testing must coexist with layer-body
  hit testing without ambiguity (already partially solved in v2).
- **Expected impact.** Removes the single most disorienting
  workflow split.

### W4. Canvas / stage interaction model

- **Objective.** Direct manipulation everywhere that something is
  visually editable.
- **User problem.** Today the canvas is partially direct, partially
  side-panel.
- **In scope.** Drop image to add layer; drag to move; drag corner
  to warp; drag mask vertex to edit cutout; double-click an edge
  to insert; shift-click to delete; selection-driven inspector;
  Esc deselects.
- **Out of scope.** Multi-select; gesture-based zoom; touch input.
- **Dependencies.** W2, W3, WP-2 (commands).
- **Risks.** Mode confusion (Warp mode vs Layer mode vs Mask
  mode). Mitigated by clear cursor + banner per mode.
- **Expected impact.** Removes friction at every editing moment.

### W5. Terminology and microcopy

- **Objective.** Same terminology, calmer wording. Add an
  in-context glossary.
- **User problem.** Help text is dense and dev-log-flavoured.
- **In scope.** Rewrite help text, error messages, empty states,
  toasts; add `?` glossary popovers on each advanced label.
- **Out of scope.** Renaming any domain term (warp, mask polygon,
  modulator, gamma, blend mode, scene, crossfade all stay).
- **Dependencies.** None (can run in parallel with most workstreams).
- **Risks.** Glossary text has to be authored carefully; treat
  it as content, not engineering filler.
- **Expected impact.** Cheap, highly visible, builds operator
  confidence.

### W6. Beginner vs advanced separation

- **Objective.** One Advanced disclosure that hides every power-
  user control without losing any of them.
- **User problem.** Beginner sees gamma, modulators, blend modes,
  source-rect editing, and external-pass JSON before they have
  added a single image.
- **In scope.** A single `Advanced` button on the toolbar that
  expands a panel containing: master gamma/brightness/contrast,
  per-layer effect chain, modulator types beyond Static, blend
  modes other than Normal, mesh rows/cols, mask SDF feather,
  source rect editing, external-pass JSON params, project
  autostart flag, output_windowed flag.
- **Out of scope.** A separate "advanced mode" entire UI. Same
  canvas, just one disclosure.
- **Dependencies.** W2.
- **Risks.** "Where did my modulator panel go?" regression for
  Sami/Marco. Mitigated by docs + telemetry on Advanced opens.
- **Expected impact.** Default surface shrinks ~70%.

### W7. Visual hierarchy and layout simplification

- **Objective.** The default screen reads at a glance.
- **In scope.** Single calm dark theme; one warm accent for
  active handles; consistent type scale; clear primary action
  per screen.
- **Out of scope.** Custom font; brand identity; light-mode theme.
- **Dependencies.** W2.
- **Risks.** Pure aesthetic work without measurable outcomes.
  Mitigated by treating it as the *finishing* phase.

### W8. Error prevention, undo, snap, lock

- **Objective.** No edit is unrecoverable; destructive operations
  are visibly undoable.
- **In scope.** Global undo/redo (Cmd-Z / Cmd-Shift-Z);
  confirmation toast for destructive ops (delete layer, clear
  mask); snap-to-edge on warp corners near framebuffer bounds;
  layer lock toggle.
- **Out of scope.** Full version history; per-property animation
  undo (modulator changes are global, not per-frame).
- **Dependencies.** WP-2 (Command pattern).
- **Risks.** Performance — the undo stack must be cheap.
  Mitigated by storing project deltas, not full snapshots.
- **Expected impact.** Largest confidence improvement.

### W9. Templates, empty states, guided starts

- **Objective.** No blank screen ever; every empty state suggests
  a concrete next action.
- **In scope.** Bundled demo projects (3 to start), canvas empty
  state, Looks-folder empty state, scene-strip empty state.
- **Out of scope.** Online template marketplace.
- **Dependencies.** W1 (launcher demos), W5 (microcopy).

### W10. Settings and advanced controls rationalisation

- **Objective.** Settings live in obvious places; the windowed-
  output flag does not require a restart.
- **In scope.** Native macOS menu bar (`File`, `Edit`, `Window`,
  `Help`); `Preferences` window for monitor / windowed /
  autosave-location; runtime hot-swap of windowed↔fullscreen.
- **Out of scope.** Cross-platform menu parity; preferences
  syncing.
- **Dependencies.** W6 (Advanced contents migration).
- **Risks.** Hot-swapping the output window between windowed and
  fullscreen requires re-creating the wgpu Surface — must be
  hardened against panic.

### W11. Instrumentation and UX observability

- **Objective.** Measure success against Section 5 metrics.
- **In scope.** `tracing` spans on `session_start`,
  `first_layer_added`, `first_warp_drag`, `go_live_clicked`,
  `project_audit_warned`, `advanced_opened`, `undo_invoked`,
  `demo_clicked`, `time_to_first_pixel`, `time_to_first_save`.
  Local-file telemetry only by default; opt-in upload deferred
  to a later spec.
- **Out of scope.** Network telemetry uplink; user identification.
- **Dependencies.** WP-2 (Command pattern, since most events are
  command-emit moments).
- **Risks.** Privacy — never log file paths, project content, or
  asset filenames.

---

## 9. Prioritization model

P0 = ship-blocking foundation. P1 = core overhaul. P2 = polish.

| Tier | Definition |
|------|------------|
| **P0** | Without this, nothing else lands cleanly. Architectural foundations + the launcher. |
| **P1** | The visible overhaul that achieves the audit's stated outcomes. |
| **P2** | Polish and delight that turn 7-star into 7+-star. |

| Workstream | Tier | Reason |
|------------|------|--------|
| W1 Onboarding | P0 + P1 | Launcher itself is P0 (foundation); demos are P1 (content). |
| W2 IA | P1 | Core overhaul. |
| W3 Mapping workflow | P1 | Core overhaul. |
| W4 Canvas interactions | P1 | Core overhaul. |
| W5 Microcopy / glossary | P1 | Cheap, parallel, visible. |
| W6 Advanced disclosure | P1 | Core overhaul. |
| W7 Visual hierarchy | P2 | After IA settles. |
| W8 Undo / safety | **P0** | Foundation; everything else assumes it. |
| W9 Templates / empty states | P1 | Demo bundle and empty states ship together. |
| W10 Settings rationalisation | P2 | Native menus are nice-to-have once the canvas works. |
| W11 Instrumentation | **P0** (hooks) + P2 (dashboards) | Hook in early so we measure from day one. |

**Foundational vs enhancement.** P0 work (state machine, command
pattern, project audit, telemetry hooks, launcher shell) **must
land before** any visual refinement. Without it the visual work
is wasted: a beautiful canvas that can't undo, can't measure
itself, and silently renders nothing is worse than today's.

---

## 10. Delivery phases

Six phases. Estimates are eng-weeks for a single full-time Rust
engineer with part-time design + product support. Adjust to team
size; the *order* is non-negotiable.

| Phase | Purpose | Eng-weeks (est.) |
|-------|---------|------------------|
| Phase 0 | Discovery, audit alignment, decision resolution | 1 |
| Phase 1 | UX foundations + architecture (state machine, commands, audit, telemetry) | 3 |
| Phase 2 | First-run experience (launcher, demo, empty states, drag-drop) | 3 |
| Phase 3 | Interaction model overhaul (canvas merge, Advanced disclosure, glossary) | 4 |
| Phase 4 | Show-day strip, scene picker, autosave, settings, polish | 3 |
| Phase 5 | Validation, dogfooding, GA readiness | 2 |
| **Total** | | **~16 weeks** |

### Phase 0 — Discovery and alignment

- **Purpose.** Resolve open decisions (Section 14). Lock terminology
  for the glossary. Assemble bundled demo assets (license check).
- **Deliverables.**
  - Decision register (Section 14 questions answered).
  - Bundled demo asset list with licenses.
  - Glossary v0 — 1–2 sentence entries for warp, mask polygon,
    modulator, gamma, blend mode, crossfade, scene, source rect,
    zone template.
  - Wireframes for launcher + canvas + Advanced.
- **Exit criteria.** Decision register signed off by product
  owner; design lead has approved canvas + launcher wireframes.
- **Risk if skipped.** Engineering proceeds with conflicting
  assumptions; rework after Phase 2.

### Phase 1 — UX foundations and architecture

- **Purpose.** Lay the architectural rails for everything else.
- **Deliverables.**
  - Explicit `AppState` enum (WP-1).
  - `init_running_app` decomposition (WP-1.1).
  - `Command` abstraction (extension of `ControlEvent`); central
    `apply_command(state, cmd)` function (WP-2).
  - Undo/Redo stack with the three Reverse-storage rules from
    WP-2 codified at the type level.
  - `ProjectAudit` pass + `Toast` system (WP-15).
  - `tracing` spans on UX events (WP-17).
- **Dependencies.** None — Phase 0 done.
- **Internal weighting.** The sanity check against the codebase
  (`app.rs`, `windows/control_panel.rs`, `windows/scene_editor.rs`)
  established that **WP-2 will consume ~65% of Phase 1** because
  the mutation surface is ~56 sites and three of them
  (`Modulator` enum-variant replacement,
  `mutate_transform_effect` create-or-update,
  `restore_scene` whole-project replacement) need bespoke
  Reverse-storage logic. WP-1 + WP-1.1 + WP-15 + WP-17 together
  share the remaining ~35%.
- **Exit criteria.** WP-1, WP-1.1, WP-2, WP-15 acceptance criteria
  met. All existing project-mutation sites refactored to go
  through `Command`. File-watcher reloads explicitly excluded
  from the undo stack and verified by test.
- **Risk if skipped.** Undo, telemetry, and audit cannot be
  retrofitted cheaply. The Reverse-storage subtleties are the
  kind of thing that produces undo-corruption bugs in week 4 if
  not anticipated in week 1 (see Risk R11).

### Phase 2 — First-run experience

- **Purpose.** New users reach success unaided.
- **Deliverables.**
  - Launcher window (WP-3).
  - Bundled demo project (WP-4).
  - Empty-state hints on canvas (WP-5).
  - Drag-drop + native file picker (WP-9).
- **Dependencies.** Phase 1 (state machine + commands).
- **Exit criteria.** A team member who has never seen rmap before
  reaches "photo on a wall" in ≤ 120s. Internal usability check
  with n ≥ 3 testers.
- **Risk if skipped.** The visible overhaul launches but the door
  is still hostile.

### Phase 3 — Interaction model overhaul

- **Purpose.** Replace the five-tab IA with the canvas + Advanced
  disclosure.
- **Deliverables.**
  - Canvas merge (WP-6).
  - Advanced disclosure (WP-7).
  - In-context glossary (WP-8).
  - Show-day strip (WP-10).
- **Dependencies.** Phase 1, Phase 2.
- **Exit criteria.** Internal users (Eva-style + Marco-style) can
  complete the canonical flow on the new IA without consulting a
  doc.
- **Risk if skipped.** The default surface remains intimidating.

### Phase 4 — Polish and rationalisation

- **Deliverables.**
  - Visual scene picker (WP-11).
  - Autosave + Save As (WP-12).
  - Live monitor names + projector test (WP-13).
  - Theme polish (WP-14).
  - Native menu (WP-16, P2).
- **Dependencies.** Phase 3.
- **Exit criteria.** All WP acceptance criteria met for P1+P2.
- **Risk if skipped.** Beta-quality; not GA-quality.

### Phase 5 — Validation and release readiness

- **Deliverables.**
  - Internal dogfooding cycle ≥ 1 week.
  - External usability test, n ≥ 5.
  - Release notes; updated README.
  - GA-readiness checklist (Section 16).
- **Exit criteria.** Section 5 metrics measured; ≥ 80% of P0+P1
  acceptance criteria green; show-day reliability gates green.

---

## 11. Rust architecture and implementation considerations

This section is the bridge between UX intent and Rust execution.
It does not contain code; it contains the architectural decisions
a senior Rust engineer can derive epics from.

### 11.1 App state machine — make the implicit explicit

**Today.** `App.state: Option<RunningApp>` is the only state
distinction. `resumed` constructs `RunningApp`; `window_event`
mutates it. There is no place to model "the launcher is open" or
"a project failed to load" or "the user clicked Go live and the
output is now fullscreen."

**Target.** An explicit enum:

```
AppState
├── Booting              // pre-resumed; CLI parsed, monitors not yet known
├── Launcher(LauncherState)   // launcher window visible; no editing yet
├── Editing(EditingState)     // canvas + control window visible
├── GoLive(EditingState)      // same as Editing, but output is fullscreen
└── Failed(FailureKind)       // project load / audit / wgpu surface failure
```

Transitions are explicit functions. Invalid transitions are
unrepresentable (e.g., `Booting → GoLive` cannot compile). This
removes a class of "did the renderer initialise?" defensive
checks scattered through the codebase today.

**Migration path.** `RunningApp` becomes the inner type for both
`Editing` and `GoLive`. `Launcher` is a separate struct. The
existing `resumed` handler becomes `Booting → Launcher` (or
`Editing` if `--autostart` is set).

### 11.2 Command pattern — extend `ControlEvent`, do not invent a parallel system

**Today.** `ControlEvent` exists (`SceneRecall`, `Blackout`,
`Freeze`, `ParamSet`, `TapTempo`) and is dispatched through a
single `dispatch_control_event` function. egui control-panel
widgets bypass it and bind `&mut project.field` directly.

**Target.** Rename `ControlEvent` → `Command`. Every project
mutation, including from egui widgets, produces a `Command`.
`Command::apply(state) -> Reverse` is the single mutation
pathway. The `Reverse` is what goes onto the undo stack.

Commands a beginner triggers:

- `Command::AddLayer { kind: LayerKind, position: usize }`
- `Command::RemoveLayer { idx: usize }`
- `Command::MoveLayer { idx: usize, translate: [f32; 2] }`
- `Command::ScaleLayer { idx: usize, scale: [f32; 2] }`
- `Command::RotateLayer { idx: usize, rotate_deg: f32 }`
- `Command::SetWarpCorner { warp: usize, r: usize, c: usize, pos: [f32; 2] }`
- `Command::SetMaskVertex { warp: usize, idx: usize, pos: [f32; 2] }`
- `Command::AddMaskVertex { warp: usize, after: usize, pos: [f32; 2] }`
- `Command::RemoveMaskVertex { warp: usize, idx: usize }`
- `Command::ApplyZoneTemplate { warp: usize, name: &'static str }`
- `Command::SaveScene { slot: usize }` (already exists)
- `Command::RecallScene { slot: usize }` (already exists)

Existing power-user commands stay: `SetGamma`, `SetBrightness`,
`SetContrast`, `SetEffectField`, etc. The egui Advanced panel
emits these.

**Why this is the linchpin.** Without it, undo/redo, telemetry,
serialised replay, headless integration testing, and even the
launcher's "Open recent" all become harder. With it, all of those
are mechanical.

#### Reverse-storage rules

Three patterns in the existing code make naive `Command::Reverse`
storage *wrong*. These rules are mandatory for any new `Command`
variant; the `Command` type should encode them so violations are
compile errors, not runtime bugs.

1. **Whole-enum Reverse.** Any command that replaces an enum
   variant (e.g., `Modulator::Static → Modulator::Sine`) stores
   the *full old enum value*, not just the field that "looks"
   different. Variant-replacement loses unrelated fields
   silently otherwise. Targets: `Modulator`, `LayerKind`,
   `BlendMode`, `Effect`, `FitMode`.
2. **Effects-Vec Reverse for transform mutators.** Drag-translate
   / drag-scale / drag-rotate commands snapshot the entire
   `effects: Vec<Effect>` of the target layer as their Reverse,
   not just the Transform field. Reason: the existing
   `mutate_transform_effect` helper *appends* a default
   `Effect::Transform` to layers that don't have one — a per-
   field Reverse would leave a stray effect on undo.
3. **Snapshot Reverse for whole-project replacements.** Scene
   recall and crossfade tick replace the entire project from a
   `serde_json::Value`. They emit a single
   `Command::ApplyProjectSnapshot { snapshot }` whose Reverse is
   the previous full snapshot. Crossfade-tick commands are
   flagged `non_undoable: true` and never enter the user-facing
   undo stack.

#### What is excluded from the command pathway

- File-watcher reloads (SVG / image hot-reload). External state,
  not a user edit. `LayerState` GPU-runtime mutations stay
  separate from `Project` domain mutations.
- Output-state ephemerals (`blackout`, `freeze`, `test_pattern`,
  `show_editor_overlay`) — session-scoped, not undoable. They
  *do* emit telemetry events through the command pathway for
  observability consistency, but their commands are
  `non_undoable: true`.
- M7 `ParamSet` / `controls/param.rs` stubs — kept as
  `#![allow(dead_code)]`; v3 does not revive them.

### 11.3 Separation of concerns

| Layer | Today | Target |
|-------|-------|--------|
| **Domain** | `project::schema::Project` | Unchanged. Still serde-serialisable. |
| **Application state** | implicit in `RunningApp` | Explicit `AppState` enum (11.1) |
| **UI state** | `ControlPanelState`, `SceneEditorState` | Unchanged in structure; reduced in scope as panels collapse. |
| **Render state** | `Compositor`, `WarpRenderer`, etc. | Unchanged. |
| **Interaction state** | scattered (drag sessions, hover) in `SceneEditorState` | Consolidated; mode-aware (Layer / Warp / Mask / Inspect). |
| **Mutation pathway** | egui binds `&mut project.field` directly | `Command::apply` is the only path. |

### 11.4 Project loader audit and toasts

**Today.** `Project::load` migrates schema and returns the project.
No semantic validation.

**Target.** A new `ProjectAudit` pass runs after migration:

```
ProjectAudit::run(&project) -> Vec<AuditFinding>
```

Findings include severity (`Info` / `Warn` / `Critical`) and an
optional `AutoFix` (a `Command` that resolves it). The UI surfaces
warnings as toasts on first frame after load:

| Finding | Severity | Auto-fix |
|---------|----------|----------|
| Layer with `transform.scale = [0, 0]` | Warn | `SetLayerScale(idx, [1, 1])` |
| Layer warp grid degenerate (< 2×2 or non-monotonic) (`DegenerateLayerWarp`, schema v4) | Warn | `ResetLayerWarpMesh(layer_idx)` |
| Layer mask polygon with < 3 vertices (`LayerMaskTooFew`, schema v4) | Info | clear that layer's mask |
| Asset path missing on disk | Warn (schema v4 — was Critical in v3) | `RelinkAssetPath(layer_idx, …)` via T2.24 picker |
| `output_monitor_index` ≥ available monitors | Warn | reset to 0 |
| `schema_version` newer than supported | Critical | refuse load, suggest upgrade |
| Project with zero layers | Info | empty-state hint |
| Schema v3 project consolidated ≥ 2 warps onto layers (`MultipleWarpsConsolidated`, schema v4 migration) | Warn | None (re-map per layer) |

The audit is deterministic, pure, and unit-testable. It is the
direct fix for the catastrophic failure mode caught in the audit.

### 11.5 Toast system

A small, dependency-free in-process notification queue:

```
Toast { kind: Info | Warn | Error, message, action: Option<Command>, ttl: Duration }
```

Surfaced via egui in the canvas top-right, max 3 visible, FIFO.
Toasts log to `tracing` at matching levels for off-screen review.
**Not** a system-level notification; never leaves the app.

### 11.6 State machines for editor modes

Within `Editing`, the canvas has interaction modes:

```
EditMode
├── Layer        // drag/scale/rotate the selected layer's body
├── Warp         // drag the selected layer's warp corners
├── Mask         // edit the selected layer's mask polygon
└── Inspect      // selection only, no drag
```

Mode transitions are explicit and cursor-driven. Banner text
changes per mode. This replaces today's overloaded handler (the
v2 scene editor inspects modifier keys to decide between drag /
scale / rotate within a single mode).

**Each non-Inspect mode is implicitly scoped to the selected
layer.** Warp and Mask modes paint and hit-test against
`project.layers[selected].warp` only. With no layer selected, the
mode banner reads *"Select a layer first."* This matches the
per-layer mapping data model introduced in §11.6a.

### 11.6a Per-layer warp + mask + effects architecture *(v3 / schema v4)*

**Today (schema v3).** `Project.layers: Vec<LayerConfig>` and
`Project.warps: Vec<WarpMesh>` are independent collections. The
render graph composites every layer first into a shared `warp_rt`
texture, then for each `WarpMesh` reads a `source_rect` of that
composite and remaps it to the projector. Layers and warps are
**unbound** — every layer is warped through whatever shared
geometry the composite gets.

**Target (schema v4).** Each `LayerConfig` owns its own `WarpMesh`
(which carries the mask polygon and feather):

```rust
struct Project {
    schema_version: 4,
    layers: Vec<LayerConfig>,
    // warps: Vec<WarpMesh>   REMOVED
    ...
}

struct LayerConfig {
    id, kind, enabled,
    transform, effects, blend_mode, opacity,
    warp: WarpMesh,         // NEW: per-layer mapping
}
```

**Render graph.**

```
per-layer raster ──► for each layer in order:
                       layer_pre_warp ── warp pass ──► warp_scratch (1× projector size)
                                                          │
                                                          ▼
                                                     blend-composite onto projector_rt
                                                       (uses layer.blend_mode + opacity)
                       │
                       ▼
                    gamma → overlay → swap
```

`warp_scratch` is a single projector-sized texture reused across
layers within a frame (clear-write-read-discard). The shared v3
`warp_rt` is gone. The egui control-window preview re-binds to the
projector-RT view (post-warp, pre-gamma); the operator sees what
the projector sees.

**Why per-layer.** Operators expect each layer to map onto its own
physical surface (photo on wall A, video on wall B, SVG overlay on
the door). The shared-warp model contradicts the layer-thumbnail
strip in WP-6: dragging a warp corner deforms every layer regardless
of which thumbnail is selected. Per-layer mapping aligns the
interaction model with the data model.

**Migration v3 → v4** (one-shot at load):

- For each layer in v3 `Project.layers`: clone `Project.warps[0]`
  (or `WarpMesh::identity()` if no warps existed) onto
  `layer.warp`.
- Drop the top-level `warps` field.
- Bump `schema_version` to 4.
- If the v3 project had > 1 warps: emit
  `AuditKind::MultipleWarpsConsolidated` (Warn, once per session) so
  the operator knows the consolidation may have lost intent.

The migration is intentionally lossy when M > 1; preserving the
multi-warp geometry would require a heuristic ("which layer goes
with which warp?") that's wrong as often as right. Operators
re-map per layer, guided by the toast.

**Mutation surface.** All warp/mask-targeted `Mutation` variants
rename `warp_idx` → `layer_idx`:

| v3 variant | v4 variant |
|---|---|
| `SetWarpDimensions` | `SetLayerWarpDimensions` |
| `SetMaskPolygon` | `SetLayerMaskPolygon` |
| `AddMaskVertex` / `RemoveMaskVertex` / `SetMaskVertex` | `AddLayerMaskVertex` / `RemoveLayerMaskVertex` / `SetLayerMaskVertex` |
| `ResetWarpMesh` | `ResetLayerWarpMesh` |
| `SetWarpMaskFeather` | `SetLayerMaskFeather` |
| *(new)* | `SetLayerWarpCorner { layer_idx, r, c, new, old }` |

**Audit findings.** `DegenerateWarp` →
`DegenerateLayerWarp { layer_idx }`; `MaskTooFew` →
`LayerMaskTooFew { layer_idx, vertex_count }`. New
`MultipleWarpsConsolidated`.

**Tasks.** This is captured by Phase 3 tasks **T3.0a** (schema +
migration), **T3.0b** (render graph), **T3.0c** (mutation rename),
**T3.0d** (audit rename + new finding). They sit at the front of
Phase 3, gating every other Phase 3 task. See
`003-tasks-phase-3.md`.

### 11.7 Decoupling UI panels from render runtime

**Today.** `control_panel` calls into `App` to rebuild GPU layer
runtime when a layer is added. That coupling is acceptable but
brittle.

**Target.** A `Command` that mutates `Project::layers` returns a
side-effect descriptor (`SideEffect::RebuildLayers`,
`SideEffect::None`, `SideEffect::ReinitWarp`, etc.) that the
event loop applies after `Command::apply`. Panels never call
into the renderer directly.

### 11.8 Progressive disclosure without conditional chaos

**Bad pattern.** A single panel with `if state.advanced { … }` checks
sprinkled through every widget.

**Good pattern.** Two distinct render functions: `render_default`
and `render_advanced`. Both consume the same `Project` reference
but render disjoint widget sets. The `Advanced` button is a
boolean toggle in `ControlPanelState`; it does not gate
individual widgets.

This keeps the default surface code-readable without conditional
ladders.

### 11.9 Reusable UI primitives

Introduce a small in-tree primitives module:

- `glossary_label(ui, term, body)` — a label with a `?` icon that
  opens a popover with the glossary entry. Used everywhere
  domain terminology surfaces.
- `toast_strip(ui, &mut ToastQueue)`.
- `command_button(ui, label, cmd, dispatcher)` — a button that
  emits a `Command`.
- `mode_banner(ui, mode)` — the per-mode instruction strip.
- `drop_target(ui, on_drop: impl FnMut(Vec<PathBuf>))` — drop
  zone for files.

These collapse repeated egui boilerplate and centralise visual
treatment.

### 11.10 Hot-swap windowed ↔ fullscreen

**Today.** `output_windowed` is a bool serialised in the project,
"Restart rmap to apply."

**Target.** Pressing **Go live** transitions from windowed to
fullscreen at runtime. The implementation re-creates the wgpu
Surface bound to the existing `OutputWindow` after re-asking the
winit Window to enter `Fullscreen::Borderless`. A failure here is
recoverable (`AppState::Failed → user retries → AppState::Editing`).

Tested under panic recovery: if surface re-creation panics, the
existing `catch_unwind` recovery path returns the user to
`Editing` with a toast.

### 11.11 Testing strategy

| Layer | Test | Tooling |
|-------|------|---------|
| `Command::apply` semantics | Unit tests on `Project` | `cargo test`, no GPU |
| `ProjectAudit` | Table-driven unit tests | `cargo test` |
| Undo/redo stack | Property tests (apply N commands, undo N times → original state) | `proptest` |
| State machine transitions | Unit tests on `AppState` transition fns | `cargo test` |
| egui rendering (visual) | Snapshot tests on egui-tessellated output | `egui_kittest` if mature; otherwise manual |
| End-to-end flow | Headless wgpu test feature (already exists for golden images); extended with synthesised input events | `cargo test --features gpu-tests` |
| Show-day reliability | Existing panic-recovery + display-sleep tests | unchanged |

The single biggest investment: a **headless command-driven test
harness** that consumes a script of commands and asserts the
resulting `Project` and render output match a golden file. This
replaces "manual click-through testing" for everything except the
final visual QA pass.

### 11.12 Telemetry hooks

`tracing` spans on the events listed in 11.2. Default subscriber
remains the daily-rolling file appender. A new `ux_metrics`
helper that:

- Records `time_to_first_pixel` from `session_start` to first
  successful render.
- Records `time_to_first_warp_drag` from `first_layer_added`.
- Records `advanced_opened` count per session.
- Records `undo_invoked` count per session.

The metrics are written to a per-day JSON file alongside the log;
no network upload in this phase.

---

## 12. Detailed implementation work packages

Each work package below carries enough detail for a single epic.
Owner roles: **PO** = product owner, **DES** = design,
**RUST** = Rust engineering, **QA** = quality / dogfooding.

### WP-1. Explicit AppState machine *(P0, Phase 1)*

- **Problem.** `Option<RunningApp>` is the only state distinction;
  `Launcher`, `Failed`, and `GoLive` cannot be modelled.
- **Rationale.** Foundation for the launcher (W1), Go-live
  transition, and project-audit failure handling.
- **Outcome.** All app states are typed; invalid transitions
  unrepresentable.
- **Implementation notes.** See Section 11.1. Move existing
  `RunningApp` into `Editing(EditingState)`. Refactor
  `resumed`/`window_event` to dispatch on `AppState`. Preserve
  `--autostart` (`Booting → Editing` directly). The macOS
  re-`resumed` guard at `app.rs:1197` (`if self.state.is_some()`)
  becomes `matches!(self.state, AppState::Launcher(_) |
  AppState::Editing(_) | AppState::GoLive(_))` — easy to miss in a
  refactor PR; QA must verify suspend/resume on macOS still works.
  `winit::ControlFlow` is set globally to `Poll` at `app.rs:374`;
  derive it from `AppState` instead (`Wait` for `Launcher` and
  `Failed`, `Poll` for `Editing`/`GoLive`) to save battery and
  CPU when no animation is running.
- **Dependencies.** None.
- **Done means:**
  - `AppState` enum exists with at least 5 variants
    (`Booting`, `Launcher`, `Editing`, `GoLive`, `Failed`); a
    sixth `ProjectLoading` is documented as a future variant but
    not implemented in v3 (project loading remains synchronous).
  - All `&mut self.state.as_mut()` patterns in `app.rs` are gone.
  - The macOS re-`resumed` guard handles all four "already
    running" states.
  - `ControlFlow` is selected per-state (no global default).
  - Adding a new state is a one-line enum extension.
  - `cargo test` passes; existing flows (CLI launch, autostart,
    `--list-monitors`) work unchanged.
- **Owner.** RUST.

### WP-1.1. Decompose `init_running_app` *(P0, Phase 1)*

- **Problem.** `init_running_app` (`app.rs:504–638`, ~130 lines)
  brings up GPU, audio, MIDI, OSC, sleep assertion, control
  window, output window, all pipelines, and the layers Vec in
  one function. WP-3 (Launcher) needs to call most of this from
  the `AppState::Launcher → AppState::Editing` transition; today
  it is monolithic and hard to compose.
- **Rationale.** Cleanly unblocks WP-3 and reduces the diff size
  of every future change to startup.
- **Outcome.** Startup decomposed into composable steps owned by
  the right modules.
- **Implementation notes.** Split into:
  - `init_gpu()` — `GpuContext::new`, no windowing.
  - `init_output_window()` — `OutputWindow::new` + `Renderer`.
  - `init_control_window()` — `ControlWindow::new`, optional.
  - `init_inputs()` — keyboard + (feature-gated) audio / MIDI /
    OSC sources.
  - `init_render_graph()` — `Compositor`, per-layer `WarpRenderer`
    instances (one per `LayerState` under schema v4 — see §11.6a),
    `GammaPipeline`, `OverlayPipeline`, projector RT,
    `warp_scratch` RT (the per-frame scratch buffer that replaces
    the v3 shared `warp_rt`), `LayerState` Vec.
  - The launcher uses `init_gpu` + `init_inputs` only; the
    editor adds the rest on transition.
- **Dependencies.** None (can ship before or alongside WP-1).
- **Done means:**
  - `init_running_app` is gone or reduced to an orchestrating
    one-pager.
  - Each split function is independently callable from a unit
    test (no GPU device assertions in tests that don't need a
    GPU).
  - Existing flows behave identically (smoke-tested).
- **Owner.** RUST.

### WP-2. Command abstraction + Undo/Redo *(P0, Phase 1, ~65% of phase budget)*

- **Problem.** egui widgets bypass `ControlEvent`; mutations are
  scattered; undo is impossible.
- **Rationale.** Linchpin for undo, telemetry, headless tests,
  serialised replay.
- **Outcome.** Every project mutation flows through `Command`.
  Cmd-Z works app-wide.

#### Mutation surface (measured against the codebase)

The plan's earlier "every today-visible widget" framing
underestimated the surface. Counted sites that must be migrated:

| Location | Count | Examples |
|----------|-------|----------|
| `windows/control_panel.rs` direct bindings | ~10 always-visible | `project.gamma`, `project.brightness`, `project.contrast`, `project.crossfade_duration_s`, `project.output_windowed`, `layer.opacity`, `layer.enabled`, `WarpMesh.mask_feather`, `WarpMesh.rows`, `WarpMesh.cols` |
| `windows/control_panel.rs` `modulator_slider` instantiations | ~15 | One per modulator-driven field across the effect chain (hue, saturation, brightness, contrast inside `Effect::Color`; blur radius; transform rotate/scale_x/scale_y) |
| `windows/control_panel.rs` `show_effect` per-effect sliders | ~9 | Effect parameters that are *not* modulator-wrapped (`Effect::Transform.translate`, etc.) |
| `windows/control_panel.rs` scenes / layers / mapping buttons | ~13 | 9 scene save+recall pairs, layer ↑/↓ reorder, layer add, preset Apply, zone-template buttons, "Reset to identity", "clear mask" |
| `windows/scene_editor.rs::handle_scene_input` | ~6 | Drag-translate, drag-scale, drag-rotate (via `mutate_transform_effect`), mask vertex drag, mask vertex insert, mask vertex delete |
| `app.rs::window_event` | 3 | `DroppedFile` layer push, `KeyCode::B/F/T/O` output-state toggles |
| **Total** | **~56 sites** | |

#### Reverse-storage rules (critical for undo correctness)

Three patterns in the existing code make naive `Command::Reverse`
storage *wrong*. Each must be handled explicitly.

1. **Whole-enum Reverse for variant-replacement commands.**
   `Modulator` is a multi-variant enum
   (`Static`/`Sine`/`Triangle`/`Noise`/`Bpm`/`Audio`); the modulator
   picker at `control_panel.rs:907–922` swaps variants wholesale
   and *loses the user's previous field values* on the way through.
   Rule: every `Command::SetModulator { layer, effect, field, new:
   Modulator }` stores the *full old `Modulator` value* (including
   nested fields) as its Reverse. The same rule applies to any
   `Command` that touches an enum (`LayerKind`, `BlendMode`,
   `Effect`).
2. **Create-or-update Reverse for `mutate_transform_effect`.**
   `scene_editor.rs:147` *appends* a default `Effect::Transform`
   to a layer's effects Vec if none exists, then mutates it. The
   Reverse must distinguish "I created this Transform" from "I
   modified an existing one" or undo will leave a stray effect
   behind. Rule: drag-translate / drag-scale / drag-rotate
   commands snapshot the *entire* `effects: Vec<Effect>` of the
   target layer as their Reverse — cheap (small Vec, deep clone)
   and unambiguous.
3. **Snapshot Reverse for whole-project replacements.**
   `schedule_scene_recall` (`app.rs:188`) and the crossfade tick
   (`app.rs:1508–1518`) call `restore_scene(project,
   &serde_json::Value)` — they replace the entire project. Trying
   to decompose them into per-field commands is wrong and would
   break crossfade math. Rule: scene recall and crossfade tick
   emit a single `Command::ApplyProjectSnapshot { snapshot:
   serde_json::Value }` whose Reverse is the previous full
   snapshot. Crossfade tick frames may *not* push to the undo
   stack (they fire ~60×/s); only the *start* of a recall does.

#### What does NOT go through `Command`

- **File-watcher reloads** (`app.rs:1431–1500`). SVG/image
  hot-reload is external state, not a user edit. Routing through
  `Command` would pollute the undo stack with reloads the user
  didn't request. `LayerState` (GPU runtime) mutations stay
  separate from `Project` (domain) mutations.
- **`ControlEvent::ParamSet`** and the rest of `controls/param.rs`.
  These M7 stubs remain `#![allow(dead_code)]` for v3 (see
  `controls/mod.rs:7`); WP-2 renames `ControlEvent → Command`
  without disturbing them. Param binding is v2.5+ scope.
- **Output-state toggles** (`OutputState::blackout`, `freeze`,
  `test_pattern`, `show_editor_overlay`) are session-scoped
  ephemeral state, not project state, and are not undoable.
  However, telemetry hooks (WP-17) *do* fire on these via the
  command pathway for analytics consistency.

#### Implementation notes

- See Section 11.2.
- Migrate existing `ControlEvent` variants into `Command`.
  Replace every `&mut project.X` egui binding with a
  `Command`-emitting helper (`command_button`, `command_slider`,
  `command_checkbox`). Some sliders need a "live preview" pattern:
  draw against a simulated value during drag, emit the `Command`
  on drag end (`Response::drag_stopped()`).
- Widen `ControlPanelAction` (`control_panel.rs:107`) from a
  three-variant enum to `Vec<Command>` returned per frame.
- Implement `UndoStack` with a soft cap (200 entries) and per-
  command reversal. Crossfade-tick `ApplyProjectSnapshot`
  commands must be flagged `non_undoable: true`.

- **Dependencies.** WP-1.
- **Done means:**
  - `Command` enum covers ≥ 95% of `Project` mutations across
    all ~56 sites enumerated above. Specifically named coverage
    targets:
    - All 10 always-visible bindings.
    - All ~15 `modulator_slider` sites.
    - All ~9 `show_effect` sites.
    - All ~13 scene/layer/mapping buttons.
    - All 6 `scene_editor::handle_scene_input` paths.
    - The `DroppedFile` layer-add path in `app.rs`.
  - Undo/redo works for all P1 commands.
  - The three Reverse-storage rules above are codified as enum-
    variant constructors that *force* correct Reverse capture
    at the type level (compile error if a Modulator command
    forgets to store the old value).
  - `cargo test` includes a proptest: any sequence of `Command`
    applications + matching undos returns the project to a
    byte-equal `serde_json::Value` snapshot.
  - No widget in `windows/control_panel.rs` or
    `windows/scene_editor.rs` binds `&mut project.X` directly.
  - File-watcher hot-reload events do *not* enter the undo stack
    (verified by a unit test that fires N reloads and checks
    `UndoStack::len() == 0` before any user input).
- **Owner.** RUST.

### WP-3. Launcher window *(P0/P1, Phase 2)*

- **Problem.** The CLI is the front door.
- **Rationale.** Highest single-change first-impression impact.
- **Outcome.** Double-click → launcher → choose start.
- **Implementation notes.** New egui window in
  `windows/launcher.rs`, sharing the wgpu device. Three big
  buttons (New / Open recent / Try a demo); projector picker with
  named monitors (WP-13); "Test" button that fires a 5-second
  test pattern on the selected projector via existing
  `TestPatternRenderer`. Launcher commits to a `Command::Launch
  { project: Path, monitor: usize, windowed: bool }` that
  transitions `AppState::Launcher → AppState::Editing`.
- **Dependencies.** WP-1, WP-13, WP-4.
- **Done means:**
  - Double-clicking `rmap.app` shows the launcher within 2s.
  - Projector dropdown lists named displays.
  - Test button fires the test pattern on the selected projector
    only.
  - "New" / "Open recent" / "Try a demo" all transition into
    `Editing` correctly.
  - Launcher does not appear when `--autostart` is set.
- **Owner.** RUST + DES.

### WP-4. Bundled demo project *(P0, Phase 2)*

- **Problem.** No first-success template exists.
- **Outcome.** A user clicks "Try a demo" and sees a real photo
  on a real wall in seconds.
- **Implementation notes.** Add `assets/demos/window-glow.rmap.json`
  + the photo asset (license-clean, original or CC0). Use
  Image-layer support from spec 002. Two more demos
  ("Slow film strip", "Test grid") deferred to P1.
- **Dependencies.** Spec 002 image-layer landed; license-cleared
  assets.
- **Done means:**
  - Loading the demo from a fresh state renders a complete photo
    on the projector with all four warp corners visible and
    draggable.
  - Removing the demo asset on disk surfaces a clear toast (audit
    finding) instead of silent failure.
- **Owner.** PO + DES + RUST.

### WP-5. Empty-state hints on canvas *(P1, Phase 2)*

- **Problem.** Empty canvas shows a developer-log line.
- **Outcome.** Empty canvas shows a friendly drop hint.
- **Implementation notes.** Repaint a soft pulsing dashed
  rectangle with copy: *"Drop a photo or SVG here to begin."*
  Hint dismisses on first layer add. Replace the
  `(scene preview not yet registered…)` literal with a
  *"Connecting to projector…"* state, escalating to a
  *"Couldn't reach the projector"* toast after 5s.
- **Dependencies.** WP-1.
- **Done means:**
  - First-launch with no project shows the drop hint immediately.
  - Hint disappears on first successful layer add.
  - The dev-log line is gone from user-visible UI.
- **Owner.** RUST + DES.

### WP-6. Canvas merge (Scene + Mapping + Layers → Canvas) *(P1, Phase 3)*

- **Problem.** Mapping happens on a placeholder thumbnail
  disconnected from the live image.
- **Outcome.** One canvas, all editing happens here.
- **Implementation notes.** Promote v2's Scene-tab preview to the
  full control-window centre. Add a `Warp` mode toggle on the
  toolbar; warp corners are handles painted on the live preview
  in Warp mode. Layers strip on left edge (thumbnails per layer
  + `+` tile). Selection-driven inspector on the right.
  Decouple from old Mapping tab; remove the 480×270 checker
  placeholder canvas entirely.
- **Dependencies.** WP-1, WP-2, WP-7 (so leftover Mapping
  controls — mesh rows/cols, mask feather — have a home in
  Advanced).
- **Done means:**
  - The old `ControlTab::Mapping` arm is deleted.
  - Warp corners can be dragged directly on the live image.
  - All warp grid and mask polygon editing happens on the canvas.
  - Mesh rows/cols and mask feather live exclusively in Advanced.
- **Owner.** RUST + DES.

### WP-7. Advanced disclosure *(P1, Phase 3)*

- **Problem.** Power-user controls share the default surface.
- **Outcome.** One labelled door for all advanced controls.
- **Implementation notes.** A toolbar button toggles
  `ControlPanelState.advanced_open: bool`. When on, an Advanced
  panel renders to the right (or as an overlay drawer) with:
  - Master gamma / brightness / contrast.
  - Per-layer effect chain (Color, Tint, Blur, Transform, External).
  - Modulator types beyond Static.
  - Blend modes other than Normal.
  - Mesh rows/cols, mask feather, source-rect editing.
  - External-pass JSON params.
  - Project file path field; `output_windowed` flag (replaced by
    runtime hot-swap, so this becomes informational).
- **Dependencies.** WP-1, WP-2.
- **Done means:**
  - Default surface contains 0 advanced controls.
  - Every today-visible control has a destination either on
    canvas, in show-day strip, or in Advanced.
  - Power users (Sami) can complete any v2 task entirely within
    Advanced.
- **Owner.** RUST.

### WP-8. In-context glossary *(P1, Phase 3)*

- **Problem.** Domain terms appear without context.
- **Outcome.** Each advanced label has a `?` popover with a
  one-line plain-English explanation.
- **Implementation notes.** `glossary_label(ui, term, body)`
  primitive (Section 11.9). Glossary content lives in
  `assets/glossary.json` (or in code as `pub const`) — at minimum
  entries for: warp, mask polygon, modulator, gamma, brightness,
  contrast, blend mode, crossfade, scene, source rect, zone
  template, blackout, freeze, test pattern, editor overlay.
- **Dependencies.** Phase 0 produced glossary v0.
- **Done means:**
  - Every advanced label has a `?` icon or hover popover.
  - Glossary entries are 1–2 sentences; reviewed by design.
  - Adding a new term is one entry in the glossary source.
- **Owner.** PO + DES (content) + RUST (primitive).

### WP-9. Drag-drop + native file picker *(P1, Phase 2)*

- **Problem.** Layer addition is a typed file path.
- **Outcome.** Drag a JPG/PNG/SVG onto the canvas, or click `+`
  to open a native file picker.
- **Implementation notes.** winit already exposes drop events;
  v2 mentions drop-to-add. Wire it through `Command::AddLayer`.
  For the file picker, use the `rfd` crate (Rust File Dialog;
  ~zero-dep, native on macOS).
- **Dependencies.** WP-2.
- **Done means:**
  - Dragging a JPG/PNG/SVG onto the canvas adds a layer.
  - Clicking `+` on the layers strip opens a native file picker.
  - The typed-path text field in `Layers` tab is removed.
  - Unsupported file types surface a friendly toast, not a
    cryptic error.
- **Owner.** RUST.

### WP-10. Show-day strip *(P1, Phase 3)*

- **Problem.** B/F/T/O hotkeys are undocumented in the UI.
- **Outcome.** Four large always-visible buttons mirror the keys.
- **Implementation notes.** Bottom-of-window strip with four
  buttons: **Blackout** *(B)*, **Freeze** *(F)*, **Test** *(T)*,
  **Outlines** *(O)*. Each emits the corresponding `Command`;
  hotkeys remain as accelerators. Visual state (active/inactive)
  reflects current `OutputState`.
- **Dependencies.** WP-2 (commands), WP-6 (canvas layout).
- **Done means:**
  - All four buttons visible in editing mode and Go-live mode.
  - Click parity with hotkeys.
  - Each button shows its key in a small badge.
- **Owner.** RUST + DES.

### WP-11. Visual scene picker *(P2, Phase 4)*

- **Problem.** Scene slots are 1–9 with no thumbnails.
- **Outcome.** Bottom film strip of scene thumbnails.
- **Implementation notes.** Capture a small (e.g., 192×108)
  thumbnail of the warp_rt at scene-save time. Display in the
  bottom strip. Click thumb → `Command::RecallScene`. Drag
  current canvas onto `+` tile → `Command::SaveScene`. Hotkeys
  1–9 unchanged.
- **Dependencies.** WP-2.
- **Done means:**
  - Saving a scene captures a thumbnail.
  - Recall by clicking a thumb.
  - The 1–9 hotkeys still work.
  - Crossfade behaviour unchanged.
- **Owner.** RUST.

### WP-12. Autosave + Save As *(P2, Phase 4)*

- **Problem.** Save requires typing `*.rmap.json`.
- **Outcome.** Continuous autosave; named projects via *Save As*.
- **Implementation notes.** Autosave to
  `~/Documents/rmap/_autosave/<session>.rmap.json` every N seconds
  (default 5) when dirty. *Save As* uses `rfd` to pick a
  destination and copies the autosave to that path. "Open recent"
  reads from `~/Documents/rmap/`. The `Project file` text-input
  panel is removed.
- **Dependencies.** WP-1, WP-2.
- **Done means:**
  - User never has to know the `.rmap.json` extension.
  - Closing and reopening rmap recovers in-progress work.
  - "Open recent" lists named projects with thumbnails.
- **Owner.** RUST.

### WP-13. Live monitor names + projector test *(P1, Phase 2)*

- **Problem.** Monitor 0 / 1 / 2 is meaningless to a beginner.
- **Outcome.** Monitor names like *"BenQ TH685 — Living Room
  Wall"* in the launcher and Advanced.
- **Implementation notes.** macOS NSScreen display name via
  existing `objc2-app-kit`. Fallback to "Display N" on other
  platforms. The "Test" button fires the existing test pattern
  for 5s on the chosen monitor using a brief windowed
  `OutputWindow` instance, then returns to launcher.
- **Dependencies.** WP-3.
- **Done means:**
  - Launcher shows human names.
  - Test button works without leaving launcher state.
  - Reasonable fallback when no name is available.
- **Owner.** RUST.

### WP-14. Theme polish + iPad-like motion *(P2, Phase 4)*

- **Problem.** Today's mix of mustard handles, blue mesh, red
  errors is visually noisy.
- **Outcome.** One calmer dark theme; one warm accent; subtle
  spring-eased drag.
- **Implementation notes.** Centralise colour constants in
  `windows/theme.rs`. Apply egui style at app construction. Use
  egui's animation helpers for handle hover/active. Keep
  contrast WCAG AA on text.
- **Dependencies.** WP-6, WP-7 (so we don't restyle twice).
- **Done means:**
  - All hard-coded colours go through `theme::Color::*`.
  - Active handles use a single accent.
  - Hover/drag states animate smoothly without jank.
- **Owner.** DES + RUST.

### WP-15. Project audit + toasts *(P0, Phase 1, ~easier than estimated)*

- **Problem.** Saved projects can render to nothing with no
  warning. The catastrophic failure mode caught during the audit.
- **Outcome.** Every load surfaces a clear toast on first frame.
- **Implementation notes.** See 11.4–11.5. `ProjectAudit` runs
  after `Project::load → migrate` (existing pipeline already
  exposes a clean post-migration hook in `project/mod.rs`).
  Findings push to a `ToastQueue`. The first-frame render reads
  the queue and paints toasts in the canvas top-right. The
  existing `tests/golden/` infrastructure plus a few JSON-fixture
  unit tests is enough — no new test harness required.
- **Dependencies.** WP-1.
- **Done means:**
  - Loading the existing `~/p1.rmap.json` (with `scale = [0, 0]`)
    surfaces a clear *"Layer 0 has zero scale. [Auto-fix]"* toast.
  - Auto-fix button restores scale `[1, 1]`.
  - A `Critical` finding (e.g., asset missing on disk) routes to
    `AppState::Failed` rather than entering `Editing` with
    broken state.
  - Audit findings are unit-tested (≥ 6 cases) using JSON
    fixtures; no GPU device required for the audit tests.
- **Owner.** RUST + PO (copy review).

### WP-16. Native menu bar (macOS) *(P2, Phase 4)*

- **Problem.** No File / Edit / Window menus; no Cmd-S, no
  Cmd-Q, no `Hide rmap`.
- **Outcome.** Standard macOS menu items wire to commands.
- **Implementation notes.** winit on macOS exposes the NSApp's
  default menu structure; a small `objc2-app-kit` shim adds File
  → New / Open / Save / Save As; Edit → Undo / Redo / Cut /
  Copy / Paste; Window → Zoom / Minimise; Help → Glossary; About.
- **Dependencies.** WP-2 (commands).
- **Done means:**
  - Cmd-Z, Cmd-S, Cmd-O all work via menu and keyboard.
  - "About rmap" shows a real about box (version, license).
- **Owner.** RUST.

### WP-17. Telemetry hooks *(P0 hooks / P2 dashboards, Phases 1 + 5)*

- **Problem.** No measurement of Section 5 metrics.
- **Outcome.** Local-file metrics for time-to-first-pixel and
  similar.
- **Implementation notes.** See 11.12. Daily rolling
  `ux_metrics_<date>.json`. No network upload.
- **Dependencies.** WP-2 (commands are the natural emit sites).
- **Done means:**
  - Each Section 5 metric has at least one tracing event behind
    it.
  - A simple `cargo run --bin metrics-summary` (or similar)
    aggregates metrics for the dogfooding cycle.
- **Owner.** RUST.

---

## 13. Design-to-engineering handoff requirements

Required artifacts before engineering starts each phase.

### 13.1 Required for Phase 1

- **Decision register.** Section 14 questions answered.
- **`Command` taxonomy.** Initial enum sketch reviewed by RUST.
- **Audit-finding catalogue.** Initial list (WP-15) reviewed by
  RUST and PO.

### 13.2 Required for Phase 2

- **Launcher wireframe** with three start options, projector
  picker, test button.
- **Demo project storyboard** — the user's perspective minute by
  minute.
- **Empty-state copy** for canvas, looks-folder, projector-not-
  found.
- **Glossary v0** — at least 8 entries.

### 13.3 Required for Phase 3

- **Canvas wireframe** showing layers strip, toolbar, inspector,
  cue strip, show-day strip.
- **Advanced panel wireframe** showing all advanced controls
  organised into sections.
- **Mode banner copy** for Layer / Warp / Mask modes.
- **Interaction spec** for warp-corner snapping, mask-vertex
  insertion/deletion, and selection model.

### 13.4 Required for Phase 4

- **Theme tokens** (background, surface, text, accent, warning,
  destructive) with hex values and rationale.
- **Animation spec** (timings, easings) for handle hover, drag,
  scene transition.
- **Native menu structure** mapped to commands.

### 13.5 Cross-phase

- **State definitions catalog.** For each `AppState` and
  `EditMode`: empty, loading, error, success, transition states.
- **Event taxonomy.** All `Command` variants × all UI
  emit-points. Used to drive QA scripts.
- **Instrumentation plan.** Mapping from Section 5 metrics to
  `tracing` spans.
- **QA checklist.** Per phase, one document; each WP's
  acceptance criteria is the seed.

---

## 14. Risks and open questions

### 14.1 Risks (with mitigation)

| # | Risk | Mitigation |
|---|------|------------|
| R1 | **Scope bloat.** Plan grows during Phase 0. | Strict P0 vs P2 tier policing. Section 4.2 non-goals enforced. |
| R2 | **Power-user regression.** Sami / Marco can't find advanced controls. | Advanced disclosure must be feature-complete before Phase 3 closes; release notes call out new locations; telemetry on `advanced_opened`. |
| R3 | **Architectural mismatch.** Rewriting the mutation pathway is invasive. | Phase 1 commits to it before any visible UX work. WP-2 is the single largest WP and is the gating step for everything else. |
| R4 | **Hidden complexity in projection workflows.** Multi-warp, modulator-bound parameters, scene crossfade, panic recovery — all must keep working. | Phase 5 dogfooding includes a real show-day rehearsal. |
| R5 | **Insufficient usability validation.** Internal team tests only on themselves. | Phase 5 includes ≥ 5 external testers (event volunteers, AV teachers). |
| R6 | **Demo asset licensing.** Bundled photos must be license-clean. | Phase 0 produces an assets-licensing register. |
| R7 | **macOS-only assumptions creeping in.** `objc2`-only code paths grow. | Each macOS-gated module must compile on Linux/Windows with a stub. |
| R8 | **Hot-swap windowed↔fullscreen instability.** Surface re-creation can panic. | WP-3 uses existing `catch_unwind` recovery; failure routes to `AppState::Failed`. |
| R9 | **In-context glossary becomes a content-debt.** Terms added without entries. | Lint: every `glossary_label(ui, term, …)` requires `term` to be a known key; missing keys fail compile. |
| R10 | **Telemetry leaks user content.** Filenames, project names, asset paths logged. | Strict policy: telemetry only stores command kind + duration, never payload. PR template requires a privacy review checkbox. |
| R11 | **Reverse-storage subtleties produce silent undo corruption.** `Modulator` enum-variant replacement, `mutate_transform_effect`'s create-or-update semantics, and whole-project `restore_scene` snapshots each need bespoke Reverse logic. Naive per-field Reverse would corrupt undo state in ways the team would only catch in week 4. | Codify the three Reverse-storage rules from Section 11.2 at the type level (constructors that *force* full-enum / full-effects-Vec / full-snapshot capture). WP-2 ships with a property test: any sequence of commands + matching undos returns the project to a byte-equal `serde_json::Value`. **Promote to highest single technical risk in Phase 1.** |

### 14.2 Decision register (resolved Phase 0)

All ten open questions resolved through stakeholder review. Every
recommendation accepted; no overrides. This section is now the
authoritative decision record.

| # | Question | Decision | Rationale |
|---|----------|----------|-----------|
| Q1 | Launcher / editor binary architecture | **One binary, internal `AppState`.** | Simpler distribution; shared wgpu device; one `.app`. The launcher is `AppState::Launcher`; editor is `AppState::Editing`/`GoLive`. |
| Q2 | First-run experience | **Skippable, default-recommended.** | Launcher highlights "Try a demo" on first launch; one-click Skip goes to blank canvas. Welcomes Eva; respects Marco/Sami. |
| Q3 | Default autosave location | **`~/Documents/rmap/_autosave/`**, named projects alongside in `~/Documents/rmap/`. | Mac convention; user-discoverable in Finder; Time Machine + iCloud-Drive friendly. |
| Q4 | Launcher remembers last-used projector | **Yes, in `~/Library/Preferences/rmap.toml`.** | Stable display identifier (NSScreen UUID where available, else display name). Graceful fallback if that display is gone. |
| Q5 | Demo project count at GA | **One: "Window glow."** Two more ("Slow film strip", "Test grid") fast-follow in v3.1. | Cheapest viable path to first success; v3.1 unblocks once asset licensing for additional photos clears. |
| Q6 | Undo scope | **App-wide.** | Cmd-Z reverses any `Command` regardless of origin. Single mental model. Aligns with WP-2's single-mutation-pathway invariant. |
| Q7 | Native macOS menu bar timing | **v3 (Phase 4 / P2 polish).** | Free win once `Command` exists. Cmd-S, Cmd-Z, Cmd-O, About box, Help → Glossary. Doesn't block the canvas merge. |
| Q8 | Telemetry posture | **Always-on, local-file only.** | Daily-rolling JSON in `~/Library/Logs/rmap/`. No network, no payload, no user identification. Single off-switch in Advanced. |
| Q9 | Cross-platform parity at GA | **macOS-first GA; Linux/Windows best-effort.** | Matches today's posture. macOS gets launcher + named monitors + native menu + sleep assertion; Linux/Windows compile, no GA promises. |
| Q10 | "Go live" success definition | **Fullscreen on chosen projector + show-day strip visible + display-sleep assertion held.** | All three required to pass the Go-live gate. Removes ambiguity for QA; preserves operator-safety guarantees from v1. |

**Implications locked in by these decisions:**

- WP-2 (Command pattern) covers **all** mutation sources, including
  Advanced-panel widgets, since undo is app-wide (Q6).
- WP-3 (Launcher) and WP-12 (Autosave) target macOS first; Linux
  and Windows fallbacks are stubs (Q9).
- WP-4 ships *one* demo asset at GA; the asset register and
  licensing work is one item, not three (Q5).
- WP-13 (monitor names) requires a stable display identifier API
  on macOS — `objc2-app-kit` exposes NSScreen UUID; fall back to
  display name where unavailable (Q4).
- WP-15 (project audit) gates `AppState::Editing` entry — a
  `Critical` finding routes to `AppState::Failed` with an
  actionable toast (Q10's "no silent failures" principle).
- WP-16 (native menu) is in scope for Phase 4, not deferred (Q7).
- WP-17 (telemetry) ships from Phase 1, not Phase 5 (Q8 + Q6:
  commands are the natural emit sites).

---

## 15. Validation strategy

### 15.1 Per-phase validation gates

| Phase | Gate |
|-------|------|
| Phase 0 | Decision register signed off. |
| Phase 1 | Property-based undo/redo tests green. Project audit unit tests cover ≥ 6 finding kinds. |
| Phase 2 | An internal team member who has never used rmap reaches "photo on a wall" in ≤ 120s in 3 of 3 attempts. |
| Phase 3 | Canonical 7-step flow completes on the new IA without docs. Sami can complete every v2 task entirely within Advanced. |
| Phase 4 | Theme + motion polish reviewed by design lead. Native menu accelerators all work. |
| Phase 5 | External usability test n ≥ 5: ≥ 80% complete the canonical flow unaided. Show-day rehearsal completes without panic recovery firing. |

### 15.2 Validation methods

- **Prototype validation (Phase 0).** Wireframes shown to 2–3
  representative users (Eva-type) for "tell me what you'd click
  first."
- **Internal dogfooding (Phase 1–4).** Every phase ships behind a
  feature flag; the team uses it on real projects between phases.
- **Usability testing (Phase 5).** Recruited testers with
  recorded sessions; success measured against Section 5 metrics.
- **Design QA (Phase 4–5).** Designer walks every screen state
  against the wireframes.
- **Telemetry-based post-release validation (post-GA).** Local
  metrics aggregated across the team's own usage; if Section 5
  targets aren't met, a v3.1 hotfix cycle is triggered.
- **Show-day reliability gate.** A full rehearsal with all four
  show-day buttons exercised and a deliberate panic injected to
  verify recovery still works.

---

## 16. Milestones and release readiness

### 16.1 Milestone checkpoints

| Milestone | Trigger | Exit criteria |
|-----------|---------|---------------|
| **M0** | Phase 0 done | Decisions signed; wireframes approved. |
| **M1** | Phase 1 done | Architecture rails (state machine + commands + audit + telemetry) shipped behind a feature flag; old UI still default. |
| **M2** | Phase 2 done | Launcher + demo + drag-drop usable end-to-end; alpha. |
| **M3** | Phase 3 done | Canvas merge + Advanced + glossary + show-day strip; **internal beta**. |
| **M4** | Phase 4 done | Polish: scene picker, autosave, monitor names, theme; **external beta**. |
| **M5** | Phase 5 done | Validation gates green; **GA**. |

### 16.2 Partial-release criteria (alpha → beta → GA)

| Stage | Definition | Gate |
|-------|------------|------|
| **Alpha** | Internal-only, feature-flagged behind a CLI switch (`--v3`). | M2 reached; smoke-test passes; old UI still default in `main`. |
| **Internal beta** | Default UI on `main`; old UI removable. | M3 reached; team uses it for a real project end-to-end. |
| **External beta** | Tagged release (`v0.3.0-beta`); shared with willing testers. | M4 reached; ≥ 1 week of internal beta with no P0 regressions. |
| **GA** | Tagged release (`v0.3.0`); README rewritten. | M5 reached; Section 5 targets met or explicit deferral; show-day rehearsal green. |

### 16.3 GA-readiness checklist

- [ ] All P0 + P1 work packages: acceptance criteria green.
- [ ] Section 5 product, UX, engineering metrics: measured and
      meeting target (or explicit deferral).
- [ ] Property test for undo/redo: green.
- [ ] Project audit: ≥ 6 finding kinds covered with auto-fixes.
- [ ] Headless command-driven test harness: covers the canonical
      7-step flow.
- [ ] External usability test: n ≥ 5; ≥ 80% complete unaided.
- [ ] Show-day rehearsal: complete with panic injection.
- [ ] README rewrite: launcher first, CLI deferred to a power-user
      section.
- [ ] CHANGELOG: documents every breaking IA change with
      migration notes.
- [ ] Glossary v1: every advanced term has an entry; reviewed by
      design.
- [ ] Telemetry privacy review: passes (no payload, no network).
- [ ] Asset license register: clean.

---

## 17. Immediate next actions

### 17.1 This week

- [x] **PO.** Section 14.2 decision register resolved. *(All ten
      questions answered; recommendations accepted as-is.)*
- [ ] **PO.** Circulate the resolved decision register to design
      and engineering; confirm no late objections.
- [ ] **DES.** Sketch launcher + canvas + Advanced wireframes
      (low-fi, paper-grade is fine). Bake the Q1/Q2/Q4 decisions
      into the launcher wireframe (one binary, skippable
      first-run, remember-last-projector).
- [ ] **DES.** Draft glossary v0: 1–2 sentences for *warp, mask
      polygon, modulator, gamma, brightness, contrast, blend
      mode, crossfade, scene*.
- [ ] **RUST.** Review WP-1 (AppState) and WP-2 (Command) plans
      against `app.rs` and `controls/mod.rs`; flag any sites
      that are harder to refactor than estimated.
- [ ] **RUST.** Audit `Project::load → migrate` flow; sketch
      `ProjectAudit` API surface.
- [ ] **PO + Legal.** Identify and license-clear one candidate
      asset for the *Window glow* demo (Q5: one demo at GA).

### 17.2 Before engineering starts (Phase 0 exit)

- [ ] Decision register: all 10 questions in Section 14 answered.
- [ ] Wireframes: launcher + canvas + Advanced approved.
- [ ] Glossary v0: at least 8 entries reviewed.
- [ ] Demo asset secured (file + license + license file).
- [ ] Test harness skeleton for headless command-driven tests
      stubbed in `tests/`.

### 17.3 First implementation sprint (Phase 1, week 1)

Sequence the week so the Reverse-storage architecture lands
*before* command volume — otherwise R11 bites.

- [ ] **Day 1.** Land `AppState` enum (WP-1, smallest viable PR).
      Verify macOS suspend/resume guard handles all four
      "running" states. Verify `--list-monitors` and `--autostart`
      still work.
- [ ] **Day 1–2.** Decompose `init_running_app` (WP-1.1).
- [ ] **Day 2–3.** Rename `ControlEvent` → `Command`; codify the
      three Reverse-storage rules from Section 11.2 in the
      `Command` type (constructors that force full-enum / full-
      effects-Vec / full-snapshot capture; compile errors on
      partial Reverse). Do not migrate any widgets yet.
- [ ] **Day 3.** Land the `UndoStack` + the proptest harness
      (apply N commands → undo N → assert byte-equal
      `serde_json::Value`). The harness must pass on a single
      stub `Command::Noop` before any real variant ships.
- [ ] **Day 3–5.** Migrate the first ~10 mutation sites to
      commands, end-to-end, including undo/redo:
      - 3 always-visible: `gamma`, `brightness`, layer `opacity`.
      - 3 from `scene_editor`: drag-translate, mask-vertex drag,
        mask-vertex insert.
      - 1 enum-replacement smoke test: `Modulator` static → sine
        and back via undo.
      - 1 effects-Vec smoke test: drag-rotate on a layer with no
        Transform effect, then undo (the effect must be removed).
      - 1 snapshot smoke test: scene recall, then undo.
      - 1 exclusion smoke test: fire 5 file-watcher reloads,
        assert `UndoStack::len() == 0`.
- [ ] **Day 5.** Stand up `ProjectAudit` with the zero-scale
      finding only — wired to a minimal toast. Verify against
      the existing `~/p1.rmap.json` fixture.
- [ ] **Day 5.** Add `tracing` spans for `session_start`,
      `first_layer_added`, `project_audit_warned`,
      `undo_invoked`.
- [ ] **End of week.** Open the first PR labelled `v3-foundation`;
      gate behind `--features v3` so `main` is unaffected.
      Demo: launch with `--features v3`, drag a layer, hit Cmd-Z,
      load `p1.rmap.json`, see the audit toast.

---

## Appendix A — Cross-reference to source UX audit

| Audit item | Fulfilled by |
|------------|--------------|
| Audit Section A1 (CLI gate) | WP-3 |
| A2 (5 flat tabs) | WP-6, WP-7 |
| A3 (typed file path) | WP-9 |
| A4 (mapping divorced from image) | WP-6 |
| A5 (always-visible expert dials) | WP-7 |
| A6 (advanced concepts surface too early) | WP-7, WP-8 |
| A7 (output invisible until you map) | WP-3, WP-5 |
| A8 (no empty state) | WP-5 |
| A9 (typed `.rmap.json` save) | WP-12 |
| A10 (no first-success moment) | WP-3, WP-4 |
| Audit Section L (zero-scale silent invisibility) | WP-15 |
| Audit Section H (microcopy) | WP-5, WP-8 |
| Audit Section D7 (show-day strip) | WP-10 |
| Audit Section D9 (visual scene picker) | WP-11 |
| Audit Section D11 (undo/redo) | WP-2 |
| Audit Section D12 (live monitor names) | WP-13 |

## Appendix B — What stays exactly as it is

Per the audit's appendix, the following are deliberately **not**
touched:

- Two-window architecture (egui control + wgpu output).
- Project file format (`*.rmap.json`).
- Render pipeline (compositor, warp, mask SDF, effects, gamma).
- Keyboard shortcuts (B/F/T/O, scene 1–9 hotkeys).
- Audio/MIDI/OSC plumbing.
- Hot-reload, autostart, crossfade behaviour, panic recovery,
  display-sleep prevention, daily log rolling.
- Domain terminology (warp, mask polygon, modulator, gamma,
  blend mode, crossfade, scene, source rect).

Every architectural decision in v1/v2 survives. v3 is a UX +
foundation refactor, not a rewrite.
