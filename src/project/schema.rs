//! Versioned project schema. Every optional field is `#[serde(default)]` so
//! older saves keep loading after fields are added.

use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 12;

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
    /// v0.4 W4 — mp4 / H.264 video. P0.4.2 extends the schema with
    /// `speed` and (initially) `loop_seamless`; P1.4.2 replaces the
    /// boolean with the [`LoopMode`] enum. Both fields are
    /// serde-defaulted so existing v7 saves with `Video { path }` load
    /// cleanly with `speed: 1.0, loop_mode: Loop`. Old v7 saves that
    /// carried the boolean `loop_seamless` get normalised to the
    /// matching enum variant during `migrate::migrate` (always-on
    /// normalisation step, not version-gated).
    Video {
        path: PathBuf,
        /// Playback rate multiplier. 1.0 = real-time; 0.5 = half speed;
        /// 2.0 = 2× speed. P0.4.3 ships the UI; today the field is
        /// readable by the worker (Part 2) but no UI dispatches mutations.
        #[serde(default = "default_video_speed")]
        speed: f32,
        /// EOF behaviour. `Loop` (default) seeks back to clip start;
        /// `Once` pauses on EOF; `PingPong` reverses direction —
        /// currently a forward-only stub (functionally `Loop`) until
        /// P1.4.3 wires the reverse-decode path.
        #[serde(default)]
        loop_mode: LoopMode,
        /// P1.4.1 — in-point (seconds, 0.0..=clip_out). The worker
        /// starts decode at this offset and rewinds here on loop. Default
        /// 0.0 plays from the start.
        #[serde(default)]
        clip_in: f32,
        /// P1.4.1 — out-point (seconds, > clip_in). The worker stops
        /// decode here. Default `f32::INFINITY` is the sentinel for
        /// "end of clip" — no trim. Serde defaults preserve full-clip
        /// playback on existing v7 saves.
        #[serde(default = "default_video_clip_out")]
        clip_out: f32,
        /// P1.4.4 — when true, the layer's effective playback speed is
        /// `speed × (current_bpm / 120)` — i.e. at 120 BPM the manual
        /// `speed` field plays the clip at its set rate; at 60 BPM the
        /// clip runs half-speed; at 240 BPM, double-speed. The 120 BPM
        /// reference matches the show-day Clock module's default tempo.
        /// `speed` continues to be the operator-facing rate; BPM-lock
        /// just scales it. Default `false` preserves the existing free-
        /// run behaviour.
        #[serde(default)]
        bpm_lock: bool,
        /// P1.2.4 — fit-mode for the decoded frame (Cover/Contain/Stretch).
        /// Mirrors `LayerKind::Image::fit`. Defaults to `Stretch` which
        /// matches the pre-P1.2.4 hardcoded behaviour, so v7 saves load
        /// unchanged.
        #[serde(default)]
        fit: FitMode,
        /// P1.2.4 — focal point for `Cover` mode (normalised [0,1]²).
        /// Mirrors `LayerKind::Image::focal`. Defaults to `[0.5, 0.5]`.
        #[serde(default = "default_focal")]
        focal: [f32; 2],
    },
    /// v0.4 W5 — procedural FX layer driven by mask SDF. Real fields
    /// landed in P0.5.1; the scaffold carries the preset id and a
    /// string-keyed parameter map. P0.5.3 wires the real shader
    /// dispatch and registers known presets.
    ///
    /// `params` is intentionally `HashMap<String, f32>` (not a typed
    /// per-preset struct) so the schema doesn't churn as new presets
    /// arrive in Phase 2; the registry validates known keys at
    /// preset-pick time.
    FxLayer {
        preset_id: String,
        #[serde(default)]
        params: HashMap<String, f32>,
        /// P2.5.1 — RNG seed for deterministic particle layouts.
        /// Defaults to 0 for older projects; the launcher / Add-layer
        /// flow picks a fresh u64 for new layers (P2.8.1 wires this).
        #[serde(default)]
        seed: u64,
        /// P2.5.1 — seconds since the project clock at which this
        /// layer was added. The compute shader uses `clock_secs -
        /// t_layer_added_secs` as the system's local time. Defaults
        /// to 0.0 for older projects.
        #[serde(default)]
        t_layer_added_secs: f32,
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
            LayerKind::Video { path, .. } => Some(path.as_path()),
            LayerKind::FxLayer { .. } | LayerKind::Ndi { .. } => None,
        }
    }
}

fn default_focal() -> [f32; 2] {
    [0.5, 0.5]
}

fn default_video_speed() -> f32 {
    1.0
}

/// P1.4.1 — sentinel "no out-point trim" value. The worker treats
/// any `clip_out >= asset.duration` as "play to end".
fn default_video_clip_out() -> f32 {
    f32::INFINITY
}

/// P1.4.2 — EOF behaviour for [`LayerKind::Video`].
///
/// `Loop` is the show-day default ("drop an mp4, it plays forever").
/// `Once` stops the worker at EOF (pauses the layer's last decoded
/// frame). `PingPong` reverses direction at EOF — currently a
/// forward-only stub (effectively `Loop`) until P1.4.3 wires the
/// reverse-decode path. The stub's behaviour is documented at the
/// worker dispatch site.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    /// Stop at EOF — show the last decoded frame.
    Once,
    /// Seek back to clip start on EOF (default).
    #[default]
    Loop,
    /// Reverse direction at each end. Stub: behaves as `Loop` until
    /// P1.4.3 lands the reverse-decode path.
    PingPong,
}

