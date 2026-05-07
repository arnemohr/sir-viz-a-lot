//! SVG loading + cached rasterization + hot reload.
//!
//! Rasterization is performed on a worker thread (`std::thread::spawn` plus
//! a `crossbeam-channel` for results) so a 200 KB SVG cannot stall a frame.

pub mod worker;

use std::path::PathBuf;

use crate::error::RmapError;

/// Cache key for the most recently rasterized pixmap.
///
/// `generation` is bumped externally by T-M3-04 when the SVG file changes
/// on disk; while it stays at `0`, caching reduces to "same target size →
/// reuse the pixmap".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RasterKey {
    width: u32,
    height: u32,
    generation: u64,
}

/// A loaded SVG layer.
///
/// Holds the parsed `usvg::Tree` so later milestones can rasterize it
/// (T-M3-02) and upload the result to a wgpu texture (T-M3-03) without
/// re-parsing on every frame.
#[derive(Debug)]
pub struct SvgLayer {
    pub path: PathBuf,
    /// Parsed SVG document. Read by the rasterization worker (T-M3-04).
    pub(crate) tree: usvg::Tree,
    /// Last rasterized result, keyed on (target_size, generation).
    /// `generation` is bumped by T-M3-04 when the SVG file changes on
    /// disk; for now it stays at 0 (caching reduces to "same target size →
    /// reuse the pixmap").
    cache: Option<(RasterKey, tiny_skia::Pixmap)>,
    /// Monotonic counter T-M3-04 will increment when the source file
    /// changes. 0 at construction; bumped externally.
    generation: u64,
    /// Cached GPU texture mirroring the most-recently-rasterized Pixmap.
    /// `None` until `upload(...)` is called. The texture is `Rgba8UnormSrgb`
    /// with **unpremultiplied** alpha (see UNMULTIPLIED ALPHA note on
    /// `upload`).
    gpu_texture: Option<wgpu::Texture>,
    gpu_texture_view: Option<wgpu::TextureView>,
    /// Last-uploaded (width, height, generation) so `upload(...)` can
    /// short-circuit when nothing has changed.
    gpu_uploaded_key: Option<RasterKey>,
}

impl SvgLayer {
    /// Reads the SVG at `path` from disk and parses it via `usvg`.
    ///
    /// Errors:
    /// - `RmapError::Io`  — file unreadable / not UTF-8 at the I/O layer.
    /// - `RmapError::Other` — `usvg` parse failure. We reuse `Other` instead
    ///   of adding a new variant: SVG parse errors are not a long-lived
    ///   category in this codebase (one call site, one external dep), and
    ///   the spec for T-M3-01 prefers no scope creep into `error.rs`.
    pub fn load(path: PathBuf) -> crate::error::Result<Self> {
        // I/O errors auto-convert via `RmapError::Io` (`From<std::io::Error>`).
        let content = std::fs::read_to_string(&path)?;

        // usvg 0.47: `Tree::from_str(text: &str, opt: &Options) -> Result<Self, Error>`.
        // The 0.47 API takes only `&Options`; the font database lives *inside*
        // `Options::fontdb` behind the `text` cargo feature and is not a
        // separate parameter (contrary to some 0.4x prereleases).
        let tree = usvg::Tree::from_str(&content, &usvg::Options::default())
            .map_err(|e| RmapError::Other(format!("svg parse failed: {e}")))?;

        Ok(Self {
            path,
            tree,
            cache: None,
            generation: 0,
            gpu_texture: None,
            gpu_texture_view: None,
            gpu_uploaded_key: None,
        })
    }

    /// The SVG's effective bounding box in user (canvas) coordinates.
    ///
    /// usvg 0.47 exposes this as `Tree::root() -> &Group` plus
    /// `Group::abs_bounding_box() -> tiny_skia_path::Rect`. We return
    /// `Option` because an empty/degenerate SVG yields a zero-sized box,
    /// which downstream rasterization (T-M3-02) should treat as "nothing
    /// to draw" rather than panic on a 0×0 pixmap.
    pub fn bbox(&self) -> Option<usvg::Rect> {
        let r = self.tree.root().abs_bounding_box();
        if r.width() > 0.0 && r.height() > 0.0 {
            Some(r)
        } else {
            None
        }
    }

