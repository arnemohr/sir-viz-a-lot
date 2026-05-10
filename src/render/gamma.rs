//! Final gamma / brightness / contrast pass (T-M5-07).

pub struct GammaPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
}

impl GammaPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gamma.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/gamma.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gamma bgl"),
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
                        // P0.8.2 — uniform extended from 16 to 64
                        // bytes: tone vec4 + 3 RGB matrix rows
                        // (each padded to 16 bytes per std140 rules).
                        min_binding_size: std::num::NonZeroU64::new(64),
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gamma layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gamma pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("gamma sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gamma uniforms"),
            size: 64,
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

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        src: &wgpu::TextureView,
        gamma: f32,
        brightness: f32,
        contrast: f32,
        rgb_matrix: [[f32; 3]; 3],
        load: wgpu::LoadOp<wgpu::Color>,
    ) {
        // 64-byte uniform: 4×vec4 = tone + 3 matrix rows. Each row
        // is `[f32; 3]` from the Rust side, padded to vec4 with a
        // trailing 0.0 to satisfy std140 alignment rules.
        let mut b = [0u8; 64];
        let tone = [gamma.max(0.01), brightness, contrast, 0.0f32];
        for (i, f) in tone.iter().enumerate() {
            b[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        // Rows start at byte offsets 16, 32, 48.
        for (row_idx, row) in rgb_matrix.iter().enumerate() {
            let base = 16 * (row_idx + 1);
            for (col_idx, f) in row.iter().enumerate() {
                let off = base + col_idx * 4;
                b[off..off + 4].copy_from_slice(&f.to_le_bytes());
            }
            // Padding word (the .w of the vec4) is left as 0.
        }
        queue.write_buffer(&self.uniform_buffer, 0, &b);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gamma bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(src),
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
            label: Some("gamma pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
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
    use crate::project::schema::rgb_matrix_identity;

    /// P0.8.2 — pin the uniform-buffer layout for the identity
    /// matrix. Tone occupies bytes 0..16; rows live at 16, 32, 48
    /// (each padded to 16 bytes per std140). For identity:
    /// row_r = (1,0,0), row_g = (0,1,0), row_b = (0,0,1).
    ///
    /// Tests the byte-layout helper logic that lives inside
    /// `GammaPipeline::render`. We can't run the helper in
    /// isolation (it's tied to a wgpu queue); we instead replicate
    /// it here against the same shape and assert byte-equal.
    #[test]
    fn identity_matrix_packs_to_expected_bytes() {
        let mut b = [0u8; 64];
        let tone: [f32; 4] = [1.0, 0.0, 1.0, 0.0];
        for (i, f) in tone.iter().enumerate() {
            b[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        let rgb_matrix = rgb_matrix_identity();
        for (row_idx, row) in rgb_matrix.iter().enumerate() {
            let base = 16 * (row_idx + 1);
            for (col_idx, f) in row.iter().enumerate() {
                let off = base + col_idx * 4;
                b[off..off + 4].copy_from_slice(&f.to_le_bytes());
            }
        }

        // Tone block.
        assert_eq!(&b[0..4], &1.0_f32.to_le_bytes());
        assert_eq!(&b[4..8], &0.0_f32.to_le_bytes());
        assert_eq!(&b[8..12], &1.0_f32.to_le_bytes());
        assert_eq!(&b[12..16], &0.0_f32.to_le_bytes());

        // row_r at 16..28: (1, 0, 0).
        assert_eq!(&b[16..20], &1.0_f32.to_le_bytes());
        assert_eq!(&b[20..24], &0.0_f32.to_le_bytes());
        assert_eq!(&b[24..28], &0.0_f32.to_le_bytes());
        // pad 28..32 stays zero (we never write to it).
        assert_eq!(&b[28..32], &[0u8; 4]);

        // row_g at 32..44: (0, 1, 0).
        assert_eq!(&b[32..36], &0.0_f32.to_le_bytes());
        assert_eq!(&b[36..40], &1.0_f32.to_le_bytes());
        assert_eq!(&b[40..44], &0.0_f32.to_le_bytes());
        assert_eq!(&b[44..48], &[0u8; 4]);

        // row_b at 48..60: (0, 0, 1).
        assert_eq!(&b[48..52], &0.0_f32.to_le_bytes());
        assert_eq!(&b[52..56], &0.0_f32.to_le_bytes());
        assert_eq!(&b[56..60], &1.0_f32.to_le_bytes());
        assert_eq!(&b[60..64], &[0u8; 4]);
    }
}
