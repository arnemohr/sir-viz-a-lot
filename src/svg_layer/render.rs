//! GPU pipeline for rendering a rasterized SVG layer to the output surface.
//!
//! [`SvgLayerPipeline`] mirrors [`TestPatternRenderer`]'s structure: a cached
//! [`wgpu::RenderPipeline`] built once at startup, with a per-frame bind group
//! created on the fly from the current texture view. T-M3-06 wires this into
//! `App`'s render priority chain between `test_pattern` and the M1
//! `render_frame` fallback.
//!
//! [`TestPatternRenderer`]: crate::test_patterns::TestPatternRenderer

/// GPU pipeline that blits a 2-D texture (the rasterized SVG layer) onto the
/// output surface as a fullscreen quad using `textured_quad.wgsl`.
///
/// `SvgLayerPipeline::render` creates a fresh bind group per call (cheap on
/// wgpu, intentional per spec) rather than caching it, because the texture
/// view may change each time a new raster result is uploaded.
pub struct SvgLayerPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl SvgLayerPipeline {
    /// Build the textured-quad pipeline. Run once at startup via
    /// `init_running_app`; the result is cached for the lifetime of the app.
    ///
    /// Blend mode: [`wgpu::BlendState::ALPHA_BLENDING`] so transparent SVG
    /// areas stay transparent over the clear color (`REPLACE` would ignore
    /// alpha). The pass clears to transparent black so letterboxing and
    /// layer margins do not become opaque black when composited over layers
    /// below.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("textured_quad.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../render/shaders/textured_quad.wgsl").into(),
            ),
        });

        // Bind group layout: binding 0 = 2-D float texture (fragment-visible),
        //                    binding 1 = filtering sampler (fragment-visible),
        //                    binding 2 = fit-mode uniform (T-M8-04).
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("svg layer bgl"),
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
            label: Some("svg layer pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("svg layer pipeline"),
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
                    format: surface_format,
                    // ALPHA_BLENDING: standard src-alpha / one-minus-src-alpha so
                    // transparent SVG areas let the cleared-black background show
                    // through. BlendState::REPLACE would make alpha<1 pixels opaque.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // Linear filter + ClampToEdge: smooth scaling for SVGs that don't
        // happen to land pixel-perfect on the render target.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("svg layer sampler"),
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

    /// Render `texture_view` (the uploaded raster) as a fullscreen quad
    /// into `dst`. Builds a fresh bind group per call; clears `dst` to
    /// transparent before sampling so only ink contributes opacity upstream.
    ///
    /// `fit_uniform` is a per-layer 16-byte buffer the caller has filled
    /// with `[fit_mode, aspect_layer, focal_x, focal_y]` (see
    /// `textured_quad.wgsl`). For SVG layers pass `[0, 1, 0.5, 0.5]`
    /// (Stretch + 1:1) — the resvg path renders to a square pixmap so
    /// stretch is identity. For Image layers fill in the actual
    /// `LayerKind::Image::{fit, focal}` and the texture's true aspect.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        texture_view: &wgpu::TextureView,
        fit_uniform: &wgpu::Buffer,
    ) {
        // Fresh bind group per frame: cheap on wgpu, necessary because the
        // texture view may have been replaced since the last upload.
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("svg layer bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: fit_uniform.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("svg layer pass"),
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
