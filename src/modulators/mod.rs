//! Modulator system. Any numeric effect parameter can be `Static` or one of
//! the time-driven variants; all variants read from the central `Clock`.

pub mod audio;
pub mod midi;
pub mod osc;
pub mod waveforms;

use serde::{Deserialize, Serialize};

use crate::clock::Clock;

/// Serialize a finite `f32` to JSON, returning an error for `NaN` / `±∞`.
///
/// `serde_json` maps non-finite `f32` to JSON `null` silently.  For project
/// fields that must survive a save → load round-trip this is unacceptable:
/// `null` fails to deserialize back as `f32`, corrupting the project.  This
/// helper rejects non-finite values at serialization time with a clear error
/// so the bug surfaces at the *save* call site rather than on the next load.
mod finite_f32_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &f32, s: S) -> Result<S::Ok, S::Error> {
        if v.is_finite() {
            s.serialize_f32(*v)
        } else {
            Err(serde::ser::Error::custom(format!(
                "Modulator::Static value must be finite, got {v:?}"
            )))
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
        let v = f32::deserialize(d)?;
        if v.is_finite() {
            Ok(v)
        } else {
            Err(serde::de::Error::custom(format!(
                "Modulator::Static value must be finite, got {v:?}"
            )))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Modulator {
    /// A constant value.  Serialized with [`finite_f32_serde`] so that
    /// `NaN` / `±∞` are rejected at save time rather than silently written
    /// as JSON `null`, which would corrupt the project on the next load
    /// (V31.1.1 fix).
    Static(#[serde(with = "finite_f32_serde")] f32),
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
    /// Reads from the live `AudioProvider` installed at startup
    /// (T-M7-03; gated on `feature = "audio"` for the actual cpal
    /// capture, but the variant exists unconditionally so projects
    /// remain readable across build flavors). Returns `0.0` when no
    /// provider is installed.
    Audio {
        band: u8,
        /// Reserved: per-modulator smoothing currently lives inside the
        /// provider's one-pole low-pass; this knob is a hook for an
        /// in-Modulator additional smoothing pass when needed.
        smoothing: f32,
        amp: f32,
        offset: f32,
    },
    /// P0.2.1 (W2.1) — reads the latest value seen for `addr` from the
    /// process-wide OSC value registry. Resolved as
    /// `osc::current_value(addr) * scale + offset`. Variant exists
    /// unconditionally; gated UDP listener (`controls::osc`,
    /// `feature = "osc"`) populates the registry. Returns `0.0` when
    /// no provider is installed or the address has never been seen.
    OscBound {
        addr: String,
        scale: f32,
        offset: f32,
    },
    /// P0.2.2 (W2.2) — reads the latest CC value for `(channel, cc)`
    /// from the process-wide MIDI value registry. Resolved as
    /// `midi::current_value(channel, cc) * scale + offset`. Variant
    /// exists unconditionally; gated MIDI decoder (`controls::midi`,
    /// `feature = "midi"`) populates the registry. Returns `0.0` when
    /// no provider is installed or the CC has never been received.
    MidiBound {
        cc: u8,
        channel: u8,
        scale: f32,
        offset: f32,
    },
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
            Self::Audio {
                band,
                smoothing: _,
                amp,
                offset,
            } => {
                let v = audio::current_band(*band);
                v * amp + offset
            }
            Self::OscBound {
                addr,
                scale,
                offset,
            } => osc::current_value(addr) * scale + offset,
            Self::MidiBound {
                cc,
                channel,
                scale,
                offset,
            } => midi::current_value(*channel, *cc) * scale + offset,
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

    // ── V31.1.1 — static-modulator round-trip proptest ──────────────────────
    //
    // Bug: before the fix, `serde_json` silently serialized `NaN` / `±∞` as
    // JSON `null`, which then failed to deserialize back as `f32`, leaving
    // the project in a corrupt state.  The fix (`finite_f32_serde`) makes the
    // serializer *return an error* for non-finite values.
    //
    // Three checks live here:
    // 1. `proptest` over the full finite f32 range: every finite value
    //    must survive a `to_string` → `from_str` round-trip bit-exactly.
    // 2. Deterministic smoke tests for the subnormals and corner cases called
    //    out by the spec.
    // 3. Non-finite values must produce a *serialization error* (not `null`).

    /// V31.1.1 — every finite f32 round-trips through JSON string bit-exactly.
    ///
    /// The proptest uses `any::<f32>()` and skips non-finite values via
    /// `prop_assume!`.  The remaining values must survive
    /// `serde_json::to_string` → `serde_json::from_str` with bit-exact
    /// identity.  Subnormals are included (they ARE finite and must round-trip).
    mod proptest_round_trip {
        use proptest::prelude::*;

        use super::Modulator;

        proptest! {
            #![proptest_config(proptest::test_runner::Config::with_cases(4096))]

            #[test]
            fn static_modulator_round_trips(v in any::<f32>()) {
                // NaN and ±∞ cannot be represented in JSON — they are covered by
                // the deterministic `non_finite_serialize_errors` test below.
                prop_assume!(v.is_finite());

                let m = Modulator::Static(v);
                let json = serde_json::to_string(&m).unwrap();
                let back: Modulator = serde_json::from_str(&json).unwrap();
                match back {
                    Modulator::Static(b) => prop_assert!(
                        v.to_bits() == b.to_bits(),
                        "static modulator round-trip lost bit-identity: \
                         a={v:?} bits={:#010x} → json='{json}' → b={b:?} bits={:#010x}",
                        v.to_bits(),
                        b.to_bits(),
                    ),
                    other => prop_assert!(
                        false,
                        "round-trip changed variant: {other:?}"
                    ),
                }
            }
        }
    }

    /// V31.1.1 — non-finite values produce a serialization error, not `null`.
    ///
    /// Before the fix, `serde_json` wrote `null` for `NaN` / `±∞`, which
    /// would then fail to deserialize and corrupt the project silently.
    /// After the fix, the serializer returns an error immediately.
    #[test]
    fn non_finite_serialize_errors() {
        for (label, v) in [
            ("NaN", f32::NAN),
            ("+Inf", f32::INFINITY),
            ("-Inf", f32::NEG_INFINITY),
        ] {
            let result = serde_json::to_string(&Modulator::Static(v));
            assert!(
                result.is_err(),
                "serializing Static({label}) must return an error, got: {result:?}"
            );
        }
    }

    /// V31.1.1 — subnormals round-trip exactly (they ARE finite).
    #[test]
    fn subnormal_round_trips() {
        let subnormals = [
            f32::from_bits(0x00000001), // smallest positive subnormal
            f32::from_bits(0x007fffff), // largest positive subnormal
            f32::from_bits(0x00400000), // mid-range subnormal
            f32::MIN_POSITIVE / 2.0,    // one subnormal step below MIN_POSITIVE
        ];
        for v in subnormals {
            assert!(
                !v.is_normal() && v.is_finite(),
                "fixture should be subnormal: {v:?}"
            );
            let m = Modulator::Static(v);
            let json = serde_json::to_string(&m).expect("subnormal should serialize without error");
            let back: Modulator =
                serde_json::from_str(&json).expect("subnormal should deserialize");
            match back {
                Modulator::Static(b) => assert!(
                    v.to_bits() == b.to_bits(),
                    "subnormal round-trip lost bit-identity: {v:?} bits={:#010x} → back={b:?} bits={:#010x}",
                    v.to_bits(),
                    b.to_bits(),
                ),
                other => panic!("round-trip changed variant: {other:?}"),
            }
        }
    }
}
