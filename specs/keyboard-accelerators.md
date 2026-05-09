# rmap keyboard accelerator audit

**Task:** T4.18  
**Status:** Documentation only — do not change behaviour.

This document lists every keyboard binding wired in rmap, the command it
triggers, and the file + line where the dispatch originates. It was produced
by reading the source directly; it does not invent bindings.

---

## Plain-letter bindings (no modifier required)

Dispatched from the output-window `WindowEvent::KeyboardInput` handler in
`handle_editing_window_event` (`src/app.rs:3402`). All use `physical_key`
(layout-independent key codes), so the position on the keyboard is fixed
regardless of the operator's locale.

| Key | Command | Effect |
|-----|---------|--------|
| `B` | `Command::Blackout` | Toggle projector black. Source: `src/app.rs:3404` |
| `F` | `Command::Freeze` | Hold current frame on projector. Source: `src/app.rs:3409` |
| `T` | `Command::CycleTestPattern` | Cycle through test patterns. Source: `src/app.rs:3412` |
| `O` | `Command::ToggleEditorOverlay` | Toggle warp/mask handles on projector. Source: `src/app.rs:3415` |
| `Escape` | `event_loop.exit()` | Quit the application (output window focused). Source: `src/app.rs:3403` |

### Scene recall (output-window focus)

Dispatched via `KeyboardSource::push_winit_key` (`src/controls/keyboard.rs:37`)
which is polled each frame by `InputState`. The key events are buffered and
drained on the next poll cycle.

| Keys | Command | Effect |
|------|---------|--------|
| `1`–`9` | `Command::SceneRecall(0..8)` | Recall scene slot 0–8 (zero-indexed). Source: `src/controls/keyboard.rs:42–50` |
| `Space` | `Command::TapTempo` | Tap tempo for BPM-linked modulators. Source: `src/controls/keyboard.rs:39` |

---

## Cmd-modified accelerators (v3 feature, output-window focus)

Wired in the same `KeyboardInput` arm as the plain-letter bindings. Only
dispatched when `state.modifiers.super_key()` (macOS Cmd) or
`state.modifiers.control_key()` (Linux/Windows Ctrl) is also held.
`state.modifiers` is updated by `WindowEvent::ModifiersChanged`
(`src/app.rs:3390`).

| Chord | Command | Effect | Source |
|-------|---------|--------|--------|
| `Cmd-Z` | `undo_stack.undo()` | Undo last mutation. | `src/app.rs:3418–3458` |
| `Cmd-Shift-Z` | `undo_stack.redo()` | Redo last undone mutation. | `src/app.rs:3418–3458` |

### Same chords from the control window (egui focus)

When the **control window** is focused instead of the output window, winit
`KeyboardInput` events are swallowed by egui. The undo/redo chords are
re-detected inside egui's input state after the `ctrl.render(…)` call using
`ui.input(|i| i.key_pressed(egui::Key::Z))`. Same semantics as the output-
window path.

| Chord | Command | Effect | Source |
|-------|---------|--------|--------|
| `Cmd-Z` (control window focused) | `undo_stack.undo()` | Undo last mutation. | `src/app.rs:3143–3167` |
| `Cmd-Shift-Z` (control window focused) | `undo_stack.redo()` | Redo last undone mutation. | `src/app.rs:3143–3167` |

---

## Bindings NOT yet wired as keyboard chords

The following operations exist in the UI (toolbar buttons / menu items) but
have no dedicated keyboard chord in the current codebase. They are called out
here so a future native menu bar (T4.19) has a clear gap list.

| Operation | How to invoke today | Notes |
|-----------|---------------------|-------|
| Save (in place) | Toolbar "Save" button → `ControlPanelAction::RequestSave` | No `Cmd-S` chord. |
| Save as… | Toolbar "Save as…" button → `ControlPanelAction::RequestSaveAs` | No `Cmd-Shift-S` chord. |
| Open | Launcher window; no open-in-editor chord | No `Cmd-O` chord. |
| Quit (control window) | macOS window close gesture / `Cmd-Q` via the OS app menu | Not wired in our `KeyboardInput` handler. `Escape` on the output window exits; the control window's `CloseRequested` event drops the window without quitting. |

---

## Conflicts: none

`O` (EditorOverlay) and a hypothetical `Cmd-O` (Open) are **not** in conflict
because they require different modifiers. Plain `O` fires only when no command
modifier is held; a `Cmd-O` chord would only fire when the modifier is present.
The two bindings occupy disjoint modifier levels.

No other conflicts exist between the plain-letter bindings and the
Cmd-modified bindings because the plain-letter path does not check for the
absence of modifiers — the operator pressing `Cmd-Z` with the output window
focused hits the `KeyCode::KeyZ` arm, where the modifier check then routes
to undo/redo rather than any plain-letter command. There is no `KeyZ` plain
binding.

---

## Index: dispatch sites

| File | Line(s) | What |
|------|---------|------|
| `src/app.rs` | 3390 | `ModifiersChanged` → `state.modifiers` |
| `src/app.rs` | 3402–3458 | Output-window `KeyboardInput` arm (plain letters + Cmd-Z) |
| `src/app.rs` | 3143–3167 | Control-window egui input poll (Cmd-Z / Cmd-Shift-Z) |
| `src/controls/keyboard.rs` | 37–56 | `KeyboardSource::push_winit_key` (Space, 1–9, B, F) |
