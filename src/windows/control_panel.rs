// egui 0.34 deprecated `SidePanel` / `TopBottomPanel` / `Panel::exact_width`
// in favour of `Panel::left/right/top/bottom` + `exact_size`. The migration
// is mechanical but ripples across every panel call site here; deferring it
// to its own commit so v0.4 work doesn't bundle UI-API churn.
#![allow(deprecated)]

//! egui control panel: effects per layer, layer order, warp corners, scenes, gamma.

use std::path::Path;
#[cfg(not(feature = "v3"))]
use std::path::PathBuf;

use egui::Ui;
use serde::Deserialize;

use crate::effects::Effect;
use crate::modulators::Modulator;
#[cfg(feature = "v3")]
use crate::project::command::{ModulatorField, Mutation};
use crate::project::schema::Project;
// BlendMode + Scene + schema module only needed in v2 tab functions.
#[cfg(not(feature = "v3"))]
use crate::project::schema;
#[cfg(not(feature = "v3"))]
use crate::project::schema::{BlendMode, Scene};
// snapshot only needed in v2 Scenes tab.
#[cfg(not(feature = "v3"))]
use crate::project::snapshot;
#[cfg(feature = "v3")]
use crate::windows::anim;
use crate::windows::scene_editor::{self, SceneEditorState};
use crate::windows::theme;

/// 003-T1.18 — live-preview slider that emits a `Mutation` on
/// drag-stop instead of binding directly to a project field.
///
/// The slider operates on a per-widget staged copy of the value,
/// kept in egui's transient memory keyed by `id`. While the user
/// drags, only the staged copy moves; the project field stays at
/// `project_value`. On `drag_stopped()` (or a text-edit / scroll
/// commit), the helper returns `Some(new)` if the value changed —
/// the caller builds the corresponding `Mutation` and pushes it
/// through `UndoStack::push`. Returns `None` while still
/// interacting or when nothing changed.
///
/// `id` must be unique per slider on screen; pass a literal like
/// `"gamma"`. Egui derives the actual widget id from the parent
/// `Ui` plus this string.
#[cfg(feature = "v3")]
pub(super) fn command_slider(
    ui: &mut Ui,
    id: &str,
    label: &str,
    project_value: f32,
    range: std::ops::RangeInclusive<f32>,
) -> Option<f32> {
    let staged_id = ui.id().with("rmap_command_slider").with(id);
    let staged: Option<f32> = ui.memory(|m| m.data.get_temp::<f32>(staged_id));
    let mut shown = staged.unwrap_or(project_value);
    let resp = ui.add(egui::Slider::new(&mut shown, range).text(label));

    if resp.drag_stopped() {
        ui.memory_mut(|m| m.data.remove::<f32>(staged_id));
        return ((shown - project_value).abs() > 1e-6).then_some(shown);
    }
    if resp.dragged() {
        ui.memory_mut(|m| m.data.insert_temp(staged_id, shown));
        return None;
    }
    if resp.changed() && (shown - project_value).abs() > 1e-6 {
        // Text-edit / scroll-wheel path: no drag start/stop, fire once.
        ui.memory_mut(|m| m.data.remove::<f32>(staged_id));
        return Some(shown);
    }
    None
}

/// 003-T1.18 — checkbox companion to [`command_slider`]. Boolean
/// toggles have no drag, so the helper just emits on `changed()`.
#[cfg(feature = "v3")]
pub(super) fn command_checkbox(ui: &mut Ui, label: &str, project_value: bool) -> Option<bool> {
    let mut shown = project_value;
    let resp = ui.checkbox(&mut shown, label);
    if resp.changed() && shown != project_value {
        Some(shown)
    } else {
        None
    }
}

/// 003-T1.18 — `DragValue<u32>` companion. Same staging idea as
/// [`command_slider`]: the project value stays put while the user
/// drags, and we emit on commit. Returns `Some(new)` once the
/// edit finalises, `None` while interacting.
#[cfg(feature = "v3")]
pub(super) fn command_dragvalue_u32(
    ui: &mut Ui,
    id: &str,
    project_value: u32,
    range: std::ops::RangeInclusive<u32>,
    prefix: &str,
) -> Option<u32> {
    let staged_id = ui.id().with("rmap_command_dragvalue_u32").with(id);
    let staged: Option<u32> = ui.memory(|m| m.data.get_temp::<u32>(staged_id));
    let mut shown = staged.unwrap_or(project_value);
    let resp = ui.add(egui::DragValue::new(&mut shown).range(range).prefix(prefix));

    if resp.drag_stopped() {
        ui.memory_mut(|m| m.data.remove::<u32>(staged_id));
        return (shown != project_value).then_some(shown);
    }
    if resp.dragged() {
        ui.memory_mut(|m| m.data.insert_temp(staged_id, shown));
        return None;
    }
    if resp.changed() && shown != project_value {
        ui.memory_mut(|m| m.data.remove::<u32>(staged_id));
        return Some(shown);
    }
    None
}

/// 003-T3.3 — `DragValue<f32>` companion. Same staged-memory pattern as
/// [`command_dragvalue_u32`] but for floating-point values. Returns
/// `Some(new)` once the edit finalises (tolerance 1e-6), `None` while
/// interacting or when the value did not change.
#[cfg(feature = "v3")]
pub(super) fn command_dragvalue_f32(
    ui: &mut egui::Ui,
    id: &str,
    project_value: f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
) -> Option<f32> {
    let staged_id = ui.id().with("rmap_command_dragvalue_f32").with(id);
    let staged: Option<f32> = ui.memory(|m| m.data.get_temp::<f32>(staged_id));
    let mut shown = staged.unwrap_or(project_value);
    let resp = ui.add(
        egui::DragValue::new(&mut shown)
            .range(range)
            .suffix(suffix)
            .speed(0.005),
    );

    if resp.drag_stopped() {
        ui.memory_mut(|m| m.data.remove::<f32>(staged_id));
        return ((shown - project_value).abs() > 1e-6).then_some(shown);
    }
    if resp.dragged() {
        ui.memory_mut(|m| m.data.insert_temp(staged_id, shown));
        return None;
    }
    if resp.changed() && (shown - project_value).abs() > 1e-6 {
        ui.memory_mut(|m| m.data.remove::<f32>(staged_id));
        return Some(shown);
    }
    None
}

/// One named effect-chain bundle authored as JSON in `assets/presets/`.
///
/// Loaded once at startup via [`load_presets_from_disk`] and surfaced in
/// the Effects tab as an "Apply preset" combobox; the operator picks one
/// and the selected layer's `effects` are replaced wholesale (T-M7-08).
#[derive(Debug, Clone, Deserialize)]
pub struct Preset {
    pub name: String,
    pub effects: Vec<Effect>,
}

/// Discover presets by scanning `assets/presets/*.json` relative to the
/// current working directory. Robust to a missing directory and to
/// individual malformed files (logs a warning, skips). Sorted by name so
/// the dropdown ordering is stable across runs.
///
/// Path resolution is intentionally simple: `cargo run` from the repo
/// root finds the bundled presets, and a packaged macOS bundle ships
/// the `assets/` directory next to the binary. Operators can drop their
/// own JSON files into the directory; reload with the "Reload" button.
pub fn load_presets_from_disk() -> Vec<Preset> {
    let dir = Path::new("assets/presets");
    if !dir.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                match std::fs::read_to_string(&path) {
                    Ok(text) => match serde_json::from_str::<Preset>(&text) {
                        Ok(p) => out.push(p),
                        Err(err) => tracing::warn!(
                            path = %path.display(),
                            ?err,
                            "preset parse failed; skipping",
                        ),
                    },
                    Err(err) => tracing::warn!(
                        path = %path.display(),
                        ?err,
                        "preset read failed; skipping",
                    ),
                }
            }
        }
        Err(err) => tracing::warn!(?err, "preset dir scan failed"),
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// 003-T3.27: ControlTab and the tab field are v2-only. Under v3 these
// items do not exist at all; the cfg gate replaces the old dual-mode
// dead_code allow so the v3 build never accidentally references them.
#[cfg(not(feature = "v3"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlTab {
    Scene,
    Effects,
    Layers,
    Scenes,
}

#[cfg(not(feature = "v3"))]
impl Default for ControlTab {
    fn default() -> Self {
        // T-M9-02: Scene is the v2 default — operators see the live preview
        // first, the slider tabs are secondary.
        Self::Scene
    }
}

#[derive(Default)]
pub struct ControlPanelState {
    #[cfg(not(feature = "v3"))]
    pub tab: ControlTab,
    pub selected_layer: usize,
    /// 003-T1.18 — `Mutation`s emitted by `command_*` helpers during
    /// the current `show()` call. The app drains this after the
    /// frame and routes each entry through `EditingState.undo_stack`
    /// so every always-visible binding becomes Cmd-Z reversible.
    /// v2 builds carry no undo machinery; the field is gated.
    #[cfg(feature = "v3")]
    pub pending_mutations: Vec<Mutation>,
    /// P0.4.3 — `VideoControl` messages to dispatch to worker threads
    /// during this frame. Each entry is `(layer_idx, message)`. The app
    /// drains this alongside `pending_mutations` and routes each message
    /// to `state.layers[layer_idx].video_control` via `try_send`. Gated
    /// with `pending_mutations` since both require v3 undo infrastructure.
    #[cfg(feature = "v3")]
    pub pending_video_controls: Vec<(usize, crate::video_layer::VideoControl)>,
    /// Buffer for the Layers tab "add layer" path field. Under v3 the layer
    /// list lives in layer_strip; these fields exist on the shared struct for
    /// API compatibility but are only read from v2 code paths.
    #[cfg_attr(feature = "v3", allow(dead_code))]
    pub new_layer_path_input: String,
    #[cfg_attr(feature = "v3", allow(dead_code))]
    pub add_layer_error: String,
    /// Target path for **Save** in the Project file panel (`*.rmap.json`).
    pub project_save_path: String,
    pub project_save_message: String,
    /// Cached preset bundles loaded from `assets/presets/`. Populated lazily
    /// on first show; refreshed via the "Reload" button (T-M7-08).
    pub presets: Vec<Preset>,
    /// `true` once we've tried to load presets — keeps the empty case from
    /// re-scanning every frame.
    pub presets_loaded: bool,
    /// Selected preset index in the Effects-tab dropdown; reset on layer change.
    pub preset_picker_index: usize,
    /// 003-T3.1 + P1.UX: when true under `--features v3`, the
    /// **Controls** window (renamed from "Advanced") opens as a
    /// floating egui window alongside the canvas. Toggled by the
    /// toolbar button; v2 builds ignore this flag (tabbed UI stays).
    pub controls_open: bool,
    /// 003-T4.2 — per-session cache of egui `TextureHandle`s for scene
    /// thumbnails, keyed by a hash of the thumbnail pixel bytes. Rebuilt
    /// automatically when a scene is saved with a new thumbnail; stale
    /// entries (from deleted scenes) are evicted lazily across frames since
    /// the set is small (≤ 9 tiles in practice).
    #[cfg(feature = "v3")]
    pub thumbnail_cache: crate::windows::cue_strip::ThumbnailCache,
    /// 003-T5.12 — whether the in-app Glossary window is visible. Toggled
    /// by the "Glossary" button in the toolbar right section. The window
    /// renders all [`GlossaryTerm`] entries in a scrollable popup so the
    /// operator can scan the full vocabulary without hovering over each label.
    #[cfg(feature = "v3")]
    pub glossary_open: bool,
    /// P0.7.5 — whether the Output panel is rendered as a peer right-side
    /// `SidePanel` (alongside Advanced, not inside it). Toggled by the
    /// "Output" button in the toolbar; visible only when
    /// `output_targets.len() >= 1`. The minimum-viable "Output mode pill":
    /// the full Warp/Mask/Content cluster + canvas mode-tint border that
    /// the spec describes is M3 follow-on work that hasn't landed yet, so
    /// for v0.4 we ship a plain toolbar toggle that reaches the same
    /// `OutputPanel` content. Advanced's embedded OutputPanel header
    /// (P0.8.1) co-exists with this — the two paths reach identical UI.
    #[cfg(feature = "v3")]
    pub output_panel_open: bool,
}

