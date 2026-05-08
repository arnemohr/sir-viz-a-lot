//! 003-T3.2 — Layer thumbnail strip on the left edge of the control panel.
//!
//! A Procreate-style vertical list: one ~80 px wide row per layer with
//! a coloured-block thumbnail, visibility toggle, layer id label,
//! up/down reorder arrows, and a "+ Add image" tile at the bottom.
//!
//! Every interaction pushes a [`Mutation`] onto
//! `st.pending_mutations`; the App's per-frame drain routes them
//! through the undo stack.

use std::hash::{Hash, Hasher};

use egui::{Color32, Stroke, Ui};

use crate::project::command::Mutation;
use crate::project::schema::{self, Project};
use crate::windows::control_panel::ControlPanelState;
use crate::windows::scene_editor::{SceneEditorState, Selection};

// ── geometry constants ──────────────────────────────────────────────────────

/// Height of each layer row, pixels.
pub const ROW_HEIGHT: f32 = 56.0;
/// Thumbnail size within each row.
pub const THUMB_W: f32 = 64.0;
pub const THUMB_H: f32 = 36.0;
/// Highlight stroke colour — matches the scene_editor's selected-handle colour.
const SELECTED_COLOUR: Color32 = Color32::from_rgb(255, 230, 110);

// ── colour derivation ───────────────────────────────────────────────────────

/// Produce a deterministic, visually-varied [`Color32`] for a layer based on
/// its `id` string. Uses a simple SipHash → hue rotation in HSV space.
///
/// The same `id` always returns the same colour; different ids (with very high
/// probability) return perceptually distinct colours. Saturation and value are
/// fixed so every thumbnail is equally vivid against a dark background.
pub fn color_for_id(id: &str) -> Color32 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    let h = hasher.finish();

    // Map the 64-bit hash onto a hue angle in [0°, 360°).
    let hue = (h % 360) as f32;
    let sat = 0.65_f32;
    let val = 0.75_f32;

    hsv_to_rgb(hue, sat, val)
}

/// Convert HSV (hue in degrees, saturation and value in [0, 1]) to [`Color32`].
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
    let h = h % 360.0;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    Color32::from_rgb(
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}

// ── hit testing ─────────────────────────────────────────────────────────────

/// Given a y-offset within the strip's scroll area (zero = top of first row),
/// return which row index is under that y. Returns `None` when `y_in_strip`
/// is negative or beyond the last row.
///
/// This is a pure function so it can be unit-tested without constructing a
/// [`Ui`]. Production code reaches this logic inline in the scroll area;
/// this helper exists so the math is exercisable in isolation.
#[allow(dead_code)] // Used in unit tests below; production hit-test is inline.
pub fn row_index_at_y(y_in_strip: f32, row_height: f32, n_layers: usize) -> Option<usize> {
    if y_in_strip < 0.0 {
        return None;
    }
    let idx = (y_in_strip / row_height.max(1.0)) as usize;
    if idx < n_layers { Some(idx) } else { None }
}

// ── main render ─────────────────────────────────────────────────────────────

