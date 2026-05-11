//! Raster-image (JPG / PNG / WEBP / GIF first-frame) layer support
//! (T-M8-02; WEBP + GIF land in P1.1.1).
//!
//! Sibling to `svg_layer`: where SVG layers go through resvg + tiny_skia +
//! the off-thread worker, image layers are loaded synchronously via the
//! `image` crate and uploaded once at `rebuild_layers` time. Output is a
//! plain `wgpu::Texture` the same `SvgLayerPipeline` blits onto the
//! per-layer effect ping-pong.
//!
//! No oversampling — raster layers are already raster; doubling them would
//! 4× the GPU memory for no quality win. We do clamp to a max dimension
//! (4096) so a 12 MP event portrait doesn't OOM the GPU on a venue
//! laptop with a modest integrated chip.
//!
//! ## Texture cache (P1.1.2)
//!
//! [`ImageTextureCache`] dedupes uploads when multiple layers point at the
//! same file. wgpu 29's `Texture` is internally an Arc; cloning is a cheap
//! reference bump. Layers share the cached `Texture` directly and each
//! creates its own `TextureView`. The cache holds `wgpu::Texture` strongly
//! (its internal Arc keeps the GPU allocation alive) — eviction is keyed
//! on `(path, mtime)` so a file edited externally invalidates the cache on
//! the next lookup. Session-lifetime growth is bounded by the set of
//! distinct files ever loaded × their texture sizes (4 K cap = 64 MB
//! each); typical shows load < 20 images.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use crate::error::RmapError;

/// Hard cap on either axis after load. A larger source image is downscaled
/// (preserving aspect) to fit. Picked to keep texture allocation under
/// 64 MB on RGBA8 (4096 × 4096 × 4 = 64 MB), which fits comfortably even
/// on integrated GPUs targeted at 1080p output.
pub const MAX_DIM: u32 = 4096;

/// Decode a raster image (PNG / JPG / any format `image` supports) and
/// upload it to a fresh `Rgba8UnormSrgb` texture. Returns the texture, a
/// default `TextureView`, and the `(width, height)` actually uploaded
/// after any aspect-preserving downscale.
#[allow(dead_code)] // T-M8-03 wires this into rebuild_layers.
pub fn upload_image_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &Path,
) -> Result<(wgpu::Texture, wgpu::TextureView, (u32, u32)), RmapError> {
    let img = image::open(path)
        .map_err(|e| RmapError::Other(format!("failed to decode image {}: {e}", path.display())))?;

    let (mut width, mut height) = (img.width(), img.height());
    // Aspect-preserving downscale to MAX_DIM if needed.
    let rgba = if width <= MAX_DIM && height <= MAX_DIM {
        img.into_rgba8()
    } else {
        let scale = (MAX_DIM as f32 / width.max(height) as f32).min(1.0);
        let new_w = ((width as f32 * scale).round() as u32).max(1);
        let new_h = ((height as f32 * scale).round() as u32).max(1);
        width = new_w;
        height = new_h;
        // image::DynamicImage::resize uses Lanczos3 — sharp enough that a
        // 4 K event shot downscaled to 4096 stays crisp on a projector.
        img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3)
            .into_rgba8()
    };

    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("image layer"),
        size: extent,
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
        rgba.as_raw(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        extent,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Ok((texture, view, (width, height)))
}

// ---------------------------------------------------------------------------
// P1.1.2 — ImageTextureCache
// ---------------------------------------------------------------------------

/// Cache key: file path + modification time. mtime is read at lookup time
/// via `fs::metadata`; if it differs from a cached entry's mtime the
/// entry is treated as stale and re-uploaded.
///
/// `mtime` is `Option` because some filesystems / cross-mount scenarios
/// don't expose modified-time reliably. `None` keys still cache; they
/// just never invalidate on edit (the operator can reload the project
/// to force a re-upload in that case).
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct CacheKey {
    path: PathBuf,
    mtime: Option<SystemTime>,
}

struct CacheEntry {
    texture: wgpu::Texture,
    dims: (u32, u32),
}

/// Process-shared image texture cache. Multiple `LayerState`s pointing at
/// the same `(path, mtime)` share a single GPU allocation; each gets its
/// own `wgpu::TextureView`.
///
/// Built once per editor session (`EditingState`) and consulted by the
/// image-layer init path. Concurrent access is guarded by an internal
/// `Mutex` — lookups + inserts are O(1) amortized over the hashmap; the
/// critical section is short.
pub struct ImageTextureCache {
    entries: Mutex<HashMap<CacheKey, CacheEntry>>,
}

