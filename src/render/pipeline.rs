//! Per-layer effect pipeline. Two ping-pong RGBA textures are allocated
//! once per layer; effect passes (T-M4-02 color, T-M4-04 blur,
//! T-M4-05 transform) render alternately into them. After N passes,
//! [`EffectPipeline::final_view`] returns the most-recent destination.
//!
//! Allocation strategy: textures match the layer's target resolution
//! (caller supplies via `new(device, width, height, format)`). Resize
//! via `resize(device, width, height)` reallocates both textures.
//!
//! Spec §2 + plan §3.4 M4 deltas.

pub struct EffectPipeline {
    ping: wgpu::Texture,
    pong: wgpu::Texture,
    ping_view: wgpu::TextureView,
    pong_view: wgpu::TextureView,
    /// Width / height of the ping-pong textures. Cached for `resize`
    /// no-op short-circuit.
    width: u32,
    height: u32,
    /// `true` means "next destination is `pong`". Toggled by
    /// `flip()` after each effect pass.
    next_is_pong: bool,
    /// Color attachment format for both textures. Matches the format
    /// effects render to (typically the surface format, or an
    /// intermediate Rgba8UnormSrgb for HDR-like compositing).
    format: wgpu::TextureFormat,
}

impl EffectPipeline {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let (ping, ping_view) = make_texture(device, width, height, format, "effect ping");
        let (pong, pong_view) = make_texture(device, width, height, format, "effect pong");
        Self {
            ping,
            pong,
            ping_view,
            pong_view,
            width,
            height,
            next_is_pong: true,
            format,
        }
    }

    /// Reallocate both textures at the new dimensions. No-op if
    /// dimensions are unchanged. Resets `next_is_pong` to `true` so
    /// the next pass starts from a known state.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }
        let (ping, ping_view) = make_texture(device, width, height, self.format, "effect ping");
        let (pong, pong_view) = make_texture(device, width, height, self.format, "effect pong");
        self.ping = ping;
        self.pong = pong;
        self.ping_view = ping_view;
        self.pong_view = pong_view;
        self.width = width;
        self.height = height;
        self.next_is_pong = true;
    }

    /// `(source_view, destination_view)` for the next effect pass.
    /// Doesn't toggle the flip — caller calls `flip()` after the pass
    /// records its draw.
    ///
    /// Note: on the very first call after construction (or after `resize`),
    /// `ping` is the source but has uninitialized contents. The caller is
    /// responsible for blitting the SVG raster (or other initial content)
    /// into `ping` before the first effect pass runs.
    pub fn current_pair(&self) -> (&wgpu::TextureView, &wgpu::TextureView) {
        if self.next_is_pong {
            (&self.ping_view, &self.pong_view)
        } else {
            (&self.pong_view, &self.ping_view)
        }
    }

    /// Toggle the flip after a pass completes.
    pub fn flip(&mut self) {
        self.next_is_pong = !self.next_is_pong;
    }

    /// View of the most recently written destination (the "current
    /// final" output). Useful after the entire effect chain runs to
    /// pass into the compositor (T-M5-01).
    pub fn final_view(&self) -> &wgpu::TextureView {
        // After N flips, the last destination is the OPPOSITE of what
        // current_pair would now report as destination. Equivalently:
        // the source side of the next pair.
        if self.next_is_pong {
            &self.ping_view // last destination was ping
        } else {
            &self.pong_view // last destination was pong
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

fn make_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
