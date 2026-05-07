//! `KeyboardSource` — buffers winit keyboard events and translates them
//! into [`ControlEvent`]s on `poll()`. T-M4-09.
//!
//! Wired by App::window_event: each `WindowEvent::KeyboardInput` pressed
//! event is forwarded to `push_winit_key(physical_key)`. The buffered
//! events drain on the next `InputState::poll()`.
//!
//! Mapping:
//!   Space  -> TapTempo
//!   1..9   -> SceneRecall(0..8)
//!   B      -> Blackout
//!   F      -> Freeze
//!
//! T-cycle test-pattern (M2) stays in the App's inline handling for now;
//! this source doesn't emit a TestPattern event. T-M4-15 (control panel)
//! may unify the input path later.

use std::collections::VecDeque;

use winit::keyboard::{KeyCode, PhysicalKey};

use crate::controls::{ControlEvent, Source};

#[derive(Default)]
pub struct KeyboardSource {
    pending: VecDeque<ControlEvent>,
}

impl KeyboardSource {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forward a pressed-key event from the winit event loop. Repeated
    /// presses (held key) are accepted — the `InputState::poll()`
    /// consumer can dedupe if it wants.
    pub fn push_winit_key(&mut self, physical_key: PhysicalKey) {
        let event = match physical_key {
            PhysicalKey::Code(KeyCode::Space) => Some(ControlEvent::TapTempo),
            PhysicalKey::Code(KeyCode::KeyB) => Some(ControlEvent::Blackout),
            PhysicalKey::Code(KeyCode::KeyF) => Some(ControlEvent::Freeze),
            PhysicalKey::Code(KeyCode::Digit1) => Some(ControlEvent::SceneRecall(0)),
            PhysicalKey::Code(KeyCode::Digit2) => Some(ControlEvent::SceneRecall(1)),
            PhysicalKey::Code(KeyCode::Digit3) => Some(ControlEvent::SceneRecall(2)),
            PhysicalKey::Code(KeyCode::Digit4) => Some(ControlEvent::SceneRecall(3)),
            PhysicalKey::Code(KeyCode::Digit5) => Some(ControlEvent::SceneRecall(4)),
            PhysicalKey::Code(KeyCode::Digit6) => Some(ControlEvent::SceneRecall(5)),
            PhysicalKey::Code(KeyCode::Digit7) => Some(ControlEvent::SceneRecall(6)),
            PhysicalKey::Code(KeyCode::Digit8) => Some(ControlEvent::SceneRecall(7)),
            PhysicalKey::Code(KeyCode::Digit9) => Some(ControlEvent::SceneRecall(8)),
            _ => None,
        };
        if let Some(e) = event {
            self.pending.push_back(e);
        }
    }
}

impl Source for KeyboardSource {
    fn poll(&mut self) -> Vec<ControlEvent> {
        self.pending.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_emits_tap() {
        let mut src = KeyboardSource::new();
        src.push_winit_key(PhysicalKey::Code(KeyCode::Space));
        let events = src.poll();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ControlEvent::TapTempo));
        // Second poll empty.
        assert!(src.poll().is_empty());
    }

    #[test]
    fn b_and_f_emit_blackout_and_freeze() {
        let mut src = KeyboardSource::new();
        src.push_winit_key(PhysicalKey::Code(KeyCode::KeyB));
        src.push_winit_key(PhysicalKey::Code(KeyCode::KeyF));
        let events = src.poll();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], ControlEvent::Blackout));
        assert!(matches!(events[1], ControlEvent::Freeze));
    }

    #[test]
    fn digits_emit_scene_recall_zero_indexed() {
        let mut src = KeyboardSource::new();
        for code in [KeyCode::Digit1, KeyCode::Digit5, KeyCode::Digit9] {
            src.push_winit_key(PhysicalKey::Code(code));
        }
        let events = src.poll();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], ControlEvent::SceneRecall(0)));
        assert!(matches!(events[1], ControlEvent::SceneRecall(4)));
        assert!(matches!(events[2], ControlEvent::SceneRecall(8)));
    }

    #[test]
    fn unmapped_keys_ignored() {
        let mut src = KeyboardSource::new();
        src.push_winit_key(PhysicalKey::Code(KeyCode::KeyT)); // T is M2 inline
        src.push_winit_key(PhysicalKey::Code(KeyCode::Escape));
        src.push_winit_key(PhysicalKey::Code(KeyCode::KeyA));
        assert!(src.poll().is_empty());
    }
}