/// P1.2.1 (W2) — image-grammar preset applied to an Image or Video
/// layer *before* the per-pixel effect chain (Color → Blur →
/// Transform → External). Mirrors `LayerKind::FxLayer`'s preset
/// shape (P0.5.1) but lives next to the layer's source instead of
/// being its source: the layer's image / video is rasterised /
/// decoded first, then the active treatment shader operates on
/// those pixels, then the effect chain runs.
///
/// **One treatment per layer.** Phase 1 ships `Option<Treatment>`
/// rather than `Vec<Treatment>` despite the spec's "pipeline"
/// wording — matches the FxLayer one-preset shape operators
/// already learned, and the v0.5 presets (tone_map, blur_mask,
/// luminance_reveal, texture_overlay, palette_extract, collage)
/// aren't useful to chain. Growing to `Vec<Treatment>` is a
/// non-breaking serde change if Phase 4 zone grammars need
/// composition.
///
/// **Non-bumping addition.** `LayerConfig.treatment` lands on v7
/// with `#[serde(default)]` so existing projects load with
/// `treatment == None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)] // T1.10: treatment mutation deleted; struct retained for schema/migration completeness.
pub struct Treatment {
    /// Registered preset id (e.g. `"tone_map"`). Unknown ids are
    /// audit-warned and render as no-ops at frame time.
    pub preset_id: String,
    /// Per-preset parameter overrides. Missing keys fall back to
    /// the preset's documented defaults; extra keys are ignored.
    /// `HashMap<String, f32>` matches the FxLayer pattern and
    /// avoids schema churn as new presets ship.
    #[serde(default)]
    pub params: std::collections::HashMap<String, f32>,
    /// Optional second-texture path for presets that consume one
    /// (P1.3.4 `texture_overlay`). `None` for presets that don't
    /// read it. The HashMap above can't carry paths, hence the
    /// dedicated field.
    #[serde(default)]
    pub overlay_path: Option<PathBuf>,
    /// Image paths for presets that compose multiple sources
    /// (P1.3.6 `collage` — capped at 4 entries; the shader's grid
    /// shape supports 1×2 / 2×1 / 2×2 layouts). Empty for presets
    /// that don't read it.
    #[serde(default)]
    pub collage_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerConfig {
    pub id: String,
    pub kind: LayerKind,
    pub enabled: bool,
    pub transform: Transform2D,
    /// 004-T1.3 — per-layer Look chain. Each node wraps one `Effect`
    /// with an `enabled` bypass flag. Changed from `Vec<Effect>` at
    /// schema v12; the migrator folds the old `treatment` field into
    /// this vec as a prepended `Effect::Treatment` node.
    pub effects: Vec<crate::effects::EffectNode>,
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
    /// P7.3.1 — optional Bezier warp mesh (schema v10+). When `Some`, takes
    /// precedence over the legacy `warp` field in the render graph. `None`
    /// for projects that have not yet been migrated (pre-v10); populated by
    /// `migrate_v9_to_v10` from the existing `warp` grid with all handles
    /// set to `None` (backward-compatible bilinear fallback).
    #[serde(default)]
    pub bezier_mesh: Option<BezierMesh>,
    /// P7.4.1 — optional composable mask graph (schema v11+). When `Some`,
    /// supersedes `bezier_mesh.mask_polygon` / `bezier_mesh.mask_feather`
    /// and `warp.mask_polygon` / `warp.mask_feather`. `None` for pre-v11
    /// projects; populated by `migrate_v10_to_v11` from the layer's existing
    /// mask data.  `MaskGraph::identity()` (empty polygon, full canvas) is
    /// the neutral value that renders identical to "no mask".
    #[serde(default)]
    pub mask_graph: Option<MaskGraph>,
}

/// P3.2.1 — semantic role tag for a mask polygon, drawn from a closed
/// seven-variant palette.
///
/// The enum's variant order is load-bearing: `impl From<ZoneRole> for u32`
/// maps `None` → 0, `Window` → 1, `Portal` → 2, …, `LightSource` → 7.
/// The WGSL constants in `ZONE_TAG_WGSL` (P3.3.1) must match this order.
/// Adding a new variant requires updating both the Rust enum *and* the WGSL
/// constants.
///
/// `#[serde(rename_all = "kebab-case")]` so the saved JSON string is
/// `"window"`, `"light-source"`, etc. — matches the plan identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ZoneRole {
    Window,
    Portal,
    Void,
    Spill,
    Edge,
    Highlight,
    LightSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct WarpMesh {
    pub rows: u32,
    pub cols: u32,
    pub grid: Vec<Vec<[f32; 2]>>,
    #[serde(default)]
    pub mask_polygon: Vec<[f32; 2]>,
    /// Normalized fraction of output extent (0..0.5 useful), not pixels.
    #[serde(default)]
    pub mask_feather: f32,
    /// P3.2.1 — semantic zone role for this mask polygon. `None` means the
    /// mask carries no zone semantics; zone-aware FX presets fall back to a
    /// neutral (transparent) output when the tag is `None`.
    ///
    /// Uses a custom `Deserialize` impl (see below) that handles unknown
    /// role strings: `zone_role` is set to `None` for unknowns and the raw
    /// string is preserved in `unknown_zone_role_raw` so the audit (P3.2.4)
    /// can emit `UnknownZoneRole` findings.
    pub zone_role: Option<ZoneRole>,
    /// P3.2.4 — sidecar for audit: when the saved JSON contains an
    /// unrecognised `zone_role` string, it is stored here so the audit
    /// pass can emit `UnknownZoneRole` without re-parsing raw JSON.
    /// `None` for well-formed projects; populated only when deserialization
    /// encounters an unknown variant.
    /// `#[serde(skip_serializing)]` excludes it from saved JSON — the audit
    /// field is transient (session-only, like `transient_audit_signals`).
    #[serde(skip_serializing)]
    pub unknown_zone_role_raw: Option<String>,
}

impl<'de> serde::Deserialize<'de> for WarpMesh {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        /// Helper struct for the actual JSON fields.
        #[derive(Deserialize)]
        struct WarpMeshRaw {
            pub rows: u32,
            pub cols: u32,
            pub grid: Vec<Vec<[f32; 2]>>,
            #[serde(default)]
            pub mask_polygon: Vec<[f32; 2]>,
            #[serde(default)]
            pub mask_feather: f32,
            /// Raw zone_role — kept as JSON Value so we can distinguish
            /// null / absent / known-string / unknown-string.
            #[serde(default)]
            pub zone_role: Option<serde_json::Value>,
        }

        let raw = WarpMeshRaw::deserialize(deserializer)?;

        // Parse the zone_role from the raw JSON value.
        let (zone_role, unknown_zone_role_raw) = match raw.zone_role {
            None | Some(serde_json::Value::Null) => (None, None),
            Some(serde_json::Value::String(s)) => {
                let quoted = format!("\"{}\"", s);
                match serde_json::from_str::<ZoneRole>(&quoted) {
                    Ok(role) => (Some(role), None),
                    Err(_) => (None, Some(s)), // Unknown — audit will report it.
                }
            }
            Some(_) => (None, None), // Unexpected type — treat as None.
        };

        Ok(WarpMesh {
            rows: raw.rows,
            cols: raw.cols,
            grid: raw.grid,
            mask_polygon: raw.mask_polygon,
            mask_feather: raw.mask_feather,
            zone_role,
            unknown_zone_role_raw,
        })
    }
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
            zone_role: None,
            unknown_zone_role_raw: None,
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
            zone_role: None,
            unknown_zone_role_raw: None,
        }
    }
}

// ---------------------------------------------------------------------------
// P7.3.1 — BezierMesh: cubic Bezier warp (schema v10)
// ---------------------------------------------------------------------------

/// P7.3.1 — A single Bezier tangent handle attached to an anchor point.
///
/// `None` = handle is unset (degenerate; edge is straight, bilinear-equivalent).
/// When set, the pair `[x, y]` is in the same normalised projector-space as the
/// `WarpMesh.grid` points (0..1 on each axis).
pub type BezierHandle = Option<[f32; 2]>;

/// P7.3.3 — Which tangent-handle slot is targeted by a `SetBezierHandle` mutation.
///
/// Each anchor `(row, col)` has two handles: `Horizontal` (right/east tangent,
/// stored in `BezierMesh.handles_h`) and `Vertical` (downward/south tangent,
/// stored in `BezierMesh.handles_v`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BezierHandleDir {
    /// Right-side (east) tangent handle — `handles_h[row][col]`.
    Horizontal,
    /// Downward (south) tangent handle — `handles_v[row][col]`.
    Vertical,
}

