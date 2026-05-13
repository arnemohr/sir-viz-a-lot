//! P6.12.1 — MIDI Timecode (MTC) quarter-frame decoder.
//!
//! MTC is sent as 8 consecutive quarter-frame messages (status byte 0xF1).
//! Each message carries a nibble pair: a 3-bit piece index (which timing
//! field the nibble belongs to) and a 4-bit value nibble.
//!
//! After 8 consecutive quarter-frame messages, a complete SMPTE frame
//! position (HH:MM:SS:FF) is assembled and stored in a shared
//! `Arc<Mutex<Option<TimecodePosition>>>` so the transport tick can read it.
//!
//! The decoder tolerates dropped or out-of-order messages by resetting on
//! any sequence gap — it waits for the next complete 8-message run.
//!
//! ## Feature gate
//!
//! MTC decoding is part of `--features midi`; no new feature gate is needed.

use std::sync::{Arc, Mutex};

use crate::project::schema::TimecodePosition;

/// P6.12.1 — Assembles MTC quarter-frame messages into `TimecodePosition`
/// frames and stores the result in a shared slot.
///
/// The MIDI callback in `midi.rs` calls [`MtcDecoder::push_quarter_frame`]
/// for each 0xF1 message; the transport tick reads the shared slot via
/// [`MtcDecoder::position`].
pub struct MtcDecoder {
    /// Shared position slot. `None` until the first complete frame is decoded.
    position: Arc<Mutex<Option<TimecodePosition>>>,
    /// Accumulated nibbles for the current 8-message run.
    nibbles: [u8; 8],
    /// How many nibbles have been received in the current run (0..=8).
    count: u8,
    /// Expected piece index of the next quarter-frame (0..=7).
    next_piece: u8,
}

impl MtcDecoder {
    /// Create a new decoder with an empty position slot.
    pub fn new() -> Self {
        MtcDecoder {
            position: Arc::new(Mutex::new(None)),
            nibbles: [0u8; 8],
            count: 0,
            next_piece: 0,
        }
    }

    /// Return a clone of the shared position slot for the transport tick to read.
    pub fn position(&self) -> Arc<Mutex<Option<TimecodePosition>>> {
        self.position.clone()
    }

    /// Feed one quarter-frame data byte (the byte AFTER the 0xF1 status byte).
    ///
    /// Layout per the MIDI spec:
    ///   bits 6-4 = piece index (0..=7)
    ///   bits 3-0 = value nibble
    ///
    /// On receiving piece 7 after pieces 0..6 in order, the full
    /// `TimecodePosition` is assembled and stored.
    pub fn push_quarter_frame(&mut self, data: u8) {
        let piece = (data >> 4) & 0x07;
        let nibble = data & 0x0F;

        // If this is not the expected piece, reset the run.
        if piece != self.next_piece {
            // Start fresh from this piece (piece 0 restarts the sequence).
            if piece == 0 {
                self.count = 0;
                self.next_piece = 0;
            } else {
                // Out-of-sequence piece — discard and wait for piece 0.
                self.count = 0;
                self.next_piece = 0;
                return;
            }
        }

        self.nibbles[piece as usize] = nibble;
        self.count += 1;
        self.next_piece = (piece + 1) & 0x07;

        // After all 8 pieces have arrived in order, assemble the position.
        if self.count == 8 {
            let pos = Self::assemble(&self.nibbles);
            if let Ok(mut guard) = self.position.lock() {
                *guard = Some(pos);
            }
            self.count = 0;
            self.next_piece = 0;
        }
    }

    /// Assemble a `TimecodePosition` from 8 accumulated nibbles.
    ///
    /// MTC nibble layout (per the MIDI spec):
    ///   nibble[0] = FF low  (bits 3-0 of frame count)
    ///   nibble[1] = FF high (bits 3-0 contain bits 4-3 of frames; 2 bits only)
    ///   nibble[2] = SS low  (bits 3-0 of seconds)
    ///   nibble[3] = SS high (bits 3-0 contain bits 4-3 of seconds; 2 bits only)
    ///   nibble[4] = MM low  (bits 3-0 of minutes)
    ///   nibble[5] = MM high (bits 3-0 contain bits 4-3 of minutes; 2 bits only)
    ///   nibble[6] = HH low  (bits 3-0 of hours)
    ///   nibble[7] = HH high (bits 1-0 contain bits 4-3 of hours; bit 2-3 = rate)
    fn assemble(n: &[u8; 8]) -> TimecodePosition {
        let ff = n[0] | ((n[1] & 0x01) << 4);
        let ss = n[2] | ((n[3] & 0x03) << 4);
        let mm = n[4] | ((n[5] & 0x03) << 4);
        let hh = n[6] | ((n[7] & 0x01) << 4);
        TimecodePosition {
            hh: hh.min(23),
            mm: mm.min(59),
            ss: ss.min(59),
            ff: ff.min(29),
        }
    }
}

