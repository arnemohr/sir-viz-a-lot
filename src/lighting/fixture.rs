//! P5.3.1 + P5.3.2 — Fixture model: `ChannelRole`, `FixturePersonality`,
//! `FixtureGroup`, `FixtureGroupId`, `PixelMap`, `OutputStrategy`, and
//! `FixtureSource`.
//!
//! All types are `Debug + Clone + Serialize + Deserialize` so they can
//! be persisted in the project JSON and cloned cheaply for Mutation Reverse
//! storage (per `src/project/CLAUDE.md`).
//!
//! `FixtureGroup` is added to `Project.fixture_groups: Vec<FixtureGroup>`
//! with `#[serde(default)]` so existing project files load without the field.
//!
//! Phase 7 extension contracts:
//! - Add new `ChannelRole` variants (White, ColorTemp, …) — additive,
//!   no migration needed.
//! - Add new `OutputStrategy` variants — additive.
//! - Add new `FixtureSource` variants — additive.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Stable identity for a fixture group within a project.
///
/// A newtype wrapping a monotonically incrementing `u64` (session-unique, not
/// globally unique). IDs are stable across undo/redo within a session;
/// they do not need to be globally unique across projects or machines.
/// Serialised as a `u64` in project JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FixtureGroupId(pub u64);

/// Global counter for generating session-unique `FixtureGroupId` values.
static FIXTURE_GROUP_COUNTER: AtomicU64 = AtomicU64::new(1);

impl FixtureGroupId {
    /// Generate a new session-unique `FixtureGroupId`.
    ///
    /// IDs are monotonically increasing within a process lifetime. The counter
    /// starts at 1 so the zero value can serve as a sentinel if needed.
    pub fn new_unique() -> Self {
        Self(FIXTURE_GROUP_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

// ---------------------------------------------------------------------------
// Channel role
// ---------------------------------------------------------------------------

/// The function of a single DMX channel within a fixture's footprint.
///
/// Phase 5 ships `Red`, `Green`, and `Blue`. Phase 7 adds `White`,
/// `ColorTemp`, `Intensity`, `Pan`, `Tilt`, and `Generic` additively —
/// no migration required because existing `Vec<ChannelRole>` channels
/// simply don't contain the new variants.
///
/// `#[non_exhaustive]` ensures match arms in the DMX writer must include
/// `_ => {}`, buying forward-compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ChannelRole {
    /// The red colour channel.
    Red,
    /// The green colour channel.
    Green,
    /// The blue colour channel.
    Blue,
    /// P7.9.2 — White channel for RGBW fixtures; receives the extracted white
    /// component computed by `apply_rgbw` when `RgbwConfig::enabled` is `true`.
    /// Writes zero when RGBW is disabled (backward-compatible: existing RGB
    /// personalities have no `White` in their channel map).
    White,
    // Phase 7 follow-on: ColorTemp, Intensity, Pan, Tilt, Generic(String)
}

// ---------------------------------------------------------------------------
// Fixture personality
// ---------------------------------------------------------------------------

/// A minimal fixture personality: the channel map plus a human-readable label.
///
/// Stored inline in `FixtureGroup` — not a separate lookup table in Phase 5.
/// `channels.len()` is the fixture's DMX footprint (channel count). The DMX
/// writer iterates `channels.iter().enumerate()` and writes the appropriate
/// byte value at `base_channel + fixture_offset + channel_idx`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixturePersonality {
    /// Channel map. Index `i` is the role of DMX channel `base_channel + i`
    /// within the fixture's footprint. Length == fixture's channel count.
    pub channels: Vec<ChannelRole>,
    /// Operator-supplied label shown in the fixture-group editor.
    /// Examples: "RGB par", "LED strip segment", "Generic 3ch".
    pub label: String,
}

impl FixturePersonality {
    /// Convenience constructor for a standard 3-channel RGB personality.
    pub fn default_rgb() -> Self {
        Self {
            channels: vec![ChannelRole::Red, ChannelRole::Green, ChannelRole::Blue],
            label: "RGB (3ch)".to_string(),
        }
    }

    /// P7.9.2 — Convenience constructor for a 4-channel RGBW personality.
    #[allow(dead_code)]
    pub fn default_rgbw() -> Self {
        Self {
            channels: vec![
                ChannelRole::Red,
                ChannelRole::Green,
                ChannelRole::Blue,
                ChannelRole::White,
            ],
            label: "RGBW (4ch)".to_string(),
        }
    }

