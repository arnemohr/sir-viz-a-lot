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
//! P6.9.2 transport control addresses:
//!
//! - `/rmap/cue/go`           → `CueGo`
//! - `/rmap/cue/prev`         → `CueArmPrev`
//! - `/rmap/cue/next`         → `CueArmNext`
//! - `/rmap/cue/back`         → `CueBackStep`
//! - `/rmap/cue/N`            → `SceneRecall(N - 1)` for N in 1..=9
//!
//! Unknown addresses are dropped silently. v0.4 W2.1 maintains a
//! process-wide OSC value registry keyed by address (analogous to
//! `audio::PROVIDER`) that the new `Modulator::OscBound { addr }`
//! resolves against.

use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crossbeam_channel::{Receiver, bounded};
use rosc::{OscMessage, OscPacket, OscType};

use crate::clock::TapSource;
use crate::controls::{Command, Source};
use crate::modulators::osc as osc_modulators;

/// Default UDP listen port for OSC. Operators expecting a different
/// port pass it to [`OscSource::start`].
pub const DEFAULT_PORT: u16 = 8765;

const QUEUE_DEPTH: usize = 256;
/// Max datagram size we accept. Generous for OSC bundles; prevents the
/// receive loop allocating unbounded buffers.
const RECV_BUF: usize = 4096;

/// Process-wide OSC value registry shared between the recv thread (writer)
/// and `Modulator::OscBound` resolves (reader, via the
/// [`crate::modulators::osc::OscProvider`] trait).
///
/// PCleanup.6.1 — populated by `OscSource::start` when the listener binds
/// successfully; the recv thread writes every well-formed message's first
/// numeric argument here, regardless of whether the message also dispatches
/// a command via [`decode_message`]. So
/// `/rmap/blur/radius 0.42` sets the registry value AND (because that
/// address has no command mapping) is silently dropped from the command
/// stream. `/rmap/tap` writes a value AND fires `TapTempo`.
#[derive(Default)]
pub struct OscValueRegistry {
    values: RwLock<HashMap<String, f32>>,
}

impl OscValueRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn store(&self, addr: &str, value: f32) {
        if let Ok(mut g) = self.values.write() {
            g.insert(addr.to_string(), value);
        }
    }
}

impl osc_modulators::OscProvider for OscValueRegistry {
    fn value(&self, addr: &str) -> f32 {
        self.values
            .read()
            .map(|g| g.get(addr).copied().unwrap_or(0.0))
            .unwrap_or(0.0)
    }
}

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
    ///
    /// PCleanup.6.1 — on successful bind, installs an [`OscValueRegistry`]
    /// as the global `Modulator::OscBound` provider via
    /// [`osc_modulators::install`]. The recv thread writes every well-formed
    /// message's first numeric argument into that registry, so OSC-bound
    /// parameters reflect incoming values within one frame. Subsequent calls
    /// to `start` are no-ops on the install side (one-time `OnceLock::set`).
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

        // PCleanup.6.1 — single value registry shared between recv thread
        // (writer) and Modulator::OscBound (reader, via PROVIDER).
        let registry = Arc::new(OscValueRegistry::new());
        osc_modulators::install(registry.clone() as Arc<dyn osc_modulators::OscProvider>);
        let registry_for_thread = registry.clone();

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
                        Ok((_, packet)) => emit_from_packet(packet, &tx, &registry_for_thread),
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

fn emit_from_packet(
    packet: OscPacket,
    tx: &crossbeam_channel::Sender<Command>,
    registry: &OscValueRegistry,
) {
    match packet {
        OscPacket::Message(msg) => {
            // PCleanup.6.1 — every message updates the value registry,
            // regardless of whether it also dispatches a command. Skip
            // messages that carry no numeric arg.
            if let Some(v) = first_numeric_arg(&msg) {
                registry.store(&msg.addr.to_lowercase(), v);
            }
            if let Some(event) = decode_message(&msg) {
                let _ = tx.try_send(event);
            }
        }
        OscPacket::Bundle(bundle) => {
            for child in bundle.content {
                emit_from_packet(child, tx, registry);
            }
        }
    }
}

