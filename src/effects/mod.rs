//! Effects are modeled as an enum (not trait objects) so adding a variant
//! without updating the renderer fails at compile time.

pub mod blur;
pub mod color;
pub mod feedback;
pub mod registry;
pub mod tint;
pub mod transform;

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::modulators::Modulator;

/// 004-T1.1 — A single node in the per-layer Look chain.
///
/// Wraps one `Effect` with an `enabled` flag so operators can bypass a node
/// without deleting it. The `#[serde(default = "default_enabled_true")]`
/// attribute is **load-bearing**: plain `#[serde(default)]` on a bool evaluates
/// to `false`, which would silently bypass every effect in any pre-v12 save and
/// every `assets/presets/*.json`. The named helper is the single highest-impact
/// line of code in this module — do not change it to `#[serde(default)]`.
#[allow(dead_code)] // consumed by Wave 2 foundation cluster (T1.3 schema change)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectNode {
    #[serde(default = "default_enabled_true")]
    pub enabled: bool,
    pub effect: Effect,
}

/// Named default-fn for `EffectNode::enabled`. Must be a named function; see
/// the `EffectNode` doc comment for why `#[serde(default)]` alone is unsafe.
fn default_enabled_true() -> bool {
    true
}

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
        /// 004-T1.2 — optional overlay texture path for overlay-style
        /// presets (`texture_overlay`). `None` for presets that don't use
        /// an overlay. `#[serde(default)]` keeps pre-T1.2 saves loading.
        #[serde(default)]
        overlay_path: Option<PathBuf>,
        /// 004-T1.2 — collage slot paths for `collage`-style presets.
        /// Empty for presets that don't use collage. `#[serde(default)]`
        /// keeps pre-T1.2 saves loading.
        #[serde(default)]
        collage_paths: Vec<PathBuf>,
    },
    /// PCleanup.1.4 — feedback / trails / motion smear. Blends the
    /// current-frame source with the previous frame's output of this
    /// effect:
    ///   * `decay = 0.0` → no trail (pure source pass-through).
    ///   * `decay = 0.95` → long trail; the current pixel inherits 95%
    ///     of the previous frame's pixel at the offset location.
    ///   * `decay = 1.0` → infinite hold (history sample only).
    ///
    /// `offset` shifts the history sample (UV-space), so a non-zero
    /// offset produces directional motion-trail behind the layer.
    ///
    /// History is kept in a per-layer texture; multiple Feedback effects
    /// stacked on one layer share that history (a deliberate scope
    /// decision — the per-effect variant would multiply allocation by
    /// chain length).
    Feedback {
        decay: Modulator,
        #[serde(default)]
        offset: [f32; 2],
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
    /// PCleanup.1.4 — Feedback / trails pipeline, used by
    /// [`Effect::Feedback`].
    pub feedback: &'a crate::effects::feedback::FeedbackPipeline,
    /// PCleanup.1.4 — per-layer feedback uniform (16 bytes; see
    /// `feedback::FeedbackParams`).
    pub feedback_uniform: &'a wgpu::Buffer,
    /// PCleanup.1.4 — per-layer history texture view. Holds the previous
    /// frame's output of the Feedback effect. The pass updates it
    /// in-place at the end of `Effect::Feedback::render`.
    pub history_view: &'a wgpu::TextureView,
    // ---- 004-T1.5: six new fields for full Treatment plumbing -----------
    // These fields are populated with null defaults at the call site in
    // app.rs until T1.8 wires the real values. The `#[allow(dead_code)]`
    // annotations suppress warnings while the fields await T1.7/T1.8.
    /// 004-T1.5 — per-layer SDF texture view (R32Float). Populated after
    /// `WarpRenderer::sync_from_layer`; used by SDF-keyed treatments
    /// (`ripple_lens`, `blur_mask`, `displacement_ripple`, etc.). `None`
    /// until T1.8 wires it at the call site in `app.rs`.
    #[allow(dead_code)] // consumed by Effect::Treatment arm in T1.7
    pub sdf_view: Option<&'a wgpu::TextureView>,
    /// 004-T1.5 — semantic zone role of this layer's mask polygon (from
    /// `cfg.warp.zone_role`). Used by `zone_brighten` / `zone_lens`.
    /// `None` until T1.8 wires it.
    #[allow(dead_code)] // consumed by Effect::Treatment arm in T1.7
    pub zone_role: Option<crate::project::schema::ZoneRole>,
    /// 004-T1.5 — stable per-layer RNG seed (e.g. `LayerState::layer_id.0`).
    /// Used by particle-based treatments (`spotlights`, `drift_pinholes`, …).
    /// `0` until T1.8 wires it.
    #[allow(dead_code)] // consumed by Effect::Treatment arm in T1.7
    pub seed: u64,
    /// 004-T1.5 — seconds (project clock) at which this layer was added.
    /// Used with `clock_secs` to compute per-layer local time for particle
    /// animation. `0.0` until T1.8 wires it.
    #[allow(dead_code)] // consumed by Effect::Treatment arm in T1.7
    pub t_layer_added_secs: f32,
    /// 004-T1.5 — optional overlay texture view for overlay-style presets
    /// (`texture_overlay`). Loaded from `Effect::Treatment.overlay_path`.
    /// `None` until T1.8 hoists the overlay-loader into the per-node loop.
    #[allow(dead_code)] // consumed by Effect::Treatment arm in T1.7
    pub overlay_view: Option<&'a wgpu::TextureView>,
    /// 004-T1.5 — collage slot texture views for `collage`-style presets.
    /// Loaded from `Effect::Treatment.collage_paths`. Empty slice until
    /// T1.8 hoists the collage-loader into the per-node loop.
    #[allow(dead_code)] // consumed by Effect::Treatment arm in T1.7
    pub collage_views: &'a [&'a wgpu::TextureView],
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
            Effect::Feedback { decay, offset } => {
                // PCleanup.1.4 — per-layer feedback / trails. Two passes:
                //   (a) mix(source, history(uv - offset), decay) → dst
                //   (b) blit dst → history (for next frame)
                // The per-layer history texture is allocated in
                // `LayerState`; FeedbackPipeline::render handles both
                // passes internally so the dispatch site stays simple.
                let params = crate::effects::feedback::FeedbackParams {
                    decay: decay.value(clock),
                    offset: *offset,
                };
                ctx.feedback.render(
                    ctx.device,
                    ctx.queue,
                    ctx.encoder,
                    ctx.source_view,
                    ctx.history_view,
                    ctx.dst_view,
                    ctx.feedback_uniform,
                    params,
                );
                true
            }
            Effect::Treatment { id, params, .. } => {
                // 004-T1.7 — per-layer treatment dispatch. Reuses the shared
                // TreatmentPipeline (the same instance the global
                // post-composition treatment pass uses).
                //
                // All RenderCtx fields are now threaded through: SDF view,
                // zone role, seed, t_layer_added_secs, overlay, and collage
                // views. The caller (app.rs, T1.8) is responsible for loading
                // the texture views from overlay_path / collage_paths and
                // populating RenderCtx before invoking render().
                let inputs = crate::render::treatments::TreatmentInputs {
                    source: ctx.source_view,
                    fit_uniform: ctx.fit_uniform,
                    params,
                    clock_secs: clock.elapsed().as_secs_f32(),
                    overlay: ctx.overlay_view,
                    collage: ctx.collage_views,
                    sdf: ctx.sdf_view,
                    intermediate: Some(ctx.intermediate_view),
                    zone_role: ctx.zone_role,
                    seed: ctx.seed,
                    t_layer_added_secs: ctx.t_layer_added_secs,
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
                        sdf = ctx.sdf_view.is_some(),
                        overlay = ctx.overlay_view.is_some(),
                        collage_len = ctx.collage_views.len(),
                        "Effect::Treatment: dispatch returned false (unknown id, or \
                         ctx is missing a required input for this preset); skipping"
                    );
                }
                rendered
            }
        }
    }
}

