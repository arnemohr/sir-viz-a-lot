//! Light trail effect — glowing rainbow comet following an SVG path.
//!
//! This module defines [`Palette`] (serialisable colour strategy) and
//! [`LightTrailParams`] (GPU uniform buffer struct with manual
//! `to_wire_bytes()` packing). No bytemuck; follows the same convention
//! as `BlurParams` in `src/effects/blur.rs`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

/// Color strategy for the light trail.
///
/// - `Fixed(Vec<[u8; 4]>)` — RGBA palette array (up to 8 colors; extras are
///   silently truncated at GPU upload with a `tracing::warn!`).
/// - `HueShift { speed }` — continuously rotating hue; `speed` is in full
///   hue-wheel rotations per second.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Palette {
    Fixed(Vec<[u8; 4]>),
    HueShift { speed: f32 },
}

impl Default for Palette {
    fn default() -> Self {
        Palette::HueShift { speed: 0.2 }
    }
}

// ---------------------------------------------------------------------------
// LightTrailParams — GPU UBO, std140 / 192 bytes
// ---------------------------------------------------------------------------

/// GPU uniform buffer for the light trail shader.
///
/// Layout mirrors WGSL std140 rules (all fields are 4-byte scalars or
/// 4-component vectors whose alignment is 4 bytes in Rust with `#[repr(C)]`).
///
/// Total size: 15 scalars × 4 bytes + 1 pad × 4 bytes + 8 × 4 × 4 bytes
///           = 60 + 4 + 128 = **192 bytes**.
///
/// Use [`LightTrailParams::to_wire_bytes`] to produce the raw slice for
/// `queue.write_buffer`. **Do not use bytemuck** — this codebase intentionally
/// avoids it (see `src/test_patterns.rs:193`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LightTrailParams {
    /// Normalised position of the comet head along the path (0..1).
    pub progress: f32,
    /// Length of the visible tail as a fraction of total path length (0..1).
    pub trail_length: f32,
    /// Radius of the bright head core in render-target pixels.
    pub head_size: f32,
    /// Thickness of the trail core stroke in render-target pixels.
    pub stroke_width: f32,
    /// Gaussian standard-deviation-equivalent halo softness in render-target
    /// pixels (NOT path-space units — halo size shifts with projector resolution
    /// as expected).
    pub glow_blur: f32,
    /// Opacity falloff exponent from head (1.0) to tail end (0.0).
    pub opacity_fade: f32,
    /// Fraction of visible trail over which the palette is distributed.
    pub gradient_spread: f32,
    /// Lower bound of the animated subrange (0..1 of path).
    pub start: f32,
    /// Upper bound of the animated subrange (0..1 of path).
    pub end: f32,
    /// Non-zero to rotate the head sprite to the path tangent.
    pub align: u32,
    /// Which `<path>` element in the SVG to follow.
    pub path_index: u32,
    /// Arc-length polyline resolution (samples).
    pub sample_resolution: u32,
    /// Palette mode: 0 = Fixed, 1 = HueShift.
    pub palette_mode: u32,
    /// HueShift rotation speed (full rotations per second).
    pub hue_shift_speed: f32,
    /// Number of active entries in `palette_colors` (capped at 8).
    pub palette_len: u32,
    /// Padding to align `palette_colors` to 16 bytes.
    pub _pad0: u32,
    /// RGBA palette (f32 components, pre-normalised from u8). Unused slots
    /// are zeroed.
    pub palette_colors: [[f32; 4]; 8],
}

/// Maximum palette entries the GPU UBO can hold.
pub const MAX_PALETTE_COLORS: usize = 8;

impl LightTrailParams {
    /// Produce the 192-byte little-endian wire representation for
    /// `queue.write_buffer`. Packing order matches the struct field order
    /// under `#[repr(C)]`.
    ///
    /// No bytemuck — every field packed via `.to_le_bytes()` (following the
    /// `BlurParams::to_wire_bytes` convention).
    pub fn to_wire_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(192);

        macro_rules! push_f32 {
            ($v:expr) => {
                out.extend_from_slice(&($v as f32).to_le_bytes());
            };
        }
        macro_rules! push_u32 {
            ($v:expr) => {
                out.extend_from_slice(&($v as u32).to_le_bytes());
            };
        }

