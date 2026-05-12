//! 003-T3.2 — Layer thumbnail strip on the left edge of the control panel.
//!
//! A Procreate-style vertical list: one ~80 px wide row per layer with
//! a coloured-block thumbnail, visibility toggle, layer id label,
//! up/down reorder arrows, S (solo) and M (mute) buttons, and a
//! "+ Add image" tile at the bottom.
//!
//! ## Controls column layout (option A — 3-row stack)
//! Row 1: eye toggle (full `ctrl_w`, 18 px tall)
//! Row 2: S | M buttons (each `ctrl_w/2`, 16 px tall)
//! Row 3: ▲ | ▼ reorder arrows (each `ctrl_w/2`, 16 px tall)
//! Total: 50 px inside the 56 px `ROW_HEIGHT`.
//!
//! ## Visual state precedence
//! - **Muted row**: thumbnail and label are dimmed to ~50 % brightness.
//! - **Solo'd row**: warm accent ring (`theme::ACCENT`, 3 px stroke) drawn
//!   around the row, replacing the 2 px selection border when both apply.
//! - A row can be both selected AND solo'd: the solo ring takes precedence
//!   (operator intent — "this is the active layer right now").
//! - A row can be both muted AND solo'd: both the dim AND the accent ring are
//!   shown simultaneously (V31.6.1 render rule: soloed-and-muted still renders).
//!
//! Every interaction pushes a [`Mutation`] onto
//! `st.pending_mutations`; the App's per-frame drain routes them
//! through the undo stack.

use std::hash::{Hash, Hasher};

use egui::{Color32, Stroke, Ui};

#[cfg(feature = "v3")]
use crate::windows::anim;
use crate::windows::theme;

use crate::project::command::Mutation;
use crate::project::schema::{self, Project, ZoneRole};
use crate::windows::control_panel::ControlPanelState;
use crate::windows::scene_editor::{SceneEditorState, Selection};

// ── geometry constants ──────────────────────────────────────────────────────

