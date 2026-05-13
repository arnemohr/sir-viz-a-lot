//! P6.5.1 — Transport state machine.
//!
//! Holds the per-session cuelist position, armed-next cue, crossfade progress,
//! follow chain, and (from P6.5.3) BPM-quantize + timecode-trigger state.
//!
//! `TransportState` is **session-only** — it is not serialised to the project
//! file. The companion `Project.cues` (P6.2.1) holds the authored cuelist; the
//! transport reads from it but does not own it.
//!
//! ## Integration with `EditingState`
//!
//! `EditingState` carries `pub transport: TransportState`. The per-frame tick
//! `TransportState::tick(delta_s, bpm, cues)` is called once per frame from
//! the render path *outside* the mutable borrow of UI state, mirroring the
//! `SideEffect` pattern in `apply_command`.
//!
//! ## No tokio
//!
//! All state updates are synchronous; the transport tick is a pure function
//! that advances `fade_progress` and fires follow-chain cues without any
//! async machinery.

use crate::project::schema::{BpmQuantize, Cue, CueFireMode, TimecodePosition};

/// P6.5.1 — Session-scoped transport state.
///
/// Not serialised; lives on `EditingState` and is reset to `default()` when
/// a new session starts.
#[derive(Debug, Clone)]
pub struct TransportState {
    /// Index of the cue currently live on the projector. `None` when no cue
    /// has been fired yet this session.
    pub current_cue: Option<usize>,
    /// Index of the next cue to fire when `CueGo` is dispatched. `None` when
    /// no cue is armed. Typically one ahead of `current_cue`; the operator
    /// can move the arm freely with `CueArmNext` / `CueArmPrev`.
    pub armed_cue: Option<usize>,
    /// Crossfade progress into the current cue's `in_time_s`. 0.0 = fade just
    /// started; 1.0 = fade complete. Frozen at 1.0 while in hold or after
    /// the cue is fully live.
    pub fade_progress: f32,
    /// Elapsed seconds in the current cue's hold period. Compared against
    /// `cues[current_cue].hold_time_s` each tick; when it exceeds the hold
    /// time the follow chain fires (P6.5.2).
    pub hold_elapsed_s: f32,
    /// Pending follow-chain indices: cues waiting to auto-fire in sequence.
    /// The transport pops from the front and fires the next index when the
    /// current cue's hold expires and its `fire_mode` is `Follow`.
    pub follow_chain: Vec<usize>,
    /// Most-recent timecode position received from an LTC/MTC decoder.
    /// Updated externally by the decoder thread (P6.11.2 / P6.12.1).
    /// `None` when no timecode signal is active.
    pub last_timecode_position: Option<TimecodePosition>,
    /// P6.5.3 — global quantize override (session-only, not saved to project).
    /// When `Some(n)`, all cues fire on the next n-bar boundary regardless of
    /// their per-cue `bpm_quantize` setting. When `None`, per-cue settings apply.
    pub global_quantize_override: Option<BpmQuantize>,
    /// Accumulated beat time (seconds) used to detect bar boundaries for BPM
    /// quantize. Advances with each tick; resets on `tap` or BPM change.
    pub beat_elapsed_s: f32,
    /// P6.5.3 — cue armed and waiting for the next BPM-bar boundary.
    pub quantize_pending_cue: Option<usize>,
}

impl Default for TransportState {
    fn default() -> Self {
        TransportState {
            current_cue: None,
            armed_cue: None,
            fade_progress: 1.0,
            hold_elapsed_s: 0.0,
            follow_chain: Vec::new(),
            last_timecode_position: None,
            global_quantize_override: None,
            beat_elapsed_s: 0.0,
            quantize_pending_cue: None,
        }
    }
}

