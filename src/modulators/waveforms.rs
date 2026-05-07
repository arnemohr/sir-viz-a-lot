//! Time-driven waveform implementations. Pure functions of (clock, params)
//! so each is trivially unit-testable without a GPU.

use std::f32::consts::TAU;

pub fn sine(t_s: f32, period_s: f32, amp: f32, phase: f32, offset: f32) -> f32 {
    let phase_rad = (t_s / period_s.max(1e-6)) * TAU + phase;
    offset + amp * phase_rad.sin()
}

pub fn triangle(t_s: f32, period_s: f32, amp: f32, offset: f32) -> f32 {
    let p = period_s.max(1e-6);
    let phase = t_s.rem_euclid(p) / p; // 0..1
    let tri = 1.0 - (phase * 2.0 - 1.0).abs() * 2.0; // -1..1
    offset + amp * tri
}

/// Smooth value-noise: deterministic interpolation between hashed random
/// samples taken once per period. `period_s` controls texture coarseness.
pub fn noise(t_s: f32, period_s: f32, amp: f32, offset: f32) -> f32 {
    let p = period_s.max(1e-6);
    let phase = t_s / p;
    let i = phase.floor();
    let f = phase - i;
    let a = hash01(i);
    let b = hash01(i + 1.0);
    let smooth = f * f * (3.0 - 2.0 * f); // smoothstep
    let v = a + (b - a) * smooth; // [0, 1]
    offset + amp * (v * 2.0 - 1.0) // [-amp, amp]
}

fn hash01(x: f32) -> f32 {
    let h = (x * 12.9898).sin() * 43_758.547;
    h - h.floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_zero_at_origin() {
        assert!(sine(0.0, 1.0, 1.0, 0.0, 0.0).abs() < 1e-6);
    }

    #[test]
    fn sine_peak_at_quarter_period() {
        let v = sine(0.25, 1.0, 1.0, 0.0, 0.0);
        assert!((v - 1.0).abs() < 1e-6, "expected 1.0, got {v}");
    }

    #[test]
    fn triangle_extrema() {
        assert!((triangle(0.0, 1.0, 1.0, 0.0) - -1.0).abs() < 1e-6);
        assert!((triangle(0.5, 1.0, 1.0, 0.0) - 1.0).abs() < 1e-6);
    }
}
