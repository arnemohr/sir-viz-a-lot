//! Color effect: hue/sat/brightness/contrast. Single fragment-shader pass
//! sampling the source texture and writing the recolored result.
//!
//! Bind group: @binding(0) source texture, @binding(1) sampler,
//! @binding(2) uniform buffer with 4 × f32 (hue/sat/bri/con).
//!
//! HSV color model is used internally (see `color.wgsl`).
//!
//! Spec §2 + plan §3.4 M4. T-M4-06 will dispatch this from the Effect
//! enum; until then the pipeline is unused.

/// Parameters for the color effect, matching the `ColorParams` uniform
/// struct in `color.wgsl` (4 × f32, 16 bytes LE, std140-compatible).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ColorParams {
    /// Hue shift in degrees [-180, 180] typical; wraps via fract().
    pub hue_shift_deg: f32,
    /// Saturation multiplier; 1.0 = unchanged, 0.0 = greyscale.
    pub saturation_mul: f32,
    /// Brightness offset added to RGB; typical [-1.0, 1.0].
    pub brightness_add: f32,
    /// Contrast multiplier around 0.5; 1.0 = unchanged.
    pub contrast_mul: f32,
}

impl ColorParams {
    /// 16-byte little-endian wire format matching color.wgsl's uniform
    /// block. f32 is 4 bytes LE on every wgpu backend.
    pub fn to_wire_bytes(self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&self.hue_shift_deg.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.saturation_mul.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.brightness_add.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.contrast_mul.to_le_bytes());
        bytes
    }
}

impl Default for ColorParams {
    fn default() -> Self {
        Self {
            hue_shift_deg: 0.0,
            saturation_mul: 1.0,
            brightness_add: 0.0,
            contrast_mul: 1.0,
        }
    }
}

/// Cached GPU pipeline for the color effect.
///
/// Construct once at startup via [`ColorPipeline::new`]; call
/// [`ColorPipeline::render`] each frame you need the effect applied.
/// T-M4-06 will hold one of these on the effect dispatcher.
pub struct ColorPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Reusable uniform buffer; we write_buffer to update each frame
    /// rather than allocating per-frame.
    uniform_buffer: wgpu::Buffer,
}

impl ColorPipeline {
    /// Build the color effect pipeline. Run once at startup; the result is
    /// cached for the lifetime of the renderer.
    ///
    /// `format` must match the texture format used by the ping-pong textures
    /// in [`crate::render::pipeline::EffectPipeline`].
    ///
    /// Blend mode: [`wgpu::BlendState::REPLACE`] — the color effect is a
    /// pass-through-with-modification, not an alpha-compositing step. The
    /// fragment shader preserves the source alpha exactly.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("color.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../render/shaders/color.wgsl").into()),
        });

        // Bind group layout:
        //   binding 0 – 2D float texture (filterable), fragment-visible.
        //   binding 1 – filtering sampler, fragment-visible.
        //   binding 2 – uniform buffer (ColorParams, 16 bytes), fragment-visible.
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("color effect bgl"),
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
            label: Some("color effect pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("color effect pipeline"),
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
                    format,
                    // REPLACE: this is a pass-through-with-modification, not
                    // alpha compositing. The fragment shader preserves source
                    // alpha without blending against the destination.
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // Linear filter + ClampToEdge: matches SvgLayerPipeline sampler.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("color effect sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Uniform buffer: 16 bytes, UNIFORM | COPY_DST.
        // Initialized to ColorParams::default() so the first render call
        // is correct even if write_buffer is skipped (not that it should be).
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("color effect uniform buffer"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            uniform_buffer,
        }
    }

    /// Update the uniform buffer with `params` and record a single render
    /// pass that samples `source_view` and writes the recolored result
    /// into `dst_view`.
    ///
    /// The render pass clears `dst_view` to black before drawing; the
    /// fragment shader writes alpha from the source, so fully-transparent
    /// source pixels produce black pixels with alpha 0 in the output.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        dst_view: &wgpu::TextureView,
        params: ColorParams,
    ) {
        // Update uniform buffer with the current params.
        queue.write_buffer(&self.uniform_buffer, 0, &params.to_wire_bytes());

        // Build a fresh bind group per call (cheap; texture view varies).
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("color effect bind group"),
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
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("color effect pass"),
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

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}
