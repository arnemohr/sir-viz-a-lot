# Phase 7 — Calibration file schema decision (P7.W0, W7)

**Status:** decision record. The runtime loading path, show-file binding
model, and calibration verify UI land in W7–W8 tasks once this decision
is ratified.

## Constraints

- **Plan locks the file extension.** `004-phase-7.md` "Engine implications"
  names the file as `.rmap-calibration.json`. This is not re-litigated here;
  the decision covers structure, location, surface-binding model, and mismatch
  behaviour.
- **Must extend v3.1 schema v5 portable monitor work (T4.12, T4.13).**
  `specs/004-v3.1-tasks.md` (V31.2.x) landed `OutputTarget { uuid:
  Option<String>, fallback_index: usize }` for portable monitor matching.
  The calibration binding model extends this: a calibration surface slot
  references an `OutputTarget`-compatible identity; at session start the
  same UUID-then-index resolution logic that already runs for show files
  runs for calibration files.
- **Separate from show file.** The plan is explicit: "venue-scoped warp +
  mask + gamma + monitor identity travels separately from the show file." A
  show file does not embed calibration data; it references a calibration
  file (or has none, falling back to identity warp).
- **Audit model.** The project audit pattern (`src/project/audit.rs`)
  raises `AuditKind::OutputTargetUuidNotFound` when a monitor referenced by
  UUID can't be found. Calibration surface mismatches follow the same shape.
- **No tokio; atomic save.** `Project::save` uses `temp-file + rename`
  for atomicity. The calibration file must follow the same pattern.
- **`I1 / Recommendation G follow-on`** (`004-phase-7.md` UX items).
  Calibration export uses the same coordinate format (px + percent + corner
  names) as the v3 coordinate readouts. No new coordinate system is
  introduced.

## Open question being locked: runtime binding model

The plan says "show files bind to abstract surface IDs." Two binding
approaches are plausible:

### Option A: show file carries a calibration file path reference (rejected)

The show file stores `calibration_path: Option<PathBuf>`. At session start,
rmap loads the calibration from that path. The operator switches calibrations
by pointing the show file at a different `.rmap-calibration.json`.