// ============================================================
// 004-T1.23 — Intent groups
// ============================================================

/// 004-T1.23 — Semantic intent category for an `Effect` node. Drives the
/// intent-grouped Add picker in the Look chain UI and the per-row glyph color.
#[allow(dead_code)] // consumed by look_chain UI (Phase 1 T1.25-T1.27)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentGroup {
    /// Warps or spatially displaces the source (fluid_warp, ripple_lens, …).
    Warp,
    /// Color grading / tone shaping (tone_map, luminance_reveal, …).
    Color,
    /// Texture compositing / blur-based masking (texture_overlay, blur_mask).
    Texture,
    /// Composition helpers and passthrough (collage, identity).
    Compose,
    /// Particle-driven animation over the source (spotlights, drift_pinholes, …).
    Animate,
    /// Fully generative / external (Effect::External).
    Generative,
}

/// 004-T1.23 — Maps an `Effect` variant to its `IntentGroup`.
///
/// For `Effect::Treatment`, delegates to
/// `crate::render::treatments::intent_group_for_preset`.
#[allow(dead_code)] // consumed by look_chain UI (Phase 1 T1.25-T1.27)
pub fn intent_group(effect: &Effect) -> IntentGroup {
    match effect {
        Effect::Color { .. } | Effect::Tint { .. } => IntentGroup::Color,
        Effect::Blur { .. } => IntentGroup::Texture,
        Effect::Transform { .. } => IntentGroup::Warp,
        Effect::Feedback { .. } => IntentGroup::Animate,
        Effect::External { .. } => IntentGroup::Generative,
        Effect::Treatment { id, .. } => crate::render::treatments::intent_group_for_preset(id),
    }
}

