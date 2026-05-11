//! Video layer support (P0.4.x). Mirrors `src/svg_layer/` —
//! per-layer background worker + per-frame texture upload.

pub mod worker;
pub use worker::{VideoControl, spawn};

// Re-export natural_size for callers that are behind the same cfg guard.
#[cfg(all(feature = "video", target_os = "macos"))]
pub use worker::natural_size;
