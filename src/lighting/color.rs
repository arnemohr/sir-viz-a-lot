//! P5.4.3 — `LightingTapBuffer` + `sample_and_convert`.
//! P5.4.4 — Per-fixture sample budget enforcement.
//! P5.6.2 — Zone-activity → DMX intensity mapping.
//!
//! The `LightingTapBuffer` holds the 64×36 RGBA8Unorm pixels produced by
//! the lighting-tap render pass (P5.4.1). `sample_and_convert` reads one
//! pixel from the buffer and converts it to a `SampledColor` using the
//! operator's chosen `ColorStrategy`.
//!
//! `budget_samples` averages a `PixelMap` grid of UV coordinates over the
//! buffer, capped at 256 samples per fixture group per frame.
//!
//! `zone_activity_to_color` converts a zone's [0,1] activity level to a
//! `SampledColor` for zone-derived fixtures (P5.6.2).

use std::sync::atomic::{AtomicBool, Ordering};

use crate::lighting::dmx_frame::SampledColor;
use crate::lighting::fixture::{FixtureGroup, FixtureSource, OutputStrategy, PixelMap};

// ---------------------------------------------------------------------------
// Tap buffer dimensions (must match the decision doc + P5.1.2 constants)
// ---------------------------------------------------------------------------

/// Width of the lighting-tap texture in pixels.
pub const TAP_WIDTH: usize = 64;
/// Height of the lighting-tap texture in pixels.
pub const TAP_HEIGHT: usize = 36;
/// Total byte count of the lighting-tap staging buffer (RGBA8Unorm).
pub const TAP_BUFFER_BYTES: usize = TAP_WIDTH * TAP_HEIGHT * 4;

/// Maximum sample points per fixture group per frame.
/// Groups with more than 256 `PixelMap` cells are clamped; a `tracing::warn!`
/// is emitted once per process lifetime (via `AtomicBool` guard).
pub const MAX_SAMPLES: usize = 256;

// One-shot warning guard so we don't flood the log.
static SAMPLE_BUDGET_WARN_EMITTED: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// LightingTapBuffer
// ---------------------------------------------------------------------------

/// A snapshot of the 64×36 RGBA8Unorm lighting-tap texture as a flat byte
/// array (row-major, RGBA packing).
///
/// Allocated once at startup by `LightingReadback` (P5.4.2); passed to
/// `sample_and_convert` on the lighting thread tick.
#[derive(Clone)]
pub struct LightingTapBuffer(pub [u8; TAP_BUFFER_BYTES]);

impl LightingTapBuffer {
    /// Construct a zeroed buffer (e.g. before the first readback completes).
    pub fn zeroed() -> Self {
        Self([0u8; TAP_BUFFER_BYTES])
    }

    /// Read the `(r, g, b)` bytes at a pixel coordinate `(x, y)`.
    ///
    /// `x` must be in `0..TAP_WIDTH`, `y` in `0..TAP_HEIGHT`.
    /// Out-of-range coordinates are clamped.
    pub fn pixel(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let x = x.min(TAP_WIDTH - 1);
        let y = y.min(TAP_HEIGHT - 1);
        let idx = (y * TAP_WIDTH + x) * 4;
        (self.0[idx], self.0[idx + 1], self.0[idx + 2])
    }
}

impl Default for LightingTapBuffer {
    fn default() -> Self {
        Self::zeroed()
    }
}

// ---------------------------------------------------------------------------
// ColorStrategy (mirrors the decision doc API; re-exported for callers)
// ---------------------------------------------------------------------------

/// The colour-space conversion strategy for a fixture group.
///
/// Maps from `OutputStrategy` (which is stored in the project) — they are
/// equivalent in Phase 5. `ColorStrategy` is the internal sampling-layer type
/// so the colour module doesn't depend on the fixture schema directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorStrategy {
    /// Scale the sampled sRGB pixel `(r, g, b)` directly to DMX values.
    RgbDirect,
    /// Convert to HSV; scale `r, g, b` by `V` (brightness); return scaled.
    HsvIntensityGate,
}

impl From<&OutputStrategy> for ColorStrategy {
    fn from(s: &OutputStrategy) -> Self {
        match s {
            OutputStrategy::RgbDirect => ColorStrategy::RgbDirect,
            OutputStrategy::HsvIntensityGate => ColorStrategy::HsvIntensityGate,
        }
    }
}

