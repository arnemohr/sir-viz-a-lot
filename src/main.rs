//! `rmap` — projection mapping tool entry point.
//!
//! See `specs/001-initial-setup.md`. This file stays thin; the interesting
//! wiring lives in `app::App`.

// Skeleton-stage allows. Tighten once milestones fill in: the codebase
// should not retain these blanket allows past M5.
#![allow(dead_code, unused_imports)]

mod app;
mod clock;
mod controls;
mod effects;
mod error;
mod modulators;
mod monitors;
mod project;
mod render;
mod show_day;
mod svg_layer;
mod test_patterns;
mod windows;

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;

use crate::app::App;

#[derive(Debug, Parser)]
#[command(name = "rmap", version, about = "Minimal projection mapping tool")]
struct Cli {
    /// Project file to load (`*.rmap.json`)
    project: Option<PathBuf>,

    /// Open the saved output window automatically on startup
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
}

fn main() -> anyhow::Result<()> {
    init_tracing();
    let cli = Cli::parse();

    if cli.list_monitors {
        App::print_monitors()?;
        return Ok(());
    }

    tracing::info!(
        project = ?cli.project,
        autostart = cli.autostart,
        monitor = ?cli.monitor,
        "rmap starting",
    );
    App::run(cli.project.clone(), cli.autostart, cli.monitor).with_context(|| {
        format!(
            "project={:?}, autostart={}, monitor={:?}",
            cli.project, cli.autostart, cli.monitor
        )
    })?;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("rmap=info,wgpu=warn,naga=warn"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .init();

    // TODO(M2): add `tracing_appender::rolling::daily` writing to
    // ~/Library/Logs/rmap/. Keep the stderr layer above for dev.
}
