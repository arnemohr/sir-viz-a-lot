//! Output surface: borderless fullscreen by default, or a decorated window on
//! the chosen monitor when windowed. Owns blackout/freeze state and recreates
//! its surface on panic or on `SurfaceError::Lost`/`SurfaceError::Outdated`.

use std::sync::Arc;

use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowAttributes};

use crate::render::RenderError;

/// 003-T4.16a — "Preview as projector" pre-show window. A thin stub that opens
/// a plain windowed output on the laptop so the operator can dry-run the show
/// before connecting a projector. The child window renders the same gamma output
/// as the real projector would.
///
/// **Stub status (T4.16a):** this struct exists so `EditingState` can hold
/// `Option<PreviewWindow>` and the toolbar can open/close it. Full rendering
/// (blitting `warp_rt_view` to the child surface) is deferred as a follow-up;
/// for now the window opens with a solid background colour.
///
/// Sleep assertion: NOT held during preview mode. Only `GoLive` holds it.
// T4.16a stub: `surface` and `config` will be consumed by the blit renderer
// once the full preview rendering lands. Suppress dead-code lint for now.
#[allow(dead_code)]
#[cfg(feature = "v3")]
pub struct PreviewWindow {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
}

#[cfg(feature = "v3")]
impl PreviewWindow {
    /// Open the preview window on the primary display, sized to `width × height`
    /// (caller picks a target-aspect size such as 640 × 360 for 16:9).
    ///
    /// # Errors
    /// Returns `RenderError::Surface` if the window or surface cannot be created
    /// (e.g. the compositor refuses a second surface).
    pub fn new(
        active_loop: &ActiveEventLoop,
        instance: &wgpu::Instance,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let attrs = WindowAttributes::default()
            .with_title("rmap — Preview")
            .with_inner_size(LogicalSize::new(width, height));

        let window = active_loop
            .create_window(attrs)
            .map_err(|e| RenderError::Surface(format!("preview window create: {e}")))?;
        let window = Arc::new(window);

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| RenderError::Surface(format!("preview surface create: {e}")))?;

        let caps = surface.get_capabilities(adapter);
        let format = if caps.formats.contains(&wgpu::TextureFormat::Bgra8UnormSrgb) {
            wgpu::TextureFormat::Bgra8UnormSrgb
        } else if let Some(srgb) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
            srgb
        } else {
            caps.formats.first().copied().ok_or_else(|| {
                RenderError::Surface("preview surface: no supported formats".into())
            })?
        };
        let alpha_mode = caps.alpha_modes.first().copied().ok_or_else(|| {
            RenderError::Surface("preview surface: no supported alpha modes".into())
        })?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
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
        })
    }
}

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
///
/// Note: operator-level show toggles (blackout, freeze, test-pattern,
/// editor overlay) are NOT stored here. They are session-scoped and live
/// once on `EditingState.output_state` — a single `OutputState` shared
/// across all projectors, not duplicated per projector.
pub struct OutputWindow {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
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
                window.set_outer_position(mh.position());
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
        })
    }

    /// Re-apply the cached SurfaceConfiguration. Used by T-M1-05 for
    /// SurfaceError::Lost / Outdated recovery.
    pub fn recreate_surface(&self, device: &wgpu::Device) {
        self.surface.configure(device, &self.config);
    }

    /// 003-T4.16 — Hot-swap the projector between windowed and borderless
    /// fullscreen at runtime.
    ///
    /// Calls `winit::window::Window::set_fullscreen` on the existing window so
    /// the wgpu surface stays bound to the same window pointer — no surface
    /// re-creation is required here. The OS will fire a `WindowEvent::Resized`
    /// next frame; the App's `Resized` handler calls `recreate_surface` +
    /// `resize_m5_gpu` + `register_scene_preview`, which handles any dimension
    /// change cleanly.
    ///
    /// **Note on the preview:** the control-window preview's `TextureId` is
    /// bound to the projector RT view (`warp_rt_view`, post-warp, pre-gamma;
    /// T3.0b). That view is an offscreen texture independent of the projector's
    /// swap chain, so it survives this call without re-registration.
    ///
    /// Wrapped in `catch_unwind` so a windowing-system panic (observed on some
    /// macOS Sequoia betas) converts to `RenderError::Surface` rather than
    /// unwinding the event loop. The failure path logs + toasts the message and
    /// the App routes to `AppState::Failed`.
    #[cfg(feature = "v3")]
    pub fn set_fullscreen(
        &self,
        fullscreen: bool,
        monitor: Option<MonitorHandle>,
    ) -> Result<(), RenderError> {
        // The winit call may panic on driver / compositor bugs. Capture it.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let fs = if fullscreen {
                Some(Fullscreen::Borderless(monitor))
            } else {
                None
            };
            self.window.set_fullscreen(fs);
        }));
        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<&str>() {
                    format!("set_fullscreen panicked: {s}")
                } else if let Some(s) = e.downcast_ref::<String>() {
                    format!("set_fullscreen panicked: {s}")
                } else {
                    "set_fullscreen panicked (unknown payload)".to_string()
                };
                Err(RenderError::Surface(msg))
            }
        }
    }
}
