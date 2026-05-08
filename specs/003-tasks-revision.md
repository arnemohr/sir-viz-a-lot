# 003 — Task Revision Pass (Practitioner Feedback Integration)

> Companion to `003-tasks.md` and the four phase files. **Read this
> first** before executing the revised backlog. Source of changes:
> the working-projection-mapper review of the original 003 plan +
> task package.

## 1. Revision summary

The practitioner review confirmed the original architecture work
is sound, but exposed three classes of gaps that materially change
the backlog:

### What changed

1. **Output preview during Go live is non-negotiable.** The original
   T4.16 / T4.17 fullscreened the output and let the operator
   "fly blind" — the practitioner flagged this as a real-show
   blocker. The control window must keep showing a live preview
   throughout `AppState::GoLive`. Now reflected in T4.16, T4.17,
   plus a new T4.16a (preview-as-projector pre-show mode).

2. **Asset portability is a real-world workflow gap.** Today
   project files store absolute asset paths (per the
   `~/p1.rmap.json` fixture). Wedding/small-event operators move
   between machines (laptop A dies, second laptop takes over). The
   original audit (WP-15) catches missing assets but doesn't fix
   the underlying portability story. Now: project-relative paths,
   "find missing media" relink toast, and explicit project-folder
   convention. New tasks T2.23, T2.24; T1.38 amended.

3. **External usability test is calendar-blocking the GA gate.**
   The original M5 gate required n ≥ 5 external tests. Recruitment
   takes weeks, and risks a single delay holding the entire
   release. Practitioner-correct: GA gates on **show-day rehearsal
   + dogfooding + a small-group field beta**. External usability
   test moves to a **post-GA validation cycle** that informs v3.1.
   Reflected in T5.6 (rescoped) and new T5.16 (field beta).

### Major gaps discovered

4. **Capability ceiling vs. positioning gap.** The plan implies
   "iPad-like projection mapping tool"; the codebase delivers
   "easiest free tool for projecting still images on one
   projector at a small event." Practitioner-honest scoping
   matters because it changes which capability deferrals are
   defensible. New decision-task **D11** (capability scope &
   v0.4 statement) makes this explicit. New T4.23 publishes a
   capability roadmap so users / contributors / contributors
   understand the trajectory.

4a. **Per-layer warp + mask architecture** *(post-Phase-2
    operator review).* The current render graph composites every
    layer first, then applies project-level warps to the
    composite. Operators expect each layer to map onto its own
    physical surface (photo on wall A, video on wall B, SVG
    overlay on the door). Without per-layer mapping the
    layer-thumbnail strip in T3.1+ (Phase 3) contradicts the
    actual rendering model: dragging a warp corner deforms
    every layer regardless of which thumbnail is selected. This
    is a structural change to the project schema, mutation
    surface, audit pass, and render graph — landed as new tasks
    **T3.0a–T3.0d** at the very front of Phase 3 (gating every
    other Phase 3 task). Schema bumps from v3 to v4 with
    migration. See **F17** below.

5. **Per-display gamma override** missing. Even single-projector
   setups need it (laptop monitor and projector colour spaces
   differ). Small Advanced-section addition; new T3.28.

6. **Practitioner field validation absent.** Internal dogfooding
   (T5.1) is engineering-focused. A wedding DJ / AV teacher
   field-using the tool on a real event is a different signal.
   New T5.16 adds this as a pre-GA gate.

### Assumptions confirmed by the review

- Operator-safety story (panic recovery, sleep prevention,
  blackout/freeze, `B/F/T/O` hotkeys) is best-in-class — preserve.
- Direct manipulation on canvas (v2 + WP-6 canvas merge) is the
  right interaction model.
- Launcher + demo + "test pattern from launcher" is genuinely
  better than every commercial tool the practitioner uses.
- Project audit toasts are unique and valuable (preserve, just
  reduce the P0 set).
- Glossary popovers are unique (keep).

### Assumptions invalidated by the review

- "iPad-like projection mapping tool" framing oversells what the
  product is. Reposition microcopy; the engineering doesn't change
  but the README / launcher copy / demo selection should.
- Compile-time-enforced Reverse-storage type machinery (T1.14) is
  excellent engineering but high-friction for future contributors.
  Soften to runtime `debug_assert!` + property test invariant.
  Same safety property; lower contributor cost.
- Audit findings (T1.35–T1.40) at six P0 items oversize Phase 1.
  Two are P0 (zero-scale, missing-asset); the other four are P1
  inside Phase 1, droppable to v3.1 if Phase 1 runs long.

### Deliberately not adopted

- **Video layer support inside 003.** Adding ffmpeg / `wgpu_video`
  dependencies, a new layer kind, decode threading, and codec
  testing would inflate Phase 1 from 3 weeks to ~8 weeks and
  re-architect the render graph. **Decision:** explicit scope
  statement that video is v0.4. Tracked as decision-task **D11**
  and roadmap commitment T4.23.

- **NDI / Syphon input.** Same reasoning: a real Phase scope.
  Decision-task **D12** captures the deferral and proposes a v0.4
  spike.

- **Edge-blend stub for two adjacent projectors.** Decision-task
  **D13**; deferred to v0.4 alongside multi-projector.

- **Live MIDI / OSC binding UI.** M7 plumbing exists; the binding
  UX is roadmap-deferred. Decision-task **D14** confirms.

- **Reposition entire spec as "wedding-only tool."** This is a
  positioning / marketing decision, not a backlog one. Reflected
  in T5.11 (README rewrite) and the capability roadmap (T4.23)
  but no spec rewrite.

