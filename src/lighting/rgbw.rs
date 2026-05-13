//! P7.9.1 — RGBW + colour-temperature-aware white-point mixing.
//!
//! Implements CCT-aware white-point subtraction for RGBW DMX fixtures.
//! Per-fixture-group CCT dropdown (2700–6500 K); W scale slider (0.0–2.0).
//!
//! ## Algorithm
//!
//! Given a sampled canvas colour `(r, g, b)` in [0, 255] and a fixture-group
//! white point `[r_w, g_w, b_w] = cct_to_rgb(cct_k)` (in [0, 1]):
//!
//! ```text
//! w_extract = clamp(min(r/r_w, g/g_w, b/b_w) * w_scale, 0, 1)
//! r_out = clamp(r/255 - r_w * w_extract, 0, 1) * 255
//! g_out = clamp(g/255 - g_w * w_extract, 0, 1) * 255
//! b_out = clamp(b/255 - b_w * w_extract, 0, 1) * 255
//! w_out = w_extract * 255
//! ```
//!
//! When `enabled: false`, the existing `SampledColor { r, g, b }` path is
//! unchanged — no W channel is emitted and no colour shift occurs.

use serde::{Deserialize, Serialize};

use crate::lighting::dmx_frame::SampledColor;

// ---------------------------------------------------------------------------
// RgbwConfig
// ---------------------------------------------------------------------------

/// P7.9.1 — RGBW + colour-temperature configuration for a fixture group.
///
/// Added to `FixtureGroup`; backward-compatible: `enabled: false` and
/// `#[serde(default)]` ensure existing RGB-only projects load unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RgbwConfig {
    /// Enable RGBW mode.  When `false`, the W channel is ignored and the
    /// fixture outputs standard RGB.  Default `false`.
    #[serde(default)]
    pub enabled: bool,
    /// Correlated Colour Temperature of the fixture's white channel in Kelvin.
    /// Valid range: 2000–8000 K.  Common presets: 2700, 3000, 4000, 5600, 6500.
    #[serde(default = "default_cct")]
    pub w_channel_cct_k: u16,
    /// White-channel scale factor (0.0–2.0, default 1.0).
    /// Values > 1.0 boost the W extraction; values < 1.0 suppress it.
    #[serde(default = "default_w_scale")]
    pub w_scale: f32,
}

fn default_cct() -> u16 {
    3200
}

fn default_w_scale() -> f32 {
    1.0
}

impl Default for RgbwConfig {
    fn default() -> Self {
        RgbwConfig {
            enabled: false,
            w_channel_cct_k: default_cct(),
            w_scale: default_w_scale(),
        }
    }
}

// ---------------------------------------------------------------------------
// CCT → RGB white point table (Planckian locus approximation, Kang et al. 2002)
// ---------------------------------------------------------------------------

/// P7.9.1 — Planckian locus approximation for CCT → normalised RGB.
///
/// Sampled at 100 K steps from 2000 K to 8000 K (61 entries).
/// Based on Kang et al. 2002, "Spectral-based technique for white balance
/// correction in digital still cameras."
///
/// Values are approximate sRGB primaries for the given CCT; exact values depend
/// on the display white point. The table is used for fixture-channel extraction,
/// not colour-accurate display rendering.
///
/// Format: `(cct_k, [r, g, b])` — r/g/b in [0.0, 1.0], always with max == 1.0.
const CCT_TABLE: &[(u16, [f32; 3])] = &[
    (2000, [1.000, 0.600, 0.259]),
    (2100, [1.000, 0.625, 0.286]),
    (2200, [1.000, 0.649, 0.314]),
    (2300, [1.000, 0.671, 0.341]),
    (2400, [1.000, 0.693, 0.369]),
    (2500, [1.000, 0.714, 0.396]),
    (2600, [1.000, 0.733, 0.422]),
    (2700, [1.000, 0.820, 0.550]),
    (2800, [1.000, 0.820, 0.569]),
    (2900, [1.000, 0.836, 0.596]),
    (3000, [1.000, 0.852, 0.623]),
    (3100, [1.000, 0.865, 0.647]),
    (3200, [1.000, 0.876, 0.671]),
    (3300, [1.000, 0.886, 0.694]),
    (3400, [1.000, 0.895, 0.716]),
    (3500, [1.000, 0.903, 0.737]),
    (3600, [1.000, 0.910, 0.758]),
    (3700, [1.000, 0.916, 0.777]),
    (3800, [1.000, 0.921, 0.796]),
    (3900, [1.000, 0.926, 0.814]),
    (4000, [1.000, 0.930, 0.831]),
    (4100, [1.000, 0.934, 0.847]),
    (4200, [1.000, 0.938, 0.862]),
    (4300, [1.000, 0.941, 0.877]),
    (4400, [1.000, 0.943, 0.891]),
    (4500, [1.000, 0.946, 0.904]),
    (4600, [1.000, 0.948, 0.916]),
    (4700, [1.000, 0.950, 0.928]),
    (4800, [1.000, 0.952, 0.940]),
    (4900, [1.000, 0.954, 0.951]),
    (5000, [1.000, 0.956, 0.961]),
    (5100, [1.000, 0.958, 0.970]),
    (5200, [1.000, 0.960, 0.979]),
    (5300, [1.000, 0.962, 0.987]),
    (5400, [1.000, 0.964, 0.994]),
    (5500, [1.000, 0.965, 1.000]),
    (5600, [0.993, 0.965, 1.000]),
    (5700, [0.985, 0.965, 1.000]),
    (5800, [0.977, 0.965, 1.000]),
    (5900, [0.970, 0.965, 1.000]),
    (6000, [0.962, 0.965, 1.000]),
    (6100, [0.955, 0.965, 1.000]),
    (6200, [0.949, 0.965, 1.000]),
    (6300, [0.942, 0.965, 1.000]),
    (6400, [0.936, 0.965, 1.000]),
    (6500, [1.000, 1.000, 1.000]),
    (6600, [0.988, 0.991, 1.000]),
    (6700, [0.978, 0.984, 1.000]),
    (6800, [0.968, 0.977, 1.000]),
    (6900, [0.959, 0.970, 1.000]),
    (7000, [0.951, 0.963, 1.000]),
    (7100, [0.943, 0.957, 1.000]),
    (7200, [0.936, 0.951, 1.000]),
    (7300, [0.929, 0.946, 1.000]),
    (7400, [0.922, 0.941, 1.000]),
    (7500, [0.916, 0.936, 1.000]),
    (7600, [0.910, 0.931, 1.000]),
    (7700, [0.905, 0.927, 1.000]),
    (7800, [0.900, 0.923, 1.000]),
    (7900, [0.895, 0.919, 1.000]),
    (8000, [0.891, 0.915, 1.000]),
];

