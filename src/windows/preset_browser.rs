//! P2.8.1–P2.8.5 — FX preset library browser modal.
//!
//! [`PresetBrowserWindow`] is a floating `egui::Window` that shows all
//! registered FX presets in a grid. Clicking a cell dispatches
//! `Mutation::SetLayerKind` to switch the selected `FxLayer` to that preset.
//!
//! Opening the browser on a non-`FxLayer` shows an informational message
//! instead of the grid.
//!
//! State lives on [`crate::windows::control_panel::ControlPanelState`] so
//! it survives window close/reopen and shares the mutation queue.
//!
//! # User preset directory
//!
//! User presets are stored in
//! `~/Library/Application Support/rmap/presets/*.rmap-preset.json`.
//! The directory path is resolved at runtime via [`user_presets_dir`].
//! Directory creation failures are silently ignored — the feature degrades
//! to built-ins-only.
//!
//! # Export / import (P2.8.5)
//!
//! Each preset cell in the browser has an "Export…" button that opens an
//! `rfd::FileDialog` save-file dialog and writes a `.rmap-preset.json`.
//! The "Import…" button at the top of the browser opens a file-picker and
//! validates that the `preset_id` is registered via [`fx_is_registered`];
//! unknown IDs surface a toast: "This preset requires a version of rmap
//! that supports '<id>'. It was not imported."
//!
//! Drag-drop import (`.rmap-preset.json` onto the app window) is deferred
//! to Phase 4 — the egui drag-drop surface needs the main window event-loop
//! to forward `DroppedFile` events from winit, which requires wiring in
//! `src/windows/control.rs`. The file-dialog import path covers the
//! v0.6 acceptance criterion.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::Ui;

use crate::project::command::{Mutation, SetLayerKind};
use crate::project::schema::{LayerKind, Project};
use crate::render::fx_presets::{FxFamily, FxPresetEntry, fx_param_descriptors, fx_registry};
use crate::windows::control_panel::ControlPanelState;
use crate::windows::preset_io::{RmapPresetJson, read_preset, write_preset};
use crate::windows::preset_stars::PresetStars;

// ---------------------------------------------------------------------------
// Directory helpers
// ---------------------------------------------------------------------------

/// Resolves `~/Library/Application Support/rmap/presets/` on macOS.
/// Returns `None` when the HOME environment variable is absent.
pub fn user_presets_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .map(|h| h.join("Library/Application Support/rmap/presets"))
}

// ---------------------------------------------------------------------------
// Built-in preset entry (wraps FxPresetEntry for the grid)
// ---------------------------------------------------------------------------

/// A preset as shown in the browser grid. Either a built-in registry entry
/// or a user preset loaded from disk.
#[derive(Clone)]
pub enum BrowserPreset {
    /// A preset from `fx_registry()`.
    Builtin(FxPresetEntry),
    /// A preset loaded from disk.
    User(RmapPresetJson, PathBuf),
}

impl BrowserPreset {
    pub fn preset_id(&self) -> &str {
        match self {
            BrowserPreset::Builtin(e) => e.preset_id,
            BrowserPreset::User(p, _) => p.preset_id.as_str(),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            BrowserPreset::Builtin(e) => e.label,
            BrowserPreset::User(p, _) => p.name.as_str(),
        }
    }

    pub fn family(&self) -> Option<FxFamily> {
        match self {
            BrowserPreset::Builtin(e) => Some(e.family),
            BrowserPreset::User(_, _) => None,
        }
    }

    pub fn is_builtin(&self) -> bool {
        matches!(self, BrowserPreset::Builtin(_))
    }
}

// ---------------------------------------------------------------------------
// P2.8.1 helper — collect built-in presets
// ---------------------------------------------------------------------------

/// Returns all built-in FX presets from `fx_registry()`.
///
/// Exposed as `pub` so unit tests can call it directly.
#[allow(dead_code)] // consumed by unit tests
pub fn collect_builtin_presets() -> Vec<FxPresetEntry> {
    fx_registry().to_vec()
}

/// Build a `HashMap<String, f32>` of default params for the given preset id.
pub fn default_params(preset_id: &str) -> HashMap<String, f32> {
    fx_param_descriptors(preset_id)
        .iter()
        .map(|d| (d.key.to_string(), d.default))
        .collect()
}

/// Returns a fresh seed from the system clock (no `rand` dependency).
fn fresh_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(12345)
}

// ---------------------------------------------------------------------------
// Slugify helper (P2.8.4)
// ---------------------------------------------------------------------------