        push_f32!(self.progress);
        push_f32!(self.trail_length);
        push_f32!(self.head_size);
        push_f32!(self.stroke_width);
        push_f32!(self.glow_blur);
        push_f32!(self.opacity_fade);
        push_f32!(self.gradient_spread);
        push_f32!(self.start);
        push_f32!(self.end);
        push_u32!(self.align);
        push_u32!(self.path_index);
        push_u32!(self.sample_resolution);
        push_u32!(self.palette_mode);
        push_f32!(self.hue_shift_speed);
        push_u32!(self.palette_len);
        push_u32!(self._pad0);

        // 8 × [f32; 4] = 128 bytes
        for color in &self.palette_colors {
            push_f32!(color[0]);
            push_f32!(color[1]);
            push_f32!(color[2]);
            push_f32!(color[3]);
        }

        debug_assert_eq!(out.len(), 192, "LightTrailParams wire size must be 192");
        out
    }

    /// Build a `LightTrailParams` from the effect's individual fields plus a
    /// resolved `progress` value (pre-evaluated from the Modulator).
    ///
    /// Truncates `Fixed` palettes longer than 8 entries with a `warn!`.
    pub fn from_fields(
        progress: f32,
        trail_length: f32,
        head_size: f32,
        stroke_width: f32,
        glow_blur: f32,
        opacity_fade: f32,
        palette: &crate::effects::light_trail::Palette,
        gradient_spread: f32,
        start: f32,
        end: f32,
        align: bool,
        path_index: u32,
        sample_resolution: u32,
    ) -> Self {
        let (palette_mode, hue_shift_speed, palette_len, palette_colors) = match palette {
            Palette::Fixed(colors) => {
                if colors.len() > MAX_PALETTE_COLORS {
                    tracing::warn!(
                        count = colors.len(),
                        max = MAX_PALETTE_COLORS,
                        "LightTrail Fixed palette truncated to 8 colors for GPU upload"
                    );
                }
                let mut arr = [[0.0f32; 4]; MAX_PALETTE_COLORS];
                let n = colors.len().min(MAX_PALETTE_COLORS);
                for (i, rgba) in colors.iter().take(n).enumerate() {
                    arr[i] = [
                        rgba[0] as f32 / 255.0,
                        rgba[1] as f32 / 255.0,
                        rgba[2] as f32 / 255.0,
                        rgba[3] as f32 / 255.0,
                    ];
                }
                (0u32, 0.0f32, n as u32, arr)
            }
            Palette::HueShift { speed } => (1u32, *speed, 0u32, [[0.0f32; 4]; MAX_PALETTE_COLORS]),
        };

        Self {
            progress,
            trail_length,
            head_size,
            stroke_width,
            glow_blur,
            opacity_fade,
            gradient_spread,
            start,
            end,
            align: u32::from(align),
            path_index,
            sample_resolution,
            palette_mode,
            hue_shift_speed,
            palette_len,
            _pad0: 0,
            palette_colors,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::Effect;
    use crate::modulators::Modulator;

    // Helper: build a default LightTrail Effect.
    fn default_light_trail() -> Effect {
        Effect::LightTrail {
            progress: Modulator::Static(0.0),
            trail_length: 0.2,
            head_size: 12.0,
            stroke_width: 3.0,
            glow_blur: 8.0,
            opacity_fade: 0.7,
            palette: Palette::default(),
            gradient_spread: 1.0,
            start: 0.0,
            end: 1.0,
            align: false,
            path_index: 0,
            sample_resolution: 512,
        }
    }

    /// T2.2-test-1: serde round-trip for Effect::LightTrail at defaults.
    #[test]
    fn light_trail_effect_serde_round_trip() {
        let e = default_light_trail();
        let json = serde_json::to_string(&e).expect("serialise");
        let back: Effect = serde_json::from_str(&json).expect("deserialise");
        match back {
            Effect::LightTrail {
                trail_length,
                head_size,
                palette,
                ..
            } => {
                assert!((trail_length - 0.2).abs() < 1e-6);
                assert!((head_size - 12.0).abs() < 1e-6);
                assert_eq!(palette, Palette::HueShift { speed: 0.2 });
            }
            other => panic!("round-trip changed variant: {other:?}"),
        }
    }

    /// T2.2-test-2: serde round-trip for Palette::Fixed.
    #[test]
    fn palette_fixed_serde_round_trip() {
        let p = Palette::Fixed(vec![[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]]);
        let json = serde_json::to_string(&p).expect("serialise");
        let back: Palette = serde_json::from_str(&json).expect("deserialise");
        match &back {
            Palette::Fixed(colors) => {
                assert_eq!(colors.len(), 3);
                assert_eq!(colors[0], [255, 0, 0, 255]);
                assert_eq!(colors[1], [0, 255, 0, 255]);
                assert_eq!(colors[2], [0, 0, 255, 255]);
            }
            other => panic!("expected Fixed, got {other:?}"),
        }
    }

    /// T2.2-test-3: serde round-trip for Palette::HueShift.
    #[test]
    fn palette_hue_shift_serde_round_trip() {
        let p = Palette::HueShift { speed: 0.5 };
        let json = serde_json::to_string(&p).expect("serialise");
        let back: Palette = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back, Palette::HueShift { speed: 0.5 });
    }

    /// T2.2-test-4: range clamping at deserialise.
    #[test]
    fn light_trail_range_clamping() {
        // trail_length = 1.5 → clamped to 1.0
        // glow_blur = -5.0 → clamped to 0.0
        // head_size = 0.0 → clamped to 1.0 (lower bound is 1)
        let json = r#"{
            "LightTrail": {
                "progress": {"Static": 0.0},
                "trail_length": 1.5,
                "head_size": 0.0,
                "stroke_width": 3.0,
                "glow_blur": -5.0,
                "opacity_fade": 0.7,
                "palette": {"HueShift": {"speed": 0.2}},
                "gradient_spread": 1.0,
                "start": 0.0,
                "end": 1.0,
                "align": false,
                "path_index": 0,
                "sample_resolution": 512
            }
        }"#;
        let e: Effect = serde_json::from_str(json).expect("deserialise");
        match e {
            Effect::LightTrail {
                trail_length,
                head_size,
                glow_blur,
                ..
            } => {
                assert!(
                    (trail_length - 1.0).abs() < 1e-6,
                    "trail_length 1.5 should clamp to 1.0, got {trail_length}"
                );
                assert!(
                    (head_size - 1.0).abs() < 1e-6,
                    "head_size 0.0 should clamp to 1.0 (lower bound), got {head_size}"
                );
                assert!(
                    (glow_blur - 0.0).abs() < 1e-6,
                    "glow_blur -5.0 should clamp to 0.0, got {glow_blur}"
                );
            }
            other => panic!("expected LightTrail, got {other:?}"),
        }
    }

    /// T2.2-test-5: to_wire_bytes() returns exactly 192 bytes + size_of == 192.
    #[test]
    fn light_trail_params_wire_size() {
        assert_eq!(
            std::mem::size_of::<LightTrailParams>(),
            192,
            "LightTrailParams must be 192 bytes"
        );

        let p = LightTrailParams::from_fields(
            0.5,
            0.2,
            12.0,
            3.0,
            8.0,
            0.7,
            &Palette::default(),
            1.0,
            0.0,
            1.0,
            false,
            0,
            512,
        );
        let bytes = p.to_wire_bytes();
        assert_eq!(
            bytes.len(),
            192,
            "to_wire_bytes must return exactly 192 bytes"
        );
    }

    /// T2.2-test-6: 12-color Fixed palette is truncated to 8 on GPU upload.
    #[test]
    fn light_trail_palette_truncation() {
        let many_colors: Vec<[u8; 4]> = (0..12).map(|i| [i * 20, 0, 0, 255]).collect();
        let palette = Palette::Fixed(many_colors);
        let p = LightTrailParams::from_fields(
            0.0, 0.2, 12.0, 3.0, 8.0, 0.7, &palette, 1.0, 0.0, 1.0, false, 0, 512,
        );
        assert_eq!(
            p.palette_len, 8,
            "palette_len must be capped at 8, got {}",
            p.palette_len
        );
    }
}
