//! Versioned project schema. Every optional field is `#[serde(default)]` so
//! older saves keep loading after fields are added.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 2;

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

/// Bilinear-resample a mesh-warp grid to new `rows`/`cols` cell counts,
/// preserving the four outer corners exactly. New interior points are
/// interpolated from the bilinear surface implied by the old grid.
///
/// Used by the Mapping tab when the operator changes mesh resolution
/// (T-M7-01) so existing customization isn't lost on resize. The
/// schema's `rows`/`cols` are cells; the returned grid is
/// `(rows+1) × (cols+1)` of normalized output-space points.
pub fn resample_grid(
    old: &[Vec<[f32; 2]>],
    new_rows: u32,
    new_cols: u32,
) -> Vec<Vec<[f32; 2]>> {
    let new_r = (new_rows as usize).max(1);
    let new_c = (new_cols as usize).max(1);
    if old.len() < 2 || old.iter().any(|row| row.len() != old[0].len()) || old[0].len() < 2 {
        return identity_grid(new_r as u32, new_c as u32);
    }
    let old_r = old.len() - 1;
    let old_c = old[0].len() - 1;
    let lerp = |a: [f32; 2], b: [f32; 2], t: f32| {
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    };
    let mut out = Vec::with_capacity(new_r + 1);
    for r in 0..=new_r {
        let fy = r as f32 / new_r as f32 * old_r as f32;
        let r0 = (fy.floor() as usize).min(old_r.saturating_sub(1));
        let ty = fy - r0 as f32;
        let r1 = r0 + 1;
        let mut row_v = Vec::with_capacity(new_c + 1);
        for c in 0..=new_c {
            let fx = c as f32 / new_c as f32 * old_c as f32;
            let c0 = (fx.floor() as usize).min(old_c.saturating_sub(1));
            let tx = fx - c0 as f32;
            let c1 = c0 + 1;
            let p00 = old[r0][c0];
            let p10 = old[r0][c1];
            let p01 = old[r1][c0];
            let p11 = old[r1][c1];
            let top = lerp(p00, p10, tx);
            let bot = lerp(p01, p11, tx);
            row_v.push(lerp(top, bot, ty));
        }
        out.push(row_v);
    }
    out
}

/// Identity grid for `rows × cols` cells: `(rows+1) × (cols+1)` points
/// uniformly spaced over `[0,1]^2`. Returned by [`resample_grid`] when
/// the input grid is degenerate.
pub fn identity_grid(rows: u32, cols: u32) -> Vec<Vec<[f32; 2]>> {
    let r = rows.max(1) as usize;
    let c = cols.max(1) as usize;
    (0..=r)
        .map(|i| {
            (0..=c)
                .map(|j| [j as f32 / c as f32, i as f32 / r as f32])
                .collect()
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: [f32; 2], b: [f32; 2], eps: f32) -> bool {
        (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps
    }

    #[test]
    fn resample_grid_preserves_outer_corners() {
        // Skewed corner pin (definitely not identity).
        let old = vec![
            vec![[0.1, 0.05], [0.9, 0.0]],
            vec![[0.0, 0.95], [1.0, 0.85]],
        ];
        let out = resample_grid(&old, 3, 3);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].len(), 4);
        // Four outer corners should match the input bit-for-bit (lerp endpoints are exact).
        assert!(approx(out[0][0], [0.1, 0.05], 1e-6));
        assert!(approx(out[0][3], [0.9, 0.0], 1e-6));
        assert!(approx(out[3][0], [0.0, 0.95], 1e-6));
        assert!(approx(out[3][3], [1.0, 0.85], 1e-6));
    }

    #[test]
    fn resample_grid_center_of_identity_is_half() {
        let old = identity_grid(1, 1);
        let out = resample_grid(&old, 2, 2);
        // Centre of a 2x2 cell grid (point [1][1]) is (0.5, 0.5).
        assert!(approx(out[1][1], [0.5, 0.5], 1e-6));
    }

    #[test]
    fn resample_grid_falls_back_to_identity_on_degenerate_input() {
        let degenerate: Vec<Vec<[f32; 2]>> = vec![];
        let out = resample_grid(&degenerate, 1, 1);
        assert_eq!(out, identity_grid(1, 1));
    }
}
