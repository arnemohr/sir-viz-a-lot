//! SVG loading + cached rasterization + hot reload.
//!
//! Rasterization is performed on a worker thread (`std::thread::spawn` plus
//! a `crossbeam-channel` for results) so a 200 KB SVG cannot stall a frame.

use std::path::PathBuf;

use crate::error::RmapError;

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
    // TODO(M3): cached resvg Pixmap, last-modified timestamp,
    //           current oversampling factor, GPU texture handle.
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

        Ok(Self { path, tree })
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

    /// Re-rasterize off-thread when the source SVG changes on disk OR the
    /// layer's effective on-screen size crosses the oversampling threshold.
    pub fn maybe_rerasterize(&mut self) {
        // TODO(M3): notify_debouncer_full + crossbeam-channel worker pool.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test for `SvgLayer::load`: writes an inline SVG to the OS
    /// temp dir, parses it, and checks the bbox is non-degenerate.
    /// CPU-only; no GPU dependency, no `tempfile` crate dependency.
    #[test]
    fn load_smoke() {
        const SVG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40">
  <circle r="10" cx="20" cy="20" fill="black" />
</svg>"#;

        let path = std::env::temp_dir().join("rmap_t-m3-01_load_smoke.svg");
        std::fs::write(&path, SVG).expect("write temp svg");

        let layer = SvgLayer::load(path.clone()).expect("load should succeed");
        let bbox = layer.bbox().expect("bbox should be Some for non-empty svg");
        assert!(bbox.width() > 0.0, "bbox width must be > 0");
        assert!(bbox.height() > 0.0, "bbox height must be > 0");

        // Best-effort cleanup; do not fail the test on cleanup error.
        let _ = std::fs::remove_file(&path);
    }
}
