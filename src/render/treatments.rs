//! P1.2.2 — `TreatmentPipeline` render integration.
//!
//! A *treatment* runs **before** the existing effect chain: per-frame layer
//! order is `raster source → treatment → effects → warp → compositor`. The
//! treatment stage writes into the effect chain's first ping-pong texture,
//! replacing the [`SvgLayerPipeline`](crate::svg_layer::render::SvgLayerPipeline)
//! blit on layers that carry a [`Treatment`](crate::project::schema::Treatment).
//!
//! This module scaffolds the dispatch + registry. v0.4 / Phase 1 W2 ships
//! exactly one preset — `"identity"` — which proves the bind-group contract
//! by performing a no-op blit through a separate pipeline (not the
//! `svg_pipeline` shared with the default path). W3 grows the registry into
//! the real preset library (`tone_map`, `blur_mask`, `luminance_reveal`,
//! `texture_overlay`, `palette_extract`, `collage`).
//!
//! # Default-path invariant
//!
//! When `layer.treatment.is_none()` *or* the treatment's `preset_id` is not
//! registered, this module is never reached — the caller falls back to the
//! pre-P1.2.2 `svg_pipeline.render` blit. The default path therefore stays
//! bit-exact identical, which is the core P1.2.2 acceptance criterion.
//!
//! # Unknown-preset behaviour
//!
//! `dispatch` returns `false` for an unregistered `preset_id`. The caller in
//! `app.rs` interprets that as "skip the treatment but still render the
//! source image" — i.e. an Image / Video layer with a bad preset_id renders
//! as if `treatment: None`, which is the user-forgiving choice. The audit
//! (W1) already emits a `Warn` for the empty / mistyped case so the
//! operator sees a UI hint; the renderer chooses not to make the layer
//! invisible. This deliberately diverges from the `FxLayer` unknown-preset
//! path (which hides the layer entirely) because an Image layer's *content*
//! is meaningful on its own.

use std::collections::HashMap;

/// Identity preset: a no-op blit that proves the dispatch contract.
/// Renders the source texture into the destination using the same
/// `textured_quad.wgsl` shader the default `svg_pipeline` path uses, but
/// through a separate pipeline owned by this module.
pub const IDENTITY_PRESET_ID: &str = "identity";

/// `tone_map` preset id (P1.3.1).
pub const TONE_MAP_PRESET_ID: &str = "tone_map";

/// Inputs threaded into every preset's `render` call. The struct grows over
/// time (W3 adds `overlay` and `collage`); existing presets ignore fields
/// they don't read.
pub struct TreatmentInputs<'a> {
    /// Source texture view (post-raster, pre-effect). For Image layers this
    /// is the uploaded RGBA texture; for Video, the per-frame upload from
    /// the AVFoundation worker; for SVG, the resvg pixmap upload.
    pub source: &'a wgpu::TextureView,

    /// Per-layer fit-mode uniform (16 bytes: `[fit_mode, aspect_layer,
    /// focal_x, focal_y]`). Caller has already written the current frame's
    /// values via `queue.write_buffer` before calling `dispatch`.
    pub fit_uniform: &'a wgpu::Buffer,

    /// Free-form per-preset params from `Treatment.params`. Each preset
    /// documents which keys it reads via [`param_descriptors`]; missing
    /// keys fall back to the descriptor's documented default.
    #[allow(dead_code)] // identity ignores params; W3 presets will read this
    pub params: &'a HashMap<String, f32>,

    /// Frame-time scalar (`Clock::elapsed().as_secs_f32()`). Only consumed
    /// by time-varying presets; identity ignores it.
    #[allow(dead_code)] // W3 presets will consume this
    pub clock_secs: f32,

    /// Optional secondary texture for overlay-style presets
    /// (`texture_overlay`). `None` for presets that don't take an overlay.
    /// W3 wires this against `Treatment.overlay_path`.
    #[allow(dead_code)] // W3 will populate this
    pub overlay: Option<&'a wgpu::TextureView>,

    /// Slot textures for collage-style presets. Empty for presets that
    /// don't take a collage. W3 wires this against `Treatment.collage_paths`.
    #[allow(dead_code)] // W3 will populate this
    pub collage: &'a [&'a wgpu::TextureView],
}

