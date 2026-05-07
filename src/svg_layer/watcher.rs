//! Hot-reload file watcher. Thin wrapper around
//! `notify_debouncer_full::new_debouncer(...)` with a 250 ms debounce
//! window so an editor's multi-event save (write + chmod + rename
//! sequence) collapses into a single `WatchEvent`. Spec §1 SVG loading.
//!
//! T-M3-04's worker pulls events off the same crossbeam-channel pattern;
//! T-M3-06 will wire watcher events into the App's per-frame drain.
//! T-M3-07 will assert the debouncer collapses 3 fast events into 1.

use std::path::PathBuf;
use std::time::Duration;

use crossbeam_channel::Receiver;

/// A coalesced file-system event for a watched SVG path. The wrapper
/// flattens `notify`'s richer event types into "the file at `path`
/// changed; consider it dirty".
#[derive(Debug, Clone)]
pub struct WatchEvent {
    pub path: PathBuf,
}

/// Owns the debouncer + the file-watcher thread internally; drop the
/// `Watcher` to stop watching and close the event channel.
pub struct Watcher {
    // Holds the debouncer so it isn't dropped early. `notify_debouncer_full`
    // returns a `Debouncer<RecommendedWatcher, RecommendedCache>` that owns
    // the background watcher thread internally.
    //
    // Type names verified against notify-debouncer-full 0.7.0 source:
    // - `notify_debouncer_full::notify::RecommendedWatcher` is the
    //   cross-platform watcher (FSEvents on macOS, inotify on Linux, etc.).
    //   `notify` is not a direct dependency of this crate; it is accessed
    //   via `notify_debouncer_full`'s re-export.
    // - `notify_debouncer_full::RecommendedCache` is the built-in file-ID
    //   cache (re-exported from `cache.rs`; was called `FileIdMap` in
    //   earlier 0.x prereleases).
    _debouncer: notify_debouncer_full::Debouncer<
        notify_debouncer_full::notify::RecommendedWatcher,
        notify_debouncer_full::RecommendedCache,
    >,
}

impl Watcher {
    /// Create a new debounced watcher over the given paths. Returns the
    /// `Watcher` (must be held to keep watching) and a `Receiver` of
    /// coalesced [`WatchEvent`]s.
    ///
    /// Debounce window is 250 ms (spec §1 — designers' multi-event saves
    /// from Illustrator etc. collapse into one WatchEvent).
    pub fn new(paths: &[PathBuf]) -> crate::error::Result<(Self, Receiver<WatchEvent>)> {
        let (tx, rx) = crossbeam_channel::unbounded::<WatchEvent>();

        // `new_debouncer` signature in 0.7:
        //   fn new_debouncer<F>(timeout, tick_rate: Option<Duration>, event_handler: F)
        //     -> Result<Debouncer<RecommendedWatcher, RecommendedCache>, Error>
        //
        // `tick_rate: None` lets the library choose 1/4 of the timeout (62 ms
        // for our 250 ms window), which is appropriate for interactive use.
        let mut debouncer = notify_debouncer_full::new_debouncer(
            Duration::from_millis(250),
            None,
            move |result: notify_debouncer_full::DebounceEventResult| match result {
                Ok(events) => {
                    for ev in events {
                        for path in &ev.paths {
                            let _ = tx.send(WatchEvent { path: path.clone() });
                        }
                    }
                }
                Err(errs) => {
                    for e in errs {
                        tracing::warn!(error = %e, "notify watcher error");
                    }
                }
            },
        )
        .map_err(|e| crate::error::RmapError::Other(format!("notify debouncer init: {e}")))?;

        for path in paths {
            debouncer
                .watch(
                    path,
                    notify_debouncer_full::notify::RecursiveMode::NonRecursive,
                )
                .map_err(|e| {
                    crate::error::RmapError::Other(format!("notify watch {}: {e}", path.display()))
                })?;
        }

        Ok((
            Self {
                _debouncer: debouncer,
            },
            rx,
        ))
    }
}
