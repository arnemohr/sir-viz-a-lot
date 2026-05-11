//! P0.4.2 Part 1 — video worker stub.
//!
//! This is the scaffolding pass. The worker thread spawns on layer
//! init, holds a control channel, and idles on `recv()` for control
//! messages — but does NOT decode. P0.4.2 Part 2 (AVFoundation behind
//! the `video` cargo feature) replaces the idle loop with the real
//! decode loop. The control-channel shape is fixed here so Part 2 is
//! a body-replacement, not an interface churn.

use std::path::PathBuf;
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::render::texture_upload::{TextureFrameSender, UploadTargetId};

/// Control messages the UI / mutation layer dispatches to the worker.
/// Mirrors the W4.4 control-channel shape from the decoder decision
/// record.
// P0.4.2a (Part 1): Play/Pause/SetSpeed/SetLoop are not yet dispatched
// by the UI — that lands in P0.4.3. They are public API so Part 2/3 can
// add producers without changing this file's interface.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum VideoControl {
    Play,
    Pause,
    SetSpeed(f32),
    SetLoop(bool),
    Stop,
}

/// Spawn a video worker for `path`. Returns the join handle (for
/// shutdown) and the control sender (for play/pause/etc).
///
/// **Part 1 status:** the worker thread is a stub — it blocks on
/// `control_rx.recv()` and processes messages, but does not decode
/// or push frames. Part 2 (`feature = "video"`) replaces the body
/// with the AVFoundation decoder loop.
pub fn spawn(
    path: PathBuf,
    target: UploadTargetId,
    upload: TextureFrameSender,
) -> (JoinHandle<()>, Sender<VideoControl>) {
    let (control_tx, control_rx) = unbounded::<VideoControl>();
    let handle = std::thread::Builder::new()
        .name(format!("rmap-video-worker({})", path.display()))
        .spawn(move || worker_loop(path, target, upload, control_rx))
        .expect("rmap-video-worker thread spawn failed");
    (handle, control_tx)
}

fn worker_loop(
    _path: PathBuf,
    _target: UploadTargetId,
    _upload: TextureFrameSender,
    control_rx: Receiver<VideoControl>,
) {
    // Part 1: no decoder. Block on the control channel; exit on Stop
    // or disconnect. This keeps the interface stable for Part 2.
    tracing::info!(
        target: "rmap::video",
        "video worker stub started (no decoder; Part 2 wires AVFoundation)",
    );
    loop {
        match control_rx.recv() {
            Ok(VideoControl::Stop) => break,
            Ok(_) => continue, // Play/Pause/SetSpeed/SetLoop: no-op in Part 1.
            Err(_) => break,   // Sender dropped — the LayerState was removed.
        }
    }
    tracing::info!(target: "rmap::video", "video worker stub exiting");
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::render::texture_upload::TextureUploadQueue;

    /// P0.4.2 — worker stub spawns cleanly; sending `Stop` causes the
    /// thread to exit within 500 ms. If the thread hangs, the test fails
    /// (recv_timeout expires and the join check panics).
    #[test]
    fn worker_stub_exits_on_stop() {
        let q = TextureUploadQueue::new();
        let sender = q.sender();
        let target = UploadTargetId(9999);
        let path = std::path::PathBuf::from("/nonexistent/test.mp4");

        let (handle, control_tx) = spawn(path, target, sender);

        // Give the worker a moment to start up, then send Stop.
        std::thread::sleep(Duration::from_millis(10));
        control_tx.send(VideoControl::Stop).expect("send Stop");

        // Poll with a 500 ms deadline — if the thread hangs, this panics.
        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        loop {
            if handle.is_finished() {
                return; // success
            }
            if std::time::Instant::now() >= deadline {
                panic!("video worker thread did not exit within 500 ms after Stop");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// P0.4.2 — dropping the control sender (channel disconnect) causes
    /// the worker to exit cleanly (Err path in worker_loop).
    #[test]
    fn worker_stub_exits_on_sender_drop() {
        let q = TextureUploadQueue::new();
        let sender = q.sender();
        let target = UploadTargetId(9998);
        let path = std::path::PathBuf::from("/nonexistent/test2.mp4");

        let (handle, control_tx) = spawn(path, target, sender);
        std::thread::sleep(Duration::from_millis(10));
        drop(control_tx); // disconnect — worker should see Err and exit

        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        loop {
            if handle.is_finished() {
                return;
            }
            if std::time::Instant::now() >= deadline {
                panic!("video worker thread did not exit within 500 ms after sender drop");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
