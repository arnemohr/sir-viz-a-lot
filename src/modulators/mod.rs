//! Modulator system. Any numeric effect parameter can be `Static` or one of
//! the time-driven variants; all variants read from the central `Clock`.

pub mod waveforms;

use serde::{Deserialize, Serialize};

use crate::clock::Clock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Modulator {
    Static(f32),
    Sine {
        period_s: f32,
        amp: f32,
        phase: f32,
        offset: f32,
    },
    Triangle {
        period_s: f32,
        amp: f32,
        offset: f32,
    },
    Noise {
        period_s: f32,
        amp: f32,
        offset: f32,
    },
    Bpm {
        divisor: f32,
        amp: f32,
        offset: f32,
    },
    // Reserved for v1.5:
    // Audio { band: u8, smoothing: f32, amp: f32, offset: f32 },
}

impl Modulator {
    pub fn value(&self, clock: &Clock) -> f32 {
        let t = clock.elapsed().as_secs_f32();
        match self {
            Self::Static(v) => *v,
            Self::Sine {
                period_s,
                amp,
                phase,
                offset,
            } => waveforms::sine(t, *period_s, *amp, *phase, *offset),
            Self::Triangle {
                period_s,
                amp,
                offset,
            } => waveforms::triangle(t, *period_s, *amp, *offset),
            Self::Noise {
                period_s,
                amp,
                offset,
            } => waveforms::noise(t, *period_s, *amp, *offset),
            Self::Bpm {
                divisor,
                amp,
                offset,
            } => {
                let beat_period_s = 60.0 / clock.bpm().max(1e-3) * divisor.max(1e-3);
                waveforms::sine(t, beat_period_s, *amp, 0.0, *offset)
            }
        }
    }
}

impl Default for Modulator {
    fn default() -> Self {
        Self::Static(0.0)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::clock::Clock;

    #[test]
    fn dispatch_static() {
        let clock = Clock::for_test(Duration::from_millis(0), 120.0);
        let m = Modulator::Static(0.5);
        let v = m.value(&clock);
        assert!((v - 0.5).abs() < 1e-6, "expected 0.5, got {v}");
    }

    #[test]
    fn dispatch_sine_quarter_period() {
        // Sine with period 1s evaluated at t=0.25s -> peak (+amp).
        let clock = Clock::for_test(Duration::from_millis(250), 120.0);
        let m = Modulator::Sine {
            period_s: 1.0,
            amp: 1.0,
            phase: 0.0,
            offset: 0.0,
        };
        let v = m.value(&clock);
        // 1e-3 tolerance: the Clock::for_test -> m.value drift across
        // a few function calls is sub-microsecond, well within 1e-3.
        assert!((v - 1.0).abs() < 1e-3, "expected ~1.0, got {v}");
    }

    #[test]
    fn dispatch_bpm_at_120() {
        // Bpm modulator with divisor=1 at 120 BPM:
        //   beat_period_s = 60 / 120 * 1 = 0.5 s
        // The implementation routes Bpm through `waveforms::sine`
        // with period = beat_period_s. At t = 0.125s (quarter of the
        // 0.5s beat period) the Bpm sine peaks at 1.0.
        let clock = Clock::for_test(Duration::from_millis(125), 120.0);
        let m = Modulator::Bpm {
            divisor: 1.0,
            amp: 1.0,
            offset: 0.0,
        };
        let v = m.value(&clock);
        assert!((v - 1.0).abs() < 1e-3, "expected ~1.0, got {v}");
    }
}
