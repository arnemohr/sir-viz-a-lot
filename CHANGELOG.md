# Changelog

All notable changes to rmap are documented here.

---

## v0.3.0 — v3 UI/UX overhaul (Spec 003)

This release replaces the v2 tabbed control panel with a canvas-first editor.
Every operator-visible change is listed below. See
[`specs/v2-to-v3-migration.md`](specs/v2-to-v3-migration.md) for an
operator-facing diff if you are upgrading from v2.

### What changed

**Canvas merge**
- The separate "control panel" concept is gone. The canvas occupies the main
  window; all controls surface as contextual panels rather than fixed tabs.

**Layer thumbnail strip (left edge)**
- Layers are listed in a vertical thumbnail strip on the left. Clicking a
  thumbnail selects the layer for editing. Drag to reorder.

**Inspector (right edge, context-sensitive)**
- Appears automatically when a layer is selected. Shows fit mode, opacity, and
  the layer's asset path.

**Advanced disclosure panel (right edge, on demand)**
- Opened via the toolbar "Advanced" button or the keyboard shortcut.
- Sections: Master (gamma / brightness / contrast), Display output (per-
  projector tone override), Selected layer (effect chain, blend mode, mapping),
  Project (output mode, save/load), Diagnostics.
- All labels carry a "?" glossary popover explaining the domain term.

**Toolbar (top)**
- Project name with dirty indicator (dot prefix when unsaved).
- Undo / Redo buttons (disabled when stack is empty).
- Save and Save as… buttons.
- Warp mode toggle.
- Preview: opens a floating secondary preview window.
- Go live / Stop: transitions to fullscreen on the projector.
- Glossary: opens the in-app term browser.
- `?`: opens the README in the default browser.

**Per-layer warp, mask, and effects (schema v4/v5)**
- Warp corners are now dragged directly on the canvas (no Mapping tab).
- Mask polygon is drawn on the canvas in Mask edit mode.
- Effects are edited in the Advanced > Selected layer > Effect chain section.

**Show-day strip (bottom)**
- Blackout, Freeze, Test Pattern, and Editor Overlay toggles are always
  visible at the bottom of the control window.
- Button colours reflect active state (accent = active, default = inactive).

**Cue strip (above show-day strip)**
- Visual row of scene tiles with thumbnails and click-to-recall.
- Active scene is highlighted; a progress bar overlays the target tile during
  a crossfade.

**Autosave + dirty tracking**
- Project is autosaved to the current file every 5 minutes when dirty.
- Dirty state is indicated by a dot in the toolbar project name.
- Undo and redo are tracked per-session; autosave does not clear the undo
  stack.

**Launcher**
- First-run window with "Try a demo", "Open a recent show", and "Open…"
  actions. No command-line flags needed to get started.
- Projector dropdown populated with human-readable display names (macOS:
  `NSScreen::localizedName`; other platforms: winit fallback).

**Project audit**
- On load, the project is audited for missing media, schema drift, and
  multi-warp consolidation needs.
- Findings surface as toasts and in the Advanced > Diagnostics section.
- Missing-media layers show a relink button in the inspector.

**Undo / Redo**
- All mutations (warp corner moves, effect changes, scene saves, layer adds)
  are tracked through an undo stack.
- `Cmd-Z` (macOS) / `Ctrl-Z` (Linux/Windows) undoes; `Cmd-Shift-Z` / `Ctrl-Shift-Z`
  redoes.

**Mode-aware editing**
- Layer mode: click to select a layer; drag to move/scale.
- Warp mode: drag individual warp mesh corners.
- Mask mode: click to add polygon vertices; drag to move.
- Inspect mode: read-only; used internally by the inspector panel.

**Glossary popovers**
- Every domain term in the Advanced panel (Warp, Mask Polygon, Modulator,
  Gamma, Blend Mode, etc.) shows a "?" icon. Hovering opens a popover
  with a 1–2 sentence explanation.

**Schema migration**
- v3 and v4 project files are migrated automatically to v5 on load. No manual
  conversion is needed.

### Known deferred items (v3.1)

Four audit findings were deferred to v3.1:
T1.36, T1.37, T1.39, T1.40. See `specs/v3-capability-scope.md` for details.

---

## v0.2.x — v2 tabbed editor

Legacy release. Tabbed control panel with Scene / Effects / Layers / Scenes
tabs. Warp editing in a separate Mapping tab. Numbered scene slots
(no thumbnails). Master gamma as a top-level slider.

No formal changelog was maintained for v0.2.x. The v2 codebase is still
buildable without `--features v3`.
