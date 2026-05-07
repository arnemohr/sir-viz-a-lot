//! MIDI input via `midir`, gated on `feature = "midi"` (T-M7-05).
//!
//! Architecture: every available `MidiInput` port is opened on startup;
//! each `midir` callback decodes a single message into a [`ControlEvent`]
//! and pushes onto a bounded crossbeam channel. The [`MidiSource::poll`]
//! impl drains the channel each frame, alongside `KeyboardSource` and
//! the OSC source (T-M7-06).
//!
//! v1 mappings (Note On only — keep the surface tiny so a wedding rig
//! with a $30 USB MIDI pad works out of the box without configuration):
//!
//! - Note 60 (C4)         → `TapTempo`
//! - Notes 61..=69        → `SceneRecall(0..=8)` (one per scene slot)
//! - Note 70              → `Blackout`
//! - Note 71              → `Freeze`
//!
//! Other messages — Control Change, Pitch Bend, Program Change, etc. —
//! are silently dropped. T-M7-05 follow-up (or M7+) can extend the
//! decoder to emit `ParamSet { binding, value }` for CC; the channel
//! and Source plumbing are already in place.

use crossbeam_channel::{bounded, Receiver};
use midir::{MidiInput, MidiInputConnection};

use crate::controls::{ControlEvent, Source};

const QUEUE_DEPTH: usize = 256;

/// Source backed by zero or more live MIDI input subscriptions. Holds
/// the [`MidiInputConnection`]s so dropping the source unsubscribes
/// cleanly (each callback's closure references the channel sender by
/// clone; once the connections drop, the senders go too).
pub struct MidiSource {
    rx: Receiver<ControlEvent>,
    // Read at Drop only.
    #[allow(dead_code)]
    connections: Vec<MidiInputConnection<()>>,
}

impl MidiSource {
    /// Subscribe to every port `midir` reports at this moment in time.
    /// Returns Err only when `MidiInput::new` itself fails (bad backend);
    /// an empty port list is Ok with `connections.is_empty()`.
    pub fn start_all() -> anyhow::Result<Self> {
        let (tx, rx) = bounded::<ControlEvent>(QUEUE_DEPTH);
        let mut connections = Vec::new();

        // First pass to enumerate; each `connect` consumes its `MidiInput`,
        // so allocate one per port.
        let probe = MidiInput::new("rmap-midi-probe")?;
        let port_descriptors: Vec<_> = probe.ports().into_iter().collect();
        drop(probe);

        for port in port_descriptors {
            let midi = MidiInput::new("rmap")?;
            let port_name = midi.port_name(&port).unwrap_or_else(|_| "<unnamed>".into());
            let tx_for_callback = tx.clone();
            match midi.connect(
                &port,
                "rmap-input",
                move |_stamp_us, message, _state| {
                    if let Some(event) = decode(message) {
                        let _ = tx_for_callback.try_send(event);
                    }
                },
                (),
            ) {
                Ok(conn) => {
                    tracing::info!(port = %port_name, "midi input connected");
                    connections.push(conn);
                }
                Err(err) => {
                    tracing::warn!(port = %port_name, %err, "midi connect failed; skipping");
                }
            }
        }

        Ok(Self { rx, connections })
    }
}

/// Decode a raw MIDI message into a `ControlEvent`. Returns `None` for
/// every message that doesn't match the v1 mapping table — keeps the
/// channel free of noise that the dispatch loop would just discard.
fn decode(msg: &[u8]) -> Option<ControlEvent> {
    if msg.len() < 3 {
        return None;
    }
    // Status byte: 0x90..=0x9F is Note On on any channel.
    if (msg[0] & 0xF0) != 0x90 {
        return None;
    }
    // Note On with velocity 0 is the running-status idiom for Note Off.
    if msg[2] == 0 {
        return None;
    }
    match msg[1] {
        60 => Some(ControlEvent::TapTempo),
        n if (61..=69).contains(&n) => Some(ControlEvent::SceneRecall((n - 61) as usize)),
        70 => Some(ControlEvent::Blackout),
        71 => Some(ControlEvent::Freeze),
        _ => None,
    }
}

impl Source for MidiSource {
    fn poll(&mut self) -> Vec<ControlEvent> {
        let mut out = Vec::new();
        while let Ok(e) = self.rx.try_recv() {
            out.push(e);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_note_on_60_is_tap() {
        let msg = [0x90, 60, 100];
        assert!(matches!(decode(&msg), Some(ControlEvent::TapTempo)));
    }

    #[test]
    fn decode_scene_recall_offset() {
        let msg = [0x91, 64, 90]; // channel 2, note 64
        match decode(&msg) {
            Some(ControlEvent::SceneRecall(idx)) => assert_eq!(idx, 3),
            other => panic!("expected SceneRecall(3), got {other:?}"),
        }
    }

    #[test]
    fn decode_zero_velocity_is_note_off() {
        let msg = [0x90, 60, 0];
        assert!(decode(&msg).is_none());
    }

    #[test]
    fn decode_unmapped_note_is_none() {
        let msg = [0x90, 50, 100];
        assert!(decode(&msg).is_none());
    }

    #[test]
    fn decode_short_message_is_none() {
        let msg = [0x90, 60];
        assert!(decode(&msg).is_none());
    }
}
