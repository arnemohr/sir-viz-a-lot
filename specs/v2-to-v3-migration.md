# v2 → v3 migration guide

**Audience:** operators who used rmap v2 (the tabbed editor) and are
upgrading to v3 (the canvas-first editor introduced in Spec 003).

**Project files are backward-compatible.** rmap v3 reads v2 `.rmap.json`
files and migrates the schema automatically. You do not need to convert
projects manually. The migration touches field names and adds new optional
fields; no content is lost.

---

## What changed and what to do instead

### "Mapping" tab — gone

**v2:** The Mapping tab contained a corner-pin editor and mesh-subdivision
controls. You edited warp corners in the tab's numeric fields.

**v3:** Warp corners are now dragged directly on the canvas. Click the
**Warp** button in the toolbar (or press `W`) to enter warp edit mode. Drag
a corner handle to move it. Mesh subdivision (rows × cols) is in
**Advanced → Selected layer → Mapping**.

### Layer typed-path field — gone

**v2:** Each layer had a text field where you typed or pasted a file path.

**v3:** Drag and drop a JPG, PNG, or SVG onto the canvas to add a layer.
Alternatively, the inspector shows the current asset path and a "Relink…"
button if the file cannot be found.

### Numbered scene slots → visual cue strip with thumbnails

**v2:** Scenes were stored in numbered slots (1–9) shown as text rows with
a "Save" button and a "Recall" button per slot.

**v3:** Scenes appear as thumbnail tiles in the cue strip above the show-day
strip. Click a tile to recall the scene (or press `1`–`9` on the keyboard).
To save the current look to a slot, use the cue strip's save button on the
desired tile.

### Master gamma + modulators + blend modes → Advanced disclosure

**v2:** Master gamma, per-layer blend modes, and the modulator picker were
split across the Scene tab (gamma) and the Effects tab (blend modes,
modulators).

**v3:** All of these live in the **Advanced** panel:
- **Advanced → Master** — gamma, brightness, contrast for the whole output.
- **Advanced → Display output** — optional per-projector tone override.
- **Advanced → Selected layer → Blend mode** — Normal, Add, Multiply, Screen.
- **Advanced → Selected layer → Effect chain** — effect parameters and
  modulator pickers.

Open Advanced with the toolbar **Advanced** button or press `Escape` while
the panel is focused to close it.

### Project files load automatically; schema migration is transparent

**v2 → v3 schema changes:**
- Schema v3 added per-layer effects and mask polygons.
- Schema v4 added per-layer blend mode and opacity.
- Schema v5 added per-projector gamma / brightness / contrast overrides.

On first load of a v2 `.rmap.json` file, rmap silently migrates through each
schema version and writes the result back if you save. If the file was written
by a newer version of rmap than the current build, an audit warning appears.

### MultipleWarpsConsolidated audit finding (T3.0a)

rmap v2 could write a project with multiple warp quads per layer in some edge
cases. The v3 schema uses a single quad with a configurable mesh. On load,
rmap detects this condition and emits a `MultipleWarpsConsolidated` audit
finding (visible as a toast). The quads are merged automatically using the
bounding-box union rule; the result may not match the v2 visual exactly. If
the merge looks wrong, use the canvas warp editor to re-adjust the corners.

---

## Keyboard shortcuts that are the same

| Key | Effect |
|-----|--------|
| `B` | Blackout toggle |
| `F` | Freeze toggle |
| `T` | Cycle test pattern |
| `O` | Toggle editor overlay |
| `1`–`9` | Scene recall |
| `Cmd-Z` / `Ctrl-Z` | Undo (new in v3) |
| `Cmd-Shift-Z` / `Ctrl-Shift-Z` | Redo (new in v3) |

See `specs/keyboard-accelerators.md` for the full list with source locations.
