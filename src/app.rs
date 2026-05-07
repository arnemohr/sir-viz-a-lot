//! Top-level application. Owns the winit event loop and holds references to
//! the output window (on the projector) and the egui control window (on the
//! primary display).
//!
//! T-M1-04 wires the bare M1 path: `EventLoop` → `ApplicationHandler::resumed`
//! brings up `GpuContext`, opens the borderless fullscreen [`OutputWindow`]
//! on monitor index 0, then constructs the [`Renderer`]. `window_event`
//! handles `CloseRequested`, Esc, `Resized` (re-configure surface), and
//! `RedrawRequested` (call into the renderer). `about_to_wait` requests
//! continuous redraws so we render at the display's vsync rate.
//!
//! Out of scope for M1: scene-recall hotkeys, blackout/freeze, the egui
//! control window, `--autostart` driving project load, surface-error
//! recovery beyond simple resize. T-M1-05 owns surface recovery; T-M2-09
//! owns B/F/T keys; T-M4-14 opens the control window; T-M6-04 wires
//! `--autostart` to project load.

use std::path::PathBuf;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::monitor::MonitorHandle;
use winit::window::WindowId;

use crate::error::{Result, RmapError};
use crate::render::{GpuContext, RenderError, Renderer};
use crate::windows::output::OutputWindow;

/// Application root. Holds the persistent state across event-loop iterations.
///
/// `state` is `None` until the first `resumed` callback. macOS may fire
/// `resumed` more than once over the lifecycle (e.g. after suspend); the
/// handler guards against re-init.
pub struct App {
    /// Project path from CLI. Currently only stored so future tasks
    /// (T-M6-04) can load it; no behaviour depends on it at M1.
    project: Option<PathBuf>,
    /// `--autostart` from CLI. Stored, not acted upon, at M1; T-M6-04
    /// turns this on for real.
    autostart: bool,
    /// Lazily-initialised GPU + window state.
    state: Option<RunningApp>,
}

/// Bundle of resources that exist only after `resumed`: the output window
/// and the renderer (which owns the [`GpuContext`]).
struct RunningApp {
    output: OutputWindow,
    renderer: Renderer,
}

impl App {
    pub fn run(project: Option<PathBuf>, autostart: bool) -> Result<()> {
        let event_loop =
            EventLoop::new().map_err(|e| RmapError::Other(format!("event loop: {e}")))?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = App {
            project,
            autostart,
            state: None,
        };

        event_loop
            .run_app(&mut app)
            .map_err(|e| RmapError::Other(format!("run_app: {e}")))?;

        Ok(())
    }
}

/// Bring up the GPU and the output window. Pulled into a free function so
/// the error path in `resumed` can `?` cleanly.
fn init_running_app(
    event_loop: &ActiveEventLoop,
    monitor: Option<MonitorHandle>,
) -> Result<RunningApp> {
    let gpu = GpuContext::new()?;
    let output = OutputWindow::new(
        event_loop,
        monitor,
        &gpu.instance,
        &gpu.adapter,
        &gpu.device,
    )?;
    let surface_format = output.config.format;
    let renderer = Renderer::new(gpu, surface_format)?;
    Ok(RunningApp { output, renderer })
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            // macOS can fire `resumed` more than once on lifecycle changes;
            // the first call already brought everything up.
            return;
        }

        // T-M6-04 will replace this with the saved index from a loaded
        // Project. At M1 we have not loaded one, so use the first monitor.
        let _ = (&self.project, self.autostart);
        let monitor_index = 0_usize;
        let monitor = event_loop.available_monitors().nth(monitor_index);

        match init_running_app(event_loop, monitor) {
            Ok(running) => {
                self.state = Some(running);
            }
            Err(e) => {
                tracing::error!(?e, "init failed; exiting");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        // T-M4-14 introduces a second window (egui control). Make sure we
        // only act on events for the output window.
        if window_id != state.output.window.id() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                state.output.config.width = new_size.width.max(1);
                state.output.config.height = new_size.height.max(1);
                state.output.recreate_surface(&state.renderer.gpu.device);
            }
            WindowEvent::RedrawRequested => match state.renderer.render_frame(&state.output) {
                Ok(()) => {}
                Err(RenderError::SurfaceLost) => {
                    tracing::warn!("surface lost; recreating");
                    state.output.recreate_surface(&state.renderer.gpu.device);
                }
                Err(RenderError::SurfaceOutdated) => {
                    tracing::warn!("surface outdated; recreating");
                    state.output.recreate_surface(&state.renderer.gpu.device);
                }
                Err(RenderError::SurfaceSuboptimal) => {
                    tracing::warn!("surface suboptimal; recreating");
                    state.output.recreate_surface(&state.renderer.gpu.device);
                }
                Err(e) => {
                    tracing::error!(?e, "render error");
                }
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = self.state.as_ref() {
            state.output.window.request_redraw();
        }
    }
}
