//! Effects are modeled as an enum (not trait objects) so adding a variant
//! without updating the renderer fails at compile time.

pub mod blur;
pub mod color;
pub mod registry;
pub mod tint;
pub mod transform;

use std::collections::HashMap;

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
    /// PCleanup.1.3 — runs a [`crate::render::treatments::TreatmentPipeline`]
    /// preset (`tone_map`, `luminance_reveal`, `palette_extract`, …) as a
    /// per-layer effect, instead of as a global post-composition pass.
    /// Operators can grade or warp one layer hard while the rest of the
    /// scene stays untouched.
    ///
    /// `id` is the preset ID from `treatments::registry()`. Unknown IDs
    /// warn-and-skip (matching `Effect::External` policy) so a project
    /// authored against a newer build loads on an older binary without
    /// losing the rest of its chain.
    ///
    /// `params` mirrors `Treatment.params` (the global-tier counterpart).
    /// Each preset documents its own keys via `treatments::param_descriptors`.
    ///
    /// SourceModifier semantics — `fluid_warp` and the W2 sibling presets —
    /// land here per the source-modifier-placement decision doc, NOT in
    /// the FX preset registry. See
    /// `specs/004-phase-cleanup-source-modifier-placement-decision.md`.
    Treatment {
        id: String,
        #[serde(default)]
        params: HashMap<String, f32>,
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
    /// PCleanup.1.3 — Treatment pipeline, used by [`Effect::Treatment`].
    /// Same instance the global treatment pass already uses; per-layer
    /// invocation reuses the existing shaders.
    pub treatment_pipeline: &'a crate::render::treatments::TreatmentPipeline,
    /// PCleanup.1.3 — per-layer fit uniform required by every treatment
    /// (16 bytes: `[fit_mode, aspect, focal_x, focal_y]`). Already lives
    /// on `LayerState`; threaded here so per-layer treatments see the
    /// same fit metadata as the global treatment pass does.
    pub fit_uniform: &'a wgpu::Buffer,
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
            Effect::Treatment { id, params } => {
                // PCleanup.1.3 — per-layer treatment dispatch. Reuses the
                // shared TreatmentPipeline (the same instance the global
                // post-composition treatment pass uses).
                //
                // SDF view is NOT plumbed in this initial cut, so the
                // SDF-requiring treatments (`blur_mask`,
                // `displacement_ripple`, `refraction`) gracefully return
                // false from `dispatch` and the ping-pong stays where it
                // was — i.e. the effect is a no-op. The trivial
                // treatments (identity, tone_map, luminance_reveal,
                // palette_extract, texture_overlay, collage) work
                // immediately. SDF plumbing is a follow-up.
                let inputs = crate::render::treatments::TreatmentInputs {
                    source: ctx.source_view,
                    fit_uniform: ctx.fit_uniform,
                    params,
                    clock_secs: clock.elapsed().as_secs_f32(),
                    overlay: None,
                    collage: &[],
                    sdf: None,
                    intermediate: Some(ctx.intermediate_view),
                };
                let rendered = ctx.treatment_pipeline.dispatch(
                    ctx.device,
                    ctx.queue,
                    ctx.encoder,
                    ctx.dst_view,
                    &inputs,
                    id,
                );
                if !rendered {
                    tracing::debug!(
                        id,
                        "Effect::Treatment: dispatch returned false (unknown id, or \
                         missing inputs for an SDF-requiring preset); skipping"
                    );
                }
                rendered
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

    // ----- PCleanup.1.3 — Effect::Treatment(id, params) ------------------

    /// PCleanup.1.3 — the Treatment variant exists, is constructable, and
    /// `effects::mod`'s exhaustive match in `Effect::render` covers it
    /// (compile-time guarantee; this test just exercises construction).
    #[test]
    fn effect_treatment_variant_constructs() {
        let mut params = HashMap::new();
        params.insert("exposure".to_string(), 0.25);
        let e = Effect::Treatment {
            id: "tone_map".to_string(),
            params,
        };
        match e {
            Effect::Treatment { id, params } => {
                assert_eq!(id, "tone_map");
                assert!((params["exposure"] - 0.25).abs() < 1e-6);
            }
            other => panic!("expected Effect::Treatment, got {other:?}"),
        }
    }

    /// PCleanup.1.3 — Treatment round-trips through serde with both id and
    /// params preserved.
    #[test]
    fn effect_treatment_serde_round_trip() {
        let mut params = HashMap::new();
        params.insert("contrast".to_string(), 1.2);
        params.insert("shoulder".to_string(), 0.5);
        let e = Effect::Treatment {
            id: "tone_map".to_string(),
            params,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: Effect = serde_json::from_str(&json).unwrap();
        match back {
            Effect::Treatment { id, params } => {
                assert_eq!(id, "tone_map");
                assert!((params["contrast"] - 1.2).abs() < 1e-6);
                assert!((params["shoulder"] - 0.5).abs() < 1e-6);
            }
            other => panic!("round-trip changed variant: {other:?}"),
        }
    }

    /// PCleanup.1.3 — projects that author Treatment with no `params` field
    /// (relying on `#[serde(default)]`) deserialise as an empty HashMap
    /// rather than failing. Future-proofs against operators or scripts
    /// emitting compact Treatment entries.
    #[test]
    fn effect_treatment_serde_default_empty_params() {
        let legacy = r#"{
            "Treatment": {
                "id": "identity"
            }
        }"#;
        let e: Effect = serde_json::from_str(legacy)
            .expect("Treatment without params must deserialise via serde default");
        match e {
            Effect::Treatment { id, params } => {
                assert_eq!(id, "identity");
                assert!(params.is_empty(), "missing params field → empty map");
            }
            other => panic!("expected Effect::Treatment, got {other:?}"),
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
