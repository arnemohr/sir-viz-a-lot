//! Composite ordered layers into an offscreen RGBA target (T-M5-01).

use crate::project::schema::BlendMode;

pub struct Compositor {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    ping: wgpu::Texture,
    pong: wgpu::Texture,
    ping_view: wgpu::TextureView,
    pong_view: wgpu::TextureView,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

impl Compositor {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compositor.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compositor.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compositor bgl"),
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
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compositor layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("compositor pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
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
            label: Some("compositor sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let (ping, ping_view) = make_tex(device, width, height, format, "comp ping");
        let (pong, pong_view) = make_tex(device, width, height, format, "comp pong");

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            ping,
            pong,
            ping_view,
            pong_view,
            width,
            height,
            format,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }
        let (ping, ping_view) = make_tex(device, width, height, self.format, "comp ping");
        let (pong, pong_view) = make_tex(device, width, height, self.format, "comp pong");
        self.ping = ping;
        self.pong = pong;
        self.ping_view = ping_view;
        self.pong_view = pong_view;
        self.width = width;
        self.height = height;
    }

    /// Composite `layers` bottom → top, writing the final image into
    /// `target_view`. The compositor's internal ping/pong textures
    /// hold intermediate iterations; the **last** iteration writes to
    /// `target_view`, so callers can use the target as both a render
    /// destination (e.g. the projector RT) and the source for a
    /// downstream pass (e.g. gamma) without an extra blit.
    ///
    /// Layer count parity therefore no longer affects which buffer
    /// holds the final image — that was the v3 behaviour, removed
    /// under v4 (T3.0b) so per-layer warp output flows cleanly into
    /// the projector RT.
    ///
    /// Each layer supplies its own `uniform` buffer (16 bytes:
    /// opacity, blend mode code, …) so `queue.write_buffer` updates
    /// do not clobber each other before the GPU runs this pass.
    pub fn composite(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        background: wgpu::Color,
        target_view: &wgpu::TextureView,
        layers: &[(&wgpu::TextureView, BlendMode, f32, &wgpu::Buffer)],
    ) {
        // No layers: clear the target to the background colour and
        // bail. Skips the ping clear too — there is nothing to read.
        if layers.is_empty() {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("compositor clear-only"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(background),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            return;
        }

        // Prime ping with the background; subsequent iterations alternate
        // ping ↔ pong. The final iteration redirects its write to
        // `target_view` so the caller doesn't have to pick the right
        // buffer.
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("compositor clear bg"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.ping_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(background),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            drop(pass);
        }

        let mut read_view: &wgpu::TextureView = &self.ping_view;
        let mut write_ping = false;
        let last_idx = layers.len() - 1;

        for (i, (layer_view, blend, opacity, uniform_buf)) in layers.iter().enumerate() {
            let dst_view = if i == last_idx {
                target_view
            } else if write_ping {
                &self.ping_view
            } else {
                &self.pong_view
            };

            let params = [
                opacity.clamp(0.0, 1.0),
                blend_mode_code(*blend) as f32,
                0.0,
                0.0,
            ];
            queue.write_buffer(uniform_buf, 0, &params_to_bytes(&params));

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compositor bg"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(read_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(layer_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: uniform_buf.as_entire_binding(),
                    },
                ],
            });

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("compositor layer"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: dst_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: unsafe {
                                wgpu::LoadOp::DontCare(wgpu::LoadOpDontCare::enabled())
                            },
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

            // Only update read_view + flip if we wrote to a ping/pong
            // (the final iteration's target is read by gamma, not by
            // a subsequent compositor iteration).
            if i != last_idx {
                read_view = dst_view;
                write_ping = !write_ping;
            }
        }
    }
}

fn params_to_bytes(p: &[f32; 4]) -> [u8; 16] {
    let mut b = [0u8; 16];
    for (i, f) in p.iter().enumerate() {
        b[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
    }
    b
}

fn make_tex(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn blend_mode_code(b: BlendMode) -> u32 {
    match b {
        BlendMode::Normal => 0,
        BlendMode::Add => 1,
        BlendMode::Multiply => 2,
        BlendMode::Screen => 3,
    }
}
