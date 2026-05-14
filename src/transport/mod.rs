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

use crate::project::schema::{
    BpmQuantize, CcBinding, Cue, CueFireMode, OscBinding, TimecodePosition,
};

// -----------------------------------------------------------------------
// PCleanup.3.4 — Cue timing binding resolution
// -----------------------------------------------------------------------
//
// Cue carries six optional bindings (in_time/hold/out_time × MIDI/OSC).
// They were authored into the schema in P6.2.1 but the transport tick
// read `cue.in_time_s` / `cue.hold_time_s` directly, ignoring the bound
// values — a stranded feature until this commit.
//
// Resolution rules:
//   * OSC binding takes precedence over MIDI binding when both are set
//     (OSC bindings are the more deliberate / less-default-configured
//     mapping in practice).
//   * Either binding REPLACES the static value when set; the static is
//     the fallback when no binding fires.
//   * scale + offset apply linearly to the controller's normalised value
//     (0.0..=1.0 from MIDI CC or OSC 0..=1 convention), matching how
//     `Modulator::MidiBound` / `Modulator::OscBound` already resolve.
//   * MIDI provider returns 0.0 when no controller has sent that CC yet;
//     OSC provider returns 0.0 for never-seen addresses. So `scale=1.0`
//     `offset=2.0` gives a 2-second default before any movement.

fn resolve_cc_binding(b: &Option<CcBinding>) -> Option<f32> {
    b.as_ref()
        .map(|cc| crate::modulators::midi::current_value(cc.channel, cc.cc) * cc.scale + cc.offset)
}

fn resolve_osc_binding(b: &Option<OscBinding>) -> Option<f32> {
    b.as_ref()
        .map(|o| crate::modulators::osc::current_value(&o.addr) * o.scale + o.offset)
}

/// PCleanup.3.4 — Effective in-time at fire-tick. Returns the OSC-bound
/// value if set, else the MIDI-CC-bound value if set, else `cue.in_time_s`.
pub fn effective_in_time(cue: &Cue) -> f32 {
    resolve_osc_binding(&cue.in_time_osc)
        .or_else(|| resolve_cc_binding(&cue.in_time_binding))
        .unwrap_or(cue.in_time_s)
}

/// PCleanup.3.4 — Effective hold-time at fire-tick. A bound value
/// produces `Some(value)` (a concrete hold, overriding the static); an
/// unbound cue returns `cue.hold_time_s` (which may itself be `None`
/// for "indefinite hold"; behaviour matches the pre-binding code path).
pub fn effective_hold_time(cue: &Cue) -> Option<f32> {
    if let Some(v) = resolve_osc_binding(&cue.hold_osc) {
        return Some(v);
    }
    if let Some(v) = resolve_cc_binding(&cue.hold_binding) {
        return Some(v);
    }
    cue.hold_time_s
}