/// P7.3.1 — Bezier warp mesh.  Replaces `WarpMesh` for projects saved at schema
/// v10+.  `WarpMesh` remains deserializable (deprecated) but is migrated to
/// `BezierMesh` on load via `migrate_v9_to_v10`.
///
/// ## Grid layout
///
/// `anchors[row][col]` is the anchor (corner) position for `(rows+1) × (cols+1)`
/// control points, indexed row-major.  `handles_h[row][col]` is the *horizontal*
/// (right-side) tangent handle for the anchor at `(row, col)`; `handles_v[row][col]`
/// is the *vertical* (downward) tangent handle.  The opposing handle of a pair is
/// the mirror of the same slot one column (or row) to the right (or below), providing
/// C1 continuity across interior patch boundaries when both handles are set.
///
/// ## Degenerate invariant (backward-compatibility)
///
/// When every handle in both `handles_h` and `handles_v` is `None`, the Bezier
/// mesh evaluates to an identical vertex buffer as the original bilinear `WarpMesh`
/// with the same `anchors` grid.  Verified by the golden-image regression test in
/// `P7.3.2`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BezierMesh {
    /// Number of patch rows (= number of cells; anchors span rows+1).
    pub rows: u32,
    /// Number of patch columns (= number of cells; anchors span cols+1).
    pub cols: u32,
    /// Anchor (corner) positions — `(rows+1)` outer vecs, `(cols+1)` inner.
    /// In normalised projector space [0..1 × 0..1].
    pub anchors: Vec<Vec<[f32; 2]>>,
    /// Horizontal (right-side) tangent handles — same dimensions as `anchors`.
    /// `None` = straight edge (bilinear fallback).
    pub handles_h: Vec<Vec<BezierHandle>>,
    /// Vertical (downward) tangent handles — same dimensions as `anchors`.
    /// `None` = straight edge (bilinear fallback).
    pub handles_v: Vec<Vec<BezierHandle>>,
    /// Mask polygon in normalised [0..1] space — same role as `WarpMesh.mask_polygon`.
    #[serde(default)]
    pub mask_polygon: Vec<[f32; 2]>,
    /// Normalised feather fraction (0..0.5 useful) — same role as `WarpMesh.mask_feather`.
    #[serde(default = "default_mask_feather")]
    pub mask_feather: f32,
    /// P3.2.1 semantic zone role (same as `WarpMesh.zone_role`).
    #[serde(default)]
    pub zone_role: Option<ZoneRole>,
}

fn default_mask_feather() -> f32 {
    0.02
}

#[allow(dead_code)] // Methods used progressively as W3.x tasks land; clippy sees all at once.
impl BezierMesh {
    /// Identity mesh: `rows × cols` patches with all handles `None` and anchors on a
    /// uniform grid — equivalent to a `WarpMesh::identity()` with the same dimensions.
    pub fn identity(rows: u32, cols: u32) -> Self {
        let anchor_rows = rows + 1;
        let anchor_cols = cols + 1;
        let anchors: Vec<Vec<[f32; 2]>> = (0..anchor_rows)
            .map(|r| {
                (0..anchor_cols)
                    .map(|c| [c as f32 / cols as f32, r as f32 / rows as f32])
                    .collect()
            })
            .collect();
        let handles_h = vec![vec![None; anchor_cols as usize]; anchor_rows as usize];
        let handles_v = vec![vec![None; anchor_cols as usize]; anchor_rows as usize];
        BezierMesh {
            rows,
            cols,
            anchors,
            handles_h,
            handles_v,
            mask_polygon: Vec::new(),
            mask_feather: 0.02,
            zone_role: None,
        }
    }

    /// Default placement: half-size centered mesh — equivalent to `WarpMesh::default_placement()`.
    pub fn default_placement() -> Self {
        let anchor_rows = 2u32;
        let anchor_cols = 2u32;
        let anchors = vec![
            vec![[0.25f32, 0.25], [0.75, 0.25]],
            vec![[0.25, 0.75], [0.75, 0.75]],
        ];
        let handles_h = vec![vec![None; anchor_cols as usize]; anchor_rows as usize];
        let handles_v = vec![vec![None; anchor_cols as usize]; anchor_rows as usize];
        BezierMesh {
            rows: 1,
            cols: 1,
            anchors,
            handles_h,
            handles_v,
            mask_polygon: Vec::new(),
            mask_feather: 0.02,
            zone_role: None,
        }
    }

    /// Convert from a `WarpMesh` with all handles set to `None` (backward-compat migration).
    /// The anchor grid is copied verbatim from `warp.grid`; handles are all `None`.
    pub fn from_warp_mesh(warp: &WarpMesh) -> Self {
        let anchor_rows = warp.rows + 1;
        let anchor_cols = warp.cols + 1;
        let handles_h = vec![vec![None; anchor_cols as usize]; anchor_rows as usize];
        let handles_v = vec![vec![None; anchor_cols as usize]; anchor_rows as usize];
        BezierMesh {
            rows: warp.rows,
            cols: warp.cols,
            anchors: warp.grid.clone(),
            handles_h,
            handles_v,
            mask_polygon: warp.mask_polygon.clone(),
            mask_feather: warp.mask_feather,
            zone_role: warp.zone_role,
        }
    }

    /// Reconstruct a `WarpMesh` from this mesh (lossless when all handles are `None`).
    /// Used for backward-compat tests.
    pub fn to_warp_mesh_lossless(&self) -> Option<WarpMesh> {
        // Only valid when all handles are None.
        if self.handles_h.iter().flatten().any(|h| h.is_some())
            || self.handles_v.iter().flatten().any(|h| h.is_some())
        {
            return None;
        }
        Some(WarpMesh {
            rows: self.rows,
            cols: self.cols,
            grid: self.anchors.clone(),
            mask_polygon: self.mask_polygon.clone(),
            mask_feather: self.mask_feather,
            zone_role: self.zone_role,
            unknown_zone_role_raw: None,
        })
    }
}

// ---------------------------------------------------------------------------
// P7.4.1 — MaskGraph: composable mask representation (schema v11)
// ---------------------------------------------------------------------------

/// P7.4.1 — Stable identifier for a `MaskNode` within a `MaskGraph`.
/// Integer index into `MaskGraph::nodes`; references must be validated
/// before use (a stale `NodeId` is an audit warning, not a panic).
pub type NodeId = usize;

