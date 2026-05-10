//! MIDI input via `midir`, gated on `feature = "midi"` (T-M7-05).
//!
//! Architecture: every available `MidiInput` port is opened on startup;
//! each `midir` callback decodes a single message into a [`Command`]
//! and pushes onto a bounded crossbeam channel. The [`MidiSource::poll`]
//! impl drains the channel each frame, alongside `KeyboardSource` and
//! the OSC source (T-M7-06).
//!
//! v1 mappings (Note On only — keep the surface tiny so a event rig
//! with a $30 USB MIDI pad works out of the box without configuration):
//!
//! - Note 60 (C4)         → `TapTempo`
//! - Notes 61..=69        → `SceneRecall(0..=8)` (one per scene slot)
//! - Note 70              → `Blackout`
//! - Note 71              → `Freeze`
//!
//! Other messages — Control Change, Pitch Bend, Program Change, etc. —
//! are silently dropped. v0.4 W2.2 extends this decoder to maintain a
//! process-wide CC value registry (analogous to `audio::PROVIDER`)
//! that the new `Modulator::MidiBound { cc, channel }` resolves
//! against.

use std::sync::{Arc, RwLock};

use crossbeam_channel::{Receiver, bounded};
use midir::{MidiInput, MidiInputConnection};

use crate::clock::TapSource;
use crate::controls::{Command, Source};
use crate::modulators::midi::{MidiProvider, NUM_CCS, NUM_CHANNELS};

const QUEUE_DEPTH: usize = 256;

/// Live CC-value table backing `Modulator::MidiBound`. The decoder
/// writes one float per `(channel, cc)` on every Control-Change
/// message; the resolve path reads through
/// `crate::modulators::midi::current_value`, which delegates to
/// the installed provider's `cc()` method.
///
/// Size is fixed (16 channels × 128 CCs × 4 bytes = 8 KB), zero
/// allocation in the hot path.
pub struct CcRegistry {
    values: Arc<RwLock<[[f32; NUM_CCS]; NUM_CHANNELS]>>,
}

impl CcRegistry {
    fn new() -> Self {
        Self {
            values: Arc::new(RwLock::new([[0.0; NUM_CCS]; NUM_CHANNELS])),
        }
    }

    fn write(&self, channel: u8, cc: u8, value: f32) {
        if let Ok(mut g) = self.values.write() {
            let ch = (channel as usize).min(NUM_CHANNELS - 1);
            let i = (cc as usize).min(NUM_CCS - 1);
            g[ch][i] = value;
        }
    }
}

impl MidiProvider for CcRegistry {
    fn cc(&self, channel: u8, cc: u8) -> f32 {
        let ch = (channel as usize).min(NUM_CHANNELS - 1);
        let i = (cc as usize).min(NUM_CCS - 1);
        self.values.read().map(|g| g[ch][i]).unwrap_or(0.0)
    }
}

/// Source backed by zero or more live MIDI input subscriptions. Holds
/// the [`MidiInputConnection`]s so dropping the source unsubscribes
/// cleanly (each callback's closure references the channel sender by
/// clone; once the connections drop, the senders go too).
pub struct MidiSource {
    rx: Receiver<Command>,
    // Read at Drop only.
    #[allow(dead_code)]
    connections: Vec<MidiInputConnection<()>>,
}

