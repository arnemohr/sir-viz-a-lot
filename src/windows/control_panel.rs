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
use crate::project::schema::{self, BlendMode, Project, Scene};
use crate::project::snapshot;
use crate::windows::scene_editor::{self, SceneEditorState};

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

// 003-T3.1: under `--features v3` the tab strip is hidden; T3.27
// deletes this enum once the canvas-merge transition completes.
#[cfg_attr(feature = "v3", allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlTab {
    Scene,
    Effects,
    Layers,
    Mapping,
    Scenes,
}

impl Default for ControlTab {
    fn default() -> Self {
        // T-M9-02: Scene is the v2 default — operators see the live preview
        // first, the slider tabs are secondary.
        Self::Scene
    }
}

#[derive(Default)]
pub struct ControlPanelState {
    #[cfg_attr(feature = "v3", allow(dead_code))]
    pub tab: ControlTab,
    pub selected_layer: usize,
    /// 003-T1.18 — `Mutation`s emitted by `command_*` helpers during
    /// the current `show()` call. The app drains this after the
    /// frame and routes each entry through `EditingState.undo_stack`
    /// so every always-visible binding becomes Cmd-Z reversible.
    /// v2 builds carry no undo machinery; the field is gated.
    #[cfg(feature = "v3")]
    pub pending_mutations: Vec<Mutation>,
    /// Buffer for the Layers tab "add layer" path field.
    pub new_layer_path_input: String,
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
    /// 003-T3.1: when true under `--features v3`, the Advanced disclosure
    /// panel renders alongside the canvas. T3.4 wires the toolbar
    /// button that toggles this; T3.11+ refines the panel's content
    /// layout. v2 builds ignore this flag (the tabbed UI stays).
    pub advanced_open: bool,
}

