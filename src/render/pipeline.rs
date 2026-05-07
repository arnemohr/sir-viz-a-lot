//! Per-layer effect pipeline. Two ping-pong textures are allocated once per
//! layer; effect passes render alternately into them so memory is bounded.

#[derive(Default)]
pub struct EffectPipeline {
    // TODO(M4): two TextureViews + a flip flag, reused across effects.
}

impl EffectPipeline {
    pub fn new() -> Self {
        Self::default()
    }
}