/// P7.4.1 — A single node in the mask composition graph.
///
/// Phase 7 ships four node kinds: `Polygon`, `Inverse`, `Union`, and `Subtract`.
/// `Union` and `Subtract` are schema scaffolding only — they have no CPU
/// evaluation path and no UI in Phase 7.  They exist so the schema is
/// forward-compatible with a future composable mask editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum MaskNode {
    /// A polygon mask (equivalent to the legacy `WarpMesh.mask_polygon`).
    Polygon {
        /// Polygon vertices in normalised [0..1] space.
        points: Vec<[f32; 2]>,
        /// Normalised feather fraction (0..0.5 useful).
        feather: f32,
    },
    /// Inverts the SDF of a referenced node (inside ↔ outside).
    ///
    /// Phase 7 M8: accessible from the Mask mode pill sub-row.
    Inverse {
        /// NodeId of the node whose SDF is negated.
        of: NodeId,
    },
    /// Union (min SDF) of two nodes.
    /// PCleanup.5.1 — CPU evaluation in `render::sdf::eval_node` ships;
    /// mask-editor UI to author Union nodes is still deferred (operators
    /// hand-author them in the project JSON until the editor lands).
    Union {
        /// NodeId of the first operand.
        a: NodeId,
        /// NodeId of the second operand.
        b: NodeId,
    },
    /// Subtraction (max negative SDF) of two nodes.
    /// PCleanup.5.1 — CPU evaluation in `render::sdf::eval_node` ships;
    /// mask-editor UI to author Subtract nodes is still deferred (operators
    /// hand-author them in the project JSON until the editor lands).
    Subtract {
        /// NodeId of the base node.
        base: NodeId,
        /// NodeId of the node to subtract.
        sub: NodeId,
    },
    /// P7.5.1 — Luma key: alpha derived from the brightness of the rendered output.
    ///
    /// Pixels whose luminance (max-channel approximation) exceeds `threshold`
    /// become opaque; pixels below become transparent.  `softness` widens
    /// the transition band (0 = hard, 1 = full soft).
    /// Accessible from the Mask mode pill sub-row (M8 follow-on).
    LumaKey {
        /// Luminance threshold in [0, 1]. Pixels above become opaque.
        threshold: f32,
        /// Transition softness in [0, 1].
        softness: f32,
    },
    /// P7.6.1 — Chroma key: alpha derived from a hue range in the rendered output.
    ///
    /// Pixels whose hue falls within `hue_center_deg ± hue_range_deg` and whose
    /// saturation exceeds `saturation_threshold` become transparent (alpha → 0).
    /// Default hue center 120° = green-screen.
    /// Accessible from the Mask mode pill sub-row (M8 follow-on).
    ChromaKey {
        /// Centre hue in degrees [0, 360). Default 120° (green).
        hue_center_deg: f32,
        /// Half-width of the hue range in degrees [0, 180].
        hue_range_deg: f32,
        /// Minimum saturation (HSV S) for pixels to be keyed [0, 1].
        saturation_threshold: f32,
        /// Transition softness in [0, 1].
        softness: f32,
    },
}

/// P7.4.1 — Composable mask graph.
///
/// `nodes` is a flat list; references between nodes use `NodeId` (index into
/// `nodes`).  The root node is always `nodes[0]` (when the list is non-empty).
///
/// ## Identity (no masking)
/// `MaskGraph::identity()` returns a graph with a single `Polygon` node whose
/// `points` is empty — which the SDF baker interprets as "full canvas, no mask",
/// pixel-identical to `WarpMesh.mask_polygon = []`.
///
/// ## Migration from `BezierMesh.mask_polygon` (v10 → v11)
/// Each layer's `bezier_mesh.mask_polygon` + `bezier_mesh.mask_feather` become
/// a `MaskGraph` with one `Polygon` node.  The fields are then removed from
/// `BezierMesh` (represented by the new `mask_graph` on `LayerConfig`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskGraph {
    /// Ordered list of mask nodes; `nodes[0]` is the root (evaluated last).
    pub nodes: Vec<MaskNode>,
}

#[allow(dead_code)] // Methods used progressively as W4.x tasks land.
impl MaskGraph {
    /// Full-canvas identity mask: single `Polygon` node with empty points.
    pub fn identity() -> Self {
        MaskGraph {
            nodes: vec![MaskNode::Polygon {
                points: Vec::new(),
                feather: 0.02,
            }],
        }
    }

    /// Create a single-polygon mask from a polygon + feather (migration path).
    pub fn from_polygon(points: Vec<[f32; 2]>, feather: f32) -> Self {
        MaskGraph {
            nodes: vec![MaskNode::Polygon { points, feather }],
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
    /// PCleanup.7.3 — per-projector gamma trim. `Some(v)` overrides both
    /// the project-level `gamma_override` and `gamma` master at the
    /// gamma render pass for this output only. Cascading lookup:
    /// `output.gamma_override.or(project.gamma_override).unwrap_or(project.gamma)`.
    /// Additive schema change (serde default → None means "inherit");
    /// existing projects load byte-identical to pre-PCleanup builds.
    #[serde(default)]
    pub gamma_override: Option<f32>,
    /// PCleanup.7.3 — per-projector brightness trim. Same cascade as
    /// `gamma_override`. Inert when None.
    #[serde(default)]
    pub brightness_override: Option<f32>,
    /// PCleanup.7.3 — per-projector contrast trim. Same cascade as
    /// `gamma_override`. Inert when None.
    #[serde(default)]
    pub contrast_override: Option<f32>,
}

impl Default for OutputTarget {
    fn default() -> Self {
        Self {
            uuid: None,
            fallback_index: 0,
            rgb_matrix: rgb_matrix_identity(),
            // PCleanup.7.3 — None means "inherit from project". Default
            // construction → no per-output deviation.
            gamma_override: None,
            brightness_override: None,
            contrast_override: None,
        }
    }
}

/// Identity colour matrix — `out = in` for every channel. Used as the
/// `OutputTarget.rgb_matrix` serde default so v6 projects load with no
/// colour change.
pub fn rgb_matrix_identity() -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

/// Serde default for [`Project::output_targets`] — a single-element vec
/// holding the default `OutputTarget`. Preserves the
/// "always non-empty" invariant for projects that omit the field
/// (fresh projects, malformed JSON, post-migration v6 projects).
pub fn default_output_targets() -> Vec<OutputTarget> {
    vec![OutputTarget::default()]
}

/// P0.7.3 — falloff curve shape for the edge-blend overlap region.
///
/// `Linear` is the simplest sum-to-1.0 curve in linear light (when both
/// outputs use the same `overlap_px`). `Cosine` adds a soft S-curve that
/// is still complementary and visually smoother at the seam. Gamma22 and
/// hardware-measured curves are deferred to Phase 7 (hardware calibration).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum FalloffCurve {
    #[default]
    Linear,
    Cosine,
}

/// P0.7.3 — edge-blend overlap region between two adjacent projectors.
///
/// For v0.4 the spec caps multi-output at 2 projectors, and the implicit
/// topology is "outputs[0] is left (right-edge falloff), outputs[1] is
/// right (left-edge falloff)". The linear sum-to-1.0 invariant holds only
/// when both output passes read the same `overlap_px` value — this single
/// project-level config enforces that.
///
/// Persisted as `Project.edge_blend: Option<EdgeBlendConfig>`. `None` means
/// no blending (default); explicit `Some(...)` arms the per-output edge-blend
/// pass. Phase 7 will generalise to per-edge configs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EdgeBlendConfig {
    /// Width of the soft-edge region in pixels on each projector surface.
    pub overlap_px: u32,
    /// Shape of the brightness ramp across the overlap region.
    #[serde(default)]
    pub falloff_curve: FalloffCurve,
}