impl TransportState {
    /// P6.5.1 — Advance the transport by `delta_s` seconds.
    ///
    /// Updates `fade_progress` for the current cue's `in_time_s`, then
    /// checks whether the hold period has expired and the follow chain
    /// should auto-fire (P6.5.2).
    ///
    /// Returns `Some(cue_idx)` when a follow-chain auto-fire occurs
    /// (i.e. the caller should apply `Command::SceneRecall(cue_idx)` on
    /// the same frame). `None` when no auto-fire occurred this tick.
    pub fn tick(&mut self, delta_s: f32, bpm: f32, cues: &[Cue]) -> Option<usize> {
        let Some(cur_idx) = self.current_cue else {
            // No cue live — advance beat clock for quantize purposes but no
            // scene-recall side effect.
            self.beat_elapsed_s += delta_s;
            return None;
        };
        let Some(cue) = cues.get(cur_idx) else {
            // Cue removed from list mid-session — clear state and bail.
            self.current_cue = None;
            self.fade_progress = 1.0;
            return None;
        };

        // Advance beat clock.
        self.beat_elapsed_s += delta_s;

        // --- In-time crossfade progress ---
        if cue.in_time_s > 0.0 {
            self.fade_progress = (self.fade_progress + delta_s / cue.in_time_s).clamp(0.0, 1.0);
        } else {
            self.fade_progress = 1.0;
        }

        // Once the in-time fade is complete, count hold time.
        if self.fade_progress >= 1.0 {
            if cue.fire_mode == CueFireMode::Follow {
                let hold_limit = cue.hold_time_s.unwrap_or(f32::INFINITY);
                self.hold_elapsed_s += delta_s;

                if self.hold_elapsed_s >= hold_limit {
                    // Follow chain: fire the next cue automatically.
                    if let Some(next_idx) = self.follow_chain.first().copied() {
                        self.follow_chain.remove(0);
                        self.fire_cue(next_idx);
                        return Some(next_idx);
                    } else {
                        // Chain exhausted — check for armed cue.
                        if let Some(armed) = self.armed_cue {
                            let next = armed;
                            self.armed_cue = None;
                            self.fire_cue(next);
                            return Some(next);
                        }
                    }
                }
            }
            // GoOnTrigger: hold_elapsed_s does not advance (operator controls).
        }

        None
    }

    /// P6.5.1 — Fire a cue: set `current_cue`, reset `fade_progress` and
    /// `hold_elapsed_s`. Callers are responsible for dispatching the scene
    /// recall to the renderer.
    pub fn fire_cue(&mut self, cue_idx: usize) {
        self.current_cue = Some(cue_idx);
        self.fade_progress = 0.0;
        self.hold_elapsed_s = 0.0;
    }

    /// Arm the next cue (move the armed pointer forward by one).
    pub fn arm_next(&mut self, cue_count: usize) {
        self.armed_cue = Some(match self.armed_cue {
            None => self.current_cue.map(|c| c + 1).unwrap_or(0),
            Some(idx) => (idx + 1).min(cue_count.saturating_sub(1)),
        });
    }

    /// Arm the previous cue (move the armed pointer back by one).
    pub fn arm_prev(&mut self) {
        self.armed_cue = Some(match self.armed_cue {
            None => self.current_cue.and_then(|c| c.checked_sub(1)).unwrap_or(0),
            Some(idx) => idx.saturating_sub(1),
        });
    }

