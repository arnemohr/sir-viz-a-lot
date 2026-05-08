# 003 — Task Master Index

> Execution backlog for the rmap UI/UX overhaul.
> Source plan: `specs/003-ui-ux-overhaul-plan.md`.
> Source audit: `specs/003-ui-ux-overhaull.md`.
>
> This file is the *index*. Phase task files hold the actual work.
> Read this first to understand sequencing, dependencies, and the
> critical path; then open the phase file you are executing.

## File map

| File | Phase | Purpose |
|------|-------|---------|
| **`003-tasks.md`** *(this file)* | All | Index, dependency map, milestones, critical path, sprint candidates, decision log. |
| **`003-tasks-revision.md`** | All | **Revision pass after practitioner review.** Read this before executing any phase: triage matrix, change log, updated sequencing, decision-tasks D11–D14, post-revision first execution slice. |
| `003-tasks-phase-1.md` | Phase 1 | Architecture foundations (state machine, commands, undo, project audit, telemetry hooks). T1.14 rewritten; T1.36–T1.40 reprioritised; T1.38 extended. |
| `003-tasks-phase-2.md` | Phase 2 | First-run experience (launcher, demo, drag-drop, empty states, monitor names) **+ T2.23 + T2.24** (asset portability + missing-media relink). |
| `003-tasks-phase-3.md` | Phase 3 | Interaction overhaul (canvas merge, Advanced disclosure, glossary, show-day strip) **+ T3.28** (per-display gamma override). |
| `003-tasks-phase-4-5.md` | Phases 4 + 5 | Polish, native menu, autosave, scene picker, theme, validation, GA. **T4.16/T4.17 rewritten** (preview persists during GoLive). **T4.16a + T4.23** added. **T5.6 rescoped**; **T5.16** field beta added. T4.12/T4.13/T4.19 deferred. |

---

## 1. Breakdown strategy

### Why split the plan into five files

Phase 1 is the *only* phase that the others sequentially depend on
end-to-end. Phases 2 and 3 each represent a coherent product
release (alpha and internal beta respectively). Phases 4 and 5 are
small enough to share a file. A single monolith would obscure the
critical path and make sprint planning unwieldy; one task file per
spec (the existing `001-tasks.md` / `002-tasks.md` convention) is
not viable for a 16-week initiative.

### How tasks were derived

Each work package (WP-1 … WP-17) decomposed into the smallest unit
that:
- a single engineer can complete and review in 0.5–2 days,
- has explicit testable acceptance criteria,
- leaves the system in a *valid working state* on its own (no
  half-finished mutations of the live binary).

Mutation-migration tasks were grouped by **code location** (always-
visible bindings vs. effect chain vs. scene editor vs. dropped-file
path) so each PR touches one area and is reviewable in isolation.
The three Reverse-storage rules from the plan's Section 11.2 each
get a *named smoke-test task* so the test acts as the rule's
contract.

### Tasks discovered that were not explicit in the plan

Twenty-eight tasks were added during decomposition. Notable ones:

- Asset-licensing register (Phase 0).
- Glossary content authoring per term (Phase 1/3).
- macOS NSScreen display-name FFI shim (Phase 2).
- `Toast` struct + queue + egui primitive (Phase 1).
- `glossary_label`, `command_button`, `command_slider`,
  `mode_banner`, `drop_target`, `toast_strip` primitives.
- `--features v3` Cargo feature for staged rollout.
- macOS keyboard-accelerator conflict audit (Cmd-Z vs. existing 1–9
  hotkeys).
- `~/Documents/rmap/` and `~/Library/Preferences/rmap.toml`
  bootstrap.
- Hot-swap windowed↔fullscreen wgpu surface re-creation.
- Removal of the placeholder 480×270 mapping canvas + the old
  `ControlTab::Mapping` arm.
- `Selection::WarpCorner` arm wiring (currently `#[allow(dead_code)]`
  in `scene_editor.rs`) — needed for the canvas merge.
