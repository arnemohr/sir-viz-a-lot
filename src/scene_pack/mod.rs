//! P7.10.1 — Scene pack export / import (`.rmap-scene-pack.zip`).
//!
//! A scene pack is a portable zip archive containing a `manifest.json` and
//! the layer configs + referenced assets (images, SVGs, FX presets) needed
//! to reproduce a set of scenes in a different project.
//!
//! ## Zip layout
//!
//! ```text
//! my-pack.rmap-scene-pack.zip
//! ├── manifest.json          (ScenePackManifest)
//! └── assets/
//!     ├── 0/photo.jpg        (asset for template 0)
//!     └── 1/logo.svg         (asset for template 1)
//! ```
//!
//! Assets are stored relative to the manifest; paths are normalised to
//! forward-slash within the zip.  Import extracts to
//! `~/Library/Application Support/rmap/scene-packs/<pack_id>/`.

pub mod schema;

// These items are public library API (used by the lib crate and future
// operator UI code).  The binary crate declares this module but has not
// yet wired every item to a call site; suppress the false-positive lints.
#[allow(unused_imports)]
pub use schema::{ScenePackError, ScenePackManifest, ScenePackTemplate};