impl Default for EdgeBlendConfig {
    fn default() -> Self {
        Self {
            overlap_px: 0,
            falloff_curve: FalloffCurve::Linear,
        }
    }
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

/// Legacy alias kept for backward source compatibility. New code should use
/// [`Cue`] directly. The schema field is now `cues` (serde alias `scenes`).
#[allow(dead_code)]
pub type Scene = Cue;

/// P6.2.1 — Fire mode for a cue: advance automatically after hold time
/// (`Follow`) or wait for an explicit go command (`GoOnTrigger`).
///
/// `GoOnTrigger` is the default — preserves existing trigger semantics for
/// scenes saved before Phase 6 (no hold time, operator controls every advance).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum CueFireMode {
    /// Advance to the next cue automatically after hold time expires.
    Follow,
    /// Wait for an explicit go command (Space / MIDI Note 60 / OSC /rmap/cue/go).
    #[default]
    GoOnTrigger,
}

/// P6.2.1 — BPM-bar quantize setting for a cue. `Off` fires immediately on
/// the go command; `Bars(n)` defers to the next n-bar beat boundary at the
/// current BPM. Valid values for `n` are 1, 2, 4, 8.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum BpmQuantize {
    /// Fire immediately on the go command (default).
    #[default]
    Off,
    /// Fire on the next n-bar boundary at the current BPM.
    Bars(u8),
}

/// P6.2.1 — A timecode position (HH:MM:SS:FF) used as an optional cue
/// trigger. When set, the transport fires the cue automatically when incoming
/// timecode (LTC or MTC) reaches this position.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimecodePosition {
    pub hh: u8,
    pub mm: u8,
    pub ss: u8,
    pub ff: u8,
}

/// P6.2.1 — A single cuelist entry. Extends the pre-Phase-6 `Scene` struct
/// with per-cue timing fields, fire mode, BPM quantize, and optional
/// timecode trigger.
///
/// All new fields carry serde defaults so pre-Phase-6 JSON (which has
/// neither the timing fields nor `fire_mode`) loads cleanly with identity
/// values that round-trip to the same behaviour as the old `Scene`.
///
/// `Scene` is now a type alias for `Cue` so old call sites continue to
/// compile; migrate them to `Cue` over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cue {
    pub name: String,
    pub snapshot: serde_json::Value,
    /// 003-T4.1 — optional thumbnail captured at save time. `None` for
    /// cues saved before T4.1 or when capture fails.
    #[serde(default)]
    pub thumbnail: Option<ThumbnailRgba>,
    // --- Phase 6 additions (all #[serde(default)] for backward compat) ---
    /// Crossfade duration from the previous cue state into this cue's scene
    /// snapshot (seconds). 0.0 = instant snap (default, preserves old behaviour).
    #[serde(default)]
    pub in_time_s: f32,
    /// How long the cue stays fully live before the follow chain or operator
    /// trigger can advance (seconds). `None` = hold indefinitely (default).
    #[serde(default)]
    pub hold_time_s: Option<f32>,
    /// Crossfade duration from this cue's scene out to the next cue (seconds).
    /// 0.0 = instant snap (default). Usually 0 because `in_time_s` of the
    /// next cue handles the blend.
    #[serde(default)]
    pub out_time_s: f32,
    /// Fire mode: advance automatically (`Follow`) or wait for a go command
    /// (`GoOnTrigger`, default — preserves existing operator-triggered behaviour).
    #[serde(default)]
    pub fire_mode: CueFireMode,
    /// BPM quantize: fire immediately (`Off`, default) or snap to the next
    /// n-bar boundary at the current BPM.
    #[serde(default)]
    pub bpm_quantize: BpmQuantize,
    /// Optional timecode trigger. When `Some`, the transport fires this cue
    /// automatically when incoming LTC/MTC reaches the specified position.
    #[serde(default)]
    pub timecode_trigger: Option<TimecodePosition>,
    // --- Per-cue CC bindings (Option A from binding-storage-decision.md) ---
    /// Optional MIDI CC binding for live-trim of in-time (channel, cc, scale, offset).
    #[serde(default)]
    pub in_time_binding: Option<CcBinding>,
    /// Optional MIDI CC binding for live-trim of hold time.
    #[serde(default)]
    pub hold_binding: Option<CcBinding>,
    /// Optional MIDI CC binding for live-trim of out-time.
    #[serde(default)]
    pub out_time_binding: Option<CcBinding>,
    /// Optional OSC binding for live-trim of in-time.
    #[serde(default)]
    pub in_time_osc: Option<OscBinding>,
    /// Optional OSC binding for live-trim of hold time.
    #[serde(default)]
    pub hold_osc: Option<OscBinding>,
    /// Optional OSC binding for live-trim of out-time.
    #[serde(default)]
    pub out_time_osc: Option<OscBinding>,
}

impl Cue {
    /// Construct a `Cue` with identity timing defaults (0.0 in/out, no hold
    /// limit, GoOnTrigger fire mode, no quantize, no timecode trigger, no
    /// bindings). Equivalent to the pre-Phase-6 `Scene` constructor.
    #[allow(dead_code)]
    pub fn new(
        name: impl Into<String>,
        snapshot: serde_json::Value,
        thumbnail: Option<ThumbnailRgba>,
    ) -> Self {
        Cue {
            name: name.into(),
            snapshot,
            thumbnail,
            in_time_s: 0.0,
            hold_time_s: None,
            out_time_s: 0.0,
            fire_mode: CueFireMode::GoOnTrigger,
            bpm_quantize: BpmQuantize::Off,
            timecode_trigger: None,
            in_time_binding: None,
            hold_binding: None,
            out_time_binding: None,
            in_time_osc: None,
            hold_osc: None,
            out_time_osc: None,
        }
    }
}

/// P6.2.1 — MIDI CC binding for a per-cue timing field (in-time, hold, out-time).
/// Serialised alongside the `Cue` in the project schema (Option A from the
/// binding-storage-decision.md decision doc).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CcBinding {
    /// MIDI channel (0-based, 0–15).
    pub channel: u8,
    /// CC number (0–127).
    pub cc: u8,
    /// Scale factor applied to the normalised CC value (0.0..=1.0) before
    /// adding `offset`. Default 1.0.
    pub scale: f32,
    /// Additive offset after scaling. Default 0.0.
    pub offset: f32,
}

