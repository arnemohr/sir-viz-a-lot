//! Versioned project schema. Every optional field is `#[serde(default)]` so
//! older saves keep loading after fields are added.

use std::cell::Cell;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 7;

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

/// How an `Image` layer's texture maps onto its layer rect.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitMode {
    /// Fill the layer rect; crop the texture's overhang. Default for photos —
    /// matches the operator expectation that a event portrait fills the wall.
    #[default]
    Cover,
    /// Fit the texture inside the layer rect; letterbox the remainder.
    Contain,
    /// Pass-through UV mapping; no aspect lock.
    Stretch,
}

/// Source of pixels for a single layer. v2 splits SVG (rasterized via resvg)
/// from raster Image (uploaded directly via the `image` crate). v0.4 adds
/// Video (mp4 / H.264, decoded on a background thread — W4), FxLayer
/// (procedural via mask SDF — W5), and Ndi (live network stream — W6).
/// All variants flow through the same compositor + effects + warp chain
/// after their respective upload paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayerKind {
    Svg {
        svg_path: PathBuf,
    },
    Image {
        path: PathBuf,
        #[serde(default)]
        fit: FitMode,
        /// Normalized point the `Cover` crop centers on. Ignored by other fits.
        #[serde(default = "default_focal")]
        focal: [f32; 2],
    },
    /// v0.4 W4 — mp4 / H.264 video. Real fields land in P0.4.1; for the
    /// P0.1.2 scaffold this carries the path only and renders a
    /// placeholder rectangle until the decoder thread + texture-upload
    /// pipeline are wired.
    Video {
        path: PathBuf,
    },
    /// v0.4 W5 — procedural FX layer driven by mask SDF. Real fields
    /// (params HashMap) land in P0.5.1; the scaffold carries the
    /// preset id only and renders a placeholder until P0.5.3 wires the
    /// real shader dispatch.
    FxLayer {
        preset_id: String,
    },
    /// v0.4 W6 — live NDI input as a layer source. Real receiver lands
    /// in P0.6.2; the scaffold carries the source name only and renders
    /// a placeholder until then.
    Ndi {
        source_name: String,
    },
}

impl LayerKind {
    /// Path on disk the renderer (or worker) reads from. `Svg` /
    /// `Image` / `Video` carry one; `FxLayer` (procedural) and `Ndi`
    /// (network) do not, returning `None`. Callers that need a path
    /// (audit's `MissingAsset` check, relinking) skip variants
    /// returning `None`.
    pub fn asset_path(&self) -> Option<&std::path::Path> {
        match self {
            LayerKind::Svg { svg_path } => Some(svg_path.as_path()),
            LayerKind::Image { path, .. } => Some(path.as_path()),
            LayerKind::Video { path } => Some(path.as_path()),
            LayerKind::FxLayer { .. } | LayerKind::Ndi { .. } => None,
        }
    }
}

