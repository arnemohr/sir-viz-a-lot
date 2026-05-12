# Decision: Phase 4 scene template schema, file extension, storage, and instantiation mutation

**Status:** open — must be resolved before P4.2.1 can start.
**Depends on:** none.
**Unblocks:** P4.2.1 (SceneTemplate struct), P4.2.2 (JSON schema + file
extension), P4.2.3 (LoadSceneTemplate mutation), P4.5.1–P4.5.8 (all W5
built-in templates), P4.8.1 (proptest round-trip).

---

## Context

The Phase 4 plan states: "Scene template format: portable JSON schema, lives
alongside the per-project file but is reusable across projects." Three concrete
sub-questions must be answered before any schema work begins:

1. **What is the JSON schema shape for a `SceneTemplate`?**
2. **What file extension distinguishes scene templates from project files?**
3. **Where do templates live on disk — built-in vs user-authored?**
4. **What Mutation does "apply this template to the project" produce?**

---

## Sub-question 1 — JSON schema shape

### Option 1A — Self-contained layer-bundle

A `SceneTemplate` is a list of fully-specified `LayerConfig` + `WarpMesh` pairs,
verbatim. The operator assigns media and zones by filling placeholder fields (e.g.
`{ "kind": "image", "path": "__SLOT_media_0__" }`). On apply, the instantiation
pass resolves placeholders against wizard choices.

**Pros:** simple serialisation — a template is just a partial project JSON.
**Cons:** templates embed warp geometry, which is projector-specific and
therefore not portable. Also, "placeholder" strings are not schema-validated.

### Option 1B — Recipe model (recommended)

A `SceneTemplate` declares:
- **`id`** — stable identifier (`"window_reveal"`, `"pixel_drift"`, …).
- **`display_name`** — operator-facing label.
- **`description`** — one-sentence summary.
- **`zones_consumed`** — list of `ZoneRole` tags the template binds to
  (`["window"]`, `["edge", "void"]`, etc.). Displayed in the zone-mapping step.
- **`media_slots`** — list of named media slot descriptors (`{ "name": "bg",
  "label": "Background image", "accepts": ["image", "video"] }`).
- **`fx_presets_used`** — list of FX preset IDs this template activates.
- **`palette`** — default palette hint (`"warm"`, `"cool"`, `"neutral"`).
- **`mood`** — default mood hint (`"calm"`, `"energetic"`, `"ethereal"`).
- **`tempo_sync`** — whether the template ties animation to BPM.
- **`builtin`** — `true` for compiled-in templates; `false` for user exports.

The instantiation pass (P4.2.3) reads the recipe and calls `AddLayer` × N via
the existing Mutation API, assigning wizard choices to media slots and zone
bindings at construction time. The template itself contains no warp geometry.

**Pros:** genuinely portable; templates travel between projects and machines
without carrying projector-specific warp state. Zones are addressed by semantic
role (Phase 3), not by polygon ID. FX preset IDs reference the Phase 2 registry.
**Cons:** instantiation logic (recipe → layers) is new code, not pure
deserialization. This is the right place for that logic to live.

### Recommendation

**Option 1B — recipe model.** Warp geometry must not be in a template.

---

## Sub-question 2 — File extension

| Extension | Notes |
|-----------|-------|
| `.rmap-scene.json` | Mirrors `.rmap-preset.json` (Phase 2, P2.8.5). |
| `.rmap-template.json` | More explicit; aligns with "template" terminology. |
| `.scene.json` | Shorter; risks collision with other tooling. |

### Recommendation

**`.rmap-scene.json`** — mirrors the preset-file convention exactly
(`.rmap-preset.json`), making operator mental model consistent.

---

## Sub-question 3 — Storage location

Mirrors Phase 2 P2.8.5 decision:

| Kind | Location |
|------|----------|
| Built-in templates | Compiled into the binary (Rust `include_str!` or a `static` registry, same pattern as FX presets). No on-disk distribution for built-ins. |
| User-exported templates | `~/Library/Application Support/rmap/scenes/*.rmap-scene.json` |
| Template star/favourite state | `~/Library/Application Support/rmap/scene_stars.json` |
| Read-only enforcement | Delete only applies to user templates; built-ins are read-only. |

This is a direct copy of the P2.8.5 policy for presets, with `scenes` replacing
`presets` in the path and `.rmap-scene.json` replacing `.rmap-preset.json`.

---

## Sub-question 4 — Instantiation Mutation

### Option 4A — Single `LoadSceneTemplate` Mutation (asymmetric)

Add a new asymmetric `Mutation::LoadSceneTemplate { template_id, choices }` that
on apply: clears all current layers (emit `RemoveLayer` × N), then emits
`AddLayer` × M for the template's generated layers. Reverse: `ApplyProjectSnapshot`
restoring the pre-wizard snapshot.

**Cons:** asymmetric variants are documented exceptions in `src/project/CLAUDE.md`;
the file already calls out only four: `AddLayer`, `RemoveLayer`,
`AddLayerMaskVertex`, `RemoveLayerMaskVertex`. Adding a fifth exception is
permissible but must be deliberate.

### Option 4B — Wizard commit via `ApplyProjectSnapshot` (recommended)

The wizard builds the full post-template project JSON in memory (by calling
`AddLayer` mutations against a scratch `Project` clone), then commits the
resulting JSON as `ApplyProjectSnapshot { new: generated_json, old: pre_wizard_snapshot, non_undoable: false }`.

The undo stack entry is a single `ApplyProjectSnapshot` snapshot-swap, making
"undo the scene wizard" a one-step operation that restores exactly the state
before the wizard opened — identical to scene recall undo.

**Pros:** leverages the battle-tested `ApplyProjectSnapshot` path. No new
Mutation variants. Wizard cancel also uses `ApplyProjectSnapshot` (same
mechanism). Undo semantics are intuitive: one Cmd-Z undoes the entire wizard.
**Cons:** the intermediate scratch `Project` build happens in memory off the
main project; must not be committed to the undo stack mid-flight.

### Recommendation

**Option 4B — `ApplyProjectSnapshot` commit.** The wizard builds a scratch
`Project` by applying `AddLayer` mutations in sequence, then the commit step
dispatches `ApplyProjectSnapshot { non_undoable: false }`. Cancel dispatches
the same with the pre-wizard snapshot, restoring the old state.

This also means the wizard does NOT need a `LoadSceneTemplate` Mutation at all
in the undo-stack sense — the mutation that matters is the final
`ApplyProjectSnapshot`. The `SceneTemplate` type is a read-only registry entry,
not a `Mutation` variant.

---

## Summary of locked choices (for task authors)

| Sub-question | Decision |
|---|---|
| Schema shape | Recipe model (1B): id, display_name, zones_consumed, media_slots, fx_presets_used, palette, mood, tempo_sync, builtin |
| File extension | `.rmap-scene.json` |
| Built-in storage | Compiled in (static registry, same as FX presets) |
| User storage | `~/Library/Application Support/rmap/scenes/*.rmap-scene.json` |
| Star state | `~/Library/Application Support/rmap/scene_stars.json` |
| Instantiation mutation | `ApplyProjectSnapshot` (no new Mutation variant) |
| Undo granularity | One Cmd-Z undoes the entire wizard commit |

---

## Action for P4.2.1

1. Accept or reject these recommendations (record decision inline here).
2. Define `SceneTemplate` struct and `SceneTemplateRegistry` in a new
   `src/project/scene_templates.rs` (parallel to `zone_templates.rs`).
3. Mark this doc "resolved" once the struct shape is agreed.
4. Update task spec: remove BLOCKED annotations from P4.2.1 onwards.
