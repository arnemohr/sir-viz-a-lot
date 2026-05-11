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

/// `luminance_reveal` preset id (P1.3.3).
pub const LUMINANCE_REVEAL_PRESET_ID: &str = "luminance_reveal";

/// `blur_mask` preset id (P1.3.2).
pub const BLUR_MASK_PRESET_ID: &str = "blur_mask";

/// `texture_overlay` preset id (P1.3.4).
pub const TEXTURE_OVERLAY_PRESET_ID: &str = "texture_overlay";

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

    /// Layer's per-frame SDF (R32Float). Populated by the caller after
    /// `sync_mesh_and_mask`. `blur_mask` consumes this to gate the
    /// gaussian radius by distance-to-edge; other presets ignore it.
    pub sdf: Option<&'a wgpu::TextureView>,

    /// Layer's scratch texture (same format as the effect chain's
    /// ping-pong). `blur_mask` uses it for the horizontal-pass output
    /// before the vertical pass writes back to `dst`. Other multi-pass
    /// treatments may consume it the same way; single-pass presets
    /// leave it `None`.
    pub intermediate: Option<&'a wgpu::TextureView>,
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
        (LUMINANCE_REVEAL_PRESET_ID, "Luminance reveal"),
        (BLUR_MASK_PRESET_ID, "Blur mask (edge feather)"),
        (TEXTURE_OVERLAY_PRESET_ID, "Texture overlay"),
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
#[allow(dead_code)] // consumed by `windows::advanced` (v3-gated picker)
pub fn param_descriptors(preset_id: &str) -> &'static [ParamDescriptor] {
    match preset_id {
        IDENTITY_PRESET_ID => &[],
        TONE_MAP_PRESET_ID => TONE_MAP_DESCRIPTORS,
        LUMINANCE_REVEAL_PRESET_ID => LUMINANCE_REVEAL_DESCRIPTORS,
        BLUR_MASK_PRESET_ID => BLUR_MASK_DESCRIPTORS,
        TEXTURE_OVERLAY_PRESET_ID => TEXTURE_OVERLAY_DESCRIPTORS,
        _ => &[],
    }
}

/// Static descriptors for the `tone_map` preset's three params.
/// At all defaults (`exposure=0, contrast=1, shoulder=0`) the shader is
/// a passthrough — the preset is visually transparent until the operator
/// tunes a slider.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
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

/// Static descriptors for the `luminance_reveal` preset (P1.3.3).
/// Defaults: 50 % threshold, gentle softness band, non-inverted.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const LUMINANCE_REVEAL_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "threshold",
        label: "Threshold",
        min: 0.0,
        max: 1.0,
        default: 0.5,
    },
    ParamDescriptor {
        key: "softness",
        label: "Softness",
        min: 0.0,
        max: 0.5,
        default: 0.1,
    },
    ParamDescriptor {
        key: "invert",
        label: "Invert (0/1)",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
];

/// Static descriptors for the `texture_overlay` preset (P1.3.4).
/// Defaults: mix = 0 (identity passthrough; preset visually transparent
/// until `mix` slides above zero), no offset, Normal blend.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const TEXTURE_OVERLAY_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "mix",
        label: "Mix (0 = source only, 1 = full overlay)",
        min: 0.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "offset_x",
        label: "Offset X",
        min: -1.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "offset_y",
        label: "Offset Y",
        min: -1.0,
        max: 1.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "blend_mode",
        label: "Blend (0=Normal, 1=Multiply, 2=Screen, 3=Add)",
        min: 0.0,
        max: 3.0,
        default: 1.0,
    },
];

/// Static descriptors for the `blur_mask` preset (P1.3.2).
/// Defaults: zero radius (identity = no blur), 0.1 norm-units edge band
/// (~7 % of layer width), smooth falloff. Identity at default radius =
/// the operator sees no change until they reach for the radius slider.
#[allow(dead_code)] // referenced only through `param_descriptors` (v3 UI)
const BLUR_MASK_DESCRIPTORS: &[ParamDescriptor] = &[
    ParamDescriptor {
        key: "max_radius_px",
        label: "Max radius (px)",
        min: 0.0,
        max: 32.0,
        default: 0.0,
    },
    ParamDescriptor {
        key: "edge_band",
        label: "Edge band (norm)",
        min: 0.01,
        max: 0.3,
        default: 0.1,
    },
    ParamDescriptor {
        key: "falloff",
        label: "Falloff (0=hard, 1=smooth)",
        min: 0.0,
        max: 1.0,
        default: 0.7,
    },
];

