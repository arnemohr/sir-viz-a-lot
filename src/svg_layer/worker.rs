//! Off-thread SVG rasterization worker.
//!
//! [`Worker::spawn`] starts a single background thread that receives
//! [`RasterJob`]s, deduplicates stale generations for the same layer, and
//! sends back [`RasterDone`] results.
//!
//! # Rasterization approach
//!
//! Rasterization logic is **inlined** here (rather than factored out of
//! `svg_layer.rs`) because the spec's "What NOT to touch" section forbids
//! modifying `svg_layer.rs` beyond the `pub mod worker;` declaration. The
//! two rasterization paths — `SvgLayer::rasterize` and `rasterize_one` here —
//! are therefore intentional duplicates. They share the same algorithm:
//! 2× oversample via `resvg`, then downsample with `image`'s Triangle filter.
//!
//! # Dedup algorithm
//!
//! On every `recv()` the worker immediately drains any additional pending jobs
//! via `try_recv()`, then keeps only the highest-generation job per
//! [`LayerId`]. Jobs for different layers are all processed. This means that
//! if the App sends several resize/hot-reload jobs for the same layer before
//! the worker picks up the first, only the most recent is rasterized —
//! avoiding redundant work on a slow machine or a burst of events.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::RmapError;

static NEXT_LAYER_ID: AtomicU64 = AtomicU64::new(1);

/// Stable identifier the App assigns to a layer to disambiguate
/// per-layer worker traffic. T-M3-06 will pick how it's allocated
/// (probably the layer's index or a monotonic counter on Project).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(pub u64);

