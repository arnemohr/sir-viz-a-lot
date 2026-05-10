//! OSC value registry (P0.2.1, W2.1) — mirrors `audio.rs`.
//!
//! `Modulator::OscBound { addr, .. }` resolves through this module's
//! process-wide [`PROVIDER`]. The OSC UDP listener (`controls::osc`,
//! gated on `feature = "osc"`) writes incoming f32 / int args into the
//! registry keyed by their address; any address bound on a parameter
//! reads the latest value here.
//!
//! When no provider is installed (no `osc` feature, no listener
//! started, no message yet seen for that address), [`current_value`]
//! returns `0.0` — same fallback shape as `audio::current_band`.

use std::sync::{Arc, OnceLock};

/// One named source of OSC address values. The trait is the extension
/// point: `controls::osc::OscRegistry` is the in-tree default; tests
/// can install a stub that returns canned values without binding a
/// real UDP socket.
pub trait OscProvider: Send + Sync {
    /// Latest value seen for `addr`. Returns `0.0` for never-seen
    /// addresses (matches `Modulator::OscBound`'s no-provider fallback).
    fn value(&self, addr: &str) -> f32;
}

/// Process-wide OSC provider, set once at app startup. `Modulator::OscBound`
/// reads through here so the resolve dispatch stays parameter-free.
static PROVIDER: OnceLock<Arc<dyn OscProvider>> = OnceLock::new();

/// Install the active OSC provider. Subsequent calls are silently
/// ignored — once the dispatch sees a provider it should not change
/// for the lifetime of the app.
///
/// W2.1 follow-up wires `controls::osc::OscSource::start` to install
/// a `CcRegistry`-equivalent provider here; until then the resolve
/// path always returns `0.0`.
#[allow(dead_code)] // wired by W2.1 OSC value-registry follow-up
pub fn install(provider: Arc<dyn OscProvider>) {
    let _ = PROVIDER.set(provider);
}

/// Current value at `addr` from the installed provider, or `0.0` if no
/// provider was installed (e.g. `osc` feature off, listener init
/// failed, address never received a message).
pub fn current_value(addr: &str) -> f32 {
    PROVIDER.get().map(|p| p.value(addr)).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::RwLock;

    /// Stub provider for tests — does not touch the process-global
    /// `PROVIDER`.
    struct StubProvider {
        values: HashMap<String, f32>,
    }

    impl OscProvider for StubProvider {
        fn value(&self, addr: &str) -> f32 {
            self.values.get(addr).copied().unwrap_or(0.0)
        }
    }

    /// `current_value` returns 0.0 when no provider is installed.
    ///
    /// Safe to run unconditionally: `osc::install` is only called at
    /// app startup (`src/app.rs`) and never in tests, so `PROVIDER` is
    /// guaranteed unset in the test process.
    #[test]
    fn current_value_is_zero_without_provider() {
        assert_eq!(current_value("/rmap/blur/radius"), 0.0);
    }

    /// Stub provider returns canned values via the trait directly
    /// (no `install` call — that path is single-set and would race
    /// other tests).
    #[test]
    fn stub_provider_returns_known_value() {
        let mut values = HashMap::new();
        values.insert("/rmap/blur/radius".to_string(), 0.42);
        let provider = StubProvider { values };
        assert!((provider.value("/rmap/blur/radius") - 0.42).abs() < 1e-6);
        assert_eq!(provider.value("/never/seen"), 0.0);
    }

    /// A live registry backed by `Arc<RwLock<HashMap<String, f32>>>`
    /// (the shape `controls::osc` will use) round-trips reads + writes.
    #[test]
    fn live_registry_round_trip() {
        struct LiveRegistry {
            values: Arc<RwLock<HashMap<String, f32>>>,
        }
        impl OscProvider for LiveRegistry {
            fn value(&self, addr: &str) -> f32 {
                self.values
                    .read()
                    .map(|g| g.get(addr).copied().unwrap_or(0.0))
                    .unwrap_or(0.0)
            }
        }
        let values = Arc::new(RwLock::new(HashMap::new()));
        let provider = LiveRegistry {
            values: values.clone(),
        };

        // Write side simulates the UDP listener.
        values
            .write()
            .unwrap()
            .insert("/rmap/blur/radius".to_string(), 0.7);

        assert!((provider.value("/rmap/blur/radius") - 0.7).abs() < 1e-6);
    }
}