pub enum ControlPanelAction {
    None,
    /// Reload GPU layer runtime from `project.layers` paths.
    // Under v3, layer adds route through advanced.rs / layer_strip; under v2
    // show_layers_tab returns this variant. app.rs matches it in all builds.
    #[cfg_attr(feature = "v3", allow(dead_code))]
    RebuildLayers,
    /// Operator clicked "recall" on a scene slot. App routes through the same
    /// scheduling logic as the keyboard hotkey so crossfade
    /// (`Project::crossfade_duration_s`) is honored from the UI too.
    // Under v3, scene recall is keyboard-driven; the v2 Scenes tab returns this
    // variant. app.rs matches it in all builds.
    #[cfg_attr(feature = "v3", allow(dead_code))]
    SceneRecall(usize),
    /// 003-T3.4: toolbar Undo button clicked. App drains through `undo_stack.undo`.
    #[cfg(feature = "v3")]
    RequestUndo,
    /// 003-T3.4: toolbar Redo button clicked. App drains through `undo_stack.redo`.
    #[cfg(feature = "v3")]
    RequestRedo,
    /// 003-T3.23: show-day strip button clicked. App routes through
    /// `apply_command` so telemetry sees one canonical event regardless
    /// of whether the action came from the keyboard or this button.
    #[cfg(feature = "v3")]
    EmitCommand(crate::controls::Command),
    /// 003-T4.8: toolbar "Save" button clicked. App writes the project to
    /// the current `project_file_path`, or falls back to a Save-as picker
    /// if no path is known yet.
    #[cfg(feature = "v3")]
    RequestSave,
    /// 003-T4.8: toolbar "Save as…" button clicked. App opens the rfd Save
    /// dialog via `Command::OpenSaveAsPicker`.
    #[cfg(feature = "v3")]
    RequestSaveAs,
    /// 003-T4.17: toolbar "Go live" button clicked while in `Editing`.
    /// App transitions `Editing → GoLive` and calls `set_fullscreen(true)`.
    #[cfg(feature = "v3")]
    RequestEnterGoLive,
    /// 003-T4.17: toolbar "Stop" button clicked while in `GoLive`.
    /// App transitions `GoLive → Editing` and calls `set_fullscreen(false)`.
    #[cfg(feature = "v3")]
    RequestExitGoLive,
    /// 003-T4.16a: toolbar "Preview" button clicked while preview is closed.
    /// App opens a `PreviewWindow` on the primary display.
    #[cfg(feature = "v3")]
    RequestOpenPreview,
    /// 003-T4.16a: toolbar "Close preview" button clicked while preview is open.
    /// App drops `EditingState::preview_window`.
    #[cfg(feature = "v3")]
    RequestClosePreview,
    /// 004-V31.8.2: thumbnail in the control-window header was clicked while
    /// the preview window is already open. App brings the preview window to
    /// front via `Window::focus_window()`.
    #[cfg(feature = "v3")]
    FocusPreview,
}

/// Per-frame inputs from the App into the control panel render. Bundled so the
/// signature doesn't grow every time we add another piece of state the panel
/// needs to read.
pub struct ControlPanelInputs {
    /// Live scene preview registered with egui as a native texture (T-M9-01).
    /// `None` when registration failed or the preview isn't available yet.
    pub scene_texture: Option<egui::TextureId>,
    /// Output framebuffer dimensions, used to compute the preview's aspect
    /// (T-M9-02). `(0, 0)` is treated as 16:9 fallback.
    pub output_size: (u32, u32),
    /// 003-T2.17: time elapsed since the editor began constructing, used
    /// by `show_scene_tab` to animate the "Connecting to projector…"
    /// dot pattern while `scene_texture` is `None`. `None` for v2 builds
    /// (the connecting copy is v3-only); the v2 default still shows the
    /// dev-log line so existing log-scraping habits don't silently break.
    #[cfg(feature = "v3")]
    pub session_age: std::time::Duration,
    /// 003-T3.4: `true` when `undo_stack` has entries — disables the Undo
    /// toolbar button when the stack is empty so the operator gets immediate
    /// visual feedback. v3-only; v2 has no undo stack.
    #[cfg(feature = "v3")]
    pub can_undo: bool,
    /// 003-T3.4: `true` when `undo_stack` has redo entries — disables the
    /// Redo toolbar button when nothing has been undone yet.
    #[cfg(feature = "v3")]
    pub can_redo: bool,
    /// 003-T3.23: snapshot of the four output-state booleans the show-day
    /// strip reads. Populated from `state.output.state` at the call site in
    /// `app.rs` so the UI never borrows `EditingState` directly.
    #[cfg(feature = "v3")]
    pub output_state_snapshot: crate::windows::show_day_strip::OutputStateSnapshot,
    /// 003-T4.9: display name derived from `project_file_path` file stem, or
    /// `"Untitled show"` when no file path is set. Derived at the call site
    /// in `app.rs` so it doesn't need to be stored on `EditingState`.
    #[cfg(feature = "v3")]
    pub project_name: String,
    /// 003-T4.9: `true` when the project has unsaved mutations. The toolbar
    /// shows a "• " prefix on the project name when this is set (T4.10).
    #[cfg(feature = "v3")]
    pub dirty: bool,
    /// 003-T4.4 — active crossfade indicator for the cue strip.
    /// `Some((target_scene_idx, progress_0_to_1))` while a crossfade is
    /// in flight; `None` when no fade is active.
    #[cfg(feature = "v3")]
    pub crossfade_progress: Option<(usize, f32)>,
    /// 003-T4.17 — `true` when the app is in `AppState::GoLive`. The toolbar
    /// uses this to show "Stop" instead of "Go live" on the same button slot.
    #[cfg(feature = "v3")]
    pub is_go_live: bool,
    /// 003-T4.16a — `true` when `EditingState::preview_window` is `Some`.
    /// The toolbar uses this to toggle the Preview button label.
    #[cfg(feature = "v3")]
    pub has_preview: bool,
    /// 003-T4.11 — human-readable monitor names from
    /// `crate::monitors::list()` (macOS: `NSScreen::localizedName`; other
    /// platforms: winit's `MonitorHandle::name()` or a numeric fallback).
    /// Index `i` corresponds to `event_loop.available_monitors().nth(i)`.
    /// Used by the Advanced > Project section to show e.g.
    /// `"Output: BenQ TH685"` instead of the bare index.
    #[cfg(feature = "v3")]
    pub monitor_names: Vec<String>,
    /// V31.7.2 — live BPM telemetry from the clock: current BPM, last tap
    /// source, and last tap timestamp. Used by the toolbar BPM HUD badge.
    #[cfg(feature = "v3")]
    pub bpm_telemetry: crate::clock::BpmTelemetry,
    /// V31.7.3 — index of the cue currently armed-and-waiting for a
    /// quantize boundary, if any. `None` when no cue is pending or when
    /// quantize is off. The cue strip renders the pending tile with the
    /// same "armed" (accent-border) visual used for a crossfade target so
    /// the operator can see the pending-fire at a glance.
    #[cfg(feature = "v3")]
    pub pending_cue: Option<usize>,
    /// P1.6.1 — texture-upload queue's running drop count. Snapshotted
    /// at the call site from `state.texture_upload_queue.dropped_count()`
    /// so the diagnostics widget can aggregate it with the audio
    /// counter without holding a reference into `EditingState`. Closes
    /// P0.3.2's deferred wiring (the video worker is now a real producer).
    #[cfg(feature = "v3")]
    pub texture_upload_dropped: u64,
}