/// Height of each layer row, pixels.
pub const ROW_HEIGHT: f32 = 56.0;
/// Thumbnail size within each row.
pub const THUMB_W: f32 = 64.0;
pub const THUMB_H: f32 = 36.0;
/// Highlight stroke colour — warm accent from the theme palette.
fn selected_colour() -> Color32 {
    theme::ACCENT
}

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

    Color32::from_rgba_unmultiplied(
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
        255,
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

// ── solo click helper ────────────────────────────────────────────────────────

/// Pure helper: compute the next `Project.solo` value when the user clicks the
/// S button on `clicked_idx`.
///
/// - If `current == Some(clicked_idx)` → un-solo (returns `None`).
/// - Otherwise → solo this layer (returns `Some(clicked_idx)`), which
///   implicitly clears any prior solo because `Project.solo` is a single
///   `Option<usize>`.
pub fn next_solo(current: Option<usize>, clicked_idx: usize) -> Option<usize> {
    if current == Some(clicked_idx) {
        None
    } else {
        Some(clicked_idx)
    }
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
                let is_muted = layer.muted;
                let is_solo = project.solo == Some(idx);

                // ── row allocation ──────────────────────────────────────
                let (row_rect, row_resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ROW_HEIGHT),
                    egui::Sense::click(),
                );

                // T4.15 — hover scale: expand the row's paint rect slightly
                // when the pointer is over it, using HOVER_FADE_MS for the
                // blend. The allocation stays fixed (avoids layout jitter);
                // only the painted region expands by up to 2%.
                let hover_id = ui.id().with(("layer_row_hover", idx));
                let hover_t =
                    anim::animate_bool_to(ui, hover_id, row_resp.hovered(), anim::HOVER_FADE_MS);
                // Expand from 0.0 to 2% of row height at full hover.
                let expand = hover_t * ROW_HEIGHT * 0.02;
                let draw_rect = row_rect.expand(expand);

                let painter = ui.painter_at(draw_rect);

                // ── row background ──────────────────────────────────────
                let bg = if is_selected {
                    // Warm tint from the theme accent, darkened.
                    theme::ACCENT.linear_multiply(0.08)
                } else {
                    theme::BG_PANEL
                };
                painter.rect_filled(draw_rect, egui::CornerRadius::same(2), bg);

                // ── highlight border (selected / solo precedence) ───────
                // Solo ring takes precedence over selection border when both
                // apply (operator intent: "this is the active layer right
                // now"). A muted+solo'd row shows both dim AND the ring.
                if is_solo {
                    // Solo ring: thicker warm accent stroke (3 px) so it reads
                    // as distinct from the 2 px selection border.
                    painter.rect_stroke(
                        draw_rect,
                        egui::CornerRadius::same(2),
                        Stroke::new(3.0, theme::ACCENT),
                        egui::StrokeKind::Inside,
                    );
                } else if is_selected {
                    painter.rect_stroke(
                        draw_rect,
                        egui::CornerRadius::same(2),
                        Stroke::new(2.0, selected_colour()),
                        egui::StrokeKind::Inside,
                    );
                }

                // ── thumbnail block ─────────────────────────────────────
                // Muted rows are dimmed to ~50 % brightness so the operator
                // can see at a glance which layers are suppressed.
                let dim = if is_muted { 0.4 } else { 1.0 };
                let thumb_colour = color_for_id(&layer.id).linear_multiply(dim);
                // P1.UX: left inset bumped 4 → 6 so the selected/solo
                // ring's 3 px Inside stroke doesn't kiss the thumb's
                // left edge. Mirrors the right-inset bump on
                // controls_rect.
                let thumb_rect = egui::Rect::from_min_size(
                    row_rect.min + egui::vec2(6.0, (ROW_HEIGHT - THUMB_H) * 0.5),
                    egui::vec2(THUMB_W, THUMB_H),
                );
                painter.rect_filled(thumb_rect, egui::CornerRadius::same(2), thumb_colour);

                // ── P1.5.1 video-row anatomy ─────────────────────────────
                // For Video layers, overlay (a) a loop-mode glyph in the
                // top-right corner of the thumbnail, and (b) two small
                // in/out markers along the thumbnail's bottom edge if
                // the layer carries a non-default trim. The inline scrub
                // bar (with hover thumbnails + click-to-seek) is deferred
                // to Phase 7 — it needs the thumbnail-extraction FFI
                // (P1.4.5's deferred half).
                if let schema::LayerKind::Video {
                    loop_mode,
                    clip_in,
                    clip_out,
                    ..
                } = &layer.kind
                {
                    // (a) loop glyph: ∞ / → / ⇆.
                    let glyph = match loop_mode {
                        schema::LoopMode::Loop => "∞",
                        schema::LoopMode::Once => "→",
                        schema::LoopMode::PingPong => "⇆",
                    };
                    painter.text(
                        egui::pos2(thumb_rect.right() - 3.0, thumb_rect.top() + 1.0),
                        egui::Align2::RIGHT_TOP,
                        glyph,
                        egui::FontId::proportional(11.0),
                        egui::Color32::WHITE.linear_multiply(dim * 0.9),
                    );

                    // (b) in/out markers. Only draw when the operator
                    // has trimmed away from the default (full clip).
                    // Position is normalised to thumb_rect width;
                    // absolute durations aren't known here (no asset
                    // probe), so we treat clip_in / clip_out as
                    // operator-facing seconds and clamp visually to
                    // a representative 0..60 s window. This is a
                    // **status indicator**, not a precise scrub —
                    // operators read the in/out values precisely from
                    // the Advanced > Video section.
                    let trimmed_in = *clip_in > 0.05;
                    let trimmed_out = clip_out.is_finite();
                    if trimmed_in || trimmed_out {
                        let bar_y = thumb_rect.bottom() - 3.0;
                        let bar_left = thumb_rect.left() + 2.0;
                        let bar_right = thumb_rect.right() - 2.0;
                        let bar_width = bar_right - bar_left;
                        // Thin grey backdrop for the trim region.
                        painter.line_segment(
                            [egui::pos2(bar_left, bar_y), egui::pos2(bar_right, bar_y)],
                            egui::Stroke::new(1.0, egui::Color32::from_white_alpha(60)),
                        );
                        // 60 s reference window — for clips longer
                        // than that the markers saturate at 1.0.
                        let ref_window = 60.0_f32;
                        let in_t = (clip_in / ref_window).clamp(0.0, 1.0);
                        let out_t = if clip_out.is_finite() {
                            (clip_out / ref_window).clamp(0.0, 1.0)
                        } else {
                            1.0
                        };
                        let mark = |t: f32, color: egui::Color32| {
                            let x = bar_left + t * bar_width;
                            painter.line_segment(
                                [egui::pos2(x, bar_y - 2.0), egui::pos2(x, bar_y + 2.0)],
                                egui::Stroke::new(1.5, color),
                            );
                        };
                        if trimmed_in {
                            mark(in_t, theme::ACCENT);
                        }
                        if trimmed_out {
                            mark(out_t, theme::ACCENT);
                        }
                    }
                }

                // ── layer id label ──────────────────────────────────────
                // `painter.text` does not auto-truncate; long filenames
                // (e.g. "skokugel_complete_20250511.png") would overflow
                // both horizontally past the thumbnail column AND
                // vertically into the next row. Lay out via a Galley with
                // an explicit `max_width = thumb_rect.width()` so egui
                // truncates with an ellipsis at the right boundary.
                let label_rect = egui::Rect::from_min_max(
                    egui::pos2(thumb_rect.left(), thumb_rect.bottom() + 2.0),
                    egui::pos2(thumb_rect.right(), row_rect.bottom() - 1.0),
                );
                let label_colour = theme::TEXT_PRIMARY.linear_multiply(dim);
                let galley = painter.layout(
                    layer.id.clone(),
                    egui::FontId::proportional(9.5),
                    label_colour,
                    thumb_rect.width(),
                );
                // Center horizontally inside `label_rect`; top-anchor
                // vertically (matches the previous CENTER_TOP align).
                let label_pos = egui::pos2(
                    label_rect.center().x - galley.size().x * 0.5,
                    label_rect.top(),
                );
                painter.galley(label_pos, galley, label_colour);

                // P3.4.2 — zone role badge: rendered below the id label in a
                // muted colour when `zone_role` is `Some`. Short abbreviation
                // so it fits under the thumbnail without overflow.
                if let Some(role) = layer.warp.zone_role {
                    let badge_text = match role {
                        ZoneRole::Window => "[window]",
                        ZoneRole::Portal => "[portal]",
                        ZoneRole::Void => "[void]",
                        ZoneRole::Spill => "[spill]",
                        ZoneRole::Edge => "[edge]",
                        ZoneRole::Highlight => "[highlight]",
                        ZoneRole::LightSource => "[light-src]",
                    };
                    let badge_colour = theme::TEXT_SECONDARY.linear_multiply(dim);
                    let badge_galley = painter.layout(
                        badge_text.to_string(),
                        egui::FontId::proportional(8.5),
                        badge_colour,
                        thumb_rect.width(),
                    );
                    let badge_pos = egui::pos2(
                        label_rect.center().x - badge_galley.size().x * 0.5,
                        label_rect.top() + 11.0, // below the id label
                    );
                    painter.galley(badge_pos, badge_galley, badge_colour);
                }

                // ── right-side controls column ──────────────────────────
                // Controls column to the right of the thumbnail: eye toggle + arrows.
                // P1.UX: right inset bumped 2 → 6 so the selection ring's
                // 3 px Inside stroke clears the controls column. M / ▼
                // were rendering past the ring's right stroke before.
                let ctrl_x = thumb_rect.right() + 4.0;
                let ctrl_w = row_rect.right() - ctrl_x - 6.0;

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
                    egui::pos2(row_rect.right() - 6.0, row_rect.bottom() - 6.0),
                );

                // Safety: `ui.allocate_rect` below might overlap with the already-allocated
                // row_rect, which is fine for the painter but child Ui widgets need a fresh
                // sub-ui. We use `ui.allocate_ui_at_rect` to punch a fresh child into the
                // controls column.
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(controls_rect));
                // P1.UX: the global theme bumped `item_spacing.x` 4 → 8
                // for breathing room in the Controls window, but inside
                // the layer rail's controls column that 8 px gap pushes
                // the S | M (and ▲ | ▼) pair past the available width
                // — `M` was rendering past the right edge of the rail.
                // Tighten spacing inside this child UI only; other UI
                // surfaces keep the wider gap.
                child.spacing_mut().item_spacing = egui::vec2(1.0, 2.0);
                child.spacing_mut().button_padding = egui::vec2(2.0, 1.0);

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

                // ── S / M buttons (row 2 of controls column) ───────────
                // Layout A: row 2 is a horizontal pair S | M, each ctrl_w/2.
                // Button fill hints: active state gets a faint accent tint so
                // the operator can read mute/solo state without inspecting the
                // row decoration.
                let (solo_resp, mute_resp) = child
                    .horizontal(|ui| {
                        let s_fill = if is_solo {
                            theme::ACCENT.linear_multiply(0.25)
                        } else {
                            Color32::TRANSPARENT
                        };
                        let s_resp = ui.add(
                            egui::Button::new(egui::RichText::new("S").size(10.0))
                                .min_size(egui::vec2(ctrl_w * 0.5 - 1.0, 16.0))
                                .fill(s_fill),
                        );

                        let m_fill = if is_muted {
                            theme::ACCENT.linear_multiply(0.15)
                        } else {
                            Color32::TRANSPARENT
                        };
                        let m_resp = ui.add(
                            egui::Button::new(egui::RichText::new("M").size(10.0))
                                .min_size(egui::vec2(ctrl_w * 0.5 - 1.0, 16.0))
                                .fill(m_fill),
                        );
                        (s_resp, m_resp)
                    })
                    .inner;

                if solo_resp.clicked() {
                    // Single-solo design: toggling S on this layer sets
                    // project.solo to Some(idx) or clears it if already set.
                    let new_solo = next_solo(project.solo, idx);
                    pending.push(project.set_solo_mutation(new_solo));
                }
                if mute_resp.clicked() {
                    pending.push(project.set_layer_muted_mutation(idx, !layer.muted));
                }

                // ── ▲ / ▼ reorder arrows (row 3 of controls column) ────
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
                // Guard all interactive controls so clicks on buttons don't
                // also change the layer selection.
                if row_resp.clicked()
                    && !eye_resp.clicked()
                    && !solo_resp.clicked()
                    && !mute_resp.clicked()
                {
                    scene.selected = Some(Selection::Layer(idx));
                }

                // ── right-click context menu ────────────────────────────
                // Single item for now: delete the layer. Cmd-Z undoes it.
                // No confirmation dialog — undo is the recovery path.
                row_resp.context_menu(|ui| {
                    if ui.button("Delete layer").clicked() {
                        pending.push(Mutation::RemoveLayer { idx });
                        ui.close();
                    }
                });
            }

            // ── "+ Add image" button ──────────────────────────────────────
            ui.add_space(4.0);
            let add_btn = ui.add(
                egui::Button::new(egui::RichText::new("+ Add image").size(11.0))
                    .min_size(egui::vec2(ui.available_width(), 28.0))
                    .fill(theme::BG_PANEL.linear_multiply(1.4)),
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

            // ── "+ Add FX layer" button ───────────────────────────────────────
            ui.add_space(2.0);
            let add_fx_btn = ui.add(
                egui::Button::new(egui::RichText::new("+ Add FX layer").size(11.0))
                    .min_size(egui::vec2(ui.available_width(), 28.0))
                    .fill(theme::BG_PANEL.linear_multiply(1.4)),
            );
            if add_fx_btn.clicked() {
                let id = unique_fx_layer_id(project);
                let params = crate::windows::preset_browser::default_params(
                    crate::render::fx_presets::RIPPLE_WASH_PRESET_ID,
                );
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0);
                let layer = schema::layer_from_fx_preset(
                    id,
                    crate::render::fx_presets::RIPPLE_WASH_PRESET_ID,
                    params,
                    seed,
                );
                let position = project.layers.len();
                pending.push(project.set_add_layer_mutation(layer, position));
            }
        });

    // Drain into ControlPanelState so the app's per-frame loop routes
    // everything through the undo stack in one pass.
    st.pending_mutations.extend(pending);
}

