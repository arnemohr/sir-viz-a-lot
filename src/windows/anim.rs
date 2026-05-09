//! Animation timing tokens and egui animation helpers for rmap's windows.
//!
//! All durations are in milliseconds and are intentionally small — rmap
//! is a live-performance tool and the operator's eye must track the
//! canvas, not wait for animations to settle. The tokens establish a
//! shared vocabulary so every interactive surface fades and slides at
//! the same pace.

// ── Timing tokens ─────────────────────────────────────────────────────────────

/// Duration (ms) for a hover-state fade-in / fade-out.
///
/// Short so hover feedback is snappy; long enough to not feel like a
/// hard cut when the pointer drifts into a control.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
pub const HOVER_FADE_MS: f32 = 120.0;

/// Duration (ms) for drag-ease animations (e.g. thumbnail scale during drag).
///
/// Slightly longer than `HOVER_FADE_MS` so drag affordances feel
/// "heavier" than pointer hovers — reflects physical inertia.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
pub const DRAG_EASE_MS: f32 = 160.0;

/// Duration (ms) for panel / banner transitions (slide-in, cross-fade).
///
/// Longest token; gives structural layout changes a bit more air so the
/// operator's eye can follow where attention should go next.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
pub const TRANSITION_MS: f32 = 220.0;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Animate a `bool` target from its current 0..=1 value toward `target`
/// using egui's internal animation manager, with `duration_ms` controlling
/// how quickly the value moves (half-life is approximately `duration_ms/2`
/// due to egui's exponential blend).
///
/// Returns the current eased `f32` value in `0.0..=1.0`.
///
/// This is allocation-free: egui stores the animated state keyed by `id`
/// in its per-frame memory without heap allocation.
#[cfg_attr(not(feature = "v3"), allow(dead_code))]
pub fn animate_bool_to(ui: &mut egui::Ui, id: egui::Id, target: bool, duration_ms: f32) -> f32 {
    ui.ctx()
        .animate_bool_with_time(id, target, duration_ms / 1000.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// T4.15 acceptance: timing tokens are all positive and ordered
    /// HOVER_FADE_MS < DRAG_EASE_MS < TRANSITION_MS.
    #[test]
    fn anim_constants_present_and_sensible() {
        assert!(HOVER_FADE_MS > 0.0, "HOVER_FADE_MS must be positive");
        assert!(DRAG_EASE_MS > 0.0, "DRAG_EASE_MS must be positive");
        assert!(TRANSITION_MS > 0.0, "TRANSITION_MS must be positive");
        assert!(
            HOVER_FADE_MS < DRAG_EASE_MS,
            "HOVER_FADE_MS ({HOVER_FADE_MS}) must be less than DRAG_EASE_MS ({DRAG_EASE_MS})"
        );
        assert!(
            DRAG_EASE_MS < TRANSITION_MS,
            "DRAG_EASE_MS ({DRAG_EASE_MS}) must be less than TRANSITION_MS ({TRANSITION_MS})"
        );
    }
}