/// Render the control panel. Mutates `project` in place.
pub fn show(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    scene: &mut SceneEditorState,
    inputs: &ControlPanelInputs,
) -> ControlPanelAction {
    let mut action = ControlPanelAction::None;

    // 003-T3.1: under `--features v3`, the canvas IS the control window.
    // Hide the v2 tab strip; the scene preview becomes the central
    // surface. The previous Effects/Layers/Mapping/Scenes tab contents
    // move to the Advanced disclosure panel (T3.11+ refines layout;
    // T3.4 wires a toolbar button to toggle `advanced_open`).
    //
    // v2 builds keep the tabbed UI unchanged.
    // 003-T3.6: Mapping tab removed; v2 strip is now Scene/Effects/Layers/Scenes.
    #[cfg(not(feature = "v3"))]
    egui::Panel::top("rmap_tabs")
        .resizable(false)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut st.tab, ControlTab::Scene, "Scene");
                ui.selectable_value(&mut st.tab, ControlTab::Effects, "Effects");
                ui.selectable_value(&mut st.tab, ControlTab::Layers, "Layers");
                ui.selectable_value(&mut st.tab, ControlTab::Scenes, "Scenes");
            });
        });

    // 003-T3.1: Advanced side panel — collapsible right-edge container
    // for the legacy tab content during the canvas-merge transition.
    // Closed by default; T3.4's Advanced button + T3.11+ content
    // reorganisation populate the proper UI. For now, stack the
    // Effects/Layers/Mapping/Scenes editors so an operator running v3
    // can still reach those controls while the proper inspector
    // (T3.3) and Advanced disclosure (T3.11) are in flight.

    // 003-T3.3 + P1.UX: selection-driven inspector. The Layer-selection
    // controls moved into the Controls window's "Selected layer"
    // section; this surface stays around for **edit-mode selections**
    // (warp corners + mask vertices), where the affordances are a
    // small read-out + a Reset button.
    //
    // P1.UX (second pass): re-mounted as a small floating
    // `egui::Window` instead of a right-edge `SidePanel`. When the
    // operator clicked a mask vertex the old SidePanel would appear
    // and squeeze the canvas left by 280 px on the next frame —
    // visually the image *jumped* during selection. A floating window
    // sits over the canvas without consuming layout width, so the
    // canvas stays still.
    #[cfg(feature = "v3")]
    {
        let inspector_visible = matches!(
            scene.selected,
            Some(crate::windows::scene_editor::Selection::WarpCorner { .. })
                | Some(crate::windows::scene_editor::Selection::MaskVertex { .. })
                | Some(crate::windows::scene_editor::Selection::SourceRect { .. })
        );
        if inspector_visible {
            let ctx = ui.ctx().clone();
            egui::Window::new(
                egui::RichText::new("Selection details").color(crate::windows::theme::ACCENT),
            )
            .id(egui::Id::new("rmap_inspector_window"))
            .resizable(true)
            .default_width(260.0)
            .default_height(160.0)
            .show(&ctx, |ui| {
                crate::windows::inspector::show(ui, project, st, scene);
            });
        }
    }

    // 003-T3.11 + P1.UX: structured **Controls** window (was the
    // "Advanced" SidePanel pre-P1.UX). Now floats over the canvas as
    // an egui Window (glossary-style) so it doesn't steal a fixed
    // right column from the scene preview when the operator wants
    // more canvas room. Toggled by the toolbar button.
    //
    // Why a Window instead of a SidePanel:
    //  • The panel grew to contain Master + Display output + every
    //    per-layer control + Project + OSC bindings + Diagnostics,
    //    so it's now the operator's primary work surface — not an
    //    occasional "advanced" disclosure.
    //  • Floating + resizable lets the operator size it to their
    //    workflow (tall + narrow for quick tweaks; short + wide for
    //    side-by-side with the canvas).
    //  • Window's built-in close button + drag-to-move match what
    //    the glossary already does, so the two floating surfaces
    //    feel consistent.
    #[cfg(feature = "v3")]
    if st.controls_open {
        let ctx = ui.ctx().clone();
        let mut still_open = true;
        egui::Window::new(egui::RichText::new("Controls").color(crate::windows::theme::ACCENT))
            .id(egui::Id::new("rmap_controls_window"))
            .open(&mut still_open)
            .resizable(true)
            .default_width(380.0)
            .default_height(640.0)
            .show(&ctx, |ui| {
                // Esc closes the window.
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    st.controls_open = false;
                    return;
                }
                let act = crate::windows::controls::show(
                    ui,
                    project,
                    st,
                    scene,
                    &inputs.monitor_names,
                    inputs.texture_upload_dropped,
                );
                match act {
                    ControlPanelAction::None => {}
                    _ => action = act,
                }
            });
        // Window's close button writes `still_open = false`; mirror
        // that back into operator state. (We can't pass
        // `&mut st.controls_open` directly because the closure also
        // mutably borrows `st` via the inner `crate::windows::controls::show`
        // call.)
        if !still_open {
            st.controls_open = false;
        }
    }

    // P0.7.5 — Output panel as a peer right-side SidePanel. Opens when
    // the toolbar's "Output" toggle sets `st.output_panel_open`. Animated
    // width mirrors the Advanced panel so the open/close feel matches.
    // Mutual exclusion with Advanced's per-output sections is enforced
    // inside `advanced::show` (it checks `st.output_panel_open` and
    // skips the duplicate surfaces) — keeps egui Grid IDs unique.
    #[cfg(feature = "v3")]
    {
        const OUTPUT_PANEL_MAX_WIDTH: f32 = 360.0;
        let out_anim_id = ui.id().with("output_panel_open");
        let out_t =
            anim::animate_bool_to(ui, out_anim_id, st.output_panel_open, anim::TRANSITION_MS);
        let out_width = out_t * OUTPUT_PANEL_MAX_WIDTH;

        if out_width >= 1.0 {
            egui::SidePanel::right("rmap_output_panel")
                .resizable(false)
                .exact_width(out_width)
                .show_inside(ui, |ui| {
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        st.output_panel_open = false;
                        return;
                    }
                    ui.heading("Output");
                    ui.separator();
                    let act =
                        crate::windows::output_panel::show(ui, project, st, &inputs.monitor_names);
                    if !matches!(act, ControlPanelAction::None) {
                        action = act;
                    }
                });
        }
    }

    // 003-T3.2: layer thumbnail strip on the left edge. Only under v3;
    // v2 builds keep the tabbed UI unchanged. Rendered BEFORE the
    // CentralPanel so egui's panel layout claims the left edge first.
    #[cfg(feature = "v3")]
    egui::SidePanel::left("rmap_layer_strip")
        .resizable(false)
        // 88 px was tight; even 108 still clipped under egui's default
        // SidePanel inner_margin (~8 px each side). 120 px + a zeroed
        // Frame margin lets layer_strip use the full rail width for its
        // row allocations without the panel chrome stealing ~16 px.
        .exact_width(120.0)
        .frame(
            egui::Frame::new()
                .fill(ui.style().visuals.panel_fill)
                .inner_margin(0.0)
                .outer_margin(0.0),
        )
        .show_inside(ui, |ui| {
            crate::windows::layer_strip::show(ui, project, st, scene);
        });

    // 003-T3.23–T3.25: show-day strip at the bottom of the canvas.
    // Claimed before the CentralPanel so the panel layout reserves
    // the bottom edge. Visible in both Editing and GoLive — both
    // AppState arms hit this code path (see app.rs:3593).
    //
    // NOTE: egui claims bottom panels from outermost (first declared) to
    // innermost (last declared). show_day_strip is declared FIRST so it
    // occupies the outermost bottom edge. cue_strip is declared SECOND so
    // it sits directly above show_day_strip, i.e. between it and the canvas.
    #[cfg(feature = "v3")]
    egui::TopBottomPanel::bottom("rmap_show_day_strip")
        .resizable(false)
        .show_inside(ui, |ui| {
            if let Some(cmd) =
                crate::windows::show_day_strip::show(ui, &inputs.output_state_snapshot)
            {
                action = ControlPanelAction::EmitCommand(cmd);
            }
        });

    // 004-V31.9.2: audio bands strip — 8 vertical FFT-magnitude bars visible
    // only when an audio source is active.  Declared after show_day_strip
    // (so it sits above show-day) and before cue_strip (so cue strip sits
    // above it), per roadmap.md §8 ideal layout.
    //
    // The TopBottomPanel is only claimed when is_audio_active() is true so
    // no vertical space is wasted when audio is inactive (choice A per spec).
    #[cfg(all(feature = "v3", feature = "audio"))]
    if crate::modulators::audio::is_audio_active() {
        egui::TopBottomPanel::bottom("rmap_audio_bands_strip")
            .resizable(false)
            .exact_size(crate::windows::audio_bands_strip::STRIP_HEIGHT)
            .show_inside(ui, |ui| {
                if let Some(band_idx) = crate::windows::audio_bands_strip::show(ui) {
                    // V31.9.2: drag-source emit. No target accepts the drop in
                    // v3.1 — the parameter-binding picker ships in v0.4.
                    tracing::info!(
                        target: "rmap::ux",
                        event = "audio_band_drag_started",
                        band = band_idx,
                    );
                }
            });
    }

    // 003-T4.2–T4.5: cue strip — horizontal row of scene tiles above the
    // show-day strip. Declared after show_day_strip so egui places it
    // between show_day_strip and the canvas (inner panel).
    #[cfg(feature = "v3")]
    egui::TopBottomPanel::bottom("rmap_cue_strip")
        .resizable(false)
        .exact_size(crate::windows::cue_strip::STRIP_HEIGHT)
        .show_inside(ui, |ui| {
            if let Some(cmd) = crate::windows::cue_strip::show(
                ui,
                project,
                &mut st.thumbnail_cache,
                inputs.crossfade_progress,
                // V31.7.3: pending-quantize cue index for armed-tile visual.
                #[cfg(feature = "v3")]
                inputs.pending_cue,
            ) {
                action = ControlPanelAction::EmitCommand(cmd);
            }
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        // 003-T3.1: under v3 the canvas is always the central surface.
        // T3.4 will replace this hard-coded scene render with toolbar +
        // mode-aware drawing.
        #[cfg(feature = "v3")]
        {
            // 003-T3.4: real toolbar replacing the T3.1 temporary checkbox.
            if let Some(req) = crate::windows::toolbar::show(ui, project, st, scene, inputs) {
                action = req;
            }
            show_scene_tab(ui, project, st, scene, inputs);
        }
        // 003-T3.6: Mapping arm deleted; show_mapping_tab is gone.
        #[cfg(not(feature = "v3"))]
        match st.tab {
            ControlTab::Scene => show_scene_tab(ui, project, st, scene, inputs),
            ControlTab::Effects => show_effects_tab(ui, project, st),
            ControlTab::Layers => {
                if matches!(show_layers_tab(ui, project, st), ControlPanelAction::RebuildLayers) {
                    action = ControlPanelAction::RebuildLayers;
                }
            }
            ControlTab::Scenes => action = show_scenes_tab(ui, project, st),
        }

        // 003-T3.12 / T3.11: under v3 these blocks move into
        // Advanced (Master section + Project section). v2 keeps them here.
        #[cfg(not(feature = "v3"))]
        {
            ui.add_space(8.0);
            egui::CollapsingHeader::new("Project file")
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("Save / load JSON projects (*.rmap.json). Layer SVG paths are stored as-is.");
                    ui.horizontal(|ui| {
                        let edit = egui::TextEdit::singleline(&mut st.project_save_path)
                            .desired_width(340.0)
                            .hint_text("my_show.rmap.json");
                        let resp = ui.add(edit);
                        if resp.changed() {
                            st.project_save_message.clear();
                        }
                        if ui.button("Save").clicked() {
                            let trim = st.project_save_path.trim();
                            if trim.is_empty() {
                                st.project_save_message = "Enter a path ending in .rmap.json".into();
                            } else if !trim.ends_with(".rmap.json") {
                                st.project_save_message =
                                    "Filename should end with .rmap.json".into();
                            } else {
                                match project.save(Path::new(trim)) {
                                    Ok(()) => {
                                        st.project_save_message =
                                            format!("Saved to {}", trim);
                                    }
                                    Err(e) => {
                                        st.project_save_message = format!("Save failed: {e}");
                                    }
                                }
                            }
                        }
                    });
                    if !st.project_save_message.is_empty() {
                        ui.label(&st.project_save_message);
                    }
                    #[cfg(feature = "v3")]
                    {
                        if let Some(new) =
                            command_checkbox(ui, "Windowed output", project.output_windowed)
                        {
                            st.pending_mutations
                                .push(project.set_output_windowed_mutation(new));
                        }
                    }
                    #[cfg(not(feature = "v3"))]
                    {
                        ui.checkbox(&mut project.output_windowed, "Windowed output");
                    }
                    ui.label(
                        "When saved in the project: opens a 1280×720 window on the output monitor instead of fullscreen. Restart rmap to apply.",
                    );
                });

            ui.add_space(8.0);
            egui::CollapsingHeader::new("Master (gamma)")
                .default_open(true)
                .show(ui, |ui| {
                    ui.add(egui::Slider::new(&mut project.gamma, 0.2..=4.0).text("gamma"));
                    ui.add(egui::Slider::new(&mut project.brightness, -1.0..=1.0).text("brightness"));
                    ui.add(egui::Slider::new(&mut project.contrast, 0.0..=4.0).text("contrast"));
                });
        }
    });

    // 003-T5.12 — in-app Glossary window. Rendered here (using the egui
    // context, not `ui`) so it floats over the panel layout. Toggled by
    // the "Glossary" button added to the toolbar's right section.
    #[cfg(feature = "v3")]
    show_glossary_window(ui.ctx(), &mut st.glossary_open);

    action
}

