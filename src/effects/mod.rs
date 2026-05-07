//! Effects are modeled as an enum (not trait objects) so adding a variant
//! without updating the renderer fails at compile time.

pub mod blur;
pub mod color;
pub mod transform;

use serde::{Deserialize, Serialize};

use crate::modulators::Modulator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Effect {
    Color {
        hue: Modulator,
        saturation: Modulator,
        brightness: Modulator,
        contrast: Modulator,
    },
    Tint {
        rgba: [f32; 4],
        amount: Modulator,
    },
    Blur {
        radius_px: Modulator,
    },
    Transform {
        translate: [f32; 2],
        rotate_deg: Modulator,
        scale_x: Modulator,
        scale_y: Modulator,
    },
}
