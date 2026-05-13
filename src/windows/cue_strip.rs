//! 003-T4.2–T4.5 — Horizontal cue strip below the canvas.
//!
//! Renders one tile (~160×100) per saved scene plus a "+" tile at the
//! right end. Clicking a scene tile emits `Command::SceneRecall(idx)`;
//! clicking "+" emits `Command::SceneSave`. While a crossfade is in
//! progress the target tile shows a thin progress bar along its bottom
//! edge (T4.4). When there are no scenes an empty-state message is shown
//! alongside the lone "+" tile (T4.5).
//!
//! # Thumbnail generation (T4.1 — placeholder path)
//!
//! Full GPU readback from `warp_rt` is deferred to a T4.1 follow-up.
//! For now, [`placeholder_thumbnail_for_name`] returns a 192×108 RGBA8
//! gradient derived from the scene name's hash, giving each cue a
//! visually distinct colour without requiring GPU resources at save time.
//!
//! TODO 003-T4.1 follow-up: replace placeholder with GPU readback from
//! `warp_rt` (post-warp, pre-gamma) via `wgpu::CommandEncoder::copy_texture_to_buffer`.

use std::collections::HashMap;

use egui::{Color32, FontId, Rect, Response, RichText, Sense, TextureHandle, Ui, vec2};

use crate::windows::theme;

use crate::controls::Command;
use crate::project::schema::{Project, ThumbnailRgba};

/// Thumbnail dimensions (pixels).
pub const THUMB_W: u32 = 192;
pub const THUMB_H: u32 = 108;

/// Tile render dimensions (egui logical pixels).
const TILE_W: f32 = 160.0;
const TILE_H: f32 = 100.0;
const PLUS_TILE_W: f32 = 80.0;
/// Height of the strip panel (includes padding).
pub const STRIP_HEIGHT: f32 = 120.0;

/// 003-T4.1 placeholder — derives a 192×108 RGBA8 gradient from the scene
/// name's hash so each cue has a unique tint without GPU readback.
///
/// The gradient runs top-left (hue derived from hash) → bottom-right
/// (slightly shifted hue), giving a diagonal sweep that reads as a
/// distinct colour field even at small tile sizes.
///
/// TODO 003-T4.1 follow-up: replace with GPU readback from warp_rt.
pub fn placeholder_thumbnail_for_name(name: &str) -> ThumbnailRgba {
    // Stable hash: FNV-1a over UTF-8 bytes.
    let h = name.bytes().fold(2166136261u32, |acc, b| {
        acc.wrapping_mul(16777619) ^ b as u32
    });
    // Map hash to two hues (0..360).
    let hue_a = (h & 0xFFFF) as f32 / 65535.0 * 360.0;
    let hue_b = ((h >> 16) as f32 / 65535.0 * 360.0 + 40.0) % 360.0;

    let w = THUMB_W as usize;
    let hgt = THUMB_H as usize;
    let mut data = Vec::with_capacity(w * hgt * 4);

    for row in 0..hgt {
        for col in 0..w {
            let t = (col as f32 / w as f32 + row as f32 / hgt as f32) * 0.5;
            let hue = hue_a * (1.0 - t) + hue_b * t;
            let (r, g, b) = hsl_to_rgb(hue, 0.55, 0.28);
            data.push(r);
            data.push(g);
            data.push(b);
            data.push(255);
        }
    }

    ThumbnailRgba {
        width: THUMB_W,
        height: THUMB_H,
        data,
    }
}

/// HSL → RGB conversion (all values in [0, 1] except hue which is [0, 360]).
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hh = h / 60.0;
    let x = c * (1.0 - (hh % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if hh < 1.0 {
        (c, x, 0.0)
    } else if hh < 2.0 {
        (x, c, 0.0)
    } else if hh < 3.0 {
        (0.0, c, x)
    } else if hh < 4.0 {
        (0.0, x, c)
    } else if hh < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c * 0.5;
    let to_u8 = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    (to_u8(r1), to_u8(g1), to_u8(b1))
}

/// Per-frame thumbnail texture cache. Keyed by a stable hash of the
/// thumbnail pixel bytes so rename / reorder never collides.
///
/// Stored in `ControlPanelState` (added in `control_panel.rs`); passed
/// by mutable reference into [`show`] each frame.
pub type ThumbnailCache = HashMap<u64, TextureHandle>;

/// Stable hash for a thumbnail's raw bytes (FNV-1a, 64-bit).
fn thumb_cache_key(data: &[u8]) -> u64 {
    data.iter().fold(14695981039346656037u64, |acc, &b| {
        acc.wrapping_mul(1099511628211) ^ b as u64
    })
}

/// Load (or reuse) an egui texture for a `ThumbnailRgba`.
fn load_thumbnail_texture(
    ctx: &egui::Context,
    cache: &mut ThumbnailCache,
    thumb: &ThumbnailRgba,
    label: &str,
) -> egui::TextureId {
    let key = thumb_cache_key(&thumb.data);
    let handle = cache.entry(key).or_insert_with(|| {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [thumb.width as usize, thumb.height as usize],
            &thumb.data,
        );
        ctx.load_texture(label, image, egui::TextureOptions::LINEAR)
    });
    handle.id()
}