- README rewrite, CHANGELOG entry, migration notes from v2 IA.
- Telemetry privacy review checklist (R10 mitigation).
- A "demo asset license register" living next to the bundled asset.

See Section 9 for the full discovered list.

### Critical path

The single sequence that gates everything:

```
T1.1 (AppState enum)
  → T1.13 (rename ControlEvent → Command)
  → T1.14 (Reverse-storage type machinery)
  → T1.15 (UndoStack)
  → T1.17 (proptest harness on Command::Noop)
  → T1.18–T1.31 (mutation site migration, parallelisable in batches)
  → T1.34–T1.44 (ProjectAudit + Toast)
  → M1 (Phase 1 done)
  → T2.1 (Launcher window shell)
  → T2.10 (demo project loads end-to-end)
  → M2 (alpha)
  → T3.1 (canvas merge)
  → T3.10 (Advanced disclosure)
  → M3 (internal beta)
  → M4 (external beta)
  → M5 (GA)
```

`T1.14` (Reverse-storage type machinery) is the **single highest-
risk early task**: it must land before any mutation-site migration
or the team will rework all migrated commands when the rules are
discovered late. See Risk R11 in the plan.

### Parallel work opportunities

- **Design / content lane** runs continuously alongside engineering
  Phase 1: glossary v0, launcher wireframe, canvas wireframe,
  Advanced wireframe, microcopy passes.
- **Asset / licensing lane** (Phase 0 → Phase 2): "Window glow"
  demo photo sourcing and license clearance.
- **Engineering parallel batches** within Phase 1: once
  T1.13–T1.17 land, the ~15 mutation-migration tasks split across
  multiple engineers without conflict (they touch different code
  regions: always-visible vs. effect chain vs. scene editor).

### Assumptions

- A1. Single full-time Rust engineer carries the critical path;
  design and PO contribute ~20–30% to parallel lanes.
- A2. The 10 Phase-0 decisions in the plan's Section 14.2 are
  authoritative and not re-litigated mid-execution.
- A3. macOS is the primary target (Q9). Linux/Windows tasks marked
  `[mac-only]` accept stub fallbacks elsewhere.
- A4. Spec 002's image-layer support is landed and stable. WP-4
  (demo project) depends on it.
- A5. The team accepts feature-flag-gated rollout (`--features v3`)
  through Phase 1–3; `main` runs the v2 UI until M3.

---

## 2. Dependency and sequencing model

### High-level dependency graph

```
                       Phase 0 (mostly done)
                              │
                    ┌─────────┴──────────┐
                    │                    │
              wireframes            asset license
              glossary v0           clearance
                    │                    │
                    └─────────┬──────────┘
                              │
                       ─── Phase 1 ───
                              │
                       T1.1 AppState ──┐
                              │        │
                    T1.7-T1.12 init    │
                       decompose       │
                              │        │
                       T1.13 rename    │
                              │        │
                       T1.14 Reverse  ◄┘  ◄── R11 mitigation
                          machinery
                              │
                       T1.15 UndoStack
                              │
                       T1.17 proptest harness
                              │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
       T1.18-T1.20   T1.21-T1.23   T1.24-T1.31
       (always-vis)  (effects)     (interactions)
              │              │              │
              └──────────────┼──────────────┘
                             ▼
                    T1.34-T1.44 ProjectAudit
                             │
                    T1.45-T1.47 Telemetry hooks
                             │
                          ── M1 ──
                             │
                       ─── Phase 2 ───
                             │
                T2.1-T2.5 Launcher shell
                             │
                T2.6-T2.10 Demo project (parallel: T2.11 wireframe→spec)
                             │
                T2.11-T2.15 Drag-drop + file picker
                             │
                T2.16-T2.20 Empty states + monitor names
                             │
                          ── M2 (alpha) ──
                             │
                       ─── Phase 3 ───
                             │
                T3.1-T3.10 Canvas merge ── delete old Mapping/Layers tabs
                             │
                T3.11-T3.18 Advanced disclosure
                             │
                T3.19-T3.22 Glossary integration
                             │
                T3.23-T3.27 Show-day strip
                             │
                          ── M3 (internal beta) ──
                             │
                       ─── Phase 4 ───
                             │
            ┌────────┬──────┴──────┬────────┐
            ▼        ▼             ▼        ▼
         T4.1-     T4.6-         T4.11-   T4.16-
         T4.5      T4.10         T4.15    T4.20
         scene     autosave      monitor  theme +
         picker    + Save As     names UI native menu
            │        │             │        │
            └────────┴──────┬──────┴────────┘
                            │
                          ── M4 (external beta) ──
                            │
                       ─── Phase 5 ───
                            │
                T5.1-T5.5 dogfooding + show rehearsal
                            │
                T5.6-T5.10 external usability test
                            │
                T5.11-T5.15 README + CHANGELOG + release prep
                            │
                          ── M5 (GA) ──
```

