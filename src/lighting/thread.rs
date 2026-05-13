//! P5.2.3 — `LightingThread` background loop.
//!
//! Mirrors `OscSource` from `src/controls/osc.rs`:
//! - background thread owns the transport and ticks at ~44 Hz
//! - bounded crossbeam channel (capacity 4) between render thread and
//!   lighting thread
//! - `Arc<AtomicBool>` stop flag; `Drop` sets the flag and joins the
//!   thread so no threads are leaked on drop or panic
//! - `try_send` on the render side is non-blocking; if the channel is
//!   full the frame is silently dropped (the lighting thread has fallen
//!   behind; it catches up on the next tick)
//!
//! The lighting thread ticks at ~44 Hz, which gives Art-Net nodes a
//! consistent 22 ms refresh. At 60 Hz render rate the render thread
//! may queue a few frames between ticks; the thread drains all pending
//! frames on each tick and keeps only the latest value per universe ID
//! (older superseded frames are discarded).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Sender, TryRecvError, bounded};

use crate::lighting::transport::DmxTransport;
use crate::lighting::universe::{UniverseFrame, UniverseId};

/// Target lighting send rate (44 Hz, matching the Art-Net spec recommendation
/// of "4-second maximum re-transmit time" — in practice the small-show
/// community expects 44 Hz for smooth dimmer response).
const TICK_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 44);

/// Capacity of the crossbeam channel between the render thread and the
/// lighting thread. At 60 fps render and 44 Hz lighting, the render
/// thread can queue at most ~1.4 frames between lighting ticks; a capacity
/// of 4 provides ample headroom before `try_send` starts dropping.
const CHANNEL_CAPACITY: usize = 4;

/// Handle to the background DMX send thread.
///
/// Created by [`LightingThread::start`]. Drop stops the thread cleanly
/// (sets the stop flag, joins the thread). This ensures no threads are
/// leaked when `EditingState` is dropped on `ExitGoLive`.
pub struct LightingThread {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
    /// P5.9.1 — `true` while packets are being sent (within ~2 s).
    /// Set by the lighting thread on each successful `send_universe`; read by
    /// the diagnostics renderer to display the DMX activity LED.
    pub dmx_active: Arc<AtomicBool>,
    /// P5.9.2 — rolling per-second packet count.
    /// Incremented by the lighting thread on each `send_universe`; read by the
    /// diagnostics renderer and reset every second by the lighting thread itself.
    pub packet_count_per_sec: Arc<AtomicU64>,
}