/// Show the live scene preview + handle direct-manipulation input
/// (T-M9-02 + T-M10-03). The preview is `warp_rt` registered as an egui
/// native texture; click-and-drag inside it selects + moves layers.
fn show_scene_tab(
    ui: &mut Ui,
    project: &mut Project,
    #[cfg_attr(not(feature = "v3"), allow(unused_variables))] st: &mut ControlPanelState,
    scene: &mut SceneEditorState,
    inputs: &ControlPanelInputs,
) {
    // 003-T3.8 — v3: per-mode instruction banner replaces the static v2 label.
    // v2: keep the original verbose hint unchanged.
    #[cfg(feature = "v3")]
    scene_editor::mode_banner(ui, scene);
    #[cfg(not(feature = "v3"))]
    ui.label(
        "Live preview. Click a layer to select; drag to move; Shift-drag to scale; Alt-drag to rotate. Drag a mask vertex to move; double-click an edge to insert; Shift-click a vertex to delete. Drop SVG / PNG / JPG to add a layer.",
    );
    // P1.UX: the "selected: layer N (id)" label was a second
    // conditional row above the canvas; appearing on selection
    // pushed the canvas down a line → operator reported a visible
    // canvas jump on every layer click. The layer is already
    // surfaced redundantly through:
    //   • the left rail (selected row gets the accent ring)
    //   • the colored layer outline on the canvas
    //   • the bold layer-id at the top of the Controls window's
    //     "Selected layer" section
    // — so removing this above-canvas row is a clean loss-of-noise.
    ui.add_space(4.0);
    let Some(tex_id) = inputs.scene_texture else {
        // 003-T2.17 — friendly transition copy with animated dots while
        // the output surface and scene-texture registration race the
        // first paint. The egui repaint cadence drives the dot count
        // off `session_age`, so the copy reads as breathing rather
        // than spinning. The 5 s escalation toast lives in app.rs
        // where the toast queue is owned (see `connecting_toast_emitted`).
        #[cfg(feature = "v3")]
        {
            let dots = match inputs.session_age.as_millis() / 400 % 4 {
                0 => "",
                1 => ".",
                2 => "..",
                _ => "...",
            };
            ui.label(format!("Connecting to projector{dots}"));
            ui.ctx().request_repaint();
        }
        #[cfg(not(feature = "v3"))]
        ui.label("(scene preview not yet registered — output window not initialized)");
        return;
    };
    let (out_w, out_h) = inputs.output_size;
    let aspect = if out_w > 0 && out_h > 0 {
        out_w as f32 / out_h as f32
    } else {
        16.0 / 9.0
    };

    let avail = ui.available_size();
    let mut w = avail.x.max(160.0);
    let mut h = w / aspect;
    if h > avail.y.max(120.0) {
        h = avail.y.max(120.0);
        w = h * aspect;
    }
    // Sense click + drag + click for double-click detection.
    let (resp, painter) = ui.allocate_painter(
        egui::vec2(avail.x, h.max(120.0)),
        egui::Sense::click_and_drag(),
    );
    let outer = resp.rect;
    let inner = egui::Rect::from_center_size(outer.center(), egui::vec2(w, h));

    // 003-T3.9 — mode-aware cursor: only set while the pointer is inside
    // the scene preview rect so leaving the canvas restores the OS default.
    #[cfg(feature = "v3")]
    if resp.hovered() {
        ui.output_mut(|out| out.cursor_icon = scene_editor::cursor_for_mode(scene.mode));
    }

    // T-M11-03: double-click on a mask edge inserts a new vertex at the
    // click point, between the two endpoints. T-M11-04: shift-click on a
    // mask vertex deletes it (refused below 4 vertices to keep the SDF
    // baker happy — `<3` collapses the mask to "no mask").
    let pointer_now = ui.input(|i| i.pointer.hover_pos());
    if let Some(pos) = pointer_now {
        if resp.double_clicked() {
            if let Some((w_idx, after, point)) = scene_editor::hit_mask_edge(project, pos, inner) {
                let insert_at = project
                    .layers
                    .get(w_idx)
                    .map(|l| (after + 1).min(l.warp.mask_polygon.len()));
                if let Some(insert_at) = insert_at {
                    #[cfg(feature = "v3")]
                    {
                        st.pending_mutations.push(
                            crate::project::command::Mutation::AddLayerMaskVertex {
                                layer_idx: w_idx,
                                position: insert_at,
                                point,
                            },
                        );
                        scene.selected = Some(scene_editor::Selection::MaskVertex {
                            warp: w_idx,
                            idx: insert_at,
                        });
                    }
                    #[cfg(not(feature = "v3"))]
                    if let Some(layer) = project.layers.get_mut(w_idx) {
                        layer.warp.mask_polygon.insert(insert_at, point);
                        scene.selected = Some(scene_editor::Selection::MaskVertex {
                            warp: w_idx,
                            idx: insert_at,
                        });
                    }
                }
            }
        }
        if resp.clicked() && ui.input(|i| i.modifiers.shift) {
            if let Some((w_idx, v_idx)) = scene_editor::hit_mask_vertex(project, pos, inner) {
                let len = project.layers.get(w_idx).map(|l| l.warp.mask_polygon.len());
                if let Some(len) = len {
                    if len > 3 {
                        // ≥3 guard preserved on both code paths.
                        #[cfg(feature = "v3")]
                        {
                            st.pending_mutations.push(
                                crate::project::command::Mutation::RemoveLayerMaskVertex {
                                    layer_idx: w_idx,
                                    idx: v_idx,
                                },
                            );
                            scene.selected = None;
                            scene.drag = None;
                        }
                        #[cfg(not(feature = "v3"))]
                        if let Some(layer) = project.layers.get_mut(w_idx) {
                            layer.warp.mask_polygon.remove(v_idx);
                            scene.selected = None;
                            scene.drag = None;
                        }
                    }
                }
            }
        }
    }
    painter.rect_filled(outer, egui::CornerRadius::ZERO, theme::BG_BACKGROUND);
    painter.image(
        tex_id,
        inner,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
    painter.rect_stroke(
        inner,
        egui::CornerRadius::ZERO,
        egui::Stroke::new(1.0, theme::BG_PANEL.linear_multiply(3.5)),
        egui::StrokeKind::Outside,
    );

    // 003-T2.16 + T2.12 — empty-state and active-drag hints. When the
    // project carries no layers the scene texture renders solid black,
    // so we overlay a pulsing dashed drop zone with the "drop a photo
    // or SVG" copy. When the OS reports a file dragged over the rmap
    // window (`hovered_files` non-empty) we paint the same overlay
    // even if layers are present — that way the operator gets visual
    // confirmation that the canvas is the drop target before they
    // release.
    //
    // The hint dismisses naturally the moment a layer is added or the
    // drag ends: the next frame finds the predicate false and skips
    // the overlay.
    //
    // Gated on `v3` because the `windows::primitives` module itself is
    // v3-only — the v2 default build still ships the bare scene tab.
    #[cfg(feature = "v3")]
    {
        let hovered_files = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        if project.layers.is_empty() || hovered_files {
            crate::windows::primitives::paint_drop_target(
                &painter,
                ui.ctx(),
                inner,
                "Drop a photo or SVG here to begin.",
            );
        }
    }

    // Per-layer colored outlines for every enabled layer (selected gets a
    // thicker stroke, same color). Painted before mask overlays so the
    // mask handles sit on top.
    scene_editor::paint_layer_outlines(project, scene, &painter, inner);
    scene_editor::paint_mask_overlays(project, scene, &painter, inner);
    // 003-T3.5 — paint the selected layer's warp grid only when the
    // operator is in Warp mode. Other layers' grids stay hidden so the
    // canvas tracks the operator's per-layer focus.
    #[cfg(feature = "v3")]
    if scene.mode == scene_editor::EditMode::Warp {
        let warp_layer = match scene.selected {
            Some(scene_editor::Selection::Layer(idx)) => Some(idx),
            Some(scene_editor::Selection::WarpCorner { warp, .. }) => Some(warp),
            _ => None,
        };
        if let Some(idx) = warp_layer {
            scene_editor::paint_warp_grid_overlay(project, idx, scene, &painter, inner);
        }
        // 003-T3.10 — show a faint magnetic-zone indicator while a
        // corner is being dragged near a framebuffer edge.
        scene_editor::paint_warp_snap_indicator(project, scene, &painter, inner);
    }

    // Route click + drag through the scene editor. Pointer pos is in
    // egui screen space; the editor converts to inner-rect-relative
    // normalized coords before mutating the project.
    let (pointer, modifiers, esc) = ui.input(|i| {
        (
            i.pointer.hover_pos(),
            i.modifiers,
            i.key_pressed(egui::Key::Escape),
        )
    });
    #[cfg(feature = "v3")]
    {
        let emitted =
            scene_editor::handle_scene_input(&resp, project, scene, inner, pointer, modifiers);
        if let Some(m) = emitted {
            st.pending_mutations.push(m);
        }
    }
    #[cfg(not(feature = "v3"))]
    scene_editor::handle_scene_input(&resp, project, scene, inner, pointer, modifiers);
    if esc {
        scene.selected = None;
        scene.drag = None;
    }

    // Sidebar properties for the selected layer (T-M10-05). Lives below the
    // preview canvas so it doesn't compete for horizontal space when the
    // window is narrow.
    if let Some(scene_editor::Selection::Layer(idx)) = scene.selected {
        if let Some(layer) = project.layers.get_mut(idx) {
            ui.add_space(6.0);
            let header = format!("Selected: {}", layer.id);
            egui::CollapsingHeader::new(header)
                .default_open(true)
                .show(ui, |ui| {
                    ui.label(
                        "Drag in the preview to move; Shift-drag to scale; Alt-drag to rotate; Esc to deselect.",
                    );
                    let (mut t, mut s, mut r) = scene_editor::effective_static_transform(layer);
                    let mut changed = false;
                    changed |= ui
                        .add(egui::Slider::new(&mut t[0], -1.0..=1.0).text("translate x"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut t[1], -1.0..=1.0).text("translate y"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut s[0], 0.05..=4.0).text("scale x"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut s[1], 0.05..=4.0).text("scale y"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut r, -180.0..=180.0).text("rotate (deg)"))
                        .changed();
                    if changed {
                        scene_editor::mutate_transform_effect(
                            layer,
                            |trans, rot, sx, sy| {
                                *trans = t;
                                *sx = Modulator::Static(s[0]);
                                *sy = Modulator::Static(s[1]);
                                *rot = Modulator::Static(r);
                            },
                        );
                    }
                    ui.add(
                        egui::Slider::new(&mut layer.opacity, 0.0..=1.0).text("opacity"),
                    );
                });
        }
    }
}

// T3.14: under v3 the effect chain lives in advanced::show. The v2
// Effects tab retains this function; v3 gating prevents "dead code" lint.
#[cfg(not(feature = "v3"))]
fn show_effects_tab(ui: &mut Ui, project: &mut Project, st: &mut ControlPanelState) {
    if project.layers.is_empty() {
        ui.label("No layers — open an SVG as the first argument.");
        return;
    }
    if !st.presets_loaded {
        st.presets = load_presets_from_disk();
        st.presets_loaded = true;
    }
    st.selected_layer = st
        .selected_layer
        .min(project.layers.len().saturating_sub(1));
    ui.label(
        "Sliders apply to the selected layer only; each layer has its own effect chain. Warp, gamma, and master brightness/contrast run after all layers are composited.",
    );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Layer:");
        egui::ComboBox::from_id_salt("layer_pick")
            .selected_text(project.layers[st.selected_layer].id.clone())
            .show_ui(ui, |ui| {
                for (i, layer) in project.layers.iter().enumerate() {
                    if ui
                        .selectable_label(st.selected_layer == i, &layer.id)
                        .clicked()
                    {
                        st.selected_layer = i;
                    }
                }
            });
    });

    // Preset picker (T-M7-08). Picks one of `st.presets` and applies its
    // entire effect chain to the selected layer on click. Keep the operator
    // far from per-parameter slider hunting — that's the usability play.
    ui.horizontal(|ui| {
        ui.label("Preset:");
        if st.presets.is_empty() {
            ui.label("(none — assets/presets/*.json not found)");
        } else {
            st.preset_picker_index = st.preset_picker_index.min(st.presets.len() - 1);
            egui::ComboBox::from_id_salt("preset_pick")
                .selected_text(st.presets[st.preset_picker_index].name.clone())
                .show_ui(ui, |ui| {
                    for (i, preset) in st.presets.iter().enumerate() {
                        if ui
                            .selectable_label(st.preset_picker_index == i, &preset.name)
                            .clicked()
                        {
                            st.preset_picker_index = i;
                        }
                    }
                });
            if ui.button("Apply").clicked() {
                #[cfg(feature = "v3")]
                {
                    let new = st.presets[st.preset_picker_index].effects.clone();
                    st.pending_mutations
                        .push(project.set_layer_effects_mutation(st.selected_layer, new));
                }
                #[cfg(not(feature = "v3"))]
                {
                    project.layers[st.selected_layer].effects =
                        st.presets[st.preset_picker_index].effects.clone();
                }
            }
        }
        if ui.button("Reload").clicked() {
            st.presets = load_presets_from_disk();
            st.preset_picker_index = 0;
        }
    });

    let layer_idx = st.selected_layer;
    let effects_len = project.layers[layer_idx].effects.len();
    ui.heading("Effect chain");
    ui.add_space(4.0);
    // 003-T1.21: collect staged EffectChanges emitted by show_effect.
    // Iteration uses indices so the borrow on project.layers[layer_idx].effects
    // is fully released after the loop — allowing the subsequent .clone() for
    // the SetLayerEffects mutation. Under non-v3 the staged_changes vec is
    // omitted entirely; show_effect still returns Option<EffectChange> but the
    // caller ignores it.
    #[cfg(feature = "v3")]
    let mut staged_changes: Vec<(usize, EffectChange)> = Vec::new();
    for idx in 0..effects_len {
        let effect = &mut project.layers[layer_idx].effects[idx];
        egui::CollapsingHeader::new(effect_label(effect))
            .id_salt(idx)
            .default_open(true)
            .show(ui, |ui| {
                #[cfg(feature = "v3")]
                {
                    if let Some(change) = show_effect(ui, idx, effect, true, layer_idx) {
                        staged_changes.push((idx, change));
                    }
                }
                #[cfg(not(feature = "v3"))]
                {
                    // v2 has no Advanced panel; always show JSON for External.
                    // layer_idx is unused in non-v3 but present for signature uniformity.
                    let _ = show_effect(ui, idx, effect, true, layer_idx);
                }
            });
    }
    // 003-T1.21/T1.22: after the loop, apply staged changes.
    // T1.22: ModulatorSwitch emits SetModulator (per-slot, whole-enum Reverse);
    // field changes (TransformTranslate*) still funnel into a single SetLayerEffects.
    #[cfg(feature = "v3")]
    if !staged_changes.is_empty() {
        let mut field_changes: Vec<(usize, EffectChange)> = Vec::new();
        for (effect_idx, change) in staged_changes {
            match change {
                EffectChange::ModulatorSwitch {
                    effect_idx: ei,
                    field,
                    new,
                } => {
                    st.pending_mutations
                        .push(project.set_modulator_mutation(layer_idx, ei, field, new));
                }
                other => field_changes.push((effect_idx, other)),
            }
        }
        if !field_changes.is_empty() {
            let old = project.layers[layer_idx].effects.clone();
            let mut new = old.clone();
            for (effect_idx, change) in field_changes {
                if let Some(crate::effects::Effect::Transform { translate, .. }) =
                    new.get_mut(effect_idx)
                {
                    match change {
                        EffectChange::TransformTranslateX(v) => translate[0] = v,
                        EffectChange::TransformTranslateY(v) => translate[1] = v,
                        EffectChange::ModulatorSwitch { .. } => unreachable!(),
                    }
                }
            }
            st.pending_mutations.push(Mutation::SetLayerEffects(
                crate::project::command::SetLayerEffects {
                    layer_idx,
                    new,
                    old,
                },
            ));
        }
    }
}

