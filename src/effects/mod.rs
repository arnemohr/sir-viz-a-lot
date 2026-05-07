//! Effects are modeled as an enum (not trait objects) so adding a variant
//! without updating the renderer fails at compile time.

pub mod blur;
pub mod color;
pub mod transform;

use serde::{Deserialize, Serialize};

use crate::modulators::Modulator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Effect {
    Color {
        hue: Modulator,
        saturation: Modulator,
        brightness: Modulator,
        contrast: Modulator,
    },
    Tint {
        rgba: [f32; 4],
        amount: Modulator,
    },
    Blur {
        radius_px: Modulator,
    },
    Transform {
        translate: [f32; 2],
        rotate_deg: Modulator,
        scale_x: Modulator,
        scale_y: Modulator,
    },
}

/// Bundle of references needed to dispatch a single effect pass.
///
/// The caller constructs pipelines once at startup and allocates
/// ping-pong textures (including a scratch `intermediate_view` for
/// multi-pass effects like Blur) at layer-resize time.
pub struct RenderCtx<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    /// Source view for this effect pass.
    pub source_view: &'a wgpu::TextureView,
    /// Destination view for this effect pass.
    pub dst_view: &'a wgpu::TextureView,
    /// Scratch view used by multi-pass effects (currently only Blur).
    /// Caller is responsible for allocating it as a third ping-pong-
    /// like texture at the same dimensions/format as source and dst.
    pub intermediate_view: &'a wgpu::TextureView,
    /// Cached pipelines. Caller constructs these once at startup.
    pub color: &'a crate::effects::color::ColorPipeline,
    pub blur: &'a crate::effects::blur::BlurPipeline,
    pub transform: &'a crate::effects::transform::TransformPipeline,
    /// Per-layer GPU uniforms (`queue.write_buffer` must target distinct buffers per layer).
    pub color_uniform: &'a wgpu::Buffer,
    pub blur_uniform: &'a wgpu::Buffer,
    pub transform_uniform: &'a wgpu::Buffer,
}

impl Effect {
    /// Returns false when [`Effect::render`] does not record draws into
    /// `ctx.dst_view` (Tint stub). Callers must **not** flip the effect
    /// ping-pong after those variants or the chain reads the wrong texture.
    pub fn writes_destination(&self) -> bool {
        !matches!(self, Effect::Tint { .. })
    }

    /// Apply this effect: read from `ctx.source_view`, write to
    /// `ctx.dst_view`. Modulator-typed fields are evaluated against
    /// `clock` to produce concrete parameters.
    ///
    /// `Tint` is currently a no-op stub — the variant exists in the
    /// enum but no TintPipeline has been built. Logged at warn! once
    /// per call. T-M4-?? (or M5) will land the tint pipeline.
    pub fn render(&self, ctx: &mut RenderCtx<'_>, clock: &crate::clock::Clock) {
        match self {
            Effect::Color {
                hue,
                saturation,
                brightness,
                contrast,
            } => {
                let params = crate::effects::color::ColorParams {
                    hue_shift_deg: hue.value(clock),
                    saturation_mul: saturation.value(clock),
                    brightness_add: brightness.value(clock),
                    contrast_mul: contrast.value(clock),
                };
                ctx.color.render(
                    ctx.device,
                    ctx.queue,
                    ctx.encoder,
                    ctx.source_view,
                    ctx.dst_view,
                    ctx.color_uniform,
                    params,
                );
            }
            Effect::Tint { rgba: _, amount: _ } => {
                tracing::warn!(
                    "Effect::Tint is not yet implemented (no TintPipeline built); skipping"
                );
                // No-op: dst_view is left as whatever it was. Caller
                // (effect chain) should treat this as "this effect did
                // nothing"; the source_view passes through unchanged
                // logically because we're not blitting. A future
                // TintPipeline will fill this in.
            }
            Effect::Blur { radius_px } => {
                let params = crate::effects::blur::BlurParams {
                    radius_px: radius_px.value(clock),
                };
                ctx.blur.apply(
                    ctx.device,
                    ctx.queue,
                    ctx.encoder,
                    ctx.source_view,
                    ctx.intermediate_view,
                    ctx.dst_view,
                    ctx.blur_uniform,
                    params,
                );
            }
            Effect::Transform {
                translate,
                rotate_deg,
                scale_x,
                scale_y,
            } => {
                let params = crate::effects::transform::TransformParams {
                    translate: glam::Vec2::new(translate[0], translate[1]),
                    rotate: rotate_deg.value(clock).to_radians(),
                    scale: glam::Vec2::new(scale_x.value(clock), scale_y.value(clock)),
                    anchor: glam::Vec2::ZERO,
                };
                ctx.transform.render(
                    ctx.device,
                    ctx.queue,
                    ctx.encoder,
                    ctx.source_view,
                    ctx.dst_view,
                    ctx.transform_uniform,
                    params,
                );
            }
        }
    }
}

/// Default chain: Color → Blur → Transform (all static / identity).
pub fn default_effect_chain() -> Vec<Effect> {
    vec![
        Effect::Color {
            hue: Modulator::Static(0.0),
            saturation: Modulator::Static(1.0),
            brightness: Modulator::Static(0.0),
            contrast: Modulator::Static(1.0),
        },
        Effect::Blur {
            radius_px: Modulator::Static(0.0),
        },
        Effect::Transform {
            translate: [0.0, 0.0],
            rotate_deg: Modulator::Static(0.0),
            scale_x: Modulator::Static(1.0),
            scale_y: Modulator::Static(1.0),
        },
    ]
}
