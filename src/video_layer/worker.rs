//! P0.4.2 Part 2 — video worker with AVFoundation decoder (behind `video` feature).
//!
//! The `spawn` function and the `VideoControl` enum are the stable public API
//! used by `app.rs`. The worker loop has two implementations:
//!
//! - `#[cfg(all(feature = "video", target_os = "macos"))]` — real AVFoundation /
//!   VideoToolbox H.264 decoder.  On each iteration it pulls a `CMSampleBuffer`,
//!   extracts the `CVPixelBuffer` (BGRA8), copies the bytes into a `Box<[u8]>`,
//!   and pushes a `TextureFrame` onto the `TextureUploadQueue`.
//!
//! - `#[cfg(not(all(feature = "video", target_os = "macos")))]` — the Part 1
//!   stub: blocks on `control_rx.recv()` and exits on Stop / sender disconnect.
//!
//! ## Thread-safety note
//!
//! All AVFoundation objects (`AVURLAsset`, `AVAssetReader`, `CVImageBuffer`, …)
//! are created and used exclusively inside the worker thread — they never cross
//! thread boundaries.  The `PathBuf` and `UploadTargetId` that move in are both
//! `Send`.  No `unsafe impl Send` stubs are needed.
//!
//! ## Dimension policy
//!
//! `pub fn natural_size(path: &Path) -> Option<(u32, u32)>` (behind the video
//! feature) probes the asset synchronously before the worker is spawned so
//! `app.rs` can allocate `video_texture` at the asset's native resolution.  If
//! the probe fails the texture falls back to output size (Part 1 behaviour), and
//! the worker transitions to the dead state immediately on init failure.
//!
//! ## Limitations (v0.4)
//!
//! - Audio is not decoded (show plays video silently).
//! - Only H.264 / mp4 tested; other formats supported by VideoToolbox may work
//!   but are untested.
//! - Dynamic texture resize (if the asset's dims later disagree) is deferred to
//!   a Phase 1 follow-up.

use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::render::texture_upload::{TextureFrameSender, UploadTargetId};

// ---------------------------------------------------------------------------
// Public API — stable across Part 1 and Part 2
// ---------------------------------------------------------------------------

/// Control messages the UI / mutation layer dispatches to the worker.
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

// ---------------------------------------------------------------------------
// AVFoundation implementation (macOS + `video` feature)
// ---------------------------------------------------------------------------

/// Probe the asset's first video track dimensions synchronously.
/// Called from `app.rs` before layer construction so `video_texture` is
/// allocated at the decoder's native resolution rather than output size.
/// Returns `None` if the asset doesn't exist, has no video tracks, or
/// reports zero dimensions.
#[cfg(all(feature = "video", target_os = "macos"))]
pub fn natural_size(path: &Path) -> Option<(u32, u32)> {
    // SAFETY: The only requirement for these objc2 APIs is that they run within
    // a valid Objective-C runtime context, which is always true on macOS.
    unsafe {
        use objc2_av_foundation::{AVMediaTypeVideo, AVURLAsset};
        use objc2_foundation::{NSString, NSURL};

        let path_str = path.to_string_lossy();
        let ns_path = NSString::from_str(&path_str);
        let url = NSURL::fileURLWithPath(&ns_path);
        let asset = AVURLAsset::URLAssetWithURL_options(&url, None);

        // `tracksWithMediaType:` is deprecated (prefer async loadTracks…) but
        // synchronous, which is exactly what we need for a quick probe.
        let media_type = AVMediaTypeVideo.as_ref()?;

        #[allow(deprecated)]
        let tracks = asset.tracksWithMediaType(media_type);
        if tracks.is_empty() {
            tracing::warn!(
                target: "rmap::video",
                path = %path.display(),
                "natural_size probe: no video tracks found",
            );
            return None;
        }
        let track = tracks.firstObject()?;
        let size = track.naturalSize();
        let w = size.width as u32;
        let h = size.height as u32;
        if w == 0 || h == 0 {
            tracing::warn!(
                target: "rmap::video",
                path = %path.display(),
                w,
                h,
                "natural_size probe returned zero dimensions",
            );
            return None;
        }
        tracing::debug!(
            target: "rmap::video",
            path = %path.display(),
            w,
            h,
            "natural_size probe ok",
        );
        Some((w, h))
    }
}