impl MidiSource {
    /// Subscribe to every port `midir` reports at this moment in time.
    /// Returns Err only when `MidiInput::new` itself fails (bad backend);
    /// an empty port list is Ok with `connections.is_empty()`.
    pub fn start_all() -> anyhow::Result<Self> {
        let (tx, rx) = bounded::<Command>(QUEUE_DEPTH);
        let mut connections = Vec::new();

        // P0.2.2 follow-up — install the CC value registry as the
        // process-wide MidiProvider. Subsequent `Modulator::MidiBound`
        // resolves read live values from here. The registry's Arc is
        // shared with each port callback so writes are visible to
        // the resolve path immediately.
        let registry = CcRegistry::new();
        let registry_arc: Arc<RwLock<[[f32; NUM_CCS]; NUM_CHANNELS]>> = registry.values.clone();
        crate::modulators::midi::install(Arc::new(registry));

        // First pass to enumerate; each `connect` consumes its `MidiInput`,
        // so allocate one per port.
        let probe = MidiInput::new("rmap-midi-probe")?;
        let port_descriptors: Vec<_> = probe.ports().into_iter().collect();
        drop(probe);

        for port in port_descriptors {
            let midi = MidiInput::new("rmap")?;
            let port_name = midi.port_name(&port).unwrap_or_else(|_| "<unnamed>".into());
            let tx_for_callback = tx.clone();
            let registry_for_callback = registry_arc.clone();
            match midi.connect(
                &port,
                "rmap-input",
                move |_stamp_us, message, _state| {
                    // Note-On commands take precedence over the CC
                    // value-table write (a single message can't be
                    // both, but the early-return keeps the hot path
                    // tight).
                    if let Some(event) = decode(message) {
                        let _ = tx_for_callback.try_send(event);
                        return;
                    }
                    // CC: status nibble 0xB. Status byte's low nibble
                    // is the channel (0..=15).
                    if message.len() >= 3 && (message[0] & 0xF0) == 0xB0 {
                        let channel = message[0] & 0x0F;
                        let cc = message[1].min(127);
                        let value = (message[2] & 0x7F) as f32 / 127.0;
                        if let Ok(mut g) = registry_for_callback.write() {
                            let ch = channel as usize;
                            let i = cc as usize;
                            g[ch][i] = value;
                        }
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

/// Decode a raw MIDI message into a `Command`. Returns `None` for
/// every message that doesn't match the v1 mapping table — keeps the
/// channel free of noise that the dispatch loop would just discard.
fn decode(msg: &[u8]) -> Option<Command> {
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
        60 => Some(Command::TapTempo(TapSource::Midi)),
        n if (61..=69).contains(&n) => Some(Command::SceneRecall((n - 61) as usize)),
        70 => Some(Command::Blackout),
        71 => Some(Command::Freeze),
        _ => None,
    }
}

impl Source for MidiSource {
    fn poll(&mut self) -> Vec<Command> {
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
        assert!(matches!(
            decode(&msg),
            Some(Command::TapTempo(TapSource::Midi))
        ));
    }

    #[test]
    fn decode_scene_recall_offset() {
        let msg = [0x91, 64, 90]; // channel 2, note 64
        match decode(&msg) {
            Some(Command::SceneRecall(idx)) => assert_eq!(idx, 3),
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

    /// P0.2.2 follow-up — `decode()` ignores Control-Change messages
    /// (they're handled by the CC registry write in the callback,
    /// not the Command path).
    #[test]
    fn decode_cc_returns_none() {
        let msg = [0xB0, 21, 64];
        assert!(decode(&msg).is_none());
    }

    /// P0.2.2 follow-up — the CC registry round-trips a write through
    /// the `MidiProvider::cc` getter.
    #[test]
    fn cc_registry_round_trip() {
        let reg = CcRegistry::new();
        reg.write(0, 21, 0.5);
        reg.write(15, 127, 1.0);
        // MidiProvider::cc reads through the same Arc.
        assert!((reg.cc(0, 21) - 0.5).abs() < 1e-6);
        assert!((reg.cc(15, 127) - 1.0).abs() < 1e-6);
        assert_eq!(reg.cc(3, 50), 0.0);
    }

    /// Out-of-range channel / CC clamps in the read path (the write
    /// path also clamps; both must agree).
    #[test]
    fn cc_registry_clamps_out_of_range() {
        let reg = CcRegistry::new();
        reg.write(255, 255, 0.7);
        // Stored at the clamped index (15, 127).
        assert!((reg.cc(15, 127) - 0.7).abs() < 1e-6);
        assert!((reg.cc(255, 255) - 0.7).abs() < 1e-6);
    }
}
