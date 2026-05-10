//! P0.3.1 (W3.1) — thread-safe texture-upload queue.
//!
//! Background-thread producers (the video decoder worker landed in
//! P0.4.1, the NDI receiver in P0.6.2) push decoded frames onto a
//! bounded `crossbeam-channel`. The render thread drains the queue
//! once per frame, BEFORE layer drawing, and uploads each frame to
//! its destination texture via `wgpu::Queue::write_texture`.
//!
//! ## Why a queue, not direct `Queue::write_texture` calls
//!
//! `wgpu::Queue` is `Send + Sync` and accepting writes from any
//! thread is in principle safe, but the actual upload competes with
//! the render thread's command-buffer submission. Funnelling all
//! uploads through a single per-frame drain on the render thread
//! (a) keeps GPU command ordering deterministic, (b) caps the work
//! per frame so a producer flood can't stall vsync, and (c) gives
//! us one place to count dropped frames for diagnostics (P0.3.2).
//!
//! ## Design notes
//!
//! - Bounded depth: producers use `try_send`; on queue-full they drop
//!   the frame and increment [`TextureUploadQueue::dropped_count`].
//!   Audio's `try_send` overflow path (`src/modulators/audio.rs:170`)
//!   is the precedent — best-effort delivery, count drops, never
//!   block the producer.
//! - Drain cap per frame ([`MAX_DRAIN_PER_FRAME`]) keeps the render
//!   thread's wall-clock budget bounded even under sustained
//!   over-production.
//! - Frames carry a `wgpu::TextureView` borrowed from the
//!   `LayerState`'s upload target; the drain calls
//!   `Queue::write_texture` against that view.
//!
//! ## Integration status
//!
//! P0.3.1 ships the queue + counters + tests. The drain hook into the
//! per-frame render path lands alongside the first real producer
//! (P0.4.2 video render integration); without a producer, the queue
//! is always empty and the drain is a no-op. The `panic_restore`
//! wrapping invariant in `src/render/CLAUDE.md` covers the drain
//! once it lands inside `frame()`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};

/// Maximum frames drained per render frame. At 60 Hz vsync this caps
/// upload work at ~16 ms even if a producer floods the channel; any
/// excess remains queued for the next frame (or is eventually
/// dropped by the bounded channel).
pub const MAX_DRAIN_PER_FRAME: usize = 8;

/// Default channel depth — gives ~1 frame of slack at 60 Hz when up
/// to 4 layers each push at 30 fps. Tune if W4 / W6 telemetry shows
/// either chronic drops or chronic backlog.
pub const DEFAULT_QUEUE_DEPTH: usize = 8;

/// Identifier for the destination layer of a queued frame. Producers
/// (video / NDI workers) attach this so the drain knows which
/// `LayerState`'s upload target to call `write_texture` against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadTargetId(pub u64);

/// One ready-to-upload frame from a background producer.
///
/// `pixels` is a `Box<[u8]>` so the producer doesn't pay reallocation
/// cost on every frame — workers reuse a fixed-size buffer pool when
/// possible. `(width, height)` describes the producer's pixel layout;
/// the drain matches this against the destination texture's actual
/// dimensions and skips the upload (incrementing
/// `format_mismatch_count`) on disagreement rather than panic.
#[derive(Debug, Clone)]
pub struct TextureFrame {
    /// Which layer this frame belongs to.
    pub target: UploadTargetId,
    /// Row-major RGBA8 (or NV12 / YUV420 once W4 lands variant
    /// formats — for P0.3.1 the queue treats the bytes as opaque).
    pub pixels: Box<[u8]>,
    /// Pixel dimensions of `pixels`.
    pub width: u32,
    pub height: u32,
    /// Texture format the producer wrote. Matched against the
    /// destination's format at drain time.
    pub format: wgpu::TextureFormat,
    /// Producer-side timestamp (monotonic, e.g.
    /// `std::time::Instant::now()`'s `Duration::as_nanos()`). Lets
    /// W4 / W6 enforce a frame-budget window — the drain can drop
    /// stale frames before uploading.
    pub timestamp_nanos: u128,
}

/// Send half handed to producer threads. Cheap to clone (`Arc` under
/// the hood); each video worker / NDI receiver holds its own clone.
#[derive(Clone)]
pub struct TextureFrameSender {
    inner: Sender<TextureFrame>,
    dropped: Arc<AtomicU64>,
}