// ---------------------------------------------------------------------------
// Worker state machine (module-level so decode helper functions can reference it)
// ---------------------------------------------------------------------------

#[cfg(all(feature = "video", target_os = "macos"))]
enum WorkerState {
    Playing { speed: f32, loop_seamless: bool },
    Paused { speed: f32, loop_seamless: bool },
    Dead,
}

/// Result of one decode pass.
#[cfg(all(feature = "video", target_os = "macos"))]
enum PassOutcome {
    /// `copyNextSampleBuffer` returned None with status == Completed.
    Eof,
    /// Stop message received or control sender disconnected.
    Stop,
    /// Fatal error — caller should transition to Dead.
    Dead,
    /// Paused by control message; WorkerState already updated.
    Paused,
}
// Note: SetSpeed and SetLoop are handled in-place inside decode_pass
// (frame_dur is recalculated without rebuilding the reader) so they
// never cause a seek-back to t=0. No ControlUpdated variant needed.

#[cfg(all(feature = "video", target_os = "macos"))]
fn worker_loop(
    path: PathBuf,
    target: UploadTargetId,
    upload: TextureFrameSender,
    control_rx: Receiver<VideoControl>,
) {
    tracing::info!(
        target: "rmap::video",
        path = %path.display(),
        "video worker starting (AVFoundation decoder)",
    );

    // Start in Playing (auto-play on layer-add).
    let mut state = WorkerState::Playing {
        speed: 1.0,
        loop_seamless: true,
    };

    loop {
        match state {
            WorkerState::Dead => {
                // Drain the control channel; exit on Stop or disconnect.
                match control_rx.recv() {
                    Ok(VideoControl::Stop) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            WorkerState::Paused {
                speed,
                loop_seamless,
            } => {
                // Block (not try_recv / thread::park) on the control channel
                // per the decision record: thread::park has coalescing-wake
                // bugs under rapid play/pause toggles.
                match control_rx.recv() {
                    Ok(VideoControl::Play) => {
                        state = WorkerState::Playing {
                            speed,
                            loop_seamless,
                        };
                    }
                    Ok(VideoControl::SetSpeed(s)) => {
                        state = WorkerState::Paused {
                            speed: s,
                            loop_seamless,
                        };
                    }
                    Ok(VideoControl::SetLoop(l)) => {
                        state = WorkerState::Paused {
                            speed,
                            loop_seamless: l,
                        };
                    }
                    Ok(VideoControl::Pause) => {
                        // Already paused; idempotent.
                        state = WorkerState::Paused {
                            speed,
                            loop_seamless,
                        };
                    }
                    Ok(VideoControl::Stop) | Err(_) => break,
                }
            }
            WorkerState::Playing {
                speed,
                loop_seamless,
            } => {
                // Build a fresh reader for one pass.
                let (speed_now, loop_seamless_now) = (speed, loop_seamless);
                let reader_result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build_reader(&path)));
                let (reader, fps) = match reader_result {
                    Err(panic_val) => {
                        tracing::error!(
                            target: "rmap::video",
                            path = %path.display(),
                            "video worker: panic during reader init: {:?}", panic_val,
                        );
                        state = WorkerState::Dead;
                        continue;
                    }
                    Ok(Err(msg)) => {
                        tracing::error!(
                            target: "rmap::video",
                            path = %path.display(),
                            "video worker: reader init failed: {}", msg,
                        );
                        state = WorkerState::Dead;
                        continue;
                    }
                    Ok(Ok(pair)) => pair,
                };

                let fps = fps.max(1.0_f32);
                let effective_speed = speed_now.max(0.01_f32);
                let frame_dur =
                    Duration::from_secs_f64(1.0 / (fps as f64 * effective_speed as f64));

                let outcome = decode_pass(
                    &reader,
                    target,
                    &upload,
                    &control_rx,
                    fps,
                    frame_dur,
                    &mut state,
                );

                match outcome {
                    PassOutcome::Eof => {
                        // loop_seamless was true (decode_pass only returns Eof
                        // when loop_seamless) — fall through to rebuild reader.
                        let _ = loop_seamless_now;
                        // state is still Playing; outer loop rebuilds.
                    }
                    PassOutcome::Stop => break,
                    PassOutcome::Dead => {
                        // decode_pass already set state = Dead; continue outer loop.
                    }
                    PassOutcome::Paused => {
                        // state updated inside; continue outer loop (enters Paused arm).
                    }
                }
            }
        }
    }

    tracing::info!(
        target: "rmap::video",
        path = %path.display(),
        "video worker exiting",
    );
}