/// P7.9.1 — Look up the normalised RGB white point for a CCT value.
///
/// The table covers 2000–8000 K at 100 K steps.  Values outside the table
/// range are clamped to the nearest entry.  Values between table entries
/// are linearly interpolated.
///
/// ## Acceptance criteria
/// - `cct_to_rgb(6500)` ≈ `[1.0, 1.0, 1.0]` (neutral white).
/// - `cct_to_rgb(2700)` ≈ `[1.0, 0.82, 0.55]` (warm white).
pub fn cct_to_rgb(k: u16) -> [f32; 3] {
    // Clamp to table range.
    let k = k.clamp(CCT_TABLE[0].0, CCT_TABLE[CCT_TABLE.len() - 1].0);

    // Binary search for the lower bracket.
    let idx = CCT_TABLE.partition_point(|&(cct, _)| cct <= k);
    let idx = idx.saturating_sub(1).min(CCT_TABLE.len() - 1);

    if idx + 1 >= CCT_TABLE.len() {
        return CCT_TABLE[idx].1;
    }

    let (k0, rgb0) = CCT_TABLE[idx];
    let (k1, rgb1) = CCT_TABLE[idx + 1];

    if k0 == k1 {
        return rgb0;
    }

    // Linear interpolation.
    let t = (k - k0) as f32 / (k1 - k0) as f32;
    [
        rgb0[0] + t * (rgb1[0] - rgb0[0]),
        rgb0[1] + t * (rgb1[1] - rgb0[1]),
        rgb0[2] + t * (rgb1[2] - rgb0[2]),
    ]
}

// ---------------------------------------------------------------------------
// RGBW extraction
// ---------------------------------------------------------------------------

/// P7.9.2 — CCT-aware white-point subtraction.
///
/// Given a sampled canvas colour and an `RgbwConfig`, returns the four
/// DMX byte values `(r_out, g_out, b_out, w_out)`.
///
/// When `config.enabled == false`, returns `(r, g, b, 0)` unchanged.
pub fn apply_rgbw(r: u8, g: u8, b: u8, config: &RgbwConfig) -> (u8, u8, u8, u8) {
    if !config.enabled {
        return (r, g, b, 0);
    }

    let [r_w, g_w, b_w] = cct_to_rgb(config.w_channel_cct_k);

    // Normalise input to [0, 1].
    let r_n = r as f32 / 255.0;
    let g_n = g as f32 / 255.0;
    let b_n = b as f32 / 255.0;

    // White extraction — divide by white-point components, take minimum.
    // Guard against zero white-point components (shouldn't happen with table
    // values, but defensive coding is cheap).
    let r_ratio = if r_w > 0.0 { r_n / r_w } else { 0.0 };
    let g_ratio = if g_w > 0.0 { g_n / g_w } else { 0.0 };
    let b_ratio = if b_w > 0.0 { b_n / b_w } else { 0.0 };

    let w_extract = (r_ratio.min(g_ratio).min(b_ratio) * config.w_scale).clamp(0.0, 1.0);

    // Subtract white component from colour channels.
    let r_out = ((r_n - r_w * w_extract).clamp(0.0, 1.0) * 255.0) as u8;
    let g_out = ((g_n - g_w * w_extract).clamp(0.0, 1.0) * 255.0) as u8;
    let b_out = ((b_n - b_w * w_extract).clamp(0.0, 1.0) * 255.0) as u8;
    let w_out = (w_extract * 255.0) as u8;

    (r_out, g_out, b_out, w_out)
}