pub enum ControlPanelAction {
    None,
    /// Reload GPU layer runtime from `project.layers` paths.
    RebuildLayers,
    /// Operator clicked "recall" on a scene slot. App routes through the same
    /// scheduling logic as the keyboard hotkey so crossfade
    /// (`Project::crossfade_duration_s`) is honored from the UI too.
    SceneRecall(usize),
    /// 003-T3.4: toolbar Undo button clicked. App drains through `undo_stack.undo`.
    #[cfg(feature = "v3")]
    RequestUndo,
    /// 003-T3.4: toolbar Redo button clicked. App drains through `undo_stack.redo`.
    #[cfg(feature = "v3")]
    RequestRedo,
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
    #[cfg(not(feature = "v3"))]
    egui::Panel::top("rmap_tabs")
        .resizable(false)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut st.tab, ControlTab::Scene, "Scene");
                ui.selectable_value(&mut st.tab, ControlTab::Effects, "Effects");
                ui.selectable_value(&mut st.tab, ControlTab::Layers, "Layers");
                ui.selectable_value(&mut st.tab, ControlTab::Mapping, "Mapping");
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

    // 003-T3.3: selection-driven right-edge inspector. Appears when
    // `scene.selected.is_some()`; rendered before the Advanced panel
    // so it sits closer to the canvas. The Advanced panel claims the
    // outermost right column when both are visible.
    #[cfg(feature = "v3")]
    if scene.selected.is_some() {
        egui::SidePanel::right("rmap_inspector")
            .resizable(false)
            .exact_width(280.0)
            .show_inside(ui, |ui| {
                crate::windows::inspector::show(ui, project, st, scene);
            });
    }

    #[cfg(feature = "v3")]
    if st.advanced_open {
        egui::SidePanel::right("rmap_advanced")
            .resizable(true)
            .default_width(420.0)
            .show_inside(ui, |ui| {
                ui.heading("Advanced");
                ui.label("Effects / Layers / Mapping / Scenes — T3.11+ rearranges this.");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::CollapsingHeader::new("Layers")
                        .default_open(true)
                        .show(ui, |ui| {
                            if matches!(
                                show_layers_tab(ui, project, st),
                                ControlPanelAction::RebuildLayers
                            ) {
                                action = ControlPanelAction::RebuildLayers;
                            }
                        });
                    egui::CollapsingHeader::new("Effects")
                        .default_open(false)
                        .show(ui, |ui| show_effects_tab(ui, project, st));
                    egui::CollapsingHeader::new("Mapping")
                        .default_open(false)
                        .show(ui, |ui| show_mapping_tab(ui, project, st));
                    egui::CollapsingHeader::new("Scenes")
                        .default_open(false)
                        .show(ui, |ui| {
                            if let ControlPanelAction::SceneRecall(idx) =
                                show_scenes_tab(ui, project, st)
                            {
                                action = ControlPanelAction::SceneRecall(idx);
                            }
                        });
                });
            });
    }

    // 003-T3.2: layer thumbnail strip on the left edge. Only under v3;
    // v2 builds keep the tabbed UI unchanged. Rendered BEFORE the
    // CentralPanel so egui's panel layout claims the left edge first.
    #[cfg(feature = "v3")]
    egui::SidePanel::left("rmap_layer_strip")
        .resizable(false)
        .exact_width(88.0)
        .show_inside(ui, |ui| {
            crate::windows::layer_strip::show(ui, project, st, scene);
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
        #[cfg(not(feature = "v3"))]
        match st.tab {
            ControlTab::Scene => show_scene_tab(ui, project, st, scene, inputs),
            ControlTab::Effects => show_effects_tab(ui, project, st),
            ControlTab::Layers => {
                if matches!(show_layers_tab(ui, project, st), ControlPanelAction::RebuildLayers) {
                    action = ControlPanelAction::RebuildLayers;
                }
            }
            ControlTab::Mapping => show_mapping_tab(ui, project, st),
            ControlTab::Scenes => action = show_scenes_tab(ui, project, st),
        }

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
                #[cfg(feature = "v3")]
                {
                    if let Some(new) =
                        command_slider(ui, "gamma", "gamma", project.gamma, 0.2..=4.0)
                    {
                        st.pending_mutations
                            .push(project.set_gamma_mutation(new));
                    }
                    if let Some(new) = command_slider(
                        ui,
                        "brightness",
                        "brightness",
                        project.brightness,
                        -1.0..=1.0,
                    ) {
                        st.pending_mutations
                            .push(project.set_brightness_mutation(new));
                    }
                    if let Some(new) = command_slider(
                        ui,
                        "contrast",
                        "contrast",
                        project.contrast,
                        0.0..=4.0,
                    ) {
                        st.pending_mutations
                            .push(project.set_contrast_mutation(new));
                    }
                }
                #[cfg(not(feature = "v3"))]
                {
                    ui.add(egui::Slider::new(&mut project.gamma, 0.2..=4.0).text("gamma"));
                    ui.add(egui::Slider::new(&mut project.brightness, -1.0..=1.0).text("brightness"));
                    ui.add(egui::Slider::new(&mut project.contrast, 0.0..=4.0).text("contrast"));
                }
            });
    });

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
    if let Some(scene_editor::Selection::Layer(idx)) = scene.selected {
        if let Some(layer) = project.layers.get(idx) {
            ui.label(format!("selected: layer {} ({})", idx, layer.id));
        }
    }
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
    painter.rect_filled(
        outer,
        egui::CornerRadius::ZERO,
        egui::Color32::from_rgb(8, 9, 12),
    );
    painter.image(
        tex_id,
        inner,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
    painter.rect_stroke(
        inner,
        egui::CornerRadius::ZERO,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 75, 85)),
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
                    if let Some(change) = show_effect(ui, idx, effect) {
                        staged_changes.push((idx, change));
                    }
                }
                #[cfg(not(feature = "v3"))]
                {
                    let _ = show_effect(ui, idx, effect);
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
            st.pending_mutations.push(Mutation::SetLayerEffects {
                layer_idx,
                new,
                old,
            });
        }
    }
}

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
        ui.colored_label(egui::Color32::from_rgb(220, 120, 100), &st.add_layer_error);
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
                        st.pending_mutations.push(Mutation::SetLayerEnabled {
                            layer_idx: i,
                            new,
                            old: layer.enabled,
                        });
                    }
                }
                #[cfg(not(feature = "v3"))]
                {
                    ui.checkbox(&mut layer.enabled, &format!("{}", layer.id));
                }
                ui.label(layer.kind.asset_path().display().to_string());
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
                            st.pending_mutations.push(Mutation::SetLayerBlendMode {
                                layer_idx: i,
                                new,
                                old: current_mode,
                            });
                        }
                    }
                    if let Some(new) = command_slider(
                        ui,
                        &format!("opacity_{i}"),
                        "opacity",
                        layer.opacity,
                        0.0..=1.0,
                    ) {
                        st.pending_mutations.push(Mutation::SetLayerOpacity {
                            layer_idx: i,
                            new,
                            old: layer.opacity,
                        });
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
                .push(Mutation::SwapLayers { i, j: i - 1 });
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
                .push(Mutation::SwapLayers { i, j: i + 1 });
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

fn blend_label(m: BlendMode) -> &'static str {
    match m {
        BlendMode::Normal => "Normal",
        BlendMode::Add => "Add",
        BlendMode::Multiply => "Multiply",
        BlendMode::Screen => "Screen",
    }
}

