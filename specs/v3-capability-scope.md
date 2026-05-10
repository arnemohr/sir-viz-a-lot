# rmap v3 capability scope

**Task:** T4.23  
This document is the canonical scope statement for rmap v3 (the v0.3.0
release built under Spec 003). It is written as scope, not as date
commitments. A separate release checklist lives in
`specs/003-tasks-phase-4-5.md` (Phase 5 gate criteria).

---

## v3 ships

These capabilities are part of the v0.3.0 release. All are either already
landed on `main` or gated behind `--features v3` and scheduled for the v3
flag flip at M3.

**Content types**
- Still images: JPEG, PNG, WEBP, GIF (first frame)
- SVG layers with hot-reload (file-system watcher; re-renders on save)
- Single projector output per rmap instance

**Canvas + warp**
- Manual warp: draggable corner-pin quad per layer
- Mesh subdivision: configurable rows × cols for finer local deformation
- Corner snapping to projector edges
- Warp editing directly on the canvas (no separate Mapping tab)

**Mask**
- Per-layer mask polygon: click-to-add vertices, drag to adjust
- Zone templates: full, left half, right half, top/bottom split, etc.
- Mask feather (soft edge up to ~0.5)

**Effects and compositing**
- Per-layer effect chain: Transform (translate, scale), External JSON
- Per-layer blend mode: Normal, Add, Multiply, Screen
- Per-layer opacity
- Per-layer gamma / brightness / contrast + per-projector display override

**Scenes and crossfades**
- Up to 9 scene slots with thumbnail capture
- Visual cue strip with scene thumbnails and one-click recall
- Configurable crossfade duration per project
- Keyboard recall: `1`–`9`

**Show-day strip**
- Blackout, Freeze, Test Pattern (cycle), Editor Overlay toggle
- Go-live mode: fullscreen on the projector, separate from editing state
- Persistent preview window (second display or floating window)

**Project management**
- Autosave every 5 minutes when dirty (configurable)
- Save in place (`Save`) and Save as… dialog (`Save as…`)
- Launcher: "Try a demo" opens a bundled sample project without CLI flags
- Open recent: last-used projects remembered across sessions
- Project audit on load: detects missing media, schema drift, multi-warp
  consolidation needed

**Schema migration**
- v3 → v4 → v5 schema migration is automatic on load; no manual conversion

**Tooling + diagnostics**
- `--list-monitors` enumerates displays with human-readable names
- Show-day operator checklist: `docs/show-day-checklist.md`
- Per-day UX metrics JSON sink (T1.47)
- Glossary popovers on every domain term in the Advanced panel
- In-app Glossary window listing all terms
- In-app help ("?") opens the README in the default browser

---

## v3.1 catches

Capabilities deliberately deferred from v3 because they would have added
schema churn, spec complexity, or test surface without operator-visible
benefit at the v0.3.0 milestone.

**Deferred audit findings**
- T1.36: static-value modulator round-trip edge case
- T1.37: `crossfade_duration_s` Reverse-storage under undo
- T1.39: `output_windowed` undo boundary case
- T1.40: empty-effects-vec snapshot parity

**Schema v5 portable monitor (T4.12, T4.13)**
- `output_monitor` field becomes `OutputTarget { uuid: Option<String>, fallback_index: usize }`
- On load, prefer UUID match; fall back to index; fall back to display 0 + audit warning
- Enables project portability across machines with different monitor orders

**Compile-time Reverse-storage refactor (from T1.14)**
- Move from per-variant reverse-storage convention to a type-level guarantee
- Reduces the risk of silent corruption when adding new `Mutation` variants

**Native macOS menu bar (T4.19, if not shipped in M4)**
- `File / Edit / Window / Help` via `objc2-app-kit::NSMenu`
- Exposes `Cmd-S`, `Cmd-Shift-S`, `Cmd-O`, `Cmd-Q` as keyboard chords
- About box: version, license, contributors
- Help → rmap Help: opens README in browser
- On Linux/Windows: no-op (egui menu suffices)

**Additional demo content**
- Film strip demo scene
- Test grid demo scene

---

## v0.4 will own

These capabilities require new subsystems, external dependencies, or
GPU pipeline work that would be unsafe to land in a patch release.
None of them are operator-visible blockers for event-scale single-projector
shows, which is rmap v3's stated target.

**Video playback**
- mp4 / H.264 minimum viable path: decoded on a background thread,
  uploaded to GPU each frame as a texture
- Seamless loop, configurable playback speed
- Requires a decoder library (e.g. `ffmpeg` bindings or `symphonia` + a video
  codec crate) and a thread-safe texture-upload pipeline

**NDI input layer**
- Receive an NDI stream as a layer source
- Requires the NDI SDK and a Rust binding

**Two-projector edge-blend stub**
- Second `OutputWindow` on a second monitor
- Per-projector warp + mask
- Shared blend region with configurable overlap and falloff
- Full calibration workflow deferred further (not in v0.4 scope)

**OSC live parameter binding UI**
- Visual patch panel: OSC address → layer parameter mapping
- Currently OSC is a cargo feature (`--features osc`) with no UI
- v0.4 adds a binding editor in the Advanced panel

**Per-projector colour calibration**
- Extends the existing per-display gamma / brightness / contrast override
- Adds a full RGB matrix (for projectors with consistent colour shift)
- Likely requires a hardware measurement workflow or at minimum a manual
  adjustment tool beyond the current slider trio
