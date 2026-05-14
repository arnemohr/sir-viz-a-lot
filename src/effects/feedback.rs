//! PCleanup.1.4 — Feedback / trails effect pipeline.
//!
//! Reads `source_view` (current frame input) and `history_view` (previous
//! frame's output), mixes them by `decay`, writes the result to `dst_view`.
//! After the render pass, the caller copies `dst_view` → `history_view`
//! so the next frame samples the freshly-written output as history.
//!
//! The history texture is allocated per-layer in `LayerState` (not
//! per-effect). Operators stacking multiple Feedback effects on one
//! layer get shared history — a deliberate scope decision (the per-effect
//! variant would multiply the allocation footprint by the chain length).
//!
//! Lifecycle: history texture lives as long as the LayerState. wgpu
//! ref-counts the texture; dropping the LayerState releases the GPU
//! resource. No extra cleanup wiring needed.

/// Parameters for the feedback effect, matching the `FeedbackParams`
/// uniform struct in `feedback.wgsl` (4 × f32, 16 bytes, std140-friendly).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FeedbackParams {
    /// Mix factor between source and history. 0.0 = pure source (no
    /// trail); 1.0 = pure history (infinite hold). Typical operator
    /// range is 0.85–0.99 for "long ghosting trail without
    /// completely freezing the image."
    pub decay: f32,
    /// UV-space offset for sampling history. Positive `offset_x` makes
    /// trails appear to drift to the LEFT (history sampled at
    /// `uv - offset`). Both components clamped at the shader's sample
    /// point.
    pub offset: [f32; 2],
}

impl FeedbackParams {
    /// 16-byte little-endian wire format matching feedback.wgsl's
    /// FeedbackParams uniform: [decay, offset_x, offset_y, _pad].
    pub fn to_wire_bytes(self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&self.decay.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.offset[0].to_le_bytes());
        bytes[8..12].copy_from_slice(&self.offset[1].to_le_bytes());
        // bytes[12..16] left zeroed for std140 padding.
        bytes
    }
}

impl Default for FeedbackParams {
    fn default() -> Self {
        Self {
            decay: 0.0,
            offset: [0.0, 0.0],
        }
    }
}