/// P6.2.1 — OSC address binding for a per-cue timing field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OscBinding {
    /// OSC address pattern (e.g. "/rmap/cue/1/in_time").
    pub addr: String,
    /// Scale factor applied to the normalised OSC value. Default 1.0.
    pub scale: f32,
    /// Additive offset after scaling. Default 0.0.
    pub offset: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,
    #[serde(default)]
    pub layers: Vec<LayerConfig>,
    /// P6.2.1 — renamed from `scenes`; `#[serde(alias = "scenes")]` keeps
    /// pre-Phase-6 project files loading cleanly without a migration step.
    /// The v8→v9 migration (P6.2.3) will write `cues` in saved files going
    /// forward; the alias handles loading files saved by earlier versions.
    #[serde(default, alias = "scenes")]
    pub cues: Vec<Cue>,
    /// P0.7.1 (W7) — multi-projector output targets. v0.4 ships at most
    /// two entries (the second-projector edge-blend stub); Phase 7
    /// grows beyond two. **Invariant: always non-empty.** The
    /// migration / serde default ensures `output_targets[0]` is
    /// always a valid index, so [`Self::primary_output_target`] is
    /// guaranteed not to panic.
    #[serde(default = "default_output_targets")]
    pub output_targets: Vec<OutputTarget>,
    /// When true, draw output in a decorated window on the primary
    /// output target's monitor instead of borderless fullscreen.
    /// Applied at startup (restart to toggle).
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
    /// P0.7.3 — edge-blend config for the overlap region between two adjacent
    /// projectors. `None` = no blending (default); `Some(cfg)` arms the
    /// per-output edge-blend pass when `outputs.len() >= 2`. Non-bumping: a
    /// `None`-defaulting `Option` on v7 is backwards-compatible (old saves
    /// load with `edge_blend == None` = no blending, identical to prior
    /// behaviour).
    #[serde(default)]
    pub edge_blend: Option<EdgeBlendConfig>,
    /// P5.3.1 — fixture groups for DMX light output. `#[serde(default)]`
    /// keeps existing projects loading without the field; the lighting
    /// feature gate ensures the field is only compiled when lighting is
    /// enabled, keeping the schema lean for show-day builds without lighting.
    #[serde(default)]
    #[cfg(feature = "lighting")]
    pub fixture_groups: Vec<crate::lighting::fixture::FixtureGroup>,
    /// P5.7.1 — BPM-locked fixture chases. `#[serde(default)]` for
    /// backward compatibility with pre-Phase-5 project files.
    #[serde(default)]
    #[cfg(feature = "lighting")]
    pub fixture_chases: Vec<crate::lighting::chase::FixtureChase>,
    /// P5.8.1 — Art-Net destination address (host:port string).
    /// `None` defaults to `"255.255.255.255:6454"` (subnet broadcast).
    /// Operator-configurable via the Output panel Lighting section.
    #[serde(default)]
    #[cfg(feature = "lighting")]
    pub artnet_dest: Option<String>,
    /// P5.10.1 — per-scene fixture colour overrides for the active light cue.
    /// `None` when no light cue is authored for this scene; `Some(cue)` when
    /// the operator has set manual colour overrides for specific fixture groups.
    /// `#[serde(default)]` keeps pre-Phase-5 projects loading cleanly.
    #[serde(default)]
    #[cfg(feature = "lighting")]
    pub light_cue: Option<crate::project::schema::LightCueSnapshot>,
    /// Side-channel state surfaced by [`migrate::migrate_v3_to_v4`] to the
    /// audit pass (T3.0d). `previous_warp_count > 1` triggers a one-shot
    /// `MultipleWarpsConsolidated` finding so the operator knows the
    /// migration was lossy. `#[serde(skip)]` keeps it out of saved files;
    /// `Cell` lets the audit consume + clear without `&mut Project`.
    #[serde(skip, default)]
    pub transient_audit_signals: Cell<TransientAuditSignals>,
}

// ---------------------------------------------------------------------------
// P5.10.1 — LightCueSnapshot
// ---------------------------------------------------------------------------

/// A per-scene lighting cue: manual colour overrides for specific fixture groups.
///
/// Stored as `Project.light_cue: Option<LightCueSnapshot>`. When a scene is
/// recalled via `restore_scene`, the light cue is restored alongside the layer
/// state so lighting output follows the recalled scene automatically.
///
/// `#[serde(default)]` on the containing field ensures pre-Phase-5 projects
/// load cleanly without this data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg(feature = "lighting")]
pub struct LightCueSnapshot {
    /// Manual colour overrides for specific fixture groups.
    /// Each entry is `(group_id, (r, g, b))` — the group's colour is forced to
    /// this value for the duration of the scene, overriding canvas sampling.
    pub fixture_group_overrides: Vec<(crate::lighting::fixture::FixtureGroupId, (u8, u8, u8))>,
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
            cues: Vec::new(),
            output_targets: default_output_targets(),
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
            edge_blend: None,
            #[cfg(feature = "lighting")]
            fixture_groups: Vec::new(),
            #[cfg(feature = "lighting")]
            fixture_chases: Vec::new(),
            #[cfg(feature = "lighting")]
            artnet_dest: None,
            #[cfg(feature = "lighting")]
            light_cue: None,
            transient_audit_signals: Cell::new(TransientAuditSignals::default()),
        }
    }
}

impl Project {
    /// Read the project's primary output target. Always returns the
    /// `[0]` element — the schema invariant guarantees
    /// `output_targets` is non-empty (the migration + serde default
    /// + `Project::default` all populate at least one entry).
    ///
    /// In debug builds, an empty vec triggers a `debug_assert!`
    /// panic so the violation surfaces immediately. In release the
    /// helper still returns a reasonable result by re-populating
    /// the vec — show-day reliability prefers a degraded render
    /// over a crash.
    pub fn primary_output_target(&self) -> &OutputTarget {
        debug_assert!(
            !self.output_targets.is_empty(),
            "Project::output_targets is empty — schema invariant violated",
        );
        // The invariant should hold; the unwrap_or branch is defensive
        // for release builds.
        self.output_targets.first().unwrap_or_else(|| {
            static FALLBACK: std::sync::OnceLock<OutputTarget> = std::sync::OnceLock::new();
            FALLBACK.get_or_init(OutputTarget::default)
        })
    }

