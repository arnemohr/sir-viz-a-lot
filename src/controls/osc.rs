//! OSC input via `rosc`, gated on `feature = "osc"` (T-M7-06).
//!
//! Architecture: a UDP socket bound to a configurable port runs in a
//! background thread that receives datagrams, decodes via `rosc`, and
//! pushes [`Command`]s into a bounded crossbeam channel. The
//! [`OscSource::poll`] impl drains the channel each frame.
//!
//! v1 address mappings (case-insensitive):
//!
//! - `/rmap/tap`              → `TapTempo` (any args)
//! - `/rmap/scene/N`          → `SceneRecall(N - 1)` for N in 1..=9
//! - `/rmap/blackout`         → `Blackout`
//! - `/rmap/freeze`           → `Freeze`
//!
//! Unknown addresses are dropped silently. T-M7-06 follow-up can extend
//! decode to emit `ParamSet { binding, value }` for `/rmap/param/...`
//! addresses with a numeric arg.

use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crossbeam_channel::{Receiver, bounded};
use rosc::{OscMessage, OscPacket};

use crate::clock::TapSource;
use crate::controls::{Command, Source};

/// Default UDP listen port for OSC. Operators expecting a different
/// port pass it to [`OscSource::start`].
pub const DEFAULT_PORT: u16 = 8765;

const QUEUE_DEPTH: usize = 256;
/// Max datagram size we accept. Generous for OSC bundles; prevents the
/// receive loop allocating unbounded buffers.
const RECV_BUF: usize = 4096;

/// Source backed by a background UDP listener. Holds a stop flag the
/// listener checks between blocking reads (via socket read timeouts) so
/// dropping the source winds the thread down cleanly.
pub struct OscSource {
    rx: Receiver<Command>,
    stop: Arc<AtomicBool>,
    // Hold the join handle so the thread is reaped on drop.
    #[allow(dead_code)]
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for OscSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl OscSource {
    /// Bind UDP on `0.0.0.0:port` (defaulting to [`DEFAULT_PORT`] when
    /// `port` is 0) and start the receive thread. Errors propagate so
    /// app startup can warn-and-continue if the port is unavailable.
    pub fn start(port: u16) -> anyhow::Result<Self> {
        let port = if port == 0 { DEFAULT_PORT } else { port };
        let socket = UdpSocket::bind(("0.0.0.0", port))?;
        socket.set_read_timeout(Some(std::time::Duration::from_millis(250)))?;
        let local = socket
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_default();
        tracing::info!(local = %local, "osc listening");

        let (tx, rx) = bounded::<Command>(QUEUE_DEPTH);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = stop.clone();

        let handle = thread::Builder::new()
            .name("rmap-osc-recv".into())
            .spawn(move || {
                let mut buf = vec![0u8; RECV_BUF];
                while !stop_for_thread.load(Ordering::Relaxed) {
                    let (size, _from) = match socket.recv_from(&mut buf) {
                        Ok(v) => v,
                        Err(err)
                            if err.kind() == std::io::ErrorKind::WouldBlock
                                || err.kind() == std::io::ErrorKind::TimedOut =>
                        {
                            continue;
                        }
                        Err(err) => {
                            tracing::warn!(?err, "osc socket recv failed; thread exiting");
                            return;
                        }
                    };
                    match rosc::decoder::decode_udp(&buf[..size]) {
                        Ok((_, packet)) => emit_from_packet(packet, &tx),
                        Err(err) => {
                            tracing::debug!(?err, "osc decode failed; dropping packet");
                        }
                    }
                }
            })?;

        Ok(Self {
            rx,
            stop,
            handle: Some(handle),
        })
    }
}

fn emit_from_packet(packet: OscPacket, tx: &crossbeam_channel::Sender<Command>) {
    match packet {
        OscPacket::Message(msg) => {
            if let Some(event) = decode_message(&msg) {
                let _ = tx.try_send(event);
            }
        }
        OscPacket::Bundle(bundle) => {
            for child in bundle.content {
                emit_from_packet(child, tx);
            }
        }
    }
}

/// Decode one OSC message against the v1 address table. Args are
/// ignored for the four supported addresses; presence of the address
/// itself is the signal.
fn decode_message(msg: &OscMessage) -> Option<Command> {
    let addr = msg.addr.to_lowercase();
    match addr.as_str() {
        "/rmap/tap" => Some(Command::TapTempo(TapSource::Osc)),
        "/rmap/blackout" => Some(Command::Blackout),
        "/rmap/freeze" => Some(Command::Freeze),
        a if a.starts_with("/rmap/scene/") => {
            let suffix = &a["/rmap/scene/".len()..];
            suffix
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=9).contains(n))
                .map(|n| Command::SceneRecall(n - 1))
        }
        _ => None,
    }
}

impl Source for OscSource {
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

    fn msg(addr: &str) -> OscMessage {
        OscMessage {
            addr: addr.into(),
            args: vec![],
        }
    }

    #[test]
    fn decode_tap() {
        assert!(matches!(
            decode_message(&msg("/rmap/tap")),
            Some(Command::TapTempo(TapSource::Osc))
        ));
    }

    #[test]
    fn decode_blackout_and_freeze() {
        assert!(matches!(
            decode_message(&msg("/rmap/blackout")),
            Some(Command::Blackout)
        ));
        assert!(matches!(
            decode_message(&msg("/rmap/freeze")),
            Some(Command::Freeze)
        ));
    }

    #[test]
    fn decode_scene_recall_one_indexed() {
        match decode_message(&msg("/rmap/scene/1")) {
            Some(Command::SceneRecall(idx)) => assert_eq!(idx, 0),
            other => panic!("got {other:?}"),
        }
        match decode_message(&msg("/rmap/scene/9")) {
            Some(Command::SceneRecall(idx)) => assert_eq!(idx, 8),
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn decode_scene_out_of_range_is_none() {
        assert!(decode_message(&msg("/rmap/scene/0")).is_none());
        assert!(decode_message(&msg("/rmap/scene/10")).is_none());
    }

    #[test]
    fn decode_unknown_address_is_none() {
        assert!(decode_message(&msg("/rmap/zzz")).is_none());
        assert!(decode_message(&msg("/other/tap")).is_none());
    }

    #[test]
    fn decode_is_case_insensitive() {
        assert!(matches!(
            decode_message(&msg("/RMAP/Tap")),
            Some(Command::TapTempo(TapSource::Osc))
        ));
    }
}