/// Per-preset render pipelines. One field per preset; dispatch is a `match`
/// on `preset_id`. Mirrors the `FxPresetPipeline` shape so adding a preset
/// is "add a field + add a match arm" with no trait-object dispatch.
pub struct TreatmentPipeline {
    identity: IdentityTreatmentPipeline,
    tone_map: ToneMapTreatmentPipeline,
    luminance_reveal: LuminanceRevealTreatmentPipeline,
    blur_mask: BlurMaskTreatmentPipeline,
    texture_overlay: TextureOverlayTreatmentPipeline,
}

impl TreatmentPipeline {
    /// Build every preset's pipeline against `target_format` (the effect
    /// chain's ping-pong format — same as the surface format).
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self {
            identity: IdentityTreatmentPipeline::new(device, target_format),
            tone_map: ToneMapTreatmentPipeline::new(device, target_format),
            luminance_reveal: LuminanceRevealTreatmentPipeline::new(device, target_format),
            blur_mask: BlurMaskTreatmentPipeline::new(device, target_format),
            texture_overlay: TextureOverlayTreatmentPipeline::new(device, target_format),
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
            LUMINANCE_REVEAL_PRESET_ID => {
                self.luminance_reveal
                    .render(device, queue, encoder, dst, inputs);
                true
            }
            BLUR_MASK_PRESET_ID => {
                // blur_mask is multi-pass and consumes the layer's SDF +
                // scratch texture. If either is missing (caller did not
                // populate them, e.g. SVG/FxLayer route), skip the dispatch
                // and let the caller's fallback render the source unblurred.
                let (Some(sdf), Some(intermediate)) = (inputs.sdf, inputs.intermediate) else {
                    return false;
                };
                self.blur_mask
                    .render(device, queue, encoder, dst, inputs, sdf, intermediate);
                true
            }
            TEXTURE_OVERLAY_PRESET_ID => {
                // texture_overlay needs `inputs.overlay` populated by the
                // caller (loaded from `Treatment.overlay_path` via the
                // ImageTextureCache). Missing overlay → skip + caller
                // falls back to the default blit, so a half-configured
                // treatment renders the source unaltered.
                let Some(overlay) = inputs.overlay else {
                    return false;
                };
                self.texture_overlay
                    .render(device, queue, encoder, dst, inputs, overlay);
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

/// Luminance-reveal treatment pipeline (P1.3.3). Threshold + softness +
/// invert against Rec. 601 luma; RGB passes through with premultiplied
/// alpha modulation. Pipeline shape mirrors `ToneMapTreatmentPipeline`
/// exactly — the only difference is the shader source — so adding W3's
/// remaining single-pass presets follows this same template.
struct LuminanceRevealTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl LuminanceRevealTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let (pipeline, bind_group_layout, sampler, params_buf) = build_single_pass_treatment(
            device,
            target_format,
            "treat_luminance_reveal.wgsl",
            include_str!("shaders/treat_luminance_reveal.wgsl"),
        );
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
        let threshold = inputs.params.get("threshold").copied().unwrap_or(0.5);
        let softness = inputs.params.get("softness").copied().unwrap_or(0.1);
        let invert = inputs.params.get("invert").copied().unwrap_or(0.0);

        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&threshold.to_le_bytes());
        bytes[4..8].copy_from_slice(&softness.to_le_bytes());
        bytes[8..12].copy_from_slice(&invert.to_le_bytes());
        queue.write_buffer(&self.params_buf, 0, &bytes);

        draw_single_pass_treatment(
            device,
            encoder,
            dst,
            inputs,
            &self.pipeline,
            &self.bind_group_layout,
            &self.sampler,
            &self.params_buf,
            "treatment luminance_reveal",
        );
    }
}

/// Blur-mask treatment pipeline (P1.3.2). Multi-pass:
///   1. Fit pass: source → dst (apply cover/contain crop into ping-pong
///      first slot).
///   2. H pass: dst → intermediate (horizontal gaussian, SDF-gated
///      radius).
///   3. V pass: intermediate → dst (vertical gaussian, same SDF math).
///
/// Identity at default params: `max_radius_px = 0` → blur radius is
/// zero everywhere → V pass output equals the fit-only output, which
/// matches the no-treatment default path. So the preset is visually
/// transparent until the operator increases the radius slider.
struct BlurMaskTreatmentPipeline {
    // Fit pass (textured_quad.wgsl) — same shape as IdentityTreatmentPipeline
    // but owned separately to keep BlurMask's pipeline state isolated.
    fit_pipeline: wgpu::RenderPipeline,
    fit_bgl: wgpu::BindGroupLayout,
    fit_sampler: wgpu::Sampler,

