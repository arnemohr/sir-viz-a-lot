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
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::monitor::MonitorHandle;
use winit::window::WindowId;

use crate::error::{Result, RmapError};
use crate::render::{GpuContext, RenderError, Renderer};
use crate::show_day::sleep_assertion::SleepAssertion;
use crate::test_patterns::{TestPattern, TestPatternRenderer};
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
    /// Operator-supplied `--monitor INDEX` override.
    ///
    /// Interim v1 path: this is the only way to point the output at a
    /// non-default monitor until the egui dropdown (T-M4-15) and the saved
    /// `Project.output_monitor_index` (T-M6-04) land. CLI value takes
    /// precedence over both. `None` here falls back to monitor 0.
    monitor_override: Option<usize>,
    /// Lazily-initialised GPU + window state.
    state: Option<RunningApp>,
}

/// Bundle of resources that exist only after `resumed`: the output window,
/// the renderer (which owns the [`GpuContext`]), the test-pattern renderer,
/// and the IOPMAssertion preventing display sleep.
struct RunningApp {
    output: OutputWindow,
    renderer: Renderer,
    test_patterns: TestPatternRenderer,
    /// Held for the lifetime of the running app — `Drop` releases the
    /// `IOPMAssertion` on macOS, no-op elsewhere. Spec §6 display-sleep
    /// prevention. Field is read only by `Drop`; underscore prefix
    /// silences the unused-field lint.
    _sleep_assertion: SleepAssertion,
}

impl App {
    pub fn run(
        project: Option<PathBuf>,
        autostart: bool,
        monitor_index: Option<usize>,
    ) -> Result<()> {
        let event_loop =
            EventLoop::new().map_err(|e| RmapError::Other(format!("event loop: {e}")))?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = App {
            project,
            autostart,
            monitor_override: monitor_index,
            state: None,
        };

        event_loop
            .run_app(&mut app)
            .map_err(|e| RmapError::Other(format!("run_app: {e}")))?;

        Ok(())
    }

    /// Enumerate winit-reported monitors and print them to stdout, then exit.
    ///
    /// Operator-facing CLI output (driven by `--list-monitors`) — uses
    /// `println!` rather than `tracing::info!` because the user typed a
    /// command and expects its output on stdout, not in log telemetry.
    ///
    /// Implementation note: winit 0.30 only exposes `available_monitors()` on
    /// `ActiveEventLoop`, so we have to spin up a real `EventLoop` and
    /// drive it via `ApplicationHandler::resumed`. We exit immediately —
    /// no window or GPU device is created.
    pub fn print_monitors() -> Result<()> {
        let event_loop =
            EventLoop::new().map_err(|e| RmapError::Other(format!("event loop: {e}")))?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut handler = ListMonitorsApp { printed: false };
        event_loop
            .run_app(&mut handler)
            .map_err(|e| RmapError::Other(format!("run_app: {e}")))?;
        Ok(())
    }
}

/// One-shot `ApplicationHandler` that prints the monitor list on the first
/// `resumed` callback and exits the loop. Does not open a window or
/// initialize wgpu.
struct ListMonitorsApp {
    printed: bool,
}

impl ApplicationHandler for ListMonitorsApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.printed {
            return;
        }
        self.printed = true;

        let monitors = crate::monitors::list(event_loop);
        println!("Monitors detected by winit (use --monitor INDEX to pick):");
        if monitors.is_empty() {
            println!("  (none reported)");
        } else {
            for m in &monitors {
                println!(
                    "  {}  {:?}   {}x{} @ ({},{})    scale {:.2}",
                    m.index, m.name, m.size.0, m.size.1, m.position.0, m.position.1, m.scale_factor,
                );
            }
        }

        event_loop.exit();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
        // No window is created, so nothing to do here.
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
    // Build the test-pattern pipelines before handing `gpu` to the
    // Renderer (Renderer takes ownership of `gpu`).
    let test_patterns = TestPatternRenderer::new(&gpu.device, surface_format);
    let renderer = Renderer::new(gpu, surface_format)?;
    // Spec §6 display-sleep prevention. Held on RunningApp; released on
    // Drop. Failures are logged inside `acquire` and yield a no-op
    // assertion (degraded mode) rather than aborting.
    let sleep_assertion = SleepAssertion::acquire("rmap output window");
    Ok(RunningApp {
        output,
        renderer,
        test_patterns,
        _sleep_assertion: sleep_assertion,
    })
}

/// Acquire the next surface texture and translate wgpu 29's
/// [`wgpu::CurrentSurfaceTexture`] discriminants into the same
/// `Result<Option<SurfaceTexture>, RenderError>` shape used by
/// [`Renderer::render_frame`]. `Ok(None)` means "drop this frame
/// silently" (Timeout / Occluded); `Ok(Some(_))` means draw.
///
/// Mirrors `Renderer::render_frame` arm-for-arm so the App's recovery
/// arms behave identically regardless of which render path was active
/// when the surface fell over. (A future M3+ refactor can hoist this
/// onto `OutputWindow` as `acquire_frame`; M2 keeps the duplication
/// localized rather than touching `render::mod`.)
fn acquire_frame(
    output: &OutputWindow,
) -> std::result::Result<Option<wgpu::SurfaceTexture>, RenderError> {
    match output.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(f) => Ok(Some(f)),
        wgpu::CurrentSurfaceTexture::Suboptimal(_) => Err(RenderError::SurfaceSuboptimal),
        wgpu::CurrentSurfaceTexture::Timeout => {
            tracing::warn!("surface acquire timed out; dropping frame");
            Ok(None)
        }
        wgpu::CurrentSurfaceTexture::Occluded => {
            tracing::debug!("surface occluded; skipping frame");
            Ok(None)
        }
        wgpu::CurrentSurfaceTexture::Outdated => Err(RenderError::SurfaceOutdated),
        wgpu::CurrentSurfaceTexture::Lost => Err(RenderError::SurfaceLost),
        wgpu::CurrentSurfaceTexture::Validation => Err(RenderError::Surface(
            "surface acquire validation error".into(),
        )),
    }
}