impl TextureFrameSender {
    /// Try to enqueue `frame`. On a full queue the frame is dropped
    /// silently and `dropped_count` is incremented; the producer
    /// must not block (audio path precedent —
    /// `src/modulators/audio.rs:170`).
    pub fn try_send(&self, frame: TextureFrame) {
        match self.inner.try_send(frame) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => {
                // Render thread shut down — producer is on its way
                // out too. Count it so the diagnostics surface
                // notices.
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// Receive half held by the renderer. Drained once per frame inside
/// the `panic_restore` boundary.
pub struct TextureUploadQueue {
    inner: Receiver<TextureFrame>,
    sender: Sender<TextureFrame>,
    dropped: Arc<AtomicU64>,
}

impl TextureUploadQueue {
    /// Allocate a new queue with the default depth.
    pub fn new() -> Self {
        Self::with_depth(DEFAULT_QUEUE_DEPTH)
    }

    /// Allocate a queue with a caller-chosen depth. Tests use this to
    /// pin overflow behaviour deterministically.
    pub fn with_depth(depth: usize) -> Self {
        let (sender, inner) = bounded::<TextureFrame>(depth);
        Self {
            inner,
            sender,
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Hand a sender to a producer thread.
    pub fn sender(&self) -> TextureFrameSender {
        TextureFrameSender {
            inner: self.sender.clone(),
            dropped: Arc::clone(&self.dropped),
        }
    }

    /// Drain up to [`MAX_DRAIN_PER_FRAME`] frames into `out`. Caller
    /// (the render thread) then iterates `out` and uploads each
    /// frame to the matching texture via `Queue::write_texture`.
    /// Returning the frames rather than uploading inline lets the
    /// caller resolve `target → texture` against `EditingState`
    /// without coupling this module to the layer state.
    pub fn drain_into(&self, out: &mut Vec<TextureFrame>) {
        out.clear();
        for _ in 0..MAX_DRAIN_PER_FRAME {
            match self.inner.try_recv() {
                Ok(frame) => out.push(frame),
                Err(_) => break,
            }
        }
    }

    /// Total frames dropped since process start, across this queue's
    /// senders. Reset only by a test harness via [`Self::reset_dropped_count`].
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Test-only: reset the dropped counter so multiple cases in a
    /// single process don't bleed into each other.
    #[cfg(test)]
    fn reset_dropped_count(&self) {
        self.dropped.store(0, Ordering::Relaxed);
    }
}

impl Default for TextureUploadQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_frame(target: u64) -> TextureFrame {
        let pixels = vec![0u8; 4].into_boxed_slice();
        TextureFrame {
            target: UploadTargetId(target),
            pixels,
            width: 1,
            height: 1,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            timestamp_nanos: 0,
        }
    }

    /// `drain_into` returns at most `MAX_DRAIN_PER_FRAME` frames per
    /// call even when more are queued.
    #[test]
    fn drain_caps_at_max_per_frame() {
        let q = TextureUploadQueue::with_depth(MAX_DRAIN_PER_FRAME * 4);
        let s = q.sender();
        for i in 0..(MAX_DRAIN_PER_FRAME * 2) {
            s.try_send(fake_frame(i as u64));
        }
        let mut out = Vec::new();
        q.drain_into(&mut out);
        assert_eq!(out.len(), MAX_DRAIN_PER_FRAME);

        // A second drain pulls the remaining frames.
        q.drain_into(&mut out);
        assert_eq!(out.len(), MAX_DRAIN_PER_FRAME);

        // Empty after.
        q.drain_into(&mut out);
        assert!(out.is_empty());
    }

    /// On a full queue, `try_send` drops + increments the counter
    /// without blocking the producer.
    #[test]
    fn full_queue_drops_and_counts() {
        let q = TextureUploadQueue::with_depth(2);
        q.reset_dropped_count();
        let s = q.sender();
        s.try_send(fake_frame(1));
        s.try_send(fake_frame(2));
        // Third send overflows.
        s.try_send(fake_frame(3));
        s.try_send(fake_frame(4));
        assert_eq!(q.dropped_count(), 2);
        // The two original frames are still drainable.
        let mut out = Vec::new();
        q.drain_into(&mut out);
        assert_eq!(out.len(), 2);
    }

    /// Multiple senders share the dropped counter (Arc).
    #[test]
    fn senders_share_dropped_counter() {
        let q = TextureUploadQueue::with_depth(1);
        q.reset_dropped_count();
        let s1 = q.sender();
        let s2 = q.sender();
        s1.try_send(fake_frame(1));
        // Both s1 and s2 overflow against the depth-1 queue.
        s1.try_send(fake_frame(2));
        s2.try_send(fake_frame(3));
        assert_eq!(q.dropped_count(), 2);
    }

    /// Drained frames preserve their producer-side fields verbatim.
    #[test]
    fn drained_frames_preserve_payload() {
        let q = TextureUploadQueue::with_depth(4);
        let s = q.sender();
        let mut original = fake_frame(42);
        original.timestamp_nanos = 1_234_567_890;
        original.width = 1920;
        original.height = 1080;
        s.try_send(original.clone());

        let mut out = Vec::new();
        q.drain_into(&mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, original.target);
        assert_eq!(out[0].timestamp_nanos, original.timestamp_nanos);
        assert_eq!(out[0].width, original.width);
        assert_eq!(out[0].height, original.height);
    }

    /// Spawn a producer thread, push 1000 frames into a depth-`MAX *
    /// 2` queue while a single drain runs on the main thread; total
    /// drained + dropped accounts for every frame.
    #[test]
    fn producer_thread_stress_no_loss() {
        const TOTAL: u64 = 1000;
        let q = TextureUploadQueue::with_depth(MAX_DRAIN_PER_FRAME * 2);
        q.reset_dropped_count();
        let s = q.sender();
        let producer = std::thread::spawn(move || {
            for i in 0..TOTAL {
                s.try_send(fake_frame(i));
            }
        });

        let mut drained = 0u64;
        let mut out = Vec::new();
        // Drain in a loop until producer is done AND the queue is
        // empty (one final drain after the join confirms).
        while !producer.is_finished() {
            q.drain_into(&mut out);
            drained += out.len() as u64;
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let _ = producer.join();
        // One final drain to flush the queue.
        loop {
            q.drain_into(&mut out);
            if out.is_empty() {
                break;
            }
            drained += out.len() as u64;
        }

        let dropped = q.dropped_count();
        assert_eq!(
            drained + dropped,
            TOTAL,
            "every produced frame should be either drained or counted as dropped",
        );
    }
}
