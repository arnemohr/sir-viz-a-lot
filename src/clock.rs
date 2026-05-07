//! Central time/BPM/tap-tempo source. Every modulator reads from the same
//! `Clock`, so blackout/freeze/scene-recall stay phase-coherent and tests can
//! advance time deterministically.

use std::time::{Duration, Instant};

pub struct Clock {
    started: Instant,
    bpm: f32,
    last_tap: Option<Instant>,
}

impl Clock {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            bpm: 120.0,
            last_tap: None,
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
    pub fn tap(&mut self) {
        self.tap_at(Instant::now());
    }

    /// Like [`tap`], but accepts an explicit timestamp. Useful for
    /// deterministic tests and for callers that need to backdate a tap.
    pub fn tap_at(&mut self, now: Instant) {
        if let Some(prev) = self.last_tap.replace(now) {
            let interval = now.duration_since(prev).as_secs_f32().max(1e-3);
            let inferred = 60.0 / interval;
            self.bpm = (self.bpm + inferred) * 0.5;
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
        }
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
        clock.tap_at(t0);
        // Three subsequent taps at 0.5s intervals -> 120 BPM inferred each.
        clock.tap_at(t0 + Duration::from_millis(500));
        clock.tap_at(t0 + Duration::from_millis(1000));
        clock.tap_at(t0 + Duration::from_millis(1500));

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
        clock.tap_at(t0);
        assert!((clock.bpm() - 137.0).abs() < 1e-6);
    }
}
