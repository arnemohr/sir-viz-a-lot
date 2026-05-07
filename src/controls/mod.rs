//! Live input. v1 in-scope: tap-tempo (Space) + scene-recall hotkeys (1–9).
//! v1.5: MIDI (`midir`) and OSC (`rosc`) bindings via a shared
//! `Param::bind` API. Reserved here so the v1.5 add is non-disruptive.

#[derive(Debug, Default)]
pub struct InputState {
    pub current_scene: Option<usize>,
}

// TODO(v1.5):
// trait Source { fn next_event(&mut self) -> Option<ControlEvent>; }
// impl Source for KeyboardSource (now), MidiSource, OscSource (later).