/// Construct an `AVURLAsset` → `AVAssetReader` ready for BGRA8 reading.
/// Returns `(reader, nominal_fps)`.
#[cfg(all(feature = "video", target_os = "macos"))]
fn build_reader(
    path: &Path,
) -> Result<(objc2::rc::Retained<objc2_av_foundation::AVAssetReader>, f32), String> {
    use objc2_av_foundation::{
        AVAssetReader, AVAssetReaderTrackOutput, AVMediaTypeVideo, AVURLAsset,
    };
    use objc2_core_video::{kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_32BGRA};
    use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

    unsafe {
        let path_str = path.to_string_lossy();
        let ns_path = NSString::from_str(&path_str);
        let url = NSURL::fileURLWithPath(&ns_path);
        let asset = AVURLAsset::URLAssetWithURL_options(&url, None);

        // Find first video track.
        let media_type = AVMediaTypeVideo
            .as_ref()
            .ok_or("AVMediaTypeVideo unavailable")?;
        #[allow(deprecated)]
        let tracks = asset.tracksWithMediaType(media_type);
        if tracks.is_empty() {
            return Err(format!("no video tracks in '{}'", path.display()));
        }
        let track = tracks
            .firstObject()
            .ok_or("tracks array unexpectedly empty")?;

        let fps = track.nominalFrameRate();

        // Build output settings requesting BGRA8.
        // `kCVPixelBufferPixelFormatTypeKey` is a CFString static, toll-free
        // bridged to NSString. Cast through a raw pointer to avoid type-alias
        // ambiguity when the `CFString` feature on objc2-core-foundation is not
        // fully resolved in the transitive feature graph.
        let key_ptr: *const objc2_foundation::NSString =
            kCVPixelBufferPixelFormatTypeKey as *const _ as *const objc2_foundation::NSString;
        let format_key: &NSString = &*key_ptr;
        let format_val = NSNumber::new_u32(kCVPixelFormatType_32BGRA);

        let output_settings: objc2::rc::Retained<NSDictionary<NSString, _>> =
            NSDictionary::from_slices(&[format_key], &[format_val.as_ref()]);

        // Construct reader.
        let reader = AVAssetReader::assetReaderWithAsset_error(asset.as_ref())
            .map_err(|e| format!("AVAssetReader init failed: {:?}", e))?;

        // Construct track output with BGRA settings.
        let track_output = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(
            &track,
            Some(&output_settings),
        );
        // Performance: don't copy sample data unnecessarily.
        track_output.setAlwaysCopiesSampleData(false);
        reader.addOutput(&track_output);

        if !reader.startReading() {
            let err = reader.error();
            return Err(format!("startReading failed: {:?}", err));
        }

        Ok((reader, fps))
    }
}

