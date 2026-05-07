//! egui control panel: effects per layer, layer order, warp corners, scenes, gamma.

use std::path::{Path, PathBuf};

use egui::Ui;
use serde::Deserialize;

use crate::effects::Effect;
use crate::modulators::Modulator;
use crate::project::schema::{self, BlendMode, Project, Scene};
use crate::project::snapshot;
use crate::windows::scene_editor::{self, SceneEditorState};

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
    pub tab: ControlTab,
    pub selected_layer: usize,
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
}

pub enum ControlPanelAction {
    None,
    /// Reload GPU layer runtime from `project.layers` paths.
    RebuildLayers,
    /// Operator clicked "recall" on a scene slot. App routes through the same
    /// scheduling logic as the keyboard hotkey so crossfade
    /// (`Project::crossfade_duration_s`) is honored from the UI too.
    SceneRecall(usize),
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

    egui::CentralPanel::default().show_inside(ui, |ui| {
        match st.tab {
            ControlTab::Scene => show_scene_tab(ui, project, scene, inputs),
            ControlTab::Effects => show_effects_tab(ui, project, st),
            ControlTab::Layers => {
                if matches!(show_layers_tab(ui, project, st), ControlPanelAction::RebuildLayers) {
                    action = ControlPanelAction::RebuildLayers;
                }
            }
            ControlTab::Mapping => show_mapping_tab(ui, project),
            ControlTab::Scenes => action = show_scenes_tab(ui, project),
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
                ui.checkbox(&mut project.output_windowed, "Windowed output");
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
    });

    action
}

/// Show the live scene preview + handle direct-manipulation input
/// (T-M9-02 + T-M10-03). The preview is `warp_rt` registered as an egui
/// native texture; click-and-drag inside it selects + moves layers.
fn show_scene_tab(
    ui: &mut Ui,
    project: &mut Project,
    scene: &mut SceneEditorState,
    inputs: &ControlPanelInputs,
) {
    ui.label(
        "Live preview. Click a layer to select; drag to move; Shift-drag to scale; Alt-drag to rotate. Drag a mask vertex to move; double-click an edge to insert; Shift-click a vertex to delete. Drop SVG / PNG / JPG to add a layer.",
    );
    if let Some(scene_editor::Selection::Layer(idx)) = scene.selected {
        if let Some(layer) = project.layers.get(idx) {
            ui.label(format!(
                "selected: layer {} ({})",
                idx,
                layer.id
            ));
        }
    }
    ui.add_space(4.0);
    let Some(tex_id) = inputs.scene_texture else {
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

    // T-M11-03: double-click on a mask edge inserts a new vertex at the
    // click point, between the two endpoints. T-M11-04: shift-click on a
    // mask vertex deletes it (refused below 4 vertices to keep the SDF
    // baker happy — `<3` collapses the mask to "no mask").
    let pointer_now = ui.input(|i| i.pointer.hover_pos());
    if let Some(pos) = pointer_now {
        if resp.double_clicked() {
            if let Some((w_idx, after, point)) =
                scene_editor::hit_mask_edge(project, pos, inner)
            {
                if let Some(w) = project.warps.get_mut(w_idx) {
                    let insert_at = (after + 1).min(w.mask_polygon.len());
                    w.mask_polygon.insert(insert_at, point);
                    scene.selected = Some(scene_editor::Selection::MaskVertex {
                        warp: w_idx,
                        idx: insert_at,
                    });
                }
            }
        }
        if resp.clicked() && ui.input(|i| i.modifiers.shift) {
            if let Some((w_idx, v_idx)) = scene_editor::hit_mask_vertex(project, pos, inner) {
                if let Some(w) = project.warps.get_mut(w_idx) {
                    if w.mask_polygon.len() > 3 {
                        w.mask_polygon.remove(v_idx);
                        scene.selected = None;
                        scene.drag = None;
                    }
                }
            }
        }
    }
    painter.rect_filled(outer, egui::CornerRadius::ZERO, egui::Color32::from_rgb(8, 9, 12));
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

    // M11: paint mask polygon overlays + draggable vertex handles before
    // the layer outline so layer drag still wins on the body of the layer.
    scene_editor::paint_mask_overlays(project, scene, &painter, inner);

    // Highlight the selected layer's static post-Transform rect. Drawn as
    // a thin amber outline so the operator can see what's selected even
    // after they release the drag.
    if let Some(scene_editor::Selection::Layer(idx)) = scene.selected {
        if let Some(layer) = project.layers.get(idx) {
            let t = &layer.transform;
            let half = [t.scale[0].abs() * 0.5, t.scale[1].abs() * 0.5];
            let center = [0.5 + t.translate[0], 0.5 + t.translate[1]];
            let to_screen = |n: [f32; 2]| {
                egui::pos2(
                    inner.left() + n[0] * inner.width(),
                    inner.top() + n[1] * inner.height(),
                )
            };
            let r = egui::Rect::from_min_max(
                to_screen([center[0] - half[0], center[1] - half[1]]),
                to_screen([center[0] + half[0], center[1] + half[1]]),
            );
            painter.rect_stroke(
                r,
                egui::CornerRadius::ZERO,
                egui::Stroke::new(1.5, egui::Color32::from_rgb(220, 200, 90)),
                egui::StrokeKind::Outside,
            );
        }
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
            egui::CollapsingHeader::new(format!("Selected: {}", layer.id))
                .default_open(true)
                .show(ui, |ui| {
                    ui.label(
                        "Drag in the preview to move; Shift-drag to scale; Alt-drag to rotate; Esc to deselect.",
                    );
                    ui.add(
                        egui::Slider::new(&mut layer.transform.translate[0], -1.0..=1.0)
                            .text("translate x"),
                    );
                    ui.add(
                        egui::Slider::new(&mut layer.transform.translate[1], -1.0..=1.0)
                            .text("translate y"),
                    );
                    ui.add(
                        egui::Slider::new(&mut layer.transform.scale[0], 0.05..=4.0)
                            .text("scale x"),
                    );
                    ui.add(
                        egui::Slider::new(&mut layer.transform.scale[1], 0.05..=4.0)
                            .text("scale y"),
                    );
                    ui.add(
                        egui::Slider::new(&mut layer.transform.rotate_deg, -180.0..=180.0)
                            .text("rotate (deg)"),
                    );
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
    st.selected_layer = st.selected_layer.min(project.layers.len().saturating_sub(1));
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
                project.layers[st.selected_layer].effects =
                    st.presets[st.preset_picker_index].effects.clone();
            }
        }
        if ui.button("Reload").clicked() {
            st.presets = load_presets_from_disk();
            st.preset_picker_index = 0;
        }
    });

    let effects = &mut project.layers[st.selected_layer].effects;
    ui.heading("Effect chain");
    ui.add_space(4.0);
    for (idx, effect) in effects.iter_mut().enumerate() {
        egui::CollapsingHeader::new(effect_label(effect))
            .id_salt(idx)
            .default_open(true)
            .show(ui, |ui| {
                show_effect(ui, idx, effect);
            });
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

fn show_layers_tab(ui: &mut Ui, project: &mut Project, st: &mut ControlPanelState) -> ControlPanelAction {
    let mut action = ControlPanelAction::None;

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
                let ext_ok = p
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
                if !p.exists() {
                    st.add_layer_error = "Path does not exist.".into();
                } else if !p.is_file() {
                    st.add_layer_error = "Path is not a file.".into();
                } else if !ext_ok {
                    st.add_layer_error = "File must have extension .svg.".into();
                } else if let Ok(canonical) = p.canonicalize() {
                    let id = unique_layer_id(project);
                    project
                        .layers
                        .push(schema::layer_from_svg_path(id, canonical));
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
                ui.checkbox(&mut layer.enabled, &format!("{}", layer.id));
                ui.label(layer.kind.asset_path().display().to_string());
            });
            ui.horizontal(|ui| {
                ui.label("blend");
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
        project.layers.swap(i, i - 1);
        if st.selected_layer == i {
            st.selected_layer = i - 1;
        } else if st.selected_layer == i - 1 {
            st.selected_layer = i;
        }
        action = ControlPanelAction::RebuildLayers;
    }
    if let Some(i) = swap_down {
        project.layers.swap(i, i + 1);
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

fn show_mapping_tab(ui: &mut Ui, project: &mut Project) {
    let Some(w) = project.warps.get_mut(0) else {
        ui.label("No warp mesh — add `warps` in project.");
        return;
    };
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
        ui.label(format!("({} × {} cells)", w.rows, w.cols));
    });

    // 16:9 thumbnail of the output framebuffer area. The canvas itself stands in for
    // the framebuffer (we don't have a cross-window snapshot in v1 — see T-M5-08 notes).
    let canvas_size = egui::vec2(480.0, 270.0);
    let (canvas_resp, painter) =
        ui.allocate_painter(canvas_size, egui::Sense::hover());
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
            let p0 = canvas_rect.left_top()
                + egui::vec2(cx as f32 * cw, cy as f32 * ch);
            let r = egui::Rect::from_min_size(p0, egui::vec2(cw, ch))
                .intersect(canvas_rect);
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
        canvas_rect.left_top()
            + egui::vec2(g[0] * canvas_rect.width(), g[1] * canvas_rect.height())
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
                    w.grid[r][c][0] =
                        (w.grid[r][c][0] + delta.x / canvas_w).clamp(0.0, 1.0);
                    w.grid[r][c][1] =
                        (w.grid[r][c][1] + delta.y / canvas_h).clamp(0.0, 1.0);
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
            // Identity 1×1 corner pin: full output rect [0,0]..[1,1].
            w.rows = 1;
            w.cols = 1;
            w.grid = vec![vec![[0.0, 0.0], [1.0, 0.0]], vec![[0.0, 1.0], [1.0, 1.0]]];
        }
        ui.label(format!("grid: {}×{}", rows, cols));
    });