### What this means for the implementation strategy

The original 16-week plan absorbs ~3–4 days of net change:
- ~1 day of T1.14 simplification offset by a slightly larger
  proptest harness (T1.17).
- ~1 day saved by deferring 4 audit findings.
- ~3 days added for asset portability (T2.23, T2.24, T1.38
  amendment).
- ~1 day added for output preview during Go live (T4.16
  amendment).
- ~1 day added for per-display gamma (T3.28).
- ~3 days added for the field beta (T5.16) — calendar time, not
  engineering.
- T5.6 moves out of GA gate, freeing ~5 calendar days.

**Net: zero engineering-week change; calendar shortened by ~1
week.** The release is *more* defensible because the GA gate is
practitioner-grounded (show rehearsal + field beta) rather than
academic-usability-grounded (n ≥ 5 lab sessions).

---

## 2. Feedback triage matrix

| # | Feedback item | Source concern | Assessment | Action | Why | Scope impact | Sequencing impact |
|---|---------------|----------------|-----------|--------|-----|--------------|-------------------|
| F1 | Output preview during Go live | "Operator can't see what the projector shows" | **Critical to incorporate now** | Amend T4.16, T4.17; add T4.16a | Show-day-blocking gap | +1 day Phase 4 | Must land before M4 |
| F2 | Asset portability + missing-media relink | Cross-machine workflow break | **Critical to incorporate now** | New T2.23, T2.24; amend T1.38 | Real-world wedding-DJ failover scenario | +3 days Phases 1+2 | Phase 2 cannot ship M2 without it |
| F3 | External usability test moved out of GA gate | Calendar blocker; weak GA signal | **Critical to incorporate now** | T5.6 rescoped; new T5.16 (field beta) | Faster, more relevant GA validation | +3 calendar days, −1 week from gate | M5 gate composition changes |
| F4 | Per-display gamma / brightness override | Mixed colour-space single-projector setups | **Important; phase 3** | New T3.28 | Cheap fix; high real-world value | +0.5 day Phase 3 | None |
| F5 | Compile-time Reverse-storage machinery friction | Future-contributor cost | **Important; soften now** | Amend T1.14 (debug_assert + invariant) | Same safety; lower onboarding cost | −1 day Phase 1 | None |
| F6 | 6 audit findings → 2 P0 | Over-engineering for wedding-scale audience | **Important; reduce now** | T1.35 + T1.38 stay P0; T1.36, T1.37, T1.39, T1.40 stay in Phase 1 but drop to "ship if slack" with v3.1 fallback | Right-size Phase 1; preserve safety net | −1 day Phase 1 if dropped | None |
| F7 | Capability scope statement (v0.4 forward plan) | Practitioner-honest expectations + contributor clarity | **Critical to incorporate now** | New T4.23 publishes roadmap; new decision-tasks D11–D14 | Avoids "iPad-like for projection mapping" overselling | +0.5 day Phase 4 | None |
| F8 | Drop native menu bar to v3.1 if Phase 4 slips | Phase 4 over-scoped | **Already covered**; mark T4.19 deferrable | Annotate T4.19 priority | Schedule resilience | 0 days, defer-only | None unless triggered |
| F9 | Schema v5 portable monitor deferred to v3.1 | Wedding operators don't move projects | **Defer** | Mark T4.12, T4.13 v3.1; remove from M4 gate | Real audience doesn't need it | −2 days Phase 4 | None |
| F10 | Video layer support | Largest capability gap practitioner cited | **Not adopted in 003**; decision-task D11; capability-roadmap T4.23 | v3 is still-image; v0.4 owns video | Out of scope by design (would 2.5×-Phase-1) | New v0.4 spec (post-003) | None |
| F11 | NDI / Syphon input layer | Integration with rest of practitioner rig | **Not adopted in 003**; decision-task D12 | v0.4 owns | Out of scope by design | New v0.4 spec | None |
| F12 | Edge-blend stub | 2-adjacent-projector real-world case | **Not adopted in 003**; decision-task D13 | v0.4 owns | Multi-projector renderer change | New v0.4 spec | None |
| F13 | OSC live parameter binding UI | M7 stubs exist; binding UX missing | **Not adopted in 003**; decision-task D14 | Roadmap-confirmed deferral | M7 plumbing already accounted for | None | None |
| F14 | "Reposition product as wedding tool" | "iPad-like projection mapping" overselling | **Partial**: capability-roadmap (T4.23) + README rewrite (T5.11) tone shift; no spec rewrite | Honest positioning is content work, not engineering | 0 days | None |
| F15 | Field beta before GA | Practitioner-grounded validation | **Critical to incorporate now** | New T5.16 | More relevant GA signal than n=5 lab tests | +3 calendar days; 0 eng days | M5 gate gains a step |
| F16 | "Find missing media" toast on Open Recent | Asset-portability adjacent | **Already covered by F2 fix** | T2.10 amended | Cheap; obvious | 0 incremental days | None |
| F17 | Per-layer warp + mask + effects (each layer mapped individually) | "Each layer should be warped and mapped individually" — current shared-warp model contradicts the per-layer thumbnail UX coming in Phase 3 | **Critical to incorporate now** | New T3.0a–T3.0d at the front of Phase 3 | Structural shift: layer thumbnails + warp-on-canvas only make sense once mapping is per-layer. Without it, T3.1+ ships a contradictory product. | +5–7 days Phase 3 (schema bump + render-graph rewrite + mutation rename + audit rename) | Phase 3 cannot ship M3 without it; T3.5 / T3.7 / T3.15 acceptance criteria changed |

