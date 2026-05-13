//! P6.12.1 — Timecode sync decoders.
//!
//! Contains MTC (MIDI Timecode) quarter-frame decoder. LTC (Linear Timecode)
//! decoder is planned for a future release (requires `libltc` + `cmake`).

#[cfg(feature = "midi")]
pub mod mtc;
