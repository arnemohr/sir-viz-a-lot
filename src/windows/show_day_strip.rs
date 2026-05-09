//! 003-T3.23–T3.25 — Show-day strip: four persistent buttons (Blackout /
//! Freeze / Test / Outlines) at the bottom of the canvas, visible in both
//! `Editing` and `GoLive`. Each button mirrors the keyboard hotkey and
//! shows a small badge with the accelerator letter.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Ui};

use crate::windows::theme;

use crate::controls::Command;

/// Snapshot of output state passed into the strip so the UI can read active
/// flags without borrowing `EditingState` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputStateSnapshot {
    pub blackout: bool,
    pub freeze: bool,
    /// `true` when any test pattern other than `None` is active.
    pub test_pattern_active: bool,
    pub overlay_on: bool,
}

/// Returns the highlighted label colour when `active`, muted otherwise.
pub fn button_color(active: bool) -> Color32 {
    if active {
        theme::ACCENT
    } else {
        theme::TEXT_PRIMARY
    }
}

/// Build a `LayoutJob` for a button: label in `label_color`, badge in muted
/// small text. Using `LayoutJob` allows two distinct text styles in one
/// `Button` without nesting widgets.
fn button_label_job(label: &str, badge: &str, label_color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(
        label,
        0.0,
        TextFormat {
            font_id: FontId::proportional(14.0),
            color: label_color,
            ..Default::default()
        },
    );
    job.append(
        &format!(" ({badge})"),
        0.0,
        TextFormat {
            font_id: FontId::proportional(10.0),
            color: theme::TEXT_SECONDARY,
            valign: egui::Align::Max,
            ..Default::default()
        },
    );
    job
}

/// Render the four show-day buttons and return the `Command` to dispatch, if
/// any button was clicked.
pub fn show(ui: &mut Ui, snap: &OutputStateSnapshot) -> Option<Command> {
    let mut out: Option<Command> = None;

    ui.horizontal(|ui| {
        let entries: &[(&str, &str, bool, Command)] = &[
            ("Blackout", "B", snap.blackout, Command::Blackout),
            ("Freeze", "F", snap.freeze, Command::Freeze),
            (
                "Test",
                "T",
                snap.test_pattern_active,
                Command::CycleTestPattern,
            ),
            (
                "Outlines",
                "O",
                snap.overlay_on,
                Command::ToggleEditorOverlay,
            ),
        ];

        for (label, badge, active, cmd) in entries {
            let label_color = button_color(*active);
            let job = button_label_job(label, badge, label_color);

            let resp = ui
                .add(egui::Button::new(job).min_size(egui::vec2(90.0, 36.0)))
                .on_hover_text(format!("Keyboard: {badge}"));

            if resp.clicked() {
                out = Some(cmd.clone());
            }
        }
    });

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_color_active_is_accent() {
        let active = button_color(true);
        let inactive = button_color(false);
        // Active must be the theme accent, inactive the primary text colour.
        assert_eq!(active, theme::ACCENT);
        assert_eq!(inactive, theme::TEXT_PRIMARY);
        assert_ne!(active, inactive);
    }

    #[test]
    fn output_state_snapshot_default_all_off() {
        let snap = OutputStateSnapshot {
            blackout: false,
            freeze: false,
            test_pattern_active: false,
            overlay_on: false,
        };
        assert!(!snap.blackout);
        assert!(!snap.freeze);
        assert!(!snap.test_pattern_active);
        assert!(!snap.overlay_on);
    }

    #[test]
    fn output_state_snapshot_active_bits() {
        let snap = OutputStateSnapshot {
            blackout: true,
            freeze: false,
            test_pattern_active: true,
            overlay_on: false,
        };
        assert!(snap.blackout);
        assert!(!snap.freeze);
        assert!(snap.test_pattern_active);
        assert!(!snap.overlay_on);
    }
}
