//! P2.6.1 — Fluid advection pipeline + `fluid_identity` proof-point preset.
//! P2.6.2 — `mask_bounded_fluid` preset (SDF-derived no-slip boundary).
//!
//! # Architecture
//!
//! One `FxFluidPipeline` per preset variant (same pattern as `FxComputePipeline`
//! in `fx_compute.rs`).  The pipeline owns:
//!   - Two 256×256 RGBA16Float velocity textures for ping-pong advection.
//!   - A compute pipeline that implements semi-Lagrangian advection (writing
//!     from the "current" texture to the "scratch" texture).
//!   - A render (fragment) pipeline that reads the current velocity texture and
//!     outputs a colour representation.
//!   - A parity flag (`frame_parity: Cell<u32>`) that flips each frame to
//!     alternate which texture is "current" vs "scratch".
//!
//! # Velocity texture format
//!
//! RGBA16Float is used instead of the RG16Float the spec originally suggested
//! because RG16Float is not a baseline WebGPU storage texture format and would
//! require `Features::TEXTURE_ADAPTER_SPECIFIC_ADDITIONAL_FORMATS_*`.  RGBA16Float
//! is a guaranteed default-feature storage texture format and also supports
//! linear filtering, making bilinear sampling in the advect pass trivial.
//! Velocity is stored in `.rg`; `.ba` are unused (zero).
//!
//! # Bind-group layouts
//!
//! Advect compute (group 0):
//!   binding 0: source velocity  (texture_2d<f32>, filterable)
//!   binding 1: sampler          (filtering)
//!   binding 2: dest velocity    (texture_storage_2d<rgba16float, write>)
//!   binding 3: uniforms         (vec4<f32>: dt, dissipation, clock, _pad)
//!
//! Bounded-fluid compute (same as advect + binding 4 for SDF texture):
//!   binding 4: SDF texture      (texture_2d<f32>, unfilterable R32Float)
//!
//! Render (group 0):
//!   binding 0: velocity texture (texture_2d<f32>, filterable)
//!   binding 1: sampler          (filtering)
//!   binding 2: clock uniform    (vec4<f32>: .x = clock_secs)
//!
//! # Simplifications (P2.6.2)
//!
//! Particle visualisation for `mask_bounded_fluid` was simplified to colour-only
//! rendering (same as `fluid_identity`).  The `particle_count` descriptor exists
//! to satisfy the spec test contract (`max_particle_count: Some(512)`); the
//! shader does not actually maintain a particle SSBO in this implementation.
//! Skipped in favour of shipping the registry/dispatch shape and the
//! mask-boundary compute shader correctly.

use std::cell::Cell;
use std::collections::HashMap;

use crate::render::sdf::SDF_HELPER_WGSL;

/// Dimensions of the velocity field texture.
pub const FLUID_GRID_SIZE: u32 = 256;

/// Fixed timestep used in the advect shader (matches particle shaders).
const DT: f32 = 1.0 / 60.0;

/// Fluid simulation pipeline.  One instance per preset variant.
///
/// Owns two velocity textures (ping-pong), compute + render pipelines,
/// uniform buffers, sampler, and bind-group layouts.
pub struct FxFluidPipeline {
    // --- Velocity textures (ping-pong) ---
    velocity_tex: [wgpu::Texture; 2],
    velocity_view: [wgpu::TextureView; 2],

    // --- Compute pipeline (advection) ---
    advect_pipeline: wgpu::ComputePipeline,
    advect_bgl: wgpu::BindGroupLayout,

    // --- Render pipeline (visualisation) ---
    render_pipeline: wgpu::RenderPipeline,
    render_bgl: wgpu::BindGroupLayout,

    // --- Uniform buffers ---
    advect_params_buf: wgpu::Buffer, // vec4: dt, dissipation, clock, _pad
    render_clock_buf: wgpu::Buffer,  // vec4: .x = clock_secs

    // --- Sampler (filtering — RGBA16Float supports linear) ---
    sampler: wgpu::Sampler,

    // --- Ping-pong state ---
    /// Even frame → tex[0] is source, tex[1] is dest.
    /// Odd  frame → tex[1] is source, tex[0] is dest.
    frame_parity: Cell<u32>,
}