fn show_mapping_tab(
    ui: &mut Ui,
    project: &mut Project,
    #[cfg_attr(not(feature = "v3"), allow(unused_variables))] st: &mut ControlPanelState,
) {
    // v4: warp lives on the first layer. The legacy v2 Mapping tab is
    // pre-T3.6 deletion; until T3.6 ships the inspector replacement,
    // this path keeps editing layer 0's warp.
    let Some(layer) = project.layers.get_mut(0) else {
        ui.label("No layers — add a layer first.");
        return;
    };
    let w = &mut layer.warp;
    let rows = w.grid.len();
    let cols = if rows > 0 { w.grid[0].len() } else { 0 };
    if rows < 2 || cols < 2 || w.grid.iter().any(|row| row.len() != cols) {
        ui.label("Mapping UI: warp grid must be at least 2×2 (corner pin).");
        return;
    }

    ui.label(
        "Drag the corners to map output to projector space. Coordinates are normalized [0,1].",
    );

    // Mesh-resolution controls. Editing rows/cols bilinear-resamples the grid so
    // the operator's existing customisation survives a resize (T-M7-01).
    ui.horizontal(|ui| {
        ui.label("mesh");
        #[cfg(feature = "v3")]
        {
            let new_rows_opt =
                command_dragvalue_u32(ui, "warp_rows", w.rows.max(1), 1..=8u32, "rows ");
            let new_cols_opt =
                command_dragvalue_u32(ui, "warp_cols", w.cols.max(1), 1..=8u32, "cols ");
            if new_rows_opt.is_some() || new_cols_opt.is_some() {
                let new_rows = new_rows_opt.unwrap_or(w.rows).max(1);
                let new_cols = new_cols_opt.unwrap_or(w.cols).max(1);
                if new_rows != w.rows || new_cols != w.cols {
                    let new_grid = schema::resample_grid(&w.grid, new_rows, new_cols);
                    st.pending_mutations.push(Mutation::SetLayerWarpDimensions {
                        layer_idx: 0,
                        new_rows,
                        new_cols,
                        new_grid,
                        old_rows: w.rows,
                        old_cols: w.cols,
                        old_grid: w.grid.clone(),
                    });
                }
            }
        }
        #[cfg(not(feature = "v3"))]
        {
            let mut new_rows = w.rows.max(1);
            let mut new_cols = w.cols.max(1);
            let r_resp = ui.add(
                egui::DragValue::new(&mut new_rows)
                    .range(1..=8u32)
                    .prefix("rows "),
            );
            let c_resp = ui.add(
                egui::DragValue::new(&mut new_cols)
                    .range(1..=8u32)
                    .prefix("cols "),
            );
            let changed = (r_resp.changed() || c_resp.changed())
                && (new_rows != w.rows || new_cols != w.cols);
            if changed {
                w.grid = schema::resample_grid(&w.grid, new_rows, new_cols);
                w.rows = new_rows;
                w.cols = new_cols;
            }
        }
        ui.label(format!("({} × {} cells)", w.rows, w.cols));
    });

    // 16:9 thumbnail of the output framebuffer area. The canvas itself stands in for
    // the framebuffer (we don't have a cross-window snapshot in v1 — see T-M5-08 notes).
    let canvas_size = egui::vec2(480.0, 270.0);
    let (canvas_resp, painter) = ui.allocate_painter(canvas_size, egui::Sense::hover());
    let canvas_rect = canvas_resp.rect;

    // Background placeholder: dark fill + checker pattern + border + axis labels.
    painter.rect_filled(
        canvas_rect,
        egui::CornerRadius::ZERO,
        egui::Color32::from_rgb(20, 22, 26),
    );
    let checker_n = 16usize;
    let cw = canvas_rect.width() / checker_n as f32;
    let ch = canvas_rect.height() / (checker_n as f32 * 9.0 / 16.0);
    let rows_n = ((canvas_rect.height() / ch).ceil() as usize).max(1);
    for cy in 0..rows_n {
        for cx in 0..checker_n {
            if (cx + cy) % 2 == 0 {
                continue;
            }
            let p0 = canvas_rect.left_top() + egui::vec2(cx as f32 * cw, cy as f32 * ch);
            let r = egui::Rect::from_min_size(p0, egui::vec2(cw, ch)).intersect(canvas_rect);
            painter.rect_filled(
                r,
                egui::CornerRadius::ZERO,
                egui::Color32::from_rgb(28, 30, 34),
            );
        }
    }
    painter.rect_stroke(
        canvas_rect,
        egui::CornerRadius::ZERO,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 75, 85)),
        egui::StrokeKind::Outside,
    );
    let label_color = egui::Color32::from_rgb(140, 145, 155);
    painter.text(
        canvas_rect.left_top() + egui::vec2(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        "0,0",
        egui::FontId::proportional(11.0),
        label_color,
    );
    painter.text(
        canvas_rect.right_bottom() + egui::vec2(-4.0, -2.0),
        egui::Align2::RIGHT_BOTTOM,
        "1,1",
        egui::FontId::proportional(11.0),
        label_color,
    );
    painter.text(
        canvas_rect.center() + egui::vec2(0.0, -2.0),
        egui::Align2::CENTER_BOTTOM,
        "output area (placeholder thumbnail)",
        egui::FontId::proportional(11.0),
        label_color,
    );

    // Helper: normalized [0,1]^2 -> screen position inside canvas_rect.
    let to_screen = |g: [f32; 2]| -> egui::Pos2 {
        canvas_rect.left_top() + egui::vec2(g[0] * canvas_rect.width(), g[1] * canvas_rect.height())
    };

    // Mesh edges (low-contrast).
    let edge_color = egui::Color32::from_rgb(120, 165, 220);
    let edge_stroke = egui::Stroke::new(1.5, edge_color);
    for r in 0..rows {
        for c in 0..cols {
            let here = to_screen(w.grid[r][c]);
            if c + 1 < cols {
                let right = to_screen(w.grid[r][c + 1]);
                painter.line_segment([here, right], edge_stroke);
            }
            if r + 1 < rows {
                let down = to_screen(w.grid[r + 1][c]);
                painter.line_segment([here, down], edge_stroke);
            }
        }
    }

    // Handles: filled circles, hover/drag-aware. We allocate per-handle responses
    // via `ui.interact(...)` so each handle gets its own hit-test rect.
    let handle_radius = 7.0_f32;
    let canvas_w = canvas_rect.width();
    let canvas_h = canvas_rect.height();
    for r in 0..rows {
        for c in 0..cols {
            let center = to_screen(w.grid[r][c]);
            let rect = egui::Rect::from_center_size(
                center,
                egui::vec2(handle_radius * 2.5, handle_radius * 2.5),
            );
            let id = canvas_resp.id.with(("corner_handle", r, c));
            let resp = ui.interact(rect, id, egui::Sense::drag());

            if resp.dragged() {
                let delta = resp.drag_delta();
                if canvas_w > 0.0 && canvas_h > 0.0 {
                    w.grid[r][c][0] = (w.grid[r][c][0] + delta.x / canvas_w).clamp(0.0, 1.0);
                    w.grid[r][c][1] = (w.grid[r][c][1] + delta.y / canvas_h).clamp(0.0, 1.0);
                }
            }

            // Re-evaluate center in case of drag this frame.
            let center = to_screen(w.grid[r][c]);
            let (fill, stroke) = if resp.dragged() {
                (
                    egui::Color32::from_rgb(255, 220, 90),
                    egui::Stroke::new(2.0, egui::Color32::WHITE),
                )
            } else if resp.hovered() {
                (
                    egui::Color32::from_rgb(220, 200, 90),
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(240, 240, 240)),
                )
            } else {
                (
                    egui::Color32::from_rgb(180, 160, 70),
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 40)),
                )
            };
            painter.circle(center, handle_radius, fill, stroke);
        }
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Reset to identity").clicked() {
            #[cfg(feature = "v3")]
            {
                // Full-snapshot Reverse (ResetWarpMesh rule 3): capture the
                // entire pre-reset WarpMesh so undo restores mask_polygon /
                // mask_feather / source_rect unchanged even though Reset only
                // writes rows / cols / grid.
                let old = w.clone();
                let mut new_mesh = old.clone();
                new_mesh.rows = 1;
                new_mesh.cols = 1;
                new_mesh.grid = vec![vec![[0.0, 0.0], [1.0, 0.0]], vec![[0.0, 1.0], [1.0, 1.0]]];
                st.pending_mutations.push(Mutation::ResetLayerWarpMesh {
                    layer_idx: 0,
                    new: new_mesh,
                    old,
                });
            }
            #[cfg(not(feature = "v3"))]
            {
                // Identity 1×1 corner pin: full output rect [0,0]..[1,1].
                w.rows = 1;
                w.cols = 1;
                w.grid = vec![vec![[0.0, 0.0], [1.0, 0.0]], vec![[0.0, 1.0], [1.0, 1.0]]];
            }
        }
        ui.label(format!("grid: {}×{}", rows, cols));
    });

    #[cfg(feature = "v3")]
    {
        if let Some(new) = command_slider(
            ui,
            "mask_feather",
            "mask feather",
            w.mask_feather,
            0.0..=0.25,
        ) {
            st.pending_mutations.push(Mutation::SetLayerMaskFeather {
                layer_idx: 0,
                new,
                old: w.mask_feather,
            });
        }
    }
    #[cfg(not(feature = "v3"))]
    {
        ui.add(egui::Slider::new(&mut w.mask_feather, 0.0..=0.25).text("mask feather"));
    }

    // T-M12-02 + T-M12-03: zone-template dropdown + clear button.
    // The Scene-tab preview lets the operator drag the resulting
    // vertices into place (T-M11-02).
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Zone:");
        for (name, build) in crate::project::zone_templates::all_templates() {
            if ui.button(name).clicked() {
                #[cfg(feature = "v3")]
                {
                    st.pending_mutations.push(Mutation::SetLayerMaskPolygon {
                        layer_idx: 0,
                        new: build(),
                        old: w.mask_polygon.clone(),
                    });
                }
                #[cfg(not(feature = "v3"))]
                {
                    w.mask_polygon = build();
                }
            }
        }
        if ui.button("clear mask").clicked() {
            #[cfg(feature = "v3")]
            {
                st.pending_mutations.push(Mutation::SetLayerMaskPolygon {
                    layer_idx: 0,
                    new: Vec::new(),
                    old: w.mask_polygon.clone(),
                });
            }
            #[cfg(not(feature = "v3"))]
            {
                w.mask_polygon.clear();
            }
        }
    });
    ui.label(format!(
        "mask: {} vertices ({})",
        w.mask_polygon.len(),
        if w.mask_polygon.len() >= 3 {
            "active"
        } else {
            "none — needs ≥ 3 vertices"
        }
    ));
}

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