    /// Back-step: step back one cue (fire the previous cue and re-arm).
    pub fn back_step(&mut self, cue_count: usize) {
        let prev = match self.current_cue {
            Some(idx) if idx > 0 => idx - 1,
            _ => return, // Already at the start.
        };
        self.fire_cue(prev);
        // Re-arm to the cue after the one we just stepped back to.
        self.armed_cue = Some((prev + 1).min(cue_count.saturating_sub(1)));
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::schema::Cue;

    fn snap_cue(name: &str) -> Cue {
        Cue::new(name, serde_json::json!({}), None)
    }

    fn follow_cue(name: &str, hold_s: f32) -> Cue {
        let mut c = Cue::new(name, serde_json::json!({}), None);
        c.fire_mode = CueFireMode::Follow;
        c.hold_time_s = Some(hold_s);
        c
    }

    /// P6.5.1 — tick advances fade_progress based on in_time_s.
    #[test]
    fn tick_advances_fade_progress() {
        let cues = vec![{
            let mut c = snap_cue("c0");
            c.in_time_s = 2.0;
            c
        }];
        let mut ts = TransportState::default();
        ts.fire_cue(0);
        // After 1 second of a 2-second fade, progress should be 0.5.
        ts.tick(1.0, 120.0, &cues);
        assert!(
            (ts.fade_progress - 0.5).abs() < 1e-5,
            "fade_progress after 1s of 2s in_time: expected 0.5, got {}",
            ts.fade_progress
        );
    }

    /// P6.5.1 — tick returns None when no auto-fire occurs.
    #[test]
    fn tick_returns_none_when_no_auto_fire() {
        let cues = vec![snap_cue("c0")];
        let mut ts = TransportState::default();
        ts.fire_cue(0);
        let result = ts.tick(0.016, 120.0, &cues);
        assert_eq!(result, None, "GoOnTrigger cue should not auto-fire");
    }

    /// P6.5.2 — follow chain auto-advances through Follow-mode cues.
    #[test]
    fn follow_chain_auto_advances() {
        // 3 cues: Follow(0.1s) → Follow(0.1s) → GoOnTrigger
        let cues = vec![
            follow_cue("c0", 0.1),
            follow_cue("c1", 0.1),
            snap_cue("c2"), // GoOnTrigger (default)
        ];
        let mut ts = TransportState::default();
        ts.follow_chain = vec![1, 2];
        ts.fire_cue(0);

        // Tick past hold time of cue 0 (> 0.1s).
        let fired = ts.tick(0.2, 120.0, &cues);
        assert_eq!(fired, Some(1), "should auto-fire cue 1 after follow");
        assert_eq!(ts.current_cue, Some(1));

        // Tick again — cue 1 is also Follow with 0.1s hold.
        let fired2 = ts.tick(0.2, 120.0, &cues);
        assert_eq!(fired2, Some(2), "should auto-fire cue 2 after follow");
        assert_eq!(ts.current_cue, Some(2));

        // Cue 2 is GoOnTrigger — no further auto-fire.
        let fired3 = ts.tick(1.0, 120.0, &cues);
        assert_eq!(fired3, None, "GoOnTrigger should halt the chain");
        assert_eq!(ts.current_cue, Some(2));
    }

    /// P6.5.2 — chain exhaustion does not panic.
    #[test]
    fn chain_exhaustion_does_not_panic() {
        let cues = vec![follow_cue("c0", 0.1)];
        let mut ts = TransportState::default();
        ts.follow_chain = vec![]; // No follow entries
        ts.fire_cue(0);
        // Tick well past hold — follow chain is empty, nothing to fire.
        let result = ts.tick(1.0, 120.0, &cues);
        assert_eq!(result, None, "empty follow chain should not panic or fire");
    }

    /// P6.5.1 — arm_next and arm_prev work correctly.
    #[test]
    fn arm_navigation() {
        let mut ts = TransportState::default();
        ts.current_cue = Some(0);
        ts.arm_next(3);
        assert_eq!(ts.armed_cue, Some(1));
        ts.arm_next(3);
        assert_eq!(ts.armed_cue, Some(2));
        ts.arm_next(3);
        assert_eq!(ts.armed_cue, Some(2), "arm_next clamps at end");
        ts.arm_prev();
        assert_eq!(ts.armed_cue, Some(1));
    }

    // P6.5.2 — proptest: follow chain invariants.
    #[cfg(test)]
    mod proptest_follow_chain {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(proptest::test_runner::Config::with_cases(1024))]

            /// P6.5.2 — follow chains always terminate (no infinite loop).
            /// GoOnTrigger always halts the chain.
            /// fade_progress is always in [0.0, 1.0] after any number of ticks.
            #[test]
            fn follow_chain_invariants(
                modes in proptest::collection::vec(proptest::bool::ANY, 1..=8_usize),
                hold_s in 0.01_f32..=1.0_f32,
            ) {
                let cues: Vec<Cue> = modes
                    .iter()
                    .enumerate()
                    .map(|(i, &follow)| {
                        let mut c = Cue::new(format!("c{i}"), serde_json::json!({}), None);
                        if follow {
                            c.fire_mode = CueFireMode::Follow;
                            c.hold_time_s = Some(hold_s);
                        }
                        c
                    })
                    .collect();

                let mut ts = TransportState::default();
                // Set up follow chain as all-indices after the first cue.
                ts.follow_chain = (1..cues.len()).collect();
                ts.fire_cue(0);

                let max_ticks = cues.len() * 100;
                let mut auto_fires = 0usize;
                for _ in 0..max_ticks {
                    let result = ts.tick(hold_s + 0.01, 120.0, &cues);
                    if result.is_some() {
                        auto_fires += 1;
                    }
                    prop_assert!(
                        (0.0_f32..=1.0_f32).contains(&ts.fade_progress),
                        "fade_progress out of [0,1]: {}",
                        ts.fade_progress
                    );
                }

                // Total auto-fires must be <= cues.len() - 1 (chain length).
                prop_assert!(
                    auto_fires <= cues.len().saturating_sub(1),
                    "auto-fires {} exceeded chain length {}",
                    auto_fires,
                    cues.len()
                );

                // If the last cue is GoOnTrigger, the chain must have halted.
                let last_is_got = cues.last().map(|c| c.fire_mode == CueFireMode::GoOnTrigger).unwrap_or(false);
                if last_is_got && cues.len() > 1 {
                    // After all Follow cues auto-fired, the GoOnTrigger cue must not auto-fire.
                    let more_fires = (0..10).filter_map(|_| ts.tick(hold_s + 0.01, 120.0, &cues)).count();
                    prop_assert_eq!(more_fires, 0, "GoOnTrigger cue fired automatically");
                }
            }
        }
    }

    /// P6.5.1 — fade_progress is always in [0.0, 1.0] after any number of ticks.
    #[test]
    fn fade_progress_clamped_to_unit_interval() {
        let cues = vec![{
            let mut c = snap_cue("c0");
            c.in_time_s = 0.001; // Very short fade
            c
        }];
        let mut ts = TransportState::default();
        ts.fire_cue(0);
        for _ in 0..100 {
            ts.tick(0.1, 120.0, &cues);
            assert!(
                (0.0..=1.0).contains(&ts.fade_progress),
                "fade_progress out of range: {}",
                ts.fade_progress
            );
        }
    }
}
