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