fn default_focal() -> [f32; 2] {
    [0.5, 0.5]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConfig {
    pub id: String,
    pub kind: LayerKind,
    pub enabled: bool,
    pub transform: Transform2D,
    pub effects: Vec<crate::effects::Effect>,
    pub blend_mode: BlendMode,
    pub opacity: f32,
    /// v4: per-layer warp + mask. T3.0b consumes this in the render
    /// graph; until then the project-level `Project.warps` field is
    /// still authoritative and this is populated by `migrate_v3_to_v4`
    /// for round-trip safety.
    #[serde(default = "WarpMesh::identity")]
    pub warp: WarpMesh,
    /// V31.6.1 — when `true`, this layer is excluded from compositing.
    /// `solo` on `Project` takes precedence: if any solo is active,
    /// this flag is irrelevant for non-soloed layers (they are hidden
    /// regardless) and is ignored for the soloed layer (it renders
    /// regardless). Defaults to `false`; `#[serde(default)]` keeps v6
    /// fixtures loading unchanged.
    #[serde(default)]
    pub muted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpMesh {
    pub rows: u32,
    pub cols: u32,
    pub grid: Vec<Vec<[f32; 2]>>,
    #[serde(default)]
    pub mask_polygon: Vec<[f32; 2]>,
    /// Normalized fraction of output extent (0..0.5 useful), not pixels.
    #[serde(default)]
    pub mask_feather: f32,
}

impl WarpMesh {
    /// Full-canvas identity warp: 2×2 grid pinned to the unit square,
    /// `mask_feather: 0.02`. Used as `LayerConfig::warp`'s serde default
    /// (so old projects loading without the field round-trip safely) and
    /// as the migration target. v3's `source_rect` field is gone — under
    /// v4 each layer's warp samples the entire layer output, so the
    /// source-rect concept doesn't apply.
    pub fn identity() -> Self {
        WarpMesh {
            rows: 1,
            cols: 1,
            grid: vec![vec![[0.0, 0.0], [1.0, 0.0]], vec![[0.0, 1.0], [1.0, 1.0]]],
            mask_polygon: Vec::new(),
            mask_feather: 0.02,
        }
    }

    /// 003-T3.29 — default warp for **newly added layers** under v5's
    /// warp-as-placement model: a half-size centered 2×2 quad. Corners
    /// land at the layer's bounding box (0.25 … 0.75) so the operator
    /// sees the warp handles on the layer, not at the projector edges.
    /// Distinct from [`identity`] (which fills the projector and remains
    /// the serde / migration fallback).
    pub fn default_placement() -> Self {
        WarpMesh {
            rows: 1,
            cols: 1,
            grid: vec![
                vec![[0.25, 0.25], [0.75, 0.25]],
                vec![[0.25, 0.75], [0.75, 0.75]],
            ],
            mask_polygon: Vec::new(),
            mask_feather: 0.02,
        }
    }
}

/// V31.2.1 — portable monitor reference stored in the project.
///
/// A projector is identified by UUID when available (captured by V31.2.3 on
/// save). `uuid: None` covers v5-migrated projects and platforms without UUID
/// support. `fallback_index` is used when `uuid` is absent or no live monitor
/// matches; it maps to the index reported by `--list-monitors`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputTarget {
    /// macOS `CGDisplayCreateUUIDFromDisplayID` value if known (V31.2.3
    /// captures it on save). `None` for v5-migrated projects and on
    /// platforms without UUID support.
    #[serde(default)]
    pub uuid: Option<String>,
    /// Display index used as a fallback when `uuid` is absent or no live
    /// monitor matches. Maps to the index reported by `--list-monitors`.
    #[serde(default)]
    pub fallback_index: usize,
    /// P0.1.2 (W8.2) — 3×3 colour-correction matrix applied per-projector
    /// at present time. Identity by default; populated by the W8.3
    /// calibration UI. Stored as row-major `[[r_r, r_g, r_b], [g_r,
    /// g_g, g_b], [b_r, b_g, b_b]]` — `out = matrix * in` per channel.
    #[serde(default = "rgb_matrix_identity")]
    pub rgb_matrix: [[f32; 3]; 3],
}

impl Default for OutputTarget {
    fn default() -> Self {
        Self {
            uuid: None,
            fallback_index: 0,
            rgb_matrix: rgb_matrix_identity(),
        }
    }
}

/// Identity colour matrix — `out = in` for every channel. Used as the
/// `OutputTarget.rgb_matrix` serde default so v6 projects load with no
/// colour change.
pub fn rgb_matrix_identity() -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

/// 003-T4.1 — 192×108 RGBA8 thumbnail captured when a scene is saved.
/// Stored as a flat `width * height * 4` byte array in row-major order.
/// `#[serde(default)]` ensures existing v5 saves (without this field) load
/// cleanly with `thumbnail = None`. Size: 192 × 108 × 4 = 82,944 bytes;
/// JSON-encoded as a numeric array that's fine for v3 scope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThumbnailRgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub snapshot: serde_json::Value,
    /// 003-T4.1 — optional thumbnail captured at save time. `None` for
    /// scenes saved before T4.1 or when capture fails.
    #[serde(default)]
    pub thumbnail: Option<ThumbnailRgba>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,
    #[serde(default)]
    pub layers: Vec<LayerConfig>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub output_target: OutputTarget,
    /// When true, draw output in a decorated window on `output_target`'s
    /// monitor instead of borderless fullscreen. Applied at startup (restart to toggle).
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
    /// 003-T3.28 — per-display tone override. `None` means inherit `gamma`
    /// (master); `Some(v)` overrides the projector output only. The control-
    /// window preview reads the pre-gamma `warp_rt_view`, so master tuning
    /// is invisible there in either case; the override therefore creates the
    /// preview-vs-projector divergence the practitioner needs without a
    /// second gamma pass. Multi-projector v0.4 will move this onto an
    /// `OutputTarget`; the `Option<f32>` shape is forward-compatible.
    #[serde(default)]
    pub gamma_override: Option<f32>,
    /// 003-T3.28 — per-display brightness override. See `gamma_override`.
    #[serde(default)]
    pub brightness_override: Option<f32>,
    /// 003-T3.28 — per-display contrast override. See `gamma_override`.
    #[serde(default)]
    pub contrast_override: Option<f32>,
    /// Seconds to interpolate between scenes on recall. `0.0` = instant snap
    /// (the default; preserves M5 behaviour). Crossfades only fire when both
    /// snapshots share the same layer paths in the same order; structural
    /// differences fall back to instant snap.
    #[serde(default)]
    pub crossfade_duration_s: f32,
    /// V31.6.1 — when `Some(idx)`, only the layer at `idx` is composited;
    /// all other layers are hidden regardless of their `muted` flag. The
    /// soloed layer renders even if its own `muted == true` (solo takes
    /// precedence). `None` means no solo is active; all layers render
    /// according to their own `muted` flags. Defaults to `None`; `#[serde(default)]`
    /// keeps v6 fixtures loading unchanged.
    #[serde(default)]
    pub solo: Option<usize>,
    /// V31.7.2 — quantize cue firing to this many bars. `None` means
    /// immediate fire (current behaviour). `Some(1)`, `Some(2)`,
    /// `Some(4)`, `Some(8)` are the values exposed in the UI; other
    /// values are accepted at the schema level for forward compat. No
    /// schema bump — `Option<u8>` defaults to `None` so v6 fixtures
    /// load unchanged.
    #[serde(default)]
    pub quantize_bars: Option<u8>,
    /// Side-channel state surfaced by [`migrate::migrate_v3_to_v4`] to the
    /// audit pass (T3.0d). `previous_warp_count > 1` triggers a one-shot
    /// `MultipleWarpsConsolidated` finding so the operator knows the
    /// migration was lossy. `#[serde(skip)]` keeps it out of saved files;
    /// `Cell` lets the audit consume + clear without `&mut Project`.
    #[serde(skip, default)]
    pub transient_audit_signals: Cell<TransientAuditSignals>,
}

