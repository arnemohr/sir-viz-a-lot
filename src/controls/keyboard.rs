//! `KeyboardSource` — buffers winit keyboard events and translates them
//! into [`Command`]s on `poll()`. T-M4-09.
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

use crate::clock::TapSource;
use crate::controls::{Command, Source};

#[derive(Default)]
pub struct KeyboardSource {
    pending: VecDeque<Command>,
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
            // Space → TapTempo (always); CueGo is dispatched from apply_command
            // when a cue is armed (Space has dual role: tap tempo + cue go).
            PhysicalKey::Code(KeyCode::Space) => Some(Command::TapTempo(TapSource::Keyboard)),
            PhysicalKey::Code(KeyCode::KeyB) => Some(Command::Blackout),
            PhysicalKey::Code(KeyCode::KeyF) => Some(Command::Freeze),
            PhysicalKey::Code(KeyCode::Digit1) => Some(Command::SceneRecall(0)),
            PhysicalKey::Code(KeyCode::Digit2) => Some(Command::SceneRecall(1)),
            PhysicalKey::Code(KeyCode::Digit3) => Some(Command::SceneRecall(2)),
            PhysicalKey::Code(KeyCode::Digit4) => Some(Command::SceneRecall(3)),
            PhysicalKey::Code(KeyCode::Digit5) => Some(Command::SceneRecall(4)),
            PhysicalKey::Code(KeyCode::Digit6) => Some(Command::SceneRecall(5)),
            PhysicalKey::Code(KeyCode::Digit7) => Some(Command::SceneRecall(6)),
            PhysicalKey::Code(KeyCode::Digit8) => Some(Command::SceneRecall(7)),
            PhysicalKey::Code(KeyCode::Digit9) => Some(Command::SceneRecall(8)),
            // P6.4.2 — cue navigation.
            // CueGo: dispatched from the TapTempo handler in apply_command
            // when a cue is armed (dual role). Dedicated CueGo variant emitted
            // here is an alternative; the apply_command arm for TapTempo already
            // checks transport.armed_cue and converts to CueGo when armed.
            #[cfg(feature = "v3")]
            PhysicalKey::Code(KeyCode::ArrowRight) => Some(Command::CueArmNext),
            #[cfg(feature = "v3")]
            PhysicalKey::Code(KeyCode::ArrowLeft) => Some(Command::CueArmPrev),
            #[cfg(feature = "v3")]
            PhysicalKey::Code(KeyCode::Backspace) => Some(Command::CueBackStep),
            _ => None,
        };
        if let Some(e) = event {
            self.pending.push_back(e);
        }
    }
}

impl Source for KeyboardSource {
    fn poll(&mut self) -> Vec<Command> {
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
        assert!(matches!(events[0], Command::TapTempo(TapSource::Keyboard)));
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
        assert!(matches!(events[0], Command::Blackout));
        assert!(matches!(events[1], Command::Freeze));
    }

    #[test]
    fn digits_emit_scene_recall_zero_indexed() {
        let mut src = KeyboardSource::new();
        for code in [KeyCode::Digit1, KeyCode::Digit5, KeyCode::Digit9] {
            src.push_winit_key(PhysicalKey::Code(code));
        }
        let events = src.poll();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], Command::SceneRecall(0)));
        assert!(matches!(events[1], Command::SceneRecall(4)));
        assert!(matches!(events[2], Command::SceneRecall(8)));
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