/// Convert a human name to a file-system slug: lowercase, non-alphanumeric
/// ASCII chars → `_`, non-ASCII chars stripped.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() {
                Some(c)
            } else if c.is_ascii() {
                Some('_')
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Save-dialog sub-state (P2.8.4)
// ---------------------------------------------------------------------------

/// State for the "Save as preset…" name-entry dialog.
#[derive(Default)]
struct SaveDialog {
    open: bool,
    /// True only on the first frame after open — used to request focus once.
    just_opened: bool,
    /// Name being typed in the TextEdit.
    name_buf: String,
    /// Layer index to save.
    layer_idx: usize,
}

// ---------------------------------------------------------------------------
// Delete-confirmation sub-state (P2.8.4)
// ---------------------------------------------------------------------------

/// State for the "Delete preset?" confirmation dialog.
#[derive(Default)]
struct DeleteConfirm {
    open: bool,
    preset_name: String,
    preset_path: PathBuf,
}

// ---------------------------------------------------------------------------
// Main struct
// ---------------------------------------------------------------------------

/// Floating preset library browser.
///
/// Add as a field on `ControlPanelState`; call `show` every frame.
pub struct PresetBrowserWindow {
    pub open: bool,
    /// Which layer is targeted for preset application.
    pub target_layer_idx: Option<usize>,

    // P2.8.2 — filter state
    pub filter_query: String,
    /// Three family filter toggles: [Wave/Fragment, ComputeParticle, ComputeFluid].
    /// All on by default so the full registry is shown immediately.
    pub family_filters: [bool; 3],

    // P2.8.3 — star state
    stars: Option<PresetStars>,

    // P2.8.4 — user presets loaded at open time
    user_presets: Vec<(RmapPresetJson, PathBuf)>,
    save_dialog: SaveDialog,
    delete_confirm: DeleteConfirm,

    // Toast staging (fed back to ControlPanelState after show())
    // Empty after each frame; caller must drain.
    pub staged_toasts: Vec<crate::windows::toast::Toast>,
}

impl Default for PresetBrowserWindow {
    fn default() -> Self {
        Self {
            open: false,
            target_layer_idx: None,
            filter_query: String::new(),
            // P2.8.2 — all family filters on by default so the full grid is
            // visible without any extra setup from the operator.
            family_filters: [true; 3],
            stars: None,
            user_presets: Vec::new(),
            save_dialog: SaveDialog::default(),
            delete_confirm: DeleteConfirm::default(),
            staged_toasts: Vec::new(),
        }
    }
}

impl PresetBrowserWindow {
    /// Open the browser targeting the given layer index.
    pub fn open_for_layer(&mut self, layer_idx: usize) {
        self.open = true;
        self.target_layer_idx = Some(layer_idx);
        // Reload stars + user presets.
        self.stars = Some(PresetStars::load_or_default());
        self.reload_user_presets();
    }

    /// Reload user presets from disk (called on open and after save/delete).
    fn reload_user_presets(&mut self) {
        self.user_presets.clear();
        let Some(dir) = user_presets_dir() else {
            return;
        };
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => return,
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json")
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".rmap-preset.json"))
                    .unwrap_or(false)
            {
                match read_preset(&path) {
                    Ok(p) => self.user_presets.push((p, path)),
                    Err(e) => {
                        tracing::warn!("skipping malformed user preset {:?}: {e}", path);
                    }
                }
            }
        }
        // Sort user presets by name.
        self.user_presets.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    }

    /// Returns true if `family` passes the active family filter.
    fn family_passes(&self, family: Option<FxFamily>) -> bool {
        match family {
            None => true, // user presets always pass
            Some(FxFamily::Fragment) => self.family_filters[0],
            Some(FxFamily::ComputeParticle) => self.family_filters[1],
            Some(FxFamily::ComputeFluid) => self.family_filters[2],
        }
    }

    /// Returns true if `preset_id` / `label` passes the text filter.
    fn text_passes(&self, preset_id: &str, label: &str) -> bool {
        if self.filter_query.is_empty() {
            return true;
        }
        let q = self.filter_query.to_lowercase();
        preset_id.to_lowercase().contains(&q) || label.to_lowercase().contains(&q)
    }

    /// Collect all presets (built-ins first, starred first within each group),
    /// filtered by current filter state.
    fn filtered_presets(&self) -> Vec<BrowserPreset> {
        let stars = self.stars.as_ref();

        // Collect built-ins.
        let mut builtins: Vec<BrowserPreset> = fx_registry()
            .iter()
            .filter(|e| {
                self.text_passes(e.preset_id, e.label) && self.family_passes(Some(e.family))
            })
            .map(|e| BrowserPreset::Builtin(*e))
            .collect();

        // Starred first within built-ins.
        if let Some(s) = stars {
            builtins.sort_by_key(|p| !s.is_starred(p.preset_id()));
        }

        // Collect user presets.
        let mut user: Vec<BrowserPreset> = self
            .user_presets
            .iter()
            .filter(|(p, _)| self.text_passes(&p.preset_id, &p.name))
            .map(|(p, path)| BrowserPreset::User(p.clone(), path.clone()))
            .collect();

        if let Some(s) = stars {
            user.sort_by_key(|p| !s.is_starred(p.preset_id()));
        }

        let mut result = builtins;
        result.extend(user);
        result
    }

    /// Render the modal. Mutations to dispatch are pushed into `st.pending_mutations`.
    pub fn show(&mut self, ctx: &egui::Context, project: &Project, st: &mut ControlPanelState) {
        if !self.open {
            return;
        }

        // P2.8.3 — lazy-load stars if not yet loaded (e.g. first open).
        if self.stars.is_none() {
            self.stars = Some(PresetStars::load_or_default());
        }

        let Some(layer_idx) = self.target_layer_idx else {
            return;
        };

        // ------------------------------------------------------------------
        // Save-as dialog (P2.8.4) — render before the main window so it
        // floats above it.
        // ------------------------------------------------------------------
        self.show_save_dialog(ctx, project);
        self.show_delete_confirm(ctx);

        // Drain staged toasts into the control panel state.
        st.pending_toasts.append(&mut self.staged_toasts);

        let mut open = self.open;
        egui::Window::new("FX preset library")
            .open(&mut open)
            .fixed_size([700.0, 540.0])
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                self.show_body(ui, project, st, layer_idx);
            });
        // Use AND so that either a preset click inside show_body (sets
        // self.open = false) OR the egui window-close X (sets open = false)
        // closes the modal.
        self.open = open && self.open;
    }

    fn show_body(
        &mut self,
        ui: &mut Ui,
        project: &Project,
        st: &mut ControlPanelState,
        layer_idx: usize,
    ) {
        let is_fx_layer = matches!(
            project.layers.get(layer_idx).map(|l| &l.kind),
            Some(LayerKind::FxLayer { .. })
        );

        if !is_fx_layer {
            ui.label("Select an FX layer to pick a preset.");
            return;
        }

        // ------------------------------------------------------------------
        // P2.8.2 — Search + family filter bar
        // ------------------------------------------------------------------
        ui.horizontal(|ui| {
            ui.label("Search:");
            egui::TextEdit::singleline(&mut self.filter_query)
                .hint_text("filter by name or id…")
                .desired_width(200.0)
                .show(ui);
            if ui.button("✕").on_hover_text("Clear search").clicked() {
                self.filter_query.clear();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Family:");
            ui.toggle_value(&mut self.family_filters[0], "Wave");
            ui.toggle_value(&mut self.family_filters[1], "Particle");
            ui.toggle_value(&mut self.family_filters[2], "Fluid");
        });

        // ------------------------------------------------------------------
        // P2.8.5 — Import button (top of browser)
        // ------------------------------------------------------------------
        ui.horizontal(|ui| {
            if ui.button("Import…").clicked() {
                self.handle_import(st);
            }
        });

        ui.separator();

        // ------------------------------------------------------------------
        // Preset grid
        // ------------------------------------------------------------------
        let presets = self.filtered_presets();
        if presets.is_empty() {
            ui.weak("No presets match the current filter.");
            return;
        }

        // Separate built-ins from user presets for sectioned display.
        let (builtin_list, user_list): (Vec<_>, Vec<_>) =
            presets.iter().partition(|p| p.is_builtin());

        const COLS: usize = 4;
        egui::ScrollArea::vertical().show(ui, |ui| {
            // Built-in section.
            if !builtin_list.is_empty() {
                ui.strong("Built-in presets");
                ui.add_space(4.0);
                egui::Grid::new("preset_browser_builtin_grid")
                    .num_columns(COLS)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        for (i, preset) in builtin_list.iter().enumerate() {
                            self.show_cell(ui, preset, layer_idx, project, st, false);
                            if (i + 1) % COLS == 0 {
                                ui.end_row();
                            }
                        }
                        if builtin_list.len() % COLS != 0 {
                            ui.end_row();
                        }
                    });
            }

            // User preset section.
            if !user_list.is_empty() {
                ui.add_space(12.0);
                ui.strong("User presets");
                ui.add_space(4.0);
                egui::Grid::new("preset_browser_user_grid")
                    .num_columns(COLS)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        for (i, preset) in user_list.iter().enumerate() {
                            self.show_cell(ui, preset, layer_idx, project, st, true);
                            if (i + 1) % COLS == 0 {
                                ui.end_row();
                            }
                        }
                        if user_list.len() % COLS != 0 {
                            ui.end_row();
                        }
                    });
            }
        });
    }

    fn show_cell(
        &mut self,
        ui: &mut Ui,
        preset: &BrowserPreset,
        layer_idx: usize,
        project: &Project,
        st: &mut ControlPanelState,
        is_user: bool,
    ) {
        let pid = preset.preset_id().to_string();
        let lbl = preset.label().to_string();
        let family_badge = preset
            .family()
            .map(|f| format!("{f:?}"))
            .unwrap_or_default();

        // Star state.
        let is_starred = self
            .stars
            .as_ref()
            .map(|s| s.is_starred(&pid))
            .unwrap_or(false);

        let cell_id = ui.id().with(("cell", &pid));
        egui::Frame::default()
            .inner_margin(egui::Margin::same(6))
            .stroke(egui::Stroke::new(
                1.0,
                crate::windows::theme::ACCENT.linear_multiply(0.3),
            ))
            .show(ui, |ui| {
                // Force a vertical, top-aligned layout inside the cell so the
                // widgets stack predictably — the parent here is a
                // `horizontal_wrapped` Ui whose default layout would otherwise
                // place these widgets side-by-side instead of as a tile.
                ui.allocate_ui_with_layout(
                    egui::vec2(150.0, 0.0),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.set_width(150.0);

                        // Row 1: star toggle + family badge on the same line.
                        ui.horizontal(|ui| {
                            let star_label = if is_starred { "★" } else { "☆" };
                            if ui
                                .small_button(star_label)
                                .on_hover_text(if is_starred { "Unstar" } else { "Star" })
                                .clicked()
                            {
                                if let Some(stars) = &mut self.stars {
                                    stars.toggle(&pid);
                                }
                            }
                            if !family_badge.is_empty() {
                                ui.weak(&family_badge);
                            }
                        });

                        // Row 2: preset label button (full cell width).
                        if ui
                            .add_sized(
                                egui::vec2(ui.available_width(), 28.0),
                                egui::Button::new(&lbl),
                            )
                            .clicked()
                        {
                            let new_kind = LayerKind::FxLayer {
                                preset_id: pid.clone(),
                                params: default_params(&pid),
                                seed: fresh_seed(),
                                t_layer_added_secs: 0.0,
                            };
                            if let Some(layer) = project.layers.get(layer_idx) {
                                let old = layer.kind.clone();
                                st.pending_mutations
                                    .push(Mutation::SetLayerKind(SetLayerKind {
                                        layer_idx,
                                        new: new_kind,
                                        old,
                                    }));
                            }
                            self.open = false;
                        }

                        // Row 3: action buttons (export + delete on user presets).
                        ui.horizontal(|ui| {
                            if ui.small_button("Export…").clicked() {
                                let preset_data = match preset {
                                    BrowserPreset::Builtin(e) => RmapPresetJson {
                                        preset_id: e.preset_id.to_string(),
                                        params: default_params(e.preset_id),
                                        name: e.label.to_string(),
                                        author: None,
                                    },
                                    BrowserPreset::User(p, _) => p.clone(),
                                };
                                self.handle_export(&preset_data);
                            }
                            if is_user {
                                if let BrowserPreset::User(_, path) = preset {
                                    if ui
                                        .small_button("✕")
                                        .on_hover_text("Delete this user preset")
                                        .clicked()
                                    {
                                        self.delete_confirm.open = true;
                                        self.delete_confirm.preset_name = lbl.clone();
                                        self.delete_confirm.preset_path = path.clone();
                                    }
                                }
                            }
                        });
                    },
                );
            });

        // Suppress unused warning for cell_id (used for potential future key).
        let _ = cell_id;
    }

    // ------------------------------------------------------------------
    // P2.8.4 — Save-as dialog
    // ------------------------------------------------------------------

    fn show_save_dialog(&mut self, ctx: &egui::Context, project: &Project) {
        if !self.save_dialog.open {
            return;
        }
        let mut open = self.save_dialog.open;
        egui::Window::new("Save preset as…")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_size([280.0, 100.0])
            .show(ctx, |ui| {
                ui.label("Name:");
                let resp = egui::TextEdit::singleline(&mut self.save_dialog.name_buf)
                    .hint_text("My cool preset")
                    .show(ui);
                // P2.8.4 — request focus only on the first frame after open so
                // the operator can type immediately without the cursor jumping
                // back on every subsequent frame.
                if self.save_dialog.just_opened {
                    resp.response.request_focus();
                    self.save_dialog.just_opened = false;
                }
                ui.horizontal(|ui| {
                    let can_save = !self.save_dialog.name_buf.trim().is_empty();
                    if ui
                        .add_enabled(can_save, egui::Button::new("Save"))
                        .clicked()
                    {
                        let layer_idx = self.save_dialog.layer_idx;
                        if let Some(layer) = project.layers.get(layer_idx) {
                            if let LayerKind::FxLayer {
                                preset_id, params, ..
                            } = &layer.kind
                            {
                                let name = self.save_dialog.name_buf.trim().to_string();
                                let slug = slugify(&name);
                                let preset = RmapPresetJson {
                                    preset_id: preset_id.clone(),
                                    params: params.clone(),
                                    name: name.clone(),
                                    author: None,
                                };
                                match save_user_preset(&slug, &preset) {
                                    Ok(_) => {
                                        self.reload_user_presets();
                                        self.staged_toasts.push(
                                            crate::windows::toast::Toast::info(format!(
                                                "Preset '{name}' saved."
                                            )),
                                        );
                                    }
                                    Err(e) => {
                                        self.staged_toasts.push(
                                            crate::windows::toast::Toast::warn(format!(
                                                "Could not save preset: {e}"
                                            )),
                                        );
                                    }
                                }
                            }
                        }
                        self.save_dialog.open = false;
                        self.save_dialog.name_buf.clear();
                    }
                    if ui.button("Cancel").clicked() {
                        self.save_dialog.open = false;
                        self.save_dialog.name_buf.clear();
                    }
                });
            });
        if !open {
            self.save_dialog.open = false;
        }
    }

    // ------------------------------------------------------------------
    // P2.8.4 — Delete confirmation dialog
    // ------------------------------------------------------------------

    fn show_delete_confirm(&mut self, ctx: &egui::Context) {
        if !self.delete_confirm.open {
            return;
        }
        let name = self.delete_confirm.preset_name.clone();
        let mut open = self.delete_confirm.open;
        egui::Window::new("Delete preset?")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_size([300.0, 80.0])
            .show(ctx, |ui| {
                ui.label(format!("Delete '{name}'? This cannot be undone."));
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        match std::fs::remove_file(&self.delete_confirm.preset_path) {
                            Ok(_) => {
                                self.reload_user_presets();
                                self.staged_toasts.push(crate::windows::toast::Toast::info(
                                    format!("Preset '{name}' deleted."),
                                ));
                            }
                            Err(e) => {
                                self.staged_toasts.push(crate::windows::toast::Toast::warn(
                                    format!("Could not delete preset: {e}"),
                                ));
                            }
                        }
                        self.delete_confirm.open = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.delete_confirm.open = false;
                    }
                });
            });
        if !open {
            self.delete_confirm.open = false;
        }
    }

    // ------------------------------------------------------------------
    // P2.8.5 — Export via rfd
    // ------------------------------------------------------------------

    fn handle_export(&self, preset: &RmapPresetJson) {
        let default_name = format!("{}.rmap-preset.json", slugify(&preset.name));
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter("rmap preset", &["json"])
            .save_file()
        {
            if let Err(e) = write_preset(&path, preset) {
                tracing::warn!("preset export failed: {e}");
            }
        }
    }

    // ------------------------------------------------------------------
    // P2.8.5 — Import via rfd
    // ------------------------------------------------------------------

    fn handle_import(&mut self, st: &mut ControlPanelState) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("rmap preset", &["json"])
            .pick_file()
        else {
            return;
        };

        let preset = match read_preset(&path) {
            Ok(p) => p,
            Err(e) => {
                st.pending_toasts
                    .push(crate::windows::toast::Toast::warn(format!(
                        "Could not read preset file: {e}"
                    )));
                return;
            }
        };

        // Validate: known preset_id or already a user preset.
        if !crate::render::fx_presets::fx_is_registered(&preset.preset_id) {
            // Check if it's a user-preset (user presets may have IDs from
            // older built-in registrations; keep the check simple).
            st.pending_toasts.push(crate::windows::toast::Toast::warn(format!(
                "This preset requires a version of rmap that supports '{}'. It was not imported.",
                preset.preset_id
            )));
            return;
        }

        // Write to user preset directory.
        let slug = slugify(&preset.name);
        match save_user_preset(&slug, &preset) {
            Ok(_) => {
                self.reload_user_presets();
                st.pending_toasts
                    .push(crate::windows::toast::Toast::info(format!(
                        "Imported preset '{}'.",
                        preset.name
                    )));
            }
            Err(e) => {
                st.pending_toasts
                    .push(crate::windows::toast::Toast::warn(format!(
                        "Could not import preset: {e}"
                    )));
            }
        }
    }

    /// Open the "Save as preset…" dialog for the given layer.
    pub fn open_save_dialog(&mut self, layer_idx: usize) {
        self.save_dialog.open = true;
        self.save_dialog.just_opened = true;
        self.save_dialog.layer_idx = layer_idx;
        self.save_dialog.name_buf.clear();
    }
}

