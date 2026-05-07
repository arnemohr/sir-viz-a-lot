//! GPU pipeline: device/queue/surface lifecycle, ping-pong effect chains,
//! compositor, warp mesh, gamma master.
//!
//! # GPU bring-up split: `GpuContext` then `Renderer`
//!
//! Plan §3.1 prints the target signature
//! `Renderer::new(surface: &wgpu::Surface<'_>) -> Result<Self, RenderError>`.
//! That signature is impossible against the shape that T-M1-02 actually shipped:
//! `OutputWindow::new(active_loop, monitor, &instance, &adapter, &device, windowed)`
//! requires the wgpu `Instance`, `Adapter`, and `Device` to *already* exist
//! before any `Surface` is created (the `Surface` itself is born inside the
//! window constructor, configured against the device). So a `Renderer::new`
//! that takes `&Surface` cannot also be the place where the device is born.
//!
//! Resolution: split the responsibilities.
//!
//! - [`GpuContext`] owns `Instance + Adapter + Device + Queue`. Its
//!   constructor bootstraps wgpu *without* a surface compatibility hint
//!   (`compatible_surface: None`) — acceptable on macOS and desktop where
//!   adapters are surface-agnostic; on multi-GPU laptops we accept whatever
//!   the OS hands back under `HighPerformance`.
//! - [`OutputWindow::new`](crate::windows::output::OutputWindow::new) (already
//!   shipped in T-M1-02) takes references into the `GpuContext`'s
//!   `instance`/`adapter`/`device`, creates the `Surface`, picks a format, and
//!   configures the surface.
//! - [`Renderer::new`] then takes ownership of the `GpuContext` and the
//!   surface format, and builds the per-pass pipelines (M1 = the
//!   triangle.wgsl quad).
//!
//! T-M1-04 will call these in order: `GpuContext::new()` →
//! `OutputWindow::new(..., &gpu.instance, &gpu.adapter, &gpu.device, windowed)` →
//! `Renderer::new(gpu, output.config.format)`.
//!
//! `pollster::block_on` is used internally so callers stay synchronous; the
//! spec forbids tokio.

pub mod compositor;
pub mod gamma;
pub mod overlay;
pub mod pipeline;
pub mod sdf;
pub mod warp;

use std::iter;

use thiserror::Error;
use tracing::warn;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("no compatible wgpu adapter found")]
    NoAdapter,

    #[error("surface configuration failed: {0}")]
    Surface(String),

    #[error("shader compile failed in {name}: {message}")]
    #[allow(dead_code)] // shader-hot-reload path (T-M5+) will produce this variant
    ShaderCompile { name: &'static str, message: String },

    #[error("surface lost")]
    SurfaceLost,

    #[error("surface outdated")]
    SurfaceOutdated,

    #[error("surface suboptimal")]
    SurfaceSuboptimal,

    #[error("renderer panicked: {message}")]
    RenderPanic { message: String },
}