    ui.add(egui::Slider::new(&mut w.mask_feather, 0.0..=0.25).text("mask feather"));

    // T-M12-02 + T-M12-03: zone-template dropdown + clear button.
    // The Scene-tab preview lets the operator drag the resulting
    // vertices into place (T-M11-02).
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Zone:");
        for (name, build) in crate::project::zone_templates::all_templates() {
            if ui.button(name).clicked() {
                w.mask_polygon = build();
            }
        }
        if ui.button("clear mask").clicked() {
            w.mask_polygon.clear();
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

fn show_scenes_tab(ui: &mut Ui, project: &mut Project) -> ControlPanelAction {
    let mut action = ControlPanelAction::None;
    ui.label("Slots 1–9 (keyboard recall). Save captures full project JSON.");
    ui.add(
        egui::Slider::new(&mut project.crossfade_duration_s, 0.0..=5.0)
            .text("crossfade duration (s)"),
    );
    ui.label(
        "Crossfade only fires when both scenes share the same layer paths in the same order; structural changes snap instantly.",
    );
    ui.add_space(4.0);
    for slot in 0..9 {
        ui.horizontal(|ui| {
            ui.label(format!("{}", slot + 1));
            if ui.button("save").clicked() {
                while project.scenes.len() <= slot {
                    project.scenes.push(Scene {
                        name: format!("scene{}", project.scenes.len() + 1),
                        snapshot: serde_json::json!({}),
                    });
                }
                project.scenes[slot].snapshot = snapshot(project);
            }
            // App routes recall through the same scheduling logic as the
            // keyboard hotkey (T-M7-04). Don't `restore` here directly —
            // that would bypass crossfade scheduling.
            if ui.button("recall").clicked() && project.scenes.get(slot).is_some() {
                action = ControlPanelAction::SceneRecall(slot);
            }
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

fn show_effect(ui: &mut Ui, idx: usize, effect: &mut Effect) {
    match effect {
        Effect::Color {
            hue,
            saturation,
            brightness,
            contrast,
        } => {
            modulator_slider(ui, (idx, "hue"), "hue (deg)", hue, -180.0..=180.0);
            modulator_slider(ui, (idx, "sat"), "saturation", saturation, 0.0..=2.0);
            modulator_slider(ui, (idx, "bri"), "brightness", brightness, -1.0..=1.0);
            modulator_slider(ui, (idx, "con"), "contrast", contrast, 0.0..=2.0);
        }
        Effect::Tint { .. } => {
            ui.label("(Tint not yet implemented; see Effect::Tint stub)");
        }
        Effect::Blur { radius_px } => {
            modulator_slider(ui, (idx, "blur"), "radius (px)", radius_px, 0.0..=32.0);
        }
        Effect::Transform {
            translate,
            rotate_deg,
            scale_x,
            scale_y,
        } => {
            ui.add(egui::Slider::new(&mut translate[0], -1.0..=1.0).text("tx"));
            ui.add(egui::Slider::new(&mut translate[1], -1.0..=1.0).text("ty"));
            modulator_slider(ui, (idx, "rot"), "rotate (deg)", rotate_deg, -180.0..=180.0);
            modulator_slider(ui, (idx, "scx"), "scale x", scale_x, 0.1..=3.0);
            modulator_slider(ui, (idx, "scy"), "scale y", scale_y, 0.1..=3.0);
        }
        Effect::External { id, params } => {
            // Extension hook: no rich UI in v1. Display the registered id and
            // let advanced users edit `params` as raw JSON. Skipped at render
            // time when no ExternalPass is registered under `id`.
            ui.label(format!("id: {id}"));
            ui.label("params (JSON, edited via project file):");
            ui.label(serde_json::to_string_pretty(params).unwrap_or_else(|_| "<unprintable>".into()));
        }
    }
}

fn modulator_slider(
    ui: &mut Ui,
    salt: (usize, &'static str),
    label: &str,
    m: &mut Modulator,
    range: std::ops::RangeInclusive<f32>,
) {
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
            ui.label(
                "(audio: requires --features audio at build; reads live FFT bands)",
            );
        }
    }
    ui.add_space(2.0);
}