**Triage totals:** 7 critical-now, 4 important-now-with-shape-change, 4 deferred-to-v0.4-or-v3.1, 2 already-covered-or-positioning-only.

---

## 3. Task-level change log

### Tasks added

| ID | Title | Phase | Reason |
|----|-------|-------|--------|
| T2.23 | Asset-portability spike: project-relative paths & embed policy | 2 | F2 — cross-machine portability |
| T2.24 | Missing-media relink flow with `rfd` "Find this file" | 2 | F2 + F16 |
| **T3.0a** | **Schema v4: per-layer `WarpMesh` + migration from v3 `Project.warps`** | **3** | **F17 — per-layer mapping** |
| **T3.0b** | **Render graph rewrite: per-layer warp pass + composite-of-warped-layers** | **3** | **F17** |
| **T3.0c** | **Mutation rename: `warp_idx` → `layer_idx` across all warp/mask variants** | **3** | **F17** |
| **T3.0d** | **Audit rename + multi-warp consolidation finding** | **3** | **F17** |
| T3.28 | Advanced > Selected output > per-display gamma + brightness override | 3 | F4 — mixed colour-space setups |
| T4.16a | Pre-show "Preview as projector" mode (windowed, scaled to projector aspect) | 4 | F1 + practitioner offsite-preview ask |
| T4.23 | Capability roadmap doc — v3 scope + v0.4 forward plan | 4 | F7 — practitioner-honest expectations |
| T5.16 | Practitioner field beta (1 wedding-DJ + 1 AV teacher minimum) | 5 | F15 — practitioner-grounded GA gate |

### Tasks rewritten

| ID | What changed | Reason |
|----|--------------|--------|
| T1.14 | Compile-time enforcement → runtime `debug_assert!` + property-test invariant; doc-comment rules; helper constructors `Project::current_*` | F5 — soften the contributor cost; preserve safety |
| T1.38 | Critical "missing asset" finding extended with relink autofix using new T2.24 flow | F2 + F16 |
| **T3.3** | **Inspector now shows the selected layer's warp grid + mask info; warp/mask edit affordances are per-layer** | **F17 — per-layer mapping** |
| **T3.5** | **`Command::SetLayerWarpCorner { layer_idx, … }` (was `SetWarpCorner { warp_idx, … }`); only the selected layer's grid is interactive in Warp mode** | **F17** |
| **T3.6** | **Per-layer mesh rows/cols + zone templates apply to the selected layer; T3.0c compatibility alias dropped here** | **F17** |
| **T3.7** | **`EditMode { Warp, Mask }` are scoped to the selected layer; banner copy "Select a layer first" when no layer selected** | **F17** |
| **T3.15** | **Mesh rows/cols + mask feather move to `Advanced > Selected layer > Mapping` (per-layer surfacing)** | **F17** |
| T4.16 | Hot-swap windowed↔fullscreen amended to **keep the control-window preview live** during GoLive; the operator never loses sight of the projector content | F1 — show-day-blocking |
| T4.17 | `Command::EnterGoLive` / `ExitGoLive` amended to confirm preview persistence across the transition; explicit acceptance criterion added | F1 |
| T5.6 | Rescoped from "5-tester external usability test as GA gate" to "post-GA validation pipeline that informs v3.1." Recruitment is post-tag, not pre-tag | F3 — calendar block + weak signal |
| T5.11 | README rewrite tone-shift: "easiest free tool for projecting photos at small events" rather than "iPad-like projection mapping tool" | F14 |

### Tasks reprioritised

| ID | From | To | Reason |
|----|------|-----|--------|
| T1.35 (zero-scale audit) | P0-must-ship | **P0-must-ship** (unchanged) | Headline failure mode |
| T1.38 (missing-asset audit) | P0-must-ship | **P0-must-ship** (unchanged) | F2 elevation |
| T1.36 (degenerate warp audit) | P0-must-ship | **P1-ship-if-slack-else-v3.1** | F6 — right-size Phase 1 |
| T1.37 (mask <3 vertices audit) | P0-must-ship | **P1-ship-if-slack-else-v3.1** | F6 |
| T1.39 (out-of-range monitor audit) | P0-must-ship | **P1-ship-if-slack-else-v3.1** | F6 |
| T1.40 (schema-too-new audit) | P0-must-ship | **P1-ship-if-slack-else-v3.1** | F6 |
| T4.12 (per-projector UUID schema v5) | M4 gate | **v3.1 backlog** | F9 — wedding operators don't move projects |
| T4.13 (output_monitor migration v5) | M4 gate | **v3.1 backlog** | F9 |
| T4.19 (native macOS menu bar) | M4 gate | **M4 gate, deferrable to v3.1 if Phase 4 slips** | F8 — schedule resilience |

### Tasks deferred from M5 GA gate (still in Phase 5 but post-tag)

| ID | Original gate | Revised gate | Reason |
|----|---------------|--------------|--------|
| T5.6 (external usability test, n ≥ 5) | M5 (GA) | post-GA validation cycle | F3 |

### Tasks added to M5 GA gate

| ID | Reason |
|----|--------|
| T5.16 (practitioner field beta) | F15 — show-day-grounded validation |

### Tasks unchanged

The Phase 1 architectural backbone (T1.1, T1.2, T1.3, T1.7–T1.13,
T1.15–T1.17 minus T1.14) is unchanged. T2.1–T2.22 (launcher, demo,
drag-drop, monitor names) untouched. T3.1–T3.27 (canvas merge,
Advanced disclosure, glossary, show-day strip) untouched. WP-12
autosave (T4.6–T4.10) untouched.