/// PCleanup.6.1 — extract the first numeric argument of an OSC message
/// as an f32. Handles the common types operators send from MIDI-to-OSC
/// bridges and software controllers: Int, Float, Double, Bool (0/1),
/// and tagged Long. Returns `None` when no usable numeric arg exists,
/// so address-only "trigger" messages (e.g. `/rmap/tap`) don't write a
/// spurious 0.0 into the registry.
fn first_numeric_arg(msg: &OscMessage) -> Option<f32> {
    for arg in &msg.args {
        match *arg {
            OscType::Float(v) => return Some(v),
            OscType::Double(v) => return Some(v as f32),
            OscType::Int(v) => return Some(v as f32),
            OscType::Long(v) => return Some(v as f32),
            OscType::Bool(b) => return Some(if b { 1.0 } else { 0.0 }),
            _ => continue,
        }
    }
    None
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
        // P6.9.2 — transport control addresses.
        #[cfg(feature = "v3")]
        "/rmap/cue/go" => Some(Command::CueGo),
        #[cfg(feature = "v3")]
        "/rmap/cue/prev" => Some(Command::CueArmPrev),
        #[cfg(feature = "v3")]
        "/rmap/cue/next" => Some(Command::CueArmNext),
        #[cfg(feature = "v3")]
        "/rmap/cue/back" => Some(Command::CueBackStep),
        #[cfg(feature = "v3")]
        a if a.starts_with("/rmap/cue/") => {
            // /rmap/cue/N (N = 1..=9) → SceneRecall(N-1) (fire cue N).
            let suffix = &a["/rmap/cue/".len()..];
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
    use crate::modulators::osc::OscProvider;

    fn msg(addr: &str) -> OscMessage {
        OscMessage {
            addr: addr.into(),
            args: vec![],
        }
    }

    fn msg_with_args(addr: &str, args: Vec<OscType>) -> OscMessage {
        OscMessage {
            addr: addr.into(),
            args,
        }
    }

    // ----- PCleanup.6.1 — OSC value registry tests --------------------

    /// PCleanup.6.1 — Float arg extracted as f32.
    #[test]
    fn first_numeric_arg_float() {
        let m = msg_with_args("/rmap/blur/radius", vec![OscType::Float(0.42)]);
        assert_eq!(first_numeric_arg(&m), Some(0.42));
    }

    /// PCleanup.6.1 — Int arg coerced to f32.
    #[test]
    fn first_numeric_arg_int() {
        let m = msg_with_args("/rmap/x", vec![OscType::Int(7)]);
        assert_eq!(first_numeric_arg(&m), Some(7.0));
    }

    /// PCleanup.6.1 — Double truncated to f32.
    #[test]
    fn first_numeric_arg_double() {
        let m = msg_with_args("/rmap/x", vec![OscType::Double(0.5)]);
        assert_eq!(first_numeric_arg(&m), Some(0.5));
    }

    /// PCleanup.6.1 — Long (i64) coerced to f32.
    #[test]
    fn first_numeric_arg_long() {
        let m = msg_with_args("/rmap/x", vec![OscType::Long(42)]);
        assert_eq!(first_numeric_arg(&m), Some(42.0));
    }

    /// PCleanup.6.1 — Bool maps to 0.0 / 1.0 so toggle-style controllers
    /// can drive a modulator-bound parameter without a workaround.
    #[test]
    fn first_numeric_arg_bool() {
        assert_eq!(
            first_numeric_arg(&msg_with_args("/rmap/x", vec![OscType::Bool(true)])),
            Some(1.0)
        );
        assert_eq!(
            first_numeric_arg(&msg_with_args("/rmap/x", vec![OscType::Bool(false)])),
            Some(0.0)
        );
    }

    /// PCleanup.6.1 — Address-only "trigger" messages (e.g. `/rmap/tap`)
    /// return None so they don't pollute the registry with 0.0.
    #[test]
    fn first_numeric_arg_empty_is_none() {
        assert_eq!(first_numeric_arg(&msg("/rmap/tap")), None);
    }

    /// PCleanup.6.1 — Skips leading non-numeric args (e.g. a leading
    /// string label) and uses the first numeric one.
    #[test]
    fn first_numeric_arg_skips_string_prefix() {
        let m = msg_with_args(
            "/rmap/x",
            vec![OscType::String("label".into()), OscType::Float(0.9)],
        );
        assert_eq!(first_numeric_arg(&m), Some(0.9));
    }

    /// PCleanup.6.1 — OscValueRegistry stores and reads back float values
    /// via the OscProvider trait.
    #[test]
    fn osc_value_registry_round_trip() {
        let r = OscValueRegistry::new();
        r.store("/rmap/blur/radius", 0.7);
        r.store("/rmap/x", 1.5);
        assert!((r.value("/rmap/blur/radius") - 0.7).abs() < 1e-6);
        assert!((r.value("/rmap/x") - 1.5).abs() < 1e-6);
        // Never-seen address → 0.0 (matches Modulator::OscBound's
        // no-provider fallback).
        assert_eq!(r.value("/rmap/never/seen"), 0.0);
    }

    /// PCleanup.6.1 — OscValueRegistry overwrite semantics (last write wins).
    #[test]
    fn osc_value_registry_overwrites() {
        let r = OscValueRegistry::new();
        r.store("/rmap/x", 0.1);
        r.store("/rmap/x", 0.9);
        assert!((r.value("/rmap/x") - 0.9).abs() < 1e-6);
    }

    /// PCleanup.6.1 — `emit_from_packet` writes the value AND, when the
    /// address has a command mapping, also dispatches a command. Address-
    /// only messages still write a value, just with `None` from
    /// `first_numeric_arg` (skipped — see `first_numeric_arg_empty_is_none`).
    #[test]
    fn emit_from_packet_writes_registry_and_dispatches_command() {
        let (tx, rx) = bounded::<Command>(8);
        let registry = OscValueRegistry::new();
        // /rmap/tap with a float arg should both fire TapTempo AND store
        // 1.0 in the registry under /rmap/tap.
        let packet = OscPacket::Message(msg_with_args("/rmap/tap", vec![OscType::Float(1.0)]));
        emit_from_packet(packet, &tx, &registry);
        // Registry write happened.
        assert!((registry.value("/rmap/tap") - 1.0).abs() < 1e-6);
        // Command was dispatched.
        let cmd = rx.try_recv().expect("expected TapTempo command");
        assert!(matches!(cmd, Command::TapTempo(TapSource::Osc)));
    }

    /// PCleanup.6.1 — Modulator-style address with no command mapping
    /// writes the registry and dispatches no command (silent drop on
    /// the command side, registry update on the modulator side).
    #[test]
    fn emit_from_packet_modulator_only_address() {
        let (tx, rx) = bounded::<Command>(8);
        let registry = OscValueRegistry::new();
        let packet = OscPacket::Message(msg_with_args(
            "/rmap/blur/radius",
            vec![OscType::Float(0.33)],
        ));
        emit_from_packet(packet, &tx, &registry);
        assert!((registry.value("/rmap/blur/radius") - 0.33).abs() < 1e-6);
        assert!(rx.try_recv().is_err(), "no command for modulator address");
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

    // P6.9.2 — transport control address tests.
    #[cfg(feature = "v3")]
    #[test]
    fn decode_cue_go() {
        assert!(matches!(
            decode_message(&msg("/rmap/cue/go")),
            Some(Command::CueGo)
        ));
    }

    #[cfg(feature = "v3")]
    #[test]
    fn decode_cue_navigation() {
        assert!(matches!(
            decode_message(&msg("/rmap/cue/prev")),
            Some(Command::CueArmPrev)
        ));
        assert!(matches!(
            decode_message(&msg("/rmap/cue/next")),
            Some(Command::CueArmNext)
        ));
        assert!(matches!(
            decode_message(&msg("/rmap/cue/back")),
            Some(Command::CueBackStep)
        ));
    }

    #[cfg(feature = "v3")]
    #[test]
    fn decode_cue_fire_by_number() {
        match decode_message(&msg("/rmap/cue/1")) {
            Some(Command::SceneRecall(idx)) => assert_eq!(idx, 0),
            other => panic!("got {other:?}"),
        }
        match decode_message(&msg("/rmap/cue/9")) {
            Some(Command::SceneRecall(idx)) => assert_eq!(idx, 8),
            other => panic!("got {other:?}"),
        }
    }

    #[cfg(feature = "v3")]
    #[test]
    fn decode_cue_out_of_range_is_none() {
        assert!(decode_message(&msg("/rmap/cue/0")).is_none());
        assert!(decode_message(&msg("/rmap/cue/10")).is_none());
    }
}