// ---------------------------------------------------------------------------
// sample_and_convert (P5.4.3)
// ---------------------------------------------------------------------------

/// Sample the lighting-tap buffer at a normalised UV coordinate and apply the
/// operator's chosen colour strategy.
///
/// `uv = (0.0, 0.0)` is the top-left corner; `(1.0, 1.0)` is the bottom-right.
/// UV coordinates are clamped to `[0, 1]` before the pixel lookup.
///
/// Pure CPU math; no allocation; safe to call from the lighting thread.
pub fn sample_and_convert(
    tap: &LightingTapBuffer,
    uv: (f32, f32),
    strategy: ColorStrategy,
) -> SampledColor {
    // Clamp UV to [0, 1].
    let u = uv.0.clamp(0.0, 1.0);
    let v = uv.1.clamp(0.0, 1.0);

    // Map to pixel indices. The spec formula:
    // `x = (u * 63) as usize`, `y = (v * 35) as usize`.
    let x = (u * (TAP_WIDTH - 1) as f32) as usize;
    let y = (v * (TAP_HEIGHT - 1) as f32) as usize;

    let (r, g, b) = tap.pixel(x, y);

    apply_strategy(r, g, b, strategy)
}

/// Apply a `ColorStrategy` to raw `(r, g, b)` bytes.
fn apply_strategy(r: u8, g: u8, b: u8, strategy: ColorStrategy) -> SampledColor {
    match strategy {
        ColorStrategy::RgbDirect => SampledColor { r, g, b },
        ColorStrategy::HsvIntensityGate => {
            // Convert RGB → HSV, extract V (brightness), scale r/g/b by V.
            let v = rgb_to_value(r, g, b);
            SampledColor {
                r: scale_by(r, v),
                g: scale_by(g, v),
                b: scale_by(b, v),
            }
        }
    }
}

/// Compute the HSV `V` (value/brightness) from `(r, g, b)` bytes.
///
/// V = max(r, g, b) / 255.0 (standard HSV definition).
/// Pure integer → float math; no heap allocation.
fn rgb_to_value(r: u8, g: u8, b: u8) -> f32 {
    r.max(g).max(b) as f32 / 255.0
}

/// Scale a byte value by a float factor in [0.0, 1.0], rounding down.
#[inline]
fn scale_by(channel: u8, factor: f32) -> u8 {
    (channel as f32 * factor) as u8
}

// ---------------------------------------------------------------------------
// budget_samples (P5.4.4)
// ---------------------------------------------------------------------------

/// Average the `rows × cols` UV sample results from `sample_and_convert`.
///
/// For a `FixtureGroup` with `source: CanvasRegion`, this derives the group's
/// output colour by averaging all grid-point samples from its `PixelMap`.
///
/// The maximum number of samples is capped at [`MAX_SAMPLES`] (256). A group
/// whose `PixelMap` exceeds 256 cells is clamped and a `tracing::warn!` is
/// emitted once per process lifetime.
///
/// For non-`CanvasRegion` sources (`ManualColor`, `ZoneTag`) the caller
/// should use other paths; this function falls back to zeroed output for
/// those variants.
pub fn budget_samples(
    group: &FixtureGroup,
    pixel_map: &PixelMap,
    tap: &LightingTapBuffer,
    strategy: ColorStrategy,
) -> SampledColor {
    let (uv_min, uv_max) = match &group.source {
        FixtureSource::CanvasRegion { uv_min, uv_max } => (*uv_min, *uv_max),
        FixtureSource::ManualColor { r, g, b } => {
            return SampledColor {
                r: *r,
                g: *g,
                b: *b,
            };
        }
        FixtureSource::ZoneTag { .. } => {
            // Zone-derived path handled by zone_activity_to_color (P5.6.2).
            return SampledColor::default();
        }
    };

    let uvs = pixel_map.sample_uvs(uv_min, uv_max);
    let total = uvs.len();

    // Enforce the per-frame sample budget.
    if total > MAX_SAMPLES && !SAMPLE_BUDGET_WARN_EMITTED.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            group_label = %group.label,
            samples = total,
            max = MAX_SAMPLES,
            "fixture group PixelMap exceeds sample budget; clamping to {} samples",
            MAX_SAMPLES,
        );
    }

    let sample_count = total.min(MAX_SAMPLES);
    if sample_count == 0 {
        return SampledColor::default();
    }

    let mut r_sum: u32 = 0;
    let mut g_sum: u32 = 0;
    let mut b_sum: u32 = 0;

    for uv in uvs.iter().take(sample_count) {
        let s = sample_and_convert(tap, *uv, strategy);
        r_sum += u32::from(s.r);
        g_sum += u32::from(s.g);
        b_sum += u32::from(s.b);
    }

    let n = sample_count as u32;
    SampledColor {
        r: (r_sum / n) as u8,
        g: (g_sum / n) as u8,
        b: (b_sum / n) as u8,
    }
}

