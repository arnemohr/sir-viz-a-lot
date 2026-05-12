//! P5.2.1 — `DmxTransport` trait + `ArtNetTransport` + `NullTransport`.
//!
//! The `DmxTransport` trait is the only abstraction the lighting thread
//! touches; Phase 7 can add `SacnTransport` by implementing the same trait.
//!
//! The `ArtNetTransport` implementation holds a `UdpSocket` bound to
//! `0.0.0.0:0` (ephemeral local port) and sends Art-Net `ArtDmx` PDUs
//! to a configurable destination. It maintains a per-universe sequence
//! counter to allow receiving nodes to detect out-of-order packets.
//!
//! `NullTransport` is a no-op used in unit / integration tests.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};

use artnet_protocol::{ArtCommand, Output, PortAddress};

use crate::lighting::error::LightingError;

/// Abstraction over DMX wire protocols.
///
/// All implementations must be `Send + 'static` so they can be moved onto
/// the `LightingThread` background thread. The render thread never calls
/// `send_universe` — it only feeds frames into the bounded crossbeam channel.
///
/// Phase 5 implements `ArtNetTransport`. Phase 7 will add `SacnTransport`
/// as a second impl without changing this trait or the fixture model.
pub trait DmxTransport: Send + 'static {
    /// Send one 512-byte universe to the configured destination.
    ///
    /// Called from the lighting thread at ~44 Hz. Must not block the
    /// render thread. On transient I/O errors the implementation should
    /// log a warning and return `Err`; the caller increments a dropped-
    /// packet counter and continues.
    fn send_universe(&mut self, universe: u16, data: &[u8; 512]) -> Result<(), LightingError>;
}

/// Art-Net UDP transport.
///
/// Holds a UDP socket bound to `0.0.0.0:0` (OS assigns the local port)
/// and sends Art-Net `Output` (`ArtDmx`) PDUs to `dest` (typically
/// `255.255.255.255:6454` for subnet broadcast, or a directed unicast
/// address for a specific node).
///
/// Sequence numbers are maintained per universe in the range `0x01..=0xff`
/// so receiving nodes can detect and discard out-of-order packets.
pub struct ArtNetTransport {
    socket: UdpSocket,
    dest: SocketAddr,
    /// Per-universe sequence counter. `0x00` disables sequencing per the
    /// Art-Net spec; we use `0x01..=0xff` so the first packet has sequence 1.
    sequences: HashMap<u16, u8>,
}

impl ArtNetTransport {
    /// Bind a UDP socket on `0.0.0.0:0` and configure it for sending to
    /// `dest`. Enables `SO_BROADCAST` so broadcast addresses work without
    /// the caller needing to set the option separately.
    pub fn new(dest: SocketAddr) -> Result<Self, LightingError> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_broadcast(true)?;
        Ok(Self {
            socket,
            dest,
            sequences: HashMap::new(),
        })
    }

    /// Advance and return the next sequence number for `universe`,
    /// wrapping from 0xff back to 0x01 (0x00 is the "disabled" sentinel).
    fn next_sequence(&mut self, universe: u16) -> u8 {
        let seq = self.sequences.entry(universe).or_insert(0);
        *seq = seq.wrapping_add(1).max(1); // skip 0x00
        *seq
    }
}

impl DmxTransport for ArtNetTransport {
    fn send_universe(&mut self, universe: u16, data: &[u8; 512]) -> Result<(), LightingError> {
        let sequence = self.next_sequence(universe);

        // Art-Net PortAddress is 15-bit; Art-Net universe numbers 0..=32_767
        // map 1:1 to PortAddress via `TryFrom<u16>`.
        let port_address = PortAddress::try_from(universe)
            .map_err(|_| LightingError::InvalidUniverse(universe))?;

        let command = ArtCommand::Output(Output {
            sequence,
            port_address,
            data: data.to_vec().into(),
            ..Output::default()
        });

        let bytes = command
            .write_to_buffer()
            .map_err(|e| LightingError::Encode(e.to_string()))?;

        self.socket
            .send_to(&bytes, self.dest)
            .map_err(LightingError::Io)?;

        Ok(())
    }
}

/// No-op transport for unit and integration tests.
///
/// `send_universe` is a silent no-op. Tests that need to capture sent
/// frames should use a loopback `UdpSocket` instead of this type; this
/// type is for tests that just want a compiling `DmxTransport` without
/// any network traffic.
pub struct NullTransport;

impl DmxTransport for NullTransport {
    fn send_universe(&mut self, _universe: u16, _data: &[u8; 512]) -> Result<(), LightingError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::net::UdpSocket;

    use artnet_protocol::ArtCommand;

    use super::*;
    use crate::lighting::universe::DmxUniverse;

    /// P5.2.1 — send a universe frame over loopback and decode the received
    /// Art-Net packet back, verifying the opcode and payload.
    #[test]
    fn artnet_transport_loopback_roundtrip() {
        // Bind a listener on an ephemeral port to act as the Art-Net "node".
        let listener = UdpSocket::bind("127.0.0.1:0").expect("bind listener");
        listener
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let listener_addr = listener.local_addr().unwrap();

        let mut transport = ArtNetTransport::new(listener_addr).expect("ArtNetTransport::new");

        // Build a universe frame with known data (channel 0 = 0xAA, 1 = 0xBB).
        let mut data = DmxUniverse::default();
        *data.channel_mut(0_usize) = 0xAA;
        *data.channel_mut(1_usize) = 0xBB;

        transport
            .send_universe(1, data.as_bytes())
            .expect("send_universe");

        // Receive and decode the packet.
        let mut buf = [0u8; 1024];
        let (len, _) = listener.recv_from(&mut buf).expect("recv_from");
        let command = ArtCommand::from_buffer(&buf[..len]).expect("ArtCommand::from_buffer");

        match command {
            ArtCommand::Output(output) => {
                let received = output.data.as_ref();
                assert_eq!(received[0], 0xAA, "channel 0 mismatch");
                assert_eq!(received[1], 0xBB, "channel 1 mismatch");
                // Universe 1 → PortAddress(1).
                assert_eq!(u16::from(output.port_address), 1u16);
            }
            other => panic!("expected ArtCommand::Output, got {other:?}"),
        }
    }

    /// P5.2.1 — `NullTransport` sends silently without error.
    #[test]
    fn null_transport_is_silent() {
        let mut transport = NullTransport;
        let data = DmxUniverse::default();
        let result = transport.send_universe(0, data.as_bytes());
        assert!(result.is_ok(), "NullTransport must not return error");
    }

    /// P5.2.1 — sequence numbers increment per universe, skip 0x00, wrap
    /// at 0xff back to 0x01.
    #[test]
    fn sequence_numbers_increment_and_wrap() {
        let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
        let dest = listener.local_addr().unwrap();
        let mut transport = ArtNetTransport::new(dest).unwrap();

        // First call → sequence 1.
        assert_eq!(transport.next_sequence(1), 1);
        // Second call → sequence 2.
        assert_eq!(transport.next_sequence(1), 2);

        // Force wrap: set to 0xff directly.
        *transport.sequences.get_mut(&1).unwrap() = 0xff;
        // Next call should wrap to 1 (skip 0x00).
        assert_eq!(transport.next_sequence(1), 1);

        // Different universe has its own independent counter.
        assert_eq!(transport.next_sequence(2), 1);
    }
}
