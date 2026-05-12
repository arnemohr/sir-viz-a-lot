//! P5.2.2 — `DmxUniverse` newtype + `UniverseId` + `UniverseFrame`.
//!
//! `DmxUniverse` is a 512-byte DMX universe buffer. It is `Default`
//! (all zeros) and `Clone` so the lighting thread can cheaply copy
//! frames without heap allocation.
//!
//! `UniverseFrame` is the payload type for the crossbeam channel between
//! the render thread and the `LightingThread`. Keeping it in its own module
//! avoids circular imports between the thread and transport layers.

/// A 512-channel DMX universe data buffer.
///
/// Initialized to all-zero (all lights off). Accessed by channel offset
/// via [`DmxUniverse::channel_mut`].
#[derive(Debug, Clone)]
pub struct DmxUniverse(pub [u8; 512]);

impl Default for DmxUniverse {
    fn default() -> Self {
        Self([0u8; 512])
    }
}

impl DmxUniverse {
    /// Mutable reference to the byte at `offset` (0-indexed channel, 0–511).
    ///
    /// # Panics
    ///
    /// Panics if `offset >= 512`.
    pub fn channel_mut(&mut self, offset: usize) -> &mut u8 {
        &mut self.0[offset]
    }

    /// Immutable reference to the byte at `offset` (0-indexed channel, 0–511).
    ///
    /// # Panics
    ///
    /// Panics if `offset >= 512`.
    pub fn channel(&self, offset: usize) -> u8 {
        self.0[offset]
    }

    /// Return the raw 512-byte slice for use by `DmxTransport::send_universe`.
    pub fn as_bytes(&self) -> &[u8; 512] {
        &self.0
    }

    /// Return `true` if all 512 channels are zero.
    pub fn is_all_zero(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }
}

/// A 15-bit Art-Net universe index (0..=32_767).
///
/// Wrapping `u16` prevents confusion with raw byte values. Art-Net nodes
/// address fixtures by universe number + channel offset within that universe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct UniverseId(pub u16);

impl Default for UniverseId {
    fn default() -> Self {
        Self(1) // Art-Net convention: universe 0 discouraged; 1 is the default.
    }
}

impl UniverseId {
    /// Create a `UniverseId` from a raw `u16`, clamped to the valid Art-Net
    /// range (0..=32_767). Values above 32_767 are clamped to 32_767.
    pub fn from_u16(v: u16) -> Self {
        Self(v.min(32_767))
    }

    /// The raw universe number.
    pub fn as_u16(self) -> u16 {
        self.0
    }
}

/// The payload type sent from the render thread to the `LightingThread`
/// via the bounded crossbeam channel.
///
/// The render thread calls `tx.try_send(UniverseFrame { id, data })` once
/// per universe per frame. If the channel is full, `try_send` returns
/// `Err(Full)` and the frame is silently dropped — the lighting thread
/// has fallen behind by at most the channel capacity (4 frames by default)
/// and will catch up on the next tick.
#[derive(Debug, Clone)]
pub struct UniverseFrame {
    /// The Art-Net universe this data belongs to.
    pub id: UniverseId,
    /// The 512-byte DMX data for this universe.
    pub data: DmxUniverse,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P5.2.2 — default `DmxUniverse` is all zero.
    #[test]
    fn default_universe_is_all_zero() {
        let u = DmxUniverse::default();
        assert!(u.is_all_zero(), "default DmxUniverse must be all zeros");
        assert_eq!(u.0.len(), 512, "DmxUniverse must have exactly 512 channels");
    }

    /// P5.2.2 — `channel_mut` writes to the correct byte offset.
    #[test]
    fn channel_mutation_writes_correct_byte() {
        let mut u = DmxUniverse::default();
        *u.channel_mut(0) = 0xFF;
        *u.channel_mut(100) = 0x42;
        *u.channel_mut(511) = 0x01;

        assert_eq!(u.0[0], 0xFF, "offset 0 mismatch");
        assert_eq!(u.0[100], 0x42, "offset 100 mismatch");
        assert_eq!(u.0[511], 0x01, "offset 511 mismatch");
        // Other bytes remain zero.
        assert_eq!(u.0[1], 0, "offset 1 should still be zero");
    }

    /// P5.2.2 — `channel` (read) returns the correct value.
    #[test]
    fn channel_read_returns_correct_value() {
        let mut u = DmxUniverse::default();
        *u.channel_mut(10) = 77;
        assert_eq!(u.channel(10), 77);
    }

    /// P5.2.2 — `is_all_zero` returns false after any write.
    #[test]
    fn is_all_zero_false_after_write() {
        let mut u = DmxUniverse::default();
        assert!(u.is_all_zero());
        *u.channel_mut(0) = 1;
        assert!(!u.is_all_zero());
    }

    /// P5.2.2 — `UniverseId::from_u16` clamps at 32_767.
    #[test]
    fn universe_id_clamps_at_max() {
        assert_eq!(UniverseId::from_u16(0).as_u16(), 0);
        assert_eq!(UniverseId::from_u16(32_767).as_u16(), 32_767);
        assert_eq!(UniverseId::from_u16(32_768).as_u16(), 32_767);
        assert_eq!(UniverseId::from_u16(u16::MAX).as_u16(), 32_767);
    }

    /// P5.2.2 — `UniverseFrame` carries id and data as expected.
    #[test]
    fn universe_frame_carries_data() {
        let mut data = DmxUniverse::default();
        *data.channel_mut(5) = 200;
        let frame = UniverseFrame {
            id: UniverseId(3),
            data: data.clone(),
        };
        assert_eq!(frame.id.as_u16(), 3);
        assert_eq!(frame.data.channel(5), 200);
    }
}
