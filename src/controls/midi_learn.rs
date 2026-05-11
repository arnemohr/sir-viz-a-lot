//! P0.2.5 — process-wide MIDI-learn state.
//!
//! Shared between the UI thread (which arms / cancels / times out)
//! and the MIDI input callback thread (which observes-and-clears
//! when a CC arrives). Uses a Mutex behind a OnceLock so all paths
//! reach the same instance.
//!
//! The module is gated `#[cfg(feature = "v3")]` because `LearnTarget`
//! embeds `ModulatorField`, which lives in `project::command` —
//! itself v3-only. The right-click context menu that arms learn-mode
//! only appears in the v3 `modulator_slider`, so non-v3 builds have
//! no path to call these functions.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// Identifier for the parameter being learned. Mirrors the address
/// tuple `SetModulator` uses, so the mutation glue is one
/// constructor call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LearnTarget {
    pub layer_idx: usize,
    pub effect_idx: usize,
    pub field: crate::project::command::ModulatorField,
}

/// Inner state. `None` when not armed.
#[derive(Debug, Clone)]
struct LearnInner {
    target: LearnTarget,
    armed_at: Instant,
}

static STATE: OnceLock<Mutex<Option<LearnInner>>> = OnceLock::new();
const LEARN_TIMEOUT_SECS: u64 = 30;

fn slot() -> &'static Mutex<Option<LearnInner>> {
    STATE.get_or_init(|| Mutex::new(None))
}

/// Arm learn-mode for `target`. Replaces any prior target (the
/// operator can re-aim mid-listen).
pub fn arm(target: LearnTarget) {
    let mut g = slot().lock().expect("midi_learn state poisoned");
    *g = Some(LearnInner {
        target,
        armed_at: Instant::now(),
    });
}

/// Cancel learn-mode without binding. Idempotent.
pub fn cancel() {
    if let Ok(mut g) = slot().lock() {
        *g = None;
    }
}

/// Is `target` the active learn target?
pub fn is_armed_for(target: LearnTarget) -> bool {
    slot()
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|i| i.target == target))
        .unwrap_or(false)
}

/// Any target currently armed?
pub fn is_active() -> bool {
    slot()
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|_| ()))
        .is_some()
}

/// UI tick: if armed and the 30 s deadline passed, clear and return
/// the timed-out target so the caller can toast.
pub fn poll_timeout() -> Option<LearnTarget> {
    let mut g = slot().lock().ok()?;
    let inner = g.as_ref()?;
    if inner.armed_at.elapsed().as_secs() >= LEARN_TIMEOUT_SECS {
        let t = inner.target;
        *g = None;
        Some(t)
    } else {
        None
    }
}

/// MIDI callback: if armed, take the target and clear in one critical
/// section. The caller (callback thread) then emits a
/// `Command::MidiLearnCapture` carrying both `target` and `(channel, cc)`.
pub fn take_target_if_armed() -> Option<LearnTarget> {
    let mut g = slot().lock().ok()?;
    let t = g.as_ref().map(|i| i.target)?;
    *g = None;
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Ensure the global slot is clean before each test run.
    fn clean_slate() {
        cancel();
    }

    fn make_target(layer: usize) -> LearnTarget {
        LearnTarget {
            layer_idx: layer,
            effect_idx: 0,
            field: crate::project::command::ModulatorField::ColorHue,
        }
    }

    /// P0.2.5-T1: arm, verify active, cancel, verify inactive.
    #[test]
    fn full_state_machine_roundtrip() {
        clean_slate();

        // --- T1: arm_and_cancel_roundtrip ---
        let ta = make_target(0);
        arm(ta);
        assert!(is_active(), "should be active after arm");
        cancel();
        assert!(!is_active(), "should be inactive after cancel");

        // --- T2: is_armed_for_matches_target ---
        let ta = make_target(1);
        let tb = make_target(2);
        arm(ta);
        assert!(is_armed_for(ta), "armed for ta");
        assert!(!is_armed_for(tb), "not armed for tb");
        cancel();

        // --- T3: take_target_if_armed_clears_state ---
        let ta = make_target(3);
        arm(ta);
        let taken = take_target_if_armed();
        assert_eq!(taken, Some(ta), "taken target matches armed");
        assert!(!is_active(), "no longer active after take");

        // --- T4: take_target_if_armed_when_unarmed_returns_none ---
        let result = take_target_if_armed();
        assert!(result.is_none(), "take when unarmed returns None");

        // --- T5: poll_timeout_fires_after_30s ---
        let ta = make_target(4);
        arm(ta);
        // Backdating armed_at to simulate 31 s elapsed.
        {
            let mut g = slot().lock().unwrap();
            if let Some(ref mut inner) = *g {
                inner.armed_at = Instant::now() - Duration::from_secs(31);
            }
        }
        let timed_out = poll_timeout();
        assert_eq!(timed_out, Some(ta), "poll_timeout returns Some after 30 s");
        // Second call: state already cleared.
        assert!(
            poll_timeout().is_none(),
            "poll_timeout returns None on second call"
        );

        // --- T6: poll_timeout_does_not_fire_before_30s ---
        let ta = make_target(5);
        arm(ta);
        // Do NOT backdating armed_at — it's fresh.
        let early = poll_timeout();
        assert!(early.is_none(), "poll_timeout returns None before 30 s");
        cancel();
    }
}
