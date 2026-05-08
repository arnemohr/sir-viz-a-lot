//! Launcher window — first window the operator sees on a fresh launch.
//!
//! Peer of [`OutputWindow`](crate::windows::output::OutputWindow) and
//! [`ControlWindow`](crate::windows::control::ControlWindow): owns its own
//! winit `Window` + `wgpu::Surface`, shares the GPU device with the rest of
//! the app, and renders egui inside that surface. Sized 600 × 400 and
//! centred on the primary display.
//!
//! 003-T2.1 stands up the empty shell with placeholder content. Subsequent
//! Phase-2 tasks fill the body:
//!
//! - 003-T2.2 wraps it in `LauncherState` and routes it through `AppState`.
//! - 003-T2.3 wires `Command::Launch` so a button click transitions to
//!   `AppState::Editing`.
//! - 003-T2.4 paints the three start buttons.
//! - 003-T2.5 / T2.6 add the projector picker and Test button.
//!
//! `CloseRequested` on the launcher window is owned by `App::window_event`
//! (matches `LauncherWindow::id()` and routes to `event_loop.exit()` like
//! the other windows). The launcher module itself is intentionally
//! ignorant of the event loop — it only forwards `winit::WindowEvent` to
//! egui via [`Self::on_window_event`].

use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

use crate::render::RenderError;

/// Default content size for the launcher. Logical pixels — winit scales by
/// the active monitor's `scale_factor` automatically.
#[allow(dead_code)] // Used by LauncherWindow::new; T-003-T2.2 wires the call site.
const LAUNCHER_LOGICAL_SIZE: (u32, u32) = (600, 400);

#[allow(dead_code)] // Constructed by T-003-T2.2 from the AppState::Launcher path.
pub struct LauncherWindow {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

#[allow(dead_code)] // Methods called by T-003-T2.2's launcher event-loop integration.
impl LauncherWindow {
    /// Open a launcher window on the primary display. Mirrors
    /// [`ControlWindow::new`](crate::windows::control::ControlWindow::new):
    /// creates the winit window, configures the wgpu surface against the
    /// shared adapter / device, and wires up egui-winit + egui-wgpu.
    ///
    /// The window is centred on the primary monitor when one is reported
    /// by winit; otherwise it falls back to the OS default position.
    pub fn new(
        active_loop: &ActiveEventLoop,
        instance: &wgpu::Instance,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
    ) -> Result<Self, RenderError> {
        let (lw, lh) = LAUNCHER_LOGICAL_SIZE;
        let inner_size = winit::dpi::LogicalSize::new(lw, lh);

        let mut attrs = WindowAttributes::default()
            .with_title("rmap")
            .with_inner_size(inner_size)
            .with_resizable(false);

        // Centre on the primary monitor when winit can identify one. The
        // primary's `position()` is in physical pixels, `size()` likewise;
        // convert through `scale_factor` to a logical position so the
        // launcher sits at the visual centre regardless of HiDPI.
        if let Some(primary) = active_loop.primary_monitor() {
            let scale = primary.scale_factor().max(1.0);
            let mon_pos = primary.position();
            let mon_size = primary.size();
            let mon_logical_x = mon_pos.x as f64 / scale;
            let mon_logical_y = mon_pos.y as f64 / scale;
            let mon_logical_w = mon_size.width as f64 / scale;
            let mon_logical_h = mon_size.height as f64 / scale;
            let x = mon_logical_x + (mon_logical_w - lw as f64) / 2.0;
            let y = mon_logical_y + (mon_logical_h - lh as f64) / 2.0;
            attrs = attrs.with_position(winit::dpi::LogicalPosition::new(x, y));
        }

        let window = active_loop
            .create_window(attrs)
            .map_err(|e| RenderError::Surface(format!("create launcher window: {e}")))?;
        let window = Arc::new(window);

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| RenderError::Surface(format!("create launcher surface: {e}")))?;

        let caps = surface.get_capabilities(adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| RenderError::Surface("launcher surface: no formats".into()))?;
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .ok_or_else(|| RenderError::Surface("launcher surface: no alpha modes".into()))?;

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(device, &config);

        let egui_ctx = egui::Context::default();
        let viewport_id = egui_ctx.viewport_id();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            viewport_id,
            &*window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            device,
            format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );

        Ok(Self {
            window,
            surface,
            config,
            egui_ctx,
            egui_state,
            egui_renderer,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// Ask winit to fire a `RedrawRequested` event on the next loop tick.
    /// The launcher runs under `ControlFlow::Wait`, so the operator only
    /// sees a frame when something asks for one — egui's repaint hint
    /// (returned in [`egui_winit::EventResponse`]) is the usual trigger,
    /// plus a one-shot call after window creation to paint the first frame.
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Forward a winit window event to egui-winit. Returns the egui
    /// response so the App can decide whether to skip downstream
    /// handling (matches `ControlWindow::on_window_event`).
    pub fn on_window_event(
        &mut self,
        event: &winit::event::WindowEvent,
    ) -> egui_winit::EventResponse {
        self.egui_state.on_window_event(&self.window, event)
    }

    /// Re-configure the surface after a resize. Launcher is non-resizable
    /// today (`with_resizable(false)`), but DPI changes still emit
    /// `Resized` events on macOS — handle them defensively.
    pub fn resize(&mut self, device: &wgpu::Device, new_size: winit::dpi::PhysicalSize<u32>) {
        self.config.width = new_size.width.max(1);
        self.config.height = new_size.height.max(1);
        self.surface.configure(device, &self.config);
    }

    /// Render one egui frame. The closure populates the UI; this method
    /// owns begin/end frame, paint, and surface present.
    ///
    /// 003-T2.1 placeholder body: callers pass a closure that emits a
    /// single "Launcher coming soon." label. T2.4 replaces that with the
    /// three start buttons + projector picker.
    pub fn render<F: FnMut(&mut egui::Ui)>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mut ui: F,
    ) -> Result<(), RenderError> {
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_ctx.run_ui(raw_input, |root_ui| {
            ui(root_ui);
        });
        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        // Upload font / image deltas BEFORE surface acquire — see the
        // matching note in `ControlWindow::render` for why the order
        // matters. egui emits each delta exactly once, so dropping it
        // on a skipped redraw blanks the UI permanently.
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(device, queue, *id, image_delta);
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(RenderError::Surface(
                    "launcher surface acquire validation error".into(),
                ));
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("launcher window encoder"),
        });

        let pixels_per_point = full_output.pixels_per_point;
        let paint_jobs = self
            .egui_ctx
            .tessellate(full_output.shapes, pixels_per_point);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point,
        };

        let user_cmd_bufs = self.egui_renderer.update_buffers(
            device,
            queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("launcher egui pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.05,
                                g: 0.05,
                                b: 0.07,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            self.egui_renderer
                .render(&mut pass, &paint_jobs, &screen_descriptor);
        }

        queue.submit(
            user_cmd_bufs
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        frame.present();
        Ok(())
    }
}
