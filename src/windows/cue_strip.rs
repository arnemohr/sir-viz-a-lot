//! 003-T4.2–T4.5 — Horizontal cue strip below the canvas.
//!
//! Renders one tile (~160×100) per saved scene plus a "+" tile at the
//! right end. Clicking a scene tile emits `Command::SceneRecall(idx)`;
//! clicking "+" emits `Command::SceneSave`. While a crossfade is in
//! progress the target tile shows a thin progress bar along its bottom
//! edge (T4.4). When there are no scenes an empty-state message is shown
//! alongside the lone "+" tile (T4.5).
//!
//! # Thumbnail generation
//!
//! PCleanup.7.1 — `snapshot_thumbnail_from_warp_rt` reads the post-warp
//! composited texture (`warp_rt`) at save time, bilinear-downsamples to
//! 192×108, and returns the bytes as a `ThumbnailRgba`. The cue tile
//! displays the actual scene contents.
//!
//! [`placeholder_thumbnail_for_name`] survives as the fallback when
//! `warp_rt` is unavailable (e.g. a save fired before the first render
//! frame, or a test environment without a GPU adapter).

use std::collections::HashMap;

use egui::{Color32, FontId, Rect, Response, RichText, Sense, TextureHandle, Ui, vec2};

use crate::windows::theme;

use crate::controls::Command;
use crate::project::schema::{Project, ThumbnailRgba};

/// P6.4.1 — State discriminant for a cue tile, computed from `TransportState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileState {
    /// No special state — cue is not live or armed.
    Idle,
    /// Cue is next in line to fire (amber accent ring).
    ArmedNext,
    /// Cue is currently live on the projector (LIVE badge + bottom bar).
    Live,
}

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