// ============================================================
// 004-T1.22 — No-op detection
// ============================================================

/// 004-T1.22 — Returns `Some(reason)` when the node is an identity
/// no-op (e.g. Blur radius = 0), `None` when the effect actively
/// modifies pixels.
///
/// `layer` is required so the `Effect::Treatment` branch can delegate to
/// `treatments::treatment_is_no_op`, which checks SDF/zone/overlay
/// prerequisites from `LayerConfig`.
///
/// Note: disabled nodes (`!node.enabled`) are not "no-ops" in this sense —
/// they are bypassed at the render-loop level, not here. Callers that want
/// to show a "bypassed" status dot should check `node.enabled` separately.
#[allow(dead_code)] // consumed by look_chain UI (Phase 1 T1.25-T1.28)
pub fn effect_is_no_op(
    node: &EffectNode,
    layer: &crate::project::schema::LayerConfig,
) -> Option<&'static str> {
    match &node.effect {
        Effect::Color {
            hue,
            saturation,
            brightness,
            contrast,
        } => {
            let identity = matches!(hue, Modulator::Static(v) if v.abs() < 1e-4)
                && matches!(saturation, Modulator::Static(v) if (v - 1.0).abs() < 1e-4)
                && matches!(brightness, Modulator::Static(v) if v.abs() < 1e-4)
                && matches!(contrast, Modulator::Static(v) if (v - 1.0).abs() < 1e-4);
            if identity {
                Some("Color effect at identity")
            } else {
                None
            }
        }
        Effect::Tint { amount, .. } => {
            if matches!(amount, Modulator::Static(v) if v.abs() < 1e-4) {
                Some("Tint amount at 0")
            } else {
                None
            }
        }
        Effect::Blur { radius_px } => {
            if matches!(radius_px, Modulator::Static(v) if v.abs() < 1e-4) {
                Some("Blur radius at 0")
            } else {
                None
            }
        }
        Effect::Transform {
            translate,
            rotate_deg,
            scale_x,
            scale_y,
        } => {
            let identity = translate[0].abs() < 1e-4
                && translate[1].abs() < 1e-4
                && matches!(rotate_deg, Modulator::Static(v) if v.abs() < 1e-4)
                && matches!(scale_x, Modulator::Static(v) if (v - 1.0).abs() < 1e-4)
                && matches!(scale_y, Modulator::Static(v) if (v - 1.0).abs() < 1e-4);
            if identity {
                Some("Transform at identity")
            } else {
                None
            }
        }
        Effect::Feedback { decay, .. } => {
            if matches!(decay, Modulator::Static(v) if v.abs() < 1e-4) {
                Some("Feedback decay at 0")
            } else {
                None
            }
        }
        Effect::External { .. } => None,
        Effect::Treatment { id, params, .. } => {
            crate::render::treatments::treatment_is_no_op(id, params, layer)
        }
    }
}