### Recommended implementation order

Strict ordering of the critical path:

1. **Pre-Phase-1 readiness** (parallel to Phase 1 start): asset
   license, glossary v0, launcher wireframe.
2. **Phase 1, week 1, days 1–2**: T1.1 (AppState) → T1.7–T1.12
   (decompose `init_running_app`) → T1.13 (rename).
3. **Phase 1, week 1, days 3–4**: T1.14 (Reverse-storage type
   machinery) → T1.15 (UndoStack) → T1.17 (proptest harness).
   *Do not start mutation migrations until these land.*
4. **Phase 1, week 2**: T1.18–T1.31 (mutation-site migrations,
   parallelised). One engineer per batch where staffing permits.
5. **Phase 1, week 3**: T1.34–T1.44 (ProjectAudit + Toast),
   T1.45–T1.47 (telemetry hooks). M1 reached.
6. **Phase 2**: T2.* in order. Launcher first; demo project last
   (depends on launcher's "Try a demo" wiring).
7. **Phase 3**: T3.* in order. Canvas merge before Advanced
   disclosure (advanced needs a destination).
8. **Phase 4**: T4.* batches can run in parallel after M3.
9. **Phase 5**: T5.* in order; dogfooding before external testing.

### Sequencing mistakes that produce rework

| Mistake | Cost |
|---------|------|
| Migrating mutation sites (T1.18+) **before** T1.14 lands | All migrated commands likely use partial Reverse storage; full rework when R11 is rediscovered. **High.** |
| Shipping the canvas merge (T3.1+) **before** Advanced disclosure (T3.11+) is feature-complete | Power-user controls have no destination; Sami/Marco cannot use the build. **High.** |
| Bundling the demo asset (T2.6+) **before** image-layer support (spec 002) is verified stable | Demo fails on first user attempt; first-impression catastrophe. **High.** |
| Removing the old `ControlTab::Mapping` arm (T3.6) **before** all its mutations route through commands | Lost mutations: things that used to mutate via the Mapping tab simply stop working. **Medium.** |
| Adding telemetry payload data (T1.45+) instead of command-kind only | Privacy review fails; hotfix required. **Medium.** |
| Hot-swapping windowed↔fullscreen at runtime (T4.16) without `catch_unwind` recovery | Show-day panic. **Critical.** Must reuse v1's panic-recovery path. |
| Skipping macOS keyboard-accelerator audit (T4.18) | Cmd-Z conflicts with an existing hotkey discovered during dogfooding. **Low–Medium.** |

---

## 3. Summary table — all tasks

Tasks are listed by phase. Owner roles: **PO** = product, **DES**
= design, **RUST** = Rust engineer, **QA** = quality, **MIX** =
multiple. Estimated scope: **S** ≤ 0.5 day, **M** 0.5–2 days, **L**
> 2 days (split unless justified).

### Phase 0 — pre-Phase-1 readiness

| ID | Title | Owner | Scope | Parallel |
|----|-------|-------|-------|----------|
| T0.1 | Author glossary v0 (≥ 8 entries) | DES + PO | M | Yes |
| T0.2 | Source + license-clear "Window glow" demo asset | PO | M | Yes |
| T0.3 | Sketch launcher wireframe (low-fi) | DES | M | Yes |
| T0.4 | Sketch canvas wireframe (low-fi) | DES | M | Yes |
| T0.5 | Sketch Advanced disclosure wireframe | DES | M | Yes |
| T0.6 | Privacy review checklist for telemetry | PO | S | Yes |
| ✅ T0.7 | Add `--features v3` Cargo feature | RUST | S | Yes |
| T0.8 | Stub headless command-driven test harness skeleton | RUST | M | Yes |

### Phase 1 — architecture foundations

See `003-tasks-phase-1.md`. **47 tasks total.** Highlights:

| ID | Title | Owner | Scope |
|----|-------|-------|-------|
| T1.1 | Define `AppState` enum (5 variants) | RUST | M |
| T1.7–T1.12 | Decompose `init_running_app` | RUST | M ×6 |
| T1.13 | Rename `ControlEvent` → `Command` | RUST | S |
| T1.14 | Reverse-storage type machinery | RUST | L (justified) |
| T1.15 | UndoStack with `non_undoable` flag | RUST | M |
| T1.17 | Proptest harness on `Command::Noop` | RUST | M |
| T1.18–T1.31 | Migrate ~14 mutation-site batches | RUST | M ×14 |
| T1.34–T1.44 | ProjectAudit + Toast system | RUST + PO | M ×11 |
| T1.45–T1.47 | Telemetry hooks | RUST | M ×3 |

### Phase 2 — first-run experience

See `003-tasks-phase-2.md`. **22 tasks.** Highlights:

| ID | Title | Owner | Scope |
|----|-------|-------|-------|
| T2.1 | New `windows/launcher.rs` shell | RUST | M |
| T2.5 | Three start-buttons + projector picker | RUST | M |
| T2.7 | macOS NSScreen display-name FFI | RUST | M |
| T2.8 | Bundle "Window glow" demo project + asset | RUST + PO | M |
| T2.11 | Drag-and-drop on canvas (winit DroppedFile) | RUST | M |
| T2.13 | Native file picker via `rfd` | RUST | M |
| T2.16 | Canvas empty-state hint | RUST + DES | M |

### Phase 3 — interaction overhaul

See `003-tasks-phase-3.md`. **27 tasks.** Highlights:

| ID | Title | Owner | Scope |
|----|-------|-------|-------|
| T3.1 | Promote scene preview to full canvas | RUST | M |
| T3.5 | Wire `Selection::WarpCorner` direct manipulation | RUST | M |
| T3.6 | Remove `ControlTab::Mapping` arm + checker placeholder | RUST | S |
| T3.11 | Single Advanced disclosure panel | RUST | M |
| T3.19 | `glossary_label` egui primitive | RUST | M |
| T3.21 | Apply glossary entries to every advanced label | RUST + DES | M |
| T3.23 | Show-day strip with B/F/T/O buttons | RUST + DES | M |

### Phase 4 — polish + rationalisation

See `003-tasks-phase-4-5.md`. **22 tasks for Phase 4.** Highlights:

| ID | Title | Owner | Scope |
|----|-------|-------|-------|
| T4.1 | Visual scene picker thumbnails | RUST | M |
| T4.6 | Autosave timer + dirty tracking | RUST | M |
| T4.8 | `Save as…` via `rfd` | RUST | M |
| T4.16 | Hot-swap windowed↔fullscreen at runtime | RUST | M |
| T4.18 | macOS keyboard accelerator audit | RUST | S |
| T4.19 | Native menu bar (File / Edit / Window / Help) | RUST | M |

### Phase 5 — validation + release

See `003-tasks-phase-4-5.md`. **15 tasks for Phase 5.** Highlights:

| ID | Title | Owner | Scope |
|----|-------|-------|-------|
| T5.1 | Internal dogfooding (≥ 1 week, real project) | MIX | L (justified) |
| T5.4 | Show-day rehearsal with panic injection | RUST + QA | M |
| T5.6 | External usability test (n ≥ 5) | PO | L (justified) |
| T5.11 | README rewrite | PO + RUST | M |
| T5.13 | CHANGELOG + migration notes | PO | M |

**Total tasks across all phases: 147** (post-revision: +6 net new
tasks — T2.23, T2.24, T3.28, T4.16a, T4.23, T5.16; T4.12/T4.13/T4.19
deferred but kept in-file for v3.1 cross-reference). **Net
engineering days: ~0** (calendar shortened ~1 week by removing
T5.6 from the GA gate).

---

## 4. Dependency table

Only cross-phase dependencies are listed here. Within-phase
dependencies live in each phase file's task descriptions.

| Task | Depends on |
|------|------------|
| T1.* (any) | T0.7 (`--features v3`) |
| T1.18–T1.31 (mutation migrations) | T1.14 (Reverse type machinery), T1.15 (UndoStack), T1.17 (proptest) |
| T1.34 (ProjectAudit) | T1.1 (AppState — `Failed` state needed) |
| T2.1 (Launcher shell) | M1 (Phase 1 complete) |
| T2.5 (Projector picker) | T2.7 (NSScreen FFI) |
| T2.8 (Bundle demo) | T0.2 (asset cleared), spec 002 image-layer support |
| T3.1 (Canvas merge) | M2 (Phase 2 complete), all of T1.18–T1.31 (migrations) |
| T3.6 (delete old Mapping tab) | T3.1 (canvas merge), T3.11 (Advanced has destinations for old Mapping controls) |
| T3.21 (Apply glossary entries) | T0.1 (glossary v0 authored), T3.19 (`glossary_label` primitive) |
| T4.16 (Hot-swap windowed↔fullscreen) | T1.1 (AppState `GoLive`) |
| T4.19 (Native menu) | T1.13 (commands exist) |
| T5.1 (dogfooding) | M3 (internal beta) |
| T5.6 (external usability — **post-GA cycle**) | M5 reached |
| T5.16 (field beta — **NEW M5 gate**) | T5.4 (show rehearsal green) |
| GA (M5) | All P0 acceptance criteria green; P1 within-Phase-1 audits green or v3.1-tracked; T5.4 + T5.16 green |

---

## 5. Milestone table

| Milestone | Trigger | Hard exit criteria |
|-----------|---------|--------------------|
| **M0** | Phase 0 done | Decision register signed; wireframes approved; demo asset license-cleared. |
| **M1** | Phase 1 done | All P0 tasks (WP-1, WP-1.1, WP-2, WP-15, WP-17 hooks) acceptance criteria green. Proptest passes on the full `Command` enum. ProjectAudit covers ≥ 6 finding kinds. `--features v3` builds and ships behind the flag. |
| **M2** | Phase 2 done — **alpha** | Launcher launches; "Try a demo" reaches projected pixel in ≤ 30 s; drag-drop adds a layer; empty state replaces dev-log line. Old UI still default on `main`. |
| **M3** | Phase 3 done — **internal beta** | Canvas merge live; Advanced disclosure feature-complete; show-day strip visible; glossary popovers functional. Default UI on `main`. Old Mapping/Layers/Scenes tab arms deleted. |
| **M4** | Phase 4 done — **external beta** | Scene picker, autosave, native menu, theme polish, hot-swap windowed↔fullscreen. Tagged `v0.3.0-beta`. |
| **M5** | Phase 5 done — **GA** | Section-5 metrics measured and meeting target (or explicit deferral). **Show-day rehearsal green (T5.4) + practitioner field beta blockers fixed (T5.16)**. Capability roadmap (T4.23) published. README rewritten. Cross-machine portability + missing-media relink validated. Tagged `v0.3.0`. *(Note post-revision: original "n ≥ 5 external usability" gate moved to a post-GA validation cycle (T5.6).)* |

---

## 6. Critical path summary

```
T0.1 (glossary)  T0.2 (asset)  T0.3-T0.5 (wireframes)
       │                │              │
       └────────────────┴──────────────┘
                        │
                        ▼
T0.7 (features flag) ─► T1.1 (AppState)
                        │
              T1.7-T1.12 (init decompose)
                        │
                T1.13 (rename Command)
                        │
            T1.14 (Reverse machinery) ◄─ R11 gate
                        │
                T1.15 (UndoStack)
                        │
                T1.17 (proptest harness)
                        │
        ┌──────────────┼───────────────┐
        ▼              ▼               ▼
   T1.18-T1.20   T1.21-T1.23     T1.24-T1.31
        │              │               │
        └──────────────┼───────────────┘
                       │
                T1.34-T1.44 (audit + toast)
                       │
                T1.45-T1.47 (telemetry)
                       │
                     ── M1 ──
                       │
                  T2.1 (launcher shell)
                       │
                  T2.7 (NSScreen FFI)
                       │
                  T2.5 (projector picker)
                       │
                  T2.8 (bundle demo)
                       │
                  T2.11+T2.13 (drag-drop + picker)
                       │
                     ── M2 ──
                       │
                  T3.1 (canvas merge)
                       │
                  T3.11 (Advanced disclosure)
                       │
                  T3.6 (delete old Mapping)
                       │
                     ── M3 ──
                       │
                  T4.16 (hot-swap go-live)
                       │
                  T4.19 (native menu)
                       │
                     ── M4 ──
                       │
                  T5.1 (dogfooding)
                       │
                  T5.6 (external usability)
                       │
                  T5.11 (README rewrite)
                       │
                     ── M5 ──
```

The **chain of seven gates** that must each clear in order:
**T1.14 → T1.17 → T1.31 (last migration) → M1 → M2 → M3 → M4 → M5**.
Anything off this chain can be parallelised.

---

## 7. First sprint candidate set

For a 5-day first sprint with one full-time Rust engineer plus
parallel design/PO support. Maps to the plan's Section 17.3.

| Day | RUST | DES + PO |
|-----|------|----------|
| 1 | T0.7 (`--features v3`); T1.1 (AppState enum) | T0.1 (glossary v0); T0.3 (launcher wireframe) |
| 1–2 | T1.7–T1.12 (init decompose) | T0.2 (asset license clearance kicked off) |
| 2–3 | T1.13 (rename); T1.14 (Reverse machinery) | T0.4 (canvas wireframe) |
| 3 | T1.15 (UndoStack); T1.17 (proptest harness on `Command::Noop`) | T0.5 (Advanced wireframe) |
| 3–5 | T1.18 (always-visible bindings); T1.20 (scene_editor drag); T1.22 (Modulator picker — enum smoke); T1.30 (scene recall snapshot smoke); exclusion smoke (file watcher) | T0.6 (privacy checklist) |
| 5 | T1.34 (ProjectAudit zero-scale finding); T1.45 (`session_start` + 3 more spans) | Demo prep for sprint review |
| End | Open `v3-foundation` PR; live demo: launch with `--features v3`, drag a layer, Cmd-Z, load `p1.rmap.json`, see audit toast | First-sprint review |

Sprint demo proof-of-life: **5 things working at once** —
state machine routes correctly, command pattern emits + reverses,
property test green, ProjectAudit catches the zero-scale bug,
telemetry span fires.

---

## 8. High-risk early tasks

Tasks with disproportionate downstream cost if delayed or done
wrong. Watch these in the first two weeks.

| Task | Why high-risk | Mitigation |
|------|---------------|------------|
| **T1.14** Reverse-storage type machinery | R11 — naive Reverse corrupts undo; rework all migrations | Land *before* any T1.18+ migration. Constructors *force* full-enum / full-effects-Vec / full-snapshot capture at compile time. |
| **T1.17** Proptest harness | Without it, R11 corruption is invisible until week 4 | Land before mutation migrations begin. Stub on `Command::Noop` first; add variants as they ship. |
| **T1.30** Scene recall as `ApplyProjectSnapshot` | Crossfade tick fires ~60×/s; wrong here → undo stack overflow + glitches | Crossfade-tick commands flagged `non_undoable`. Tests cover both the snap path *and* the crossfade tick path. |
| **T2.7** NSScreen FFI | macOS-specific FFI via `objc2-app-kit`; fallible on display reconfiguration | Graceful fallback to `"Display N"`; never panics on an unplugged display. |
| **T2.8** Bundle demo project | First impression depends on it; broken demo = catastrophic first-use experience | Property test that `assets/demos/window-glow.rmap.json` loads cleanly through `ProjectAudit` and renders ≥ 1 visible pixel. |
| **T3.6** Delete old Mapping tab arm | Premature deletion silently drops mutations | Keep `ControlTab::Mapping` rendering through to T3.11; only delete after Advanced has destinations for all its controls. |
| **T4.16** Hot-swap windowed↔fullscreen | Surface re-creation can panic mid-show | Reuse v1's `catch_unwind` recovery; failure routes to `AppState::Failed` with a clear toast, not a crash. |

---

## 9. Missing tasks discovered during breakdown

The plan's work packages were specific but did not enumerate every
implementation step. The decomposition surfaced 28 additional
tasks. They are listed here so reviewers can confirm they are
real:

### Architecture / type-machinery

- T1.14 was implied; the plan said "Most commands store the
  previous value before applying" but did not specify type-level
  enforcement. Now an explicit task with compile-error contracts.
- T1.32: route output-state toggles (B/F/T/O) through `Command`
  for telemetry only (`non_undoable`).
- T1.33: explicit test that file-watcher hot-reloads do *not*
  enter the undo stack.

### UI primitives

- T3.19 `glossary_label`, T2.11 `drop_target`, T1.42 `toast_strip`,
  T3.27 `mode_banner`, T1.18 `command_button` / `command_slider` —
  the plan named them in Section 11.9 without scoping them.

### Content / authoring

- T0.1 glossary v0 is now a concrete deliverable with ≥ 8 entries
  named.
- T3.21 *applying* the glossary entries to every advanced label is
  separate from authoring them.

### Platform / FFI

- T2.7 NSScreen display-name FFI shim.
- T2.18 `~/Library/Preferences/rmap.toml` schema + read/write.
- T2.19 `~/Documents/rmap/` directory bootstrap.

### Cleanup / migration

- T3.6 explicit removal of the placeholder 480×270 mapping canvas
  + the `ControlTab::Mapping` arm + the typed-path Layers tab
  field.
- T3.7 wire `Selection::WarpCorner` (currently
  `#[allow(dead_code)]` in `scene_editor.rs:42`).
- T1.13 mechanical rename — plan implied it; surfacing as its own
  task makes the diff small and reviewable.

### Test infrastructure

- T0.8 headless command-driven test harness skeleton.
- T1.16 central `apply_command(state, cmd)` function (the plan
  mentioned it but didn't task it).
- T1.43, T2.21, T3.26 phase-specific test additions to the
  harness.

### Rollout

- T0.7 `--features v3` Cargo feature for staged rollout.
- T4.18 macOS keyboard-accelerator conflict audit.
- T5.11 README rewrite (Phase 5).
- T5.13 CHANGELOG entry + v2→v3 migration notes for users who
  knew the old IA.

---

## 10. Questions / blockers requiring human decision

The 10 plan-level decisions in Section 14.2 are resolved. Residual
decisions surfaced during breakdown:

| # | Decision | Owner | Blocking task |
|---|----------|-------|---------------|
| D1 | Glossary entry tone (warm vs. neutral)? Sample of one entry needed before authoring all eight. | DES + PO | T0.1 |
| D2 | Demo asset: original photograph commissioned, or CC0 from Unsplash? | PO | T0.2 |
| D3 | Native menu structure final layout — exact items under File / Edit / Window / Help? | DES + PO | T4.19 |
| D4 | Toast queue maximum visible (3? 5?) and duration (4 s? 6 s? sticky for warnings?) | DES | T1.41 |
| D5 | Theme accent colour: warm gold (today's mustard) preserved, or new accent? | DES | T4.20 |
| D6 | "Open recent" menu length (5? 10? unlimited?) | PO | T4.7 |
| D7 | When the user is mid-drag and clicks Cmd-Z, do we cancel the drag or undo the previous drag? | DES + RUST | T1.15 |
| D8 | External usability test: paid recruits vs. friends-of-team? Affects validity but also speed. | PO | T5.6 |
| D9 | Should `--features v3` flip to default-on at M3 or M4? | PO + RUST | T0.7 / M3 transition |
| D10 | Is windowed-output 1280×720 the right default for the demo, or should it match the projector's native resolution? | DES + RUST | T2.8 |

Open these with the relevant owners before the blocking tasks
start. None are critical-path blockers for the first sprint.

---

## 11. Required-task-category coverage check

The user's required categories, mapped to tasks. A category with
no task is justified inline.

| Category | Covered by |
|----------|------------|
| Architecture and enabling foundations | T1.1, T1.7–T1.12, T1.13–T1.17 |
| Discovery spikes / technical validation | T2.7 (NSScreen spike), T4.16 (hot-swap surface spike) |
| UX flow definition and design handoff | T0.3–T0.5, T2.6, T3.4 |
| Terminology and microcopy | T0.1, T3.21, all empty-state / error-message tasks |
| State model and mode handling | T1.1, T3.7 (`EditMode`) |
| Navigation / IA | T3.1, T3.6, T3.11 |
| First-run / onboarding | T2.1–T2.10 |
| Core workflow redesign | T3.1–T3.10 |
| Canvas / stage interaction | T3.1, T3.5, T3.7, T3.10 |
| Safeguards: undo, reset, snap, lock, validation | T1.14–T1.17, T1.34–T1.44, T3.18 (snap-to-edge) |
| Empty / loading / error / success states | T2.16–T2.20 |
| Telemetry / instrumentation | T1.45–T1.47, T5.7 (post-release validation) |
| Migration / compatibility | T1.34 (audit covers stale projects); T5.13 (v2→v3 migration notes) |
| Test coverage | T0.8, T1.17, T1.43, T2.21, T3.26, T5.4 |
| Design QA | T4.21 |
| Documentation | T5.11–T5.13 |
| Release readiness | T5.* |
| Post-implementation validation | T5.6–T5.10 |

All categories covered.

---

## 12. How to use this index

**Post-practitioner-revision update:** before executing any
phase, also read `003-tasks-revision.md`. It contains the change
log, the four new decision-tasks (D11–D14), and the revised
first execution slice.

1. **Sprint planning.** Read Section 7 (first sprint candidate set)
   for the immediate week, then cross-check against
   `003-tasks-revision.md` Section 8 for any post-revision
   updates. Section 6 (critical path) shows where slack lives.
2. **Issue creation.** Each task in the phase files copies cleanly
   into Linear / GitHub Issues / Jira; the template is consistent.
3. **Dependency checks.** Before starting any task, verify its
   `Dependencies` field in the phase file.
4. **Re-planning.** If a task slips, walk Section 4 to see what
   downstream work is blocked. M1, M2, M3 are hard gates.
5. **Risk monitoring.** Section 8 names the seven highest-risk
   tasks; surface them in retros and track until they ship.

For the actual task contents, open the relevant phase file:

- `003-tasks-phase-1.md` — start here once Phase 0 is M0-clear.
- `003-tasks-phase-2.md` — start after M1.
- `003-tasks-phase-3.md` — start after M2.
- `003-tasks-phase-4-5.md` — start after M3.
