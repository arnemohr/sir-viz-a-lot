//! `rmap` — projection mapping tool entry point.
//!
//! See `specs/001-initial-setup.md`. This file stays thin; the interesting
//! wiring lives in `app::App`.

mod app;
mod clock;
mod controls;
mod effects;
mod error;
mod image_layer;
#[cfg(feature = "lighting")]
mod lighting;
#[cfg(target_os = "macos")]
mod macos;
mod modulators;
mod monitors;
mod project;
mod render;
mod show_day;
mod svg_layer;
#[cfg(feature = "v3")]
mod telemetry;
mod test_patterns;
mod video_layer;
mod windows;

use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::Parser;

use crate::app::App;

#[derive(Debug, Parser)]
#[command(name = "rmap", version, about = "Minimal projection mapping tool")]
struct Cli {
    /// Project file: `*.rmap.json` (full show) or a single `*.svg` layer bootstrap.
    project: Option<PathBuf>,

    /// With a `*.rmap.json` argument: load that project and use its monitor index
    /// unless `--monitor` is set (no extra startup gate in this build).
    #[arg(long)]
    autostart: bool,

    /// Print the list of monitors winit reports and exit. Useful for
    /// figuring out which `--monitor INDEX` to pass.
    #[arg(long = "list-monitors")]
    list_monitors: bool,

    /// Index of the monitor to open the output window on. Defaults to 0
    /// (or the saved index from `Project.output_monitor_index` once
    /// T-M6-04 lands). CLI takes precedence over the project file.
    #[arg(long = "monitor", value_name = "INDEX")]
    monitor: Option<usize>,

    /// Draw output in a normal window (1280×720) on the chosen monitor instead of borderless fullscreen.
    #[arg(long, conflicts_with = "fullscreen")]
    windowed: bool,

    /// Force borderless fullscreen output (overrides `--windowed` and `output_windowed` in the project file).
    #[arg(long, conflicts_with = "windowed")]
    fullscreen: bool,
}

fn main() -> anyhow::Result<()> {
    let _log_guard = init_tracing();
    let cli = Cli::parse();

    if cli.list_monitors {
        App::print_monitors()?;
        return Ok(());
    }

    tracing::info!(
        project = ?cli.project,
        autostart = cli.autostart,
        monitor = ?cli.monitor,
        windowed = cli.windowed,
        fullscreen = cli.fullscreen,
        "rmap starting",
    );
    App::run(
        cli.project.clone(),
        cli.autostart,
        cli.monitor,
        cli.windowed,
        cli.fullscreen,
    )
    .with_context(|| {
        format!(
            "project={:?}, autostart={}, monitor={:?}, windowed={}, fullscreen={}",
            cli.project, cli.autostart, cli.monitor, cli.windowed, cli.fullscreen
        )
    })?;
    Ok(())
}

fn init_tracing() -> Vec<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("rmap=info,wgpu=warn,naga=warn"));

    // Resolve ~/Library/Logs/rmap/ via $HOME. On macOS HOME is always set,
    // but we degrade gracefully to a stderr-backed non-blocking writer so
    // that logging-setup failure never prevents rmap from starting.
    let log_dir: Option<PathBuf> = match std::env::var("HOME") {
        Ok(home) => {
            let dir = Path::new(&home).join("Library/Logs/rmap");
            match std::fs::create_dir_all(&dir) {
                Ok(()) => Some(dir),
                Err(e) => {
                    eprintln!(
                        "warning: could not create log dir {}: {e}; file logging disabled",
                        dir.display()
                    );
                    None
                }
            }
        }
        Err(_) => {
            eprintln!("warning: $HOME not set; file logging disabled");
            None
        }
    };

    let (file_writer, file_guard) = if let Some(ref dir) = log_dir {
        let file_appender = tracing_appender::rolling::daily(dir, "rmap.log");
        tracing_appender::non_blocking(file_appender)
    } else {
        tracing_appender::non_blocking(std::io::stderr())
    };

    // 003-T1.47 — UX-metrics JSON sink. Filtered to target = "rmap::ux"
    // so only the privacy-reviewed Plan §11.7 telemetry events land in
    // ux_metrics_<date>.json. Returns None if the log dir is missing —
    // we keep the main subscriber pipeline running either way.
    #[cfg(feature = "v3")]
    let (ux_layer, ux_guard) = match log_dir.as_ref() {
        Some(dir) => match crate::telemetry::ux_metrics_layer(dir) {
            Some((layer, guard)) => (Some(layer), Some(guard)),
            None => (None, None),
        },
        None => (None, None),
    };

    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(fmt::layer().with_writer(file_writer).with_ansi(false));
    #[cfg(feature = "v3")]
    let registry = registry.with(ux_layer);
    registry.init();

    tracing::info!(?log_dir, "logging initialized");

    #[cfg_attr(not(feature = "v3"), allow(unused_mut))]
    let mut guards = vec![file_guard];
    #[cfg(feature = "v3")]
    if let Some(g) = ux_guard {
        guards.push(g);
    }
    guards
}
