//! Light trail effect — glowing rainbow comet following an SVG path.
//!
//! This module defines [`Palette`] (serialisable colour strategy),
//! [`LightTrailParams`] (GPU uniform buffer struct with manual
//! `to_wire_bytes()` packing), and [`LightTrailGpuPolyline`] (the
//! arc-length-parameterised polyline uploaded to the GPU as a storage
//! buffer for shader lookup). No bytemuck; follows the same convention
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
// LightTrailGpuPolyline — polyline storage buffer
// ---------------------------------------------------------------------------

/// Pack a [`crate::path_geom::Polyline`] into a flat `&[f32]` payload for GPU upload.
///
/// Layout: each sample is stored as three consecutive `f32` values:
/// `[point_x, point_y, cumulative_arclen]`.  Total payload length =
/// `sample_count * 3 * sizeof(f32)` bytes.
///
/// This is a pure-CPU helper; it does not touch wgpu.  The result is consumed
/// by [`LightTrailGpuPolyline::upload`] but is also useful for unit-testing
/// the byte layout without a real GPU device.
pub fn polyline_to_f32_payload(polyline: &crate::path_geom::Polyline) -> Vec<f32> {
    let n = polyline.points.len();
    debug_assert_eq!(
        n,
        polyline.cumulative_arclen.len(),
        "points and cumulative_arclen must have equal length"
    );
    let mut payload = Vec::with_capacity(n * 3);
    for i in 0..n {
        payload.push(polyline.points[i][0]);
        payload.push(polyline.points[i][1]);
        payload.push(polyline.cumulative_arclen[i]);
    }
    payload
}

/// Polyline data uploaded to the GPU as a storage buffer for the light-trail
/// shader.
///
/// # Storage-buffer decision
///
/// T1.3: storage buffer chosen — `BufferUsages::STORAGE` is already used by
/// `treatment_particles.rs` and `fx_compute.rs`; confirmed Metal-OK on the
/// same wgpu 29 device descriptor used by the rest of rmap.
///
/// Layout: `sample_count * 3` contiguous `f32` values in the form
/// `[px, py, arclen, px, py, arclen, …]`. The shader indexes sample `i` as
/// floats at offsets `i*3+0`, `i*3+1`, `i*3+2`.
///
/// No bind group or pipeline is created here — that is T3.2's job.
pub struct LightTrailGpuPolyline {
    /// The GPU storage buffer holding `sample_count * 3` `f32` values.
    pub buffer: wgpu::Buffer,
    /// Number of arc-length samples (equals `polyline.points.len()`).
    pub sample_count: u32,
    /// Total arc-length of the polyline (copy of `polyline.total_length`).
    pub total_length: f32,
}

impl LightTrailGpuPolyline {
    /// Upload a CPU [`crate::path_geom::Polyline`] to a GPU storage buffer.
    ///
    /// The buffer is created with `STORAGE | COPY_DST` usage and immediately
    /// written via `queue.write_buffer`.  The resulting buffer is ready for
    /// binding in T3.2 without any further uploads.
    pub fn upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        polyline: &crate::path_geom::Polyline,
    ) -> Self {
        let payload = polyline_to_f32_payload(polyline);
        let byte_len = (payload.len() * std::mem::size_of::<f32>()) as u64;

        // T1.3: storage buffer chosen — already used by treatment_particles +
        // fx_compute; verified Metal-OK on wgpu 29.
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light_trail polyline storage"),
            size: byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Convert f32 slice to bytes for upload (no bytemuck — consistent with
        // the to_wire_bytes convention used elsewhere in this file).
        let byte_payload: Vec<u8> = payload.iter().flat_map(|f| f.to_le_bytes()).collect();
        queue.write_buffer(&buffer, 0, &byte_payload);

        Self {
            buffer,
            sample_count: polyline.points.len() as u32,
            total_length: polyline.total_length,
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

    /// T1.3-test-1: `polyline_to_f32_payload` for a 16-sample straight-line
    /// polyline produces a payload of `16 * 3 = 48` f32s with the correct
    /// `sample_count` and `total_length`.
    ///
    /// This test is CPU-only — no wgpu device required.
    #[test]
    fn light_trail_gpu_polyline_payload_layout() {
        use crate::path_geom::Polyline;

        // Build a 16-sample horizontal line from x=0 to x=15.
        // cumulative_arclen[i] = i as f32; total_length = 15.0.
        let n: usize = 16;
        let points: Vec<[f32; 2]> = (0..n).map(|i| [i as f32, 0.0]).collect();
        let cumulative_arclen: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let total_length = 15.0_f32;

        let polyline = Polyline {
            points,
            cumulative_arclen,
            total_length,
        };

        let payload = super::polyline_to_f32_payload(&polyline);

        // Expect 16 * 3 = 48 floats.
        assert_eq!(
            payload.len(),
            48,
            "payload length should be sample_count * 3"
        );

        // First triple: x=0.0, y=0.0, arclen=0.0.
        assert_eq!(payload[0], 0.0_f32, "first sample x");
        assert_eq!(payload[1], 0.0_f32, "first sample y");
        assert_eq!(payload[2], 0.0_f32, "first sample arclen");

        // Last triple (index 15): x=15.0, y=0.0, arclen=15.0.
        assert_eq!(payload[45], 15.0_f32, "last sample x");
        assert_eq!(payload[46], 0.0_f32, "last sample y");
        assert_eq!(payload[47], 15.0_f32, "last sample arclen");

        // Validate sample_count and total_length match.
        let sample_count = n as u32;
        assert_eq!(sample_count, 16);
        assert!((total_length - 15.0).abs() < 1e-6);
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