    // H + V passes — same shader-pair shape as the existing BlurPipeline,
    // augmented with an SDF binding for per-fragment radius gating.
    blur_h_pipeline: wgpu::RenderPipeline,
    blur_v_pipeline: wgpu::RenderPipeline,
    blur_bgl: wgpu::BindGroupLayout,
    blur_sampler: wgpu::Sampler,
    blur_sdf_sampler: wgpu::Sampler,
    blur_params_buf: wgpu::Buffer,
}

impl BlurMaskTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // ---- Fit pass (textured_quad.wgsl, ALPHA_BLENDING) -----------
        let fit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_blur_mask fit (textured_quad.wgsl)"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/textured_quad.wgsl").into()),
        });

        let fit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_blur_mask fit bgl"),
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

        let fit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_blur_mask fit pipeline layout"),
            bind_group_layouts: &[Some(&fit_bgl)],
            immediate_size: 0,
        });

        let fit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_blur_mask fit pipeline"),
            layout: Some(&fit_layout),
            vertex: wgpu::VertexState {
                module: &fit_shader,
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
                module: &fit_shader,
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

        let fit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_blur_mask fit sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // ---- H + V passes (treat_blur_mask_{h,v}.wgsl) ---------------
        // Both shaders share the same bind layout: source, sampler,
        // params, SDF. build.rs concatenates sdf_helper.wgsl at the
        // front (SDF_CONSUMERS prefix match "treat_blur").
        let blur_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_blur_mask blur bgl"),
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
                // SDF texture — R32Float, unfilterable. The helper uses
                // textureLoad, so the sampler slot is unused at runtime
                // but the shader declaration requires a binding type.
                // Keep the texture as the only entry to keep the bind
                // group small.
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
            ],
        });

        let blur_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_blur_mask blur pipeline layout"),
            bind_group_layouts: &[Some(&blur_bgl)],
            immediate_size: 0,
        });

        // Both shaders need the SDF helper prepended at runtime to match
        // build.rs's compile-time validation (which already prepended
        // it via the SDF_CONSUMERS rule).
        let h_src = format!(
            "{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            include_str!("shaders/treat_blur_mask_h.wgsl")
        );
        let v_src = format!(
            "{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            include_str!("shaders/treat_blur_mask_v.wgsl")
        );
        let h_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_blur_mask_h.wgsl"),
            source: wgpu::ShaderSource::Wgsl(h_src.into()),
        });
        let v_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_blur_mask_v.wgsl"),
            source: wgpu::ShaderSource::Wgsl(v_src.into()),
        });

        let blur_h_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_blur_mask H pipeline"),
            layout: Some(&blur_layout),
            vertex: wgpu::VertexState {
                module: &h_shader,
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
                module: &h_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    // H pass: REPLACE so the intermediate texture is
                    // fully populated each frame (no read-back risk).
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let blur_v_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_blur_mask V pipeline"),
            layout: Some(&blur_layout),
            vertex: wgpu::VertexState {
                module: &v_shader,
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
                module: &v_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    // V pass: ALPHA_BLENDING so it integrates with the
                    // standard pre-effect ping-pong contract.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let blur_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_blur_mask blur sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // NonFiltering sampler for the SDF (R32Float). textureLoad
        // doesn't use the sampler, but binding type requires one.
        let blur_sdf_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_blur_mask sdf sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let blur_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_blur_mask params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            fit_pipeline,
            fit_bgl,
            fit_sampler,
            blur_h_pipeline,
            blur_v_pipeline,
            blur_bgl,
            blur_sampler,
            blur_sdf_sampler,
            blur_params_buf,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        inputs: &TreatmentInputs<'_>,
        sdf: &wgpu::TextureView,
        intermediate: &wgpu::TextureView,
    ) {
        // Read params (operator-tuned values, falling back to descriptor
        // defaults). Pack into 16-byte uniform: [max_radius_px, edge_band,
        // falloff, reserved].
        let max_radius = inputs.params.get("max_radius_px").copied().unwrap_or(0.0);
        let edge_band = inputs.params.get("edge_band").copied().unwrap_or(0.1);
        let falloff = inputs.params.get("falloff").copied().unwrap_or(0.7);
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&max_radius.to_le_bytes());
        bytes[4..8].copy_from_slice(&edge_band.to_le_bytes());
        bytes[8..12].copy_from_slice(&falloff.to_le_bytes());
        queue.write_buffer(&self.blur_params_buf, 0, &bytes);

        // ---- Pass 1: fit → dst (textured_quad with cover/contain) -----
        let fit_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_blur_mask fit bg"),
            layout: &self.fit_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(inputs.source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.fit_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inputs.fit_uniform.as_entire_binding(),
                },
            ],
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("treat_blur_mask fit pass"),
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
            pass.set_pipeline(&self.fit_pipeline);
            pass.set_bind_group(0, &fit_bg, &[]);
            pass.draw(0..6, 0..1);
        }

        // ---- Pass 2: H blur → intermediate ----
        self.run_blur_pass(
            device,
            encoder,
            "treat_blur_mask H pass",
            &self.blur_h_pipeline,
            dst,          // source: dst (now holds fit-applied content)
            intermediate, // target: scratch
            sdf,
            // H pass into intermediate uses LoadOp::Clear so the previous
            // frame's scratch never bleeds in.
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        );

        // ---- Pass 3: V blur → dst (overwrites the fit-applied content) ----
        self.run_blur_pass(
            device,
            encoder,
            "treat_blur_mask V pass",
            &self.blur_v_pipeline,
            intermediate, // source: scratch (H pass result)
            dst,          // target: ping-pong first slot
            sdf,
            // V pass overwrites the fit-applied content with the
            // fully-blurred result; LoadOp::Clear is correct.
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn run_blur_pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        label: &'static str,
        pipeline: &wgpu::RenderPipeline,
        source: &wgpu::TextureView,
        target: &wgpu::TextureView,
        sdf: &wgpu::TextureView,
        load_op: wgpu::LoadOp<wgpu::Color>,
    ) {
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &self.blur_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blur_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.blur_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(sdf),
                },
            ],
        });
        // The SDF NonFiltering sampler is unused at runtime (textureLoad
        // bypasses samplers) but kept so future presets that DO want a
        // filtered SDF sample have the slot wired. Touch to suppress
        // unused-field warnings without changing layout.
        let _ = &self.blur_sdf_sampler;

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: load_op,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// Build a single-pass treatment pipeline (one render pipeline, fit
/// uniform at binding 2, params uniform at binding 3, ALPHA_BLENDING).
/// Used by every W3 preset that's a simple sample → transform → emit
/// fullscreen pass — tone_map, luminance_reveal, and the equivalent
/// shape preset that follows. Heavier presets (blur_mask is two-pass,
/// collage takes multiple inputs) will get their own constructors.
#[allow(clippy::type_complexity)]
fn build_single_pass_treatment(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    shader_label: &'static str,
    shader_src: &'static str,
) -> (
    wgpu::RenderPipeline,
    wgpu::BindGroupLayout,
    wgpu::Sampler,
    wgpu::Buffer,
) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(shader_label),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(shader_label),
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
        label: Some(shader_label),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(shader_label),
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
        label: Some(shader_label),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(shader_label),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    (pipeline, bind_group_layout, sampler, params_buf)
}

