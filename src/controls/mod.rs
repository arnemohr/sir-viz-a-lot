//! Live input. v1 in-scope: tap-tempo (Space) + scene-recall hotkeys (1–9).
//! v1.5: MIDI (`midir`) and OSC (`rosc`) bindings via a shared
//! `Param::bind` API. Reserved here so the v1.5 add is non-disruptive.

// M7 hooks: `ParamSet`, `Source::read`, `InputState` and its methods
// are stubs that MIDI / OSC sources will consume in T-M7-05 / T-M7-06.
#![allow(dead_code)]

pub mod keyboard;
#[cfg(feature = "midi")]
pub mod midi;
#[cfg(feature = "osc")]
pub mod osc;
pub mod param;

use crate::controls::param::SourceRef;

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
    TapTempo,
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
    ParamSet {
        binding: SourceRef,
        value: f32,
    },
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
}

/// A pluggable input. v1 ships [`KeyboardSource`] (T-M4-09); v1.5
/// adds MIDI (T-M7-05) and OSC (T-M7-06) impls. Each is owned by
/// [`InputState`] and polled per frame.
pub trait Source {
    /// Drain any pending events since the last poll.
    fn poll(&mut self) -> Vec<Command>;

    /// Read a source's current value by handle. Used by
    /// `Param::Bound` resolution. Default impl returns `None` so
    /// keyboard / OSC sources without queryable state don't have to
    /// implement it.
    fn read(&self, _binding: SourceRef) -> Option<f32> {
        None
    }
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

    /// Read a value through the source registry (for `Param::Bound`).
    /// Asks each source in turn; first non-None wins.
    pub fn read(&self, binding: SourceRef) -> Option<f32> {
        for s in self.sources.iter() {
            if let Some(v) = s.read(binding) {
                return Some(v);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSource {
        events: Vec<Command>,
        read_value: Option<f32>,
    }

    impl Source for MockSource {
        fn poll(&mut self) -> Vec<Command> {
            std::mem::take(&mut self.events)
        }

        fn read(&self, _binding: SourceRef) -> Option<f32> {
            self.read_value
        }
    }

    /// Smoke test: empty InputState polls to empty Vec; register + poll
    /// drains the mock source; a second poll is empty; read() delegates.
    #[test]
    fn source_registry_smoke() {
        // Empty state.
        let mut state = InputState::default();
        assert!(state.poll().is_empty());

        // Register a mock with two seeded events.
        let mock = MockSource {
            events: vec![Command::TapTempo, Command::Blackout],
            read_value: Some(0.5),
        };
        state.register(Box::new(mock));

        // First poll drains the two seeded events.
        let events = state.poll();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], Command::TapTempo));
        assert!(matches!(events[1], Command::Blackout));

        // Second poll returns nothing (source drained).
        assert!(state.poll().is_empty());

        // read() returns the mock's value.
        assert_eq!(state.read(SourceRef(1)), Some(0.5));
    }
}