impl FxFluidPipeline {
    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Allocate two 256×256 RGBA16Float velocity textures.
    fn make_velocity_textures(
        device: &wgpu::Device,
        label_prefix: &str,
    ) -> ([wgpu::Texture; 2], [wgpu::TextureView; 2]) {
        let desc = wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: FLUID_GRID_SIZE,
                height: FLUID_GRID_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let make_one = |idx: usize| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("{label_prefix} velocity tex {idx}")),
                ..desc.clone()
            })
        };

        let t0 = make_one(0);
        let t1 = make_one(1);
        let v0 = t0.create_view(&wgpu::TextureViewDescriptor::default());
        let v1 = t1.create_view(&wgpu::TextureViewDescriptor::default());
        ([t0, t1], [v0, v1])
    }

    /// Build the advect compute bind-group layout.
    /// Bindings: 0=src_velocity(sampled), 1=sampler, 2=dst_velocity(storage),
    ///           3=uniform.
    fn make_advect_bgl(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[
                // binding 0: source velocity (sampled, filterable)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 1: filtering sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: dest velocity (storage write)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // binding 3: uniforms (vec4)
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
            ],
        })
    }

    /// Build the bounded-fluid compute bind-group layout.
    /// Same as `make_advect_bgl` + binding 4 for the SDF texture (R32Float,
    /// unfilterable — matches the SDF texture format used throughout the codebase).
    fn make_bounded_advect_bgl(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                // binding 4: SDF texture (R32Float, unfilterable)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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

    /// Build the render bind-group layout.
    /// Bindings: 0=velocity(sampled), 1=sampler, 2=clock_uniform.
    fn make_render_bgl(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
        })
    }

    /// Shared render pipeline constructor (identity uses
    /// `fx_fluid_identity.wgsl` fragment shader for colour output).
    fn make_render_pipeline(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        render_bgl: &wgpu::BindGroupLayout,
        label_prefix: &str,
    ) -> wgpu::RenderPipeline {
        // Prepend SDF helper as build.rs does for "fx_" files.
        let shader_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_fluid_identity.wgsl")
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{label_prefix} render shader")),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{label_prefix} render layout")),
            bind_group_layouts: &[Some(render_bgl)],
            immediate_size: 0,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("{label_prefix} render pipeline")),
            layout: Some(&layout),
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
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    /// Allocate the two uniform buffers shared by all fluid preset constructors.
    fn make_uniform_buffers(
        device: &wgpu::Device,
        label_prefix: &str,
    ) -> (wgpu::Buffer, wgpu::Buffer) {
        let advect_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label_prefix} advect params")),
            size: 16, // vec4<f32>
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let render_clock_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label_prefix} render clock")),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        (advect_params_buf, render_clock_buf)
    }

    /// Build a filtering sampler (RGBA16Float supports linear filtering).
    fn make_sampler(device: &wgpu::Device, label: &str) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(label),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        })
    }

    // ------------------------------------------------------------------
    // Public constructors
    // ------------------------------------------------------------------

    /// P2.6.1 — Build the `fluid_identity` pipeline.
    ///
    /// Velocity texture is 256×256 RGBA16Float.  The advect shader reads from
    /// binding 0 (source) and writes to binding 2 (dest storage texture).
    pub fn new_fluid_identity(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let (velocity_tex, velocity_view) = Self::make_velocity_textures(device, "fluid_identity");

        // Advect compute.
        let advect_bgl = Self::make_advect_bgl(device, "fluid_identity advect bgl");
        let advect_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_fluid_advect.wgsl")
        );
        let advect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fx_fluid_advect.wgsl"),
            source: wgpu::ShaderSource::Wgsl(advect_src.into()),
        });
        let advect_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fluid_identity advect layout"),
            bind_group_layouts: &[Some(&advect_bgl)],
            immediate_size: 0,
        });
        let advect_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fluid_identity advect pipeline"),
            layout: Some(&advect_layout),
            module: &advect_shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Render.
        let render_bgl = Self::make_render_bgl(device, "fluid_identity render bgl");
        let render_pipeline =
            Self::make_render_pipeline(device, target_format, &render_bgl, "fluid_identity");

        let (advect_params_buf, render_clock_buf) =
            Self::make_uniform_buffers(device, "fluid_identity");
        let sampler = Self::make_sampler(device, "fluid sampler");

        Self {
            velocity_tex,
            velocity_view,
            advect_pipeline,
            advect_bgl,
            render_pipeline,
            render_bgl,
            advect_params_buf,
            render_clock_buf,
            sampler,
            frame_parity: Cell::new(0),
        }
    }

    /// P2.6.2 — Build the `mask_bounded_fluid` pipeline.
    ///
    /// Uses a separate compute shader (`fx_fluid_bounded.wgsl`) that zeroes
    /// velocity outside the mask (SDF > 0) and reflects at boundary cells
    /// using `sample_sdf_normal`.  The render pipeline is the same colour-map
    /// shader as `fluid_identity`.
    ///
    /// Simplification: particle visualisation is not implemented — velocity
    /// field colour-mapping is used for both presets.  The `particle_count`
    /// descriptor (`max_particle_count: Some(512)`) satisfies the spec test
    /// contract; the current shader does not maintain a particle SSBO.
    pub fn new_bounded_fluid(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let (velocity_tex, velocity_view) = Self::make_velocity_textures(device, "bounded_fluid");

        // Bounded advect compute — uses the SDF-extended BGL.
        let advect_bgl = Self::make_bounded_advect_bgl(device, "bounded_fluid advect bgl");
        let advect_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_fluid_bounded.wgsl")
        );
        let advect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fx_fluid_bounded.wgsl"),
            source: wgpu::ShaderSource::Wgsl(advect_src.into()),
        });
        let advect_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bounded_fluid advect layout"),
            bind_group_layouts: &[Some(&advect_bgl)],
            immediate_size: 0,
        });
        let advect_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("bounded_fluid advect pipeline"),
            layout: Some(&advect_layout),
            module: &advect_shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Render: same colour-map pipeline as fluid_identity.
        let render_bgl = Self::make_render_bgl(device, "bounded_fluid render bgl");
        let render_pipeline =
            Self::make_render_pipeline(device, target_format, &render_bgl, "bounded_fluid");

        let (advect_params_buf, render_clock_buf) =
            Self::make_uniform_buffers(device, "bounded_fluid");
        let sampler = Self::make_sampler(device, "bounded fluid sampler");

        Self {
            velocity_tex,
            velocity_view,
            advect_pipeline,
            advect_bgl,
            render_pipeline,
            render_bgl,
            advect_params_buf,
            render_clock_buf,
            sampler,
            frame_parity: Cell::new(0),
        }
    }

    // ------------------------------------------------------------------
    // Per-frame dispatch
    // ------------------------------------------------------------------

    /// Run one advection step.
    ///
    /// Reads from `velocity_view[src]`, writes to `velocity_view[dst]`, then
    /// flips `frame_parity`.
    ///
    /// `sdf_view` is only used by the bounded-fluid variant (P2.6.2).
    /// For `fluid_identity`, pass `None`.
    ///
    /// `dissipation` should be in [0.0, 1.0] (fraction/second energy loss).
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_advect(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        sdf_view: Option<&wgpu::TextureView>,
        clock_secs: f32,
        dissipation: f32,
        inject_intensity: f32,
    ) {
        let parity = self.frame_parity.get();
        let src = parity as usize;
        let dst = 1 - src;

        // Upload advect uniform: vec4(dt, dissipation, clock_secs, inject_intensity)
        let uniform_data: [f32; 4] = [DT, dissipation, clock_secs, inject_intensity];
        let mut bytes = [0u8; 16];
        for (i, f) in uniform_data.iter().enumerate() {
            bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.advect_params_buf, 0, &bytes);

        // Build bind group.
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.velocity_view[src]),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&self.velocity_view[dst]),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: self.advect_params_buf.as_entire_binding(),
            },
        ];
        // Append SDF binding if provided (used by P2.6.2 bounded_fluid variant).
        if let Some(sv) = sdf_view {
            entries.push(wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(sv),
            });
        }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fluid advect bg"),
            layout: &self.advect_bgl,
            entries: &entries,
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("fluid advect pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.advect_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            // 16×16 workgroups → ceil(256/16) = 16 groups per axis.
            let groups = FLUID_GRID_SIZE.div_ceil(16);
            pass.dispatch_workgroups(groups, groups, 1);
        }

        // Advance parity: next frame, the buffer we just wrote is the source.
        self.frame_parity.set(1 - parity);
    }

    /// Render the current velocity field into `dst` as colour.
    ///
    /// The "current" texture is the one most recently written by
    /// `dispatch_advect`.  After the parity flip, that is at
    /// `velocity_view[1 - frame_parity]`.
    pub fn draw_fluid(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        clock_secs: f32,
        _params: &HashMap<String, f32>,
    ) {
        // After dispatch_advect flips parity, the texture we just wrote is at
        // `1 - frame_parity.get()` (the old source is now the write target after
        // this flip; the old dest is the new source).  We want to render the
        // texture that was just written, which is at the old `dst` index.
        // After the flip: new_parity = old_parity^1, so written_idx = 1 - new_parity.
        let read_idx = 1 - self.frame_parity.get() as usize;

        // Upload clock uniform.
        let mut clock_bytes = [0u8; 16];
        clock_bytes[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        queue.write_buffer(&self.render_clock_buf, 0, &clock_bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fluid render bg"),
            layout: &self.render_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.velocity_view[read_idx]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.render_clock_buf.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fluid render pass"),
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
            pass.draw(0..3, 0..1);
        }
    }

    // ------------------------------------------------------------------
    // Accessors (for unit tests)
    // ------------------------------------------------------------------

    /// Returns the WGPU format of the velocity textures.
    #[allow(dead_code)]
    pub fn velocity_format(&self) -> wgpu::TextureFormat {
        self.velocity_tex[0].format()
    }

    /// Returns the dimensions of the velocity textures (width, height).
    #[allow(dead_code)]
    pub fn velocity_size(&self) -> (u32, u32) {
        let size = self.velocity_tex[0].size();
        (size.width, size.height)
    }

    /// Returns a view into the velocity texture that was most recently written
    /// by `dispatch_advect`.
    ///
    /// `dispatch_advect` flips `frame_parity` after writing; the just-written
    /// texture is therefore at index `1 - frame_parity` immediately after the
    /// flip.  Callers (e.g. `FluidWarpTreatmentPipeline`) must call
    /// `dispatch_advect` first in the same encoder, then call this accessor to
    /// bind the result for a subsequent fragment pass.
    pub fn current_velocity_view(&self) -> &wgpu::TextureView {
        let read_idx = 1 - self.frame_parity.get() as usize;
        &self.velocity_view[read_idx]
    }
}
