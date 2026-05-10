//! Built-in calibration sources. Independent of any SVG layer so warp setup
//! works before any content is loaded.

use wgpu::util::DeviceExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestPattern {
    #[default]
    None,
    Grid50,
    Crosshair,
    White100,
    White50,
    White25,
    ColorBars,
    /// P0.7.4 — alignment cross with quarter / half / three-quarter
    /// reference markings, for two-projector physical alignment.
    AlignmentCross,
    /// P0.7.4 — horizontal 0→1 luminance ramp across the canvas.
    /// Verifies the edge-blend overlap + falloff (P0.7.3) without
    /// media on the canvas.
    EdgeBlendGradient,
}

impl TestPattern {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "off",
            Self::Grid50 => "grid 50px",
            Self::Crosshair => "crosshair",
            Self::White100 => "white 100%",
            Self::White50 => "white 50%",
            Self::White25 => "white 25%",
            Self::ColorBars => "color bars",
            Self::AlignmentCross => "alignment cross",
            Self::EdgeBlendGradient => "edge-blend gradient",
        }
    }

    /// Cycle order driven by the `T` key in `App::window_event`. Wraps
    /// back to `None` after the last pattern. Exhaustive match —
    /// adding a variant later forces an update here.
    ///
    /// The two-projector patterns sit at the end of the cycle so the
    /// single-projector operator sees the v3 patterns first and only
    /// scrolls past them when reaching for calibration tools.
    pub fn next(self) -> Self {
        match self {
            Self::None => Self::Grid50,
            Self::Grid50 => Self::Crosshair,
            Self::Crosshair => Self::White100,
            Self::White100 => Self::White50,
            Self::White50 => Self::White25,
            Self::White25 => Self::ColorBars,
            Self::ColorBars => Self::AlignmentCross,
            Self::AlignmentCross => Self::EdgeBlendGradient,
            Self::EdgeBlendGradient => Self::None,
        }
    }
}

/// Owns the cached pipelines + per-mode bind groups for the three test-pattern
/// shaders (`test_grid.wgsl`, `test_crosshair.wgsl`, `test_levels.wgsl`).
///
/// `TestPattern` is a plain `Copy` enum carrying no GPU state, so the dispatch
/// entry point lives here instead of on the enum itself. T-M2-09 will hold one
/// of these on the renderer / output window and call [`Self::render`] each
/// frame the user has selected a non-`None` pattern.
///
/// The four `levels` variants share a single pipeline; they're disambiguated
/// by a `mode: u32` uniform sourced from one of four pre-built bind groups
/// (one per mode). Each bind group is backed by a 4-byte uniform buffer that
/// must outlive it, so the buffers are kept in their own array on the struct.
pub struct TestPatternRenderer {
    grid: wgpu::RenderPipeline,
    crosshair: wgpu::RenderPipeline,
    levels: wgpu::RenderPipeline,
    levels_bind_groups: [wgpu::BindGroup; 4],
    // Backing storage for the bind groups above. The bind groups borrow these
    // buffers internally (via Arc inside wgpu), so they outlive their
    // referents trivially, but we keep the field anyway so the buffers can be
    // inspected / re-uploaded if we ever need that and so the lifetime story
    // is obvious from the struct layout.
    #[allow(dead_code)]
    levels_uniform_buffers: [wgpu::Buffer; 4],
    /// P0.7.4 — two-projector alignment + edge-blend calibration
    /// patterns. Each gets its own pipeline (no shared uniform — the
    /// shaders are entirely procedural off the fragment UV).
    alignment_cross: wgpu::RenderPipeline,
    edge_blend_gradient: wgpu::RenderPipeline,
}

