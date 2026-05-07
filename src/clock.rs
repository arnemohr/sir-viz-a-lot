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
        let now = Instant::now();
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