// unique_layer_id is called only from show_layers_tab, which is v2-only.
#[cfg(not(feature = "v3"))]
fn unique_layer_id(project: &Project) -> String {
    let mut n = project.layers.len();
    loop {
        let id = format!("layer{n}");
        if !project.layers.iter().any(|l| l.id == id) {
            return id;
        }
        n += 1;
    }
}

// T3.2: under v3 the layer list lives in the layer_strip. v2 keeps this tab.
#[cfg(not(feature = "v3"))]
fn show_layers_tab(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
) -> ControlPanelAction {
    let mut action = ControlPanelAction::None;

    // 003-T2.14 — native "+ Add image" button. Sits above the typed-path
    // field so the operator's eye lands on the picker first; the typed
    // path stays available behind the v3-keep-typed-path opt-out and is
    // removed entirely in T-003-T2.15.
    #[cfg(feature = "v3")]
    if ui
        .button("+ Add image")
        .on_hover_text("Pick a JPG, PNG, or SVG to add as a new layer")
        .clicked()
    {
        if let Some(picked) = crate::windows::file_dialogs::pick_image_to_add() {
            // Same extension policy as the drop handler: SVG → svg
            // layer, raster → image layer, anything else falls through
            // to a friendly inline error. The picker's filter already
            // restricts choices to JPG/PNG/SVG, but operators can
            // switch the filter to "All files" in the native panel,
            // so we still validate.
            let ext = picked
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_ascii_lowercase());
            let id = unique_layer_id(project);
            let layer = match ext.as_deref() {
                Some("svg") => Some(crate::project::schema::layer_from_svg_path(
                    id,
                    picked.clone(),
                )),
                Some("png") | Some("jpg") | Some("jpeg") => Some(
                    crate::project::schema::layer_from_image_path(id, picked.clone()),
                ),
                _ => None,
            };
            match layer {
                Some(new_layer) => {
                    let position = project.layers.len();
                    st.selected_layer = position;
                    st.pending_mutations.push(Mutation::AddLayer {
                        layer: new_layer,
                        position,
                    });
                    st.new_layer_path_input.clear();
                    st.add_layer_error.clear();
                    action = ControlPanelAction::RebuildLayers;
                    tracing::info!(
                        target: "rmap::ux",
                        event = "layer_added_via_picker",
                        path = %picked.display(),
                    );
                }
                None => {
                    st.add_layer_error =
                        "That file type isn't supported yet. Try a JPG, PNG, or SVG.".into();
                }
            }
        }
    }

    // 003-T2.15 — typed-path "Add layer" field is removed under v3
    // now that drag-drop (T2.12) and the "+ Add image" picker (T2.14)
    // both work and are easier to reach. The v2 default build still
    // ships the typed path while v3 stabilises; both branches share
    // the `add_layer_error` field so the picker's "unsupported file"
    // message renders below either flow.
    #[cfg(not(feature = "v3"))]
    {
        ui.label("Add an SVG as a new compositor layer (bottom list order = draw order).");
        ui.horizontal(|ui| {
            let edit = egui::TextEdit::singleline(&mut st.new_layer_path_input)
                .desired_width(280.0)
                .hint_text("/absolute/or/relative/path.svg");
            let resp = ui.add(edit);
            if resp.changed() {
                st.add_layer_error.clear();
            }
            if ui.button("Add layer").clicked() {
                let trimmed = st.new_layer_path_input.trim();
                if trimmed.is_empty() {
                    st.add_layer_error = "Enter path to an SVG file.".into();
                } else {
                    let p = PathBuf::from(trimmed);
                    let ext_ok = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
                    if !p.exists() {
                        st.add_layer_error = "Path does not exist.".into();
                    } else if !p.is_file() {
                        st.add_layer_error = "Path is not a file.".into();
                    } else if !ext_ok {
                        st.add_layer_error = "File must have extension .svg.".into();
                    } else if let Ok(canonical) = p.canonicalize() {
                        let id = unique_layer_id(project);
                        let new_layer = schema::layer_from_svg_path(id, canonical);
                        project.layers.push(new_layer);
                        st.selected_layer = project.layers.len() - 1;
                        st.new_layer_path_input.clear();
                        st.add_layer_error.clear();
                        action = ControlPanelAction::RebuildLayers;
                    } else {
                        st.add_layer_error = "Could not resolve path.".into();
                    }
                }
            }
        });
    }
    #[cfg(feature = "v3")]
    ui.label("Drop a JPG, PNG, or SVG onto the canvas, or use + Add image above. Order in this list = draw order, top to bottom.");
    if !st.add_layer_error.is_empty() {
        ui.colored_label(theme::DESTRUCTIVE, &st.add_layer_error);
    }

    ui.add_space(6.0);
    ui.label("Reorder (↑ / ↓). GPU layers reload after reorder.");
    let len = project.layers.len();
    let mut swap_up: Option<usize> = None;
    let mut swap_down: Option<usize> = None;

    for (i, layer) in project.layers.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                #[cfg(feature = "v3")]
                {
                    if let Some(new) = command_checkbox(ui, &layer.id, layer.enabled) {
                        st.pending_mutations.push(Mutation::SetLayerEnabled(
                            crate::project::command::SetLayerEnabled {
                                layer_idx: i,
                                new,
                                old: layer.enabled,
                            },
                        ));
                    }
                }
                #[cfg(not(feature = "v3"))]
                {
                    ui.checkbox(&mut layer.enabled, &format!("{}", layer.id));
                }
                ui.label(match &layer.kind {
                    crate::project::schema::LayerKind::Svg { svg_path } => {
                        svg_path.display().to_string()
                    }
                    crate::project::schema::LayerKind::Image { path, .. } => {
                        path.display().to_string()
                    }
                    crate::project::schema::LayerKind::Video { path, .. } => {
                        path.display().to_string()
                    }
                    crate::project::schema::LayerKind::FxLayer { preset_id, .. } => {
                        format!("FX preset: {preset_id}")
                    }
                    crate::project::schema::LayerKind::Ndi { source_name } => {
                        format!("NDI: {source_name}")
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label("blend");
                #[cfg(feature = "v3")]
                {
                    let current_mode = layer.blend_mode;
                    let mut staged: Option<BlendMode> = None;
                    egui::ComboBox::from_id_salt(("blend", i))
                        .selected_text(blend_label(current_mode))
                        .show_ui(ui, |ui| {
                            for mode in [
                                BlendMode::Normal,
                                BlendMode::Add,
                                BlendMode::Multiply,
                                BlendMode::Screen,
                            ] {
                                if ui
                                    .selectable_label(current_mode == mode, blend_label(mode))
                                    .clicked()
                                {
                                    staged = Some(mode);
                                }
                            }
                        });
                    if let Some(new) = staged {
                        if new != current_mode {
                            st.pending_mutations.push(Mutation::SetLayerBlendMode(
                                crate::project::command::SetLayerBlendMode {
                                    layer_idx: i,
                                    new,
                                    old: current_mode,
                                },
                            ));
                        }
                    }
                    if let Some(new) = command_slider(
                        ui,
                        &format!("opacity_{i}"),
                        "opacity",
                        layer.opacity,
                        0.0..=1.0,
                    ) {
                        st.pending_mutations.push(Mutation::SetLayerOpacity(
                            crate::project::command::SetLayerOpacity {
                                layer_idx: i,
                                new,
                                old: layer.opacity,
                            },
                        ));
                    }
                }
                #[cfg(not(feature = "v3"))]
                {
                    egui::ComboBox::from_id_salt(("blend", i))
                        .selected_text(blend_label(layer.blend_mode))
                        .show_ui(ui, |ui| {
                            for mode in [
                                BlendMode::Normal,
                                BlendMode::Add,
                                BlendMode::Multiply,
                                BlendMode::Screen,
                            ] {
                                if ui
                                    .selectable_label(layer.blend_mode == mode, blend_label(mode))
                                    .clicked()
                                {
                                    layer.blend_mode = mode;
                                }
                            }
                        });
                    ui.add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).text("opacity"));
                }
            });
            ui.horizontal(|ui| {
                if ui.button("↑").clicked() && i > 0 {
                    swap_up = Some(i);
                }
                if ui.button("↓").clicked() && i + 1 < len {
                    swap_down = Some(i);
                }
            });
        });
    }

    if let Some(i) = swap_up {
        #[cfg(feature = "v3")]
        {
            st.pending_mutations
                .push(Mutation::SwapLayers(crate::project::command::SwapLayers {
                    i,
                    j: i - 1,
                }));
        }
        #[cfg(not(feature = "v3"))]
        {
            project.layers.swap(i, i - 1);
        }
        if st.selected_layer == i {
            st.selected_layer = i - 1;
        } else if st.selected_layer == i - 1 {
            st.selected_layer = i;
        }
        action = ControlPanelAction::RebuildLayers;
    }
    if let Some(i) = swap_down {
        #[cfg(feature = "v3")]
        {
            st.pending_mutations
                .push(Mutation::SwapLayers(crate::project::command::SwapLayers {
                    i,
                    j: i + 1,
                }));
        }
        #[cfg(not(feature = "v3"))]
        {
            project.layers.swap(i, i + 1);
        }
        if st.selected_layer == i {
            st.selected_layer = i + 1;
        } else if st.selected_layer == i + 1 {
            st.selected_layer = i;
        }
        action = ControlPanelAction::RebuildLayers;
    }

    action
}