/// Static descriptor for a tunable preset parameter. The Selected-layer UI
/// (P1.2.3) uses this metadata to render per-key sliders with the right
/// label + range + default.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // fields read by P1.2.3 UI scaffold
pub struct ParamDescriptor {
    /// HashMap key under `Treatment.params`.
    pub key: &'static str,
    /// Human-readable label shown next to the slider.
    pub label: &'static str,
    /// Slider min (inclusive).
    pub min: f32,
    /// Slider max (inclusive).
    pub max: f32,
    /// Default value when the key is missing from `Treatment.params`.
    pub default: f32,
}

/// `(preset_id, display_label)` pairs for every preset registered with the
/// renderer. The Selected-layer UI sources its combobox options from this.
pub fn registry() -> &'static [(&'static str, &'static str)] {
    &[
        (IDENTITY_PRESET_ID, "Identity (no-op)"),
        (TONE_MAP_PRESET_ID, "Tone map"),
    ]
}

/// `true` if `preset_id` corresponds to a registered preset. CPU-only;
/// safe to call without a GPU device (used by audit + tests).
#[allow(dead_code)] // wired by future audit + P1.2.3 UI
pub fn is_registered(preset_id: &str) -> bool {
    registry().iter().any(|(id, _)| *id == preset_id)
}

/// Param descriptors for the named preset. Returns an empty slice for
/// unknown presets and for presets with no tunable parameters (identity).
pub fn param_descriptors(preset_id: &str) -> &'static [ParamDescriptor] {
    match preset_id {
        IDENTITY_PRESET_ID => &[],
        TONE_MAP_PRESET_ID => TONE_MAP_DESCRIPTORS,
        _ => &[],
    }
}

/// Static descriptors for the `tone_map` preset's three params.
/// At all defaults (`exposure=0, contrast=1, shoulder=0`) the shader is
/// a passthrough — the preset is visually transparent until the operator
/// tunes a slider.
const TONE_MAP_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "exposure",
        label: "Exposure (stops)",
        min: -2.0,
        max: 2.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "contrast",
        label: "Contrast",
        min: 0.5,
        max: 1.5,
        default: 1.0,
    },
    ParamDescriptor {
        key: "shoulder",
        label: "Highlight rolloff",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
];

/// Per-preset render pipelines. One field per preset; dispatch is a `match`
/// on `preset_id`. Mirrors the `FxPresetPipeline` shape so adding a preset
/// is "add a field + add a match arm" with no trait-object dispatch.
pub struct TreatmentPipeline {
    identity: IdentityTreatmentPipeline,
    tone_map: ToneMapTreatmentPipeline,
}

impl TreatmentPipeline {
    /// Build every preset's pipeline against `target_format` (the effect
    /// chain's ping-pong format — same as the surface format).
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self {
            identity: IdentityTreatmentPipeline::new(device, target_format),
            tone_map: ToneMapTreatmentPipeline::new(device, target_format),
        }
    }

    /// Render `inputs` through the preset named by `preset_id` into `dst`.
    ///
    /// Returns `true` if the dispatch ran. Returns `false` for an
    /// unregistered `preset_id`, leaving `dst` untouched — the caller is
    /// expected to fall back to the default `svg_pipeline` blit so the
    /// layer's source content still appears.
    pub fn dispatch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        preset_id: &str,
    ) -> bool {
        match preset_id {
            IDENTITY_PRESET_ID => {
                self.identity.render(device, encoder, dst, inputs);
                true
            }
            TONE_MAP_PRESET_ID => {
                self.tone_map.render(device, queue, encoder, dst, inputs);
                true
            }
            _ => false,
        }
    }
}