/// Render a single black frame to the output window. Used when the
/// operator hits Blackout (`B`). Wrapped in
/// `panic_restore::run_frame_assert_unwind_safe` so a panic mid-frame
/// converts to `RenderError::RenderPanic` and reaches the App's
/// recovery arm — same contract as `Renderer::render_frame`.
fn render_blackout(
    renderer: &Renderer,
    output: &OutputWindow,
) -> std::result::Result<(), RenderError> {
    crate::show_day::panic_restore::run_frame_assert_unwind_safe(|| {
        let frame = match acquire_frame(output)? {
            Some(f) => f,
            None => return Ok(()),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            renderer
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("blackout encoder"),
                });
        // Single empty render pass with Clear → BLACK. No draw call: the
        // clear *is* the frame.
        let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blackout pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer.gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    })
}

/// Render the chosen test pattern via [`TestPatternRenderer::render`].
/// Same wgpu boilerplate as `render_blackout`; differs only in the
/// inner pass body, which is delegated.
fn render_test_pattern(
    renderer: &Renderer,
    output: &OutputWindow,
    test_patterns: &TestPatternRenderer,
    pattern: TestPattern,
) -> std::result::Result<(), RenderError> {
    crate::show_day::panic_restore::run_frame_assert_unwind_safe(|| {
        let frame = match acquire_frame(output)? {
            Some(f) => f,
            None => return Ok(()),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            renderer
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("test_pattern encoder"),
                });
        test_patterns.render(pattern, &mut encoder, &view);
        renderer.gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    })
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            // macOS can fire `resumed` more than once on lifecycle changes;
            // the first call already brought everything up.
            return;
        }

        // X-06: CLI `--monitor INDEX` is the v1 interim override. T-M6-04
        // will additionally read `Project.output_monitor_index`; T-M4-15
        // adds the egui dropdown. Until then, fall back to monitor 0.
        let _ = (&self.project, self.autostart);
        let monitor_index = self.monitor_override.unwrap_or(0);
        let monitor = event_loop.available_monitors().nth(monitor_index);
        if monitor.is_none() {
            tracing::warn!(
                requested = monitor_index,
                available = event_loop.available_monitors().count(),
                "requested monitor index out of range; falling back to platform default",
            );
        }

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
                event: key_event, ..
            } if key_event.state == ElementState::Pressed => {
                // Use `physical_key` so the bindings are layout-
                // independent: a French-AZERTY operator still hits the
                // same physical keys for Blackout / Freeze / cycle Test
                // Pattern. Letter keys arrive as `Key::Character` (not
                // `Named`), so logical-key matching is not reliable for
                // single letters across layouts.
                match key_event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                    PhysicalKey::Code(KeyCode::KeyB) => {
                        state.output.state.toggle_blackout();
                        tracing::info!(blackout = state.output.state.blackout, "blackout toggled");
                    }
                    PhysicalKey::Code(KeyCode::KeyF) => {
                        state.output.state.toggle_freeze();
                        tracing::info!(freeze = state.output.state.freeze, "freeze toggled");
                    }
                    PhysicalKey::Code(KeyCode::KeyT) => {
                        state.output.state.cycle_test_pattern();
                        tracing::info!(
                            pattern = state.output.state.test_pattern.label(),
                            "test pattern"
                        );
                    }
                    _ => {}
                }
            }
            WindowEvent::Resized(new_size) => {
                state.output.config.width = new_size.width.max(1);
                state.output.config.height = new_size.height.max(1);
                state.output.recreate_surface(&state.renderer.gpu.device);
            }
            WindowEvent::RedrawRequested => {
                // Render-order priority per spec §3.6 / T-M2-09:
                // blackout > freeze > test_pattern > normal.
                // Blackout wins over freeze: if the operator hits B
                // while frozen, the projector goes black immediately
                // rather than continuing to show the frozen frame.
                let result = if state.output.state.blackout {
                    render_blackout(&state.renderer, &state.output)
                } else if state.output.state.freeze {
                    // Freeze: skip rendering entirely. The window keeps
                    // showing its last presented frame because we
                    // never call `frame.present()` again. Pragmatic M2
                    // implementation; a perfect "freeze" would copy
                    // and re-present the last framebuffer every frame.
                    Ok(())
                } else if state.output.state.test_pattern != TestPattern::None {
                    render_test_pattern(
                        &state.renderer,
                        &state.output,
                        &state.test_patterns,
                        state.output.state.test_pattern,
                    )
                } else {
                    state.renderer.render_frame(&state.output)
                };
                match result {
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
                    Err(RenderError::RenderPanic { message }) => {
                        tracing::error!(%message, "renderer panicked; recovered");
                        crate::windows::control::error_overlay(&message);
                    }
                    Err(e) => {
                        tracing::error!(?e, "render error");
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = self.state.as_ref() {
            state.output.window.request_redraw();
        }
    }
}