/// PCleanup.7.1 — read back the current `warp_rt` texture and produce a
/// 192×108 RGBA8 thumbnail for the cue tile. Returns the placeholder
/// (name-hash gradient) when the readback fails for any reason — a
/// safe-show-day fallback that never blocks the scene-save path.
///
/// The readback pattern is the standard wgpu shape:
///   1. Allocate a CPU-visible buffer sized for `bytes_per_row × height`
///      where `bytes_per_row` is the projector width × 4 rounded up to
///      `COPY_BYTES_PER_ROW_ALIGNMENT` (256).
///   2. `encoder.copy_texture_to_buffer`; submit; queue.poll(Wait).
///   3. Map the buffer; copy rows (skipping the alignment padding) into
///      a tightly-packed Vec<u8>.
///   4. CPU bilinear downsample to 192×108.
///
/// Synchronous via `device.poll(Wait)` rather than async because
/// `Command::SceneSave` is operator-triggered and one-shot — no need to
/// keep the event loop spinning while we wait. Typical readback for a
/// 1920×1080 RGBA8 frame is ~5 ms on M-series wall-clock; the operator
/// notices nothing.
pub fn snapshot_thumbnail_from_warp_rt(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    warp_rt: &wgpu::Texture,
    scene_name_for_fallback: &str,
) -> ThumbnailRgba {
    let size = warp_rt.size();
    let (src_w, src_h) = (size.width, size.height);
    if src_w == 0 || src_h == 0 {
        return placeholder_thumbnail_for_name(scene_name_for_fallback);
    }

    // wgpu requires the buffer copy row stride to be a multiple of
    // COPY_BYTES_PER_ROW_ALIGNMENT (256). RGBA8 = 4 bytes per pixel.
    const ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let unpadded_bpr = src_w * 4;
    let padding = (ALIGN - unpadded_bpr % ALIGN) % ALIGN;
    let padded_bpr = unpadded_bpr + padding;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("warp_rt thumbnail readback"),
        size: (padded_bpr as u64) * (src_h as u64),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("warp_rt readback encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: warp_rt,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bpr),
                rows_per_image: Some(src_h),
            },
        },
        wgpu::Extent3d {
            width: src_w,
            height: src_h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    // Synchronous map: pattern matches the wgpu examples for one-shot
    // texture readback. The closure is invoked when the GPU completes
    // the copy; device.poll(Wait) drives it to completion in-thread.
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    // PollType::Wait { submission_index, timeout }: both Option fields
    // set to None means "wait for the latest submission, no max
    // timeout." That matches the synchronous-readback intent.
    if let Err(err) = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    }) {
        tracing::warn!(
            ?err,
            "warp_rt readback poll failed; falling back to placeholder"
        );
        return placeholder_thumbnail_for_name(scene_name_for_fallback);
    }
    let map_result = match rx.recv_timeout(std::time::Duration::from_millis(500)) {
        Ok(r) => r,
        Err(err) => {
            tracing::warn!(
                ?err,
                "warp_rt readback map timed out; falling back to placeholder"
            );
            return placeholder_thumbnail_for_name(scene_name_for_fallback);
        }
    };
    if let Err(err) = map_result {
        tracing::warn!(
            ?err,
            "warp_rt readback map failed; falling back to placeholder"
        );
        return placeholder_thumbnail_for_name(scene_name_for_fallback);
    }

    let mapped = slice.get_mapped_range();
    // Strip row padding into a tightly-packed RGBA8 src image.
    let mut src_rgba = Vec::with_capacity((src_w * src_h * 4) as usize);
    for row in 0..src_h {
        let start = (row * padded_bpr) as usize;
        let end = start + unpadded_bpr as usize;
        src_rgba.extend_from_slice(&mapped[start..end]);
    }
    drop(mapped);
    buffer.unmap();

    // PCleanup.7.1 — bilinear downsample to thumbnail dimensions.
    let thumb_rgba = bilinear_downsample_rgba8(&src_rgba, src_w, src_h, THUMB_W, THUMB_H);

    // PCleanup.7.1 — the warp_rt format depends on the surface format. On
    // macOS this is typically Bgra8UnormSrgb, which means the bytes
    // arrive in BGRA order. ThumbnailRgba expects RGBA, so swap R↔B.
    // The check is conservative: when format is already RGBA, the swap
    // would be wrong; we infer from texture format.
    let mut data = thumb_rgba;
    if matches!(
        warp_rt.format(),
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    ) {
        for px in data.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
    }

    ThumbnailRgba {
        width: THUMB_W,
        height: THUMB_H,
        data,
    }
}

