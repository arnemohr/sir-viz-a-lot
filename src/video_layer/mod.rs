//! Video layer support (P0.4.x). Mirrors `src/svg_layer/` —
//! per-layer background worker + per-frame texture upload.

pub mod worker;
pub use worker::{VideoControl, spawn};