impl ImageTextureCache {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Look up `path` in the cache; on miss, upload via
    /// [`upload_image_rgba8`] and insert the result.
    ///
    /// Returns a clone of the cached `Texture` (cheap — wgpu 29's
    /// `Texture` is an `Arc` under the hood) plus a freshly-created
    /// `TextureView` for the caller. The dimensions are the
    /// **post-downscale** size (after any `MAX_DIM` clamp inside
    /// `upload_image_rgba8`).
    ///
    /// On mtime change for an already-cached path, the stale entry is
    /// evicted before re-upload, so the cache doesn't grow unboundedly
    /// when an operator edits a file repeatedly.
    pub fn lookup_or_upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &Path,
    ) -> Result<(wgpu::Texture, wgpu::TextureView, (u32, u32)), RmapError> {
        let mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
        let key = CacheKey {
            path: path.to_path_buf(),
            mtime,
        };

        // Fast path — cache hit.
        {
            let entries = self
                .entries
                .lock()
                .expect("ImageTextureCache mutex poisoned");
            if let Some(entry) = entries.get(&key) {
                let view = entry
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                return Ok((entry.texture.clone(), view, entry.dims));
            }
        }

        // Miss — actually upload, then insert.
        let (texture, _view_discarded, dims) = upload_image_rgba8(device, queue, path)?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut entries = self
                .entries
                .lock()
                .expect("ImageTextureCache mutex poisoned");
            // Lazy eviction: drop stale entries for the same path with a
            // different mtime so the cache doesn't grow on every save of
            // a hot-reloaded file.
            entries.retain(|k, _| !(k.path == path && k.mtime != mtime));
            entries.insert(
                key,
                CacheEntry {
                    texture: texture.clone(),
                    dims,
                },
            );
        }
        Ok((texture, view, dims))
    }

    /// Drop every cached entry. Mostly useful in tests to assert
    /// per-call upload counts deterministically.
    #[cfg(test)]
    fn clear(&self) {
        self.entries.lock().expect("mutex").clear();
    }

    /// Number of cache entries currently held. Diagnostic only.
    pub fn len(&self) -> usize {
        self.entries.lock().map(|g| g.len()).unwrap_or(0)
    }
}

impl Default for ImageTextureCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::{CacheKey, ImageTextureCache};
    use std::path::PathBuf;
    use std::time::SystemTime;

    /// New cache starts empty.
    #[test]
    fn cache_starts_empty() {
        let c = ImageTextureCache::new();
        assert_eq!(c.len(), 0);
    }

    /// Two keys with the same path but different mtimes are distinct —
    /// this is the invariant that drives eviction-on-edit. If keys
    /// collided, an edited file would silently render the stale cached
    /// texture forever.
    #[test]
    fn cache_key_distinguishes_mtime_changes() {
        let path = PathBuf::from("/tmp/test.png");
        let t1 = SystemTime::UNIX_EPOCH;
        let t2 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(60);
        let k1 = CacheKey {
            path: path.clone(),
            mtime: Some(t1),
        };
        let k2 = CacheKey {
            path: path.clone(),
            mtime: Some(t2),
        };
        assert_ne!(k1, k2, "keys with different mtimes must differ");
    }

    /// Two keys for distinct paths with the same mtime are distinct —
    /// the path is part of the key. Two operators dropping different
    /// files with identical mtimes (e.g. both freshly downloaded) still
    /// get separate cache slots.
    #[test]
    fn cache_key_distinguishes_paths() {
        let t = Some(SystemTime::UNIX_EPOCH);
        let k1 = CacheKey {
            path: PathBuf::from("/tmp/a.png"),
            mtime: t,
        };
        let k2 = CacheKey {
            path: PathBuf::from("/tmp/b.png"),
            mtime: t,
        };
        assert_ne!(k1, k2);
    }

    /// `mtime: None` keys (filesystems that don't expose modification
    /// time) still cache; equal paths with `None` mtime hit the same
    /// slot. Operators on such filesystems lose the auto-invalidation
    /// but the cache itself still dedupes.
    #[test]
    fn cache_key_none_mtimes_equal_when_path_matches() {
        let path = PathBuf::from("/tmp/x.png");
        let k1 = CacheKey {
            path: path.clone(),
            mtime: None,
        };
        let k2 = CacheKey { path, mtime: None };
        assert_eq!(k1, k2);
    }

    /// `clear()` (test-only) empties the cache. Used by future
    /// integration tests that exercise hit/miss counters across
    /// multiple lookups.
    #[test]
    fn cache_clear_drops_all_entries() {
        let c = ImageTextureCache::new();
        // The cache is empty here (no real upload-or-lookup happened);
        // the test is really asserting clear() doesn't panic on an
        // empty cache + leaves it empty. GPU integration tests cover
        // the populate-then-clear behaviour.
        c.clear();
        assert_eq!(c.len(), 0);
    }

    /// Smoke test: synthesize a 4×4 PNG to a temp file, ensure the path
    /// helper resolves it, and confirm `image::open` loads it back.
    /// We don't reach wgpu in this test (no Device); GPU-touching tests
    /// live behind `--features gpu-tests`. This proves the decode path.
    #[test]
    fn image_decode_smoke() {
        // Build a 4×4 RGBA buffer with a known pattern.
        let mut buf: Vec<u8> = Vec::with_capacity(4 * 4 * 4);
        for _ in 0..16 {
            buf.extend_from_slice(&[200, 100, 50, 255]);
        }
        let mut path = std::env::temp_dir();
        path.push(format!("rmap_image_smoke_{}.png", std::process::id()));
        let out_image: image::RgbaImage = image::RgbaImage::from_raw(4, 4, buf).expect("buf");
        let mut file = std::fs::File::create(&path).expect("create");
        out_image
            .write_to(&mut file, image::ImageFormat::Png)
            .expect("write png");
        file.flush().expect("flush");
        drop(file);

        // Decode round-trip: confirms image::open works on the new file.
        let decoded = image::open(&path).expect("decode");
        assert_eq!(decoded.width(), 4);
        assert_eq!(decoded.height(), 4);
        let _ = std::fs::remove_file(&path);
    }
}