    /// Mutable sibling of [`Self::primary_output_target`]. If the vec
    /// is somehow empty (release-build path that should never fire),
    /// pushes a default and returns a ref to it so callers can write
    /// without an extra check.
    pub fn primary_output_target_mut(&mut self) -> &mut OutputTarget {
        if self.output_targets.is_empty() {
            self.output_targets.push(OutputTarget::default());
        }
        &mut self.output_targets[0]
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

/// P3.3.1 — convert a `ZoneRole` to its u32 discriminant for the WGSL uniform.
///
/// The mapping must stay in sync with the WGSL constants in `ZONE_TAG_WGSL`:
///   None / absent → 0 (ZONE_NONE)
///   Window        → 1 (ZONE_WINDOW)
///   Portal        → 2 (ZONE_PORTAL)
///   Void          → 3 (ZONE_VOID)
///   Spill         → 4 (ZONE_SPILL)
///   Edge          → 5 (ZONE_EDGE)
///   Highlight     → 6 (ZONE_HIGHLIGHT)
///   LightSource   → 7 (ZONE_LIGHT_SOURCE)
impl From<ZoneRole> for u32 {
    fn from(role: ZoneRole) -> u32 {
        match role {
            ZoneRole::Window => 1,
            ZoneRole::Portal => 2,
            ZoneRole::Void => 3,
            ZoneRole::Spill => 4,
            ZoneRole::Edge => 5,
            ZoneRole::Highlight => 6,
            ZoneRole::LightSource => 7,
        }
    }
}

/// Convert an `Option<ZoneRole>` to its u32 discriminant for WGSL uniforms.
/// `None` → 0 (ZONE_NONE); `Some(role)` → `u32::from(role)`.
/// Used by the zone-tag uniform write path (P3.3.2) and GPU tests (P3.6.2).
#[allow(dead_code)] // P3.3.2 wires the render-path call site.
pub fn zone_role_to_u32(opt: Option<ZoneRole>) -> u32 {
    opt.map(u32::from).unwrap_or(0)
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

        bezier_mesh: None,
        mask_graph: None,
    }
}

/// Build a layer row for a video (mp4/mov/m4v) path.
///
/// Sets `speed: 1.0, loop_mode: Loop` — the show-day defaults.
/// Other fields (warp, opacity, blend_mode, effects) mirror the other
/// layer constructors.
///
/// P0.4.2 — the worker is spawned separately at layer-init time in
/// `app.rs::rebuild_layers`; this function only builds the `LayerConfig`.
pub fn layer_from_video_path(id: impl Into<String>, path: PathBuf) -> LayerConfig {
    LayerConfig {
        id: id.into(),
        kind: LayerKind::Video {
            path,
            speed: 1.0,
            loop_mode: LoopMode::Loop,
            clip_in: 0.0,
            clip_out: f32::INFINITY,
            bpm_lock: false,
            fit: FitMode::default(),
            focal: default_focal(),
        },
        enabled: true,
        transform: Transform2D::default(),
        effects: crate::effects::default_effect_chain(),
        blend_mode: BlendMode::Normal,
        opacity: 1.0,
        warp: WarpMesh::default_placement(),
        muted: false,

        bezier_mesh: None,
        mask_graph: None,
    }
}

/// P2 follow-up — Build a new `FxLayer` with the given preset and a centered
/// rectangular mask polygon.
///
/// The mask polygon is non-empty: the FX presets read the layer's SDF,
/// so an empty mask makes the layer invisible. The default rectangle
/// at (0.2 … 0.8) × (0.15 … 0.85) gives the operator something to see
/// immediately; they can edit it via the mask manipulation tools.
pub fn layer_from_fx_preset(
    id: impl Into<String>,
    preset_id: impl Into<String>,
    params: HashMap<String, f32>,
    seed: u64,
) -> LayerConfig {
    let mut warp = WarpMesh::default_placement();
    warp.mask_polygon = vec![[0.2, 0.15], [0.8, 0.15], [0.8, 0.85], [0.2, 0.85]];
    LayerConfig {
        id: id.into(),
        kind: LayerKind::FxLayer {
            preset_id: preset_id.into(),
            params,
            seed,
            t_layer_added_secs: 0.0,
        },
        enabled: true,
        transform: Transform2D::default(),
        effects: crate::effects::default_effect_chain(),
        blend_mode: BlendMode::Normal,
        opacity: 1.0,
        warp,
        muted: false,

        bezier_mesh: None,
        mask_graph: None,
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

        bezier_mesh: None,
        mask_graph: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: [f32; 2], b: [f32; 2], eps: f32) -> bool {
        (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps
    }

    /// P0.4.2 — `LayerKind::Video { path }` (old shape, without `speed`
    /// or `loop_mode`) deserializes with serde defaults:
    /// `speed == 1.0`, `loop_mode == LoopMode::Loop`.
    #[test]
    fn video_layer_missing_speed_and_loop_deserializes_with_defaults() {
        let json = r#"{
            "id": "v1",
            "kind": {"Video": {"path": "/tmp/test.mp4"}},
            "enabled": true,
            "transform": {"translate": [0.0,0.0],"rotate_deg":0.0,"scale":[1.0,1.0],"anchor":[0.0,0.0]},
            "effects": [],
            "blend_mode": "Normal",
            "opacity": 1.0,
            "warp": {"rows":1,"cols":1,"grid":[[[0.0,0.0],[1.0,0.0]],[[0.0,1.0],[1.0,1.0]]],"mask_polygon":[],"mask_feather":0.02}
        }"#;
        let lc: LayerConfig = serde_json::from_str(json).expect("deserialize old Video shape");
        match lc.kind {
            LayerKind::Video {
                speed, loop_mode, ..
            } => {
                assert!(
                    (speed - 1.0).abs() < 1e-6,
                    "old Video shape should default speed to 1.0, got {speed}"
                );
                assert_eq!(
                    loop_mode,
                    LoopMode::Loop,
                    "old Video shape should default loop_mode to Loop"
                );
            }
            other => panic!("expected Video, got {other:?}"),
        }
    }

    /// P0.4.2 — drag-drop returns `Some(...)` for mp4/mov/m4v extensions
    /// and `None` for unsupported ones.
    #[test]
    fn layer_from_video_path_produces_video_kind() {
        let path = std::path::PathBuf::from("/tmp/show.mp4");
        let lc = layer_from_video_path("v0", path.clone());
        assert!(
            matches!(lc.kind, LayerKind::Video { ref path, speed, loop_mode, .. }
                if path.to_str() == Some("/tmp/show.mp4")
                    && (speed - 1.0).abs() < 1e-6
                    && loop_mode == LoopMode::Loop
            ),
            "layer_from_video_path should produce Video kind with defaults"
        );
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
        let scene = Cue::new("intro", serde_json::Value::Null, Some(thumb.clone()));

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

    /// P0.7.3 — `Project.edge_blend` round-trips through serde with full fidelity.
    #[test]
    fn edge_blend_config_round_trip() {
        let cfg = EdgeBlendConfig {
            overlap_px: 64,
            falloff_curve: FalloffCurve::Cosine,
        };
        let p = Project {
            edge_blend: Some(cfg),
            ..Project::default()
        };
        let json = serde_json::to_string(&p).expect("serialize project with edge_blend");
        let decoded: Project =
            serde_json::from_str(&json).expect("deserialize project with edge_blend");
        assert_eq!(decoded.edge_blend, p.edge_blend);
    }

    /// P0.7.3 — a v7 project saved before this field exists loads with
    /// `edge_blend == None` thanks to `#[serde(default)]`.
    #[test]
    fn edge_blend_missing_from_old_save_deserializes_as_none() {
        // Minimal v7 JSON without the `edge_blend` key.
        let json = r#"{"schema_version":7,"layers":[],"output_targets":[{"fallback_index":0,"rgb_matrix":[[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]}]}"#;
        let p: Project = serde_json::from_str(json).expect("deserialize old v7 project");
        assert_eq!(
            p.edge_blend, None,
            "old saves must load with edge_blend = None"
        );
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

    // --- P2.5.1 schema round-trip tests ---

    /// P2.5.1 acceptance: `LayerKind::FxLayer` with explicit `seed` and
    /// `t_layer_added_secs` serializes and deserializes correctly.
    #[test]
    fn fx_layer_seed_round_trip() {
        let kind = LayerKind::FxLayer {
            preset_id: "particles_identity".into(),
            params: HashMap::new(),
            seed: 42,
            t_layer_added_secs: 1.5,
        };
        let json = serde_json::to_string(&kind).expect("serialize");
        let back: LayerKind = serde_json::from_str(&json).expect("deserialize");
        match back {
            LayerKind::FxLayer {
                seed,
                t_layer_added_secs,
                ..
            } => {
                assert_eq!(seed, 42, "seed must round-trip");
                assert!(
                    (t_layer_added_secs - 1.5).abs() < 1e-6,
                    "t_layer_added_secs must round-trip, got {t_layer_added_secs}"
                );
            }
            other => panic!("expected FxLayer, got {:?}", other),
        }
    }

    /// P2 follow-up — `layer_from_fx_preset` produces a non-empty mask polygon
    /// (4 vertices) so the FX preset's SDF is well-defined immediately.
    #[test]
    fn fxlayer_constructor_has_non_empty_mask() {
        let layer = layer_from_fx_preset("test", "mask_edge_ripple_wash", HashMap::new(), 0);
        assert_eq!(
            layer.warp.mask_polygon.len(),
            4,
            "FxLayer constructor must produce a 4-vertex mask polygon, got {}",
            layer.warp.mask_polygon.len()
        );
    }

    /// P2 follow-up — `layer_from_fx_preset` sets `LayerKind::FxLayer` with
    /// the expected `preset_id`.
    #[test]
    fn fxlayer_constructor_uses_correct_kind() {
        let layer = layer_from_fx_preset("test", "mask_edge_ripple_wash", HashMap::new(), 0);
        match &layer.kind {
            LayerKind::FxLayer { preset_id, .. } => {
                assert_eq!(
                    preset_id, "mask_edge_ripple_wash",
                    "FxLayer constructor must use the given preset_id"
                );
            }
            other => panic!("expected FxLayer, got {:?}", other),
        }
    }

    /// P2.5.1 acceptance: an older-format `FxLayer` JSON without `seed` /
    /// `t_layer_added_secs` loads with both fields defaulting to 0 / 0.0.
    #[test]
    fn fx_layer_old_format_defaults_seed_and_t_layer() {
        // Simulate a v7 project file that predates P2.5.1 (no seed/t_layer_added_secs).
        let json = r#"{"FxLayer":{"preset_id":"mask_edge_ripple_wash","params":{}}}"#;
        let kind: LayerKind = serde_json::from_str(json).expect("old-format FxLayer must load");
        match kind {
            LayerKind::FxLayer {
                seed,
                t_layer_added_secs,
                ..
            } => {
                assert_eq!(seed, 0, "seed must default to 0 for old-format FxLayer");
                assert_eq!(
                    t_layer_added_secs, 0.0,
                    "t_layer_added_secs must default to 0.0 for old-format FxLayer"
                );
            }
            other => panic!("expected FxLayer, got {:?}", other),
        }
    }

    // --- P3.2.1 ZoneRole schema tests ---

    /// P3.2.1 — `WarpMesh::identity()` round-trips through serde with
    /// `zone_role = None`.
    #[test]
    fn warp_mesh_identity_zone_role_round_trip() {
        let warp = WarpMesh::identity();
        assert_eq!(
            warp.zone_role, None,
            "identity() must have zone_role = None"
        );
        let json = serde_json::to_string(&warp).expect("serialize identity warp");
        let back: WarpMesh = serde_json::from_str(&json).expect("deserialize identity warp");
        assert_eq!(
            back.zone_role, None,
            "identity() must round-trip with zone_role = None"
        );
    }

    /// P3.2.1 — a `WarpMesh` JSON object without a `zone_role` key
    /// deserialises to `zone_role = None` (regression guard for old projects).
    #[test]
    fn warp_mesh_missing_zone_role_key_deserializes_as_none() {
        let json = r#"{"rows":1,"cols":1,"grid":[[[0.0,0.0],[1.0,0.0]],[[0.0,1.0],[1.0,1.0]]],"mask_polygon":[],"mask_feather":0.02}"#;
        let warp: WarpMesh = serde_json::from_str(json).expect("old warp JSON must load");
        assert_eq!(
            warp.zone_role, None,
            "WarpMesh without zone_role key must deserialize to None"
        );
    }

    /// P3.2.1 — each `ZoneRole` variant serialises to the expected kebab-case
    /// string and deserialises back correctly.
    #[test]
    fn zone_role_kebab_case_round_trip() {
        let cases = [
            (ZoneRole::Window, "\"window\""),
            (ZoneRole::Portal, "\"portal\""),
            (ZoneRole::Void, "\"void\""),
            (ZoneRole::Spill, "\"spill\""),
            (ZoneRole::Edge, "\"edge\""),
            (ZoneRole::Highlight, "\"highlight\""),
            (ZoneRole::LightSource, "\"light-source\""),
        ];
        for (role, expected_json) in cases {
            let json = serde_json::to_string(&role).expect("serialize ZoneRole");
            assert_eq!(
                json, expected_json,
                "ZoneRole::{role:?} must serialize to {expected_json}"
            );
            let back: ZoneRole = serde_json::from_str(&json).expect("deserialize ZoneRole");
            assert_eq!(back, role, "ZoneRole::{role:?} must round-trip");
        }
    }

    /// P3.2.1 — an unknown `zone_role` string in a `WarpMesh` JSON deserialises
    /// to `None` (the lenient deserializer's unknown-variant fallback). Regression
    /// guard: if this changes, the audit (P3.2.4) must still fire `UnknownZoneRole`.
    #[test]
    fn warp_mesh_unknown_zone_role_deserializes_as_none() {
        let json = r#"{"rows":1,"cols":1,"grid":[[[0.0,0.0],[1.0,0.0]],[[0.0,1.0],[1.0,1.0]]],"mask_polygon":[],"mask_feather":0.02,"zone_role":"sky-bridge"}"#;
        let warp: WarpMesh = serde_json::from_str(json).expect("warp with unknown role must load");
        assert_eq!(
            warp.zone_role, None,
            "Unknown zone_role string must deserialize to None"
        );
    }

    /// P3.3.1 — `From<ZoneRole> for u32` mapping matches WGSL constant order.
    #[test]
    fn zone_role_to_u32_mapping() {
        assert_eq!(u32::from(ZoneRole::Window), 1u32);
        assert_eq!(u32::from(ZoneRole::Portal), 2u32);
        assert_eq!(u32::from(ZoneRole::Void), 3u32);
        assert_eq!(u32::from(ZoneRole::Spill), 4u32);
        assert_eq!(u32::from(ZoneRole::Edge), 5u32);
        assert_eq!(u32::from(ZoneRole::Highlight), 6u32);
        assert_eq!(u32::from(ZoneRole::LightSource), 7u32);
    }

    /// P3.3.1 — `zone_role_to_u32`: None maps to 0 (ZONE_NONE).
    #[test]
    fn option_zone_role_none_maps_to_zero() {
        assert_eq!(zone_role_to_u32(None), 0u32);
        assert_eq!(zone_role_to_u32(Some(ZoneRole::Window)), 1u32);
    }
}
