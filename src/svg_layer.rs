//! SVG loading + cached rasterization + hot reload.
//!
//! Rasterization is performed on a worker thread (`std::thread::spawn` plus
//! a `crossbeam-channel` for results) so a 200 KB SVG cannot stall a frame.

use std::path::PathBuf;

#[derive(Debug)]
pub struct SvgLayer {
    pub path: PathBuf,
    // TODO(M3): cached resvg Pixmap, last-modified timestamp,
    //           current oversampling factor, GPU texture handle.
}

impl SvgLayer {
    pub fn load(path: PathBuf) -> crate::error::Result<Self> {
        // TODO(M3): parse via usvg, rasterize via resvg+tiny-skia at the
        //           layer's effective on-screen size with 2× oversampling,
        //           upload to a wgpu texture.
        Ok(Self { path })
    }

    /// Re-rasterize off-thread when the source SVG changes on disk OR the
    /// layer's effective on-screen size crosses the oversampling threshold.
    pub fn maybe_rerasterize(&mut self) {
        // TODO(M3): notify_debouncer_full + crossbeam-channel worker pool.
    }
}
