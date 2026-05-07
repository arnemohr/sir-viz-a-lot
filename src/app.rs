//! Top-level application. Owns the winit event loop and holds references to
//! the output window (on the projector) and the egui control window (on the
//! primary display).

use std::path::PathBuf;

use crate::error::Result;

pub struct App {
    // TODO(M1): wgpu instance, surfaces, ApplicationHandler state.
}

impl App {
    pub fn run(_project: Option<PathBuf>, _autostart: bool) -> Result<()> {
        // TODO(M1): build the EventLoop, register an ApplicationHandler that
        //   - opens a borderless fullscreen output window on the chosen monitor
        //   - hosts an egui control window on the primary display
        //   - drives the per-frame render pipeline (see render/mod.rs)
        tracing::info!("rmap skeleton — App::run is not yet implemented");
        Ok(())
    }
}