    /// The fixture's DMX footprint (number of channels).
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

// ---------------------------------------------------------------------------
// Output strategy (P5.3.1 + P5.0.3)
// ---------------------------------------------------------------------------

/// How the sampled canvas colour is translated to DMX byte values per fixture.
///
/// Phase 5 ships `RgbDirect` and `HsvIntensityGate`. Phase 7 will add
/// `RgbwFill` and `ColorTemp` additively without breaking Phase 5 project files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStrategy {
    /// Scale the sampled sRGB pixel `(r, g, b)` directly to DMX byte values.
    /// What the camera sees is what the fixture emits. Phase 5 default.
    #[default]
    RgbDirect,
    /// Convert sampled pixel to HSV; gate the fixture's intensity by `V`
    /// (brightness) while keeping the hue/saturation from the source pixel.
    /// Useful for wash fixtures that should dim when the canvas is dark.
    HsvIntensityGate,
    // Phase 7: RgbwFill, ColorTemp, HsiDirect
}

// ---------------------------------------------------------------------------
// Fixture source (P5.3.1 + P5.6.1)
// ---------------------------------------------------------------------------

/// The colour source for a fixture group.
///
/// Phase 5 ships `CanvasRegion` (sample a UV rectangle on the rendered canvas)
/// and `ManualColor` (operator-set RGB). Phase 5 also unblocks `ZoneTag` now
/// that Phase 3 zones have landed — see P5.6.1.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureSource {
    /// Sample a UV-space rectangle of the composited canvas.
    ///
    /// `uv_min` and `uv_max` are normalised [0,1] coordinates in the canvas
    /// output texture. The lighting-tap buffer is sampled at the grid of UV
    /// coordinates derived from the assigned `PixelMap`.
    CanvasRegion {
        /// Top-left corner of the sample region in UV space.
        uv_min: (f32, f32),
        /// Bottom-right corner of the sample region in UV space.
        uv_max: (f32, f32),
    },
    /// Operator-specified constant RGB colour.
    ///
    /// Useful for single-colour wash fixtures that should not follow the
    /// canvas (e.g. a cue-fired manual gel).
    ManualColor { r: u8, g: u8, b: u8 },
    /// P5.6.1 — derive colour from the activity level of a semantic zone.
    ///
    /// `role` is the Zone role whose light-source / highlight activity
    /// drives the fixture intensity. Phase 3's `ZoneRole` enum (in
    /// `src/project/schema.rs`) covers `Window`, `Portal`, `Void`,
    /// `Spill`, `Edge`, `Highlight`, and `LightSource`.
    ///
    /// The sampling path in `src/lighting/color.rs` calls
    /// `zone_activity_to_color` for this variant.
    ZoneTag {
        role: crate::project::schema::ZoneRole,
    },
}

impl Default for FixtureSource {
    fn default() -> Self {
        // Default to the full canvas, a safe starting point for new groups.
        Self::CanvasRegion {
            uv_min: (0.0, 0.0),
            uv_max: (1.0, 1.0),
        }
    }
}

// ---------------------------------------------------------------------------
// Fixture group (P5.3.1)
// ---------------------------------------------------------------------------

/// A named collection of identical fixtures sharing a personality, DMX universe,
/// and canvas sampling region.
///
/// The operator's primary lighting object in rmap. Added to `Project.fixture_groups`
/// with `#[serde(default)]` so existing projects load cleanly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureGroup {
    /// Stable identity. Preserved across saves and undo/redo.
    pub id: FixtureGroupId,
    /// Operator-supplied label. Shown in the fixture-group editor.
    pub label: String,
    /// The channel map and label for each fixture in the group.
    /// All fixtures share the same personality (same channel count, same roles).
    pub personality: FixturePersonality,
    /// The Art-Net universe that holds this group's DMX channels.
    pub universe_id: crate::lighting::universe::UniverseId,
    /// DMX start address (0-indexed) of the first fixture in the group.
    /// The `n`th fixture starts at `base_channel + n * personality.channel_count()`.
    pub base_channel: u8,
    /// Number of fixtures in the group (1–16 for Phase 5; no hard limit in schema).
    pub fixture_count: u8,
    /// How the canvas colour is translated to DMX values.
    pub output_strategy: OutputStrategy,
    /// The colour source for this group (canvas region, manual, or zone tag).
    pub source: FixtureSource,
    /// P7.9.1 — RGBW + colour-temperature configuration.
    /// `enabled: false` (default) preserves the existing RGB-only output path.
    #[serde(default)]
    pub rgbw_config: crate::lighting::rgbw::RgbwConfig,
}