/// Per-load signals from `migrate` to `audit`. The audit calls
/// [`Cell::take`] to consume + clear in one step so the
/// `MultipleWarpsConsolidated` finding fires only once per session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransientAuditSignals {
    /// Number of `Project.warps` entries the v3 → v4 migration
    /// consolidated onto per-layer warps. `0` for v4-native projects;
    /// `1` for the common case (no audit finding); `> 1` triggers
    /// `MultipleWarpsConsolidated` exactly once.
    pub previous_warp_count: usize,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            layers: Vec::new(),
            scenes: Vec::new(),
            output_target: OutputTarget::default(),
            output_windowed: false,
            output_resolution: None,
            background_color: default_bg(),
            asset_root: None,
            gamma: 1.0,
            brightness: 0.0,
            contrast: 1.0,
            gamma_override: None,
            brightness_override: None,
            contrast_override: None,
            crossfade_duration_s: 0.0,
            solo: None,
            quantize_bars: None,
            transient_audit_signals: Cell::new(TransientAuditSignals::default()),
        }
    }
}

fn default_bg() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_one() -> f32 {
    1.0
}

impl Project {
    /// V31.6.1 — compute whether the layer at `idx` should be composited
    /// for this frame, given the current `solo` state.
    ///
    /// Rule: render iff `!muted && (solo.is_none() || solo == Some(idx))`.
    /// Solo takes precedence: the soloed layer renders even if its own
    /// `muted == true`; non-soloed layers hide when any solo is active.
    pub fn layer_is_visible(&self, idx: usize) -> bool {
        let layer = match self.layers.get(idx) {
            Some(l) => l,
            None => return false,
        };
        match self.solo {
            // V31.6.1: solo'd layer renders even if muted; non-solo'd layers
            // hide when any solo is active.
            Some(solo_idx) => solo_idx == idx,
            None => !layer.muted,
        }
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
pub fn resample_grid(old: &[Vec<[f32; 2]>], new_rows: u32, new_cols: u32) -> Vec<Vec<[f32; 2]>> {
    let new_r = (new_rows as usize).max(1);
    let new_c = (new_cols as usize).max(1);
    if old.len() < 2 || old.iter().any(|row| row.len() != old[0].len()) || old[0].len() < 2 {
        return identity_grid(new_r as u32, new_c as u32);
    }
    let old_r = old.len() - 1;
    let old_c = old[0].len() - 1;
    let lerp =
        |a: [f32; 2], b: [f32; 2], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
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
///
/// 003-T3.29 — newly-added layers get [`WarpMesh::default_placement`]
/// (a centered half-size quad) so the operator's first sight of the
/// warp handles is on the layer, not at the projector edges.
pub fn layer_from_svg_path(id: impl Into<String>, svg_path: PathBuf) -> LayerConfig {
    LayerConfig {
        id: id.into(),
        kind: LayerKind::Svg { svg_path },
        enabled: true,
        transform: Transform2D::default(),
        effects: crate::effects::default_effect_chain(),
        blend_mode: BlendMode::Normal,
        opacity: 1.0,
        warp: WarpMesh::default_placement(),
        muted: false,
    }
}

/// Build a layer row for an image (JPG/PNG) path using the v1 default chain.
/// Defaults to `Cover` fit + center focal — matches the "drop a photo,
/// it fills the wall" operator expectation (T-M8-05).
///
/// 003-T3.29 — sees [`WarpMesh::default_placement`] for the same reason
/// as [`layer_from_svg_path`].
#[allow(dead_code)] // Consumed by T-M8-05 drag-drop path; predates that hook.
pub fn layer_from_image_path(id: impl Into<String>, path: PathBuf) -> LayerConfig {
    LayerConfig {
        id: id.into(),
        kind: LayerKind::Image {
            path,
            fit: FitMode::Cover,
            focal: default_focal(),
        },
        enabled: true,
        transform: Transform2D::default(),
        effects: crate::effects::default_effect_chain(),
        blend_mode: BlendMode::Normal,
        opacity: 1.0,
        warp: WarpMesh::default_placement(),
        muted: false,
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

    /// 003-T4.1 — `thumbnail: Option<ThumbnailRgba>` round-trips through
    /// `serde_json` and the `#[serde(default)]` attribute keeps old projects
    /// (without the field) loading cleanly with `thumbnail = None`.
    #[test]
    fn scene_thumbnail_round_trip_through_serde() {
        // Build a scene with a synthetic thumbnail.
        let data: Vec<u8> = (0u8..=255).cycle().take(192 * 108 * 4).collect();
        let thumb = ThumbnailRgba {
            width: 192,
            height: 108,
            data: data.clone(),
        };
        let scene = Scene {
            name: "intro".to_string(),
            snapshot: serde_json::Value::Null,
            thumbnail: Some(thumb.clone()),
        };

        // Serialize → deserialize → assert byte-equal thumbnail.
        let json = serde_json::to_string(&scene).expect("serialize scene");
        let decoded: Scene = serde_json::from_str(&json).expect("deserialize scene");
        assert_eq!(decoded.thumbnail, Some(thumb));
    }

    /// Old saves (without a `thumbnail` field) must deserialize cleanly with
    /// `thumbnail = None` thanks to `#[serde(default)]`.
    #[test]
    fn scene_missing_thumbnail_deserializes_as_none() {
        let json = r#"{"name":"old-scene","snapshot":null}"#;
        let scene: Scene = serde_json::from_str(json).expect("deserialize old scene");
        assert_eq!(scene.thumbnail, None);
        assert_eq!(scene.name, "old-scene");
    }

    /// V31.1.4 — a JSON file that omits the `effects` field entirely must fail
    /// loudly (missing required field), not silently populate effects from the
    /// default chain. This documents the current serde behaviour: `effects` has
    /// no `#[serde(default)]` so it is a required field and an older hypothetical
    /// serialiser that emits `effects: []` (not absent) is the safe pattern.
    ///
    /// This is a sentinel test: if someone accidentally adds `#[serde(default)]`
    /// to `effects` with a custom default function that returns `default_effect_chain()`,
    /// this test will still pass (it won't catch that). The real guard is
    /// `empty_effects_vec_survives_snapshot_round_trip` in `mod.rs`.
    ///
    /// If `#[serde(default)]` is ever added to `effects` to support loading
    /// forward-compat JSON that omits the field, use `Vec::new` (empty) as the
    /// default, NOT `default_effect_chain()`.
    #[test]
    fn layer_config_missing_effects_field_errors() {
        // JSON with no effects field at all — not even an empty array.
        let json = r#"{
            "id": "test",
            "kind": {"Svg": {"svg_path": "/tmp/x.svg"}},
            "enabled": true,
            "transform": {"translate": [0.0, 0.0], "rotate_deg": 0.0, "scale": [1.0, 1.0], "anchor": [0.0, 0.0]},
            "blend_mode": "Normal",
            "opacity": 1.0
        }"#;
        let result: Result<LayerConfig, _> = serde_json::from_str(json);
        // Currently a missing `effects` field is a deserialization error because
        // LayerConfig has no serde(default) on effects. If this ever changes to Ok,
        // verify the deserialized effects vec is EMPTY (not default_effect_chain()).
        match result {
            Err(_) => { /* expected: missing required field */ }
            Ok(lc) => assert_eq!(
                lc.effects.len(),
                0,
                "If effects gains serde(default), the default MUST be empty vec (not default_effect_chain). \
                 Got {} effects: {:?}",
                lc.effects.len(),
                lc.effects
            ),
        }
    }
}
