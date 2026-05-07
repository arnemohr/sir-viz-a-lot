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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
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
    #[serde(default)]
    pub mask_feather_px: f32,
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
}

impl Default for Project {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            layers: Vec::new(),
            warps: Vec::new(),
            scenes: Vec::new(),
            output_monitor_index: 0,
            output_resolution: None,
            background_color: default_bg(),
            asset_root: None,
            gamma: 1.0,
            brightness: 0.0,
            contrast: 1.0,
        }
    }
}

fn default_bg() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_one() -> f32 {
    1.0
}