/// Render the cue strip.
///
/// Returns `Some(Command)` when the operator clicks a tile.
///
/// # Parameters
/// - `project` — current project (read-only; provides the scene list).
/// - `cache` — per-session texture handle map; updated in place.
/// - `crossfade_progress` — `Some((target_idx, 0.0..=1.0))` while a
///   crossfade is in progress.
/// - `pending_cue` — V31.7.3: index of a cue armed-and-waiting for the
///   next quantize boundary (`None` when quantize is off or no cue is
///   pending). The tile renders with an accent border (same visual as the
///   crossfade target) so the operator can see the pending fire.
#[cfg_attr(not(feature = "v3"), allow(unused_variables))]
pub fn show(
    ui: &mut Ui,
    project: &Project,
    cache: &mut ThumbnailCache,
    crossfade_progress: Option<(usize, f32)>,
    #[cfg(feature = "v3")] pending_cue: Option<usize>,
) -> Option<Command> {
    let mut out: Option<Command> = None;

    // --- Empty-state copy (T4.5) -----------------------------------------
    if project.cues.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new("(no cues yet)")
                    .color(theme::TEXT_SECONDARY)
                    .font(FontId::proportional(13.0)),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("Save your first cue →")
                    .color(theme::TEXT_SECONDARY.linear_multiply(0.78))
                    .font(FontId::proportional(12.0))
                    .italics(),
            );
            ui.add_space(12.0);
            if plus_tile(ui).clicked() {
                out = Some(Command::SceneSave);
            }
        });
        return out;
    }

    // --- Scene tiles (T4.2–T4.4) -----------------------------------------
    egui::ScrollArea::horizontal()
        .id_salt("rmap_cue_strip_scroll")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(8.0);

                for (idx, scene) in project.cues.iter().enumerate() {
                    let tile_resp = scene_tile(
                        ui,
                        idx,
                        scene,
                        cache,
                        crossfade_progress,
                        // V31.7.3: armed-pending-quantize visual reuses the
                        // accent-border state. For V31.7.3 the pending tile
                        // is visually indistinguishable from a crossfade
                        // target; a future task can add a distinct pulse or
                        // badge if operators request it.
                        #[cfg(feature = "v3")]
                        pending_cue,
                    );
                    if tile_resp.clicked() {
                        out = Some(Command::SceneRecall(idx));
                    }
                }

                ui.add_space(8.0);
                if plus_tile(ui).clicked() {
                    out = Some(Command::SceneSave);
                }
            });
        });

    out
}

