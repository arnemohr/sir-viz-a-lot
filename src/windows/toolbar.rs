//! 003-T3.4: top-of-canvas toolbar rendered under `--features v3`.
//!
//! Left side: project-name placeholder + Undo / Redo buttons (disabled when
//! stacks are empty). Right side: Warp toggle (flips `EditMode` between Layer
//! and Warp), Advanced disclosure toggle (binds to `st.advanced_open`), and a
//! Go-live stub (Phase 4 wires the fullscreen transition).
//!
//! Returns `Some(ControlPanelAction)` when the operator triggers Undo or Redo;
//! returns `None` for all other interactions (mode / disclosure toggles have no
//! action to propagate — they write directly into the mutable state they were
//! handed).

use crate::project::schema::Project;
use crate::windows::control_panel::{ControlPanelAction, ControlPanelInputs, ControlPanelState};
use crate::windows::scene_editor::{EditMode, SceneEditorState};

/// 004-V31.8.2 — Compute `(width, height)` for the header thumbnail.
///
/// `output_size` is the projector framebuffer dimensions from
/// `ControlPanelInputs::output_size`; `(0, 0)` falls back to 16:9. The
/// returned width is `thumb_height × aspect` so both the `ImageButton`
/// and the placeholder rectangle allocate exactly the same space,
/// preventing jitter when the texture cycles through `None` on resize.
pub(super) fn thumbnail_size(output_size: (u32, u32), thumb_height: f32) -> (f32, f32) {
    let aspect = if output_size.0 > 0 && output_size.1 > 0 {
        output_size.0 as f32 / output_size.1 as f32
    } else {
        16.0 / 9.0
    };
    (thumb_height * aspect, thumb_height)
}

/// Flip `EditMode` between `Layer` and `Warp`.
///
/// Any mode other than `Warp` (e.g. `Mask`, `Inspect`) is treated as "not
/// warp" — clicking the Warp button takes the operator to `Warp`. Clicking
/// it again returns to `Layer`. The previous non-Warp mode is not remembered;
/// T3.4's button is a binary Layer↔Warp toggle only.
pub fn flip_warp(mode: EditMode) -> EditMode {
    match mode {
        EditMode::Warp => EditMode::Layer,
        _ => EditMode::Warp,
    }
}