/// Run one decode pass until EOF, Stop, pause, or error.
/// Updates `state` in place.
#[cfg(all(feature = "video", target_os = "macos"))]
fn decode_pass(
    reader: &objc2::rc::Retained<objc2_av_foundation::AVAssetReader>,
    target: UploadTargetId,
    upload: &TextureFrameSender,
    control_rx: &Receiver<VideoControl>,
    fps: f32,
    mut frame_dur: Duration,
    state: &mut WorkerState,
) -> PassOutcome {
    use objc2_av_foundation::AVAssetReaderStatus;
    use objc2_core_video::{
        CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
        CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
        CVPixelBufferUnlockBaseAddress, kCVReturnSuccess,
    };

    loop {
        // --- non-blocking control poll ---
        match control_rx.try_recv() {
            Ok(VideoControl::Stop) => return PassOutcome::Stop,
            Ok(VideoControl::Pause) => {
                let (speed, loop_seamless) = match state {
                    WorkerState::Playing {
                        speed,
                        loop_seamless,
                    } => (*speed, *loop_seamless),
                    _ => (1.0, true),
                };
                *state = WorkerState::Paused {
                    speed,
                    loop_seamless,
                };
                return PassOutcome::Paused;
            }
            Ok(VideoControl::SetSpeed(s)) => {
                if let WorkerState::Playing { speed, .. } = state {
                    *speed = s;
                    // Recalculate frame pacing in-place so the current pass
                    // continues without seeking back to t=0 (which would happen
                    // if we returned ControlUpdated and rebuilt the reader).
                    let effective = s.max(0.01_f32);
                    frame_dur = Duration::from_secs_f64(1.0 / (fps as f64 * effective as f64));
                }
                // Stay in the decode loop — no seek, no reader rebuild.
            }
            Ok(VideoControl::SetLoop(l)) => {
                if let WorkerState::Playing { loop_seamless, .. } = state {
                    *loop_seamless = l;
                }
                // Loop flag change takes effect at next EOF; continue current pass.
            }
            Ok(VideoControl::Play) => {} // already playing
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                return PassOutcome::Stop;
            }
        }

        // --- pull next sample buffer from the first output ---
        let sample_buf = unsafe {
            let outputs = reader.outputs();
            let output = outputs
                .firstObject()
                .expect("reader must have at least one output");
            output.copyNextSampleBuffer()
        };

        match sample_buf {
            None => {
                // EOF or error.
                let status = unsafe { reader.status() };
                if status == AVAssetReaderStatus::Completed {
                    let loop_seamless = match state {
                        WorkerState::Playing { loop_seamless, .. } => *loop_seamless,
                        _ => false,
                    };
                    if loop_seamless {
                        return PassOutcome::Eof; // caller rebuilds reader
                    } else {
                        let (speed, loop_seamless) = match state {
                            WorkerState::Playing {
                                speed,
                                loop_seamless,
                            } => (*speed, *loop_seamless),
                            _ => (1.0, false),
                        };
                        *state = WorkerState::Paused {
                            speed,
                            loop_seamless,
                        };
                        return PassOutcome::Paused;
                    }
                } else {
                    tracing::error!(
                        target: "rmap::video",
                        status = ?status,
                        "video worker: copyNextSampleBuffer returned None (error status)",
                    );
                    *state = WorkerState::Dead;
                    return PassOutcome::Dead;
                }
            }
            Some(sbuf) => {
                // --- extract pixel bytes (panic-safe) ---
                let frame_pushed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || -> Option<crate::render::texture_upload::TextureFrame> {
                        unsafe {
                            // image_buffer() requires feature = "objc2-core-video"
                            // on objc2-core-media (declared in Cargo.toml).
                            // CVPixelBuffer == CVImageBuffer (type alias in core-video).
                            let img_buf: objc2_core_foundation::CFRetained<
                                objc2_core_video::CVImageBuffer,
                            > = sbuf.image_buffer()?;
                            let pixel_buf: &objc2_core_video::CVImageBuffer = img_buf.as_ref();

                            let rc =
                                CVPixelBufferLockBaseAddress(pixel_buf, CVPixelBufferLockFlags(0));
                            if rc != kCVReturnSuccess {
                                tracing::warn!(
                                    target: "rmap::video",
                                    rc,
                                    "CVPixelBufferLockBaseAddress failed — skipping frame",
                                );
                                return None;
                            }

                            let width = CVPixelBufferGetWidth(pixel_buf) as u32;
                            let height = CVPixelBufferGetHeight(pixel_buf) as u32;
                            let stride = CVPixelBufferGetBytesPerRow(pixel_buf);
                            let base = CVPixelBufferGetBaseAddress(pixel_buf);

                            let result = if !base.is_null() && width > 0 && height > 0 {
                                // Stride may be wider than width*4 (padding).
                                // Copy only the visible pixels row-by-row.
                                let row_bytes = (width as usize) * 4;
                                let mut pixels = Vec::with_capacity(row_bytes * height as usize);
                                for y in 0..height as usize {
                                    let row_ptr = (base as *const u8).add(y * stride);
                                    pixels.extend_from_slice(std::slice::from_raw_parts(
                                        row_ptr, row_bytes,
                                    ));
                                }
                                let ts = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_nanos();
                                Some(crate::render::texture_upload::TextureFrame {
                                    target,
                                    pixels: pixels.into_boxed_slice(),
                                    width,
                                    height,
                                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                                    timestamp_nanos: ts,
                                })
                            } else {
                                None
                            };

                            CVPixelBufferUnlockBaseAddress(pixel_buf, CVPixelBufferLockFlags(0));
                            result
                        }
                    },
                ));

                match frame_pushed {
                    Err(panic_val) => {
                        tracing::error!(
                            target: "rmap::video",
                            "video worker: panic extracting frame: {:?}", panic_val,
                        );
                        *state = WorkerState::Dead;
                        return PassOutcome::Dead;
                    }
                    Ok(Some(frame)) => {
                        upload.try_send(frame);
                    }
                    Ok(None) => {} // zero-dim frame or null base — skip silently
                }

                // Frame pacing: sleep based on nominal FPS × playback speed.
                std::thread::sleep(frame_dur);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Stub implementation (no `video` feature or non-macOS)
// ---------------------------------------------------------------------------

#[cfg(not(all(feature = "video", target_os = "macos")))]
fn worker_loop(
    _path: PathBuf,
    _target: UploadTargetId,
    _upload: TextureFrameSender,
    control_rx: Receiver<VideoControl>,
) {
    // Part 1 stub: block on the control channel; exit on Stop or disconnect.
    tracing::info!(
        target: "rmap::video",
        "video worker stub started (no decoder; Part 2 wires AVFoundation)",
    );
    loop {
        match control_rx.recv() {
            Ok(VideoControl::Stop) => break,
            Ok(_) => continue, // Play/Pause/SetSpeed/SetLoop: no-op in stub.
            Err(_) => break,   // Sender dropped — the LayerState was removed.
        }
    }
    tracing::info!(target: "rmap::video", "video worker stub exiting");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::render::texture_upload::TextureUploadQueue;

    /// Worker spawns cleanly and exits within 2 s after Stop, regardless of
    /// whether the video feature is active (file does not exist — the worker
    /// enters the dead state and still exits on Stop).
    #[test]
    fn worker_exits_on_stop() {
        let q = TextureUploadQueue::new();
        let sender = q.sender();
        let target = UploadTargetId(9999);
        let path = std::path::PathBuf::from("/nonexistent/test.mp4");

        let (handle, control_tx) = spawn(path, target, sender);

        // Give the worker a moment to start up, then send Stop.
        std::thread::sleep(Duration::from_millis(50));
        control_tx.send(VideoControl::Stop).expect("send Stop");

        let deadline = std::time::Instant::now() + Duration::from_millis(2000);
        loop {
            if handle.is_finished() {
                return;
            }
            if std::time::Instant::now() >= deadline {
                panic!("video worker thread did not exit within 2 s after Stop");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Dropping the control sender (channel disconnect) causes the worker to exit.
    #[test]
    fn worker_exits_on_sender_drop() {
        let q = TextureUploadQueue::new();
        let sender = q.sender();
        let target = UploadTargetId(9998);
        let path = std::path::PathBuf::from("/nonexistent/test2.mp4");

        let (handle, control_tx) = spawn(path, target, sender);
        std::thread::sleep(Duration::from_millis(50));
        drop(control_tx);

        let deadline = std::time::Instant::now() + Duration::from_millis(2000);
        loop {
            if handle.is_finished() {
                return;
            }
            if std::time::Instant::now() >= deadline {
                panic!("video worker thread did not exit within 2 s after sender drop");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// `natural_size` on a missing file returns `None` (does not panic or block).
    ///
    /// Tests the error path without requiring a real fixture. The AVURLAsset
    /// construction is lazy so a non-existent file probes the track list and
    /// finds nothing (or fails at startReading time in the worker — both are
    /// acceptable outcomes that result in None / Dead-state).
    #[cfg(all(feature = "video", target_os = "macos"))]
    #[test]
    fn natural_size_missing_file_returns_none() {
        let result = super::natural_size(std::path::Path::new("/nonexistent/test.mp4"));
        assert!(
            result.is_none(),
            "expected None for nonexistent asset, got {:?}",
            result
        );
    }

    /// P1.4.0 — auto-play on spawn (no explicit Play required).
    ///
    /// The Phase 1 acceptance criterion says an operator can drop an mp4
    /// and see it play within one click. The drag-drop path spawns a
    /// worker and never sends `VideoControl::Play`; the worker must
    /// therefore enter the decode loop on its own. This test spawns
    /// against a real fixture (when present) and asserts ≥1 frame lands
    /// on the upload queue within ~1 s without anyone touching the
    /// control channel.
    ///
    /// Skipped unless `tests/fixtures/test.mp4` is present (same fixture
    /// gate as `natural_size_fixture_if_present`; the fixture lands in
    /// P1.7.4). Without the fixture this contract is verified by code
    /// review — `worker_loop` initialises `state = WorkerState::Playing`
    /// rather than `Paused`.
    #[cfg(all(feature = "video", target_os = "macos"))]
    #[test]
    fn auto_plays_on_spawn_without_explicit_play() {
        let fixture = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/test.mp4"
        ));
        if !fixture.exists() {
            // No fixture committed — skip. Land P1.7.4 to exercise this.
            return;
        }

        let q = TextureUploadQueue::new();
        let sender = q.sender();
        let target = UploadTargetId(7777);
        let (handle, control_tx) = spawn(fixture.to_path_buf(), target, sender);

        // Poll the queue for up to ~1 s. The worker must produce frames
        // entirely from its own initial Playing state — we never send Play.
        let deadline = std::time::Instant::now() + Duration::from_millis(1000);
        let mut out = Vec::new();
        let saw_frame = loop {
            q.drain_into(&mut out);
            if !out.is_empty() {
                break true;
            }
            if std::time::Instant::now() >= deadline {
                break false;
            }
            std::thread::sleep(Duration::from_millis(10));
        };

        // Shut the worker down regardless of outcome.
        let _ = control_tx.send(VideoControl::Stop);
        let _ = handle.join();

        assert!(
            saw_frame,
            "video worker did not push a frame within 1 s of spawn — \
             auto-play regression (worker_loop's initial state must be Playing)",
        );
    }

    /// `natural_size` integration test.
    ///
    /// Skipped unless `tests/fixtures/test.mp4` is present — see the note in
    /// P0.4.2b task docs. To run this: place any 1-second H.264/mp4 at that
    /// path.  The test asserts that dims are non-zero.
    ///
    /// ffmpeg example:
    ///   ffmpeg -f lavfi -i color=c=blue:size=320x240:duration=1 -c:v libx264 \
    ///          tests/fixtures/test.mp4
    #[cfg(all(feature = "video", target_os = "macos"))]
    #[test]
    fn natural_size_fixture_if_present() {
        let fixture = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/test.mp4"
        ));
        if !fixture.exists() {
            // No fixture committed — skip.
            return;
        }
        let dims = super::natural_size(fixture);
        assert!(
            dims.is_some(),
            "natural_size returned None for an existing fixture mp4",
        );
        let (w, h) = dims.unwrap();
        assert!(
            w > 0 && h > 0,
            "natural_size returned zero dimensions: {}x{}",
            w,
            h
        );
    }
}
