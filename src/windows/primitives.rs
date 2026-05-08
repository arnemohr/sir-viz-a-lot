//! Reusable egui drawing primitives shared by the launcher and the
//! editor windows.
//!
//! 003-T2.11 introduces the first primitive: [`drop_target`], a dashed
//! border + centred label rendered inside the canvas drop zone. Used
//! by 003-T2.12 (drag-an-image-onto-the-canvas affordance) and 003-T2.16
//! (canvas empty-state hint).
//!
//! Keeping these in a single small module instead of inlining at each
//! call site lets the visual treatment evolve in one place — the
//! pulse curve, the dash spacing, the colour palette — without a
//! cross-tab grep to keep them in sync.

use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui};

/// 003-T2.11 — paint a dashed-border drop zone with a centred label.
///
/// The border is always painted; the brightness pulses subtly when the
/// canvas is empty and intensifies when the operating system reports a
/// file being dragged over the rmap window (egui's
/// `RawInput::hovered_files`). The intensified state is what the spec
/// calls "pulses on `is_anything_being_dragged`" — for OS-level file
/// drag-and-drop, that signal lives on `RawInput::hovered_files`, not on
/// `Context::dragging_something`, which only covers egui-side widget
/// drags.
///
/// `rect` is the area to paint; the function does *not* allocate or
/// reserve egui layout for it — the caller decides where on the canvas
/// to place the drop zone (`show_scene_tab` puts it inside the existing
/// preview rect; T-003-T2.16 uses the same call). The returned
/// [`Response`] covers `rect` with a `hover` sense so callers can ask
/// "did the operator just hover the drop zone?" without re-allocating.
///
/// Repaint is requested every frame the function runs so the pulse
/// keeps animating; egui throttles this against vsync, so the cost is
/// bounded.
#[allow(dead_code)] // Wired by T-003-T2.12 (drop visual) + T-003-T2.16 (empty-state).
pub fn drop_target(ui: &mut Ui, rect: Rect, label: &str) -> Response {
    let response = ui.allocate_rect(rect, Sense::hover());

    let ctx = ui.ctx();
    let hovering_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
    let pulse = pulse_phase(ctx.input(|i| i.time));
    let intensity = if hovering_files {
        // Hovering — full-strength pulse, palette skewed brighter.
        0.85 + 0.15 * pulse
    } else {
        // Idle — gentle pulse so the affordance is visible without
        // grabbing focus from layer thumbnails or selection chrome.
        0.45 + 0.20 * pulse
    };

    let painter = ui.painter_at(rect);
    let alpha = (255.0 * intensity).clamp(0.0, 255.0) as u8;
    let stroke_color = Color32::from_white_alpha(alpha);
    let stroke_width = 1.5 + 1.5 * intensity;

    paint_dashed_rect(&painter, rect, stroke_width, stroke_color);

    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(16.0),
        stroke_color,
    );

    ctx.request_repaint();
    response
}

/// Pulse curve mapped to `[0.0, 1.0]`. A 1.5 s period reads as
/// "deliberate, calm" rather than "alert" — the empty-state hint
/// should beckon, not flash. Pulled out so it stays consistent with
/// future primitives that share the same visual rhythm (e.g. the
/// "Recommended" badge on the demo button).
fn pulse_phase(t: f64) -> f32 {
    let period = 1.5_f64;
    let phase = (t / period).fract();
    let radians = phase * std::f64::consts::TAU;
    (0.5 + 0.5 * radians.sin()) as f32
}

fn paint_dashed_rect(painter: &egui::Painter, rect: Rect, width: f32, color: Color32) {
    let edges = [
        (rect.left_top(), rect.right_top()),
        (rect.right_top(), rect.right_bottom()),
        (rect.right_bottom(), rect.left_bottom()),
        (rect.left_bottom(), rect.left_top()),
    ];
    for (start, end) in edges {
        paint_dashed_line(painter, start, end, width, color);
    }
}

const DASH_LEN: f32 = 8.0;
const GAP_LEN: f32 = 6.0;

fn paint_dashed_line(painter: &egui::Painter, start: Pos2, end: Pos2, width: f32, color: Color32) {
    let total = (end - start).length();
    if total <= 0.0 {
        return;
    }
    let dir = (end - start) / total;
    for (a, b) in dash_segments(total, DASH_LEN, GAP_LEN) {
        let p1 = start + dir * a;
        let p2 = start + dir * b;
        painter.line_segment([p1, p2], Stroke::new(width, color));
    }
}

/// Pure helper splitting `[0, total)` into `(start, end)` pairs of dashes
/// separated by gaps. Pulled out so the math is unit-testable without an
/// egui context.
fn dash_segments(total: f32, dash_len: f32, gap_len: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    if total <= 0.0 || dash_len <= 0.0 {
        return out;
    }
    let stride = dash_len + gap_len.max(0.0);
    let mut offset = 0.0_f32;
    while offset < total {
        let end = (offset + dash_len).min(total);
        out.push((offset, end));
        offset += stride;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dash_segments_covers_total_with_correct_count() {
        let segs = dash_segments(100.0, 8.0, 6.0);
        // Period = 14; 100 / 14 = 7.14… → 8 dashes (last one may be short).
        assert_eq!(segs.len(), 8);
        // First segment starts at 0.
        assert!((segs[0].0 - 0.0).abs() < 1e-4);
        // Each segment's end is at most start + dash_len.
        for (a, b) in &segs {
            assert!(*b - *a <= 8.0 + 1e-4);
        }
        // Last segment must not exceed `total`.
        assert!(segs.last().unwrap().1 <= 100.0 + 1e-4);
    }

    #[test]
    fn dash_segments_empty_for_zero_length() {
        assert!(dash_segments(0.0, 8.0, 6.0).is_empty());
        assert!(dash_segments(50.0, 0.0, 6.0).is_empty());
    }

    #[test]
    fn pulse_phase_stays_in_unit_interval() {
        for &t in &[0.0, 0.4, 1.5, 7.5, 1234.5] {
            let v = pulse_phase(t);
            assert!(
                (0.0..=1.0).contains(&v),
                "pulse_phase({t}) = {v} out of [0, 1]"
            );
        }
    }
}
