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

    /// P6.12.2 — Directly set the BPM (used by MIDI-clock source to drive
    /// the clock without tap events). The last-tap source is unchanged
    /// so the UI can distinguish MIDI-clock-driven BPM from tap-tempo.
    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = bpm.clamp(20.0, 400.0);
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

        // PCleanup.6.2 — re-anchor bar phase so the tap lands on beat 1 of a
        // new bar. Without this, `self.bpm` updates from taps but `started`
        // doesn't, so the bar index (computed from `elapsed()` in
        // `app::process_pending_cue`) drifts relative to the tap and
        // BPM-quantised cues fire off-beat after a tap.
        //
        // UX decision: tap = beat 1 of the bar (the operator-conductor
        // convention — operators tap "1, 2, 3, 4" mentally tracking
        // downbeats, not "nearest beat" which would distort the bar grid
        // mid-bar). To preserve elapsed monotonicity we snap `started` to
        // the most recent bar boundary at or before the tap, not to `now`
        // outright — that way modulators using `elapsed()` as a phase
        // experience at most one-bar of phase shift, never a full-time
        // reset.
        //
        // Constants here mirror `app::process_pending_cue`'s 4-beats-per-
        // bar assumption; a future variable-time-signature feature would
        // need to parameterise both call sites.
        let bar_period_s = (60.0 / self.bpm.max(1e-3)) * 4.0;
        let elapsed_at_tap = now.duration_since(self.started).as_secs_f32();
        if elapsed_at_tap >= 0.0 && bar_period_s > 0.0 {
            // floor((elapsed_at_tap) / bar_period_s) → most recent integer
            // bar boundary at or before the tap. Snap started back by the
            // partial-bar remainder so the tap coincides with that bar
            // boundary's position in elapsed time.
            let bars_completed = (elapsed_at_tap / bar_period_s).floor();
            let snapped_elapsed = bars_completed * bar_period_s;
            // shift = how far we need to advance started forward to land
            // the tap on snapped_elapsed (a bar boundary).
            let shift_s = elapsed_at_tap - snapped_elapsed;
            if shift_s > 0.0 {
                self.started += std::time::Duration::from_secs_f32(shift_s);
            }
        }
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

    // ----- PCleanup.6.2 — bar-phase re-anchor on tap -------------------

    /// PCleanup.6.2 — after a tap, the elapsed time AT the tap instant is
    /// an integer multiple of the bar period: the tap lands on beat 1 of
    /// a new bar. Operator-conductor convention: tapping "1, 2, 3, 4"
    /// places "1" on a bar boundary, not a random sub-beat.
    #[test]
    fn tap_re_anchors_bar_phase_to_beat_one() {
        // 120 BPM → bar period = 2.0s (4 beats × 0.5s/beat).
        let mut clock = Clock::for_test(Duration::from_millis(2700), 120.0);
        // Inside the bar at elapsed=2.7s: 1 full bar done (2.0s),
        // 0.7s into the second bar (i.e. mid-bar).
        let t = Instant::now();
        clock.tap_at(t, TapSource::Keyboard);
        // After the tap, elapsed at t (= now relative to started) must be
        // an integer multiple of the bar period. The snap moved `started`
        // forward by 0.7s, so the tap is now at elapsed = 2.0s.
        let elapsed_at_tap_s = t.duration_since(clock.started).as_secs_f32();
        let bar_period_s = (60.0 / clock.bpm()) * 4.0;
        let bars_at_tap = elapsed_at_tap_s / bar_period_s;
        let frac = bars_at_tap - bars_at_tap.floor();
        assert!(
            frac < 1e-3 || (1.0 - frac) < 1e-3,
            "expected tap on bar boundary; got frac={frac} \
             (elapsed={elapsed_at_tap_s}s, bar_period={bar_period_s}s)"
        );
    }

    /// PCleanup.6.2 — re-anchor preserves elapsed monotonicity: the
    /// post-tap elapsed at the tap instant must be ≤ pre-tap elapsed
    /// (we snap BACK to the previous bar boundary, never forward past
    /// the tap). Catches an off-by-one that would jump elapsed forward
    /// past `now`.
    #[test]
    fn tap_re_anchor_preserves_elapsed_monotonicity() {
        let mut clock = Clock::for_test(Duration::from_millis(2700), 120.0);
        let t = Instant::now();
        let pre_elapsed = t.duration_since(clock.started).as_secs_f32();
        clock.tap_at(t, TapSource::Keyboard);
        let post_elapsed = t.duration_since(clock.started).as_secs_f32();
        assert!(
            post_elapsed <= pre_elapsed + 1e-3,
            "elapsed jumped forward past the tap: pre={pre_elapsed}s, \
             post={post_elapsed}s"
        );
        // Phase shift bounded: the snap throws away at most one bar of
        // elapsed time (2.0s at 120 BPM). For pre=2.7s, post should be
        // 2.0s.
        let bar_period_s = (60.0 / clock.bpm()) * 4.0;
        assert!(
            pre_elapsed - post_elapsed <= bar_period_s + 1e-3,
            "phase shift ({}s) exceeded one bar ({}s)",
            pre_elapsed - post_elapsed,
            bar_period_s
        );
    }

    /// PCleanup.6.2 — a tap before any time has elapsed (started ≈ now,
    /// elapsed ≈ 0) is a no-op for the re-anchor — there's no
    /// partial-bar to snap back to. Specifically: `started` does not
    /// rewind to before its own value (the saturating subtraction in
    /// the implementation guards against this).
    #[test]
    fn tap_at_zero_elapsed_does_not_rewind_started() {
        let mut clock = Clock::for_test(Duration::ZERO, 120.0);
        let started_before = clock.started;
        let t = Instant::now();
        clock.tap_at(t, TapSource::Keyboard);
        // started should not move (it might creep forward by microseconds
        // due to the elapsed measurement itself, but cannot move backwards).
        assert!(
            clock.started >= started_before,
            "started moved backwards on a zero-elapsed tap"
        );
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