impl LightingThread {
    /// Start the background lighting thread.
    ///
    /// Returns `(LightingThread, Sender<UniverseFrame>)`. The caller (render
    /// thread) pushes frames via `tx.try_send(frame)` — non-blocking. If the
    /// channel is full, `try_send` returns `Err(Full)` and the frame is
    /// silently dropped; the lighting thread is never blocked.
    ///
    /// The thread takes ownership of `transport` and uses it to send
    /// `ArtDmx` PDUs at ~44 Hz. `NullTransport` is suitable for tests.
    pub fn start(mut transport: impl DmxTransport) -> (Self, Sender<UniverseFrame>) {
        let (tx, rx) = bounded::<UniverseFrame>(CHANNEL_CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = stop.clone();
        let dmx_active = Arc::new(AtomicBool::new(false));
        let dmx_active_thread = dmx_active.clone();
        let packet_count = Arc::new(AtomicU64::new(0));
        let packet_count_thread = packet_count.clone();

        let handle = thread::Builder::new()
            .name("rmap-lighting".into())
            .spawn(move || {
                // Latest snapshot per universe. We keep only the newest frame
                // for each universe ID; the render thread may have sent multiple
                // frames between lighting ticks (at 60 fps vs 44 Hz) — all but
                // the last are superseded and discarded.
                let mut latest: HashMap<UniverseId, [u8; 512]> = HashMap::new();
                // P5.9.2 — packet-rate tracking.
                let mut rate_window_start = Instant::now();
                let mut rate_count: u64 = 0;
                // P5.9.1 — activity LED: set to false after 2 s of no sends.
                let mut last_send_at: Option<Instant> = None;

                loop {
                    if stop_for_thread.load(Ordering::Relaxed) {
                        break;
                    }

                    // Drain all pending frames, keeping only the latest per universe.
                    loop {
                        match rx.try_recv() {
                            Ok(frame) => {
                                latest.insert(frame.id, frame.data.0);
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                // Sender dropped → sender side is gone; exit the thread.
                                return;
                            }
                        }
                    }

                    // Send all queued universes.
                    let sent_this_tick = !latest.is_empty();
                    for (id, data) in &latest {
                        if let Err(e) = transport.send_universe(id.as_u16(), data) {
                            tracing::warn!(universe = id.as_u16(), ?e, "lighting send error");
                        } else {
                            rate_count += 1;
                        }
                    }

                    // P5.9.1 — update activity flag.
                    if sent_this_tick {
                        last_send_at = Some(Instant::now());
                        dmx_active_thread.store(true, Ordering::Relaxed);
                    } else if let Some(t) = last_send_at {
                        if t.elapsed() > Duration::from_secs(2) {
                            dmx_active_thread.store(false, Ordering::Relaxed);
                            last_send_at = None;
                        }
                    }

                    // P5.9.2 — update packet-rate counter every second.
                    if rate_window_start.elapsed() >= Duration::from_secs(1) {
                        packet_count_thread.store(rate_count, Ordering::Relaxed);
                        rate_count = 0;
                        rate_window_start = Instant::now();
                    }

                    thread::sleep(TICK_INTERVAL);
                }
            })
            .expect("failed to spawn rmap-lighting thread");

        (
            Self {
                stop,
                handle: Some(handle),
                dmx_active,
                packet_count_per_sec: packet_count,
            },
            tx,
        )
    }
}

impl Drop for LightingThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            // Allow up to two tick intervals for the thread to notice the flag.
            // In tests the thread responds within one tick (≤23 ms).
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::lighting::transport::NullTransport;
    use crate::lighting::universe::{DmxUniverse, UniverseFrame, UniverseId};

    /// P5.2.3 — spawn thread with NullTransport; send 100 frames rapidly;
    /// verify thread drains cleanly and join succeeds within 200 ms.
    #[test]
    fn lighting_thread_drains_and_stops_cleanly() {
        let (thread, tx) = LightingThread::start(NullTransport);

        // Send 100 frames rapidly. Some will be dropped (channel bounded at 4).
        let mut sent = 0u32;
        for i in 0..100u32 {
            let mut data = DmxUniverse::default();
            *data.channel_mut(0_usize) = (i % 256) as u8;
            let frame = UniverseFrame {
                id: UniverseId(1),
                data,
            };
            if tx.try_send(frame).is_ok() {
                sent += 1;
            }
        }

        // Channel is bounded; at least some sends should succeed.
        assert!(
            sent > 0,
            "expected at least one successful try_send, got {sent}"
        );

        // Drop the thread handle — this sets the stop flag and joins.
        let start = Instant::now();
        drop(thread);
        let elapsed = start.elapsed();

        // Join must complete in < 200 ms (two tick intervals).
        assert!(
            elapsed < Duration::from_millis(200),
            "thread join took {elapsed:?}, expected < 200 ms"
        );
    }

    /// P5.2.3 — thread exits when sender is dropped (disconnected channel).
    #[test]
    fn lighting_thread_exits_on_sender_drop() {
        let (thread, tx) = LightingThread::start(NullTransport);
        // Drop the sender side — the thread should detect disconnection.
        drop(tx);
        let start = Instant::now();
        drop(thread);
        assert!(
            start.elapsed() < Duration::from_millis(200),
            "thread did not exit promptly on sender drop"
        );
    }

    /// P5.2.3 — verify CHANNEL_CAPACITY constant matches the module doc.
    #[test]
    fn channel_capacity_is_correct() {
        const {
            assert!(
                CHANNEL_CAPACITY == 4,
                "channel capacity must be 4 for frame-drop safety"
            )
        }
    }
}
