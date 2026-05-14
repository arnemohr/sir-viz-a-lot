//! Effects are modeled as an enum (not trait objects) so adding a variant
//! without updating the renderer fails at compile time.

pub mod blur;
pub mod color;
pub mod registry;
pub mod tint;
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
        /// PCleanup.4.1 — three-mode tint. `#[serde(default)]` so projects
        /// serialised before the field existed deserialise as
        /// [`tint::TintMode::Multiply`] (the conventional tint).
        #[serde(default)]
        mode: tint::TintMode,
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
    /// Extension hook (T-M7-07). Looked up at render time in
    /// [`registry::ExternalRegistry`]; missing IDs warn-and-skip so a
    /// project that loads on a binary without the extension still
    /// renders the rest of its chain.
    External {
        id: String,
        params: serde_json::Value,
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
    /// PCleanup.4.1 — Tint pipeline, used by [`Effect::Tint`].
    pub tint: &'a crate::effects::tint::TintPipeline,
    /// Per-layer GPU uniforms (`queue.write_buffer` must target distinct buffers per layer).
    pub color_uniform: &'a wgpu::Buffer,
    pub blur_uniform: &'a wgpu::Buffer,
    pub transform_uniform: &'a wgpu::Buffer,
    /// PCleanup.4.1 — per-layer tint uniform (32 bytes; see `tint::TintParams`).
    pub tint_uniform: &'a wgpu::Buffer,
    /// Extension-pass lookup, consulted by [`Effect::External`] (T-M7-07).
    /// Empty by default; v1 ships no built-in External passes.
    pub external_registry: &'a registry::ExternalRegistry,
}

impl Effect {
    /// Apply this effect: read from `ctx.source_view`, write to
    /// `ctx.dst_view`. Modulator-typed fields are evaluated against
    /// `clock` to produce concrete parameters. Returns `true` when
    /// the destination view was actually written (so the caller can
    /// flip its ping-pong) and `false` for no-op stubs and unregistered
    /// `External` ids.
    ///
    /// `External { id }` is the extension hook (T-M7-07): looked up
    /// in `ctx.external_registry`; missing ids warn-and-skip.
    pub fn render(&self, ctx: &mut RenderCtx<'_>, clock: &crate::clock::Clock) -> bool {
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
                true
            }
            Effect::Tint { rgba, amount, mode } => {
                let params = crate::effects::tint::TintParams {
                    rgba: *rgba,
                    amount: amount.value(clock),
                    mode: *mode,
                };
                ctx.tint.render(
                    ctx.device,
                    ctx.queue,
                    ctx.encoder,
                    ctx.source_view,
                    ctx.dst_view,
                    ctx.tint_uniform,
                    params,
                );
                true
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
                true
            }
            Effect::Transform {
                translate,
                rotate_deg,
                scale_x,
                scale_y,
            } => {
                // Schema convention: translate is in normalized output-space
                // [-1, 1] (±1 = a full screen width / height of shift), with
                // y-down to match the egui preview axes. The GPU shader works
                // in NDC ([-1, 1] over half the screen, y-up), so map
                // schema → NDC: multiply by 2 and flip y.
                let params = crate::effects::transform::TransformParams {
                    translate: glam::Vec2::new(translate[0] * 2.0, translate[1] * -2.0),
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
                true
            }
            Effect::External { id, params } => {
                // Look up the pass via the registry reference held in ctx.
                // Pulling it into a local lets the borrow-checker treat the
                // remaining ctx field reborrows (encoder, views) as disjoint.
                let pass = ctx.external_registry.get(id);
                match pass {
                    Some(pass) => {
                        pass.render(
                            ctx.device,
                            ctx.queue,
                            ctx.encoder,
                            ctx.source_view,
                            ctx.dst_view,
                            params,
                            clock,
                        );
                        pass.writes_destination(params)
                    }
                    None => {
                        tracing::warn!(
                            id,
                            "Effect::External: no pass registered under this id; skipping",
                        );
                        false
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// PCleanup.4.1 — old projects serialised before the `mode` field
    /// existed must still deserialise cleanly, with `mode` defaulting to
    /// [`tint::TintMode::Multiply`]. Catches accidental removal of the
    /// `#[serde(default)]` attribute on the variant field.
    #[test]
    fn effect_tint_legacy_json_back_compat() {
        // Legacy JSON: no `mode` field.
        let legacy = r#"{
            "Tint": {
                "rgba": [1.0, 0.5, 0.25, 1.0],
                "amount": {"Static": 0.8}
            }
        }"#;
        let parsed: Effect =
            serde_json::from_str(legacy).expect("legacy Tint JSON must deserialise");
        match parsed {
            Effect::Tint { rgba, mode, .. } => {
                assert_eq!(rgba, [1.0, 0.5, 0.25, 1.0]);
                assert_eq!(mode, tint::TintMode::Multiply, "missing mode → Multiply");
            }
            other => panic!("expected Effect::Tint, got {other:?}"),
        }
    }

    /// PCleanup.4.1 — newer JSON with an explicit `mode` round-trips.
    #[test]
    fn effect_tint_new_json_round_trip() {
        for m in [
            tint::TintMode::Multiply,
            tint::TintMode::Additive,
            tint::TintMode::Screen,
        ] {
            let e = Effect::Tint {
                rgba: [0.2, 0.4, 0.6, 1.0],
                amount: Modulator::Static(0.5),
                mode: m,
            };
            let json = serde_json::to_string(&e).unwrap();
            let back: Effect = serde_json::from_str(&json).unwrap();
            match back {
                Effect::Tint { mode, .. } => assert_eq!(mode, m, "mode lost for {m:?}"),
                other => panic!("round-trip changed variant: {other:?}"),
            }
        }
    }
}