impl Default for MtcDecoder {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the 8 quarter-frame data bytes for a given `TimecodePosition`.
    /// Piece order: 0=FF_lo, 1=FF_hi, 2=SS_lo, 3=SS_hi, 4=MM_lo, 5=MM_hi,
    /// 6=HH_lo, 7=HH_hi.
    fn encode_mtc(hh: u8, mm: u8, ss: u8, ff: u8) -> [u8; 8] {
        [
            ff & 0x0F,                 // piece 0: FF low nibble (piece index 0)
            0x10 | ((ff >> 4) & 0x01), // piece 1: FF high nibble (1 bit)
            0x20 | (ss & 0x0F),        // piece 2: SS low
            0x30 | ((ss >> 4) & 0x03), // piece 3: SS high (2 bits)
            0x40 | (mm & 0x0F),        // piece 4: MM low
            0x50 | ((mm >> 4) & 0x03), // piece 5: MM high (2 bits)
            0x60 | (hh & 0x0F),        // piece 6: HH low
            0x70 | ((hh >> 4) & 0x01), // piece 7: HH high (1 bit, no rate)
        ]
    }

    /// P6.12.1 acceptance: feed 8 synthetic quarter-frame bytes; verify
    /// decoded `TimecodePosition`.
    #[test]
    fn assembles_8_quarter_frames() {
        let mut decoder = MtcDecoder::new();
        let position_slot = decoder.position();

        // Encode 01:23:45:12 as 8 quarter-frame bytes.
        let bytes = encode_mtc(1, 23, 45, 12);
        for byte in bytes {
            decoder.push_quarter_frame(byte);
        }

        let pos = position_slot.lock().unwrap();
        let pos = pos.expect("position should be set after 8 frames");
        assert_eq!(pos.hh, 1, "hours");
        assert_eq!(pos.mm, 23, "minutes");
        assert_eq!(pos.ss, 45, "seconds");
        assert_eq!(pos.ff, 12, "frames");
    }

    /// Identity position (00:00:00:00) round-trips.
    #[test]
    fn zero_position_round_trips() {
        let mut decoder = MtcDecoder::new();
        let position_slot = decoder.position();
        for byte in encode_mtc(0, 0, 0, 0) {
            decoder.push_quarter_frame(byte);
        }
        let pos = position_slot.lock().unwrap().expect("position set");
        assert_eq!(pos.hh, 0);
        assert_eq!(pos.mm, 0);
        assert_eq!(pos.ss, 0);
        assert_eq!(pos.ff, 0);
    }

    /// Out-of-sequence piece resets the run; position is not updated.
    #[test]
    fn out_of_sequence_resets_run() {
        let mut decoder = MtcDecoder::new();
        let position_slot = decoder.position();

        // Send piece 3 first — out of order.
        decoder.push_quarter_frame(0x30); // piece 3, value 0
        // Position must remain None.
        assert!(position_slot.lock().unwrap().is_none());

        // Now send all 8 pieces in order starting from piece 0.
        for byte in encode_mtc(1, 0, 0, 5) {
            decoder.push_quarter_frame(byte);
        }
        // Position should now be set.
        assert!(position_slot.lock().unwrap().is_some());
    }

    /// Max plausible values (23:59:59:29) are clamped and survive.
    #[test]
    fn max_values_survive() {
        let mut decoder = MtcDecoder::new();
        let position_slot = decoder.position();
        for byte in encode_mtc(23, 59, 59, 29) {
            decoder.push_quarter_frame(byte);
        }
        let pos = position_slot.lock().unwrap().expect("position set");
        assert_eq!(pos.hh, 23);
        assert_eq!(pos.mm, 59);
        assert_eq!(pos.ss, 59);
        assert_eq!(pos.ff, 29);
    }
}
