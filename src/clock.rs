//! Central time/BPM/tap-tempo source. Every modulator reads from the same
//! `Clock`, so blackout/freeze/scene-recall stay phase-coherent and tests can
//! advance time deterministically.

use std::time::{Duration, Instant};

/// Which input source produced the most-recent tap-tempo event.
///
/// Used by [`BpmTelemetry`] so the UI can show "tapped via Space 0.4 s ago"
/// without any allocation on the read path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapSource {
    Keyboard,
    Midi,
    Osc,
}

impl TapSource {
    /// Short user-facing label, suitable for inline display in the BPM HUD.
    pub fn label(&self) -> &'static str {
        match self {
            TapSource::Keyboard => "Space",
            TapSource::Midi => "MIDI",
            TapSource::Osc => "OSC",
        }
    }
}

/// Read-only snapshot of the clock's tap-tempo state. Returned by
/// [`Clock::telemetry`] each frame; allocation-free because all fields
/// are `Copy`.
#[derive(Debug, Clone, Copy)]
pub struct BpmTelemetry {
    pub current_bpm: f32,
    pub last_tap_source: Option<TapSource>,
    pub last_tap_at: Option<Instant>,
}

pub struct Clock {
    started: Instant,
    bpm: f32,
    last_tap: Option<Instant>,
    last_tap_source: Option<TapSource>,
}

impl Clock {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            bpm: 120.0,
            last_tap: None,
            last_tap_source: None,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Record a tap-tempo press. Two consecutive taps are sufficient to
    /// derive a BPM; subsequent taps update the running estimate.
    pub fn tap(&mut self, source: TapSource) {
        self.tap_at(Instant::now(), source);
    }

    /// Like [`tap`], but accepts an explicit timestamp. Useful for
    /// deterministic tests and for callers that need to backdate a tap.
    pub fn tap_at(&mut self, now: Instant, source: TapSource) {
        if let Some(prev) = self.last_tap.replace(now) {
            let interval = now.duration_since(prev).as_secs_f32().max(1e-3);
            let inferred = 60.0 / interval;
            self.bpm = (self.bpm + inferred) * 0.5;
        }
        self.last_tap_source = Some(source);
    }

