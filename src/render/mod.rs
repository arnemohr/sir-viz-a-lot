//! GPU pipeline: device/queue/surface lifecycle, ping-pong effect chains,
//! compositor, warp mesh, gamma master.

pub mod compositor;
pub mod gamma;
pub mod pipeline;
pub mod warp;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("no compatible wgpu adapter found")]
    NoAdapter,

    #[error("surface configuration failed: {0}")]
    Surface(String),

    #[error("shader compile failed in {name}: {message}")]
    ShaderCompile { name: &'static str, message: String },
}

pub struct Renderer {
    // TODO(M1): wgpu::Instance, Adapter, Device, Queue, Surface,
    //           SurfaceConfiguration, plus the per-pass pipelines below.
}

impl Renderer {
    pub fn new() -> Result<Self, RenderError> {
        // TODO(M1): pollster::block_on the async wgpu init dance:
        //   instance.request_adapter -> adapter.request_device ->
        //   surface.configure. No tokio.
        Ok(Self {})
    }

    /// Per-frame entry point. Composes layer effects → compositor → gamma →
    /// warp → present. Wrapped in `catch_unwind` higher up so a malformed
    /// SVG can't take the show down.
    pub fn render_frame(&mut self) -> Result<(), RenderError> {
        Ok(())
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new().expect("renderer init")
    }
}