impl FixtureGroup {
    /// Create a new `FixtureGroup` with a standard 3-channel RGB personality,
    /// universe 1, base channel 0, and a full-canvas sampling region.
    pub fn new_default() -> Self {
        Self {
            id: FixtureGroupId::new_unique(),
            label: "New fixture group".to_string(),
            personality: FixturePersonality::default_rgb(),
            universe_id: crate::lighting::universe::UniverseId::default(),
            base_channel: 0,
            fixture_count: 1,
            output_strategy: OutputStrategy::RgbDirect,
            source: FixtureSource::default(),
            rgbw_config: crate::lighting::rgbw::RgbwConfig::default(),
        }
    }
}

/// Flat struct of all mutable fields in `FixtureGroup` except `id`.
///
/// Used by `Mutation::SetFixtureGroupParams` as the Reverse-storage payload —
/// the whole struct is cloned before the mutation and stored as the reverse.
/// `id` is excluded because the mutation targets a specific group by ID
/// and must not change the identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureGroupParams {
    pub label: String,
    pub personality: FixturePersonality,
    pub universe_id: crate::lighting::universe::UniverseId,
    pub base_channel: u8,
    pub fixture_count: u8,
    pub output_strategy: OutputStrategy,
    pub source: FixtureSource,
    /// P7.9.1 — RGBW + colour-temperature configuration.
    #[serde(default)]
    pub rgbw_config: crate::lighting::rgbw::RgbwConfig,
}

impl FixtureGroupParams {
    /// Extract the current params from a `FixtureGroup`.
    pub fn from_group(g: &FixtureGroup) -> Self {
        Self {
            label: g.label.clone(),
            personality: g.personality.clone(),
            universe_id: g.universe_id,
            base_channel: g.base_channel,
            fixture_count: g.fixture_count,
            output_strategy: g.output_strategy.clone(),
            source: g.source.clone(),
            rgbw_config: g.rgbw_config.clone(),
        }
    }

    /// Apply these params to a `FixtureGroup` in place.
    pub fn apply_to(&self, g: &mut FixtureGroup) {
        g.label = self.label.clone();
        g.personality = self.personality.clone();
        g.universe_id = self.universe_id;
        g.base_channel = self.base_channel;
        g.fixture_count = self.fixture_count;
        g.output_strategy = self.output_strategy.clone();
        g.source = self.source.clone();
        g.rgbw_config = self.rgbw_config.clone();
    }
}

// ---------------------------------------------------------------------------
// PixelMap (P5.3.2)
// ---------------------------------------------------------------------------

/// A grid of UV sample points spread across a canvas region.
///
/// Given a `FixtureGroup` with `source: CanvasRegion { uv_min, uv_max }`,
/// the `PixelMap { rows, cols }` subdivides the region into a `rows × cols`
/// grid of sample coordinates. The lighting thread averages the sampled
/// colours across all grid points to derive the fixture group's output colour.
///
/// Maximum samples per frame: 256 (`rows × cols` clamped by
/// `budget_samples` in `src/lighting/color.rs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PixelMap {
    /// Number of rows in the sample grid (≥ 1).
    pub rows: u8,
    /// Number of columns in the sample grid (≥ 1).
    pub cols: u8,
}

impl Default for PixelMap {
    fn default() -> Self {
        // Single-point sample at the centre of the region.
        Self { rows: 1, cols: 1 }
    }
}

