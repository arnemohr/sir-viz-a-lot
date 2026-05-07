//! Borderless fullscreen output window on the chosen monitor. Hides the
//! cursor, owns blackout/freeze state, and recreates its surface on panic
//! or on `SurfaceError::Lost`/`SurfaceError::Outdated`.

use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowAttributes};

use crate::render::RenderError;

#[derive(Debug, Default)]
pub struct OutputState {
    pub blackout: bool,
    pub freeze: bool,
}

impl OutputState {
    pub fn toggle_blackout(&mut self) {
        self.blackout = !self.blackout;
    }

    pub fn toggle_freeze(&mut self) {
        self.freeze = !self.freeze;
    }
}

/// Borderless fullscreen output window plus its `wgpu::Surface` and the
/// `SurfaceConfiguration` we configured it with. The cached config lets
/// T-M1-05 re-`configure` the surface verbatim on `Lost`/`Outdated`
/// without re-deriving the format / present-mode pick.
///
/// The window is held inside an `Arc` because `wgpu::Surface<'static>`
/// requires the window-handle source to live as long as the surface.
/// `Arc<winit::window::Window>` implements wgpu's `DisplayAndWindowHandle`
/// blanket impl, so it converts directly into `SurfaceTarget<'static>`.
pub struct OutputWindow {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub state: OutputState,
}

impl OutputWindow {
    /// Open the borderless fullscreen window on `monitor` (or the
    /// platform-chosen default if `None`), hide the cursor, and create +
    /// configure the wgpu surface against the supplied `device`.
    ///
    /// Caller (T-M1-03 / T-M1-04) is responsible for having created the
    /// `Instance`, requested an `Adapter`, and acquired a `Device` *before*
    /// calling this — Surface creation needs the Instance and surface
    /// configuration needs the Adapter (for capability queries) and the
    /// Device (to actually configure).
    pub fn new(
        active_loop: &ActiveEventLoop,
        monitor: Option<MonitorHandle>,
        instance: &wgpu::Instance,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
    ) -> Result<Self, RenderError> {
        let attrs = WindowAttributes::default()
            .with_title("rmap")
            .with_fullscreen(Some(Fullscreen::Borderless(monitor)));

        let window = active_loop
            .create_window(attrs)
            .map_err(|e| RenderError::Surface(format!("create window: {e}")))?;
        window.set_cursor_visible(false);
        let window = Arc::new(window);

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| RenderError::Surface(format!("create surface: {e}")))?;

        let caps = surface.get_capabilities(adapter);

        // Prefer Bgra8UnormSrgb — most desktop GPUs offer it and it gives
        // correct sRGB compositing for free. Fall back to any sRGB format
        // the surface lists, then to whatever's first.
        let format = if caps.formats.contains(&wgpu::TextureFormat::Bgra8UnormSrgb) {
            wgpu::TextureFormat::Bgra8UnormSrgb
        } else if let Some(srgb) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
            srgb
        } else {
            caps.formats[0]
        };

        // winit can hand back a 0-sized inner_size during minimization or
        // very early in the lifecycle on some compositors; wgpu refuses to
        // configure a 0-dimension surface, so clamp to 1.
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(device, &config);

        Ok(Self {
            window,
            surface,
            config,
            state: OutputState::default(),
        })
    }

    /// Re-apply the cached SurfaceConfiguration. Used by T-M1-05 for
    /// SurfaceError::Lost / Outdated recovery.
    pub fn recreate_surface(&self, device: &wgpu::Device) {
        self.surface.configure(device, &self.config);
    }
}
