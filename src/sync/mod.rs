//! P6.12.1 / P6.12.2 — Timecode sync decoders.
//!
//! - MTC (MIDI Timecode) quarter-frame decoder (P6.12.1).
//! - MIDI-clock BPM tracker (P6.12.2).
//! - LTC (Linear Timecode) decoder planned for a future release
//!   (requires `libltc` + `cmake`).

#[cfg(feature = "midi")]
pub mod midi_clock;
#[cfg(feature = "midi")]
pub mod mtc;