// ── helpers ───────────────────────────────────────────────────────────────

/// P2 follow-up — generate a non-colliding id for a new FX layer.
/// Walks `project.layers` looking for `fx_1`, `fx_2`, …, picking the first
/// that is not already taken.
fn unique_fx_layer_id(project: &crate::project::schema::Project) -> String {
    let mut n = 1u32;
    loop {
        let candidate = format!("fx_{n}");
        if !project.layers.iter().any(|l| l.id == candidate) {
            return candidate;
        }
        n += 1;
    }
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

    // ── next_solo helper ──────────────────────────────────────────────────

    /// Clicking S on the currently-solo'd layer un-solos it (returns None).
    #[test]
    fn next_solo_clears_current() {
        assert_eq!(next_solo(Some(1), 1), None);
    }

    /// Clicking S on a non-solo'd layer solos it.
    #[test]
    fn next_solo_sets_new() {
        assert_eq!(next_solo(None, 2), Some(2));
    }

    /// Clicking S on a different layer solos that layer (overrides previous solo).
    #[test]
    fn next_solo_switches_to_new_layer() {
        assert_eq!(next_solo(Some(0), 2), Some(2));
    }

    // ── solo / mute Mutation dispatch (V31.6.2) ───────────────────────────

    /// Build a `Project` with `n` stub layers for testing.
    fn make_test_project(n: usize) -> crate::project::schema::Project {
        use std::path::PathBuf;
        let mut p = crate::project::schema::Project::default();
        for i in 0..n {
            p.layers.push(crate::project::schema::layer_from_svg_path(
                format!("l{i}"),
                PathBuf::from(format!("/tmp/test_layer_{i}.svg")),
            ));
        }
        p
    }

    /// Clicking S on layer 1 (not currently solo'd): produces SetLayerSolo
    /// with new=Some(1), old=None.
    #[test]
    fn solo_button_toggles_solo() {
        use crate::project::command::Mutation;

        let mut p = make_test_project(3);

        // Click S on layer 1 — no prior solo.
        let new_val = next_solo(p.solo, 1);
        let m = p.set_solo_mutation(new_val);
        match &m {
            Mutation::SetLayerSolo(s) => {
                assert_eq!(s.new, Some(1), "new should be Some(1)");
                assert_eq!(s.old, None, "old should be None");
            }
            _ => panic!("expected SetLayerSolo"),
        }
        // Apply it so project.solo is updated.
        let _rev = m.apply(&mut p);
        assert_eq!(p.solo, Some(1));

        // Click S again on layer 1 — should un-solo.
        let new_val2 = next_solo(p.solo, 1);
        let m2 = p.set_solo_mutation(new_val2);
        match &m2 {
            Mutation::SetLayerSolo(s) => {
                assert_eq!(s.new, None, "second click should un-solo");
                assert_eq!(s.old, Some(1));
            }
            _ => panic!("expected SetLayerSolo"),
        }
        let _rev2 = m2.apply(&mut p);
        assert_eq!(p.solo, None);

        // Solo layer 1 again, then click S on layer 2 → overrides old solo.
        let _ = p.set_solo_mutation(Some(1)).apply(&mut p);
        assert_eq!(p.solo, Some(1));

        let new_val3 = next_solo(p.solo, 2);
        let m3 = p.set_solo_mutation(new_val3);
        match &m3 {
            Mutation::SetLayerSolo(s) => {
                assert_eq!(s.new, Some(2));
                assert_eq!(s.old, Some(1));
            }
            _ => panic!("expected SetLayerSolo"),
        }
        let _rev3 = m3.apply(&mut p);
        assert_eq!(p.solo, Some(2));
    }

    /// Clicking M on layer 0 toggles the muted flag and round-trips.
    #[test]
    fn mute_button_toggles_layer_muted() {
        use crate::project::command::Mutation;

        let mut p = make_test_project(3);
        assert!(!p.layers[0].muted, "initially not muted");

        // First click: mute layer 0.
        let m = p.set_layer_muted_mutation(0, !p.layers[0].muted);
        match &m {
            Mutation::SetLayerMuted(s) => {
                assert!(s.new);
                assert!(!s.old);
            }
            _ => panic!("expected SetLayerMuted"),
        }
        let _rev = m.apply(&mut p);
        assert!(p.layers[0].muted);

        // Second click: un-mute.
        let m2 = p.set_layer_muted_mutation(0, !p.layers[0].muted);
        match &m2 {
            Mutation::SetLayerMuted(s) => {
                assert!(!s.new);
                assert!(s.old);
            }
            _ => panic!("expected SetLayerMuted"),
        }
        let _rev2 = m2.apply(&mut p);
        assert!(!p.layers[0].muted);
    }

    // ── hsv_to_rgb sanity ─────────────────────────────────────────────────

    /// Pure red (h=0, s=1, v=1) should come back as (255, 0, 0).
    #[test]
    fn hsv_red() {
        let c = hsv_to_rgb(0.0, 1.0, 1.0);
        assert_eq!(c, Color32::RED);
    }

    /// Pure green (h=120, s=1, v=1) should come back as (0, 255, 0).
    #[test]
    fn hsv_green() {
        let c = hsv_to_rgb(120.0, 1.0, 1.0);
        assert_eq!(c, Color32::GREEN);
    }

    /// Pure blue (h=240, s=1, v=1) should come back as (0, 0, 255).
    #[test]
    fn hsv_blue() {
        let c = hsv_to_rgb(240.0, 1.0, 1.0);
        assert_eq!(c, Color32::BLUE);
    }
}
