//! Blends N pre-effected layer textures into a single output texture using
//! each layer's blend mode and opacity.

#[derive(Default)]
pub struct Compositor {
    // TODO(M5): output texture handle, per-blend-mode pipeline cache.
}

impl Compositor {
    pub fn new() -> Self {
        Self::default()
    }
}
