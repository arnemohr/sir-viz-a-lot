//! 003-T3.4: top-of-canvas toolbar rendered under `--features v3`.
//!
//! Left side: project-name placeholder + Undo / Redo buttons (disabled when
//! stacks are empty). Right side: Warp toggle (flips `EditMode` between Layer
//! and Warp), Controls window toggle (binds to `st.controls_open`), and a
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

        // --- BPM HUD badge (V31.7.2 + P1.UX) --- live BPM + tap source +
        // quantize selector. P1.UX additions:
        //   • Hover tooltips on BPM and Quantize labels so the operator
        //     can self-discover what these do without reading the spec.
        //   • Tap button as a discoverable fallback to the Space-bar
        //     shortcut (operators won't always guess the shortcut).
        //   • Tap-flash: the BPM number pulses accent for ~250 ms after
        //     a tap registers, so a single Space-press is visibly
        //     received even before the second tap (which is what
        //     actually changes the inferred BPM).
        #[cfg(feature = "v3")]
        {
            use std::time::Duration;
            ui.add_space(12.0);

            let bpm_tooltip = "Beats-per-minute clock used by:\n\
                 • Modulator::Bpm (sine-wave parameter automation\n   \
                   tied to the beat)\n\
                 • Video layers with BPM-lock on (playback rate\n   \
                   scales with this value; 120 = identity)\n\
                 • Scene-recall quantization (see Quantize)\n\n\
                 Tap tempo: press Space twice in time with the beat,\n\
                 or click the Tap button. The first tap records the\n\
                 time; the second tap derives the BPM from the\n\
                 interval. Subsequent taps smooth the running estimate.";

            // Per-tap visual pulse on the BPM number — fades over
            // ~250 ms after each tap so a single Space-press gives
            // immediate confirmation that the tap was received.
            let fresh_tap = inputs
                .bpm_telemetry
                .last_tap_at
                .map(|t| t.elapsed() < Duration::from_millis(250))
                .unwrap_or(false);
            let bpm_text = format!("BPM: {:.1}", inputs.bpm_telemetry.current_bpm);
            let bpm_label: egui::WidgetText = if fresh_tap {
                egui::RichText::new(bpm_text)
                    .color(crate::windows::theme::ACCENT)
                    .strong()
                    .into()
            } else {
                bpm_text.into()
            };
            ui.label(bpm_label).on_hover_text(bpm_tooltip);
            if fresh_tap {
                // Keep repainting through the flash window so the
                // colour decays smoothly back to normal.
                ui.ctx().request_repaint_after(Duration::from_millis(50));
            }

            if let (Some(src), Some(at)) = (
                inputs.bpm_telemetry.last_tap_source,
                inputs.bpm_telemetry.last_tap_at,
            ) {
                let age_s = at.elapsed().as_secs_f32();
                ui.weak(format!("({}, {:.1}s)", src.label(), age_s));
            }

            // Explicit Tap button so the Space-bar shortcut isn't a
            // hidden affordance. Clicking goes through the same
            // `apply_command` path the keyboard tap uses, so the
            // smoothed-BPM logic is identical.
            if ui
                .button("Tap")
                .on_hover_text("Tap twice in time with the beat (or press Space).")
                .clicked()
            {
                action = Some(ControlPanelAction::EmitCommand(
                    crate::controls::Command::TapTempo(crate::clock::TapSource::Keyboard),
                ));
            }

            ui.add_space(8.0);

            let quantize_tooltip = "Scene-recall quantization. With Quantize Off (default),\n\
                 pressing a scene key (1-9) recalls the scene immediately.\n\
                 With Quantize set to N bars, the recall is **armed** and\n\
                 fires at the next N-bar boundary in the BPM clock — so\n\
                 scene changes land on the downbeat instead of mid-bar.\n\n\
                 Off: instant. 1: next bar. 2/4/8: next 2/4/8 bars.";
            ui.label("Quantize:").on_hover_text(quantize_tooltip);
            for opt in [None, Some(1u8), Some(2u8), Some(4u8), Some(8u8)] {
                let label = match opt {
                    None => "Off",
                    Some(1) => "1",
                    Some(2) => "2",
                    Some(4) => "4",
                    Some(8) => "8",
                    _ => unreachable!(),
                };
                let opt_tooltip = match opt {
                    None => "Off — scene cues fire instantly on keypress.",
                    Some(n) => &format!("Arm scene cues to fire at the next {n}-bar boundary."),
                };
                let is_active = project.quantize_bars == opt;
                let button = egui::Button::new(label).fill(if is_active {
                    crate::windows::theme::ACCENT.linear_multiply(0.25)
                } else {
                    egui::Color32::TRANSPARENT
                });
                let resp = ui.add(button).on_hover_text(opt_tooltip);
                if resp.clicked() && !is_active {
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

            // P4.4.1 — "New scene from template" button opens the scene wizard.
            // Only available while in Editing (not GoLive) — wizarding from GoLive
            // is undefined. The button is absent in GoLive mode so the operator
            // isn't confused by a disabled button that doesn't explain itself.
            #[cfg(feature = "v3")]
            if !inputs.is_go_live {
                if ui.button("New scene…").clicked() {
                    action = Some(ControlPanelAction::RequestEnterSceneWizard);
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

            // P1.UX — renamed from "Advanced". The window now floats
            // over the canvas (glossary-style) and contains every
            // per-layer + project-level control, so "Advanced"
            // (which read as "expert-only") no longer fit.
            ui.toggle_value(&mut st.controls_open, "Controls");
            ui.add_space(8.0);

            // P0.7.5 — Output panel toggle. The minimum-viable "Output
            // mode pill": opens `OutputPanel` (P0.8.1) as a peer right-side
            // SidePanel. Advanced's per-output sections are skipped while
            // this is open (mutual exclusion avoids duplicate egui IDs and
            // matches the spec's mode-pill semantic). Always shown — the
            // schema invariant guarantees `output_targets.len() >= 1`.
            #[cfg(feature = "v3")]
            {
                ui.toggle_value(&mut st.output_panel_open, "Output");
                ui.add_space(8.0);
            }

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