/// PCleanup.3.4 — Effective out-time at fire-tick. Bindings override the
/// static value when set. Out-time is currently consumed only by future
/// crossfade-out work (P6.5.x); wired here for completeness and parity
/// with in-time / hold-time binding behaviour.
pub fn effective_out_time(cue: &Cue) -> f32 {
    resolve_osc_binding(&cue.out_time_osc)
        .or_else(|| resolve_cc_binding(&cue.out_time_binding))
        .unwrap_or(cue.out_time_s)
}

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
    /// P6.5.1/P6.5.3 — Advance the transport by `delta_s` seconds.
    ///
    /// Updates `fade_progress` for the current cue's `in_time_s`, then
    /// checks whether the hold period has expired and the follow chain
    /// should auto-fire (P6.5.2). Also checks BPM-quantize pending cues
    /// and timecode-trigger cues (P6.5.3).
    ///
    /// Returns `Some(cue_idx)` when an auto-fire occurs so the caller can
    /// dispatch `Command::SceneRecall(cue_idx)`. Returns `None` otherwise.
    pub fn tick(&mut self, delta_s: f32, bpm: f32, cues: &[Cue]) -> Option<usize> {
        // Advance beat clock unconditionally (used for BPM-quantize boundary check).
        self.beat_elapsed_s += delta_s;

        // --- P6.5.3: BPM quantize pending cue check ---
        if let Some(pending_idx) = self.quantize_pending_cue {
            if let Some(fire_idx) = self.check_quantize_boundary(pending_idx, bpm, cues) {
                self.quantize_pending_cue = None;
                self.fire_cue(fire_idx);
                return Some(fire_idx);
            }
        }

        // --- P6.5.3: timecode trigger check ---
        if let Some(pos) = self.last_timecode_position {
            if let Some(fire_idx) = self.check_timecode_triggers(pos, cues) {
                self.fire_cue(fire_idx);
                return Some(fire_idx);
            }
        }

        let cur_idx = self.current_cue?;
        let Some(cue) = cues.get(cur_idx) else {
            // Cue removed from list mid-session — clear state and bail.
            self.current_cue = None;
            self.fade_progress = 1.0;
            return None;
        };

        // --- In-time crossfade progress ---
        // PCleanup.3.4 — read through effective_in_time so a per-cue
        // OSC/MIDI binding can live-trim the fade duration during the show.
        let in_time = effective_in_time(cue);
        if in_time > 0.0 {
            self.fade_progress = (self.fade_progress + delta_s / in_time).clamp(0.0, 1.0);
        } else {
            self.fade_progress = 1.0;
        }

        // Once the in-time fade is complete and fire_mode is Follow, count hold time.
        if self.fade_progress >= 1.0 && cue.fire_mode == CueFireMode::Follow {
            // PCleanup.3.4 — effective_hold_time returns the bound value
            // when set; falls back to cue.hold_time_s (which may itself be
            // None for indefinite hold).
            let hold_limit = effective_hold_time(cue).unwrap_or(f32::INFINITY);
            self.hold_elapsed_s += delta_s;

            if self.hold_elapsed_s >= hold_limit {
                // Follow chain: fire the next cue automatically.
                if let Some(next_idx) = self.follow_chain.first().copied() {
                    self.follow_chain.remove(0);
                    self.fire_cue(next_idx);
                    return Some(next_idx);
                }
                // Chain exhausted — check for armed cue.
                if let Some(armed) = self.armed_cue {
                    let next = armed;
                    self.armed_cue = None;
                    self.fire_cue(next);
                    return Some(next);
                }
            }
        }
        // GoOnTrigger: hold_elapsed_s does not advance (operator controls).

        None
    }

    /// P6.5.3 — Arm a cue for firing, applying BPM-quantize if configured.
    ///
    /// If the effective quantize setting is `Off`, fires immediately and returns
    /// `Some(cue_idx)`. If `Bars(n)`, arms the cue for the next n-bar boundary
    /// and returns `None` (the caller should not fire yet; `tick` will fire it).
    ///
    /// Effective quantize priority: global override > per-cue setting.
    pub fn go(&mut self, cue_idx: usize, cues: &[Cue], _bpm: f32) -> Option<usize> {
        let effective_quantize = match self.global_quantize_override {
            Some(gq) => gq,
            None => cues
                .get(cue_idx)
                .map(|c| c.bpm_quantize)
                .unwrap_or(BpmQuantize::Off),
        };

        match effective_quantize {
            BpmQuantize::Off => {
                // Fire immediately.
                self.quantize_pending_cue = None;
                self.fire_cue(cue_idx);
                Some(cue_idx)
            }
            BpmQuantize::Bars(_) => {
                // Arm for the next bar boundary.
                self.quantize_pending_cue = Some(cue_idx);
                None
            }
        }
    }

    /// P6.5.3 — Check whether the BPM-quantize boundary has been crossed for
    /// the pending cue. Returns `Some(cue_idx)` when it's time to fire.
    fn check_quantize_boundary(&self, pending_idx: usize, bpm: f32, cues: &[Cue]) -> Option<usize> {
        if bpm <= 0.0 {
            return None;
        }
        let bars = match self.global_quantize_override {
            Some(BpmQuantize::Bars(n)) => n,
            None => match cues.get(pending_idx).map(|c| c.bpm_quantize) {
                Some(BpmQuantize::Bars(n)) => n,
                _ => return None, // Off — shouldn't be in quantize_pending_cue
            },
            _ => return None,
        };
        if bars == 0 {
            return Some(pending_idx);
        }
        // A bar is 4 beats. Bar period = 4 * (60 / bpm) seconds.
        // n-bar period = bars * 4 * (60 / bpm) seconds.
        let bar_period_s = 4.0 * 60.0 / bpm;
        let n_bar_period_s = bar_period_s * bars as f32;
        if n_bar_period_s <= 0.0 {
            return None;
        }
        // Fire when beat_elapsed_s is within one frame of an n-bar boundary.
        let phase = self.beat_elapsed_s % n_bar_period_s;
        let frame_budget_s = 1.0 / 60.0;
        if phase < frame_budget_s {
            Some(pending_idx)
        } else {
            None
        }
    }

    /// P6.5.3 — Check all cues with timecode triggers against the current
    /// timecode position. Returns the first matching cue index.
    fn check_timecode_triggers(&self, pos: TimecodePosition, cues: &[Cue]) -> Option<usize> {
        for (idx, cue) in cues.iter().enumerate() {
            if let Some(trigger) = cue.timecode_trigger {
                if trigger == pos {
                    return Some(idx);
                }
            }
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
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::project::schema::Cue;

    fn snap_cue(name: &str) -> Cue {
        Cue::new(name, serde_json::json!({}), None)
    }

    // ----- PCleanup.3.4 — cue timing binding resolution ------------------

    /// PCleanup.3.4 — no binding set → effective_* returns the static value.
    /// Regression guard: the binding plumbing must not change behaviour for
    /// existing cues that have no bindings (the common case for v1 shows).
    #[test]
    fn effective_times_fall_back_to_static_when_unbound() {
        let mut c = snap_cue("plain");
        c.in_time_s = 2.0;
        c.hold_time_s = Some(5.0);
        c.out_time_s = 0.5;
        assert_eq!(effective_in_time(&c), 2.0);
        assert_eq!(effective_hold_time(&c), Some(5.0));
        assert_eq!(effective_out_time(&c), 0.5);
    }

    /// PCleanup.3.4 — hold_time_s = None (indefinite) is preserved when no
    /// binding fires. Catches accidental replacement with Some(0.0).
    #[test]
    fn effective_hold_time_preserves_indefinite() {
        let c = snap_cue("indefinite");
        // Cue::new defaults hold_time_s to None.
        assert_eq!(c.hold_time_s, None);
        assert_eq!(effective_hold_time(&c), None);
    }

    /// PCleanup.3.4 — OSC binding alone (never-seen address → 0.0 from
    /// provider) resolves to `0.0 * scale + offset = offset`. Lets an
    /// operator set a sensible default through the binding's offset field
    /// without sending an initial OSC packet.
    ///
    /// No PROVIDER installed in this test process (matches
    /// `modulators::osc::tests::current_value_is_zero_without_provider`'s
    /// safety guarantee), so the addr lookup yields 0.0.
    #[test]
    fn effective_in_time_osc_binding_offset_only() {
        let mut c = snap_cue("osc-bound");
        c.in_time_s = 9.0; // static — should be overridden
        c.in_time_osc = Some(OscBinding {
            addr: "/rmap/cue/1/in_time".into(),
            scale: 10.0,
            offset: 1.5, // default when address never seen
        });
        // 0.0 * 10.0 + 1.5 = 1.5; static 9.0 ignored because binding is set.
        assert!((effective_in_time(&c) - 1.5).abs() < 1e-6);
    }

    /// PCleanup.3.4 — CC binding alone with never-seen CC resolves to
    /// `offset`. Same semantics as the OSC case for parity.
    #[test]
    fn effective_in_time_cc_binding_offset_only() {
        let mut c = snap_cue("cc-bound");
        c.in_time_s = 9.0;
        c.in_time_binding = Some(CcBinding {
            channel: 0,
            cc: 7,
            scale: 5.0,
            offset: 0.25,
        });
        // 0.0 * 5.0 + 0.25 = 0.25; static 9.0 ignored.
        assert!((effective_in_time(&c) - 0.25).abs() < 1e-6);
    }

    /// PCleanup.3.4 — OSC takes precedence over CC when both are set on
    /// the same field. This is the documented resolution order.
    #[test]
    fn effective_in_time_osc_precedes_cc() {
        let mut c = snap_cue("both-bound");
        c.in_time_s = 9.0;
        c.in_time_osc = Some(OscBinding {
            addr: "/rmap/cue/1/in_time".into(),
            scale: 1.0,
            offset: 3.0, // OSC offset wins
        });
        c.in_time_binding = Some(CcBinding {
            channel: 0,
            cc: 7,
            scale: 1.0,
            offset: 7.0, // CC offset, should be shadowed by OSC
        });
        // Resolution: OSC -> 0.0 * 1.0 + 3.0 = 3.0. CC not consulted.
        assert!((effective_in_time(&c) - 3.0).abs() < 1e-6);
    }

    /// PCleanup.3.4 — bound hold time produces a concrete Some(value),
    /// overriding a static `None` (indefinite hold). Bound-by-static-None
    /// is the exact case the test exists for: an operator binds hold to
    /// a knob so the indefinite default becomes a concrete duration.
    #[test]
    fn effective_hold_time_binding_overrides_none() {
        let mut c = snap_cue("hold-bound");
        c.hold_time_s = None; // indefinite by default
        c.hold_binding = Some(CcBinding {
            channel: 0,
            cc: 8,
            scale: 1.0,
            offset: 4.0,
        });
        // Bound -> 0.0 + 4.0 = 4.0, so hold is now Some(4.0).
        assert_eq!(effective_hold_time(&c), Some(4.0));
    }

    /// PCleanup.3.4 — out_time wiring parity (currently unused at runtime
    /// but the helper exists so future crossfade-out work picks it up
    /// without re-plumbing).
    #[test]
    fn effective_out_time_respects_binding() {
        let mut c = snap_cue("out-bound");
        c.out_time_s = 1.0;
        c.out_time_osc = Some(OscBinding {
            addr: "/rmap/cue/1/out".into(),
            scale: 1.0,
            offset: 2.5,
        });
        assert!((effective_out_time(&c) - 2.5).abs() < 1e-6);
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

    // --- P6.5.3 tests ---

    /// P6.5.3 — BPM-quantize off fires immediately.
    #[test]
    fn bpm_quantize_off_fires_immediately() {
        let cues = vec![snap_cue("c0"), snap_cue("c1")];
        let mut ts = TransportState::default();
        ts.fire_cue(0);
        let result = ts.go(1, &cues, 120.0);
        assert_eq!(result, Some(1), "BpmQuantize::Off should fire immediately");
        assert_eq!(ts.current_cue, Some(1));
    }

    /// P6.5.3 — BPM-quantize Bars(4) arms the cue and fires on the next boundary.
    #[test]
    fn bpm_quantize_bars_defers_fire() {
        let mut cue1 = snap_cue("c1");
        cue1.bpm_quantize = BpmQuantize::Bars(4);
        let cues = vec![snap_cue("c0"), cue1];
        let mut ts = TransportState::default();
        ts.fire_cue(0);
        // arm for 4-bar quantize
        let result = ts.go(1, &cues, 120.0);
        assert_eq!(result, None, "Bars(4) should defer the fire");
        assert_eq!(ts.quantize_pending_cue, Some(1), "cue should be pending");

        // Advance beat_elapsed_s to just before a 4-bar boundary.
        // At 120 BPM, 4 bars = 4 * (4 * 0.5s) = 8s. A 4-bar period = 8s.
        // If beat_elapsed_s = 8.0 - 1/60 ≈ 7.983, phase ≈ 1/60 → within boundary.
        ts.beat_elapsed_s = 0.0; // Reset so we control it
        // Tick well before boundary → no fire
        let fire_before = ts.tick(7.9, 120.0, &cues);
        assert_eq!(fire_before, None, "should not fire before boundary");
        // Tick to just after boundary (beat_elapsed_s > 8s)
        // At beat_elapsed_s = 8s + epsilon, phase = epsilon < 1/60 → fires
        ts.beat_elapsed_s = 8.0 + 1e-4; // Just past the 8s boundary
        let fire_at = ts.tick(0.0, 120.0, &cues); // zero delta_s to not advance
        assert_eq!(fire_at, Some(1), "should fire at 4-bar boundary");
    }

    /// P6.5.3 — timecode trigger fires the matching cue.
    #[test]
    fn timecode_trigger_fires_matching_cue() {
        use crate::project::schema::TimecodePosition;
        let target_pos = TimecodePosition {
            hh: 0,
            mm: 0,
            ss: 10,
            ff: 0,
        };
        let mut cue1 = snap_cue("c1");
        cue1.timecode_trigger = Some(target_pos);
        let cues = vec![snap_cue("c0"), cue1];
        let mut ts = TransportState::default();
        ts.fire_cue(0);
        // Inject timecode at a non-matching position — no fire.
        ts.last_timecode_position = Some(TimecodePosition {
            hh: 0,
            mm: 0,
            ss: 5,
            ff: 0,
        });
        let no_fire = ts.tick(0.016, 120.0, &cues);
        assert_eq!(no_fire, None, "non-matching timecode should not fire");

        // Inject timecode at the matching position — should fire cue 1.
        ts.last_timecode_position = Some(target_pos);
        let fired = ts.tick(0.016, 120.0, &cues);
        assert_eq!(fired, Some(1), "matching timecode should fire cue 1");
        assert_eq!(ts.current_cue, Some(1));
    }
}
