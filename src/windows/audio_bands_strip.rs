//! 004-V31.9.2 — Audio bands strip.
//!
//! Renders an 8-band FFT meter as a row of vertical bars above the show-day
//! strip. Visible only when an audio source is active (i.e.
//! [`crate::modulators::audio::is_audio_active`] returns `true`).
//!
//! Each bar is draggable — dragging emits a `band_idx` so that a future
//! parameter-binding picker (v0.4 / Phase 0) can receive it as a drop target.
//! In v3.1 the drag starts but no target accepts the drop; the drag completes
//! as a no-op and the band index is logged at debug level.
//!
//! Placement: declared between `rmap_show_day_strip` (outermost bottom) and
//! `rmap_cue_strip` in `control_panel.rs`, so it sits above the show-day strip
//! but below the cue strip, per `roadmap.md` §8 ideal layout.

use egui::{CornerRadius, Stroke, StrokeKind, Ui};

use crate::windows::theme;

/// Height the `TopBottomPanel` reserves when the strip is visible.
pub const STRIP_HEIGHT: f32 = 40.0;

/// Compute bar geometry given the available horizontal space and the number of
/// bars to render. Returns `(bar_w, bar_h, gutter)`.
///
/// Extracted as a pure function so it can be unit-tested without egui context.
///
/// # Panics
///
/// Does not panic for zero `bar_count`; returns `(0.0, bar_h, gutter)` in that
/// degenerate case (caller's loop over 0 bands does nothing).
pub fn bar_layout(avail_x: f32, avail_y: f32, bar_count: usize) -> (f32, f32, f32) {
    let gutter = 4.0_f32;
    let bar_h = avail_y.clamp(12.0, STRIP_HEIGHT - 8.0);
    let bar_w = if bar_count == 0 {
        0.0
    } else {
        ((avail_x - gutter * (bar_count as f32 + 1.0)) / bar_count as f32).max(8.0)
    };
    (bar_w, bar_h, gutter)
}

/// Render the 8-band audio meter.
///
/// Returns `Some(band_idx)` when the user initiates a drag on a band; the
/// App side logs this in v3.1 and will route it to a parameter-binding picker
/// in v0.4. Returns `None` for plain hover/click/idle.
///
/// When `audio::is_audio_active()` is `false`, returns `None` immediately and
/// paints nothing — the caller wraps the `TopBottomPanel` call in an
/// `is_audio_active()` guard anyway, so this inner guard is belt-and-braces.
pub fn show(ui: &mut Ui) -> Option<u8> {
    if !crate::modulators::audio::is_audio_active() {
        return None;
    }
    let bands = crate::modulators::audio::current_bands_snapshot();
    let mut drag_event: Option<u8> = None;

    let avail = ui.available_size();
    let (bar_w, bar_h, gutter) = bar_layout(avail.x, avail.y, bands.len());

    ui.add_space(gutter); // top margin
    ui.horizontal(|ui| {
        ui.add_space(gutter);
        // P6.10.1 — Frequency-range labels for the 8 FFT bands.
        // Approximate ranges at 44.1 kHz with 8 log-spaced bins.
        const BAND_LABELS: [&str; 8] =
            ["Sub", "Bass", "LMid", "Mid", "HMid", "Pres", "Bril", "Air"];

        for (idx, &mag) in bands.iter().enumerate() {
            ui.vertical(|ui| {
                let (rect, resp) = ui.allocate_exact_size(
                    egui::vec2(bar_w, bar_h - 12.0),
                    egui::Sense::click_and_drag(),
                );

                // Bar background (full height, slightly lighter than panel).
                let bg = theme::BG_PANEL.linear_multiply(1.8);
                ui.painter().rect_filled(rect, CornerRadius::same(2), bg);

                // Filled foreground grows from the bottom up.
                let clamped = mag.clamp(0.0, 1.0);
                let fill_h = (rect.height() * clamped).round();
                if fill_h > 0.5 {
                    let fill_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.min.x, rect.max.y - fill_h),
                        rect.max,
                    );
                    ui.painter()
                        .rect_filled(fill_rect, CornerRadius::same(2), theme::ACCENT);
                }

                if resp.drag_started() {
                    drag_event = Some(idx as u8);
                }

                // Hover: brighten border, change cursor to communicate drag affordance.
                if resp.hovered() {
                    ui.painter().rect_stroke(
                        rect,
                        CornerRadius::same(2),
                        Stroke::new(1.0, theme::ACCENT),
                        StrokeKind::Inside,
                    );
                }
                resp.on_hover_cursor(egui::CursorIcon::Grab);

                // Frequency label below the bar.
                ui.label(
                    egui::RichText::new(BAND_LABELS[idx])
                        .small()
                        .color(theme::TEXT_SECONDARY),
                );
            });

            // Gutter between bars.
            if idx + 1 < bands.len() {
                ui.add_space(gutter);
            }
        }
    });

    drag_event
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// bar_layout with a generous width: bar_w comes from the formula.
    #[test]
    fn bar_layout_normal() {
        let (bar_w, bar_h, gutter) = bar_layout(400.0, 32.0, 8);
        // 400 - 4*(8+1) = 400 - 36 = 364; 364/8 = 45.5
        assert!((bar_w - 45.5).abs() < 0.5, "bar_w = {bar_w}");
        assert_eq!(gutter, 4.0);
        // bar_h = min(32, STRIP_HEIGHT-8).max(12) = min(32,32).max(12) = 32
        assert!((bar_h - 32.0).abs() < 0.1, "bar_h = {bar_h}");
    }

    /// Very narrow avail: bar_w clamps to the 8.0 floor.
    #[test]
    fn bar_layout_clamps_to_minimum() {
        let (bar_w, _bar_h, _gutter) = bar_layout(10.0, 30.0, 8);
        assert!(bar_w >= 8.0, "bar_w should be at least 8.0, got {bar_w}");
    }

    /// Zero bars: should not divide by zero.
    #[test]
    fn bar_layout_zero_bars() {
        let (bar_w, _bar_h, _gutter) = bar_layout(400.0, 30.0, 0);
        assert_eq!(bar_w, 0.0);
    }

    /// show() returns None when no audio provider is installed.
    /// We can't easily call egui without a Context, but we can test the
    /// `is_audio_active()` guard separately (the PROVIDER static starts as
    /// unset in fresh test processes; audio-feature tests that install a
    /// provider use a different test binary with `--features audio`).
    #[test]
    fn is_audio_inactive_at_test_start() {
        // No provider installed in test binary without `--features audio`.
        // The function returns false; show() would return None.
        assert!(
            !crate::modulators::audio::is_audio_active(),
            "PROVIDER should be unset in a bare test binary"
        );
    }
}