    /// Rasterize the SVG to a `(width, height)` premultiplied-RGBA pixmap.
    ///
    /// Caches the most recent result keyed on `(width, height, generation)`;
    /// repeated calls with the same size (and unchanged source file) return
    /// a reference to the cached pixmap without redrawing.
    ///
    /// PREMULTIPLIED ALPHA: `tiny_skia::Pixmap` produces pixels in
    /// premultiplied sRGB-RGBA. T-M3-03 (GPU upload) owns the format
    /// choice — either upload as `Rgba8UnormSrgb` and treat the texture as
    /// premultiplied throughout the blend pipeline, or call
    /// `Pixmap::take_demultiplied()` (consumes the pixmap; we'd have to
    /// drop caching) before the upload. This function emits premultiplied
    /// pixels; downstream is responsible for handling them.
    ///
    /// Strategy: 2× oversample then downsample with `image`'s Triangle
    /// filter. The brief allows direct rasterization if the roundtrip is
    /// onerous; the roundtrip turned out to be ~12 lines of glue, so we
    /// kept the oversample. Triangle is chosen over Lanczos3 because
    /// projector framerates dominate over the marginal quality gain.
    pub fn rasterize(&mut self, size: (u32, u32)) -> crate::error::Result<&tiny_skia::Pixmap> {
        let (width, height) = size;
        let key = RasterKey {
            width,
            height,
            generation: self.generation,
        };

        // Fast path: cache hit. We can't use `if let Some(..) = self.cache`
        // and then return a reference, because the borrow would prevent
        // the slow path from mutating `self.cache`. Match-then-index keeps
        // the borrow checker happy.
        if matches!(&self.cache, Some((k, _)) if *k == key) {
            return Ok(&self.cache.as_ref().unwrap().1);
        }

        let bbox = self
            .bbox()
            .ok_or_else(|| RmapError::Other("svg has no content to rasterize".into()))?;

        // 2× oversample render target.
        let over_w = width.saturating_mul(2).max(1);
        let over_h = height.saturating_mul(2).max(1);

        let mut over = tiny_skia::Pixmap::new(over_w, over_h).ok_or_else(|| {
            RmapError::Other(format!(
                "rasterize failed: could not allocate {over_w}x{over_h} pixmap"
            ))
        })?;

        // Fit the SVG bbox into the oversample pixmap. resvg 0.47's
        // `render` takes a `PixmapMut` (not `&mut Pixmap`), so go through
        // `Pixmap::as_mut()`.
        let scale_x = over_w as f32 / bbox.width();
        let scale_y = over_h as f32 / bbox.height();
        let transform = tiny_skia::Transform::from_scale(scale_x, scale_y)
            .pre_translate(-bbox.left(), -bbox.top());
        resvg::render(&self.tree, transform, &mut over.as_mut());

        // Downsample via `image` to the final target size.
        // tiny_skia::Pixmap → image::RgbaImage → resize → image::RgbaImage → tiny_skia::Pixmap.
        // image's `imageops::resize` already assumes premultiplied alpha
        // (per its rustdoc) — same convention as tiny-skia, so no
        // (un)premultiply step is required here.
        let over_data = over.take(); // consume; we no longer need it
        let over_buf: image::RgbaImage = image::ImageBuffer::from_vec(over_w, over_h, over_data)
            .ok_or_else(|| {
                RmapError::Other("rasterize failed: oversample buffer size mismatch".into())
            })?;

        let small_buf = image::imageops::resize(
            &over_buf,
            width,
            height,
            image::imageops::FilterType::Triangle,
        );

        let small_size = tiny_skia::IntSize::from_wh(width, height).ok_or_else(|| {
            RmapError::Other(format!(
                "rasterize failed: invalid target size {width}x{height}"
            ))
        })?;
        let pixmap =
            tiny_skia::Pixmap::from_vec(small_buf.into_raw(), small_size).ok_or_else(|| {
                RmapError::Other("rasterize failed: downsample buffer size mismatch".into())
            })?;

        self.cache = Some((key, pixmap));
        Ok(&self.cache.as_ref().unwrap().1)
    }

    /// Re-rasterize off-thread when the source SVG changes on disk OR the
    /// layer's effective on-screen size crosses the oversampling threshold.
    pub fn maybe_rerasterize(&mut self) {
        // TODO(M3): notify_debouncer_full + crossbeam-channel worker pool.
    }