---

## 4. Updated sequencing model

### New critical path

```
T0.* (Phase 0 prep)
  → T1.1 → T1.7–T1.12 → T1.13
  → T1.14 (rewritten — runtime invariant, no compile-time gate)
  → T1.15 → T1.17
  → T1.18–T1.31 (mutation migrations)
  → T1.34 → T1.35 (zero-scale; P0)
  → T1.38 (missing-asset; P0)  ← extended for F2
  → T1.41–T1.44 (Toast + AppState::Failed routing)
  → M1
  → T2.1 (launcher) → T2.7 (NSScreen FFI) → T2.5 (picker) → T2.8 (demo)
  → T2.23 (asset-portability spike) ◄── NEW
  → T2.24 (missing-media relink flow) ◄── NEW
  → T2.10 (Open Recent uses T2.24 for missing media)
  → T2.11–T2.22
  → M2
  → T3.0a (schema v4: per-layer warp) ◄── NEW (gates Phase 3)
  → T3.0b (render graph rewrite: per-layer warp + composite) ◄── NEW
  → T3.0c (mutation rename warp_idx → layer_idx) ◄── NEW
  → T3.0d (audit rename + multi-warp consolidation finding) ◄── NEW
  → T3.1–T3.27 (canvas merge + Advanced + glossary + show-day strip)
  → T3.28 (per-display gamma override) ◄── NEW
  → M3
  → T4.16 (rewritten — preview persists in GoLive)
  → T4.16a (preview-as-projector pre-show mode) ◄── NEW
  → T4.17 (rewritten)
  → T4.23 (capability roadmap doc) ◄── NEW
  → (T4.12, T4.13 deferred to v3.1)
  → (T4.19 native menu — deferrable if slack)
  → M4
  → T5.1 (dogfooding) → T5.4 (show-day rehearsal) → T5.16 (field beta) ◄── NEW
  → T5.11 (README rewrite, tone-shifted)
  → T5.13–T5.15 (CHANGELOG, packaging, GA tag)
  → M5  (T5.6 moves OUT of M5 gate to post-GA cycle)
  → post-GA: T5.6 (external usability informs v3.1)
```

### What changed in the order

- **Phase 2 grows by two tasks** (T2.23, T2.24) which become a hard
  prerequisite for T2.10 (Open Recent). Without them, Open Recent
  can show projects that fail to load with no recovery path.
- **Phase 3 grows by five tasks** (T3.0a, T3.0b, T3.0c, T3.0d, T3.28).
  T3.0a–T3.0d are inserted at the **front** of Phase 3 — they gate
  every other Phase 3 task because the per-layer warp + mask data
  model + render graph is the prerequisite for the layer-thumbnail
  + warp-corners-on-canvas interaction model. T3.5 / T3.7 / T3.15
  acceptance criteria adjusted to consume the per-layer model.
- **Phase 4** has one rewritten task (T4.16), one new task (T4.16a),
  one new doc task (T4.23). Two original tasks (T4.12, T4.13)
  drop out to v3.1.
- **Phase 5 changes shape**: T5.6 leaves M5, T5.16 enters M5.

### Tasks newly blocked

| Task | Was unblocked after | Now blocked by |
|------|---------------------|----------------|
| T2.10 (Open Recent listing) | T2.4 | T2.4 + T2.24 (missing-media flow) |
| M2 declaration | T2.21 | T2.21 + T2.23 + T2.24 |
| T3.1 (canvas merge) | M2 | M2 + T3.0b (render graph rewrite) |
| T3.5 (warp-corner direct manipulation) | T3.4 | T3.4 + T3.0c (renamed mutation) |
| T3.7 (EditMode enum) | T3.1 | T3.1 + T3.0a (per-layer fields) |
| T3.15 (mesh + mask feather → Advanced) | T3.11 | T3.11 + T3.0a + T3.0c |
| M3 declaration | T3.27 + others | T3.0a + T3.0b + T3.0c + T3.0d + T3.27 + others |
| M4 declaration | T4.21 | T4.21 + T4.16a + T4.23 (roadmap doc) |
| M5 declaration | T5.6 + others | T5.16 + show-rehearsal; T5.6 OUT |

### Tasks that gained parallelism

- T2.23 (portability spike) can start immediately after M1, in
  parallel with T2.1 (launcher shell). Different module surfaces.
- T3.28 (per-display gamma) can run in parallel with T3.1–T3.27.
- T4.23 (capability roadmap doc) is a writing task; can run in
  parallel with all of Phase 4 engineering.
- T5.16 (field beta) is calendar-bound; engineering can continue
  on T5.10 (bug-fix cycle) in parallel.

### Sequencing risks to watch

