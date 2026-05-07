//! Separable gaussian blur: horizontal pass then vertical pass into
//! ping-pong textures. Kernel size derived from radius_px (clamped to
//! ≤ 32 px in the shader). Spec §2.

/// Parameters for the blur effect, matching the `BlurParams` uniform struct in
/// `blur_h.wgsl` and `blur_v.wgsl` (1 × f32, 16-byte wire format).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BlurParams {
    pub radius_px: f32,
}

impl BlurParams {
    /// 16-byte LE wire format. WGSL std uniform layout pads a 4-byte
    /// scalar to 16 bytes when alone in a struct. We write the scalar
    /// at offset 0 and zero the padding.
    pub fn to_wire_bytes(self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&self.radius_px.to_le_bytes());
        bytes
    }
}

impl Default for BlurParams {
    fn default() -> Self {
        Self { radius_px: 0.0 }
    }
}

/// Cached GPU pipelines for the separable gaussian blur.
///
/// Holds both the horizontal-pass and vertical-pass pipelines plus a shared
/// sampler and uniform buffer (both passes use the same `radius_px`).
///
/// Construct once at startup via [`BlurPipeline::new`]; call
/// [`BlurPipeline::apply`] each frame you need the effect applied.
/// T-M4-06 will hold one of these on the effect dispatcher.
pub struct BlurPipeline {
    pipeline_h: wgpu::RenderPipeline,
    pipeline_v: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl BlurPipeline {
    /// Build both blur pipelines (h + v) sharing a single BGL, sampler, and
    /// uniform buffer.
    ///
    /// `format` must match the texture format used by the ping-pong textures
    /// in [`crate::render::pipeline::EffectPipeline`].
    ///
    /// Blend mode: [`wgpu::BlendState::REPLACE`] — the blur is a
    /// pass-through-with-modification, not an alpha-compositing step.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader_h = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blur_h.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../render/shaders/blur_h.wgsl").into()),
        });

        let shader_v = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blur_v.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../render/shaders/blur_v.wgsl").into()),
        });

        // Bind group layout:
        //   binding 0 – 2D float texture (filterable), fragment-visible.
        //   binding 1 – filtering sampler, fragment-visible.
        //   binding 2 – uniform buffer (BlurParams, 16 bytes), fragment-visible.
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blur effect bgl"),
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
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blur effect pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline_h = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blur h pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_h,
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
                module: &shader_h,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // REPLACE: this is a pass-through-with-modification, not
                    // alpha compositing.
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let pipeline_v = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blur v pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_v,
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
                module: &shader_v,
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
        });

        // Linear filter + ClampToEdge: matches SvgLayerPipeline sampler.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blur effect sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            pipeline_h,
            pipeline_v,
            bind_group_layout,
            sampler,
        }
    }

    /// Apply the separable blur: `source_view` → `intermediate_view` (h pass)
    /// → `dst_view` (v pass). Two render passes recorded into `encoder`.
    ///
    /// `intermediate_view` must be a texture distinct from both `source_view`
    /// and `dst_view`. A typical in-place arrangement is
    /// `(source=ping, intermediate=pong, dst=ping)`: pass 1 reads ping and
    /// writes pong; pass 2 reads pong and writes ping, which is safe because
    /// pong is not re-read after being written.
    ///
    /// Both passes clear to [`wgpu::Color::BLACK`] before drawing.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        intermediate_view: &wgpu::TextureView,
        dst_view: &wgpu::TextureView,
        uniform_buffer: &wgpu::Buffer,
        params: BlurParams,
    ) {
        queue.write_buffer(uniform_buffer, 0, &params.to_wire_bytes());

        // --- Horizontal pass: source -> intermediate ---
        let bind_group_h = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur h bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur h pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: intermediate_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.pipeline_h);
            pass.set_bind_group(0, &bind_group_h, &[]);
            pass.draw(0..6, 0..1);
        }

        // --- Vertical pass: intermediate -> dst ---
        let bind_group_v = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur v bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(intermediate_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur v pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dst_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.pipeline_v);
            pass.set_bind_group(0, &bind_group_v, &[]);
            pass.draw(0..6, 0..1);
        }
    }
}
