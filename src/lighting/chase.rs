//! P5.7.1 — `FixtureChase` data model + BPM-locked chase ticker.
//!
//! A `FixtureChase` drives a fixture group through a sequence of colour
//! steps locked to the project BPM via `Modulator::Bpm`. Each step holds
//! a colour and a hold duration in beats. The `ChaseTicker` advances the
//! step index as beat boundaries are crossed.

use serde::{Deserialize, Serialize};

use crate::lighting::fixture::FixtureGroupId;

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicU64, Ordering};

/// Stable identity for a fixture chase within a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FixtureChaseid(pub u64);

static CHASE_COUNTER: AtomicU64 = AtomicU64::new(1);

impl FixtureChaseid {
    pub fn new_unique() -> Self {
        Self(CHASE_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// A single step in a fixture chase: a colour and how many beats to hold it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaseStep {
    /// RGB colour to output during this step.
    pub color: (u8, u8, u8),
    /// Number of beats to hold this step before advancing. 1 = one beat.
    pub hold_beats: u8,
}

/// All mutable fields of a `FixtureChase` (used for `SetFixtureChaseParams` Reverse).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureChaseParams {
    pub label: String,
    pub steps: Vec<ChaseStep>,
    pub beat_divisor: u8,
}

impl FixtureChaseParams {
    pub fn from_chase(c: &FixtureChase) -> Self {
        Self {
            label: c.label.clone(),
            steps: c.steps.clone(),
            beat_divisor: c.beat_divisor,
        }
    }

    pub fn apply_to(&self, c: &mut FixtureChase) {
        c.label = self.label.clone();
        c.steps = self.steps.clone();
        c.beat_divisor = self.beat_divisor;
    }
}

/// A BPM-locked chase: a sequence of colour steps advanced in time with
/// the project BPM clock.
///
/// `beat_divisor` divides the BPM tick:
/// - 1 = one step per beat
/// - 2 = one step per half-beat
/// - 4 = one step per quarter-beat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureChase {
    /// Stable identity.
    pub id: FixtureChaseid,
    /// Operator-supplied label.
    pub label: String,
    /// The fixture group this chase drives.
    pub group_id: FixtureGroupId,
    /// Colour steps to cycle through.
    pub steps: Vec<ChaseStep>,
    /// BPM subdivider for the step advance rate.
    pub beat_divisor: u8,
}

impl FixtureChase {
    pub fn new_default(group_id: FixtureGroupId) -> Self {
        Self {
            id: FixtureChaseid::new_unique(),
            label: "New chase".to_string(),
            group_id,
            steps: vec![
                ChaseStep {
                    color: (255, 0, 0),
                    hold_beats: 1,
                },
                ChaseStep {
                    color: (0, 255, 0),
                    hold_beats: 1,
                },
                ChaseStep {
                    color: (0, 0, 255),
                    hold_beats: 1,
                },
            ],
            beat_divisor: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Chase ticker (P5.7.5)
// ---------------------------------------------------------------------------

/// Advances a chase's step index based on BPM and elapsed time.
///
/// Call `advance(bpm, dt)` once per lighting thread tick. Returns the
/// current step index, or `None` if BPM is not set (0.0) or the chase
/// has no steps.
pub struct ChaseTicker {
    /// Accumulated beat phase (0.0 = start of a beat, 1.0 = next beat).
    beat_phase: f32,
    /// Current step index within the chase's `steps` slice.
    step_index: usize,
}

impl ChaseTicker {
    pub fn new() -> Self {
        Self {
            beat_phase: 0.0,
            step_index: 0,
        }
    }

    /// Advance the ticker by `dt` seconds at `bpm` beats per minute.
    ///
    /// Returns the current step index, or `None` if BPM is 0 or steps is 0.
    pub fn advance(&mut self, chase: &FixtureChase, bpm: f32, dt: f32) -> Option<usize> {
        if bpm <= 0.0 || chase.steps.is_empty() {
            return None;
        }
        let beats_per_sec = bpm / 60.0;
        let sub_beats_per_sec = beats_per_sec * f32::from(chase.beat_divisor.max(1));
        self.beat_phase += sub_beats_per_sec * dt;

        while self.beat_phase >= 1.0 {
            self.beat_phase -= 1.0;
            self.step_index = (self.step_index + 1) % chase.steps.len();
        }

        Some(self.step_index)
    }
}

impl Default for ChaseTicker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::fixture::FixtureGroupId;

    fn dummy_group_id() -> FixtureGroupId {
        FixtureGroupId(42)
    }

    /// P5.7.1 — serde roundtrip for FixtureChase.
    #[test]
    fn fixture_chase_serde_roundtrip() {
        let chase = FixtureChase::new_default(dummy_group_id());
        let json = serde_json::to_string(&chase).expect("serialize");
        let back: FixtureChase = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.label, chase.label);
        assert_eq!(back.steps.len(), 3);
        assert_eq!(back.beat_divisor, 1);
    }

    /// P5.7.5 — at 120 BPM with beat_divisor=1, step advances every 0.5 s.
    #[test]
    fn chase_ticker_advances_at_120_bpm() {
        let chase = FixtureChase::new_default(dummy_group_id());
        let mut ticker = ChaseTicker::new();

        // Step 0 initially.
        assert_eq!(ticker.advance(&chase, 120.0, 0.0), Some(0));

        // After 0.5 s at 120 BPM → one beat → advance to step 1.
        let result = ticker.advance(&chase, 120.0, 0.5);
        assert_eq!(
            result,
            Some(1),
            "should advance to step 1 after 0.5 s at 120 BPM"
        );

        // After another 0.5 s → step 2.
        let result = ticker.advance(&chase, 120.0, 0.5);
        assert_eq!(result, Some(2));

        // After another 0.5 s → wraps back to step 0.
        let result = ticker.advance(&chase, 120.0, 0.5);
        assert_eq!(result, Some(0));
    }

    /// P5.7.5 — bpm = 0 returns None.
    #[test]
    fn chase_ticker_returns_none_when_no_bpm() {
        let chase = FixtureChase::new_default(dummy_group_id());
        let mut ticker = ChaseTicker::new();
        assert_eq!(ticker.advance(&chase, 0.0, 1.0), None);
    }
}