| Risk | Mitigation |
|------|------------|
| T1.14 softening leaves a contributor PR with partial Reverse storage that the proptest doesn't catch | The runtime `debug_assert!` fires in test builds; proptest with ≥ 1024 cases catches the rest |
| T2.23 / T2.24 land *after* T2.10 (Open Recent) is wired, leaving a brief window where a missing-asset project crashes the recent-projects flow | Order T2.10 to consume T2.24's API; gate with a feature-flag during transitions |
| T4.16's "preview persists during GoLive" introduces a wgpu surface lifecycle subtlety (the offscreen warp_rt is sampled by both the projector full-screen surface AND the control-window egui texture) | Reuse the existing `register_native_texture` path from T-M9-01; verify no double-free on transition. **Post-T3.0b note:** the shared `warp_rt` is gone; the egui preview now binds to the projector RT view (post-warp, pre-gamma). The same `register_native_texture` helper applies, just to a different texture. |
| T3.0b render graph rewrite breaks the existing single-`warp_rt` invariant; any code path that assumes "all layers composite into one buffer before warp" silently produces wrong output | Land T3.0a + T3.0b + T3.0c + T3.0d behind a sub-flag (`v3-per-layer-warp`) until the golden-image suite passes; flip default once green. The headless GPU test `per_layer_warp_distinct_corners` (T3.0b acceptance #2) is the smoke test. |
| T3.0a v3 → v4 migration loses information when M > 1 warps existed in the original project | The `MultipleWarpsConsolidated` audit toast (T3.0d) tells the operator. The migration is intentionally lossy because preserving M > 1 warps in a per-layer-mapping model would require a heuristic ("which layer goes with which warp?") that's wrong as often as right. |
| T5.16 field beta produces show-stopping bugs that block GA | T5.16 happens after T5.4 show rehearsal; treat as "go / no-go" not "happy path." If field beta finds blockers, M5 slips and they are fixed via T5.10 |
| Capability roadmap (T4.23) sets expectations the team can't keep | Roadmap is a "v0.4 spec exists, scope is X" declaration, not a date promise |

---

## 5. Revised task files (pointers)

The actual task-content edits live in:

- **`003-tasks-phase-1.md`**: T1.14 rewritten; T1.36–T1.40 reprioritised; T1.38 extended.
- **`003-tasks-phase-2.md`**: T2.23, T2.24 added; T2.10, T2.21 amended.
- **`003-tasks-phase-3.md`**: T3.0a, T3.0b, T3.0c, T3.0d **added at the front** (gate every other Phase 3 task). T3.3, T3.5, T3.6, T3.7, T3.15 acceptance criteria amended for the per-layer model. T3.28 added.
- **`003-tasks-phase-4-5.md`**: T4.16, T4.17 rewritten; T4.16a, T4.23, T5.16 added; T4.12, T4.13 marked v3.1; T4.19 marked deferrable; T5.6 rescoped.
- **`003-tasks.md`**: index, milestone gates, summary table updated.

The edits have been applied. Diff highlights are reproduced in
Section 5b below for review without opening every file.

### 5b. Inlined task contents for the new / rewritten tasks

The full task templates for tasks added or rewritten in this
revision live in their respective phase files. For convenience,
the new tasks are reproduced here in summary form. Read the phase
files for the full templates.

#### T1.14 — rewritten (Phase 1)

**Reverse-storage runtime invariant + property-test contract**
(replacement for compile-time enforcement). Soften the original
"compile-error on missing Reverse" approach to:
- Doc-comment rules at the `Command` enum head with `#[deny(missing_docs)]`
  enforcement on every variant.
- Helper constructors `Project::current_modulator(...)`,
  `Project::current_layer_effects(...)`,
  `Project::current_snapshot()` so call sites read the old value
  inside the constructor — no caller can forget.
- `debug_assert!` in `Reversible::apply` that the stored old value
  matches the current project state pre-application (catches
  contributor errors in test builds).
- Property test (T1.17) is the runtime contract: any sequence of
  commands + matching undos returns the project to byte-equal
  serde_json. This *is* the safety guarantee.
- **Compile-time enforcement explicitly deferred to v3.1** as a
  refactor; mark as a v3.1 backlog item.

#### T1.38 — amended (Phase 1)

**Audit: missing asset on disk (Critical) — extended with relink
autofix.** Original task stays; the autofix gains a
`Command::RelinkAssetPath { layer_idx, new_path, old_path }`
variant. The autofix UI presents two options: "Find this file…"
(opens `rfd` file picker, see T2.24) or "Remove this layer."
Routes through the new T2.24 flow.

#### T2.23 — new (Phase 2)

**Asset-portability spike: project-relative paths + embed policy.**
Decide and implement: when a project saves, asset paths convert
to project-relative form where possible (e.g., asset is in the
same folder as `*.rmap.json` or in a `media/` subfolder).
Absolute paths preserved with a warning toast when relativisation
fails. Schema field `LayerKind::Image.path` and
`LayerKind::Svg.path` documented as "may be relative to the
project file." Migration: existing absolute-path projects load
unchanged but a one-time toast suggests "Save As… to migrate to
relative paths."

#### T2.24 — new (Phase 2)

**Missing-media relink flow.** When `ProjectAudit` reports a
`MissingAsset` finding, the toast offers two actions: **Find this
file…** opens an `rfd` file picker with the file's basename as a
default filter; **Remove this layer** prompts confirmation.
Successful relink emits `Command::RelinkAssetPath` (same struct
as T1.38) and re-runs `ProjectAudit` for any other missing assets
sharing the same parent folder ("Found 3 missing files in
`/old/path/`. Relink all from `/new/path/`?").

#### T3.0a — new (Phase 3)

**Schema v4: per-layer `WarpMesh` + migration from v3 `Project.warps`.**
Move mapping into the layer. `LayerConfig` gains
`pub warp: WarpMesh`; `Project.warps` is removed. Bump
`CURRENT_SCHEMA_VERSION` to 4. v3 → v4 migration copies `warps[0]`
(or a default identity warp if the v3 project had none) onto each
layer; if M > 1 warps existed, T3.0d's audit fires the
`MultipleWarpsConsolidated` Warn finding once. The bundled demo
(`assets/demos/window-glow.rmap.json`) is rewritten in the same
commit so the canonical bundle ships v4. Full task body lives in
`003-tasks-phase-3.md`.

#### T3.0b — new (Phase 3)

**Render graph rewrite: per-layer warp pass + composite-of-warped-
layers.** Replace the shared-`warp_rt` composite-then-warp model
with a per-layer warp-then-composite. Each layer is warped onto a
single reusable `warp_scratch` RT (sized to projector), then
alpha-composited onto the running projector RT with the layer's
`blend_mode` and `opacity`. `scene_texture_id` re-binds to the
projector RT view (post-warp, pre-gamma). `OverlayPipeline` and
`panic_restore` integration unchanged. New headless GPU test
`per_layer_warp_distinct_corners` validates that ≥ 3 layers each
land in their own corner-pinned region.

#### T3.0c — new (Phase 3)

**Mutation rename: `warp_idx` → `layer_idx` across all warp/mask
variants.** Mechanical rename of `Mutation::SetWarpDimensions`,
`SetMaskPolygon`, `AddMaskVertex`, `RemoveMaskVertex`,
`SetMaskVertex`, `ResetWarpMesh`, `SetWarpMaskFeather` to their
`SetLayer*` / `Layer*` siblings. Plus a new
`Mutation::SetLayerWarpCorner { layer_idx, r, c, new, old }` for
T3.5's per-layer corner drag. Apply / Reverse logic structurally
unchanged; only the index source changes. Helper constructors
`Project::set_layer_*_mutation(...)` ship alongside. Proptest
harness extended.

#### T3.0d — new (Phase 3)

**Audit rename + multi-warp consolidation finding.**
`AuditKind::DegenerateWarp` → `DegenerateLayerWarp { layer_idx }`;
`MaskTooFew` → `LayerMaskTooFew { layer_idx, vertex_count }`. New
`AuditKind::MultipleWarpsConsolidated { previous_warp_count,
layer_count }` with `Severity::Warn`, fired exactly once per
session for v3 projects whose migration consolidated > 1 warps
onto layers. Audit pass walks `project.layers` (no longer reads a
non-existent `Project.warps`).

#### T3.28 — new (Phase 3)

**Per-display gamma + brightness override.** New section in
Advanced > Selected output: per-output `gamma_override`,
`brightness_override`, `contrast_override` (all default to
inherit-from-master). Stored in the `Project.warps[i]` (or in a
new `Project.outputs[]` collection if the schema needs it; T4.13
already plans an `OutputTarget` type, so reuse). The render
pipeline applies per-output gamma in the final pass. Does **not**
require multi-output (still single-projector); the override
addresses the laptop-vs-projector colour-space gap.

#### T4.16 — rewritten (Phase 4)

**Hot-swap windowed↔fullscreen at Go-live, with persistent control-
window preview.** The preview that lives in the control window
during `Editing` continues rendering during `GoLive`. The
projector's surface goes fullscreen; the control window stays at
its current size with the same `warp_rt`-backed live preview.
Implementation: the existing `register_native_texture` path keeps
working through the surface re-creation because the *control
window's* egui texture is bound to `warp_rt_view`, not to the
projector's surface. The projector's surface re-creation does
not invalidate the control-window preview. Acceptance criterion
added: **the control-window preview FPS during `GoLive` is within
20% of `Editing` FPS, with no perceived stutter on the projector.**

#### T4.16a — new (Phase 4)

**"Preview as projector" pre-show mode.** Before clicking Go live,
the operator can choose **Preview** (new toolbar button next to
Go live). This opens an extra small window on the laptop sized
to the projector's aspect ratio (no projector connection
required). Operator can dry-run mapping / scenes / cues. No
display-sleep assertion held. Closing the preview returns to
`Editing` cleanly. Implementation: same `OutputWindow` path as
the real projector but on the primary display, windowed,
non-fullscreen. New `AppState` variant **not** required —
preview is a transient sub-mode of `Editing`.

#### T4.23 — new (Phase 4)

**Capability roadmap doc.** New file `specs/v3-capability-scope.md`
that explicitly states what v3 ships and what v0.4 will own.
Targets:
- v3 ships: still images + SVG, single projector, manual warp +
  mask, scenes + crossfades, autosave, launcher, demo,
  show-day strip, project audit.
- v0.4 will own: video layer (mp4/H.264), NDI input layer (one
  layer kind), 2-projector edge-blend stub, OSC live parameter
  binding UI (revival of M7 stubs), per-projector colour
  calibration.
- v3.1 catches the deferred items: 4 audit findings, schema v5
  portable monitor, compile-time Reverse-storage enforcement,
  native menu bar (if dropped from M4), film-strip + test-grid
  demos.

This document doubles as a contributor charter and an honest
pitch for the README rewrite (T5.11).

#### T5.6 — rewritten (Phase 5)

**External usability test (post-GA validation cycle).** Rescoped
from "GA gate" to "post-GA cycle that informs v3.1." Recruitment
starts the day after `v0.3.0` ships. Sessions: n ≥ 5, 30 min
each, recording the canonical 7-step flow. Results inform v3.1
backlog priorities. **Not a release blocker.**

#### T5.16 — new (Phase 5)

**Practitioner field beta.** Before `v0.3.0` tag, recruit one
wedding-DJ-style operator and one AV-teacher-style operator. Each
runs the tool through a real (or simulated-real) one-event use:
a small ceremony, a school assembly, a gallery opening — single
projector, real photos, real venue. Sessions are observed and
note-taken; the tool is **not** changed mid-session. After both
sessions: triage findings into "blocker for GA" vs. "post-GA
fix." Blockers gate M5; non-blockers feed v3.1.

---

## 6. Required-category coverage (post-revision)

| Category | Coverage status |
|----------|-----------------|
| Real-world setup and calibration workflow | T2.1, T2.6 (test pattern from launcher), T3.1 (canvas merge), T3.5 (warp on canvas), T3.10 (snap), T3.28 (per-display gamma), T4.16a (offsite preview) |
| Simple single-projector success path | T2.8 (demo), T2.9 (try a demo) — bull's-eye scenario |
| Multi-projector / pro readiness | **Explicitly out of scope**; T4.23 (roadmap doc) makes the deferral honest. Decision-tasks D11–D14 |
| Stage / canvas manipulation suitability | T3.1–T3.10 unchanged from original |
| Operator performance and reliability | T1.32, T3.23, T4.16 (rewritten), T5.4 (panic injection rehearsal) |
| Performance and playback validation | T4.22 (60 fps + 1% idle CPU), T5.5 (stress test) |
| Format / hardware assumptions | T2.7 (NSScreen FFI), T2.20 (last-projector memory), T4.16 (hot-swap surface) |
| Error recovery and fallback workflows | T1.43, T1.44 (Failed state), T2.24 (missing-media relink) ◄── new, T4.16 catch_unwind |
| Showtime-friendly operation | T1.32 + T3.23 (show-day strip), T4.16 (preview persists), T4.17 (Stop button reverts) |
| Compatibility and migration concerns | T2.23 (portability), T1.40 (schema-too-new audit), T4.13 (deferred v3.1) |
| Instrumentation and validation | T1.45–T1.47, T5.2 (telemetry summary), T5.7 (privacy review) |
| Design QA / field QA | T4.21 (design QA), T5.4 (show rehearsal), T5.16 (field beta) ◄── new, T5.6 (post-GA cycle) |

All categories covered. The two with material additions: **error
recovery** (T2.24 missing-media relink) and **field QA** (T5.16
practitioner field beta).

---

## 7. Decision tasks and open questions

The practitioner review surfaced four product-scope decisions
that engineering cannot resolve unilaterally. Each is captured as
a decision-task; each must be resolved before the relevant
implementation work can begin.

### D11. Capability scope statement: video in v3?

- **Question.** Does Sir-viz-a-lot v0.3.0 include any video
  playback? Even a minimal mp4 still layer?
- **Why it matters.** Practitioner-flagged as the single biggest
  gap. Including it inflates Phase 1 from ~3 weeks to ~8 weeks
  (decode threading, codec tests, GPU-upload pipeline, format
  matrix). Excluding it preserves the 16-week schedule but
  publishes "still images only" honestly.
- **Options.**
  1. **Defer to v0.4** *(recommended)*. Ship v3 still-only;
     publish T4.23 capability roadmap that names v0.4 as
     video-bearing.
  2. Include minimal mp4-only video layer in v3 — adds ~3–5
     engineering weeks; renders Phase schedule unviable.
  3. Spike now, decide later — adds risk of late-Phase-3
     scope explosion.
- **Recommended next step.** Adopt option 1; capture the
  forward-plan in T4.23.
- **Impact if unresolved.** README rewrite (T5.11) and capability
  roadmap (T4.23) cannot be finalised; users will ask.

### D12. NDI / Syphon input layer in v3?

- **Question.** Is one of NDI / Syphon / Spout in v3 scope?
- **Why it matters.** Practitioner-flagged as "single feature,
  big competitive jump." Adds ~2 engineering weeks (Rust crate
  integration is straightforward; testing matrix is not). Excluded
  cleanly via roadmap doc.