fn effect_label(e: &Effect) -> &'static str {
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
enum EffectChange {
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

fn show_effect(ui: &mut Ui, idx: usize, effect: &mut Effect) -> Option<EffectChange> {
    // `mut` is required under v3 (assignment inside cfg block); lint disagrees
    // in non-v3 builds where the write sites are compiled out.
    #[allow(unused_mut)]
    let mut change: Option<EffectChange> = None;
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
                ));
                change = change.or(modulator_slider(
                    ui,
                    (idx, "sat"),
                    "saturation",
                    saturation,
                    0.0..=2.0,
                    ModulatorField::ColorSaturation,
                    idx,
                ));
                change = change.or(modulator_slider(
                    ui,
                    (idx, "bri"),
                    "brightness",
                    brightness,
                    -1.0..=1.0,
                    ModulatorField::ColorBrightness,
                    idx,
                ));
                change = change.or(modulator_slider(
                    ui,
                    (idx, "con"),
                    "contrast",
                    contrast,
                    0.0..=2.0,
                    ModulatorField::ColorContrast,
                    idx,
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
                ));
                change = change.or(modulator_slider(
                    ui,
                    (idx, "scx"),
                    "scale x",
                    scale_x,
                    0.1..=3.0,
                    ModulatorField::TransformScaleX,
                    idx,
                ));
                change = change.or(modulator_slider(
                    ui,
                    (idx, "scy"),
                    "scale y",
                    scale_y,
                    0.1..=3.0,
                    ModulatorField::TransformScaleY,
                    idx,
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
            // Extension hook: no rich UI in v1. Display the registered id and
            // let advanced users edit `params` as raw JSON. Skipped at render
            // time when no ExternalPass is registered under `id`.
            ui.label(format!("id: {id}"));
            ui.label("params (JSON, edited via project file):");
            ui.label(
                serde_json::to_string_pretty(params).unwrap_or_else(|_| "<unprintable>".into()),
            );
        }
    }
    change
}