/// Render one scene tile. Returns the `Response` for the clickable area.
///
/// `pending_cue` (V31.7.3): index of the pending-quantize cue, if any.
/// The pending tile is rendered with an accent border matching the
/// crossfade-target visual so the operator can see the armed state.
#[cfg_attr(not(feature = "v3"), allow(unused_variables))]
fn scene_tile(
    ui: &mut Ui,
    idx: usize,
    scene: &crate::project::schema::Cue,
    cache: &mut ThumbnailCache,
    crossfade_progress: Option<(usize, f32)>,
    #[cfg(feature = "v3")] pending_cue: Option<usize>,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(TILE_W, TILE_H), Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Background.
        let bg = theme::BG_PANEL.linear_multiply(1.5);
        // V31.7.3: armed-pending-quantize uses the same accent border as a
        // crossfade target. This gives the operator immediate visual feedback
        // that the cue will fire at the next bar boundary. A future task may
        // add a distinct pulsing badge to differentiate the two states.
        let highlight = crossfade_progress.is_some_and(|(t, _)| t == idx);
        #[cfg(feature = "v3")]
        let highlight = highlight || pending_cue == Some(idx);
        let border_col = if highlight {
            theme::ACCENT
        } else if resp.hovered() {
            theme::TEXT_SECONDARY
        } else {
            theme::BG_PANEL.linear_multiply(3.0)
        };
        let rounding = egui::CornerRadius::same(4);
        painter.rect_filled(rect, rounding, bg);
        painter.rect_stroke(
            rect,
            rounding,
            egui::Stroke::new(1.0, border_col),
            egui::StrokeKind::Inside,
        );

        // Thumbnail image.
        if let Some(thumb) = &scene.thumbnail {
            let label = format!("rmap_cue_thumb_{idx}");
            let tex_id = load_thumbnail_texture(ui.ctx(), cache, thumb, &label);
            let img_rect = Rect::from_min_size(rect.min, vec2(TILE_W, TILE_H - 18.0));
            painter.image(
                tex_id,
                img_rect,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            // No thumbnail yet — fill with a muted gradient-like placeholder.
            let mid = rect.min + vec2(TILE_W * 0.5, (TILE_H - 18.0) * 0.5);
            painter.text(
                mid,
                egui::Align2::CENTER_CENTER,
                "—",
                FontId::proportional(18.0),
                theme::TEXT_SECONDARY.linear_multiply(0.57),
            );
        }

        // Index label ("1", "2", …) in the bottom-left corner.
        let label_pos = rect.min + vec2(6.0, TILE_H - 16.0);
        painter.text(
            label_pos,
            egui::Align2::LEFT_TOP,
            (idx + 1).to_string(),
            FontId::proportional(11.0),
            theme::TEXT_PRIMARY,
        );

        // Scene name, right of the index.
        let name_pos = rect.min + vec2(20.0, TILE_H - 16.0);
        painter.text(
            name_pos,
            egui::Align2::LEFT_TOP,
            &scene.name,
            FontId::proportional(11.0),
            theme::TEXT_SECONDARY,
        );

        // T4.4 — crossfade progress bar along the bottom edge.
        if let Some((target_idx, progress)) = crossfade_progress {
            if target_idx == idx && progress > 0.0 {
                let bar_h = 4.0;
                let bar_y = rect.max.y - bar_h;
                let bar_w = TILE_W * progress.clamp(0.0, 1.0);
                let bar_rect =
                    Rect::from_min_size(egui::pos2(rect.min.x, bar_y), vec2(bar_w, bar_h));
                painter.rect_filled(bar_rect, 0.0, theme::ACCENT);
            }
        }
    }

    resp
}

/// Render the "+" add-cue tile. Returns its `Response`.
fn plus_tile(ui: &mut Ui) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(PLUS_TILE_W, TILE_H), Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let bg = theme::BG_BACKGROUND.linear_multiply(1.3);
        let border_col = if resp.hovered() {
            theme::SUCCESS
        } else {
            theme::BG_PANEL.linear_multiply(2.5)
        };
        let rounding = egui::CornerRadius::same(4);
        painter.rect_filled(rect, rounding, bg);
        painter.rect_stroke(
            rect,
            rounding,
            egui::Stroke::new(1.5, border_col),
            egui::StrokeKind::Inside,
        );

        let center = rect.center();
        let plus_col = if resp.hovered() {
            theme::SUCCESS
        } else {
            theme::TEXT_SECONDARY
        };
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            "+",
            FontId::proportional(28.0),
            plus_col,
        );

        // Small hint text.
        let hint_pos = center + vec2(0.0, 22.0);
        painter.text(
            hint_pos,
            egui::Align2::CENTER_CENTER,
            "Save cue",
            FontId::proportional(10.0),
            theme::TEXT_SECONDARY.linear_multiply(0.78),
        );
    }

    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_thumbnail_dimensions_and_alpha() {
        let t = placeholder_thumbnail_for_name("intro");
        assert_eq!(t.width, THUMB_W);
        assert_eq!(t.height, THUMB_H);
        assert_eq!(t.data.len() as u32, THUMB_W * THUMB_H * 4);
        // All alpha channels should be 255.
        for (i, &byte) in t.data.iter().enumerate() {
            if i % 4 == 3 {
                assert_eq!(byte, 255, "alpha at pixel {} should be 255", i / 4);
            }
        }
    }

    #[test]
    fn different_names_produce_different_thumbnails() {
        let a = placeholder_thumbnail_for_name("intro");
        let b = placeholder_thumbnail_for_name("outro");
        // Different names → different pixel content (probabilistically certain).
        assert_ne!(a.data, b.data);
    }

    #[test]
    fn same_name_produces_stable_thumbnail() {
        let a = placeholder_thumbnail_for_name("event");
        let b = placeholder_thumbnail_for_name("event");
        assert_eq!(a.data, b.data);
    }

    #[test]
    fn thumb_cache_key_stable() {
        let data = vec![1u8, 2, 3, 4];
        assert_eq!(thumb_cache_key(&data), thumb_cache_key(&data));
    }

    #[test]
    fn thumb_cache_key_differs_on_different_data() {
        let a = thumb_cache_key(&[1u8, 2, 3]);
        let b = thumb_cache_key(&[1u8, 2, 4]);
        assert_ne!(a, b);
    }
}
