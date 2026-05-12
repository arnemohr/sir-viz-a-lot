//! P2.5.1 — compute pipeline + double-buffered SSBO + quad-instance render
//! pipeline for particle FX presets. Shared scaffolding; per-preset compute
//! shaders branch in dispatch.
//!
//! # Particle layout (`Particle` struct — 32 bytes, std430)
//!
//! | field     | type       | offset | size |
//! |-----------|------------|--------|------|
//! | `pos`     | `vec2<f32>`|  0     |  8   |
//! | `vel`     | `vec2<f32>`|  8     |  8   |
//! | `age_secs`| `f32`      | 16     |  4   |
//! | `_pad`    | `f32`      | 20     |  4   |
//! | `_pad2`   | `f32`      | 24     |  4   |
//! | `_pad3`   | `f32`      | 28     |  4   |
//! Total: 32 bytes per particle.  `MAX_PARTICLES = 2048`.
//!
//! # Double-buffer strategy
//!
//! Two SSBOs of equal size (`MAX_PARTICLES × 32` bytes) are allocated at
//! construction time.  Each frame, the compute pass writes into the *write*
//! buffer; the vertex/fragment pass reads from the same *write* buffer (since
//! `particles_identity` does not read prior state).  The "current" index
//! (`write_idx`) flips after `dispatch_compute`; `draw_particles` always reads
//! the buffer that the compute pass most recently wrote, which is the
//! *other* index after the flip.
//!
//! For `particles_identity` the flip is cosmetically irrelevant (the compute
//! pass ignores prior state), but the contract must be correct now so P2.5.2
//! (read-modify-write physics) can rely on it.
//!
//! # Bind-group slots
//!
//! Compute pass (group 0):
//!   binding 2: `FxParamsUniform`      (8 × f32, 32 bytes)
//!   binding 3: `ClockUniform`         (vec4<f32>)
//!   binding 5: output SSBO            (`var<storage, read_write>`)
//!
//! Render pass (group 0):
//!   binding 3: `ClockUniform`         (vec4<f32>)
//!   binding 4: `ResUniform`           (vec4<f32>, .xy = output size)
//!   binding 5: particle SSBO          (`var<storage, read>`)
//!
//! # TODO
//!
//! ```text
//! // TODO(P2.9.2): golden test for 16 dots on circular mask seed=42.
//! ```

use std::collections::HashMap;

use crate::render::fx_presets::FxParamsUniform;
use crate::render::sdf::SDF_HELPER_WGSL;

/// Maximum number of particles per `FxComputePipeline` SSBO.
/// Sized for the highest Phase-2 leaf-preset budget; the compute shader
/// clamps `n_particles` to this value before writing.
pub const MAX_PARTICLES: u32 = 2048;

/// Size in bytes of one `Particle` in the SSBO (std430 layout, 32-byte stride).
const PARTICLE_STRIDE: u64 = 32;

/// Total SSBO size for `MAX_PARTICLES` particles.
const SSBO_SIZE: u64 = MAX_PARTICLES as u64 * PARTICLE_STRIDE;

/// Compute + render pipeline for particle-based FX presets.
///
/// Owns two SSBOs (double-buffered), the compute pipeline (writes particle
/// positions), and the vertex + fragment pipelines (reads positions, draws
/// 2×2 px quads).
///
/// One `FxComputePipeline` is constructed per preset variant; different
/// preset variants (`particles_identity`, future ones) use different compute
/// shaders but the same vertex + fragment shaders.
pub struct FxComputePipeline {
    // --- Compute ---
    compute_pipeline: wgpu::ComputePipeline,
    compute_bgl: wgpu::BindGroupLayout,

    // --- Render ---
    render_pipeline: wgpu::RenderPipeline,
    render_bgl: wgpu::BindGroupLayout,

    // --- Shared uniform buffers (written each frame) ---
    params_buf: wgpu::Buffer,
    clock_buf: wgpu::Buffer,
    res_buf: wgpu::Buffer,