    /// Return a cheap, allocation-free snapshot of tap-tempo state that the
    /// UI can poll per frame. The struct is `Copy` so reads have zero heap
    /// cost.
    pub fn telemetry(&self) -> BpmTelemetry {
        BpmTelemetry {
            current_bpm: self.bpm,
            last_tap_source: self.last_tap_source,
            last_tap_at: self.last_tap,
        }
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl Clock {
    /// Test-only constructor: produces a Clock that reports `elapsed`
    /// as exactly `elapsed_target` and `bpm` as `bpm`. Used by
    /// modulator dispatch tests (T-M4-11) and tap-tempo tests
    /// (T-M4-12).
    pub fn for_test(elapsed_target: std::time::Duration, bpm: f32) -> Self {
        Self {
            // started = now - elapsed_target so the next elapsed()
            // call returns ~elapsed_target (modulo the microseconds
            // between this line and the test's value() call).
            started: std::time::Instant::now() - elapsed_target,
            bpm,
            last_tap: None,
            last_tap_source: None,
        }
    }

    /// V31.7.3 — advance the clock so `elapsed()` returns approximately
    /// `elapsed`. Updates `started = Instant::now() - elapsed`. Used
    /// by bar-boundary integration tests to step the clock across
    /// multiple bar markers without creating a new `Clock` instance
    /// (which would reset `bpm` and `last_tap`).
    pub fn set_elapsed(&mut self, elapsed: std::time::Duration) {
        self.started = std::time::Instant::now() - elapsed;
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn tap_tempo_converges() {
        // Start at 100 BPM (off-target by 20). With the simple
        // averaging filter `bpm = (bpm + inferred) * 0.5`, 4 taps
        // at 0.5s (= 120 inferred BPM) converge to ~117.5 — well
        // within the spec's [115, 125] band.
        //
        // Trace:
        //   bpm = 100
        //   tap 0: last_tap=None, no update, bpm=100
        //   tap 1: interval=0.5s, inferred=120, bpm=(100+120)/2=110
        //   tap 2: same, bpm=(110+120)/2=115
        //   tap 3: same, bpm=(115+120)/2=117.5
        //
        // Total: 4 taps after starting state, bpm in range.
        let mut clock = Clock::for_test(Duration::ZERO, 100.0);

        let t0 = Instant::now();
        // First tap establishes the baseline (no inferred bpm change).
        clock.tap_at(t0, TapSource::Keyboard);
        // Three subsequent taps at 0.5s intervals -> 120 BPM inferred each.
        clock.tap_at(t0 + Duration::from_millis(500), TapSource::Keyboard);
        clock.tap_at(t0 + Duration::from_millis(1000), TapSource::Keyboard);
        clock.tap_at(t0 + Duration::from_millis(1500), TapSource::Keyboard);

        let bpm = clock.bpm();
        assert!(
            (115.0..=125.0).contains(&bpm),
            "bpm should converge into [115, 125] from initial 100; got {bpm}",
        );
    }

    #[test]
    fn tap_at_first_call_does_not_change_bpm() {
        // First tap (last_tap was None) only seeds last_tap; bpm
        // is unchanged because there's nothing to compute an
        // interval from.
        let mut clock = Clock::for_test(Duration::ZERO, 137.0);
        let t0 = Instant::now();
        clock.tap_at(t0, TapSource::Keyboard);
        assert!((clock.bpm() - 137.0).abs() < 1e-6);
    }

    #[test]
    fn telemetry_reports_zero_taps_initially() {
        let clock = Clock::for_test(Duration::ZERO, 120.0);
        let t = clock.telemetry();
        assert!(
            t.last_tap_source.is_none(),
            "fresh clock should have no tap source"
        );
        assert!(
            t.last_tap_at.is_none(),
            "fresh clock should have no tap timestamp"
        );
    }

    #[test]
    fn telemetry_after_tap_records_source() {
        let mut clock = Clock::for_test(Duration::ZERO, 120.0);
        let t0 = Instant::now();
        clock.tap_at(t0, TapSource::Keyboard);
        let t = clock.telemetry();
        assert_eq!(t.last_tap_source, Some(TapSource::Keyboard));
        assert!(t.last_tap_at.is_some(), "tap_at should set last_tap_at");
    }

    #[test]
    fn telemetry_updates_source_on_each_tap() {
        let mut clock = Clock::for_test(Duration::ZERO, 120.0);
        let t0 = Instant::now();
        clock.tap_at(t0, TapSource::Keyboard);
        clock.tap_at(t0 + Duration::from_millis(500), TapSource::Midi);
        let t = clock.telemetry();
        assert_eq!(
            t.last_tap_source,
            Some(TapSource::Midi),
            "telemetry should reflect the most recent tap source"
        );
    }

    #[test]
    fn telemetry_bpm_matches_clock_bpm() {
        let mut clock = Clock::for_test(Duration::ZERO, 100.0);
        let t0 = Instant::now();
        clock.tap_at(t0, TapSource::Osc);
        clock.tap_at(t0 + Duration::from_millis(500), TapSource::Osc);
        clock.tap_at(t0 + Duration::from_millis(1000), TapSource::Osc);
        assert_eq!(clock.telemetry().current_bpm, clock.bpm());
    }

    /// V31.7.3 — `set_elapsed` should make `elapsed()` return the target
    /// duration within a 1 ms tolerance (wall-clock drift between the two
    /// `Instant::now()` calls inside `set_elapsed` and `elapsed()`).
    #[test]
    fn set_elapsed_round_trips_within_tolerance() {
        let mut clock = Clock::for_test(Duration::ZERO, 120.0);
        let target = Duration::from_secs(8);
        clock.set_elapsed(target);
        let got = clock.elapsed();
        let diff = got.abs_diff(target);
        assert!(
            diff < Duration::from_millis(1),
            "elapsed() should match set_elapsed() target within 1 ms; got diff {diff:?}"
        );
    }

    /// `set_elapsed` must not disturb `bpm` or tap state.
    #[test]
    fn set_elapsed_preserves_bpm_and_tap_state() {
        let t0 = Instant::now();
        let mut clock = Clock::for_test(Duration::ZERO, 137.0);
        clock.tap_at(t0, TapSource::Keyboard);
        clock.set_elapsed(Duration::from_secs(4));
        assert!(
            (clock.bpm() - 137.0).abs() < 1e-3,
            "bpm should be unaffected by set_elapsed"
        );
        assert_eq!(clock.telemetry().last_tap_source, Some(TapSource::Keyboard));
    }
}
