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
}

fn main() -> anyhow::Result<()> {
    init_tracing();
    let cli = Cli::parse();
    App::run(cli.project, cli.autostart)?;
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