/// Default chain: Color → Blur → Transform (all static / identity).
///
/// 004-T1.3 — returns `Vec<EffectNode>` so callers assign to
/// `LayerConfig.effects: Vec<EffectNode>` without wrapping at every site.
pub fn default_effect_chain() -> Vec<EffectNode> {
    vec![
        EffectNode {
            enabled: true,
            effect: Effect::Color {
                hue: Modulator::Static(0.0),
                saturation: Modulator::Static(1.0),
                brightness: Modulator::Static(0.0),
                contrast: Modulator::Static(1.0),
            },
        },
        EffectNode {
            enabled: true,
            effect: Effect::Blur {
                radius_px: Modulator::Static(0.0),
            },
        },
        EffectNode {
            enabled: true,
            effect: Effect::Transform {
                translate: [0.0, 0.0],
                rotate_deg: Modulator::Static(0.0),
                scale_x: Modulator::Static(1.0),
                scale_y: Modulator::Static(1.0),
            },
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
            overlay_path: None,
            collage_paths: vec![],
        };
        match e {
            Effect::Treatment { id, params, .. } => {
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
            overlay_path: None,
            collage_paths: vec![],
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: Effect = serde_json::from_str(&json).unwrap();
        match back {
            Effect::Treatment { id, params, .. } => {
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
            Effect::Treatment { id, params, .. } => {
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

    // ----- 004-T1.1 — EffectNode default_enabled_true -------------------

    /// 004-T1.1 — Deserializing `{"effect": {"Color": {…}}}` (no `enabled`
    /// field) must produce `enabled: true`. Catches accidental removal of the
    /// `default_enabled_true` helper or accidental change to `#[serde(default)]`
    /// on the `enabled` field, which would return `false` and silently bypass
    /// every effect in pre-v12 saves.
    #[test]
    fn effect_node_missing_enabled_defaults_to_true() {
        let json = r#"{
            "effect": {
                "Color": {
                    "hue": {"Static": 0.0},
                    "saturation": {"Static": 1.0},
                    "brightness": {"Static": 0.0},
                    "contrast": {"Static": 1.0}
                }
            }
        }"#;
        let node: EffectNode =
            serde_json::from_str(json).expect("EffectNode without enabled must deserialise");
        assert!(
            node.enabled,
            "missing enabled field must default to true (not false)"
        );
    }

    // ----- 004-T1.2 — Effect::Treatment overlay_path / collage_paths -----

    /// 004-T1.2 — Treatment with overlay_path Some and 2 collage_paths
    /// round-trips through serde with both new fields preserved.
    #[test]
    fn effect_treatment_overlay_and_collage_round_trip() {
        use std::path::PathBuf;
        let e = Effect::Treatment {
            id: "texture_overlay".to_string(),
            params: HashMap::new(),
            overlay_path: Some(PathBuf::from("/assets/textures/grunge.png")),
            collage_paths: vec![
                PathBuf::from("/assets/a.jpg"),
                PathBuf::from("/assets/b.jpg"),
            ],
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: Effect = serde_json::from_str(&json).unwrap();
        match back {
            Effect::Treatment {
                id,
                overlay_path,
                collage_paths,
                ..
            } => {
                assert_eq!(id, "texture_overlay");
                assert_eq!(
                    overlay_path,
                    Some(PathBuf::from("/assets/textures/grunge.png"))
                );
                assert_eq!(collage_paths.len(), 2);
                assert_eq!(collage_paths[0], PathBuf::from("/assets/a.jpg"));
                assert_eq!(collage_paths[1], PathBuf::from("/assets/b.jpg"));
            }
            other => panic!("round-trip changed variant: {other:?}"),
        }
    }

    // ----- 004-T1.22 — effect_is_no_op -----------------------------------

    /// 004-T1.22 — Color effect at identity (hue=0, sat=1, bright=0,
    /// contrast=1) returns Some(reason).
    #[test]
    fn effect_is_no_op_color_identity() {
        let layer = dummy_layer();
        let node = EffectNode {
            enabled: true,
            effect: Effect::Color {
                hue: Modulator::Static(0.0),
                saturation: Modulator::Static(1.0),
                brightness: Modulator::Static(0.0),
                contrast: Modulator::Static(1.0),
            },
        };
        assert!(
            effect_is_no_op(&node, &layer).is_some(),
            "identity Color must be detected as no-op"
        );
        // Non-identity: hue shifted.
        let non_id = EffectNode {
            enabled: true,
            effect: Effect::Color {
                hue: Modulator::Static(45.0),
                saturation: Modulator::Static(1.0),
                brightness: Modulator::Static(0.0),
                contrast: Modulator::Static(1.0),
            },
        };
        assert!(
            effect_is_no_op(&non_id, &layer).is_none(),
            "non-identity Color must not be detected as no-op"
        );
    }

    /// 004-T1.22 — Tint at amount=0 is a no-op; amount>0 is not.
    #[test]
    fn effect_is_no_op_tint_amount_zero() {
        let layer = dummy_layer();
        let node = EffectNode {
            enabled: true,
            effect: Effect::Tint {
                rgba: [1.0, 0.0, 0.0, 1.0],
                amount: Modulator::Static(0.0),
                mode: tint::TintMode::Multiply,
            },
        };
        assert!(
            effect_is_no_op(&node, &layer).is_some(),
            "Tint amount=0 must be no-op"
        );
        let active = EffectNode {
            enabled: true,
            effect: Effect::Tint {
                rgba: [1.0, 0.0, 0.0, 1.0],
                amount: Modulator::Static(0.5),
                mode: tint::TintMode::Multiply,
            },
        };
        assert!(
            effect_is_no_op(&active, &layer).is_none(),
            "Tint amount=0.5 must not be no-op"
        );
    }

    /// 004-T1.22 — Blur at radius_px=0 is a no-op; radius_px>0 is not.
    #[test]
    fn effect_is_no_op_blur_radius_zero() {
        let layer = dummy_layer();
        let node = EffectNode {
            enabled: true,
            effect: Effect::Blur {
                radius_px: Modulator::Static(0.0),
            },
        };
        assert!(
            effect_is_no_op(&node, &layer).is_some(),
            "Blur radius=0 must be no-op"
        );
        let active = EffectNode {
            enabled: true,
            effect: Effect::Blur {
                radius_px: Modulator::Static(5.0),
            },
        };
        assert!(
            effect_is_no_op(&active, &layer).is_none(),
            "Blur radius=5 must not be no-op"
        );
    }

    /// 004-T1.22 — Transform at identity (translate=[0,0], rotate=0,
    /// scale=[1,1]) is a no-op; any deviation is not.
    #[test]
    fn effect_is_no_op_transform_identity() {
        let layer = dummy_layer();
        let node = EffectNode {
            enabled: true,
            effect: Effect::Transform {
                translate: [0.0, 0.0],
                rotate_deg: Modulator::Static(0.0),
                scale_x: Modulator::Static(1.0),
                scale_y: Modulator::Static(1.0),
            },
        };
        assert!(
            effect_is_no_op(&node, &layer).is_some(),
            "identity Transform must be no-op"
        );
        let active = EffectNode {
            enabled: true,
            effect: Effect::Transform {
                translate: [0.1, 0.0],
                rotate_deg: Modulator::Static(0.0),
                scale_x: Modulator::Static(1.0),
                scale_y: Modulator::Static(1.0),
            },
        };
        assert!(
            effect_is_no_op(&active, &layer).is_none(),
            "translated Transform must not be no-op"
        );
    }

    /// 004-T1.22 — Feedback at decay=0 is a no-op; decay>0 is not.
    #[test]
    fn effect_is_no_op_feedback_decay_zero() {
        let layer = dummy_layer();
        let node = EffectNode {
            enabled: true,
            effect: Effect::Feedback {
                decay: Modulator::Static(0.0),
                offset: [0.0, 0.0],
            },
        };
        assert!(
            effect_is_no_op(&node, &layer).is_some(),
            "Feedback decay=0 must be no-op"
        );
        let active = EffectNode {
            enabled: true,
            effect: Effect::Feedback {
                decay: Modulator::Static(0.9),
                offset: [0.0, 0.0],
            },
        };
        assert!(
            effect_is_no_op(&active, &layer).is_none(),
            "Feedback decay=0.9 must not be no-op"
        );
    }

    // ----- 004-T1.23 — intent_group mapping ------------------------------

    /// 004-T1.23 — Every Effect variant and every registered treatment
    /// preset_id maps to a well-defined IntentGroup. This test ensures no
    /// variant is accidentally unmapped (i.e., falls through to a panic).
    #[test]
    fn intent_group_covers_all_variants_and_presets() {
        use crate::render::treatments::registry;

        // Check the raw Effect variants.
        let color = Effect::Color {
            hue: Modulator::Static(0.0),
            saturation: Modulator::Static(1.0),
            brightness: Modulator::Static(0.0),
            contrast: Modulator::Static(1.0),
        };
        assert_eq!(
            intent_group(&color),
            IntentGroup::Color,
            "Color → Color"
        );

        let tint = Effect::Tint {
            rgba: [1.0, 0.0, 0.0, 1.0],
            amount: Modulator::Static(0.5),
            mode: tint::TintMode::Multiply,
        };
        assert_eq!(intent_group(&tint), IntentGroup::Color, "Tint → Color");

        let blur = Effect::Blur {
            radius_px: Modulator::Static(5.0),
        };
        assert_eq!(intent_group(&blur), IntentGroup::Texture, "Blur → Texture");

        let transform = Effect::Transform {
            translate: [0.0, 0.0],
            rotate_deg: Modulator::Static(0.0),
            scale_x: Modulator::Static(1.0),
            scale_y: Modulator::Static(1.0),
        };
        assert_eq!(
            intent_group(&transform),
            IntentGroup::Warp,
            "Transform → Warp"
        );

        let feedback = Effect::Feedback {
            decay: Modulator::Static(0.9),
            offset: [0.0, 0.0],
        };
        assert_eq!(
            intent_group(&feedback),
            IntentGroup::Animate,
            "Feedback → Animate"
        );

        let external = Effect::External {
            id: "some_ext".to_string(),
            params: serde_json::Value::Null,
        };
        assert_eq!(
            intent_group(&external),
            IntentGroup::Generative,
            "External → Generative"
        );

        // Check every registered treatment preset_id returns a valid group.
        for (preset_id, _label) in registry() {
            let treatment = Effect::Treatment {
                id: preset_id.to_string(),
                params: HashMap::new(),
                overlay_path: None,
                collage_paths: vec![],
            };
            // Just calling intent_group must not panic; all registered IDs
            // must map to a concrete group (the function is total).
            let group = intent_group(&treatment);
            // Spot-check a few expected mappings.
            match *preset_id {
                "identity" => assert_eq!(group, IntentGroup::Compose, "identity → Compose"),
                "tone_map" => assert_eq!(group, IntentGroup::Color, "tone_map → Color"),
                "blur_mask" => assert_eq!(group, IntentGroup::Texture, "blur_mask → Texture"),
                "fluid_warp" => assert_eq!(group, IntentGroup::Warp, "fluid_warp → Warp"),
                "spotlights" => {
                    assert_eq!(group, IntentGroup::Animate, "spotlights → Animate")
                }
                _ => {} // Other IDs just need to not panic.
            }
        }
    }

    // ----- 004-T1.7 — ctx field wiring regression guard --------------------

    /// 004-T1.7 — Verify that the `Effect::Treatment` dispatch arm threads all
    /// six `RenderCtx` fields through to `TreatmentInputs` instead of using
    /// hardcoded nulls. This is a source-text check: the compile-time guarantee
    /// that the fields are present is the real proof; a dispatch-returning-true
    /// test requires a wgpu device and lives in T1.34 (golden image smoke).
    #[test]
    fn effect_treatment_arm_reads_ctx_sdf_view() {
        let src = include_str!("mod.rs");
        assert!(
            src.contains("sdf: ctx.sdf_view"),
            "Effect::Treatment must thread ctx.sdf_view"
        );
        assert!(
            src.contains("zone_role: ctx.zone_role"),
            "Effect::Treatment must thread ctx.zone_role"
        );
        assert!(
            src.contains("seed: ctx.seed"),
            "Effect::Treatment must thread ctx.seed"
        );
        assert!(
            src.contains("t_layer_added_secs: ctx.t_layer_added_secs"),
            "Effect::Treatment must thread ctx.t_layer_added_secs"
        );
        assert!(
            src.contains("overlay: ctx.overlay_view"),
            "Effect::Treatment must thread ctx.overlay_view"
        );
        assert!(
            src.contains("collage: ctx.collage_views"),
            "Effect::Treatment must thread ctx.collage_views"
        );
    }

    // ----- Shared test fixture -------------------------------------------

    /// Minimal [`crate::project::schema::LayerConfig`] for unit tests that
    /// need a `layer` argument but don't exercise schema fields.
    fn dummy_layer() -> crate::project::schema::LayerConfig {
        use crate::project::schema::{LayerConfig, LayerKind};
        LayerConfig {
            id: "test-layer".to_string(),
            kind: LayerKind::Svg {
                svg_path: std::path::PathBuf::from("test.svg"),
            },
            enabled: true,
            transform: Default::default(),
            effects: vec![],
            blend_mode: Default::default(),
            opacity: 1.0,
            warp: crate::project::schema::WarpMesh::identity(),
            muted: false,
            bezier_mesh: None,
            mask_graph: None,
        }
    }
}
