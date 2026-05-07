//! Output surface: borderless fullscreen by default, or a decorated window on
//! the chosen monitor when windowed. Owns blackout/freeze state and recreates
//! its surface on panic or on `SurfaceError::Lost`/`SurfaceError::Outdated`.

use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::dpi::LogicalSize;
use winit::window::{Fullscreen, Window, WindowAttributes};

use crate::render::RenderError;

#[derive(Debug)]
pub struct OutputState {
    pub blackout: bool,
    pub freeze: bool,
    /// Currently-selected built-in test pattern. `Default` is
    /// [`TestPattern::None`]; the `T` key cycles through variants via
    /// [`Self::cycle_test_pattern`].
    pub test_pattern: crate::test_patterns::TestPattern,
    /// Paint per-layer bounding rects + per-warp mask polygon outlines
    /// directly on the projector after the gamma pass, so the operator
    /// sees on the actual surface where each layer is mapped while
    /// dragging in the control window. Toggle with `O`. Defaults to
    /// `true`: a fresh user benefits from the feedback far more than
    /// they're hurt by it; flip it off before the show.
    pub show_editor_overlay: bool,
}

impl Default for OutputState {
    fn default() -> Self {
        Self {
            blackout: false,
            freeze: false,
            test_pattern: crate::test_patterns::TestPattern::default(),
            show_editor_overlay: true,
        }
    }
}

impl OutputState {
    pub fn toggle_blackout(&mut self) {
        self.blackout = !self.blackout;
    }

    pub fn toggle_freeze(&mut self) {
        self.freeze = !self.freeze;
    }

    pub fn cycle_test_pattern(&mut self) {
        self.test_pattern = self.test_pattern.next();
    }

    pub fn toggle_editor_overlay(&mut self) {
        self.show_editor_overlay = !self.show_editor_overlay;
    }
}

/// Default inner size when [`OutputWindow::new`] is called with `windowed = true`.
pub const WINDOWED_DEFAULT_WIDTH: u32 = 1280;
/// Default inner height for windowed output.
pub const WINDOWED_DEFAULT_HEIGHT: u32 = 720;

/// Output window plus its `wgpu::Surface` and the
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
    /// Open the output window on `monitor` (used for fullscreen target or
    /// windowed placement). Borderless fullscreen unless `windowed` is true.
    ///
    /// Caller must have created `instance`, `adapter`, and `device` before this.
    pub fn new(
        active_loop: &ActiveEventLoop,
        monitor: Option<MonitorHandle>,
        instance: &wgpu::Instance,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        windowed: bool,
    ) -> Result<Self, RenderError> {
        let attrs = WindowAttributes::default().with_title("rmap");
        let attrs = if windowed {
            attrs.with_inner_size(LogicalSize::new(
                WINDOWED_DEFAULT_WIDTH,
                WINDOWED_DEFAULT_HEIGHT,
            ))
        } else {
            attrs.with_fullscreen(Some(Fullscreen::Borderless(monitor.clone())))
        };

        let window = active_loop
            .create_window(attrs)
            .map_err(|e| RenderError::Surface(format!("create window: {e}")))?;

        if windowed {
            if let Some(mh) = monitor {
                let _ = window.set_outer_position(mh.position());
            }
            window.set_cursor_visible(true);
        } else {
            window.set_cursor_visible(false);
        }

        let window = Arc::new(window);

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| RenderError::Surface(format!("create surface: {e}")))?;

        let caps = surface.get_capabilities(adapter);

        // Prefer Bgra8UnormSrgb — most desktop GPUs offer it and it gives
        // correct sRGB compositing for free. Fall back to any sRGB format
        // the surface lists, then to whatever's first. If the surface
        // reports no formats at all (pathological), error out instead of
        // panicking on `[0]`.
        let format = if caps.formats.contains(&wgpu::TextureFormat::Bgra8UnormSrgb) {
            wgpu::TextureFormat::Bgra8UnormSrgb
        } else if let Some(srgb) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
            srgb
        } else {
            caps.formats.first().copied().ok_or_else(|| {
                RenderError::Surface("surface reports no supported formats".into())
            })?
        };

        let alpha_mode = caps.alpha_modes.first().copied().ok_or_else(|| {
            RenderError::Surface("surface reports no supported alpha modes".into())
        })?;

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
            alpha_mode,
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