- **Options.**
  1. **Defer to v0.4** *(recommended)*. NDI is the natural
     candidate (Rust `ndi-sys`); spec it in v0.4.
  2. Spike NDI in v3.1 (not v3) as a post-GA addition.
  3. Include in v3 — schedule risk.
- **Recommended next step.** Adopt option 1.
- **Impact if unresolved.** Same as D11.

### D13. Edge-blend stub for two adjacent projectors in v3?

- **Question.** Does v3 ship any multi-projector capability?
- **Why it matters.** Practitioner-flagged as "closer to wedding
  scope than full architectural mapping." Realistically requires
  a second `OutputWindow` + soft-edge alpha mask + output
  identification — a real Phase scope.
- **Options.**
  1. **Defer to v0.4** *(recommended)*. v0.4 covers multi-output;
     edge-blend lives there.
  2. v3.1 includes a "two-output" stub.
  3. v3 includes it — schedule blocker.
- **Recommended next step.** Adopt option 1.
- **Impact if unresolved.** Same as D11.

### D14. OSC live parameter binding UI revival in v3?

- **Question.** Does v3 expose the existing M7 OSC plumbing as a
  live binding UI?
- **Why it matters.** The plumbing exists; the UX does not.
  Practitioner says "VJ-friendly distance closed by exposing what
  exists." But the binding UX itself is non-trivial (~5 days).
