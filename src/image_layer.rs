//! Raster-image (JPG / PNG) layer support (T-M8-02).
//!
//! Sibling to `svg_layer`: where SVG layers go through resvg + tiny_skia +
//! the off-thread worker, image layers are loaded synchronously via the
//! `image` crate and uploaded once at `rebuild_layers` time. Output is a
//! plain `wgpu::Texture` the same `SvgLayerPipeline` blits onto the
//! per-layer effect ping-pong.
//!
//! No oversampling — raster layers are already raster; doubling them would
//! 4× the GPU memory for no quality win. We do clamp to a max dimension
//! (4096) so a 12 MP wedding portrait doesn't OOM the GPU on a venue
//! laptop with a modest integrated chip.

use std::path::Path;

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
    let img = image::open(path).map_err(|e| {
        RmapError::Other(format!(
            "failed to decode image {}: {e}",
            path.display()
        ))
    })?;

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
        // 4 K wedding shot downscaled to 4096 stays crisp on a projector.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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