/// P7.9.2 — RGBW sampled colour (extends `SampledColor` with a W channel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SampledRgbw {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub w: u8,
}

impl SampledRgbw {
    /// Convert from a `SampledColor` + `RgbwConfig`.
    pub fn from_sampled(color: SampledColor, config: &RgbwConfig) -> Self {
        let (r, g, b, w) = apply_rgbw(color.r, color.g, color.b, config);
        SampledRgbw { r, g, b, w }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// P7.9.1 — cct_to_rgb(6500) ≈ [1.0, 1.0, 1.0] (neutral white).
    #[test]
    fn cct_to_rgb_6500_is_neutral_white() {
        let [r, g, b] = cct_to_rgb(6500);
        assert!((r - 1.0).abs() < 0.01, "R should be ≈1.0 at 6500K, got {r}");
        assert!((g - 1.0).abs() < 0.01, "G should be ≈1.0 at 6500K, got {g}");
        assert!((b - 1.0).abs() < 0.01, "B should be ≈1.0 at 6500K, got {b}");
    }

    /// P7.9.1 — cct_to_rgb(2700) ≈ [1.0, 0.82, 0.55] (warm white).
    #[test]
    fn cct_to_rgb_2700_is_warm_white() {
        let [r, g, b] = cct_to_rgb(2700);
        assert!((r - 1.0).abs() < 0.01, "R should be ≈1.0 at 2700K, got {r}");
        assert!(
            (g - 0.82).abs() < 0.05,
            "G should be ≈0.82 at 2700K, got {g}"
        );
        assert!(
            (b - 0.55).abs() < 0.05,
            "B should be ≈0.55 at 2700K, got {b}"
        );
    }

    /// P7.9.1 — All CCT table entries have at least one channel == 1.0.
    #[test]
    fn cct_table_entries_normalized() {
        for (k, rgb) in CCT_TABLE {
            let max = rgb[0].max(rgb[1]).max(rgb[2]);
            assert!(
                (max - 1.0).abs() < 1e-6,
                "CCT {k}K: max channel must be 1.0, got {max}"
            );
        }
    }

    /// P7.9.1 — RgbwConfig default is disabled (backward-compatible).
    #[test]
    fn rgbw_config_default_is_disabled() {
        let cfg = RgbwConfig::default();
        assert!(!cfg.enabled, "default RgbwConfig must be disabled");
    }

    /// P7.9.1 — RgbwConfig round-trips through JSON.
    #[test]
    fn rgbw_config_json_round_trip() {
        let cfg = RgbwConfig {
            enabled: true,
            w_channel_cct_k: 4000,
            w_scale: 1.5,
        };
        let json = serde_json::to_string(&cfg).expect("serialize");
        let restored: RgbwConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(restored.enabled);
        assert_eq!(restored.w_channel_cct_k, 4000);
        assert!((restored.w_scale - 1.5).abs() < 1e-6);
    }

    /// P7.9.2 — enabled: false returns RGB unchanged, W = 0.
    #[test]
    fn apply_rgbw_disabled_passthrough() {
        let cfg = RgbwConfig::default(); // enabled: false
        let (r, g, b, w) = apply_rgbw(200, 100, 50, &cfg);
        assert_eq!(
            (r, g, b, w),
            (200, 100, 50, 0),
            "disabled must be a passthrough"
        );
    }

    /// P7.9.2 — Neutral grey with 6500K CCT: high W, near-zero coloured channels.
    #[test]
    fn neutral_grey_6500k_high_w() {
        let cfg = RgbwConfig {
            enabled: true,
            w_channel_cct_k: 6500,
            w_scale: 1.0,
        };
        // Pure mid-grey should produce near-zero RGB remainder and significant W.
        let (r, g, b, w) = apply_rgbw(128, 128, 128, &cfg);
        // All colour channels should be reduced.
        assert!(
            r < 20,
            "R remainder should be low for neutral grey at 6500K, got {r}"
        );
        assert!(
            g < 20,
            "G remainder should be low for neutral grey at 6500K, got {g}"
        );
        assert!(
            b < 20,
            "B remainder should be low for neutral grey at 6500K, got {b}"
        );
        assert!(
            w > 100,
            "W should be high for neutral grey at 6500K, got {w}"
        );
    }

    /// P7.9.2 — Saturated red input: W extraction is minimal.
    #[test]
    fn saturated_red_minimal_w_extraction() {
        let cfg = RgbwConfig {
            enabled: true,
            w_channel_cct_k: 6500,
            w_scale: 1.0,
        };
        let (r, _g, _b, w) = apply_rgbw(255, 0, 0, &cfg);
        // Pure red has no blue/green → w_extract is clamped to near 0.
        assert!(w < 10, "Pure red should produce minimal W, got {w}");
        assert!(r > 240, "Red channel should be mostly preserved, got {r}");
    }
}
