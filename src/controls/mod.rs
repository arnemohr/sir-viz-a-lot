//! Live input. v1 in-scope: tap-tempo (Space) + scene-recall hotkeys (1–9).
//! Live MIDI / OSC parameter binding ships in v0.4 via new `Modulator`
//! variants (`OscBound`, `MidiBound`) parallel to the existing
//! `Modulator::Audio` path — see `specs/004-phase-0-tasks.md` W2.

#![allow(dead_code)]

pub mod keyboard;
#[cfg(feature = "midi")]
pub mod midi;
/// P0.2.5 — process-wide MIDI-learn state (armed target, timeout, take).
/// Gated on v3 because `LearnTarget` embeds `ModulatorField` from
/// `project::command`, which is itself v3-only. The right-click menu that
/// arms learn-mode lives exclusively in the v3 `modulator_slider`.
#[cfg(feature = "v3")]
pub mod midi_learn;
#[cfg(feature = "osc")]
pub mod osc;

use crate::clock::TapSource;

/// 003-T2.3 — where the launcher should pull the project from when the
/// operator clicks one of the start buttons.
///
/// The variant set mirrors the three launcher buttons defined in
/// `T-003-T2.4`: a blank canvas (Empty), a recently-opened project from
/// `~/Documents/rmap/` (RecentPath), or a bundled demo (Demo). The
/// `Demo` payload is a stable identifier — currently only `"window-glow"`
/// — that maps to a path under `assets/demos/` resolved at launch time
/// by `resolve_project_source` (see `src/app.rs`).
#[cfg(feature = "v3")]
#[derive(Debug, Clone)]
pub enum ProjectSource {
    Empty,
    RecentPath(std::path::PathBuf),
    Demo(&'static str),
}

/// Operator-driven event coming from any registered [`Source`].
#[derive(Debug, Clone)]
pub enum Command {
    TapTempo(TapSource),
    SceneRecall(usize),
    Blackout,
    Freeze,
    /// 003-T1.32 — cycle the projector test pattern (T hotkey). Routed
    /// through `apply_command` so telemetry sees one event per press
    /// regardless of source (keyboard / future MIDI-mapped button).
    CycleTestPattern,
    /// 003-T1.32 — toggle the editor overlay on the output window
    /// (O hotkey). Same telemetry rationale as `CycleTestPattern`.
    ToggleEditorOverlay,
    /// 003-T2.3 — launcher → editor transition. Dispatched from
    /// `LauncherState`, not from `EditingState`, so it does not flow
    /// through the per-frame `apply_command(&mut EditingState, …)`
    /// path; see `apply_launch_command` in `src/app.rs`.
    ///
    /// Treated as `non_undoable` (the operator cannot Cmd-Z back to
    /// the launcher) — the variant is recorded in telemetry but never
    /// pushed onto the undo stack.
    #[cfg(feature = "v3")]
    Launch {
        project: ProjectSource,
        monitor: usize,
        /// Secondary monitor index from the launcher's two-projector picker.
        /// `None` for single-projector sessions. P0.7.2 wires this to open
        /// a second `OutputWindow` on the selected display.
        secondary_monitor: Option<usize>,
        windowed: bool,
    },
    /// 003-T2.24 — operator clicked "Find this file…" on a missing-
    /// media toast. The handler in `apply_command` runs an
    /// `rfd::FileDialog` filtered to the original asset's extension
    /// and, on a successful pick, emits a `Mutation::RelinkAssetPath`
    /// via the undo stack so the relink is Cmd-Z reversible.
    ///
    /// `missing_path` is captured from the audit finding so the dialog
    /// can prefill its title and filter to the extension the operator
    /// is replacing.
    #[cfg(feature = "v3")]
    OpenRelinkPicker {
        layer_idx: usize,
        missing_path: std::path::PathBuf,
    },
    /// 003-T4.8 — operator clicked "Save as…" in the toolbar. The handler
    /// in `apply_command` opens an rfd Save dialog; on a successful pick
    /// the project is written via `save_portable` (relativises asset paths),
    /// `project_file_path` is updated, and the dirty flag is cleared.
    #[cfg(feature = "v3")]
    OpenSaveAsPicker,
    /// 003-T4.3 — operator clicked the "+" tile in the cue strip, requesting
    /// that the current project state be saved as a new scene slot. The handler
    /// in `apply_command` captures a thumbnail placeholder and pushes a
    /// `Mutation::SetProjectScenes` through the undo stack.
    #[cfg(feature = "v3")]
    SceneSave,
    /// 003-T4.17 — operator clicked "Go live". Transitions
    /// `AppState::Editing → GoLive` and hot-swaps the projector to borderless
    /// fullscreen on the monitor stored in `EditingState`.
    ///
    /// Routed through `App::window_event` (not `apply_command`) because the
    /// transition mutates `AppState` one level above `EditingState`. An
    /// `apply_command` arm exists only to drop the event with a warning if it
    /// somehow leaks into the editing dispatch path after the transition.
    ///
    /// `non_undoable: true` — the operator cannot Cmd-Z back from a live show.
    #[cfg(feature = "v3")]
    EnterGoLive,
    /// 003-T4.17 — operator clicked "Stop". Transitions
    /// `AppState::GoLive → Editing` and returns the projector to windowed mode.
    ///
    /// Same dispatch note as `EnterGoLive`.
    #[cfg(feature = "v3")]
    ExitGoLive,
    /// 003-T4.16a — operator clicked "Preview". Opens a child `PreviewWindow`
    /// on the laptop in `EditingState::preview_window` so the operator can
    /// dry-run the show without a projector.
    ///
    /// No display-sleep assertion is held during preview mode.
    #[cfg(feature = "v3")]
    OpenPreview,
    /// 003-T4.16a — close the preview window opened by `OpenPreview`.
    /// The child surface is dropped; the next frame skips preview rendering.
    #[cfg(feature = "v3")]
    ClosePreview,
    /// P0.2.5 — MIDI callback captured a CC while learn-mode was armed.
    /// Carries the target parameter address and the raw `(channel, cc)` pair.
    /// Dispatched from the midir callback thread; handled in `apply_command`
    /// by building a `SetModulator(MidiBound)` mutation on the undo stack.
    ///
    /// Double-gated: `midi` because it originates in the MIDI callback;
    /// `v3` because `apply_command` needs `set_modulator_mutation` and the
    /// undo stack, both of which are v3-only.
    #[cfg(all(feature = "v3", feature = "midi"))]
    MidiLearnCapture {
        target: crate::controls::midi_learn::LearnTarget,
        channel: u8,
        cc: u8,
        /// Range-derived scale captured at arm-time so the resulting
        /// `MidiBound` sweeps the parameter's full range. See
        /// [`crate::controls::midi_learn::LearnInner`] for the formula.
        scale: f32,
        /// Range-derived offset captured at arm-time (the parameter's
        /// `range.start()`).
        offset: f32,
    },
    // -------------------------------------------------------------------
    // P6.4.2 — Cue navigation commands
    // -------------------------------------------------------------------
    /// P6.4.2 — Fire the armed cue (Space key, MIDI Note 60 when a cue is
    /// armed). When no cue is armed, falls back to TapTempo (Note 60 dual
    /// role — see the MIDI dispatcher for the conditional dispatch).
    #[cfg(feature = "v3")]
    CueGo,
    /// P6.4.2 — Move the armed-next pointer one step forward (→ key).
    #[cfg(feature = "v3")]
    CueArmNext,
    /// P6.4.2 — Move the armed-next pointer one step backward (← key).
    #[cfg(feature = "v3")]
    CueArmPrev,
    /// P6.4.2 — Back-step: fire the previous cue and re-arm the current
    /// one (Backspace key).
    #[cfg(feature = "v3")]
    CueBackStep,
}

/// A pluggable input. v1 ships [`KeyboardSource`] (T-M4-09); v0.4
/// adds MIDI (W2.2) and OSC (W2.1) impls. Each is owned by
/// [`InputState`] and polled per frame.
pub trait Source {
    /// Drain any pending events since the last poll.
    fn poll(&mut self) -> Vec<Command>;
}

/// Live input aggregator. Owns all registered [`Source`] impls and
/// tracks per-frame state such as the currently active scene.
#[derive(Default)]
pub struct InputState {
    pub current_scene: Option<usize>,
    sources: Vec<Box<dyn Source>>,
}

impl InputState {
    pub fn register(&mut self, source: Box<dyn Source>) {
        self.sources.push(source);
    }

    /// Drain every registered source, concatenating their events.
    pub fn poll(&mut self) -> Vec<Command> {
        let mut events = Vec::new();
        for s in self.sources.iter_mut() {
            events.extend(s.poll());
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use crate::clock::TapSource;

    use super::*;

    struct MockSource {
        events: Vec<Command>,
    }

    impl Source for MockSource {
        fn poll(&mut self) -> Vec<Command> {
            std::mem::take(&mut self.events)
        }
    }

    /// Smoke test: empty InputState polls to empty Vec; register + poll
    /// drains the mock source; a second poll is empty.
    #[test]
    fn source_registry_smoke() {
        // Empty state.
        let mut state = InputState::default();
        assert!(state.poll().is_empty());

        // Register a mock with two seeded events.
        let mock = MockSource {
            events: vec![Command::TapTempo(TapSource::Keyboard), Command::Blackout],
        };
        state.register(Box::new(mock));

        // First poll drains the two seeded events.
        let events = state.poll();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], Command::TapTempo(TapSource::Keyboard)));
        assert!(matches!(events[1], Command::Blackout));

        // Second poll returns nothing (source drained).
        assert!(state.poll().is_empty());
    }
}
