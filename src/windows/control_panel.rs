//! egui control panel UI. Pure UI: operates on a mutable `Vec<Effect>`
//! reference. The closure passed to `ControlWindow::render` owns the
//! App-side borrow-splitting dance; this module never touches the GPU.
//!
//! T-M4-15: drives the load-bearing M4 user-visible payoff. The operator
//! drags a slider here and the projector visibly changes within one frame
//! because each `Effect`'s modulator-typed fields are evaluated against
//! the central `Clock` on every frame.
//!
//! Modulator UI: Static and Sine variants are exposed via a small
//! `ComboBox` selector (defers the full right-click "→ Modulator" context
//! menu to a later milestone — Triangle/Noise/Bpm get only static-mode UI
//! for v1).

use egui::Ui;

use crate::effects::Effect;
use crate::modulators::Modulator;

/// Render the entire control panel into `ui`. `effects` is mutated in
/// place when the operator drags any slider.
pub fn show(ui: &mut Ui, effects: &mut [Effect]) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.heading("Effect chain");
        ui.add_space(4.0);
        for (idx, effect) in effects.iter_mut().enumerate() {
            egui::CollapsingHeader::new(effect_label(effect))
                .id_salt(idx)
                .default_open(true)
                .show(ui, |ui| {
                    show_effect(ui, idx, effect);
                });
        }
    });
}

fn effect_label(e: &Effect) -> &'static str {
    match e {
        Effect::Color { .. } => "Color",
        Effect::Tint { .. } => "Tint",
        Effect::Blur { .. } => "Blur",
        Effect::Transform { .. } => "Transform",
    }
}

fn show_effect(ui: &mut Ui, idx: usize, effect: &mut Effect) {
    match effect {
        Effect::Color {
            hue,
            saturation,
            brightness,
            contrast,
        } => {
            modulator_slider(ui, (idx, "hue"), "hue (deg)", hue, -180.0..=180.0);
            modulator_slider(ui, (idx, "sat"), "saturation", saturation, 0.0..=2.0);
            modulator_slider(ui, (idx, "bri"), "brightness", brightness, -1.0..=1.0);
            modulator_slider(ui, (idx, "con"), "contrast", contrast, 0.0..=2.0);
        }
        Effect::Tint { .. } => {
            ui.label("(Tint not yet implemented; see Effect::Tint stub)");
        }
        Effect::Blur { radius_px } => {
            modulator_slider(ui, (idx, "blur"), "radius (px)", radius_px, 0.0..=32.0);
        }
        Effect::Transform {
            translate,
            rotate_deg,
            scale_x,
            scale_y,
        } => {
            ui.add(egui::Slider::new(&mut translate[0], -1.0..=1.0).text("tx"));
            ui.add(egui::Slider::new(&mut translate[1], -1.0..=1.0).text("ty"));
            modulator_slider(ui, (idx, "rot"), "rotate (deg)", rotate_deg, -180.0..=180.0);
            modulator_slider(ui, (idx, "scx"), "scale x", scale_x, 0.1..=3.0);
            modulator_slider(ui, (idx, "scy"), "scale y", scale_y, 0.1..=3.0);
        }
    }
}

/// Edit a `Modulator` for a single numeric field.
///
/// For `Static`, exposes a single slider over its scalar value.
///
/// For `Sine`, exposes period/amp/phase/offset sliders. The
/// variant-selector `ComboBox` lets the operator switch between Static
/// and Sine (Triangle/Noise/Bpm are not exposed in v1 — picking them in
/// state shows a "no UI in v1" placeholder; the menu omits them so the
/// operator can't pick them). Satisfies the spec's "Set a sine modulator
/// → projector animates" without the full right-click context menu.
fn modulator_slider(
    ui: &mut Ui,
    salt: (usize, &'static str),
    label: &str,
    m: &mut Modulator,
    range: std::ops::RangeInclusive<f32>,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let cur_label = match m {
            Modulator::Static(_) => "static",
            Modulator::Sine { .. } => "sine",
            Modulator::Triangle { .. } => "tri",
            Modulator::Noise { .. } => "noise",
            Modulator::Bpm { .. } => "bpm",
        };
        // Salt the ComboBox id with both the effect index and the field
        // name so multiple modulators in the same chain don't collide.
        egui::ComboBox::from_id_salt(salt)
            .selected_text(cur_label)
            .show_ui(ui, |ui| {
                let is_static = matches!(m, Modulator::Static(_));
                let is_sine = matches!(m, Modulator::Sine { .. });
                if ui.selectable_label(is_static, "static").clicked() && !is_static {
                    *m = Modulator::Static(*range.start());
                }
                if ui.selectable_label(is_sine, "sine").clicked() && !is_sine {
                    let span = range.end() - range.start();
                    *m = Modulator::Sine {
                        period_s: 1.0,
                        amp: span * 0.5,
                        phase: 0.0,
                        offset: (range.start() + range.end()) * 0.5,
                    };
                }
                // Triangle / Noise / Bpm intentionally not offered in v1.
            });
    });
    match m {
        Modulator::Static(v) => {
            ui.add(egui::Slider::new(v, range).text("value"));
        }
        Modulator::Sine {
            period_s,
            amp,
            phase,
            offset,
        } => {
            let span = range.end() - range.start();
            ui.add(egui::Slider::new(period_s, 0.05..=10.0).text("period (s)"));
            ui.add(egui::Slider::new(amp, 0.0..=span).text("amp"));
            ui.add(egui::Slider::new(phase, 0.0..=std::f32::consts::TAU).text("phase"));
            ui.add(egui::Slider::new(offset, range.clone()).text("offset"));
        }
        Modulator::Triangle { .. } | Modulator::Noise { .. } | Modulator::Bpm { .. } => {
            ui.label("(this modulator variant has no UI in v1)");
        }
    }
    ui.add_space(2.0);
}