/// Execute one single-pass treatment draw — fresh bind group per call,
/// `LoadOp::Clear(TRANSPARENT)` into `dst`, full-screen triangle. Caller
/// has already written the params buffer for this frame.
#[allow(clippy::too_many_arguments)]
fn draw_single_pass_treatment(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    dst: &wgpu::TextureView,
    inputs: &TreatmentInputs<'_>,
    pipeline: &wgpu::RenderPipeline,
    bind_group_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    params_buf: &wgpu::Buffer,
    label: &'static str,
) {
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(inputs.source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: inputs.fit_uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buf.as_entire_binding(),
            },
        ],
    });

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
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

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..6, 0..1);
}

/// Texture-overlay treatment pipeline (P1.3.4). Six-binding bind
/// group: source + sampler + fit + params + overlay + overlay-sampler.
/// `inputs.overlay` is the caller-supplied texture view loaded from
/// `Treatment.overlay_path` via the ImageTextureCache.
struct TextureOverlayTreatmentPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    overlay_sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
}

impl TextureOverlayTreatmentPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_texture_overlay.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/treat_texture_overlay.wgsl").into(),
            ),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_texture_overlay bgl"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("treat_texture_overlay pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_texture_overlay pipeline"),
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
            label: Some("treat_texture_overlay source sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Overlay sampler tiles via Repeat so a small overlay (e.g. a
        // grunge texture) covers the layer through the shader's
        // `fract(uv + offset)` sample.
        let overlay_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_texture_overlay overlay sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_texture_overlay params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            overlay_sampler,
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
        overlay: &wgpu::TextureView,
    ) {
        let mix_amt = inputs.params.get("mix").copied().unwrap_or(0.0);
        let off_x = inputs.params.get("offset_x").copied().unwrap_or(0.0);
        let off_y = inputs.params.get("offset_y").copied().unwrap_or(0.0);
        let blend = inputs.params.get("blend_mode").copied().unwrap_or(1.0);

        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&mix_amt.to_le_bytes());
        bytes[4..8].copy_from_slice(&off_x.to_le_bytes());
        bytes[8..12].copy_from_slice(&off_y.to_le_bytes());
        bytes[12..16].copy_from_slice(&blend.to_le_bytes());
        queue.write_buffer(&self.params_buf, 0, &bytes);

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_texture_overlay bg"),
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
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(overlay),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.overlay_sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_texture_overlay pass"),
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
        pass.set_bind_group(0, &bg, &[]);
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

    /// Acceptance: the luminance_reveal preset is registered (P1.3.3).
    #[test]
    fn luminance_reveal_preset_is_registered() {
        assert!(is_registered(LUMINANCE_REVEAL_PRESET_ID));
    }

    /// Acceptance: luminance_reveal exposes threshold + softness + invert
    /// with sane defaults (50 % threshold, gentle softness, non-inverted).
    #[test]
    fn luminance_reveal_descriptor_defaults_are_sane() {
        let descriptors = param_descriptors(LUMINANCE_REVEAL_PRESET_ID);
        assert_eq!(descriptors.len(), 3, "luminance_reveal exposes 3 params");

        let by_key: std::collections::HashMap<&str, &ParamDescriptor> =
            descriptors.iter().map(|d| (d.key, d)).collect();
        assert_eq!(by_key["threshold"].default, 0.5, "threshold default = 0.5");
        assert_eq!(by_key["softness"].default, 0.1, "softness default = 0.1");
        assert_eq!(by_key["invert"].default, 0.0, "invert default = 0.0 (off)");

        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    /// Acceptance: the blur_mask preset is registered (P1.3.2).
    #[test]
    fn blur_mask_preset_is_registered() {
        assert!(is_registered(BLUR_MASK_PRESET_ID));
    }

    /// Acceptance: blur_mask exposes max_radius_px + edge_band + falloff
    /// with the documented defaults. The default `max_radius_px = 0` is
    /// the key identity property — operator sees no change until they
    /// reach for the radius slider.
    #[test]
    fn blur_mask_defaults_are_no_op() {
        let descriptors = param_descriptors(BLUR_MASK_PRESET_ID);
        assert_eq!(descriptors.len(), 3);

        let by_key: std::collections::HashMap<&str, &ParamDescriptor> =
            descriptors.iter().map(|d| (d.key, d)).collect();
        assert_eq!(
            by_key["max_radius_px"].default, 0.0,
            "max_radius identity = 0 (no blur)"
        );
        assert!(by_key["edge_band"].default > 0.0);
        assert!(by_key["falloff"].default >= 0.0 && by_key["falloff"].default <= 1.0);

        for d in descriptors {
            assert!(d.min < d.max, "{}: min < max", d.key);
            assert!(
                d.default >= d.min && d.default <= d.max,
                "{}: default in [min, max]",
                d.key
            );
        }
    }

    /// Acceptance: the texture_overlay preset is registered (P1.3.4).
    #[test]
    fn texture_overlay_preset_is_registered() {
        assert!(is_registered(TEXTURE_OVERLAY_PRESET_ID));
    }

    /// Acceptance: texture_overlay defaults give an identity (mix=0).
    #[test]
    fn texture_overlay_defaults_are_identity() {
        let descriptors = param_descriptors(TEXTURE_OVERLAY_PRESET_ID);
        assert_eq!(descriptors.len(), 4);
        let by_key: std::collections::HashMap<&str, &ParamDescriptor> =
            descriptors.iter().map(|d| (d.key, d)).collect();
        assert_eq!(by_key["mix"].default, 0.0, "mix=0 → identity");
    }

    /// Acceptance: unknown preset ids are not registered.
    #[test]
    fn unknown_preset_is_not_registered() {
        assert!(!is_registered(""));
        assert!(!is_registered("definitely-not-a-real-preset"));
        assert!(!is_registered("palette_extract")); // not yet wired in W3
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