impl LayerId {
    /// Mint a fresh `LayerId` from a monotonically increasing global counter.
    /// Each call returns a unique value that never repeats, preventing stale
    /// `RasterDone` results from a previous layer vector from matching a new
    /// `LayerState` rebuilt with the same index-based numeric ID.
    pub fn next() -> Self {
        Self(NEXT_LAYER_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// A rasterization request sent to the worker thread.
#[derive(Debug, Clone)]
pub struct RasterJob {
    pub layer_id: LayerId,
    pub path: PathBuf,
    pub size: (u32, u32),
    pub generation: u64,
}

/// A completed rasterization result returned by the worker thread.
#[derive(Debug)]
pub struct RasterDone {
    pub layer_id: LayerId,
    pub pixmap: tiny_skia::Pixmap,
    pub generation: u64,
}

/// Unit struct whose associated [`Worker::spawn`] function starts the worker
/// thread and returns the channel endpoints.
pub struct Worker;

impl Worker {
    /// Spawn the background rasterization worker.
    ///
    /// Returns `(job_tx, result_rx)`:
    /// - Send [`RasterJob`]s on `job_tx`.
    /// - Receive [`RasterDone`]s on `result_rx`.
    ///
    /// The worker exits cleanly when `job_tx` (and all clones) are dropped:
    /// `job_rx.recv()` returns `Err(RecvError)` and the thread's loop breaks.
    pub fn spawn() -> (
        crossbeam_channel::Sender<RasterJob>,
        crossbeam_channel::Receiver<RasterDone>,
    ) {
        let (job_tx, job_rx) = crossbeam_channel::unbounded::<RasterJob>();
        let (result_tx, result_rx) = crossbeam_channel::unbounded::<RasterDone>();

        std::thread::spawn(move || {
            // Block until at least one job arrives; exit when all senders are dropped.
            while let Ok(initial) = job_rx.recv() {
                // Drain any additional pending jobs and deduplicate by layer_id,
                // keeping only the highest generation per layer.
                let mut batch: HashMap<LayerId, RasterJob> = HashMap::new();
                batch.insert(initial.layer_id, initial);

                while let Ok(next) = job_rx.try_recv() {
                    batch
                        .entry(next.layer_id)
                        .and_modify(|cur| {
                            if next.generation > cur.generation {
                                *cur = next.clone();
                            }
                        })
                        .or_insert(next);
                }

                // Process the deduped batch (one job per layer_id).
                for (_, job) in batch {
                    match rasterize_one(&job) {
                        Ok(pixmap) => {
                            let done = RasterDone {
                                layer_id: job.layer_id,
                                pixmap,
                                generation: job.generation,
                            };
                            // If the receiver has been dropped, discard silently.
                            let _ = result_tx.send(done);
                        }
                        Err(e) => {
                            tracing::warn!(
                                layer_id = ?job.layer_id,
                                path = %job.path.display(),
                                generation = job.generation,
                                error = %e,
                                "raster worker: skipping failed job"
                            );
                        }
                    }
                }
            }
        });

        (job_tx, result_rx)
    }
}

/// Load and rasterize a single SVG file to the requested pixel size.
///
/// This mirrors the logic in `SvgLayer::load` + `SvgLayer::rasterize` but
/// operates on bare paths and sizes without the `SvgLayer` cache.
///
/// Algorithm: read file → `usvg::Tree::from_str` → 2× oversample via `resvg`
/// (uniform scale, centered letterbox — aspect-preserving) → downsample with
/// `image` Triangle filter → return `tiny_skia::Pixmap`.
fn rasterize_one(job: &RasterJob) -> Result<tiny_skia::Pixmap, RmapError> {
    let (width, height) = job.size;

    // --- Load + parse ---
    let content = std::fs::read_to_string(&job.path)?;
    let tree = usvg::Tree::from_str(&content, &usvg::Options::default())
        .map_err(|e| RmapError::Other(format!("svg parse failed: {e}")))?;

    // --- Effective bounding box ---
    let bbox = {
        let r = tree.root().abs_bounding_box();
        if r.width() > 0.0 && r.height() > 0.0 {
            r
        } else {
            return Err(RmapError::Other(
                "svg has no content to rasterize".to_string(),
            ));
        }
    };

    // --- 2× oversample ---
    let over_w = width.saturating_mul(2).max(1);
    let over_h = height.saturating_mul(2).max(1);

    let mut over = tiny_skia::Pixmap::new(over_w, over_h).ok_or_else(|| {
        RmapError::Other(format!(
            "rasterize failed: could not allocate {over_w}x{over_h} pixmap"
        ))
    })?;

    let transform = super::raster_uniform_fit_transform(&bbox, over_w, over_h);
    resvg::render(&tree, transform, &mut over.as_mut());

    // --- Downsample via `image` ---
    let over_data = over.take();
    let over_buf: image::RgbaImage = image::ImageBuffer::from_vec(over_w, over_h, over_data)
        .ok_or_else(|| {
            RmapError::Other("rasterize failed: oversample buffer size mismatch".to_string())
        })?;

    let small_buf = image::imageops::resize(
        &over_buf,
        width,
        height,
        image::imageops::FilterType::Triangle,
    );

    let small_size = tiny_skia::IntSize::from_wh(width, height).ok_or_else(|| {
        RmapError::Other(format!(
            "rasterize failed: invalid target size {width}x{height}"
        ))
    })?;

    let pixmap =
        tiny_skia::Pixmap::from_vec(small_buf.into_raw(), small_size).ok_or_else(|| {
            RmapError::Other("rasterize failed: downsample buffer size mismatch".to_string())
        })?;

    Ok(pixmap)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crossbeam_channel::RecvTimeoutError;

    use super::*;

    /// Verify that when two jobs for the same [`LayerId`] arrive before the
    /// worker processes them, only the higher-generation job is rasterized.
    ///
    /// # Determinism
    ///
    /// After sending both jobs we sleep 50 ms so the worker thread has time
    /// to wake up, drain the channel (finding both jobs at once), and apply
    /// the dedup. Because crossbeam is an unbounded queue and `send` is
    /// non-blocking, both jobs land in the channel before the sleep expires.
    /// The worker sees them together and drops generation 1 in favor of 2.
    #[test]
    fn stale_generation_dropped() {
        const SVG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40">
  <circle r="10" cx="20" cy="20" fill="red" />
</svg>"#;

        let path = std::env::temp_dir().join("rmap_t-m3-04_stale.svg");
        std::fs::write(&path, SVG).expect("write temp svg");

        let (job_tx, result_rx) = Worker::spawn();

        // Send both jobs back-to-back (no yield between them).
        job_tx
            .send(RasterJob {
                layer_id: LayerId(1),
                path: path.clone(),
                size: (40, 40),
                generation: 1,
            })
            .expect("send job gen=1");

        job_tx
            .send(RasterJob {
                layer_id: LayerId(1),
                path: path.clone(),
                size: (40, 40),
                generation: 2,
            })
            .expect("send job gen=2");

        // Give the worker time to drain both jobs from the channel before
        // processing. 50 ms is far more than a thread wake-up + two try_recv
        // calls need on any CI machine; the dedup therefore sees gen=1 and
        // gen=2 together and drops gen=1.
        std::thread::sleep(Duration::from_millis(50));

        // Drop the sender so the worker exits cleanly after finishing.
        drop(job_tx);

        // Collect all results until the channel closes or a timeout fires.
        let mut results: Vec<RasterDone> = Vec::new();
        loop {
            match result_rx.recv_timeout(Duration::from_secs(2)) {
                Ok(done) => results.push(done),
                Err(RecvTimeoutError::Timeout) => {
                    panic!("recv timed out — worker may be stuck");
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        // Best-effort cleanup.
        let _ = std::fs::remove_file(&path);

        assert_eq!(results.len(), 1, "expected exactly one RasterDone");
        let r = &results[0];
        assert_eq!(r.layer_id, LayerId(1), "layer_id must be LayerId(1)");
        assert_eq!(r.generation, 2, "only gen=2 must survive dedup");
    }
}
