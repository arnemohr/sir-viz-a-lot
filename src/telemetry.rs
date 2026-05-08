//! 003-T1.47 — `ux_metrics` daily JSON sink.
//!
//! Subscribes to `tracing` events with `target = "rmap::ux"` (T1.45 /
//! T1.46 emitters) and writes each one as a JSON line to
//! `~/Library/Logs/rmap/ux_metrics_<date>.json`. Daily rotation matches
//! the existing `rmap.log` rotation set up by `init_tracing` in
//! `main.rs`.
//!
//! # Privacy contract (Plan §11.12)
//!
//! Events SHALL NOT carry user payload — no filenames, paths, layer
//! ids, or message text. The emitters in T1.45 / T1.46 only ship the
//! event name + a small set of structured numeric / enum fields
//! (severity, did). The privacy review (T0.6) checklist expects each
//! line to round-trip cleanly through `serde_json::from_str` with
//! only this restricted vocabulary; an emitter that ships a path
//! breaks the contract and surfaces immediately in the daily file.
//!
//! # Layer composition
//!
//! `ux_metrics_layer()` builds a `tracing_subscriber` Layer that:
//! 1. Filters per-event with `filter::filter_fn` so only
//!    `target == "rmap::ux"` reaches the sink.
//! 2. Formats each event as one JSON object per line via the
//!    `fmt::layer().json()` builder.
//! 3. Writes to a `tracing_appender::rolling::daily` writer rotating
//!    on `ux_metrics.json` (the appender suffixes the date).
//!
//! `init` returns `(Layer, WorkerGuard)`. The guard must be held by
//! the caller (`main.rs::init_tracing`) for the lifetime of the
//! process — dropping it stops the background writer thread.
//!
//! # Failure mode
//!
//! If `~/Library/Logs/rmap` can't be created, this module returns
//! `None` — the rest of tracing keeps working, just without the
//! UX sink. The user can still inspect events via `RUST_LOG=rmap=info`
//! on stderr / the main rmap.log file.

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::Layer;
use tracing_subscriber::filter;
use tracing_subscriber::fmt;
use tracing_subscriber::registry::LookupSpan;

/// Build the UX-metrics tracing layer + its background-writer guard.
/// Returns `None` if the log directory can't be created — caller
/// should keep the rest of the tracing pipeline working without it.
///
/// The returned `Box<dyn Layer<...>>` is type-erased so the caller in
/// `main.rs` can `.with(...)` it onto the registry alongside the
/// existing console + file layers without re-stating the JSON-output
/// generic mess.
///
/// `dir` is typically `~/Library/Logs/rmap` (computed by `main.rs`);
/// passing it in keeps this module agnostic of platform path
/// conventions.
pub fn ux_metrics_layer<S>(dir: &PathBuf) -> Option<(Box<dyn Layer<S> + Send + Sync>, WorkerGuard)>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    if !dir.exists() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!(
                "warning: could not create ux_metrics dir {}: {e}; UX sink disabled",
                dir.display()
            );
            return None;
        }
    }
    let appender = tracing_appender::rolling::daily(dir, "ux_metrics.json");
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let layer = fmt::layer()
        .json()
        // Compact event format: timestamp + fields, no span hierarchy
        // or thread metadata in the payload (those are debugging aids
        // we don't need in the privacy-reviewed sink).
        .with_target(true)
        .with_level(false)
        .with_thread_names(false)
        .with_thread_ids(false)
        .with_writer(writer)
        .with_ansi(false)
        // Only events whose `metadata.target()` is "rmap::ux" reach
        // this sink. The existing rmap.log + stderr layers continue
        // to receive *all* events.
        .with_filter(filter::filter_fn(|metadata| {
            metadata.target() == "rmap::ux"
        }));

    Some((Box::new(layer), guard))
}