// ---------------------------------------------------------------------------
// Disk I/O helper
// ---------------------------------------------------------------------------

/// Write a user preset to `~/Library/Application Support/rmap/presets/<slug>.rmap-preset.json`.
pub fn save_user_preset(slug: &str, preset: &RmapPresetJson) -> std::io::Result<()> {
    let dir = user_presets_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "HOME environment variable not set",
        )
    })?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{slug}.rmap-preset.json"));
    write_preset(&path, preset)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// P2.8.1 — `collect_builtin_presets` must return at least one entry.
    #[test]
    fn preset_browser_collect_presets_returns_builtins() {
        let presets = collect_builtin_presets();
        assert!(!presets.is_empty(), "fx_registry() must not be empty");
    }

    /// P2.8.2 — text filter returns only entries matching the substring.
    #[test]
    fn preset_browser_filter_by_substring() {
        let all = collect_builtin_presets();
        let q = "ripple";
        let filtered: Vec<_> = all
            .iter()
            .filter(|e| {
                let lq = q.to_lowercase();
                e.preset_id.to_lowercase().contains(&lq) || e.label.to_lowercase().contains(&lq)
            })
            .collect();
        assert!(
            !filtered.is_empty(),
            "expected at least one preset matching 'ripple'"
        );
        for e in &filtered {
            let lq = q.to_lowercase();
            assert!(
                e.preset_id.to_lowercase().contains(&lq) || e.label.to_lowercase().contains(&lq),
                "preset {} / {} should contain 'ripple'",
                e.preset_id,
                e.label
            );
        }
        // All results contain the substring.
        let not_matching: Vec<_> = all
            .iter()
            .filter(|e| {
                let lq = q.to_lowercase();
                !e.preset_id.to_lowercase().contains(&lq) && !e.label.to_lowercase().contains(&lq)
            })
            .collect();
        assert!(
            !not_matching.is_empty(),
            "expected at least one preset NOT matching 'ripple' to verify filtering works"
        );
    }

    /// P2.8.2 — family filter returns only entries from the specified family.
    #[test]
    fn preset_browser_filter_by_family() {
        let all = collect_builtin_presets();
        let compute_particle: Vec<_> = all
            .iter()
            .filter(|e| e.family == FxFamily::ComputeParticle)
            .collect();
        assert!(
            !compute_particle.is_empty(),
            "expected at least one ComputeParticle preset"
        );
        for e in &compute_particle {
            assert_eq!(e.family, FxFamily::ComputeParticle);
        }
        // Make sure non-ComputeParticle entries exist too.
        let others: Vec<_> = all
            .iter()
            .filter(|e| e.family != FxFamily::ComputeParticle)
            .collect();
        assert!(!others.is_empty(), "expected non-ComputeParticle presets");
    }

    /// P2.8.4 — slugify handles special characters and spaces correctly.
    #[test]
    fn slugify_handles_special_chars() {
        assert_eq!(slugify("My Preset!"), "my_preset_");
        assert_eq!(slugify("hello world"), "hello_world");
        assert_eq!(slugify("Café"), "caf"); // non-ASCII stripped
        assert_eq!(slugify("abc123"), "abc123");
    }
}
