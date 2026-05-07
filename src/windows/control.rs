//! egui-backed control window. Opens on the primary display; shares the
//! wgpu::Device with OutputWindow's renderer. T-M4-14 stands up an empty
//! window; T-M4-15 fills it with sliders bound to layer / effect /
//! modulator parameters.
//!
//! Architecture: a second winit::Window living alongside OutputWindow.
//! egui_winit::State hosts input translation; egui_wgpu::Renderer paints
//! into the control window's own wgpu::Surface using the shared device.

use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

use crate::render::RenderError;

pub struct ControlWindow {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    /// Frame counter, used solely to throttle the per-frame diagnostic log
    /// in `render` to one line every N frames. Wrapping is fine — modulo is
    /// what we read.
    frame_counter: u64,
}

impl ControlWindow {
    pub fn new(
        active_loop: &ActiveEventLoop,
        instance: &wgpu::Instance,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
    ) -> Result<Self, RenderError> {
        let attrs = WindowAttributes::default()
            .with_title("rmap control")
            .with_inner_size(winit::dpi::LogicalSize::new(420u32, 600u32));
        let window = active_loop
            .create_window(attrs)
            .map_err(|e| RenderError::Surface(format!("create control window: {e}")))?;
        let window = Arc::new(window);

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| RenderError::Surface(format!("create control surface: {e}")))?;

        let caps = surface.get_capabilities(adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| RenderError::Surface("control surface: no formats".into()))?;
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .ok_or_else(|| RenderError::Surface("control surface: no alpha modes".into()))?;

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
            None, // theme — let egui pick
            None, // max texture side — let egui pick
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
            frame_counter: 0,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// Forward a winit event to egui_winit. Returns the egui response so
    /// the App can decide whether to skip downstream handling.
    pub fn on_window_event(
        &mut self,
        event: &winit::event::WindowEvent,
    ) -> egui_winit::EventResponse {
        self.egui_state.on_window_event(&self.window, event)
    }

    /// Re-configure the surface after a resize.
    pub fn resize(&mut self, device: &wgpu::Device, new_size: winit::dpi::PhysicalSize<u32>) {
        self.config.width = new_size.width.max(1);
        self.config.height = new_size.height.max(1);
        self.surface.configure(device, &self.config);
    }

    /// Register a wgpu texture view with the egui renderer so it can be
    /// shown inside the control window's UI as `egui::Image`. Returns
    /// the egui-side handle. Callers re-register after the underlying
    /// texture is recreated (e.g. on output-window resize) — the old
    /// `TextureId` becomes invalid (T-M9-01).
    pub fn register_native_texture(
        &mut self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
    ) -> egui::TextureId {
        self.egui_renderer
            .register_native_texture(device, view, wgpu::FilterMode::Linear)
    }

    /// Drop a previously-registered native texture binding. Pair every
    /// `register_native_texture` with `free_native_texture` to keep the
    /// renderer's GPU bind-group cache from growing on resize churn.
    pub fn free_native_texture(&mut self, id: egui::TextureId) {
        self.egui_renderer.free_texture(&id);
    }

    /// Render one egui frame. The closure populates the egui UI; the body
    /// of this function handles begin_frame / end_frame, paint, and
    /// surface present.
    ///
    /// T-M4-14 acceptance: the closure does
    /// `egui::CentralPanel::default().show_inside(ui, |ui| ui.label("rmap control"));`
    /// to prove rendering works. T-M4-15 will replace this with real sliders.
    pub fn render<F: FnMut(&mut egui::Ui)>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mut ui: F,
    ) -> Result<(), RenderError> {
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let raw_screen_rect = raw_input.screen_rect;
        let raw_viewport_pp = raw_input
            .viewports
            .get(&raw_input.viewport_id)
            .and_then(|v| v.native_pixels_per_point);
        let full_output = self.egui_ctx.run_ui(raw_input, |root_ui| {
            ui(root_ui);
        });
        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        // egui emits each texture delta only once per `TextureManager::take_delta`.
        // Upload before surface acquire: `get_current_texture` can return
        // Occluded / Timeout / Outdated / Lost and skip rendering this redraw.
        // If we returned early without `update_texture`, the font atlas delta is
        // dropped forever and later frames show `textures_delta.set` empty while
        // the GPU still has no atlas — blank UI with shapes > 0.
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
                    "control surface acquire validation error".into(),
                ));
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("control window encoder"),
        });

        // egui docs: prefer `FullOutput::pixels_per_point` for tessellation
        // and the screen descriptor — it's the value egui used internally
        // for this frame, so feathering / scissor scaling stay consistent.
        // Falling back to `egui_ctx.pixels_per_point()` would also work but
        // can drift if the ctx setting changes mid-frame.
        let pixels_per_point = full_output.pixels_per_point;
        let ctx_pp = self.egui_state.egui_ctx().pixels_per_point();
        let shapes_count = full_output.shapes.len();
        let textures_set_count = full_output.textures_delta.set.len();
        let textures_free_count = full_output.textures_delta.free.len();
        let paint_jobs = self
            .egui_ctx
            .tessellate(full_output.shapes, pixels_per_point);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point,
        };

        // Per-frame diagnostic, throttled to one line per ~120 frames (≈2 s
        // at 60 Hz). Ungated by feature so the operator's normal `cargo run`
        // emits it; `RUST_LOG=rmap=debug` surfaces the line. Cheap: integer
        // arithmetic + a tracing macro that does nothing if the level is off.
        // The operator's "dark grey, no UI" report doesn't tell us whether
        // shapes==0 (closure never paints), shapes>0 but textures missing
        // (no font atlas), or shapes>0 and textures present (downstream
        // viewport / scissor / blend issue). This line discriminates.
        if self.frame_counter % 120 == 0 {
            tracing::debug!(
                frame = self.frame_counter,
                width = self.config.width,
                height = self.config.height,
                pixels_per_point,
                ctx_pp,
                ?raw_screen_rect,
                ?raw_viewport_pp,
                shapes = shapes_count,
                paint_jobs = paint_jobs.len(),
                textures_set = textures_set_count,
                textures_free = textures_free_count,
                "control window render frame"
            );
        }
        self.frame_counter = self.frame_counter.wrapping_add(1);

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
                    label: Some("control egui pass"),
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

        // Submit user command buffers from update_buffers (texture uploads, callbacks)
        // together with the main encoder.
        queue.submit(
            user_cmd_bufs
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        // Free textures marked for destruction AFTER submit: they may still be
        // referenced by the command buffers submitted above. Destroying before
        // submit would invalidate those command buffers under wgpu validation.
        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        frame.present();
        Ok(())
    }
}

/// Push a sticky error message onto the control-window error overlay so
/// the operator sees it without having to read the log file. T-M4-15
/// integrates this into the UI; today it just logs (preserves the API
/// the App's RenderPanic handler calls).
pub fn error_overlay(msg: &str) {
    tracing::error!(
        msg = msg,
        "error overlay (egui rendering deferred to T-M4-15)"
    );
}