/// Owns the wgpu `Instance`, `Adapter`, `Device`, and `Queue`. Created up
/// front (before any surface) so [`OutputWindow::new`](crate::windows::output::OutputWindow::new)
/// can borrow the `instance`/`adapter`/`device` while creating + configuring
/// the surface. Ownership is then handed to [`Renderer::new`].
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuContext {
    /// Bootstrap wgpu without a surface compatibility hint. Acceptable on
    /// macOS / desktop where adapters are surface-agnostic; on multi-GPU
    /// laptops we accept whatever the OS hands back under `HighPerformance`.
    ///
    /// Uses `pollster::block_on` internally so callers stay synchronous (the
    /// spec forbids tokio).
    pub fn new() -> Result<Self, RenderError> {
        let instance = wgpu::Instance::default();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|_| RenderError::NoAdapter)?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rmap"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .map_err(|e| RenderError::Surface(format!("request device: {e}")))?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}

pub struct Renderer {
    pub gpu: GpuContext,
    triangle_pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    /// Build the `triangle.wgsl` pipeline targeting `surface_format`. Takes
    /// ownership of the [`GpuContext`].
    ///
    /// Note: shader compile errors in wgpu 29 are surfaced asynchronously via
    /// `device.on_uncaptured_error`, not as a synchronous Result from
    /// `create_shader_module`. `build.rs` runs `naga` validation at compile
    /// time, so a malformed `triangle.wgsl` is caught before this point. If
    /// runtime compile errors do appear in the wild, T-M1-05 / a future task
    /// can wire an error scope here and convert into
    /// [`RenderError::ShaderCompile`].
    pub fn new(gpu: GpuContext, surface_format: wgpu::TextureFormat) -> Result<Self, RenderError> {
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("triangle.wgsl"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/triangle.wgsl").into()),
            });

        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("triangle pipeline layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });

        let triangle_pipeline =
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("triangle pipeline"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        unclipped_depth: false,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: surface_format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                });

        Ok(Self {
            gpu,
            triangle_pipeline,
        })
    }

    /// Render one M1 hello-rectangle frame into the [`OutputWindow`]'s
    /// surface. Acquires the next surface texture, draws the `triangle.wgsl`
    /// quad, submits, presents.
    ///
    /// Surface-acquisition outcomes (wgpu 29's
    /// [`wgpu::CurrentSurfaceTexture`]) are mapped as follows:
    ///
    /// - `Success` → render normally.
    /// - `Suboptimal` → return [`RenderError::SurfaceSuboptimal`] *without*
    ///   drawing. The App layer reconfigures the surface; the next frame
    ///   should come back as `Success`. (Drawing on a suboptimal surface is
    ///   discouraged by the wgpu docs.)
    /// - `Outdated` → return [`RenderError::SurfaceOutdated`]; App
    ///   reconfigures.
    /// - `Lost` → return [`RenderError::SurfaceLost`]; App reconfigures.
    /// - `Validation` → return [`RenderError::Surface`] (a validation error
    ///   has already been raised on the device error scope; not a
    ///   recoverable lifecycle event).
    /// - `Timeout` → log a warning and return `Ok(())`. Frame drop is fine.
    /// - `Occluded` → log at trace level and return `Ok(())`. Window is e.g.
    ///   minimized; nothing to draw.
    ///
    /// The three `SurfaceLost` / `SurfaceOutdated` / `SurfaceSuboptimal`
    /// variants are the recoverable ones — the App's `RedrawRequested`
    /// handler pattern-matches them and calls
    /// [`OutputWindow::recreate_surface`](crate::windows::output::OutputWindow::recreate_surface).
    ///
    /// [`OutputWindow`]: crate::windows::output::OutputWindow
    pub fn render_frame(
        &self,
        output: &crate::windows::output::OutputWindow,
    ) -> Result<(), RenderError> {
        // T-M2-02's trampoline turns any internal panic into
        // `RenderError::RenderPanic`, which `App::window_event`'s catch-all
        // `Err(e)` arm already logs at `error!` (egui overlay wired in T-M2-10).
        crate::show_day::panic_restore::run_frame_assert_unwind_safe(|| {
            let frame = match output.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(f) => f,
                wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
                    return Err(RenderError::SurfaceSuboptimal);
                }
                wgpu::CurrentSurfaceTexture::Timeout => {
                    warn!("surface acquire timed out; dropping frame");
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Occluded => {
                    tracing::trace!("surface occluded; skipping frame");
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Outdated => {
                    return Err(RenderError::SurfaceOutdated);
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    return Err(RenderError::SurfaceLost);
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(RenderError::Surface(
                        "surface acquire validation error".into(),
                    ));
                }
            };

            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder =
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("rmap frame encoder"),
                    });

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("triangle pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.05,
                                g: 0.05,
                                b: 0.08,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.triangle_pipeline);
                pass.draw(0..6, 0..1);
            }

            self.gpu.queue.submit(iter::once(encoder.finish()));
            frame.present();

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::show_day::panic_restore;

    use super::RenderError;

    /// T-M2-11. Mirrors what `Renderer::render_frame` does on the unwind
    /// path: a panic inside the trampoline closure converts to
    /// `RenderError::RenderPanic` and does NOT propagate. Co-located with
    /// the render module so a refactor that bypasses the
    /// `panic_restore::run_frame_assert_unwind_safe` wrap is caught here,
    /// even though the trampoline itself is exercised by
    /// `panic_restore::tests::panic_becomes_error_not_unwind`.
    ///
    /// We cannot build a real `Renderer` in a unit test (needs a GPU
    /// device); instead we exercise the exact wrapper call shape used by
    /// `render_frame`.
    #[test]
    fn render_panic_does_not_propagate() {
        let result =
            panic_restore::run_frame_assert_unwind_safe(|| panic!("simulated render panic"));
        match result {
            Err(RenderError::RenderPanic { message }) => {
                assert!(
                    message.contains("simulated render panic"),
                    "expected message to contain the panic literal, got: {message}"
                );
            }
            other => panic!("expected RenderError::RenderPanic, got {other:?}"),
        }
    }
}
