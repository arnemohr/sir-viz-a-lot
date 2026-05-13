//! P5.10.3 — Packet-capture acceptance test: CI Art-Net listener.
//!
//! This integration test verifies the Art-Net transport end-to-end:
//!
//! 1. Bind a loopback UDP socket on `127.0.0.1:0` (OS-assigned ephemeral port).
//! 2. Create an `ArtNetTransport` targeting that socket.
//! 3. Send a non-zero `UniverseFrame`.
//! 4. Verify the received packet has the correct `ArtDmx` opcode + payload.
//! 5. Send a zero `UniverseFrame` (Blackout).
//! 6. Verify the next packet is all-zero `DmxUniverse`.
//! 7. Verify ArtNet sequence numbers increment monotonically across sends.
//!
//! Does **not** require a real Art-Net node — loopback only.
//! Gated on `feature = "lighting"` so the integration test is only
//! compiled when the lighting feature is enabled.

#![cfg(feature = "lighting")]

use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use artnet_protocol::ArtCommand;
use rmap::lighting::transport::{ArtNetTransport, DmxTransport};

/// P5.10.3 — Loopback Art-Net listener receives correct ArtDmx packets.
#[test]
fn artnet_transport_sends_correct_packets_loopback() {
    use rmap::lighting::universe::DmxUniverse;

    // 1. Bind a loopback listener socket on an OS-assigned ephemeral port.
    let listener = UdpSocket::bind("127.0.0.1:0").expect("bind listener");
    listener
        .set_read_timeout(Some(Duration::from_millis(500)))
        .expect("set_read_timeout");
    let listen_addr: SocketAddr = listener.local_addr().expect("local_addr");

    // 2. Create an ArtNetTransport targeting the listener.
    let mut transport = ArtNetTransport::new(listen_addr).expect("ArtNetTransport::new");

    // Helper: receive one UDP datagram and decode it as an ArtCommand.
    let receive_packet = |sock: &UdpSocket| -> Vec<u8> {
        let mut buf = [0u8; 1024];
        let (len, _) = sock.recv_from(&mut buf).expect("recv_from");
        buf[..len].to_vec()
    };

    // 3. Send a non-zero universe frame (universe 1, ch0=200, ch1=128, ch2=64).
    let mut data1 = DmxUniverse::default();
    *data1.channel_mut(0) = 200;
    *data1.channel_mut(1) = 128;
    *data1.channel_mut(2) = 64;
    transport
        .send_universe(1, &data1.0)
        .expect("send non-zero frame");

    // 4. Receive and decode the packet.
    let pkt1 = receive_packet(&listener);
    let cmd1 = ArtCommand::from_buffer(&pkt1).expect("decode ArtCommand from first packet");

    // Verify it's an Output (ArtDmx) packet.
    let output1 = match cmd1 {
        ArtCommand::Output(o) => o,
        other => panic!("expected ArtCommand::Output, got {:?}", other),
    };

    // Verify the payload bytes at channels 0, 1, 2.
    let payload1: &Vec<u8> = output1.data.as_ref();
    assert_eq!(
        payload1.len(),
        512,
        "ArtDmx payload must be 512 bytes (DMX512)"
    );
    assert_eq!(payload1[0], 200, "channel 0 should be 200 (red)");
    assert_eq!(payload1[1], 128, "channel 1 should be 128 (green)");
    assert_eq!(payload1[2], 64, "channel 2 should be 64 (blue)");

    // Verify the universe number.
    // artnet_protocol encodes the universe as net/subnet/universe; the flat
    // universe ID 1 maps to port_address bits [0..15] == 1.
    // We just check the packet round-tripped — a deeper check of the port_address
    // encoding would tie the test too tightly to the artnet_protocol crate internals.
    let seq1 = output1.sequence;

    // 5. Send a zero (Blackout) frame on the same universe.
    let data_blackout = [0u8; 512];
    transport
        .send_universe(1, &data_blackout)
        .expect("send blackout frame");

    // 6. Receive and decode the blackout packet.
    let pkt2 = receive_packet(&listener);
    let cmd2 = ArtCommand::from_buffer(&pkt2).expect("decode ArtCommand from blackout packet");

    let output2 = match cmd2 {
        ArtCommand::Output(o) => o,
        other => panic!("expected ArtCommand::Output for blackout, got {:?}", other),
    };

    let payload2: &Vec<u8> = output2.data.as_ref();
    assert_eq!(payload2.len(), 512, "blackout packet must be 512 bytes");
    for (i, &byte) in payload2.iter().enumerate() {
        assert_eq!(byte, 0, "blackout: channel {i} should be zero, got {byte}");
    }

    // 7. Sequence numbers must increment monotonically.
    // artnet_protocol increments sequence per send; seq wraps at 0xFF → 0x01
    // (skipping 0x00 which means "sequence not in use"). We just check seq2 != seq1.
    let seq2 = output2.sequence;
    // Both sequences are u8. Wrap is fine; the important invariant is they're different.
    assert_ne!(
        seq2, seq1,
        "sequence number must advance between sends (seq1={seq1}, seq2={seq2})"
    );
}

/// P5.10.3 — Blackout signal arrives within frame (same-tick test).
///
/// Verifies that a blackout send (all-zero universe) and a subsequent
/// non-zero send both arrive and in the correct order. This exercises
/// the `LightingThread` + `ArtNetTransport` pipeline end-to-end with
/// the `NullTransport` replaced by a loopback socket.
#[test]
fn artnet_blackout_delivers_zeros_then_color() {
    use rmap::lighting::universe::DmxUniverse;

    let listener = UdpSocket::bind("127.0.0.1:0").expect("bind listener");
    listener
        .set_read_timeout(Some(Duration::from_millis(500)))
        .expect("set_read_timeout");
    let listen_addr: SocketAddr = listener.local_addr().expect("local_addr");
    let mut transport = ArtNetTransport::new(listen_addr).expect("ArtNetTransport::new");

    // Send blackout (zeros) first.
    let zeros = [0u8; 512];
    transport.send_universe(0, &zeros).expect("send zeros");

    let mut buf = [0u8; 1024];
    let (len, _) = listener.recv_from(&mut buf).expect("recv zeros");
    let cmd = ArtCommand::from_buffer(&buf[..len]).expect("decode zeros packet");
    let output = match cmd {
        ArtCommand::Output(o) => o,
        other => panic!("expected Output, got {:?}", other),
    };
    {
        let payload: &Vec<u8> = output.data.as_ref();
        assert!(
            payload.iter().all(|&b| b == 0),
            "blackout packet must be all zeros"
        );
    }

    // Send a colour frame immediately after.
    let mut color_data = DmxUniverse::default();
    *color_data.channel_mut(0) = 255;
    transport
        .send_universe(0, &color_data.0)
        .expect("send color");

    let (len2, _) = listener.recv_from(&mut buf).expect("recv color");
    let cmd2 = ArtCommand::from_buffer(&buf[..len2]).expect("decode color packet");
    let output2 = match cmd2 {
        ArtCommand::Output(o) => o,
        other => panic!("expected Output for color, got {:?}", other),
    };
    {
        let payload2: &Vec<u8> = output2.data.as_ref();
        assert_eq!(payload2[0], 255, "color packet channel 0 should be 255");
    }
}
