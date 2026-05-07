//! Built-in calibration sources. Independent of any SVG layer so warp setup
//! works before any content is loaded.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestPattern {
    #[default]
    None,
    Grid50,
    Crosshair,
    White100,
    White50,
    White25,
    ColorBars,
}

impl TestPattern {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "off",
            Self::Grid50 => "grid 50px",
            Self::Crosshair => "crosshair",
            Self::White100 => "white 100%",
            Self::White50 => "white 50%",
            Self::White25 => "white 25%",
            Self::ColorBars => "color bars",
        }
    }
}