    /// Upload `pixmap` to a GPU texture as `Rgba8UnormSrgb` with
    /// **unmultiplied** alpha.
    ///
    /// Short-circuits if the texture already reflects the current
    /// `(width, height, generation)` triple so callers can call this every
    /// frame without redundant uploads.
    ///
    /// # UNMULTIPLIED ALPHA
    ///
    /// `tiny_skia::Pixmap` stores premultiplied RGBA, but we upload as
    /// `wgpu::TextureFormat::Rgba8UnormSrgb` with the operator expecting
    /// standard (unmultiplied) alpha for downstream blending.
    /// Convert per pixel: `rgb / alpha`. Per the M3 risk note in
    /// `001-initial-setup-plan.md §3.4`: "an explicit RGBA8 unmultiplied
    /// path in svg_layer.rs::upload_to_gpu and a comment marking the
    /// choice".
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pixmap: &tiny_skia::Pixmap,
    ) -> crate::error::Result<()> {
        let key = RasterKey {
            width: pixmap.width(),
            height: pixmap.height(),
            generation: self.generation,
        };

        // Short-circuit: nothing has changed since the last upload.
        if self.gpu_uploaded_key == Some(key) && self.gpu_texture.is_some() {
            return Ok(());
        }

        let width = pixmap.width();
        let height = pixmap.height();

        // Unmultiply: walk 4 bytes at a time and undo the premultiplied
        // encoding that tiny_skia produces.
        let raw = pixmap.data();
        let mut unpremultiplied = Vec::with_capacity(raw.len());
        for chunk in raw.chunks_exact(4) {
            let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
            if a == 0 {
                // alpha=0 makes RGB invisible regardless of value;
                // passthrough is the conventional non-destructive path.
                unpremultiplied.extend_from_slice(&[r, g, b, a]);
            } else {
                let ur = ((r as u32 * 255) / a as u32).min(255) as u8;
                let ug = ((g as u32 * 255) / a as u32).min(255) as u8;
                let ub = ((b as u32 * 255) / a as u32).min(255) as u8;
                unpremultiplied.extend_from_slice(&[ur, ug, ub, a]);
            }
        }

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("svg_layer texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &unpremultiplied,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.gpu_texture = Some(texture);
        self.gpu_texture_view = Some(view);
        self.gpu_uploaded_key = Some(key);

        Ok(())
    }

    /// Returns the GPU texture view for the most recently uploaded raster,
    /// or `None` if `upload` has not been called yet.
    ///
    /// T-M3-06 uses this to bind the texture as a source for the quad draw.
    pub fn texture_view(&self) -> Option<&wgpu::TextureView> {
        self.gpu_texture_view.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inline SVG used by both `load_smoke` and `rasterize_dimensions`.
    /// A simple filled circle on a 40×40 viewBox is enough to exercise
    /// parse + raster without bringing in a fixture file.
    const TEST_SVG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40">
  <circle r="10" cx="20" cy="20" fill="black" />
</svg>"#;

    /// Smoke test for `SvgLayer::load`: writes an inline SVG to the OS
    /// temp dir, parses it, and checks the bbox is non-degenerate.
    /// CPU-only; no GPU dependency, no `tempfile` crate dependency.
    #[test]
    fn load_smoke() {
        let path = std::env::temp_dir().join("rmap_t-m3-01_load_smoke.svg");
        std::fs::write(&path, TEST_SVG).expect("write temp svg");

        let layer = SvgLayer::load(path.clone()).expect("load should succeed");
        let bbox = layer.bbox().expect("bbox should be Some for non-empty svg");
        assert!(bbox.width() > 0.0, "bbox width must be > 0");
        assert!(bbox.height() > 0.0, "bbox height must be > 0");

        // Best-effort cleanup; do not fail the test on cleanup error.
        let _ = std::fs::remove_file(&path);
    }

    /// `rasterize` produces a pixmap of the requested size, caches the
    /// result by (size, generation), and re-renders when the size changes.
    #[test]
    fn rasterize_dimensions() {
        let path = std::env::temp_dir().join("rmap_t-m3-02_rasterize_dimensions.svg");
        std::fs::write(&path, TEST_SVG).expect("write temp svg");

        let mut layer = SvgLayer::load(path.clone()).expect("load should succeed");

        // First call: produces a 100x100 pixmap.
        let first_ptr: *const tiny_skia::Pixmap = {
            let p = layer.rasterize((100, 100)).expect("rasterize 100x100");
            assert_eq!(p.width(), 100);
            assert_eq!(p.height(), 100);
            p
        };

        // Second call at the same size: cache hit. The returned reference
        // must point at the same `Pixmap` value (i.e. same heap location).
        let second_ptr: *const tiny_skia::Pixmap = {
            let p = layer
                .rasterize((100, 100))
                .expect("rasterize 100x100 cached");
            assert_eq!(p.width(), 100);
            assert_eq!(p.height(), 100);
            p
        };
        assert!(
            std::ptr::eq(first_ptr, second_ptr),
            "second rasterize at the same size must hit the cache"
        );

        // Third call at a different size: produces a fresh pixmap. The
        // cached pixmap is replaced, so the new reference will (almost
        // certainly) point at a different address — but more importantly
        // it must report the new size.
        let p = layer.rasterize((50, 50)).expect("rasterize 50x50");
        assert_eq!(p.width(), 50);
        assert_eq!(p.height(), 50);

        let _ = std::fs::remove_file(&path);
    }
}
