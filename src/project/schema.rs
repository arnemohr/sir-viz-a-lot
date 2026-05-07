//! Versioned project schema. Every optional field is `#[serde(default)]` so
//! older saves keep loading after fields are added.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Transform2D {
    pub translate: [f32; 2],
    pub rotate_deg: f32,
    pub scale: [f32; 2],
    pub anchor: [f32; 2],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    #[default]
    Normal,
    Add,
    Multiply,
    Screen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConfig {
    pub id: String,
    pub svg_path: PathBuf,
    pub enabled: bool,
    pub transform: Transform2D,
    pub effects: Vec<crate::effects::Effect>,
    pub blend_mode: BlendMode,
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpMesh {
    pub rows: u32,
    pub cols: u32,
    pub grid: Vec<Vec<[f32; 2]>>,
    pub source_rect: [f32; 4],
    #[serde(default)]
    pub mask_polygon: Vec<[f32; 2]>,
    /// Normalized fraction of output extent (0..0.5 useful), not pixels.
    #[serde(default)]
    pub mask_feather: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub snapshot: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,
    #[serde(default)]
    pub layers: Vec<LayerConfig>,
    #[serde(default)]
    pub warps: Vec<WarpMesh>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub output_monitor_index: usize,
    /// When true, draw output in a decorated window on `output_monitor_index`
    /// instead of borderless fullscreen. Applied at startup (restart to toggle).
    #[serde(default)]
    pub output_windowed: bool,
    #[serde(default)]
    pub output_resolution: Option<(u32, u32)>,
    #[serde(default = "default_bg")]
    pub background_color: [f32; 4],
    #[serde(default)]
    pub asset_root: Option<PathBuf>,
    #[serde(default = "default_one")]
    pub gamma: f32,
    #[serde(default)]
    pub brightness: f32,
    #[serde(default = "default_one")]
    pub contrast: f32,
    /// Seconds to interpolate between scenes on recall. `0.0` = instant snap
    /// (the default; preserves M5 behaviour). Crossfades only fire when both
    /// snapshots share the same layer paths in the same order; structural
    /// differences fall back to instant snap.
    #[serde(default)]
    pub crossfade_duration_s: f32,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            layers: Vec::new(),
            warps: Vec::new(),
            scenes: Vec::new(),
            output_monitor_index: 0,
            output_windowed: false,
            output_resolution: None,
            background_color: default_bg(),
            asset_root: None,
            gamma: 1.0,
            brightness: 0.0,
            contrast: 1.0,
            crossfade_duration_s: 0.0,
        }
    }
}

fn default_bg() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_one() -> f32 {
    1.0
}

/// Full-frame corner pin in normalized output space (0–1). One warp matches one composed frame.
pub fn default_warp_mesh() -> WarpMesh {
    WarpMesh {
        rows: 1,
        cols: 1,
        grid: vec![
            vec![[0.0, 0.0], [1.0, 0.0]],
            vec![[0.0, 1.0], [1.0, 1.0]],
        ],
        source_rect: [0.0, 0.0, 1.0, 1.0],
        mask_polygon: Vec::new(),
        mask_feather: 0.02,
    }
}

/// Build a layer row for an SVG path using the v1 default effect chain.
pub fn layer_from_svg_path(id: impl Into<String>, svg_path: PathBuf) -> LayerConfig {
    LayerConfig {
        id: id.into(),
        svg_path,
        enabled: true,
        transform: Transform2D::default(),
        effects: crate::effects::default_effect_chain(),
        blend_mode: BlendMode::Normal,
        opacity: 1.0,
    }
}