// ---------------------------------------------------------------------------
// zone_activity_to_color (P5.6.2)
// ---------------------------------------------------------------------------

/// Convert a zone's normalised activity level `[0.0, 1.0]` to a `SampledColor`.
///
/// Used for `FixtureSource::ZoneTag` groups where fixture intensity follows
/// the zone's `light-source` / `highlight` activity.
///
/// - `RgbDirect`: white wash scaled by `activity` — `(255*a, 255*a, 255*a)`.
/// - `HsvIntensityGate`: same semantics (activity IS the HSV V factor here).
///
/// `activity` is clamped to `[0.0, 1.0]` before conversion.
pub fn zone_activity_to_color(activity: f32, strategy: ColorStrategy) -> SampledColor {
    let a = activity.clamp(0.0, 1.0);
    let v = (255.0 * a) as u8;
    // Both strategies produce a white wash scaled by activity for zone-derived
    // fixtures. HsvIntensityGate would normally also track hue/saturation from
    // the canvas, but zone-derived intensity is intentionally a white wash.
    let _ = strategy; // used uniformly for white wash in P5.6.2
    SampledColor { r: v, g: v, b: v }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::fixture::{
        FixtureGroup, FixturePersonality, FixtureSource, OutputStrategy,
    };
    use crate::lighting::universe::UniverseId;

    fn make_tap_solid(r: u8, g: u8, b: u8) -> LightingTapBuffer {
        let mut buf = [0u8; TAP_BUFFER_BYTES];
        for chunk in buf.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = 255;
        }
        LightingTapBuffer(buf)
    }

    fn canvas_group(uv_min: (f32, f32), uv_max: (f32, f32)) -> FixtureGroup {
        use crate::lighting::fixture::FixtureGroupId;
        FixtureGroup {
            id: FixtureGroupId(1),
            label: "test".to_string(),
            personality: FixturePersonality::default_rgb(),
            universe_id: UniverseId(1),
            base_channel: 0,
            fixture_count: 1,
            output_strategy: OutputStrategy::RgbDirect,
            source: FixtureSource::CanvasRegion { uv_min, uv_max },
        }
    }

    // --- P5.4.3 tests ---

    /// P5.4.3 — RgbDirect returns exact bytes.
    #[test]
    fn rgb_direct_returns_exact_bytes() {
        let tap = make_tap_solid(200, 100, 50);
        let result = sample_and_convert(&tap, (0.5, 0.5), ColorStrategy::RgbDirect);
        assert_eq!((result.r, result.g, result.b), (200, 100, 50));
    }

    /// P5.4.3 — HsvIntensityGate on a black pixel returns (0, 0, 0).
    #[test]
    fn hsv_intensity_gate_black_returns_zero() {
        let tap = make_tap_solid(0, 0, 0);
        let result = sample_and_convert(&tap, (0.5, 0.5), ColorStrategy::HsvIntensityGate);
        assert_eq!((result.r, result.g, result.b), (0, 0, 0));
    }

    /// P5.4.3 — HsvIntensityGate on a white pixel returns (255, 255, 255).
    #[test]
    fn hsv_intensity_gate_white_returns_255() {
        let tap = make_tap_solid(255, 255, 255);
        let result = sample_and_convert(&tap, (0.5, 0.5), ColorStrategy::HsvIntensityGate);
        // V = max(255,255,255)/255 = 1.0; scale_by(255, 1.0) = 255.
        assert_eq!((result.r, result.g, result.b), (255, 255, 255));
    }

    /// P5.4.3 — UV (0,0) reads top-left pixel; (1,1) reads bottom-right.
    #[test]
    fn uv_extremes_read_correct_pixels() {
        let mut tap = LightingTapBuffer::zeroed();
        // Set top-left pixel to red.
        tap.0[0] = 200;
        tap.0[1] = 0;
        tap.0[2] = 0;
        tap.0[3] = 255;
        // Set bottom-right pixel (63, 35) to blue.
        let br_idx = (35 * TAP_WIDTH + 63) * 4;
        tap.0[br_idx] = 0;
        tap.0[br_idx + 1] = 0;
        tap.0[br_idx + 2] = 150;
        tap.0[br_idx + 3] = 255;

        let tl = sample_and_convert(&tap, (0.0, 0.0), ColorStrategy::RgbDirect);
        assert_eq!((tl.r, tl.g, tl.b), (200, 0, 0), "top-left should be red");

        let br = sample_and_convert(&tap, (1.0, 1.0), ColorStrategy::RgbDirect);
        assert_eq!(
            (br.r, br.g, br.b),
            (0, 0, 150),
            "bottom-right should be blue"
        );
    }

    // --- P5.4.4 tests ---

    /// P5.4.4 — PixelMap 16×16 = 256 samples (no clamp).
    #[test]
    fn pixel_map_256_samples_no_clamp() {
        let tap = make_tap_solid(100, 150, 200);
        let group = canvas_group((0.0, 0.0), (1.0, 1.0));
        let pm = PixelMap { rows: 16, cols: 16 };
        let result = budget_samples(&group, &pm, &tap, ColorStrategy::RgbDirect);
        // All pixels are the same colour; average == that colour.
        assert_eq!((result.r, result.g, result.b), (100, 150, 200));
    }

    /// P5.4.4 — PixelMap 17×17 is clamped (> 256 samples) but still returns
    /// an averaged result (not zeroed).
    #[test]
    fn pixel_map_over_budget_is_clamped_not_zeroed() {
        let tap = make_tap_solid(80, 160, 240);
        let group = canvas_group((0.0, 0.0), (1.0, 1.0));
        let pm = PixelMap { rows: 17, cols: 17 };
        // 17×17 = 289 > 256; should be clamped but still produce a result.
        let result = budget_samples(&group, &pm, &tap, ColorStrategy::RgbDirect);
        // Solid colour tap → average is the same colour regardless of sample count.
        assert_eq!((result.r, result.g, result.b), (80, 160, 240));
    }

    /// P5.4.4 — ManualColor source bypasses the tap and returns the fixed colour.
    #[test]
    fn manual_color_source_returns_fixed_color() {
        let tap = make_tap_solid(0, 0, 0); // tap is all black
        let mut group = canvas_group((0.0, 0.0), (1.0, 1.0));
        group.source = FixtureSource::ManualColor {
            r: 128,
            g: 64,
            b: 32,
        };
        let pm = PixelMap { rows: 1, cols: 1 };
        let result = budget_samples(&group, &pm, &tap, ColorStrategy::RgbDirect);
        assert_eq!((result.r, result.g, result.b), (128, 64, 32));
    }

    // --- P5.6.2 tests ---

    /// P5.6.2 — activity = 0.0 → all zeros.
    #[test]
    fn zone_activity_zero_is_black() {
        let c = zone_activity_to_color(0.0, ColorStrategy::RgbDirect);
        assert_eq!((c.r, c.g, c.b), (0, 0, 0));
    }

    /// P5.6.2 — activity = 1.0 → all 255.
    #[test]
    fn zone_activity_one_is_white() {
        let c = zone_activity_to_color(1.0, ColorStrategy::RgbDirect);
        assert_eq!((c.r, c.g, c.b), (255, 255, 255));
    }

    /// P5.6.2 — activity = 0.5 → approximately 127.
    #[test]
    fn zone_activity_half_is_mid() {
        let c = zone_activity_to_color(0.5, ColorStrategy::RgbDirect);
        // (255 * 0.5) as u8 = 127.
        assert_eq!((c.r, c.g, c.b), (127, 127, 127));
    }

    /// P5.6.2 — HsvIntensityGate produces same result as RgbDirect for zone activity.
    #[test]
    fn zone_activity_hsv_same_as_rgb() {
        let rgb = zone_activity_to_color(0.75, ColorStrategy::RgbDirect);
        let hsv = zone_activity_to_color(0.75, ColorStrategy::HsvIntensityGate);
        assert_eq!((rgb.r, rgb.g, rgb.b), (hsv.r, hsv.g, hsv.b));
    }
}