impl TestPatternRenderer {
    /// Build all three pipelines + the four `levels` bind groups. Run once at
    /// startup; the result is cached for the lifetime of the renderer.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        // ---------- shader modules ----------
        let grid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_grid.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("render/shaders/test_grid.wgsl").into()),
        });
        let crosshair_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_crosshair.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("render/shaders/test_crosshair.wgsl").into(),
            ),
        });
        let levels_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_levels.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("render/shaders/test_levels.wgsl").into(),
            ),
        });
        let alignment_cross_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_alignment_cross.wgsl"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("render/shaders/test_alignment_cross.wgsl").into(),
            ),
        });
        let edge_blend_gradient_shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("test_edge_blend_gradient.wgsl"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("render/shaders/test_edge_blend_gradient.wgsl").into(),
                ),
            });

        // ---------- bind group layout for the levels `mode` uniform ----------
        let levels_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("test_levels mode bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // ---------- pipeline layouts ----------
        let empty_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("test_pattern empty pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let levels_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("test_levels pipeline layout"),
            bind_group_layouts: &[Some(&levels_bgl)],
            immediate_size: 0,
        });

        // ---------- pipelines ----------
        let grid = build_pipeline(
            device,
            &grid_shader,
            &empty_layout,
            surface_format,
            "test_grid pipeline",
        );
        let crosshair = build_pipeline(
            device,
            &crosshair_shader,
            &empty_layout,
            surface_format,
            "test_crosshair pipeline",
        );
        let levels = build_pipeline(
            device,
            &levels_shader,
            &levels_layout,
            surface_format,
            "test_levels pipeline",
        );
        let alignment_cross = build_pipeline(
            device,
            &alignment_cross_shader,
            &empty_layout,
            surface_format,
            "test_alignment_cross pipeline",
        );
        let edge_blend_gradient = build_pipeline(
            device,
            &edge_blend_gradient_shader,
            &empty_layout,
            surface_format,
            "test_edge_blend_gradient pipeline",
        );

        // ---------- per-mode uniform buffers + bind groups ----------
        // `bytemuck` is intentionally not a dependency; `u32::to_le_bytes`
        // gives us the 4 little-endian bytes wgpu wants directly. (wgpu's
        // uniform buffer layout for a `u32` is 4 bytes LE on every backend.)
        let make_buffer = |mode: u32| -> wgpu::Buffer {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(match mode {
                    0 => "test_levels uniform mode=0 (white100)",
                    1 => "test_levels uniform mode=1 (white50)",
                    2 => "test_levels uniform mode=2 (white25)",
                    _ => "test_levels uniform mode=3 (color bars)",
                }),
                contents: &mode.to_le_bytes(),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        };

        let levels_uniform_buffers: [wgpu::Buffer; 4] = [
            make_buffer(0),
            make_buffer(1),
            make_buffer(2),
            make_buffer(3),
        ];

        let make_bind_group = |idx: usize, buf: &wgpu::Buffer| -> wgpu::BindGroup {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(match idx {
                    0 => "test_levels bind group mode=0",
                    1 => "test_levels bind group mode=1",
                    2 => "test_levels bind group mode=2",
                    _ => "test_levels bind group mode=3",
                }),
                layout: &levels_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            })
        };

        let levels_bind_groups: [wgpu::BindGroup; 4] = [
            make_bind_group(0, &levels_uniform_buffers[0]),
            make_bind_group(1, &levels_uniform_buffers[1]),
            make_bind_group(2, &levels_uniform_buffers[2]),
            make_bind_group(3, &levels_uniform_buffers[3]),
        ];

        Self {
            grid,
            crosshair,
            levels,
            levels_bind_groups,
            levels_uniform_buffers,
            alignment_cross,
            edge_blend_gradient,
        }
    }

    /// Render `pattern` into `dst`. `TestPattern::None` is a no-op (no pass
    /// recorded); every other variant records a single render pass that
    /// clears `dst` to black and draws the relevant fullscreen quad.
    ///
    /// Clearing to black is safe even for the levels variants that overwrite
    /// every pixel — the clear is just paid as a cheap initial state.
    pub fn render(
        &self,
        pattern: TestPattern,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
    ) {
        // Compile-time exhaustiveness: any new variant added to `TestPattern`
        // forces an update here.
        let (pipeline, bind_group): (&wgpu::RenderPipeline, Option<&wgpu::BindGroup>) =
            match pattern {
                TestPattern::None => return,
                TestPattern::Grid50 => (&self.grid, None),
                TestPattern::Crosshair => (&self.crosshair, None),
                TestPattern::White100 => (&self.levels, Some(&self.levels_bind_groups[0])),
                TestPattern::White50 => (&self.levels, Some(&self.levels_bind_groups[1])),
                TestPattern::White25 => (&self.levels, Some(&self.levels_bind_groups[2])),
                TestPattern::ColorBars => (&self.levels, Some(&self.levels_bind_groups[3])),
                TestPattern::AlignmentCross => (&self.alignment_cross, None),
                TestPattern::EdgeBlendGradient => (&self.edge_blend_gradient, None),
            };

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("test_pattern pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
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
        pass.set_pipeline(pipeline);
        if let Some(bg) = bind_group {
            pass.set_bind_group(0, bg, &[]);
        }
        pass.draw(0..6, 0..1);
    }
}

/// Build a render pipeline with the standard fullscreen-quad shape used by
/// every test-pattern shader: vertex stage `vs_main`, fragment stage
/// `fs_main`, no vertex buffers, triangle list, no depth/stencil, no MSAA,
/// blend = REPLACE, write all color channels, target = `surface_format`.
fn build_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    surface_format: wgpu::TextureFormat,
    label: &str,
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
                format: surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