    // --- Double-buffered SSBOs ---
    ssbo: [wgpu::Buffer; 2],
    /// Index of the SSBO the compute pass will write to next frame.
    write_idx: std::cell::Cell<usize>,
}

impl FxComputePipeline {
    /// Build the `particles_identity` variant.
    ///
    /// `target_format` must match the `fx_texture` format so blending is
    /// correct when the particle quads are composited over the FxLayer.
    pub fn new_particles_identity(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        // ---- compute shader ------------------------------------------------
        let compute_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_particles_identity_compute.wgsl")
        );
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fx_particles_identity_compute.wgsl"),
            source: wgpu::ShaderSource::Wgsl(compute_src.into()),
        });

        // ---- compute bind-group layout -------------------------------------
        // binding 2: FxParamsUniform
        // binding 3: ClockUniform
        // binding 5: output SSBO (read_write)
        let compute_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particles identity compute bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particles identity compute pipeline layout"),
            bind_group_layouts: &[Some(&compute_bgl)],
            immediate_size: 0,
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("particles identity compute pipeline"),
            layout: Some(&compute_layout),
            module: &compute_shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // ---- vertex + fragment shaders -------------------------------------
        let vertex_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_particles_vertex.wgsl")
        );
        let fragment_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_particles_fragment.wgsl")
        );
        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fx_particles_vertex.wgsl"),
            source: wgpu::ShaderSource::Wgsl(vertex_src.into()),
        });
        let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fx_particles_fragment.wgsl"),
            source: wgpu::ShaderSource::Wgsl(fragment_src.into()),
        });

        // ---- render bind-group layout --------------------------------------
        // binding 3: ClockUniform
        // binding 4: ResUniform
        // binding 5: particle SSBO (read-only)
        let render_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particles identity render bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let render_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particles identity render pipeline layout"),
            bind_group_layouts: &[Some(&render_bgl)],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("particles identity render pipeline"),
            layout: Some(&render_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
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
                module: &fragment_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // ---- uniform buffers -----------------------------------------------
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particles identity params"),
            size: std::mem::size_of::<FxParamsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let clock_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particles identity clock"),
            size: 16, // vec4<f32>
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let res_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particles identity res"),
            size: 16, // vec4<f32>
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ---- SSBOs ---------------------------------------------------------
        let ssbo = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("particles identity ssbo 0"),
                size: SSBO_SIZE,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("particles identity ssbo 1"),
                size: SSBO_SIZE,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        ];

        Self {
            compute_pipeline,
            compute_bgl,
            render_pipeline,
            render_bgl,
            params_buf,
            clock_buf,
            res_buf,
            ssbo,
            write_idx: std::cell::Cell::new(0),
        }
    }

    /// Upload uniforms and run the compute pass, writing particle positions
    /// into the write-side SSBO.
    ///
    /// After this call, `write_idx` advances so `draw_particles` reads the
    /// freshly written buffer.
    ///
    /// `n_particles` is clamped to `MAX_PARTICLES` before dispatch.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_compute(
        &self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        n_particles: u32,
        seed: u64,
        clock_secs: f32,
        t_layer_added_secs: f32,
        params: &HashMap<String, f32>,
    ) {
        let n = n_particles.min(MAX_PARTICLES);
        let t_local = clock_secs - t_layer_added_secs;

        // Pack u64 seed into the lower 23 bits of a f32 mantissa (lossless
        // for seed values up to 2^23; sufficient for layout variation).
        let seed_f = (seed as u32 & 0x7f_ffff) as f32;

        // --- upload FxParamsUniform: particle_count → wavelength slot -------
        let pu = FxParamsUniform::for_particles_identity(params);
        let mut params_bytes = [0u8; 32];
        let floats = [
            pu.wavelength,
            pu.speed,
            pu.falloff,
            pu.base_r,
            pu.base_g,
            pu.base_b,
            pu._pad0,
            pu._pad1,
        ];
        for (i, f) in floats.iter().enumerate() {
            params_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.params_buf, 0, &params_bytes);

        // --- upload ClockUniform ----------------------------------------
        // .x = clock_secs, .y = t_layer_local_secs, .z = seed_f, .w = n
        let mut clock_bytes = [0u8; 16];
        clock_bytes[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        clock_bytes[4..8].copy_from_slice(&t_local.to_le_bytes());
        clock_bytes[8..12].copy_from_slice(&seed_f.to_le_bytes());
        clock_bytes[12..16].copy_from_slice(&(n as f32).to_le_bytes());
        queue.write_buffer(&self.clock_buf, 0, &clock_bytes);

        // --- compute pass ---------------------------------------------------
        let w_idx = self.write_idx.get();
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particles identity compute bg"),
            layout: &self.compute_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.clock_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.ssbo[w_idx].as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("particles identity compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            // Workgroup size is 64; dispatch ceil(n / 64) groups.
            let groups = n.div_ceil(64);
            pass.dispatch_workgroups(groups, 1, 1);
        }

        // Advance write index so draw_particles reads the buffer we just wrote.
        self.write_idx.set(1 - w_idx);
    }

    // ------------------------------------------------------------------
    // P2.5.2–P2.5.5 SDF-reading compute variants
    // ------------------------------------------------------------------

    /// Internal helper: build a compute bind-group layout that adds an SDF
    /// texture (R32Float, unfilterable) at binding 6, in addition to the
    /// standard identity slots (bindings 2, 3, 5).
    ///
    /// All four SDF-reading particle presets (P2.5.2–P2.5.5) share this
    /// layout, differing only in their compute shader source.
    fn make_sdf_compute_bgl(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 6: SDF texture (R32Float, unfilterable)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        })
    }

    /// Internal helper: build a compute pipeline from shader source + the
    /// given bind-group layout.  Used by all SDF-reading particle presets.
    fn make_sdf_compute_pipeline(
        device: &wgpu::Device,
        shader_label: &str,
        compute_src: &str,
        compute_bgl: &wgpu::BindGroupLayout,
        pipeline_label: &str,
    ) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(shader_label),
            source: wgpu::ShaderSource::Wgsl(compute_src.to_string().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(pipeline_label),
            bind_group_layouts: &[Some(compute_bgl)],
            immediate_size: 0,
        });
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(pipeline_label),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        })
    }

    /// Internal helper: build the vertex + fragment pipelines and render
    /// bind-group layout shared by all particle presets (identical to
    /// `new_particles_identity`; factored out to avoid repetition).
    fn make_particle_render_pipeline(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        label_prefix: &str,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
        let vertex_src = format!(
            "{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            include_str!("shaders/fx_particles_vertex.wgsl")
        );
        let fragment_src = format!(
            "{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            include_str!("shaders/fx_particles_fragment.wgsl")
        );
        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{label_prefix} vertex")),
            source: wgpu::ShaderSource::Wgsl(vertex_src.into()),
        });
        let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{label_prefix} fragment")),
            source: wgpu::ShaderSource::Wgsl(fragment_src.into()),
        });
        let render_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{label_prefix} render bgl")),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let render_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{label_prefix} render layout")),
            bind_group_layouts: &[Some(&render_bgl)],
            immediate_size: 0,
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("{label_prefix} render pipeline")),
            layout: Some(&render_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
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
                module: &fragment_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        (render_pipeline, render_bgl)
    }

    /// Internal helper: allocate the three uniform buffers and two SSBOs
    /// that all particle presets share.
    fn make_particle_buffers(
        device: &wgpu::Device,
        label_prefix: &str,
    ) -> (wgpu::Buffer, wgpu::Buffer, wgpu::Buffer, [wgpu::Buffer; 2]) {
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label_prefix} params")),
            size: std::mem::size_of::<FxParamsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let clock_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label_prefix} clock")),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let res_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label_prefix} res")),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ssbo = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{label_prefix} ssbo 0")),
                size: SSBO_SIZE,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{label_prefix} ssbo 1")),
                size: SSBO_SIZE,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        ];
        (params_buf, clock_buf, res_buf, ssbo)
    }

    /// P2.5.2 — Build the `mask_constrained_drift` variant.
    pub fn new_constrained_drift(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let compute_bgl = Self::make_sdf_compute_bgl(device, "constrained_drift compute bgl");
        let compute_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_particles_drift.wgsl")
        );
        let compute_pipeline = Self::make_sdf_compute_pipeline(
            device,
            "fx_particles_drift.wgsl",
            &compute_src,
            &compute_bgl,
            "constrained_drift compute pipeline",
        );
        let (render_pipeline, render_bgl) =
            Self::make_particle_render_pipeline(device, target_format, "constrained_drift");
        let (params_buf, clock_buf, res_buf, ssbo) =
            Self::make_particle_buffers(device, "constrained_drift");
        Self {
            compute_pipeline,
            compute_bgl,
            render_pipeline,
            render_bgl,
            params_buf,
            clock_buf,
            res_buf,
            ssbo,
            write_idx: std::cell::Cell::new(0),
        }
    }

    /// P2.5.3 — Build the `mask_edge_emission` variant.
    pub fn new_edge_emission(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let compute_bgl = Self::make_sdf_compute_bgl(device, "edge_emission compute bgl");
        let compute_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_particles_edge_emission.wgsl")
        );
        let compute_pipeline = Self::make_sdf_compute_pipeline(
            device,
            "fx_particles_edge_emission.wgsl",
            &compute_src,
            &compute_bgl,
            "edge_emission compute pipeline",
        );
        let (render_pipeline, render_bgl) =
            Self::make_particle_render_pipeline(device, target_format, "edge_emission");
        let (params_buf, clock_buf, res_buf, ssbo) =
            Self::make_particle_buffers(device, "edge_emission");
        Self {
            compute_pipeline,
            compute_bgl,
            render_pipeline,
            render_bgl,
            params_buf,
            clock_buf,
            res_buf,
            ssbo,
            write_idx: std::cell::Cell::new(0),
        }
    }

    /// P2.5.4 — Build the `mask_field_flow` variant.
    pub fn new_field_flow(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let compute_bgl = Self::make_sdf_compute_bgl(device, "field_flow compute bgl");
        let compute_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_particles_field_flow.wgsl")
        );
        let compute_pipeline = Self::make_sdf_compute_pipeline(
            device,
            "fx_particles_field_flow.wgsl",
            &compute_src,
            &compute_bgl,
            "field_flow compute pipeline",
        );
        let (render_pipeline, render_bgl) =
            Self::make_particle_render_pipeline(device, target_format, "field_flow");
        let (params_buf, clock_buf, res_buf, ssbo) =
            Self::make_particle_buffers(device, "field_flow");
        Self {
            compute_pipeline,
            compute_bgl,
            render_pipeline,
            render_bgl,
            params_buf,
            clock_buf,
            res_buf,
            ssbo,
            write_idx: std::cell::Cell::new(0),
        }
    }

    /// P2.5.5 — Build the `mask_collision_reflection` variant.
    pub fn new_collision_reflection(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let compute_bgl = Self::make_sdf_compute_bgl(device, "collision_reflection compute bgl");
        let compute_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_particles_collision_reflection.wgsl")
        );
        let compute_pipeline = Self::make_sdf_compute_pipeline(
            device,
            "fx_particles_collision_reflection.wgsl",
            &compute_src,
            &compute_bgl,
            "collision_reflection compute pipeline",
        );
        let (render_pipeline, render_bgl) =
            Self::make_particle_render_pipeline(device, target_format, "collision_reflection");
        let (params_buf, clock_buf, res_buf, ssbo) =
            Self::make_particle_buffers(device, "collision_reflection");
        Self {
            compute_pipeline,
            compute_bgl,
            render_pipeline,
            render_bgl,
            params_buf,
            clock_buf,
            res_buf,
            ssbo,
            write_idx: std::cell::Cell::new(0),
        }
    }

    /// P2.5.2–P2.5.5 — SDF-reading compute dispatch.
    ///
    /// Identical to `dispatch_compute` except it also binds `sdf_view` at
    /// binding 6 in the compute bind group. Must be used with pipelines
    /// built by `new_constrained_drift`, `new_edge_emission`,
    /// `new_field_flow`, or `new_collision_reflection`.
    ///
    /// `n_particles` is clamped to `MAX_PARTICLES` before dispatch.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_compute_with_sdf(
        &self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        n_particles: u32,
        seed: u64,
        clock_secs: f32,
        t_layer_added_secs: f32,
        params: &HashMap<String, f32>,
        sdf_view: &wgpu::TextureView,
    ) {
        let n = n_particles.min(MAX_PARTICLES);
        let t_local = clock_secs - t_layer_added_secs;
        let seed_f = (seed as u32 & 0x7f_ffff) as f32;

        // Upload FxParamsUniform.
        let pu = FxParamsUniform::for_sdf_particle_preset(params);
        let mut params_bytes = [0u8; 32];
        let floats = [
            pu.wavelength,
            pu.speed,
            pu.falloff,
            pu.base_r,
            pu.base_g,
            pu.base_b,
            pu._pad0,
            pu._pad1,
        ];
        for (i, f) in floats.iter().enumerate() {
            params_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.params_buf, 0, &params_bytes);

        // Upload ClockUniform.
        let mut clock_bytes = [0u8; 16];
        clock_bytes[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        clock_bytes[4..8].copy_from_slice(&t_local.to_le_bytes());
        clock_bytes[8..12].copy_from_slice(&seed_f.to_le_bytes());
        clock_bytes[12..16].copy_from_slice(&(n as f32).to_le_bytes());
        queue.write_buffer(&self.clock_buf, 0, &clock_bytes);

        // Compute pass with SDF binding.
        let w_idx = self.write_idx.get();
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sdf particle compute bg"),
            layout: &self.compute_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.clock_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.ssbo[w_idx].as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(sdf_view),
                },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("sdf particle compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let groups = n.div_ceil(64);
            pass.dispatch_workgroups(groups, 1, 1);
        }

        self.write_idx.set(1 - w_idx);
    }

    /// Render particle quads for the `n_particles` most recently computed
    /// positions.  Reads from the SSBO that `dispatch_compute` wrote to.
    ///
    /// Clears `dst` to transparent before drawing so the fx_texture starts
    /// clean each frame (matching the fragment-preset convention).
    pub fn draw_particles(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        n_particles: u32,
        output_size: [u32; 2],
    ) {
        // The write index was already advanced by dispatch_compute; the
        // buffer we want to read is the one we just finished writing,
        // which is now at index (write_idx - 1) mod 2 = 1 - write_idx.
        let read_idx = 1 - self.write_idx.get();
        let n = n_particles.min(MAX_PARTICLES);

        // Upload resolution uniform: .x = width, .y = height.
        let mut res_bytes = [0u8; 16];
        res_bytes[0..4].copy_from_slice(&(output_size[0] as f32).to_le_bytes());
        res_bytes[4..8].copy_from_slice(&(output_size[1] as f32).to_le_bytes());
        queue.write_buffer(&self.res_buf, 0, &res_bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("particles identity render bg"),
            layout: &self.render_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.clock_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.res_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.ssbo[read_idx].as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("particles identity render pass"),
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
            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            // 6 vertices per quad (two triangles), n_particles instances.
            pass.draw(0..6, 0..n);
        }
    }
}