// T3.16: blend_label is called in show_layers_tab (v2-only now).
#[cfg(not(feature = "v3"))]
fn blend_label(m: BlendMode) -> &'static str {
    match m {
        BlendMode::Normal => "Normal",
        BlendMode::Add => "Add",
        BlendMode::Multiply => "Multiply",
        BlendMode::Screen => "Screen",
    }
}

// 003-T3.6: show_mapping_tab deleted. Mapping content has fully migrated to
// Advanced > Selected layer > Mapping (T3.15). Zone templates + corner-pin
// direct manipulation are reachable via the canvas (T3.5). The
// checker-pattern 480×270 placeholder canvas is gone.

// Scene slot UI lives in the v2 Scenes tab. Under v3 scenes are recalled
// via keyboard; a future task will surface this UI elsewhere.
#[cfg(not(feature = "v3"))]
fn show_scenes_tab(
    ui: &mut Ui,
    project: &mut Project,
    #[cfg_attr(not(feature = "v3"), allow(unused_variables))] st: &mut ControlPanelState,
) -> ControlPanelAction {
    let mut action = ControlPanelAction::None;
    ui.label("Slots 1–9 (keyboard recall). Save captures the full project state.");
    #[cfg(feature = "v3")]
    {
        if let Some(new) = command_slider(
            ui,
            "crossfade_duration_s",
            "crossfade duration (s)",
            project.crossfade_duration_s,
            0.0..=5.0,
        ) {
            st.pending_mutations
                .push(project.set_crossfade_duration_s_mutation(new));
        }
    }
    #[cfg(not(feature = "v3"))]
    {
        ui.add(
            egui::Slider::new(&mut project.crossfade_duration_s, 0.0..=5.0)
                .text("crossfade duration (s)"),
        );
    }
    ui.label(
        "Crossfade only fires when both scenes share the same layer paths in the same order; structural changes snap instantly.",
    );
    ui.add_space(4.0);
    for slot in 0..9 {
        ui.horizontal(|ui| {
            ui.label(format!("{}", slot + 1));
            if ui.button("save").clicked() {
                #[cfg(feature = "v3")]
                {
                    // Build the post-save scenes Vec without writing to project,
                    // then push as a SetProjectScenes mutation so undo can
                    // restore the slot list (including any placeholder additions).
                    let mut new = project.scenes.clone();
                    while new.len() <= slot {
                        new.push(Scene {
                            name: format!("scene{}", new.len() + 1),
                            snapshot: serde_json::json!({}),
                        });
                    }
                    new[slot].snapshot = snapshot(project);
                    st.pending_mutations
                        .push(project.set_project_scenes_mutation(new));
                }
                #[cfg(not(feature = "v3"))]
                {
                    while project.scenes.len() <= slot {
                        project.scenes.push(Scene {
                            name: format!("scene{}", project.scenes.len() + 1),
                            snapshot: serde_json::json!({}),
                            thumbnail: None,
                        });
                    }
                    project.scenes[slot].snapshot = snapshot(project);
                }
            }
            // Tell apart "recall is no-op because the slot was never
            // saved" from "recall fired but I missed the visual change."
            // A saved slot's snapshot is non-empty Object; a freshly-
            // pushed placeholder has `Object({})`. Empty placeholders
            // shouldn't be recallable.
            let has_data = project
                .scenes
                .get(slot)
                .map(|s| match &s.snapshot {
                    serde_json::Value::Object(m) => !m.is_empty(),
                    _ => false,
                })
                .unwrap_or(false);
            // App routes recall through the same scheduling logic as the
            // keyboard hotkey (T-M7-04). Don't `restore` here directly —
            // that would bypass crossfade scheduling.
            let recall = ui.add_enabled(has_data, egui::Button::new("recall"));
            if recall.clicked() {
                action = ControlPanelAction::SceneRecall(slot);
            }
            ui.label(if has_data { "saved" } else { "empty" });
        });
    }
    action
}

pub(super) fn effect_label(e: &Effect) -> &'static str {
    match e {
        Effect::Color { .. } => "Color",
        Effect::Tint { .. } => "Tint",
        Effect::Blur { .. } => "Blur",
        Effect::Transform { .. } => "Transform",
        Effect::External { .. } => "External",
    }
}

/// 003-T1.21 — staged change emitted from `show_effect` when a non-modulator
/// slider commits. The caller composes this with the pre-edit effects snapshot
/// to build a `Mutation::SetLayerEffects`.
///
/// The enum is unconditional (no cfg gate) so `show_effect`'s return type is
/// the same under all feature combinations. Under non-v3 builds the type is
/// dead code; the emit paths inside `show_effect` are cfg-gated so the return
/// value is always `None` without v3.
///
/// `Copy` was dropped in 003-T1.22 when `ModulatorSwitch` was added (Modulator
/// is not Copy). Existing move semantics are unaffected — push/destructure uses moves.
#[allow(dead_code)] // populated only under the v3 feature
#[derive(Debug, Clone)]
pub(super) enum EffectChange {
    /// `Effect::Transform.translate[0]` set to `new`.
    TransformTranslateX(f32),
    /// `Effect::Transform.translate[1]` set to `new`.
    TransformTranslateY(f32),
    /// 003-T1.22 — picker chose a different `Modulator` variant.
    /// 003-T1.23 — also emitted when a parameter slider (period_s, amp,
    /// phase, offset, band, …) commits a value within the current variant.
    /// In both cases, carries the complete new `Modulator` to install; the
    /// caller (`show_effects_tab`) reads `old` from the project at
    /// emit time and pushes a `Mutation::SetModulator`.
    #[cfg(feature = "v3")]
    ModulatorSwitch {
        effect_idx: usize,
        field: ModulatorField,
        new: crate::modulators::Modulator,
    },
}

