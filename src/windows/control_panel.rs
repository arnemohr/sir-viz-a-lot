//! egui control panel: effects per layer, layer order, warp corners, scenes, gamma.

use std::path::{Path, PathBuf};

use egui::Ui;

use crate::effects::Effect;
use crate::modulators::Modulator;
use crate::project::schema::{self, BlendMode, Project, Scene};
use crate::project::{restore, snapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlTab {
    Effects,
    Layers,
    Mapping,
    Scenes,
}

impl Default for ControlTab {
    fn default() -> Self {
        Self::Effects
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
}

pub enum ControlPanelAction {
    None,
    /// Reload GPU layer runtime from `project.layers` paths.
    RebuildLayers,
}

/// Render the control panel. Mutates `project` in place.
pub fn show(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
) -> ControlPanelAction {
    let mut action = ControlPanelAction::None;

    egui::Panel::top("rmap_tabs")
        .resizable(false)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut st.tab, ControlTab::Effects, "Effects");
                ui.selectable_value(&mut st.tab, ControlTab::Layers, "Layers");
                ui.selectable_value(&mut st.tab, ControlTab::Mapping, "Mapping");
                ui.selectable_value(&mut st.tab, ControlTab::Scenes, "Scenes");
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        match st.tab {
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

fn show_effects_tab(ui: &mut Ui, project: &mut Project, st: &mut ControlPanelState) {
    if project.layers.is_empty() {
        ui.label("No layers — open an SVG as the first argument.");
        return;
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
                ui.label(layer.svg_path.display().to_string());
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
    if w.rows != 1 || w.cols != 1 || w.grid.len() != 2 || w.grid[0].len() != 2 {
        ui.label("Mapping UI supports 1×1 corner-pin only for now.");
        return;
    }
    ui.label("Corner pin (normalized 0–1 output space).");
    corner_slider(ui, &mut w.grid[0][0], "UL");
    corner_slider(ui, &mut w.grid[0][1], "UR");
    corner_slider(ui, &mut w.grid[1][1], "BR");
    corner_slider(ui, &mut w.grid[1][0], "BL");
    ui.add(egui::Slider::new(&mut w.mask_feather, 0.0..=0.25).text("mask feather"));
    ui.label("Mask polygon: edit JSON/project file for now (vec of [x,y]).");
}

fn corner_slider(ui: &mut Ui, p: &mut [f32; 2], label: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(&mut p[0], 0.0..=1.0).text("x"));
        ui.add(egui::Slider::new(&mut p[1], 0.0..=1.0).text("y"));
    });
}

fn show_scenes_tab(ui: &mut Ui, project: &mut Project) -> ControlPanelAction {
    let mut action = ControlPanelAction::None;
    ui.label("Slots 1–9 (keyboard recall). Save captures full project JSON.");
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
            if ui.button("recall").clicked() {
                if let Some(sc) = project.scenes.get(slot) {
                    let snap = sc.snapshot.clone();
                    if restore(project, &snap).is_ok() {
                        action = ControlPanelAction::RebuildLayers;
                    }
                }
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
    }
    ui.add_space(2.0);
}
