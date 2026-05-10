//! MIDI value registry (P0.2.2, W2.2) — mirrors `audio.rs` / `osc.rs`.
//!
//! `Modulator::MidiBound { cc, channel, .. }` resolves through this
//! module's process-wide [`PROVIDER`]. The MIDI decoder
//! (`controls::midi`, gated on `feature = "midi"`) extends in W2.2 to
//! write Control Change messages into the registry as
//! `value as f32 / 127.0`.
//!
//! When no provider is installed (no `midi` feature, no listener
//! started, no CC yet seen for that key), [`current_value`] returns
//! `0.0` — same fallback shape as `audio::current_band`.

use std::sync::{Arc, OnceLock};

/// Number of MIDI channels (1..=16, encoded 0..=15 here for the
/// status-byte channel nibble).
pub const NUM_CHANNELS: usize = 16;

/// Number of MIDI Control Change indices per channel (0..=127).
pub const NUM_CCS: usize = 128;

/// One named source of MIDI CC values. The trait is the extension
/// point: `controls::midi::CcRegistry` is the in-tree default; tests
/// install a stub that returns canned values without binding a real
/// MIDI port.
pub trait MidiProvider: Send + Sync {
    /// Latest normalised value (`[0.0, 1.0]`) for `(channel, cc)`.
    /// Returns `0.0` for never-seen pairs (matches
    /// `Modulator::MidiBound`'s no-provider fallback). `channel` is
    /// 0-indexed (status byte's low nibble); `cc` is 0..=127.
    fn cc(&self, channel: u8, cc: u8) -> f32;
}

/// Process-wide MIDI provider, set once at app startup.
/// `Modulator::MidiBound` reads through here so the resolve dispatch
/// stays parameter-free.
static PROVIDER: OnceLock<Arc<dyn MidiProvider>> = OnceLock::new();

/// Install the active MIDI provider. Subsequent calls are silently
/// ignored — once the dispatch sees a provider it should not change
/// for the lifetime of the app.
#[cfg_attr(not(feature = "midi"), allow(dead_code))]
pub fn install(provider: Arc<dyn MidiProvider>) {
    let _ = PROVIDER.set(provider);
}

/// Current value for `(channel, cc)` from the installed provider, or
/// `0.0` if no provider was installed (e.g. `midi` feature off,
/// no controller connected, CC never received).
pub fn current_value(channel: u8, cc: u8) -> f32 {
    PROVIDER.get().map(|p| p.cc(channel, cc)).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stub provider for tests — does not touch the process-global
    /// `PROVIDER`.
    struct StubProvider {
        values: [[f32; NUM_CCS]; NUM_CHANNELS],
    }

    impl MidiProvider for StubProvider {
        fn cc(&self, channel: u8, cc: u8) -> f32 {
            let ch = (channel as usize).min(NUM_CHANNELS - 1);
            let i = (cc as usize).min(NUM_CCS - 1);
            self.values[ch][i]
        }
    }

    /// `current_value` returns 0.0 when no provider is installed.
    ///
    /// Safe to run unconditionally: `midi::install` is only called at
    /// app startup (`src/app.rs`) and never in tests.
    #[test]
    fn current_value_is_zero_without_provider() {
        assert_eq!(current_value(0, 21), 0.0);
        assert_eq!(current_value(15, 127), 0.0);
    }

    /// Stub provider returns canned values via the trait directly.
    #[test]
    fn stub_provider_returns_known_value() {
        let mut values = [[0.0; NUM_CCS]; NUM_CHANNELS];
        values[0][21] = 0.42;
        values[15][127] = 1.0;
        let provider = StubProvider { values };
        assert!((provider.cc(0, 21) - 0.42).abs() < 1e-6);
        assert!((provider.cc(15, 127) - 1.0).abs() < 1e-6);
        assert_eq!(provider.cc(3, 0), 0.0);
    }

    /// Out-of-range channel / CC indices clamp rather than panic —
    /// defensive shape for the decoder and the resolve path.
    #[test]
    fn out_of_range_channel_or_cc_clamps() {
        let values = [[0.0; NUM_CCS]; NUM_CHANNELS];
        let provider = StubProvider { values };
        // u8 max channels at 255; clamps into the table.
        assert_eq!(provider.cc(255, 255), 0.0);
    }
}