/// PCleanup.7.1 — bilinear downsample of a tightly-packed RGBA8 image.
/// Cheap CPU resize from projector resolution (e.g. 1920×1080) to
/// thumbnail resolution (192×108). For thumbnails-only use, quality is
/// more than sufficient; a GPU mip-style downsample would be marginal
/// improvement at meaningful complexity.
fn bilinear_downsample_rgba8(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Vec<u8> {
    let mut out = Vec::with_capacity((dst_w * dst_h * 4) as usize);
    let sx_scale = src_w as f32 / dst_w as f32;
    let sy_scale = src_h as f32 / dst_h as f32;
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            // Sample at the centre of the destination texel for a
            // crisp bilinear filter.
            let sx = (dx as f32 + 0.5) * sx_scale - 0.5;
            let sy = (dy as f32 + 0.5) * sy_scale - 0.5;
            let sx0 = sx.floor().clamp(0.0, (src_w - 1) as f32) as u32;
            let sy0 = sy.floor().clamp(0.0, (src_h - 1) as f32) as u32;
            let sx1 = (sx0 + 1).min(src_w - 1);
            let sy1 = (sy0 + 1).min(src_h - 1);
            let fx = (sx - sx0 as f32).clamp(0.0, 1.0);
            let fy = (sy - sy0 as f32).clamp(0.0, 1.0);
            let idx = |x: u32, y: u32| ((y * src_w + x) * 4) as usize;
            for ch in 0..4 {
                let v00 = src[idx(sx0, sy0) + ch] as f32;
                let v10 = src[idx(sx1, sy0) + ch] as f32;
                let v01 = src[idx(sx0, sy1) + ch] as f32;
                let v11 = src[idx(sx1, sy1) + ch] as f32;
                let row0 = v00 + (v10 - v00) * fx;
                let row1 = v01 + (v11 - v01) * fx;
                let val = row0 + (row1 - row0) * fy;
                out.push(val.round().clamp(0.0, 255.0) as u8);
            }
        }
    }
    out
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
    // P6.4.1 — optional transport state for 3-state tile rendering.
    // When `None`, falls back to the pre-P6.4 crossfade-only visual.
    #[cfg(feature = "v3")] transport: Option<&crate::transport::TransportState>,
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
                    // P6.4.1 — compute TileState from TransportState.
                    #[cfg(feature = "v3")]
                    let tile_state = {
                        if let Some(ts) = transport {
                            if ts.current_cue == Some(idx) {
                                TileState::Live
                            } else if ts.armed_cue == Some(idx) {
                                TileState::ArmedNext
                            } else {
                                TileState::Idle
                            }
                        } else {
                            TileState::Idle
                        }
                    };
                    let tile_resp = scene_tile(
                        ui,
                        idx,
                        scene,
                        cache,
                        crossfade_progress,
                        #[cfg(feature = "v3")]
                        pending_cue,
                        #[cfg(feature = "v3")]
                        tile_state,
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
/// `tile_state` (P6.4.1): discriminant from `TransportState`.
#[cfg_attr(not(feature = "v3"), allow(unused_variables))]
fn scene_tile(
    ui: &mut Ui,
    idx: usize,
    scene: &crate::project::schema::Cue,
    cache: &mut ThumbnailCache,
    crossfade_progress: Option<(usize, f32)>,
    #[cfg(feature = "v3")] pending_cue: Option<usize>,
    #[cfg(feature = "v3")] tile_state: TileState,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(TILE_W, TILE_H), Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Background.
        let bg = theme::BG_PANEL.linear_multiply(1.5);
        // P6.4.1: compute border colour from TileState.
        // ArmedNext → amber (distinct from crossfade ACCENT ring).
        // Live → solid ACCENT.
        // Also preserve: crossfade target → ACCENT, pending-quantize → ACCENT.
        let highlight = crossfade_progress.is_some_and(|(t, _)| t == idx);
        #[cfg(feature = "v3")]
        let highlight = highlight || pending_cue == Some(idx);
        #[cfg(feature = "v3")]
        let amber = egui::Color32::from_rgb(0xd4, 0x9a, 0x00); // distinct from ACCENT
        #[cfg(feature = "v3")]
        let border_col = match tile_state {
            TileState::Live => theme::ACCENT,
            TileState::ArmedNext => amber,
            TileState::Idle if highlight => theme::ACCENT,
            TileState::Idle if resp.hovered() => theme::TEXT_SECONDARY,
            TileState::Idle => theme::BG_PANEL.linear_multiply(3.0),
        };
        #[cfg(not(feature = "v3"))]
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

        // P6.4.1 — Live state: solid accent bottom bar + "LIVE" badge.
        #[cfg(feature = "v3")]
        if tile_state == TileState::Live {
            // Solid 3-px bottom bar in ACCENT colour.
            let bar_rect =
                Rect::from_min_size(egui::pos2(rect.min.x, rect.max.y - 3.0), vec2(TILE_W, 3.0));
            painter.rect_filled(bar_rect, 0.0, theme::ACCENT);
            // "LIVE" badge in the top-left corner, semi-opaque accent background.
            let badge_pos = rect.min + vec2(4.0, 4.0);
            let badge_rect = Rect::from_min_size(badge_pos, vec2(30.0, 12.0));
            painter.rect_filled(badge_rect, 2.0, theme::ACCENT.linear_multiply(0.85));
            painter.text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                "LIVE",
                FontId::proportional(8.0),
                egui::Color32::WHITE,
            );
        }

        // P6.4.1 — ArmedNext: amber pulsing ring (painted over the border).
        #[cfg(feature = "v3")]
        if tile_state == TileState::ArmedNext {
            // A second, slightly inset stroke for the amber armed ring.
            let armed_rounding = egui::CornerRadius::same(4);
            painter.rect_stroke(
                rect.shrink(2.0),
                armed_rounding,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(0xd4, 0x9a, 0x00)),
                egui::StrokeKind::Inside,
            );
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

    // ----- PCleanup.7.1 — bilinear_downsample_rgba8 -----------------------

    /// PCleanup.7.1 — downsample produces a correctly-sized buffer and
    /// preserves the (uniform) colour of a flat source.
    #[test]
    fn bilinear_downsample_preserves_flat_colour() {
        let src_w = 32u32;
        let src_h = 32u32;
        // Uniform fill: every pixel is (128, 64, 192, 255).
        let src: Vec<u8> = (0..(src_w * src_h))
            .flat_map(|_| [128u8, 64, 192, 255].into_iter())
            .collect();
        let out = bilinear_downsample_rgba8(&src, src_w, src_h, 8, 4);
        assert_eq!(out.len(), 8 * 4 * 4);
        // Every output pixel should also be (128, 64, 192, 255).
        for px in out.chunks_exact(4) {
            assert_eq!(px[0], 128);
            assert_eq!(px[1], 64);
            assert_eq!(px[2], 192);
            assert_eq!(px[3], 255);
        }
    }

    /// PCleanup.7.1 — downsample to identical dimensions is a near-
    /// passthrough (subject to bilinear sample-centre rounding). Spot-
    /// check center pixels match closely.
    #[test]
    fn bilinear_downsample_identity_size() {
        let src = vec![
            10u8, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160,
        ];
        // 2×2 RGBA image (src_w=2, src_h=2).
        let out = bilinear_downsample_rgba8(&src, 2, 2, 2, 2);
        assert_eq!(out.len(), 2 * 2 * 4);
        // Output should be approximately equal to source. With the
        // sample-centre offset, identity sizing should give back the
        // input bit-for-bit (the centres align exactly).
        assert_eq!(out, src);
    }

    /// PCleanup.7.1 — extreme downsample (32→1 in each axis) produces a
    /// single-pixel result that's roughly the source average. Validates
    /// the bilinear weighting doesn't accidentally pull pixels from
    /// outside the image.
    #[test]
    fn bilinear_downsample_to_single_pixel() {
        let src_w = 32u32;
        let src_h = 32u32;
        // Gradient: each pixel's red channel = its x coordinate × 8 (mod 256).
        let mut src = Vec::with_capacity((src_w * src_h * 4) as usize);
        for y in 0..src_h {
            for x in 0..src_w {
                src.push(((x * 8) % 256) as u8);
                src.push(((y * 8) % 256) as u8);
                src.push(0);
                src.push(255);
            }
        }
        let out = bilinear_downsample_rgba8(&src, src_w, src_h, 1, 1);
        assert_eq!(out.len(), 4);
        // The single-pixel result samples the source centre (16, 16).
        // Red ≈ 16*8 = 128 (±filter); Green ≈ 16*8 = 128 (±filter).
        // Wide tolerance because bilinear at extreme downsample samples
        // a single bilinear cell, not an area average.
        let dr = (out[0] as i32 - 128).abs();
        let dg = (out[1] as i32 - 128).abs();
        assert!(
            dr < 32,
            "expected R ≈ 128 at single-pixel down; got {}",
            out[0]
        );
        assert!(
            dg < 32,
            "expected G ≈ 128 at single-pixel down; got {}",
            out[1]
        );
        assert_eq!(out[2], 0);
        assert_eq!(out[3], 255);
    }
}