- **Options.**
  1. **Defer to v0.4** *(recommended)*. v0.4 owns live control
     surface UX; the M7 stubs stay.
  2. v3.1 ships a minimal UI for static OSC bindings.
  3. v3 includes — Phase 3 scope explosion.
- **Recommended next step.** Adopt option 1.
- **Impact if unresolved.** Same as D11.

### D15. Existing decision Q1–Q10 still hold?

The original 10 plan decisions (Section 14.2 of
`003-ui-ux-overhaul-plan.md`) remain valid post-revision. No
revisits needed. Confirmed by the triage.

### D16 (residual from `003-tasks.md` Section 10)

D1–D10 from the master index remain. None changed by this
revision pass.

---

## 8. First execution slice (post-revision)

The first sprint following this revision targets the changes that
land highest user value with lowest engineering surface, while
preserving Phase 1's foundational ordering.

### Sprint 1 goals (5-day scope)

| Goal | Tasks | Notes |
|------|-------|-------|
| Land foundation (unchanged) | T0.7, T1.1, T1.7–T1.12, T1.13 | Same as the original first sprint |
| Land soft-Reverse machinery | T1.14 (rewritten) | Lighter than the original; ~1 day saved |
| Land UndoStack + proptest | T1.15, T1.17 | Unchanged |
| Land first 6 mutation migrations | T1.18, T1.20, T1.22, T1.24, T1.26, T1.30 | One per Reverse pattern (always-visible, scene-recall, drag-translate, drag-rotate effects-Vec, Modulator whole-enum, ApplyProjectSnapshot) |
| Land 2 P0 audit findings only | T1.34, T1.35 (zero-scale), T1.38 (missing-asset) | T1.36, T1.37, T1.39, T1.40 deferred within Phase 1 |
| Telemetry skeleton | T1.45 | Unchanged |
| Open `v3-foundation` PR | (continuous) | `--features v3` gate |
| **Add to sprint demo:** | | |
| Asset-portability spike | T2.23 | Run as a Phase 1.5 spike — 1 day; doesn't block M1 |
| Capability roadmap draft | T4.23 (draft only) | Writing task; runs in parallel with engineering |

