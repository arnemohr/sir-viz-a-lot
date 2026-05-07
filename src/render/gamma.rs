//! Final master pass: applies global gamma / brightness / contrast to the
//! composited (and warped) output before present.

#[derive(Debug, Clone, Copy)]
pub struct GammaMaster {
    pub gamma: f32,
    pub brightness: f32,
    pub contrast: f32,
}

impl Default for GammaMaster {
    fn default() -> Self {
        Self {
            gamma: 1.0,
            brightness: 0.0,
            contrast: 1.0,
        }
    }
}