**Rejected because:**
- Path portability: absolute paths break when the operator moves files between
  machines (the known footgun from `CLAUDE.md`'s gitignore section — save-path
  fields don't shell-expand).
- The show file must change whenever the venue changes, coupling content to
  venue identity.
- Calibration changes cannot be saved without also re-saving the show file.

### Option B: calibration file loaded separately at session start; bound by surface-slot UUID (chosen)

The calibration file is a peer to the show file, not embedded in it. At
session start the operator opens both independently (or rmap looks for a
calibration in the same directory as the show file, by naming convention).
Binding is by "surface slot UUID" — a stable identifier assigned to each
projector surface slot in the calibration file. Show files reference the same
UUID in their `OutputTarget`, so the runtime can join them without path
coupling.

The mismatch behaviour when no calibration matches:
- **Soft miss (UUID present in show file but no calibration loaded):** audit
  warning `AuditKind::CalibrationSurfaceUnmatched`. Identity warp/mask/gamma
  applied. No hard fail — the show plays, the operator sees a badge.
- **No calibration loaded at all:** no audit warning; identity warp/mask/gamma
  is the default. Calibration is optional.

**The mismatch MUST NOT be a hard fail** — show-day reliability (`CLAUDE.md`
"show-day reliability" section) means missing calibration = identity, not crash.

## Chosen file extension and location

- **Extension:** `.rmap-calibration.json` (plan-locked).
- **Location on disk:** operator-chosen on save, with a suggested default of
  `~/Documents/rmap/calibrations/<venue-name>.rmap-calibration.json`.
  rmap does not auto-discover calibration files from the filesystem (avoids
  ambiguity when multiple calibrations exist for the same venue). The operator
  explicitly loads a calibration file via File > Load Calibration (or drag-drop
  into the Output panel).
- **Naming convention for same-directory auto-load:** if a file named
  `<show-file-stem>.rmap-calibration.json` exists in the same directory as
  the open show file, rmap offers (not forces) to load it on project open.
  The offer is a toast/banner in the Output panel, not a modal. This matches
  the "venue file + show file in the same folder" workflow common in live AV.

## Schema

```json
{
  "schema_version": 1,
  "calibration_id": "<uuid-v4>",
  "venue_name": "Warehouse B – Main Wall",
  "created_at": "2026-05-12T20:00:00Z",
  "surfaces": [
    {
      "surface_slot_id": "<uuid-v4>",
      "display_name": "Left projector",
      "output_target": {
        "uuid": "<display-uuid>",
        "fallback_index": 1
      },
      "warp": { /* BezierMesh or WarpMesh */ },
      "mask_polygon": [ /* Vec<[f32;2]> */ ],
      "mask_feather": 0.02,
      "gamma_matrix": [[1,0,0],[0,1,0],[0,0,1]],
      "brightness": 1.0,
      "contrast": 1.0
    }
  ]
}
```

Key design choices:
- `calibration_id` is a UUID stable across saves — show files that reference
  this calibration use the `calibration_id` (not a path), stored in the
  show file's `OutputTarget` as an optional field.
- `surface_slot_id` per surface mirrors `OutputTarget.uuid` — the runtime
  join is `show.output_targets[i].calibration_surface_slot_id ==
  calibration.surfaces[j].surface_slot_id`.
- `warp` stores a `BezierMesh` (schema v8 per the Bezier decision) or a
  `WarpMesh` (v7, loaded via migration). Calibration files have their own
  `schema_version` counter (starts at 1, independent of project schema).
- `gamma_matrix`, `brightness`, `contrast` mirror the existing per-output
  colour correction fields in `OutputTarget`. They override the show file's
  per-output values when a calibration is loaded (show file values are the
  content-level intent; calibration values are the venue-level correction).
  The merge rule: `final = gamma_matrix_calib × gamma_matrix_show`.

## Runtime binding model (for W7 follow-up tasks)

1. `EditingState` gains `loaded_calibration: Option<CalibrationFile>`.
2. On project load + on "Load Calibration" action:
   - For each `surface_slot` in the calibration, find the matching
     `OutputTarget` in the show file by `calibration_surface_slot_id`.
   - If matched: apply warp, mask, gamma from the calibration slot to the
     `OutputTarget`'s runtime state (not to the persisted show file — the
     show file does not change).
   - If unmatched: emit `AuditKind::CalibrationSurfaceUnmatched { slot_id,
     display_name }`.
3. Calibration is applied at the session level — it is not a `Mutation` (it
   does not enter the undo stack). "Save Calibration" is its own action,
   distinct from "Save Project".
4. Calibration file save: temp file + `rename` (same atomicity guarantee as
   project save).

## Acceptance gates

- [ ] `.rmap-calibration.json` schema defined; `calibration_id` and
      `surface_slot_id` are UUIDs stable across saves.
- [ ] Show file loads without a calibration present; identity warp/mask/gamma
      applied; no crash.
- [ ] `AuditKind::CalibrationSurfaceUnmatched` appears in the audit panel
      when a loaded calibration references an unmatched surface slot.
- [ ] Calibration file saved via File > Save Calibration; atomic (temp + rename).
- [ ] Auto-load offer: if `<stem>.rmap-calibration.json` exists beside the
      show file, a non-blocking toast offers to load it.
- [ ] Calibration warp/mask/gamma applied at runtime; show file on disk
      unchanged by loading a calibration.
- [ ] `I1 / Recommendation G`: coordinate format in calibration file matches
      the v3 coordinate readout format (normalised 0–1 + percent overlay).

## Out of scope

- Embedding calibration data inside the show file (Option A, rejected above).
- Calibration cloud sync or venue "library" discovery UI.
- Multi-surface calibration for >2 projectors (deferred: multi-output is
  out of scope for Phase 7 per the plan's "optional only if single-surface
  workflow is excellent" clause).
- Calibration file encryption or signing for live-show integrity.