### Validate early

- **Reverse storage** via the proptest harness (R11 mitigation
  intact; runtime path now).
- **Asset-portability assumptions** via T2.23 spike before T2.24
  implementation.
- **Capability roadmap draft** via PO/practitioner-feedback
  consumer (the same person who wrote the practitioner review)
  before T4.23 final.

### Feature-flag

- All v3 work behind `--features v3` (T0.7) until M3.
- The new T2.23 / T2.24 portability change behind a sub-flag
  `--features v3-portable-paths` until validated; `main` build
  with `--features v3` still uses absolute paths until the spike
  resolves.

### Defer

- T1.36, T1.37, T1.39, T1.40 (4 audit findings) — slack-available
  Phase 1; v3.1 fallback.
- T4.12, T4.13 (schema v5 portable monitor) — v3.1 backlog.
- T4.19 (native menu bar) — M4 if slack, v3.1 if not.
- T5.6 (external usability test) — post-GA validation cycle.
- D11–D14 capability scope deferrals — confirmed via T4.23.

### Test with practitioners before expanding

- **Field beta (T5.16)** is the gate before GA. Run before tag.
- **Capability roadmap (T4.23)** is published with the
  practitioner-reviewer signed-off ("does this v0.4 plan match
  what you'd want from a v0.4?").
- **README rewrite (T5.11)** uses the practitioner's own framing
  ("easiest free tool for projecting photos at small events")
  unless PO and DES override.

---

## 9. Quality bar self-check

- [x] Mapped every major feedback theme into concrete task actions
      (16 items in Section 2).
- [x] Preserved Phase 1's architectural ordering; no foundational
      task was reordered.
- [x] Added validation work (T5.16 field beta) and softened the
      academically-strong-but-calendar-blocking T5.6 into a
      post-GA cycle.
- [x] Added performance work (preserved T4.22, added per-display
      gamma).
- [x] Avoided scope-bloat: net engineering days delta is ~0;
      calendar shortened.
- [x] Separated immediate (Section 8 sprint), deferred (T4.12,
      T4.13, T1.36, T1.37, T1.39, T1.40), and decision
      (D11–D14) work clearly.
- [x] Did not silently accept overreach (rejected video / NDI /
      edge-blend in v3 via decision-tasks).
- [x] Did not silently dismiss any major feedback (every
      practitioner item appears in Section 2 with an explicit
      action).

---

## 10. Traceability index

For each numbered practitioner-feedback item from the review:

| Practitioner item | Lives in |
|-------------------|----------|
| Output preview during Go live | T4.16 (rewritten) |
| Asset portability + missing-media relink | T2.23, T2.24, T1.38 (extended) |
| External usability test out of GA gate | T5.6 (rescoped) |
| Per-display gamma override | T3.28 (new) |
| Compile-time Reverse machinery friction | T1.14 (rewritten) |
| 6 audit findings reduction | T1.36–T1.40 reprioritised |
| Capability scope statement (v0.4 plan) | T4.23 (new) + D11–D14 decisions |
| Native menu deferrable | T4.19 marked deferrable |
| Schema v5 deferred | T4.12, T4.13 marked v3.1 |
| Video in v3 (rejected) | D11 |
| NDI / Syphon (rejected) | D12 |
| Edge-blend stub (rejected) | D13 |
| OSC binding UI (rejected) | D14 |
| Reposition product (partial) | T5.11 (tone shift) + T4.23 |
| Field beta before GA | T5.16 (new) |
| Find missing media on Open Recent | T2.10 (amended via T2.24) |
| Offsite preview mode | T4.16a (new) |
| Per-layer warp + mask + effects | T3.0a, T3.0b, T3.0c, T3.0d (new); T3.3, T3.5, T3.6, T3.7, T3.15 (amended) |

End of revision document. Apply the targeted edits to the four
phase files and the master index before starting Sprint 1.
