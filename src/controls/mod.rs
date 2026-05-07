//! Live input. v1 in-scope: tap-tempo (Space) + scene-recall hotkeys (1–9).
//! v1.5: MIDI (`midir`) and OSC (`rosc`) bindings via a shared
//! `Param::bind` API. Reserved here so the v1.5 add is non-disruptive.

// M7 hooks: `ParamSet`, `Source::read`, `InputState` and its methods
// are stubs that MIDI / OSC sources will consume in T-M7-05 / T-M7-06.
#![allow(dead_code)]

pub mod keyboard;
pub mod param;

use crate::controls::param::SourceRef;

/// Operator-driven event coming from any registered [`Source`].
#[derive(Debug, Clone)]
pub enum ControlEvent {
    TapTempo,
    SceneRecall(usize),
    Blackout,
    Freeze,
    ParamSet { binding: SourceRef, value: f32 },
}

/// A pluggable input. v1 ships [`KeyboardSource`] (T-M4-09); v1.5
/// adds MIDI (T-M7-05) and OSC (T-M7-06) impls. Each is owned by
/// [`InputState`] and polled per frame.
pub trait Source {
    /// Drain any pending events since the last poll.
    fn poll(&mut self) -> Vec<ControlEvent>;

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
    pub fn poll(&mut self) -> Vec<ControlEvent> {
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
        events: Vec<ControlEvent>,
        read_value: Option<f32>,
    }

    impl Source for MockSource {
        fn poll(&mut self) -> Vec<ControlEvent> {
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
            events: vec![ControlEvent::TapTempo, ControlEvent::Blackout],
            read_value: Some(0.5),
        };
        state.register(Box::new(mock));

        // First poll drains the two seeded events.
        let events = state.poll();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], ControlEvent::TapTempo));
        assert!(matches!(events[1], ControlEvent::Blackout));

        // Second poll returns nothing (source drained).
        assert!(state.poll().is_empty());

        // read() returns the mock's value.
        assert_eq!(state.read(SourceRef(1)), Some(0.5));
    }
}