impl PixelMap {
    /// Compute the list of UV coordinates for all `rows × cols` sample points.
    ///
    /// Sample points are distributed evenly across the region defined by
    /// `uv_min`/`uv_max`. For `rows == 1, cols == 1` the single point is
    /// at the centre of the region.
    ///
    /// Returns `rows × cols` UV pairs in row-major order.
    pub fn sample_uvs(&self, uv_min: (f32, f32), uv_max: (f32, f32)) -> Vec<(f32, f32)> {
        let rows = usize::from(self.rows).max(1);
        let cols = usize::from(self.cols).max(1);
        let mut out = Vec::with_capacity(rows * cols);

        let (u0, v0) = uv_min;
        let (u1, v1) = uv_max;

        for r in 0..rows {
            for c in 0..cols {
                // Cell centres: offset by half a step from the edges.
                let u = if rows == 1 {
                    (u0 + u1) / 2.0
                } else {
                    u0 + (u1 - u0) * (r as f32 + 0.5) / rows as f32
                };
                let v = if cols == 1 {
                    (v0 + v1) / 2.0
                } else {
                    v0 + (v1 - v0) * (c as f32 + 0.5) / cols as f32
                };
                out.push((u, v));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P5.3.1 — serde roundtrip for FixtureGroup.
    #[test]
    fn fixture_group_serde_roundtrip() {
        let group = FixtureGroup::new_default();
        let json = serde_json::to_string(&group).expect("serialize");
        let back: FixtureGroup = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.label, group.label);
        assert_eq!(back.base_channel, group.base_channel);
        assert_eq!(back.fixture_count, group.fixture_count);
        assert_eq!(back.personality.channels.len(), 3, "RGB has 3 channels");
    }

    /// P5.3.1 — CanvasRegion source roundtrips.
    #[test]
    fn canvas_region_source_roundtrip() {
        let src = FixtureSource::CanvasRegion {
            uv_min: (0.1, 0.2),
            uv_max: (0.8, 0.9),
        };
        let json = serde_json::to_string(&src).expect("serialize");
        let back: FixtureSource = serde_json::from_str(&json).expect("deserialize");
        match back {
            FixtureSource::CanvasRegion { uv_min, uv_max } => {
                assert!((uv_min.0 - 0.1_f32).abs() < 1e-5);
                assert!((uv_max.1 - 0.9_f32).abs() < 1e-5);
            }
            _ => panic!("wrong variant"),
        }
    }

    /// P5.3.1 — ManualColor source roundtrips.
    #[test]
    fn manual_color_source_roundtrip() {
        let src = FixtureSource::ManualColor {
            r: 255,
            g: 128,
            b: 0,
        };
        let json = serde_json::to_string(&src).expect("serialize");
        let back: FixtureSource = serde_json::from_str(&json).expect("deserialize");
        match back {
            FixtureSource::ManualColor { r, g, b } => {
                assert_eq!((r, g, b), (255, 128, 0));
            }
            _ => panic!("wrong variant"),
        }
    }

    /// P5.3.1 — ChannelRole serde uses snake_case.
    #[test]
    fn channel_role_serde_snake_case() {
        let role = ChannelRole::Red;
        let json = serde_json::to_string(&role).expect("serialize");
        assert_eq!(json, r#""red""#);
        let back: ChannelRole = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, ChannelRole::Red);
    }

    /// P5.3.1 — OutputStrategy default is RgbDirect.
    #[test]
    fn output_strategy_default_is_rgb_direct() {
        let s = OutputStrategy::default();
        assert_eq!(s, OutputStrategy::RgbDirect);
    }

    /// P5.3.2 — PixelMap 2×2 over [0,0..1,1] produces four corner-centres.
    #[test]
    fn pixel_map_2x2_produces_four_sample_uvs() {
        let pm = PixelMap { rows: 2, cols: 2 };
        let uvs = pm.sample_uvs((0.0, 0.0), (1.0, 1.0));
        assert_eq!(uvs.len(), 4, "2×2 grid has 4 sample points");
        // Each quadrant centre: (0.25, 0.25), (0.25, 0.75), (0.75, 0.25), (0.75, 0.75)
        let expected: [(f32, f32); 4] = [
            (0.25_f32, 0.25_f32),
            (0.25, 0.75),
            (0.75, 0.25),
            (0.75, 0.75),
        ];
        for (i, (got, exp)) in uvs.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got.0 - exp.0).abs() < 1e-5 && (got.1 - exp.1).abs() < 1e-5,
                "sample {i}: got {got:?}, expected {exp:?}"
            );
        }
    }

    /// P5.3.2 — PixelMap 1×1 returns the centre of the region.
    #[test]
    fn pixel_map_1x1_returns_centre() {
        let pm = PixelMap { rows: 1, cols: 1 };
        let uvs = pm.sample_uvs((0.2, 0.4), (0.8, 0.6));
        assert_eq!(uvs.len(), 1);
        assert!((uvs[0].0 - 0.5).abs() < 1e-5, "u should be centre 0.5");
        assert!((uvs[0].1 - 0.5).abs() < 1e-5, "v should be centre 0.5");
    }

    /// P5.6.1 — ZoneTag source roundtrips with ZoneRole enum.
    #[test]
    fn zone_tag_source_roundtrip() {
        use crate::project::schema::ZoneRole;
        let src = FixtureSource::ZoneTag {
            role: ZoneRole::LightSource,
        };
        let json = serde_json::to_string(&src).expect("serialize");
        let back: FixtureSource = serde_json::from_str(&json).expect("deserialize");
        match back {
            FixtureSource::ZoneTag { role } => {
                assert_eq!(role, ZoneRole::LightSource);
            }
            _ => panic!("wrong variant"),
        }
    }
}