/// Identity treatment: blits `source` → `dst` through the same
/// `textured_quad.wgsl` shader the default path uses. Owns its own
/// `wgpu::RenderPipeline` (intentionally not shared with `svg_pipeline`)
/// so an end-to-end "identity treatment matches default path" test
/// exercises the dispatch code path rather than aliasing it.
struct IdentityTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl IdentityTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treatment identity (textured_quad.wgsl)"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/textured_quad.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treatment identity bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treatment identity pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treatment identity pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treatment identity sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treatment identity bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treatment identity pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// Tone-map treatment pipeline (P1.3.1). Reads exposure / contrast /
/// shoulder from `inputs.params`, applies the S-curve to each sampled
/// fragment, and writes into `dst` with alpha preserved.
struct ToneMapTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl ToneMapTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_tone_map.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/treat_tone_map.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treatment tone_map bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // fit_uniform (16 bytes)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
                // tone_map params (16 bytes)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treatment tone_map pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treatment tone_map pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treatment tone_map sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treatment tone_map params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
    ) {
        // Resolve params: exposure / contrast / shoulder fall back to the
        // descriptor defaults documented in `TONE_MAP_DESCRIPTORS`. The
        // identity defaults make the shader bit-exact passthrough.
        let exposure = inputs.params.get("exposure").copied().unwrap_or(0.0);
        let contrast = inputs.params.get("contrast").copied().unwrap_or(1.0);
        let shoulder = inputs.params.get("shoulder").copied().unwrap_or(0.0);

        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&exposure.to_le_bytes());
        bytes[4..8].copy_from_slice(&contrast.to_le_bytes());
        bytes[8..12].copy_from_slice(&shoulder.to_le_bytes());
        // bytes[12..16] reserved (zero).
        queue.write_buffer(&self.params_buf, 0, &bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treatment tone_map bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params_buf.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treatment tone_map pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Acceptance: the identity preset is registered.
    #[test]
    fn identity_preset_is_registered() {
        assert!(is_registered(IDENTITY_PRESET_ID));
    }

    /// Acceptance: the tone_map preset is registered (P1.3.1).
    #[test]
    fn tone_map_preset_is_registered() {
        assert!(is_registered(TONE_MAP_PRESET_ID));
    }

    /// Acceptance: tone_map's three documented params have descriptors,
    /// and all defaults round-trip through the identity case (exposure=0,
    /// contrast=1, shoulder=0 ⇒ shader passthrough).
    #[test]
    fn tone_map_descriptor_defaults_are_identity() {
        let descriptors = param_descriptors(TONE_MAP_PRESET_ID);
        assert_eq!(descriptors.len(), 3, "tone_map exposes exactly 3 params");

        let by_key: std::collections::HashMap<&str, &ParamDescriptor> =
            descriptors.iter().map(|d| (d.key, d)).collect();
        assert_eq!(by_key["exposure"].default, 0.0, "exposure identity = 0");
        assert_eq!(by_key["contrast"].default, 1.0, "contrast identity = 1");
        assert_eq!(by_key["shoulder"].default, 0.0, "shoulder identity = 0");

        // Min/max sanity: ranges are non-degenerate and bracket the default.
        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    /// Acceptance: unknown preset ids are not registered.
    #[test]
    fn unknown_preset_is_not_registered() {
        assert!(!is_registered(""));
        assert!(!is_registered("definitely-not-a-real-preset"));
        assert!(!is_registered("blur_mask")); // not yet wired in W3
    }

    /// Acceptance: every registered preset has an entry in
    /// `param_descriptors` (even if empty). Guards against W3 adding a
    /// registry entry but forgetting to wire descriptors.
    #[test]
    fn every_registered_preset_has_descriptor_entry() {
        for (id, _label) in registry() {
            // No panic = ok; identity returns &[], which is the documented
            // behaviour for presets with no tunable params.
            let _ = param_descriptors(id);
        }
    }

    /// Acceptance: the registry has no duplicate preset_ids. Catches a W3
    /// copy-paste mistake before it ships.
    #[test]
    fn registry_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for (id, _) in registry() {
            assert!(seen.insert(*id), "duplicate preset_id in registry: {id}");
        }
    }
}