/// Inner body of `modulator_slider` — shared between v3 and non-v3.
/// Returns `Some(EffectChange::ModulatorSwitch { .. })` in v3 mode on a
/// variant switch; in non-v3 it writes directly to `*m` and returns `None`.
#[cfg(feature = "v3")]
fn modulator_slider(
    ui: &mut Ui,
    salt: (usize, &'static str),
    label: &str,
    m: &mut Modulator,
    range: std::ops::RangeInclusive<f32>,
    field: ModulatorField,
    effect_idx: usize,
) -> Option<EffectChange> {
    let mut change: Option<EffectChange> = None;

    ui.horizontal(|ui| {
        ui.label(label);
        let cur_label = match m {
            Modulator::Static(_) => "static",
            Modulator::Sine { .. } => "sine",
            Modulator::Triangle { .. } => "tri",
            Modulator::Noise { .. } => "noise",
            Modulator::Bpm { .. } => "bpm",
            Modulator::Audio { .. } => "audio",
        };
        egui::ComboBox::from_id_salt(salt)
            .selected_text(cur_label)
            .show_ui(ui, |ui| {
                let is_static = matches!(m, Modulator::Static(_));
                let is_sine = matches!(m, Modulator::Sine { .. });
                if ui.selectable_label(is_static, "static").clicked() && !is_static {
                    change = Some(EffectChange::ModulatorSwitch {
                        effect_idx,
                        field,
                        new: Modulator::Static(*range.start()),
                    });
                }
                if ui.selectable_label(is_sine, "sine").clicked() && !is_sine {
                    let span = range.end() - range.start();
                    change = Some(EffectChange::ModulatorSwitch {
                        effect_idx,
                        field,
                        new: Modulator::Sine {
                            period_s: 1.0,
                            amp: span * 0.5,
                            phase: 0.0,
                            offset: (range.start() + range.end()) * 0.5,
                        },
                    });
                }
            });
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