/// Render the layer thumbnail strip into `ui`.
///
/// Pushes zero or more mutations onto `st.pending_mutations`.
pub fn show(
    ui: &mut Ui,
    project: &mut Project,
    st: &mut ControlPanelState,
    scene: &mut SceneEditorState,
) {
    let n = project.layers.len();
    let mut pending: Vec<Mutation> = Vec::new();

    egui::ScrollArea::vertical()
        .id_salt("layer_strip_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);

            for idx in 0..n {
                let layer = &project.layers[idx];
                let is_selected = matches!(scene.selected, Some(Selection::Layer(i)) if i == idx);

                // ── row allocation ──────────────────────────────────────
                let (row_rect, row_resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ROW_HEIGHT),
                    egui::Sense::click(),
                );

                let painter = ui.painter_at(row_rect);

                // ── row background ──────────────────────────────────────
                let bg = if is_selected {
                    Color32::from_rgb(45, 42, 35)
                } else {
                    Color32::from_rgb(28, 28, 32)
                };
                painter.rect_filled(row_rect, egui::CornerRadius::same(2), bg);

                // ── selected highlight border ───────────────────────────
                if is_selected {
                    painter.rect_stroke(
                        row_rect,
                        egui::CornerRadius::same(2),
                        Stroke::new(2.0, SELECTED_COLOUR),
                        egui::StrokeKind::Inside,
                    );
                }

                // ── thumbnail block ─────────────────────────────────────
                let thumb_colour = color_for_id(&layer.id);
                let thumb_rect = egui::Rect::from_min_size(
                    row_rect.min + egui::vec2(4.0, (ROW_HEIGHT - THUMB_H) * 0.5),
                    egui::vec2(THUMB_W, THUMB_H),
                );
                painter.rect_filled(thumb_rect, egui::CornerRadius::same(2), thumb_colour);

                // ── layer id label ──────────────────────────────────────
                let label_rect = egui::Rect::from_min_max(
                    egui::pos2(thumb_rect.left(), thumb_rect.bottom() + 2.0),
                    egui::pos2(thumb_rect.right(), row_rect.bottom() - 1.0),
                );
                painter.text(
                    label_rect.center_top(),
                    egui::Align2::CENTER_TOP,
                    &layer.id,
                    egui::FontId::proportional(9.5),
                    Color32::from_gray(190),
                );

                // ── right-side controls column ──────────────────────────
                // Controls column to the right of the thumbnail: eye toggle + arrows.
                let ctrl_x = thumb_rect.right() + 4.0;
                let ctrl_w = row_rect.right() - ctrl_x - 2.0;

                // We use child UI rects placed via `put`. In order to keep
                // allocations simple, we call `ui.put(rect, widget)` on the
                // *parent* ui — which requires the region to be allocated via
                // `allocate_rect`. However, since we're inside a custom
                // allocated `row_rect`, we use `painter` + manual hit-testing
                // instead of nested Ui widgets to avoid double-allocation
                // confusion. We add a small child Ui just for the interactive
                // controls.

                let controls_rect = egui::Rect::from_min_max(
                    egui::pos2(ctrl_x, row_rect.top() + 2.0),
                    egui::pos2(row_rect.right() - 2.0, row_rect.bottom() - 2.0),
                );

                // Safety: `ui.allocate_rect` below might overlap with the already-allocated
                // row_rect, which is fine for the painter but child Ui widgets need a fresh
                // sub-ui. We use `ui.allocate_ui_at_rect` to punch a fresh child into the
                // controls column.
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(controls_rect));

                let eye_label = if layer.enabled { "👁" } else { "○" };
                let eye_resp = child.add(
                    egui::Button::new(egui::RichText::new(eye_label).size(13.0))
                        .min_size(egui::vec2(ctrl_w, 18.0))
                        .fill(Color32::TRANSPARENT),
                );
                if eye_resp.clicked() {
                    // Visibility toggle — intercept before the row click
                    pending.push(project.set_layer_enabled_mutation(idx, !layer.enabled));
                }

                // Up / Down reorder arrows
                let up_enabled = idx > 0;
                let dn_enabled = idx + 1 < n;

                child.horizontal(|ui| {
                    ui.add_enabled_ui(up_enabled, |ui| {
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new("▲").size(10.0))
                                    .min_size(egui::vec2(ctrl_w * 0.5 - 1.0, 16.0))
                                    .fill(Color32::TRANSPARENT),
                            )
                            .clicked()
                        {
                            pending.push(project.set_swap_layers_mutation(idx, idx - 1));
                        }
                    });
                    ui.add_enabled_ui(dn_enabled, |ui| {
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new("▼").size(10.0))
                                    .min_size(egui::vec2(ctrl_w * 0.5 - 1.0, 16.0))
                                    .fill(Color32::TRANSPARENT),
                            )
                            .clicked()
                        {
                            pending.push(project.set_swap_layers_mutation(idx, idx + 1));
                        }
                    });
                });

                // ── row click → selection ───────────────────────────────
                // Only handle the row click if no child button consumed it.
                if row_resp.clicked() && !eye_resp.clicked() {
                    scene.selected = Some(Selection::Layer(idx));
                }
            }

            // ── "+ Add image" button ──────────────────────────────────────
            ui.add_space(4.0);
            let add_btn = ui.add(
                egui::Button::new(egui::RichText::new("+ Add image").size(11.0))
                    .min_size(egui::vec2(ui.available_width(), 28.0))
                    .fill(Color32::from_rgb(38, 42, 50)),
            );
            if add_btn.clicked() {
                if let Some(path) = crate::windows::file_dialogs::pick_image_to_add() {
                    let id = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("layer")
                        .to_string();
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let layer = if ext == "svg" {
                        schema::layer_from_svg_path(id, path)
                    } else {
                        schema::layer_from_image_path(id, path)
                    };
                    let position = project.layers.len();
                    pending.push(project.set_add_layer_mutation(layer, position));
                }
            }
        });

    // Drain into ControlPanelState so the app's per-frame loop routes
    // everything through the undo stack in one pass.
    st.pending_mutations.extend(pending);
}

// ── tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── color_for_id ──────────────────────────────────────────────────────

    /// Same id always produces the same colour (deterministic hash).
    #[test]
    fn color_for_id_is_deterministic() {
        let a = color_for_id("layer-foo");
        let b = color_for_id("layer-foo");
        assert_eq!(
            a, b,
            "color_for_id must return the same value for the same id"
        );
    }

    /// Two different ids should yield different colours (very high probability).
    #[test]
    fn color_for_id_differs_for_different_ids() {
        let a = color_for_id("alpha");
        let b = color_for_id("beta");
        assert_ne!(a, b, "distinct ids should produce distinct colours");
    }

    /// An empty string is a valid id; should not panic.
    #[test]
    fn color_for_id_empty_string_ok() {
        let _ = color_for_id("");
    }

    // ── row_index_at_y ────────────────────────────────────────────────────

    /// Clicking at the vertical center of row 0 yields `Some(0)`.
    #[test]
    fn row_index_center_of_row_zero() {
        let y = ROW_HEIGHT * 0.5;
        assert_eq!(row_index_at_y(y, ROW_HEIGHT, 3), Some(0));
    }

    /// Clicking at the vertical center of row 2 (of 3) yields `Some(2)`.
    #[test]
    fn row_index_center_of_row_two() {
        let y = ROW_HEIGHT * 2.5;
        assert_eq!(row_index_at_y(y, ROW_HEIGHT, 3), Some(2));
    }

    /// Clicking below all rows yields `None`.
    #[test]
    fn row_index_below_all_rows() {
        let y = ROW_HEIGHT * 5.0;
        assert_eq!(row_index_at_y(y, ROW_HEIGHT, 3), None);
    }

    /// Negative y yields `None`.
    #[test]
    fn row_index_negative_y() {
        assert_eq!(row_index_at_y(-1.0, ROW_HEIGHT, 3), None);
    }

    /// Zero layers: any positive y yields `None`.
    #[test]
    fn row_index_no_layers() {
        assert_eq!(row_index_at_y(10.0, ROW_HEIGHT, 0), None);
    }

    // ── hsv_to_rgb sanity ─────────────────────────────────────────────────

    /// Pure red (h=0, s=1, v=1) should come back as (255, 0, 0).
    #[test]
    fn hsv_red() {
        let c = hsv_to_rgb(0.0, 1.0, 1.0);
        assert_eq!(c, Color32::from_rgb(255, 0, 0));
    }

    /// Pure green (h=120, s=1, v=1) should come back as (0, 255, 0).
    #[test]
    fn hsv_green() {
        let c = hsv_to_rgb(120.0, 1.0, 1.0);
        assert_eq!(c, Color32::from_rgb(0, 255, 0));
    }

    /// Pure blue (h=240, s=1, v=1) should come back as (0, 0, 255).
    #[test]
    fn hsv_blue() {
        let c = hsv_to_rgb(240.0, 1.0, 1.0);
        assert_eq!(c, Color32::from_rgb(0, 0, 255));
    }
}