pub(super) fn show_effect(
    ui: &mut Ui,
    idx: usize,
    effect: &mut Effect,
    inside_advanced: bool,
    // P0.2.5: layer index, needed to construct `LearnTarget` for the
    // MIDI-learn context menu inside `modulator_slider` (v3 only).
    // Present unconditionally so all call sites have a consistent signature;
    // the non-v3 path ignores it.
    layer_idx: usize,
) -> Option<EffectChange> {
    // `mut` is required under v3 (assignment inside cfg block); lint disagrees
    // in non-v3 builds where the write sites are compiled out.
    #[allow(unused_mut)]
    let mut change: Option<EffectChange> = None;
    // layer_idx is used only by the v3 modulator_slider context menu; suppress
    // the dead-code lint in non-v3 builds.
    #[cfg(not(feature = "v3"))]
    let _ = layer_idx;
    match effect {
        Effect::Color {
            hue,
            saturation,
            brightness,
            contrast,
        } => {
            #[cfg(feature = "v3")]
            {
                change = change.or(modulator_slider(
                    ui,
                    (idx, "hue"),
                    "hue (deg)",
                    hue,
                    -180.0..=180.0,
                    ModulatorField::ColorHue,
                    idx,
                    layer_idx,
                ));
                change = change.or(modulator_slider(
                    ui,
                    (idx, "sat"),
                    "saturation",
                    saturation,
                    0.0..=2.0,
                    ModulatorField::ColorSaturation,
                    idx,
                    layer_idx,
                ));
                change = change.or(modulator_slider(
                    ui,
                    (idx, "bri"),
                    "brightness",
                    brightness,
                    -1.0..=1.0,
                    ModulatorField::ColorBrightness,
                    idx,
                    layer_idx,
                ));
                change = change.or(modulator_slider(
                    ui,
                    (idx, "con"),
                    "contrast",
                    contrast,
                    0.0..=2.0,
                    ModulatorField::ColorContrast,
                    idx,
                    layer_idx,
                ));
            }
            #[cfg(not(feature = "v3"))]
            {
                modulator_slider(ui, (idx, "hue"), "hue (deg)", hue, -180.0..=180.0, (), idx);
                modulator_slider(
                    ui,
                    (idx, "sat"),
                    "saturation",
                    saturation,
                    0.0..=2.0,
                    (),
                    idx,
                );
                modulator_slider(
                    ui,
                    (idx, "bri"),
                    "brightness",
                    brightness,
                    -1.0..=1.0,
                    (),
                    idx,
                );
                modulator_slider(ui, (idx, "con"), "contrast", contrast, 0.0..=2.0, (), idx);
            }
        }
        Effect::Tint { .. } => {
            ui.label("(Tint not yet implemented; see Effect::Tint stub)");
        }
        Effect::Blur { radius_px } => {
            #[cfg(feature = "v3")]
            {
                change = change.or(modulator_slider(
                    ui,
                    (idx, "blur"),
                    "radius (px)",
                    radius_px,
                    0.0..=32.0,
                    ModulatorField::BlurRadius,
                    idx,
                    layer_idx,
                ));
            }
            #[cfg(not(feature = "v3"))]
            {
                modulator_slider(
                    ui,
                    (idx, "blur"),
                    "radius (px)",
                    radius_px,
                    0.0..=32.0,
                    (),
                    idx,
                );
            }
        }
        Effect::Transform {
            translate,
            rotate_deg,
            scale_x,
            scale_y,
        } => {
            #[cfg(feature = "v3")]
            {
                if let Some(new) = command_slider(
                    ui,
                    &format!("effect_{idx}_tx"),
                    "tx",
                    translate[0],
                    -1.0..=1.0,
                ) {
                    change = Some(EffectChange::TransformTranslateX(new));
                }
                if let Some(new) = command_slider(
                    ui,
                    &format!("effect_{idx}_ty"),
                    "ty",
                    translate[1],
                    -1.0..=1.0,
                ) {
                    change = change.or(Some(EffectChange::TransformTranslateY(new)));
                }
                change = change.or(modulator_slider(
                    ui,
                    (idx, "rot"),
                    "rotate (deg)",
                    rotate_deg,
                    -180.0..=180.0,
                    ModulatorField::TransformRotateDeg,
                    idx,
                    layer_idx,
                ));
                change = change.or(modulator_slider(
                    ui,
                    (idx, "scx"),
                    "scale x",
                    scale_x,
                    0.1..=3.0,
                    ModulatorField::TransformScaleX,
                    idx,
                    layer_idx,
                ));
                change = change.or(modulator_slider(
                    ui,
                    (idx, "scy"),
                    "scale y",
                    scale_y,
                    0.1..=3.0,
                    ModulatorField::TransformScaleY,
                    idx,
                    layer_idx,
                ));
            }
            #[cfg(not(feature = "v3"))]
            {
                ui.add(egui::Slider::new(&mut translate[0], -1.0..=1.0).text("tx"));
                ui.add(egui::Slider::new(&mut translate[1], -1.0..=1.0).text("ty"));
                modulator_slider(
                    ui,
                    (idx, "rot"),
                    "rotate (deg)",
                    rotate_deg,
                    -180.0..=180.0,
                    (),
                    idx,
                );
                modulator_slider(ui, (idx, "scx"), "scale x", scale_x, 0.1..=3.0, (), idx);
                modulator_slider(ui, (idx, "scy"), "scale y", scale_y, 0.1..=3.0, (), idx);
            }
        }
        Effect::External { id, params } => {
            // T3.17: raw JSON only inside Advanced. v2 callers always pass
            // `inside_advanced = true` (v2 has no Advanced concept, so the
            // Effects tab keeps showing JSON).  Under v3, `show_effect` is
            // only called from `advanced::show_effect_chain`, so `inside_advanced`
            // is always true there too.  The flag guards a future surface that
            // might call `show_effect` outside Advanced.
            ui.label(format!("id: {id}"));
            if inside_advanced {
                ui.label("params (JSON, edited via project file):");
                ui.label(
                    serde_json::to_string_pretty(params).unwrap_or_else(|_| "<unprintable>".into()),
                );
            } else {
                ui.label("This effect is configured in the project file.");
            }
        }
    }
    change
}

/// Construct a fresh `Modulator` payload for a newly-picked
/// [`BindingSource`], using `range`-aware defaults so the new
/// modulator's amplitude / offset don't snap the parameter to an
/// out-of-range value.
///
/// P0.2.3b/c migration recipe — every existing inline modulator
/// picker switches to this helper. Defaults match the v3 hand-rolled
/// values for Static / Sine; the new variants (Triangle / Noise /
/// Bpm / Audio / OscBound / MidiBound) follow the same `span * 0.5`
/// amp + midpoint-offset pattern.
#[cfg(feature = "v3")]
fn modulator_for_source(
    source: crate::windows::components::binding_picker::BindingSource,
    range: &std::ops::RangeInclusive<f32>,
) -> Modulator {
    use crate::windows::components::binding_picker::BindingSource;
    let span = range.end() - range.start();
    let mid = (range.start() + range.end()) * 0.5;
    match source {
        BindingSource::Fixed => Modulator::Static(*range.start()),
        BindingSource::Sine => Modulator::Sine {
            period_s: 1.0,
            amp: span * 0.5,
            phase: 0.0,
            offset: mid,
        },
        BindingSource::Triangle => Modulator::Triangle {
            period_s: 1.0,
            amp: span * 0.5,
            offset: mid,
        },
        BindingSource::Noise => Modulator::Noise {
            period_s: 1.0,
            amp: span * 0.5,
            offset: mid,
        },
        BindingSource::Bpm => Modulator::Bpm {
            divisor: 1.0,
            amp: span * 0.5,
            offset: mid,
        },
        BindingSource::Audio => Modulator::Audio {
            band: 0,
            smoothing: 0.0,
            amp: span,
            offset: *range.start(),
        },
        BindingSource::Osc => Modulator::OscBound {
            addr: "/rmap/param".to_string(),
            scale: span,
            offset: *range.start(),
        },
        BindingSource::Midi => Modulator::MidiBound {
            cc: 21,
            channel: 0,
            scale: span,
            offset: *range.start(),
        },
    }
}

/// Inner body of `modulator_slider` — shared between v3 and non-v3.
/// Returns `Some(EffectChange::ModulatorSwitch { .. })` in v3 mode on a
/// variant switch; in non-v3 it writes directly to `*m` and returns `None`.
#[cfg(feature = "v3")]
// P0.2.5 added `layer_idx` for the MIDI-learn context menu; clippy counts 8.
#[allow(clippy::too_many_arguments)]
fn modulator_slider(
    ui: &mut Ui,
    salt: (usize, &'static str),
    label: &str,
    m: &mut Modulator,
    range: std::ops::RangeInclusive<f32>,
    field: ModulatorField,
    effect_idx: usize,
    layer_idx: usize,
) -> Option<EffectChange> {
    use crate::windows::components::binding_picker::{BindingSource, binding_picker};

    let mut change: Option<EffectChange> = None;

    // P0.2.5: build the learn target for this row; used for the context menu
    // and the armed-state pulse.
    let learn_target = crate::controls::midi_learn::LearnTarget {
        layer_idx,
        effect_idx,
        field,
    };

    ui.horizontal(|ui| {
        // P0.2.5: label response carries the right-click context menu for
        // "Learn next MIDI CC". The menu arms the global learn state; any
        // subsequent CC in the MIDI callback fires `MidiLearnCapture`.
        let label_resp = ui.label(label);
        label_resp.context_menu(|ui| {
            if ui.button("Learn next MIDI CC").clicked() {
                // Pre-compute the range-derived scale/offset so the captured
                // `MidiBound` sweeps the full parameter range — same shape
                // `modulator_for_source(BindingSource::Midi, &range)` produces.
                let scale = range.end() - range.start();
                let offset = *range.start();
                crate::controls::midi_learn::arm(learn_target, scale, offset);
                ui.close_menu();
            }
        });

        // P0.2.5: pulsing accent dot while this row is the armed learn target.
        // We modulate alpha via a sine of the current egui time, giving a smooth
        // 2 Hz pulse with no external animation state. `request_repaint_after`
        // drives continuous redraws at ~50 ms intervals while armed.
        if crate::controls::midi_learn::is_armed_for(learn_target) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
            let t = ui.input(|i| i.time);
            let alpha = ((t * std::f64::consts::TAU).sin() * 0.5 + 0.5) as f32;
            let warm =
                egui::Color32::from_rgba_unmultiplied(0xd0, 0xa0, 0x40, (alpha * 255.0) as u8);
            ui.colored_label(warm, "●");
        }

        // P0.2.3b/c — replace the bespoke ComboBox + Static/Sine arms
        // with the shared `BindingPicker`. Operators can now switch
        // to any of the 8 sources (was: only Static / Sine were
        // selectable, even though every variant resolved at runtime).
        let current = BindingSource::from_modulator(m);
        if let Some(new_source) = binding_picker(ui, salt, current) {
            change = Some(EffectChange::ModulatorSwitch {
                effect_idx,
                field,
                new: modulator_for_source(new_source, &range),
            });
        }
    });
    // 003-T1.23: if the picker emitted a variant switch this frame, still
    // render the parameter widgets for the *current* modulator so the UI
    // doesn't go blank, but suppress any param emission (picker wins).
    // If no picker change, wire param commits to EffectChange::ModulatorSwitch.
    if change.is_none() {
        if let Some(new) = modulator_slider_params(ui, salt, m, range.clone()) {
            change = Some(EffectChange::ModulatorSwitch {
                effect_idx,
                field,
                new,
            });
        }
    } else {
        let _ = modulator_slider_params(ui, salt, m, range);
    }
    change
}