/// Render the top-of-canvas toolbar and return any undo/redo action that was
/// requested. Warp / Advanced / Go-live interactions write directly into `scene`
/// and `st` rather than going through `ControlPanelAction`.
pub fn show(
    ui: &mut egui::Ui,
    project: &Project,
    st: &mut ControlPanelState,
    scene: &mut SceneEditorState,
    inputs: &ControlPanelInputs,
) -> Option<ControlPanelAction> {
    let mut action: Option<ControlPanelAction> = None;
    ui.horizontal(|ui| {
        // --- Left side ---
        // 003-T4.9 / T4.10: project name with dirty indicator.
        let label_text = if inputs.dirty {
            format!("• {}", inputs.project_name)
        } else {
            inputs.project_name.clone()
        };
        ui.label(label_text);
        ui.add_space(12.0);
        if ui
            .add_enabled(inputs.can_undo, egui::Button::new("⟲ Undo"))
            .clicked()
        {
            action = Some(ControlPanelAction::RequestUndo);
        }
        if ui
            .add_enabled(inputs.can_redo, egui::Button::new("⟳ Redo"))
            .clicked()
        {
            action = Some(ControlPanelAction::RequestRedo);
        }
        ui.add_space(8.0);
        // 003-T4.8: Save button — enabled only when dirty (nothing to save
        // otherwise). Save-as is always available.
        if ui
            .add_enabled(inputs.dirty, egui::Button::new("Save"))
            .clicked()
        {
            action = Some(ControlPanelAction::RequestSave);
        }
        if ui.button("Save as\u{2026}").clicked() {
            action = Some(ControlPanelAction::RequestSaveAs);
        }

        // --- BPM HUD badge (V31.7.2) --- live BPM + tap source + quantize selector
        #[cfg(feature = "v3")]
        {
            ui.add_space(12.0);
            ui.label(format!("BPM: {:.1}", inputs.bpm_telemetry.current_bpm));
            if let (Some(src), Some(at)) = (
                inputs.bpm_telemetry.last_tap_source,
                inputs.bpm_telemetry.last_tap_at,
            ) {
                let age_s = at.elapsed().as_secs_f32();
                ui.weak(format!("({}, {:.1}s)", src.label(), age_s));
            }
            ui.add_space(8.0);
            ui.label("Quantize:");
            for opt in [None, Some(1u8), Some(2u8), Some(4u8), Some(8u8)] {
                let label = match opt {
                    None => "Off",
                    Some(1) => "1",
                    Some(2) => "2",
                    Some(4) => "4",
                    Some(8) => "8",
                    _ => unreachable!(),
                };
                let is_active = project.quantize_bars == opt;
                let button = egui::Button::new(label).fill(if is_active {
                    crate::windows::theme::ACCENT.linear_multiply(0.25)
                } else {
                    egui::Color32::TRANSPARENT
                });
                if ui.add(button).clicked() && !is_active {
                    st.pending_mutations
                        .push(project.set_quantize_bars_mutation(opt));
                }
            }
        }

        // --- Right side --- push remaining widgets to the right edge
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // 004-V31.8.2: projector-output thumbnail — rightmost widget.
            // `right_to_left` means this block executes first → renders at
            // the far right. Clicking focuses the preview window (if open)
            // or opens it (if closed). Placeholder rect keeps layout stable
            // while the texture is unregistered (init gap / post-resize).
            #[cfg(feature = "v3")]
            {
                const THUMB_H: f32 = 56.0;
                let (tw, th) = thumbnail_size(inputs.output_size, THUMB_H);
                if let Some(tex) = inputs.scene_texture {
                    let sized = egui::load::SizedTexture::new(tex, egui::vec2(tw, th));
                    let resp = ui
                        .add(egui::Button::image(sized))
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.clicked() {
                        if inputs.has_preview {
                            action = Some(ControlPanelAction::FocusPreview);
                        } else {
                            action = Some(ControlPanelAction::RequestOpenPreview);
                        }
                    }
                } else {
                    let (rect, _resp) =
                        ui.allocate_exact_size(egui::vec2(tw, th), egui::Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius::same(2),
                        crate::windows::theme::BG_PANEL,
                    );
                }
                ui.add_space(4.0);
            }

            // 003-T4.17: Go-live / Stop button. Label flips on is_go_live;
            // the click returns RequestEnterGoLive or RequestExitGoLive so
            // App::window_event can perform the AppState swap.
            #[cfg(feature = "v3")]
            {
                let go_live_label = if inputs.is_go_live { "Stop" } else { "Go live" };
                if ui.button(go_live_label).clicked() {
                    if inputs.is_go_live {
                        action = Some(ControlPanelAction::RequestExitGoLive);
                    } else {
                        action = Some(ControlPanelAction::RequestEnterGoLive);
                    }
                }
            }
            // 003-T4.16a: Preview button. Opens / closes the child preview window.
            #[cfg(feature = "v3")]
            {
                let preview_label = if inputs.has_preview {
                    "Close preview"
                } else {
                    "Preview"
                };
                if ui.button(preview_label).clicked() {
                    if inputs.has_preview {
                        action = Some(ControlPanelAction::RequestClosePreview);
                    } else {
                        action = Some(ControlPanelAction::RequestOpenPreview);
                    }
                }
            }
            // Non-v3 stub preserved so v2 builds continue to compile.
            #[cfg(not(feature = "v3"))]
            {
                let _ = ui.button("Go live");
            }
            ui.add_space(8.0);

            // Advanced disclosure toggle
            ui.toggle_value(&mut st.advanced_open, "Advanced");
            ui.add_space(8.0);

            // Warp toggle: any non-Warp mode ↔ Warp
            let mut is_warp = scene.mode == EditMode::Warp;
            if ui.toggle_value(&mut is_warp, "Warp").clicked() {
                scene.mode = flip_warp(scene.mode);
            }

            // 003-T5.12 — Glossary + Help buttons.
            // Glossary toggles the in-app term window; "?" opens the README
            // in the default browser via `std::process::Command`.
            #[cfg(feature = "v3")]
            {
                ui.add_space(4.0);
                ui.toggle_value(&mut st.glossary_open, "Glossary");
                if ui
                    .button("?")
                    .on_hover_text("Open rmap help in browser")
                    .clicked()
                {
                    crate::windows::control_panel::open_help_url();
                }
            }
        });
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 003-T3.4 — `flip_warp` cycles `Layer ↔ Warp`. Starting from `Layer`
    /// arrives at `Warp`; starting from `Warp` returns to `Layer`.
    #[test]
    fn toolbar_warp_button_toggles_mode() {
        assert_eq!(flip_warp(EditMode::Layer), EditMode::Warp);
        assert_eq!(flip_warp(EditMode::Warp), EditMode::Layer);
    }

    /// Non-Layer modes (Mask, Inspect) are also treated as "not Warp" by the
    /// toolbar toggle, so pressing Warp always enters `EditMode::Warp`.
    #[test]
    fn toolbar_warp_button_from_other_modes() {
        assert_eq!(flip_warp(EditMode::Mask), EditMode::Warp);
        assert_eq!(flip_warp(EditMode::Inspect), EditMode::Warp);
    }

    // ---- 004-V31.8.2: thumbnail_size ----------------------------------------

    /// `(0, 0)` output size falls back to 16:9; height is preserved exactly.
    #[test]
    fn thumbnail_size_zero_falls_back_to_16x9() {
        let (w, h) = thumbnail_size((0, 0), 56.0);
        assert!((h - 56.0).abs() < 1e-4, "height must equal thumb_height");
        let expected_w = 56.0 * (16.0 / 9.0);
        assert!(
            (w - expected_w).abs() < 1e-3,
            "width should be 56×16/9 ≈ {expected_w:.2}, got {w:.2}"
        );
    }

    /// 1920×1080 (16:9) — width ≈ height × (16/9).
    #[test]
    fn thumbnail_size_1920x1080() {
        let (w, h) = thumbnail_size((1920, 1080), 56.0);
        assert!((h - 56.0).abs() < 1e-4);
        let expected_w = 56.0 * (1920.0 / 1080.0);
        assert!(
            (w - expected_w).abs() < 1e-3,
            "expected {expected_w:.2}, got {w:.2}"
        );
    }

    /// 800×600 (4:3) — width ≈ height × (4/3).
    #[test]
    fn thumbnail_size_800x600_4x3() {
        let (w, h) = thumbnail_size((800, 600), 56.0);
        assert!((h - 56.0).abs() < 1e-4);
        let expected_w = 56.0 * (800.0 / 600.0);
        assert!(
            (w - expected_w).abs() < 1e-3,
            "expected {expected_w:.2}, got {w:.2}"
        );
    }
}