/// Cached GPU pipeline for the feedback effect. Owns TWO render
/// pipelines:
///   * `mix_pipeline` — the main feedback pass (source × history → dst).
///   * `blit_pipeline` — passthrough copy of dst → history so the next
///     frame's mix samples this frame's output as history.
///
/// The two pipelines exist as separate shader modules + bind-group
/// layouts because trying to put both entry points in one .wgsl with
/// overlapping `@group(0) @binding(N)` declarations conflicts at
/// validation time (binding 1 would be a Texture for mix and a Sampler
/// for blit). Two .wgsl files keep each pipeline's bind-group surface
/// clean.
pub struct FeedbackPipeline {
    mix_pipeline: wgpu::RenderPipeline,
    mix_bgl: wgpu::BindGroupLayout,
    blit_pipeline: wgpu::RenderPipeline,
    blit_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl FeedbackPipeline {
    /// Build the feedback pipeline. Constructs the mix + blit
    /// sub-pipelines from their respective shaders.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        // ----- Mix pipeline (4 bindings: source, history, sampler, uniform) -----
        let mix_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("feedback.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../render/shaders/feedback.wgsl").into(),
            ),
        });

        let mix_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("feedback mix bgl"),
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
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let mix_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("feedback mix pipeline layout"),
            bind_group_layouts: &[Some(&mix_bgl)],
            immediate_size: 0,
        });

        let mix_pipeline = make_simple_quad_pipeline(
            device,
            &mix_shader,
            &mix_layout,
            "feedback mix pipeline",
            format,
        );

        // ----- Blit pipeline (2 bindings: source, sampler) -----
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("feedback_blit.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../render/shaders/feedback_blit.wgsl").into(),
            ),
        });

        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("feedback blit bgl"),
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
            ],
        });

        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("feedback blit pipeline layout"),
            bind_group_layouts: &[Some(&blit_bgl)],
            immediate_size: 0,
        });

        let blit_pipeline = make_simple_quad_pipeline(
            device,
            &blit_shader,
            &blit_layout,
            "feedback blit pipeline",
            format,
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("feedback effect sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            mix_pipeline,
            mix_bgl,
            blit_pipeline,
            blit_bgl,
            sampler,
        }
    }

    /// Run the two-pass feedback effect:
    ///   1. Mix: read source + history → write dst.
    ///   2. Blit: read dst → write history (so the next frame's mix sees
    ///      this frame's output).
    ///
    /// The two passes share `sampler`; pass 2 is a tiny textured-quad
    /// blit that bumps history forward without needing to extend the
    /// ping-pong texture allocator's usage flags to include COPY_SRC.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        history_view: &wgpu::TextureView,
        dst_view: &wgpu::TextureView,
        uniform_buffer: &wgpu::Buffer,
        params: FeedbackParams,
    ) {
        queue.write_buffer(uniform_buffer, 0, &params.to_wire_bytes());

        // ----- Pass 1: mix(source, history) → dst --------------------------
        {
            let mix_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("feedback mix bind group"),
                layout: &self.mix_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(source_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(history_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("feedback mix pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dst_view,
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
            pass.set_pipeline(&self.mix_pipeline);
            pass.set_bind_group(0, &mix_bg, &[]);
            pass.draw(0..6, 0..1);
        }

        // ----- Pass 2: blit(dst) → history --------------------------------
        // history_view becomes the attachment; dst_view is sampled as the
        // input. Pass 1 finished writing dst_view above (separate render
        // pass), so the read-after-write isn't a hazard.
        {
            let blit_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("feedback blit bind group"),
                layout: &self.blit_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(dst_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("feedback blit pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: history_view,
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
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &blit_bg, &[]);
            pass.draw(0..6, 0..1);
        }
    }
}

/// PCleanup.1.4 — shared render-pipeline constructor for the two feedback
/// passes (mix + blit). Both pipelines share the fullscreen-quad vertex
/// shape, no depth, REPLACE blend; only the bind-group layout + entry
/// point differ.
fn make_simple_quad_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    label: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
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
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PCleanup.1.4 — wire format matches the WGSL FeedbackParams struct
    /// (16 bytes: 4 × f32, std140-aligned).
    #[test]
    fn feedback_params_wire_format_is_16_bytes() {
        let bytes = FeedbackParams::default().to_wire_bytes();
        assert_eq!(bytes.len(), 16);
    }

    /// PCleanup.1.4 — default params produce a no-trail passthrough
    /// (decay=0). Inert-on-construction matches the pattern set by
    /// Effect::Tint (PCleanup.4.1) and the Color/Blur/Transform default
    /// chain — adding a Feedback effect doesn't change the layer until
    /// the operator turns up the decay.
    #[test]
    fn feedback_params_default_is_inert() {
        let p = FeedbackParams::default();
        assert_eq!(p.decay, 0.0);
        assert_eq!(p.offset, [0.0, 0.0]);
    }

    /// PCleanup.1.4 — wire-format encodes decay at offset 0, offset_x at
    /// 4, offset_y at 8, padding at 12. Renumbering would silently
    /// change every saved Feedback effect.
    #[test]
    fn feedback_params_wire_layout_stable() {
        let p = FeedbackParams {
            decay: 0.95,
            offset: [0.01, -0.02],
        };
        let bytes = p.to_wire_bytes();
        let read_decay = f32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let read_x = f32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let read_y = f32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let read_pad = f32::from_le_bytes(bytes[12..16].try_into().unwrap());
        assert!((read_decay - 0.95).abs() < 1e-6);
        assert!((read_x - 0.01).abs() < 1e-6);
        assert!((read_y - (-0.02)).abs() < 1e-6);
        assert_eq!(read_pad, 0.0);
    }
}