/// Inner body of `modulator_slider` — non-v3 version. Writes directly to `*m`.
#[cfg(not(feature = "v3"))]
fn modulator_slider(
    ui: &mut Ui,
    salt: (usize, &'static str),
    label: &str,
    m: &mut Modulator,
    range: std::ops::RangeInclusive<f32>,
    _field: (),
    _effect_idx: usize,
) -> Option<EffectChange> {
    ui.horizontal(|ui| {
        ui.label(label);
        let cur_label = match m {
            Modulator::Static(_) => "static",
            Modulator::Sine { .. } => "sine",
            Modulator::Triangle { .. } => "tri",
            Modulator::Noise { .. } => "noise",
            Modulator::Bpm { .. } => "bpm",
            Modulator::Audio { .. } => "audio",
            Modulator::OscBound { .. } => "osc",
            Modulator::MidiBound { .. } => "midi",
        };
        egui::ComboBox::from_id_salt(salt)
            .selected_text(cur_label)
            .show_ui(ui, |ui| {
                let is_static = matches!(m, Modulator::Static(_));
                let is_sine = matches!(m, Modulator::Sine { .. });
                if ui.selectable_label(is_static, "static").clicked() && !is_static {
                    *m = Modulator::Static(*range.start());
                }
                if ui.selectable_label(is_sine, "sine").clicked() && !is_sine {
                    let span = range.end() - range.start();
                    *m = Modulator::Sine {
                        period_s: 1.0,
                        amp: span * 0.5,
                        phase: 0.0,
                        offset: (range.start() + range.end()) * 0.5,
                    };
                }
            });
    });
    modulator_slider_params(ui, salt, m, range);
    None
}

/// Parameter sliders for the currently-active `Modulator` variant.
///
/// In v3 mode (`#[cfg(feature = "v3")]`): reads `m` read-only, uses
/// `command_slider` / `command_dragvalue_u32` helpers, and returns
/// `Some(new_modulator)` when a slider commits a value.  The caller
/// (`modulator_slider`) wraps that into `EffectChange::ModulatorSwitch`.
///
/// In non-v3 mode: binds `egui::Slider` directly to `*m`'s fields and
/// always returns `None`.
///
/// `salt` is forwarded to the widget id so that each parameter slider has
/// a globally-unique id even when the same variant appears on multiple
/// effects or layers.
#[cfg(feature = "v3")]
fn modulator_slider_params(
    ui: &mut Ui,
    salt: (usize, &'static str),
    m: &mut Modulator,
    range: std::ops::RangeInclusive<f32>,
) -> Option<Modulator> {
    let mut new_modulator: Option<Modulator> = None;
    match m {
        Modulator::Static(v) => {
            let id = format!("mod_{}_{}_static", salt.0, salt.1);
            if let Some(new) = command_slider(ui, &id, "value", *v, range.clone()) {
                new_modulator = Some(Modulator::Static(new));
            }
        }
        Modulator::Sine {
            period_s,
            amp,
            phase,
            offset,
        } => {
            let span = range.end() - range.start();
            let cur_period_s = *period_s;
            let cur_amp = *amp;
            let cur_phase = *phase;
            let cur_offset = *offset;
            let id_period = format!("mod_{}_{}_period", salt.0, salt.1);
            let id_amp = format!("mod_{}_{}_amp", salt.0, salt.1);
            let id_phase = format!("mod_{}_{}_phase", salt.0, salt.1);
            let id_offset = format!("mod_{}_{}_offset", salt.0, salt.1);
            if let Some(new) =
                command_slider(ui, &id_period, "period (s)", cur_period_s, 0.05..=10.0)
            {
                new_modulator = Some(Modulator::Sine {
                    period_s: new,
                    amp: cur_amp,
                    phase: cur_phase,
                    offset: cur_offset,
                });
            }
            if let Some(new) = command_slider(ui, &id_amp, "amp", cur_amp, 0.0..=span) {
                new_modulator = new_modulator.or(Some(Modulator::Sine {
                    period_s: cur_period_s,
                    amp: new,
                    phase: cur_phase,
                    offset: cur_offset,
                }));
            }
            if let Some(new) = command_slider(
                ui,
                &id_phase,
                "phase",
                cur_phase,
                0.0..=std::f32::consts::TAU,
            ) {
                new_modulator = new_modulator.or(Some(Modulator::Sine {
                    period_s: cur_period_s,
                    amp: cur_amp,
                    phase: new,
                    offset: cur_offset,
                }));
            }
            if let Some(new) = command_slider(ui, &id_offset, "offset", cur_offset, range.clone()) {
                new_modulator = new_modulator.or(Some(Modulator::Sine {
                    period_s: cur_period_s,
                    amp: cur_amp,
                    phase: cur_phase,
                    offset: new,
                }));
            }
        }
        Modulator::Triangle { .. } | Modulator::Noise { .. } | Modulator::Bpm { .. } => {
            ui.label("(this modulator variant has no UI in v1)");
        }
        Modulator::OscBound { addr, .. } => {
            // P0.2.1 — minimal placeholder UI. The full BindingPicker
            // surface (text edit for address, scale / offset sliders,
            // live value pill) lands in P0.2.3a. For now show the bound
            // address read-only so projects load without crashing.
            ui.label(format!("OSC: {addr}"));
        }
        Modulator::MidiBound { cc, channel, .. } => {
            // P0.2.2 — minimal placeholder UI. Full picker + learn
            // workflow lands in P0.2.3a / P0.2.5.
            ui.label(format!("MIDI CC {cc} / Ch {}", *channel + 1));
        }
        Modulator::Audio {
            band,
            smoothing,
            amp,
            offset,
        } => {
            let span = range.end() - range.start();
            let cur_band = *band;
            let cur_smoothing = *smoothing;
            let cur_amp = *amp;
            let cur_offset = *offset;
            let id_band = format!("mod_{}_{}_band", salt.0, salt.1);
            let id_amp = format!("mod_{}_{}_amp", salt.0, salt.1);
            let id_offset = format!("mod_{}_{}_offset", salt.0, salt.1);
            if let Some(new) =
                command_dragvalue_u32(ui, &id_band, cur_band as u32, 0u32..=7u32, "band ")
            {
                let band_u8 = new.min(u8::MAX as u32) as u8;
                new_modulator = Some(Modulator::Audio {
                    band: band_u8,
                    smoothing: cur_smoothing,
                    amp: cur_amp,
                    offset: cur_offset,
                });
            }
            if let Some(new) = command_slider(ui, &id_amp, "amp", cur_amp, 0.0..=span) {
                new_modulator = new_modulator.or(Some(Modulator::Audio {
                    band: cur_band,
                    smoothing: cur_smoothing,
                    amp: new,
                    offset: cur_offset,
                }));
            }
            if let Some(new) = command_slider(ui, &id_offset, "offset", cur_offset, range.clone()) {
                new_modulator = new_modulator.or(Some(Modulator::Audio {
                    band: cur_band,
                    smoothing: cur_smoothing,
                    amp: cur_amp,
                    offset: new,
                }));
            }
            ui.label("(audio: requires --features audio at build; reads live FFT bands)");
        }
    }
    ui.add_space(2.0);
    new_modulator
}

/// Parameter sliders for the currently-active `Modulator` variant — non-v3 version.
/// Binds `egui::Slider` / `egui::DragValue` directly to `*m`'s fields. Always returns `None`.
#[cfg(not(feature = "v3"))]
fn modulator_slider_params(
    ui: &mut Ui,
    _salt: (usize, &'static str),
    m: &mut Modulator,
    range: std::ops::RangeInclusive<f32>,
) -> Option<Modulator> {
    match m {
        Modulator::Static(v) => {
            ui.add(egui::Slider::new(v, range.clone()).text("value"));
        }
        Modulator::Sine {
            period_s,
            amp,
            phase,
            offset,
        } => {
            let span = range.end() - range.start();
            ui.add(egui::Slider::new(period_s, 0.05..=10.0).text("period (s)"));
            ui.add(egui::Slider::new(amp, 0.0..=span).text("amp"));
            ui.add(egui::Slider::new(phase, 0.0..=std::f32::consts::TAU).text("phase"));
            ui.add(egui::Slider::new(offset, range.clone()).text("offset"));
        }
        Modulator::Triangle { .. } | Modulator::Noise { .. } | Modulator::Bpm { .. } => {
            ui.label("(this modulator variant has no UI in v1)");
        }
        Modulator::OscBound { addr, .. } => {
            ui.label(format!("OSC: {addr}"));
        }
        Modulator::MidiBound { cc, channel, .. } => {
            ui.label(format!("MIDI CC {cc} / Ch {}", *channel + 1));
        }
        Modulator::Audio {
            band,
            smoothing: _,
            amp,
            offset,
        } => {
            let span = range.end() - range.start();
            ui.add(egui::DragValue::new(band).range(0..=7u8).prefix("band "));
            ui.add(egui::Slider::new(amp, 0.0..=span).text("amp"));
            ui.add(egui::Slider::new(offset, range.clone()).text("offset"));
            ui.label("(audio: requires --features audio at build; reads live FFT bands)");
        }
    }
    ui.add_space(2.0);
    None
}

// ---------------------------------------------------------------------------
// 003-T5.12 — in-app Glossary window
// ---------------------------------------------------------------------------

/// Render a floating egui `Window` listing every [`GlossaryTerm`] with its
/// headline and body text. Toggled by `open`; the window's own close button
/// also clears the flag so the operator has two ways to dismiss it.
///
/// This is a `v3`-only function (`advanced.rs` and `glossary.rs` are both
/// behind the `v3` feature gate).
#[cfg(feature = "v3")]
fn show_glossary_window(ctx: &egui::Context, open: &mut bool) {
    use crate::windows::glossary::{all_terms, entry};

    egui::Window::new("Glossary")
        .open(open)
        .resizable(true)
        .default_width(420.0)
        .default_height(500.0)
        .show(ctx, |ui| {
            ui.label(
                "Every term used in rmap, with a short explanation. \
                 Hover the ? next to a label in the Advanced panel for context-sensitive help.",
            );
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for &term in all_terms() {
                    let e = entry(term);
                    ui.strong(e.headline);
                    ui.label(e.body);
                    ui.add_space(8.0);
                }
            });
        });
}

// ---------------------------------------------------------------------------
// 003-T5.12 — help URL
// ---------------------------------------------------------------------------

/// The canonical URL for rmap's built-in help. Opens in the default browser
/// via `open_help_url()`. Kept as a constant so both the "?" button and any
/// future deep-link code share a single definition.
///
/// No repository URL is set in `Cargo.toml`; this placeholder points to the
/// docs.rs page. Update when the canonical public URL is established.
///
/// TODO: replace with the GitHub README URL when the repository is public.
#[cfg(feature = "v3")]
pub const HELP_URL: &str = "https://docs.rs/rmap";

/// Open `HELP_URL` in the user's default browser (macOS `open` command).
/// Returns `Ok(())` immediately — the browser opens asynchronously. Logs a
/// warning (does not panic) if the `open` subprocess cannot be spawned.
#[cfg(feature = "v3")]
pub fn open_help_url() {
    match std::process::Command::new("open").arg(HELP_URL).spawn() {
        Ok(_) => {
            tracing::info!(url = HELP_URL, "opened help URL in browser");
        }
        Err(e) => {
            tracing::warn!(?e, url = HELP_URL, "failed to spawn 'open' for help URL");
        }
    }
}

// ---------------------------------------------------------------------------
// 003-T5.12 unit tests
// ---------------------------------------------------------------------------
#[cfg(all(test, feature = "v3"))]
mod help_tests {
    use super::HELP_URL;
    use crate::windows::glossary::{all_terms, entry};

    /// T5.12 — the help URL is non-empty and starts with https://.
    #[test]
    fn help_url_is_non_empty_https() {
        assert!(!HELP_URL.is_empty(), "HELP_URL must not be empty");
        assert!(
            HELP_URL.starts_with("https://"),
            "HELP_URL should be an HTTPS URL, got: {HELP_URL:?}"
        );
    }

    /// T5.12 — the Glossary window can iterate every GlossaryTerm and produce
    /// a non-empty headline + body (same check as T3.22 but verifies the
    /// `all_terms()` path that the window iterates, not just the entry()
    /// exhaustive match).
    #[test]
    fn glossary_window_assembles_every_term() {
        for &term in all_terms() {
            let e = entry(term);
            assert!(
                !e.headline.is_empty(),
                "glossary window: headline empty for {term:?}"
            );
            assert!(
                !e.body.is_empty(),
                "glossary window: body empty for {term:?}"
            );
        }
    }
}
