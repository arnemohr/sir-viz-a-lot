//! 2D affine transform: translate, rotate, scale (about anchor) of the
//! layer's fullscreen quad. Vertex-stage mat4x4 multiplication; fragment
//! stage samples the source texture unchanged.
//!
//! Bind group: @binding(0) source texture, @binding(1) sampler,
//! @binding(2) uniform TransformParams { matrix: mat4x4<f32> } (64 bytes).
//!
//! Spec §2 + plan §3.4 M4.

use glam::{Mat3, Mat4, Vec2, Vec4};

/// Operator-facing transform controls. The matrix sent to the GPU is
/// `T(translate) * T(anchor) * R(rotate) * S(scale) * T(-anchor)` —
/// rotate-scale around `anchor`, then translate.
#[derive(Debug, Clone, Copy)]
pub struct TransformParams {
    pub translate: Vec2,
    /// Radians.
    pub rotate: f32,
    pub scale: Vec2,
    /// Pivot point in NDC space (default (0, 0) = origin).
    pub anchor: Vec2,
}

impl TransformParams {
    pub fn to_matrix(self) -> Mat3 {
        // 2D affine: T(translate) * T(anchor) * R * S * T(-anchor)
        let center = Mat3::from_translation(self.anchor);
        let uncenter = Mat3::from_translation(-self.anchor);
        let rot_scale = Mat3::from_scale_angle_translation(self.scale, self.rotate, Vec2::ZERO);
        let translate = Mat3::from_translation(self.translate);
        translate * center * rot_scale * uncenter
    }

    /// 64-byte LE wire format matching transform.wgsl's mat4x4<f32>.
    /// We promote the 2D Mat3 to a Mat4 (Z-axis identity) so WGSL's
    /// mat4x4 layout (16-byte aligned columns) maps trivially to the
    /// glam column-major bytes.
    pub fn to_wire_bytes(self) -> [u8; 64] {
        let m3 = self.to_matrix();
        // Promote to Mat4: x and y axes from m3 columns 0 and 1 (with z=0),
        // z axis is identity (0, 0, 1, 0), translation column from m3 col 2.
        let mat4 = Mat4::from_cols(
            Vec4::new(m3.x_axis.x, m3.x_axis.y, 0.0, 0.0),
            Vec4::new(m3.y_axis.x, m3.y_axis.y, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(m3.z_axis.x, m3.z_axis.y, 0.0, 1.0),
        );
        let arr = mat4.to_cols_array(); // [f32; 16] column-major
        let mut bytes = [0u8; 64];
        for (i, f) in arr.iter().enumerate() {
            bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        bytes
    }
}

impl Default for TransformParams {
    fn default() -> Self {
        Self {
            translate: Vec2::ZERO,
            rotate: 0.0,
            scale: Vec2::ONE,
            anchor: Vec2::ZERO,
        }
    }
}

/// Cached GPU pipeline for the transform effect.
///
/// Construct once at startup via [`TransformPipeline::new`]; call
/// [`TransformPipeline::render`] each frame you need the effect applied.
/// T-M4-06 will hold one of these on the effect dispatcher.
pub struct TransformPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl TransformPipeline {
    /// Build the transform effect pipeline. Run once at startup; the result is
    /// cached for the lifetime of the renderer.
    ///
    /// `format` must match the texture format used by the ping-pong textures
    /// in [`crate::render::pipeline::EffectPipeline`].
    ///
    /// Blend mode: [`wgpu::BlendState::REPLACE`] — the transform is a
    /// pass-through-with-modification, not an alpha-compositing step. The
    /// fragment shader preserves the source alpha exactly.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transform.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../render/shaders/transform.wgsl").into(),
            ),
        });

        // Bind group layout:
        //   binding 0 – 2D float texture (filterable), fragment-visible.
        //   binding 1 – filtering sampler, fragment-visible.
        //   binding 2 – uniform buffer (TransformParams, 64 bytes), vertex-visible.
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("transform effect bgl"),
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
                    visibility: wgpu::ShaderStages::VERTEX,
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
            label: Some("transform effect pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("transform effect pipeline"),
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
            label: Some("transform effect sampler"),
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

    /// Update the uniform buffer with `params` and record a single render
    /// pass that samples `source_view` and writes the transformed result
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
        uniform_buffer: &wgpu::Buffer,
        params: TransformParams,
    ) {
        queue.write_buffer(uniform_buffer, 0, &params.to_wire_bytes());

        // Build a fresh bind group per call (cheap; texture view varies).
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("transform effect bind group"),
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

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("transform effect pass"),
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
