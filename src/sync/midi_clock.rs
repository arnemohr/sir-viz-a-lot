//! P6.12.2 — MIDI-clock BPM tracker.
//!
//! MIDI clock sends 24 pulses per quarter note (status byte 0xF8).
//! This module timestamps incoming 0xF8 messages and computes a rolling BPM
//! average from the last 24 pulses (one quarter note).
//!
//! The derived BPM is stored in an `Arc<RwLock<Option<f32>>>` readable by the
//! main thread via [`MidiClockTracker::bpm`]. The transport HUD shows "MIDI
//! Clock" as the tap source when this path is active.
//!
//! ## Feature gate
//!
//! MIDI-clock decoding is part of `--features midi`; no new feature gate
//! is needed.

use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Number of clock pulses per quarter note in the MIDI clock spec.
const PULSES_PER_BEAT: usize = 24;

/// P6.12.2 — Derives a rolling BPM average from MIDI-clock 0xF8 pulses.
///
/// The MIDI callback in `midi.rs` calls [`MidiClockTracker::push_pulse`]
/// for each 0xF8 message. The BPM is readable from [`MidiClockTracker::bpm`].
pub struct MidiClockTracker {
    /// Shared BPM slot. `None` when fewer than 2 pulses have been received.
    bpm: Arc<RwLock<Option<f32>>>,
    /// Timestamps of the last `PULSES_PER_BEAT` pulses.
    pulse_times: [Option<Instant>; PULSES_PER_BEAT],
    /// Write index into `pulse_times` (ring buffer).
    write_idx: usize,
    /// Total pulses received (saturates at `PULSES_PER_BEAT` for the ring).
    pulse_count: usize,
}

impl MidiClockTracker {
    /// Create a new tracker with an empty BPM slot.
    pub fn new() -> Self {
        MidiClockTracker {
            bpm: Arc::new(RwLock::new(None)),
            pulse_times: [None; PULSES_PER_BEAT],
            write_idx: 0,
            pulse_count: 0,
        }
    }

    /// Return a clone of the shared BPM slot.
    pub fn bpm(&self) -> Arc<RwLock<Option<f32>>> {
        self.bpm.clone()
    }

    /// Record a 0xF8 pulse received now. After receiving 2+ pulses, updates
    /// the shared BPM slot with a rolling average over the last 24 pulses.
    pub fn push_pulse(&mut self) {
        self.push_pulse_at(Instant::now());
    }

    /// Like [`push_pulse`], but accepts an explicit timestamp for deterministic
    /// tests.
    pub fn push_pulse_at(&mut self, now: Instant) {
        self.pulse_times[self.write_idx] = Some(now);
        self.write_idx = (self.write_idx + 1) % PULSES_PER_BEAT;
        self.pulse_count += 1;

        // We need at least 2 pulses to compute a BPM.
        let filled = self.pulse_count.min(PULSES_PER_BEAT);
        if filled < 2 {
            return;
        }

        // The oldest entry in the ring buffer (or the one just before the
        // current write pointer if the buffer is full).
        let oldest_idx = if self.pulse_count >= PULSES_PER_BEAT {
            // Ring is full: oldest is at write_idx (which we just overwrote).
            self.write_idx
        } else {
            // Ring is not full: oldest is at index 0.
            0
        };

        let oldest = match self.pulse_times[oldest_idx] {
            Some(t) => t,
            None => return,
        };
        let newest =
            match self.pulse_times[(self.write_idx + PULSES_PER_BEAT - 1) % PULSES_PER_BEAT] {
                Some(t) => t,
                None => return,
            };

        let span_s = newest.duration_since(oldest).as_secs_f64();
        if span_s <= 0.0 {
            return;
        }

        // `filled - 1` intervals across `span_s` seconds.
        // Each interval = one pulse gap = 1/24 of a quarter note.
        let intervals = (filled - 1) as f64;
        let pulse_period_s = span_s / intervals;
        let quarter_note_s = pulse_period_s * PULSES_PER_BEAT as f64;
        let bpm = 60.0 / quarter_note_s;

        if let Ok(mut guard) = self.bpm.write() {
            *guard = Some(bpm as f32);
        }
    }
}

impl Default for MidiClockTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Feed 25 pulses at 120 BPM spacing (1/24 of 0.5 s = ~20.83 ms each).
    /// After 24 pulses the BPM slot should be ~120.
    #[test]
    fn midi_clock_bpm_converges_to_120() {
        let mut tracker = MidiClockTracker::new();
        let bpm_slot = tracker.bpm();

        let start = Instant::now();
        // At 120 BPM, a quarter note = 0.5 s, so each pulse = 0.5/24 s.
        let pulse_gap = Duration::from_nanos((500_000_000 / PULSES_PER_BEAT) as u64);

        for i in 0..=PULSES_PER_BEAT {
            tracker.push_pulse_at(start + pulse_gap * i as u32);
        }

        let bpm = bpm_slot.read().unwrap();
        let bpm = bpm.expect("BPM should be set after 25 pulses");
        assert!(
            (bpm - 120.0).abs() < 1.0,
            "BPM should be ~120, got {bpm:.2}"
        );
    }

    /// Fewer than 2 pulses → BPM slot remains None.
    #[test]
    fn fewer_than_2_pulses_gives_none() {
        let mut tracker = MidiClockTracker::new();
        let bpm_slot = tracker.bpm();

        tracker.push_pulse();
        assert!(bpm_slot.read().unwrap().is_none());
    }

    /// Two pulses at a known interval give the correct BPM.
    #[test]
    fn two_pulses_gives_bpm() {
        let mut tracker = MidiClockTracker::new();
        let bpm_slot = tracker.bpm();

        let t0 = Instant::now();
        // Gap = 1/(24 * 120/60) = 1/48 s ≈ 20.83 ms → 120 BPM
        let gap = Duration::from_nanos(500_000_000 / PULSES_PER_BEAT as u64);
        tracker.push_pulse_at(t0);
        tracker.push_pulse_at(t0 + gap);

        let bpm = bpm_slot.read().unwrap();
        let bpm = bpm.expect("BPM should be set after 2 pulses");
        // With only 1 interval, BPM = 60 / (gap * 24).
        assert!(
            (bpm - 120.0).abs() < 2.0,
            "BPM should be ~120, got {bpm:.2}"
        );
    }
}
