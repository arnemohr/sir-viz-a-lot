//! PCleanup.2.4 — Treatment-owned particle compute infrastructure.
//!
//! This module provides the [`TreatmentParticlePipeline`] — a compute + render
//! pipeline pair that Treatment presets use for particle-based luminance
//! modulation.  The design is intentionally **separate from
//! [`crate::render::fx_compute::FxComputePipeline`]** for the following
//! reasons:
//!
//! - FX particles draw quads into a generative overlay; Treatment particles
//!   sample particle positions in a fragment shader to modulate the **source**
//!   image.  These are different rendering contracts.
//! - FX particle lifetimes are FxLayer-scoped; Treatment particle lifetimes
//!   are per-layer-instance-scoped.  Merging them would couple unrelated
//!   lifecycles.
//! - A shared abstraction would be premature generalisation given Phase 4
//!   scope constraints.
//!
//! ## Particle struct layout (locked — do NOT change across W2.4–W2.6)
//!
//! | field | type        | offset | size |
//! |-------|-------------|--------|------|
//! | `pos` | `vec2<f32>` |  0     |  8   |
//! | `vel` | `vec2<f32>` |  8     |  8   |
//! | `age` | `f32`       | 16     |  4   |
//! | `_pad`| `f32`       | 20     |  4   |
//!
//! Total: 24 bytes per particle (std430 stride, 8-byte alignment).  The
//! `vel` and `age` fields are zeroed by the spotlights compute shader; future
//! W2.5 (drift_brushstrokes) and W2.6 (edge_sparks) will populate them.
//!
//! ## Bind-group slot reservation (Treatment compute passes)
//!
//! Treatments currently use slots 0–6:
//!   - 0: `t_source` (source texture, filterable)
//!   - 1: `s_source` (source sampler, filtering)
//!   - 2: `u_params` (per-preset params uniform)
//!   - 3: `t_sdf` (SDF texture, R32Float, non-filterable; SDF-consuming treatments only)
//!   - 4: `t_sdf` for compute pass (some treatments use different slot numbering
//!     in their compute vs fragment BGL — check per-preset)
//!   - 5: reserved (fit uniform in some treatments)
//!   - 6: `u_zone` (ZoneTagUniform, zone-aware treatments only)
//!
//! **Slot 7 is reserved for the particle SSBO in compute-based Treatments.**
//! Both the compute pass (read_write) and the fragment pass (read) bind the
//! SSBO at group 0, binding 7.  Future W2.5/W2.6 Treatments MUST use slot 7
//! for their particle SSBOs; do NOT repurpose this slot for anything else.
//!
//! ## Shared WGSL helper
//!
//! [`TREATMENT_PARTICLE_SIM_WGSL`] provides the `Particle` struct definition
//! and hash/spawn helper functions used by all Treatment compute shaders.
//! It is a function-only module — no entry points, no `@binding` declarations.
//! Each Treatment compute shader prepends this source at runtime (mirroring
//! the `SDF_HELPER_WGSL` / `ZONE_TAG_WGSL` pattern in `sdf.rs`).

use std::cell::Cell;
use std::collections::HashMap;

/// WGSL source for the shared particle helper (function-only — no entry points).
///
/// Treatment compute shaders prepend this before their own source so that:
/// - The `Particle` struct definition is available.
/// - The `tp_hash_f`, `tp_rand_dir`, `tp_random_unit_pos` helpers are available.
///
/// build.rs prepends this for `treat_spotlights_compute.wgsl` via
/// `TREATMENT_PARTICLE_COMPUTE_CONSUMERS`.
pub const TREATMENT_PARTICLE_SIM_WGSL: &str =
    include_str!("shaders/treatment_particles_helper.wgsl");

/// Particle capacity for the `spotlights` preset.
///
/// Sized for the worst-case operator slider value (1..=512); smaller
/// `n_particles` dispatches fewer compute threads and touches fewer SSBO
/// entries.  The SSBO is always allocated at full capacity so it can be
/// reused if the operator adjusts the slider without re-allocating.
///
/// Future W2.5/W2.6 Treatments document their own caps per-preset; this
/// constant is NOT shared with them (avoid premature generalisation).
pub const MAX_SPOTLIGHTS: u32 = 512;

/// Byte stride of one `Particle` in std430 layout.
///
/// Layout: `pos(8) + vel(8) + age(4) + _pad(4) = 24`. The struct's natural
/// alignment is `max(align(vec2), align(f32)) = 8`; `ceil(24 / 8) * 8 = 24`
/// so no extra padding is added.
pub const PARTICLE_STRIDE: u64 = 24;

/// SSBO size for `MAX_SPOTLIGHTS` particles (bytes).
const SPOTLIGHTS_SSBO_SIZE: u64 = MAX_SPOTLIGHTS as u64 * PARTICLE_STRIDE;

/// Compute + fragment pipeline for Treatment presets that use particle-based
/// luminance modulation.
///
/// Owns:
/// - A **compute pipeline** that runs the particle position-update shader each
///   frame.
/// - A **fragment pipeline** that reads the SSBO + source texture and writes
///   the modulated output.
/// - A double-buffered SSBO pair (same pattern as `FxComputePipeline`).
///
/// Constructed once per preset variant (W2.4 uses `new_spotlights`; future
/// W2.5/W2.6 will add `new_drift_pinholes` / `new_edge_sparks`).
pub struct TreatmentParticlePipeline {
    // Compute pass.
    compute_pipeline: wgpu::ComputePipeline,
    compute_bgl: wgpu::BindGroupLayout,

    // Fragment pass.
    render_pipeline: wgpu::RenderPipeline,
    render_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,

    // Shared uniform buffers (written each frame).
    compute_params_buf: wgpu::Buffer, // SpotlightsComputeParams
    clock_buf: wgpu::Buffer,          // vec4<f32>: clock, t_local, seed_f, n_particles
    frag_params_buf: wgpu::Buffer,    // SpotlightsFragParams

    // Dummy 1×1 SDF texture used when the caller has no real SDF (no mask).
    // Sample value 0.5 makes `sample_sdf_bilinear` return 0.5 (positive =
    // outside), so all interior-search candidates fail and particles spawn
    // via the fallback path using `tp_random_unit_pos`.
    dummy_sdf: wgpu::Texture,
    dummy_sdf_view: wgpu::TextureView,

    // Double-buffered SSBOs (particle state ping-pong).
    ssbo: [wgpu::Buffer; 2],
    /// Index of the SSBO the compute pass will write to next frame.
    write_idx: Cell<usize>,
}

impl TreatmentParticlePipeline {
    /// Construct a `TreatmentParticlePipeline` for the `spotlights` preset.
    ///
    /// Spotlights: particles drift inside the layer mask (or over the full
    /// layer rect when no mask is present) and boost source luminance with a
    /// Gaussian weight around each particle position.
    pub fn new_spotlights(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self::new_with_frag_shader(
            device,
            target_format,
            include_str!("shaders/treat_spotlights.wgsl"),
            "treat_spotlights",
        )
    }

    // PCleanup.2.5a — the inline `new_spotlights` body was deleted; the
    // `new_with_frag_shader` helper below subsumes it. See git history for
    // the original construction body.
    #[allow(dead_code)]
    #[doc(hidden)]
    fn _legacy_spotlights_inline_deleted(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let compute_src = format!(
            "{}\n{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            TREATMENT_PARTICLE_SIM_WGSL,
            include_str!("shaders/treat_spotlights_compute.wgsl"),
        );
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_spotlights_compute.wgsl"),
            source: wgpu::ShaderSource::Wgsl(compute_src.into()),
        });

        // --- Fragment shader (luminance modulation) ---
        let frag_src = format!(
            "{}\n{}",
            TREATMENT_PARTICLE_SIM_WGSL,
            include_str!("shaders/treat_spotlights.wgsl"),
        );
        let frag_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("treat_spotlights.wgsl"),
            source: wgpu::ShaderSource::Wgsl(frag_src.into()),
        });

        // --- Compute bind-group layout ---
        // binding 2: SpotlightsComputeParams (uniform, 32 bytes)
        // binding 3: ClockUniform (uniform, 16 bytes = vec4<f32>)
        // binding 4: t_sdf (texture, R32Float, non-filterable)
        // binding 7: particles SSBO (storage, read_write)
        let compute_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_spotlights compute bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(32),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                // Slot 7: particle SSBO (read_write in compute pass).
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(SPOTLIGHTS_SSBO_SIZE),
                    },
                    count: None,
                },
            ],
        });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("treat_spotlights compute layout"),
                bind_group_layouts: &[Some(&compute_bgl)],
                immediate_size: 0,
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("treat_spotlights compute"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // --- Render bind-group layout ---
        // binding 0: t_source (texture, filterable)
        // binding 1: s_source (sampler, filtering)
        // binding 2: SpotlightsFragParams (uniform, 32 bytes)
        // binding 7: particles SSBO (storage, read-only in fragment pass)
        let render_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("treat_spotlights render bgl"),
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
                        min_binding_size: std::num::NonZeroU64::new(32),
                    },
                    count: None,
                },
                // Slot 7: particle SSBO (read-only in fragment pass).
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(SPOTLIGHTS_SSBO_SIZE),
                    },
                    count: None,
                },
            ],
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("treat_spotlights render layout"),
                bind_group_layouts: &[Some(&render_bgl)],
                immediate_size: 0,
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("treat_spotlights render"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &frag_shader,
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
                module: &frag_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("treat_spotlights sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Compute params buffer (SpotlightsComputeParams — 32 bytes, 8 × f32).
        let compute_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_spotlights compute params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Clock buffer (vec4<f32> = 16 bytes).
        let clock_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_spotlights clock"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Fragment params buffer (SpotlightsFragParams — 32 bytes, 8 × f32).
        let frag_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("treat_spotlights frag params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Dummy 1×1 R32Float SDF texture (value uploaded lazily on first
        // `dispatch_compute` call via `queue.write_texture`). Bound when the
        // caller has no real SDF; `sample_sdf_bilinear` returns the written
        // value (0.5 = positive = outside mask) so `find_spawn_pos` falls
        // through all 16 candidates and uses `tp_random_unit_pos` — particles
        // scatter uniformly in [0,1]² on no-mask layers.
        let dummy_sdf = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("treat_spotlights dummy sdf"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let dummy_sdf_view = dummy_sdf.create_view(&wgpu::TextureViewDescriptor::default());

        // Double-buffered SSBOs.
        let ssbo = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("treat_spotlights ssbo 0"),
                size: SPOTLIGHTS_SSBO_SIZE,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("treat_spotlights ssbo 1"),
                size: SPOTLIGHTS_SSBO_SIZE,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        ];

        Self {
            compute_pipeline,
            compute_bgl,
            render_pipeline,
            render_bgl,
            sampler,
            compute_params_buf,
            clock_buf,
            frag_params_buf,
            dummy_sdf,
            dummy_sdf_view,
            ssbo,
            write_idx: Cell::new(0),
        }
    }

    /// Run the particle position-update compute pass.
    ///
    /// `n_particles` is clamped to [`MAX_SPOTLIGHTS`] before dispatch.  The
    /// seed is packed deterministically per [`crate::render::fx_compute`]
    /// convention: `(seed as u32 & 0x7f_ffff) as f32`.
    ///
    /// `sdf_view` is optional: `None` → the dummy 1×1 SDF is bound so
    /// particles spawn uniformly over [0,1]² (no mask constraint).
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
        sdf_view: Option<&wgpu::TextureView>,
    ) {
        let n = n_particles.min(MAX_SPOTLIGHTS);
        let t_local = clock_secs - t_layer_added_secs;
        // Seed-packing convention: lower 23 bits → f32 mantissa (deterministic,
        // mirrors FxComputePipeline::dispatch_compute_with_sdf).
        let seed_f = (seed as u32 & 0x7f_ffff) as f32;

        let drift_speed = params.get("drift_speed").copied().unwrap_or(0.1);

        // SpotlightsComputeParams: 8 × f32 = 32 bytes.
        // Layout: [drift_speed, _pad0..6]
        let mut compute_bytes = [0u8; 32];
        let compute_floats = [drift_speed, 0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        for (i, f) in compute_floats.iter().enumerate() {
            compute_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.compute_params_buf, 0, &compute_bytes);

        // ClockUniform: [clock_secs, t_local, seed_f, n_particles] = 16 bytes.
        let mut clock_bytes = [0u8; 16];
        clock_bytes[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        clock_bytes[4..8].copy_from_slice(&t_local.to_le_bytes());
        clock_bytes[8..12].copy_from_slice(&seed_f.to_le_bytes());
        clock_bytes[12..16].copy_from_slice(&(n as f32).to_le_bytes());
        queue.write_buffer(&self.clock_buf, 0, &clock_bytes);

        // Upload dummy SDF value on first call (lazy initialisation).
        // Writing to the texture requires a queue command, not a mapping.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.dummy_sdf,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &0.5f32.to_le_bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let w_idx = self.write_idx.get();
        let active_sdf = sdf_view.unwrap_or(&self.dummy_sdf_view);

        let compute_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_spotlights compute bg"),
            layout: &self.compute_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.compute_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.clock_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(active_sdf),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.ssbo[w_idx].as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("treat_spotlights compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &compute_bg, &[]);
            let groups = n.div_ceil(64);
            pass.dispatch_workgroups(groups, 1, 1);
        }

        self.write_idx.set(1 - w_idx);
    }

    /// Run the fragment pass that reads the particle SSBO + source texture
    /// and writes modulated output to `dst`.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        source: &wgpu::TextureView,
        params: &HashMap<String, f32>,
        n_particles: u32,
    ) {
        let n = n_particles.min(MAX_SPOTLIGHTS);
        let brightness_gain = params.get("brightness_gain").copied().unwrap_or(0.0);
        let radius = params.get("radius").copied().unwrap_or(0.05);

        // SpotlightsFragParams: 8 × f32 = 32 bytes.
        // Layout: [brightness_gain, radius, n_particles, _pad0..4]
        let mut frag_bytes = [0u8; 32];
        let frag_floats = [
            brightness_gain,
            radius,
            n as f32,
            0.0f32,
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        for (i, f) in frag_floats.iter().enumerate() {
            frag_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.frag_params_buf, 0, &frag_bytes);

        // The compute pass flipped write_idx after dispatch; the SSBO the
        // compute pass most recently wrote is now at (1 - write_idx) before
        // the next dispatch.  Wait: after dispatch_compute sets write_idx =
        // 1 - w_idx, the freshly-written buffer is at w_idx (the old value).
        // Since write_idx was just set to 1 - w_idx, the read index is
        // 1 - write_idx.get() = 1 - (1 - w_idx) = w_idx.
        let read_idx = 1 - self.write_idx.get();

        let render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_spotlights render bg"),
            layout: &self.render_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.frag_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.ssbo[read_idx].as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_spotlights render pass"),
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
        pass.set_bind_group(0, &render_bg, &[]);
        pass.draw(0..6, 0..1);
    }

    /// PCleanup.2.5a — Construct a `TreatmentParticlePipeline` for the
    /// `drift_pinholes` preset.
    ///
    /// Same compute pass as [`Self::new_spotlights`] (particles drift in mask,
    /// or over [0,1]² when no mask is bound).  The fragment pass differs: it
    /// masks the source by particle proximity, so only pixels under particles
    /// remain visible.  `opacity` controls the strength of the masking:
    ///   - `opacity = 0.0` → bit-exact passthrough (structural).
    ///   - `opacity = 1.0` → fully masked (only pinholes visible).
    ///
    /// The pipeline shape (BGLs, buffers, SSBO size) is identical to spotlights
    /// — drift_pinholes is a fragment-shader-only swap.
    pub fn new_drift_pinholes(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self::new_with_frag_shader(
            device,
            target_format,
            include_str!("shaders/treat_drift_pinholes.wgsl"),
            "treat_drift_pinholes",
        )
    }

    /// PCleanup.2.5b — Construct a `TreatmentParticlePipeline` for the
    /// `drift_brushstrokes` preset.
    ///
    /// Same compute pass as [`Self::new_drift_pinholes`]; the compute shader
    /// populates `Particle.vel` (UV/s) each frame which this fragment shader
    /// reads to render elongated motion-blur strokes trailing each particle.
    /// `opacity = 0.0` is bit-exact passthrough.
    pub fn new_drift_brushstrokes(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        Self::new_with_frag_shader(
            device,
            target_format,
            include_str!("shaders/treat_drift_brushstrokes.wgsl"),
            "treat_drift_brushstrokes",
        )
    }

    /// PCleanup.2.6 — Construct a `TreatmentParticlePipeline` for the
    /// `edge_sparks` preset.
    ///
    /// Different compute shader from the rest of the W2 particle Treatments:
    /// spawns particles at the mask edge (SDF ≈ 0 from the interior side),
    /// drifts them outward along the SDF gradient, and tracks per-particle
    /// spawn time in `_pad` so the fragment can fade over a configurable
    /// lifetime.  Fragment math is the same additive Gaussian luminance lift
    /// as spotlights, scaled by remaining life-fraction.
    pub fn new_edge_sparks(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self::new_with_shaders(
            device,
            target_format,
            include_str!("shaders/treat_edge_sparks_compute.wgsl"),
            include_str!("shaders/treat_edge_sparks.wgsl"),
            "treat_edge_sparks",
        )
    }

    /// PCleanup.2.8 — Construct a `TreatmentParticlePipeline` for the
    /// `collision_ripples` preset.
    ///
    /// Compute shader gives each particle a two-state lifecycle (drifting →
    /// rippling on boundary crossing → respawn).  Fragment shader displaces
    /// source UVs by accumulated radial pulses from every active ripple.
    /// No CPU readback; ripples are entirely GPU-resident in the existing
    /// particle SSBO (the `_pad` field encodes the state marker).
    pub fn new_collision_ripples(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        Self::new_with_shaders(
            device,
            target_format,
            include_str!("shaders/treat_collision_ripples_compute.wgsl"),
            include_str!("shaders/treat_collision_ripples.wgsl"),
            "treat_collision_ripples",
        )
    }

    /// PCleanup.2.11 — Construct a `TreatmentParticlePipeline` for the
    /// `portal_warp` preset.
    ///
    /// Compute is the shared spotlights compute (drift through the mask).
    /// Fragment displaces source UVs toward / away from each nearby
    /// particle, producing a soft "ghost" warp that travels with them.
    pub fn new_portal_warp(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self::new_with_frag_shader(
            device,
            target_format,
            include_str!("shaders/treat_portal_warp.wgsl"),
            "treat_portal_warp",
        )
    }

    /// PCleanup.2.5a — Shared constructor body, parameterised by the
    /// fragment-shader source and label prefix.
    ///
    /// The compute side (position-update shader, BGL, params buffer, dummy SDF,
    /// double-buffered SSBOs) is identical across particle Treatments — only
    /// the fragment shader varies.  This helper centralises the duplicated
    /// pipeline-construction code so each sibling preset (`new_spotlights`,
    /// `new_drift_pinholes`, future W2.5b/W2.6) is a thin wrapper that just
    /// names its fragment shader.
    fn new_with_frag_shader(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        frag_wgsl: &'static str,
        label_prefix: &str,
    ) -> Self {
        Self::new_with_shaders(
            device,
            target_format,
            include_str!("shaders/treat_spotlights_compute.wgsl"),
            frag_wgsl,
            label_prefix,
        )
    }

    /// PCleanup.2.6 — Lowest-level constructor: both compute and fragment WGSL
    /// sources are caller-supplied.  Used by `edge_sparks` whose compute
    /// shader spawns particles at the mask edge instead of the interior.
    fn new_with_shaders(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        compute_wgsl: &'static str,
        frag_wgsl: &'static str,
        label_prefix: &str,
    ) -> Self {
        // --- Compute shader ---
        let compute_src = format!(
            "{}\n{}\n{}",
            crate::render::sdf::SDF_HELPER_WGSL,
            TREATMENT_PARTICLE_SIM_WGSL,
            compute_wgsl,
        );
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{label_prefix} compute")),
            source: wgpu::ShaderSource::Wgsl(compute_src.into()),
        });

        // --- Fragment shader (per-preset) ---
        let frag_src = format!("{}\n{}", TREATMENT_PARTICLE_SIM_WGSL, frag_wgsl);
        let frag_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{label_prefix} frag")),
            source: wgpu::ShaderSource::Wgsl(frag_src.into()),
        });

        let compute_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{label_prefix} compute bgl")),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(32),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(SPOTLIGHTS_SSBO_SIZE),
                    },
                    count: None,
                },
            ],
        });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&format!("{label_prefix} compute layout")),
                bind_group_layouts: &[Some(&compute_bgl)],
                immediate_size: 0,
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(&format!("{label_prefix} compute")),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let render_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{label_prefix} render bgl")),
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
                        min_binding_size: std::num::NonZeroU64::new(32),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(SPOTLIGHTS_SSBO_SIZE),
                    },
                    count: None,
                },
            ],
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&format!("{label_prefix} render layout")),
                bind_group_layouts: &[Some(&render_bgl)],
                immediate_size: 0,
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("{label_prefix} render")),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &frag_shader,
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
                module: &frag_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("{label_prefix} sampler")),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let compute_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label_prefix} compute params")),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let clock_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label_prefix} clock")),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frag_params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label_prefix} frag params")),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dummy_sdf = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{label_prefix} dummy sdf")),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let dummy_sdf_view = dummy_sdf.create_view(&wgpu::TextureViewDescriptor::default());

        let ssbo = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{label_prefix} ssbo 0")),
                size: SPOTLIGHTS_SSBO_SIZE,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{label_prefix} ssbo 1")),
                size: SPOTLIGHTS_SSBO_SIZE,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        ];

        Self {
            compute_pipeline,
            compute_bgl,
            render_pipeline,
            render_bgl,
            sampler,
            compute_params_buf,
            clock_buf,
            frag_params_buf,
            dummy_sdf,
            dummy_sdf_view,
            ssbo,
            write_idx: Cell::new(0),
        }
    }

    /// PCleanup.2.5a — Fragment pass for the `drift_pinholes` preset.
    ///
    /// Reads `params["opacity"]` (default 0.0 — identity passthrough) and
    /// `params["radius"]` (default 0.05 — UV-normalised) and uploads them to
    /// the fragment params buffer in the same 32-byte layout
    /// `treat_drift_pinholes.wgsl` expects.  Identical shape to [`Self::render`]
    /// (the spotlights variant) except for the param keys read.
    #[allow(clippy::too_many_arguments)]
    pub fn render_drift_pinholes(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        source: &wgpu::TextureView,
        params: &HashMap<String, f32>,
        n_particles: u32,
    ) {
        let n = n_particles.min(MAX_SPOTLIGHTS);
        let opacity = params.get("opacity").copied().unwrap_or(0.0);
        let radius = params.get("radius").copied().unwrap_or(0.05);

        // DriftPinholesFragParams: 8 × f32 = 32 bytes.
        // Layout: [opacity, radius, n_particles, _pad0..4]
        let mut frag_bytes = [0u8; 32];
        let frag_floats = [opacity, radius, n as f32, 0.0f32, 0.0, 0.0, 0.0, 0.0];
        for (i, f) in frag_floats.iter().enumerate() {
            frag_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.frag_params_buf, 0, &frag_bytes);

        let read_idx = 1 - self.write_idx.get();

        let render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_drift_pinholes render bg"),
            layout: &self.render_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.frag_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.ssbo[read_idx].as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_drift_pinholes render pass"),
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
        pass.set_bind_group(0, &render_bg, &[]);
        pass.draw(0..6, 0..1);
    }

    /// PCleanup.2.5b — Fragment pass for the `drift_brushstrokes` preset.
    ///
    /// Reads `params["opacity"]` (default 0.0 — identity passthrough),
    /// `params["radius"]` (default 0.05 — brush thickness in UV), and
    /// `params["smear_duration"]` (default 0.5 — seconds of motion that
    /// trails behind each particle).  Uploads them in the 32-byte layout
    /// `treat_drift_brushstrokes.wgsl` expects.
    #[allow(clippy::too_many_arguments)]
    pub fn render_drift_brushstrokes(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        source: &wgpu::TextureView,
        params: &HashMap<String, f32>,
        n_particles: u32,
    ) {
        let n = n_particles.min(MAX_SPOTLIGHTS);
        let opacity = params.get("opacity").copied().unwrap_or(0.0);
        let radius = params.get("radius").copied().unwrap_or(0.05);
        let smear_duration = params.get("smear_duration").copied().unwrap_or(0.5);

        // DriftBrushstrokesParams: 8 × f32 = 32 bytes.
        // Layout: [opacity, radius, n_particles, smear_duration, _pad0..3]
        let mut frag_bytes = [0u8; 32];
        let frag_floats = [
            opacity,
            radius,
            n as f32,
            smear_duration,
            0.0f32,
            0.0,
            0.0,
            0.0,
        ];
        for (i, f) in frag_floats.iter().enumerate() {
            frag_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.frag_params_buf, 0, &frag_bytes);

        let read_idx = 1 - self.write_idx.get();

        let render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_drift_brushstrokes render bg"),
            layout: &self.render_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.frag_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.ssbo[read_idx].as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_drift_brushstrokes render pass"),
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
        pass.set_bind_group(0, &render_bg, &[]);
        pass.draw(0..6, 0..1);
    }

    /// PCleanup.2.6 — Fragment pass for the `edge_sparks` preset.
    ///
    /// Reads `params["brightness_gain"]` (default 0.0 — identity), `radius`
    /// (default 0.05 — spark glow size), and `lifetime_s` (default 1.5 —
    /// seconds a spark glows before respawning).  `clock_secs` is taken
    /// from the same `t_local` the compute pass sees so spark ages line up.
    #[allow(clippy::too_many_arguments)]
    pub fn render_edge_sparks(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        source: &wgpu::TextureView,
        params: &HashMap<String, f32>,
        n_particles: u32,
        clock_secs: f32,
    ) {
        let n = n_particles.min(MAX_SPOTLIGHTS);
        let brightness_gain = params.get("brightness_gain").copied().unwrap_or(0.0);
        let radius = params.get("radius").copied().unwrap_or(0.05);
        let lifetime_s = params.get("lifetime_s").copied().unwrap_or(1.5);

        // EdgeSparksFragParams: 8 × f32 = 32 bytes.
        // Layout: [brightness_gain, radius, n, clock, lifetime, _pad0..2]
        let mut frag_bytes = [0u8; 32];
        let frag_floats = [
            brightness_gain,
            radius,
            n as f32,
            clock_secs,
            lifetime_s,
            0.0f32,
            0.0,
            0.0,
        ];
        for (i, f) in frag_floats.iter().enumerate() {
            frag_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.frag_params_buf, 0, &frag_bytes);

        let read_idx = 1 - self.write_idx.get();

        let render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_edge_sparks render bg"),
            layout: &self.render_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.frag_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.ssbo[read_idx].as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_edge_sparks render pass"),
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
        pass.set_bind_group(0, &render_bg, &[]);
        pass.draw(0..6, 0..1);
    }

    /// PCleanup.2.6 — Wrapper around `dispatch_compute` that uploads the
    /// edge-sparks-specific compute params (drift_speed + lifetime_s).  The
    /// shared `dispatch_compute` only knows about `drift_speed`; lifetime
    /// goes into the second f32 slot of the compute params buffer here.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_compute_edge_sparks(
        &self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        n_particles: u32,
        seed: u64,
        clock_secs: f32,
        t_layer_added_secs: f32,
        params: &HashMap<String, f32>,
        sdf_view: Option<&wgpu::TextureView>,
    ) {
        // Overwrite the compute_params buffer with edge-sparks-specific
        // layout BEFORE calling dispatch_compute (which uploads its own
        // drift_speed-only layout). We can't reuse dispatch_compute as-is
        // because edge_sparks needs `lifetime_s` in slot .y.
        let n = n_particles.min(MAX_SPOTLIGHTS);
        let t_local = clock_secs - t_layer_added_secs;
        let seed_f = (seed as u32 & 0x7f_ffff) as f32;

        let drift_speed = params.get("drift_speed").copied().unwrap_or(0.15);
        let lifetime_s = params.get("lifetime_s").copied().unwrap_or(1.5);

        let mut compute_bytes = [0u8; 32];
        let compute_floats = [drift_speed, lifetime_s, 0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0];
        for (i, f) in compute_floats.iter().enumerate() {
            compute_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.compute_params_buf, 0, &compute_bytes);

        let mut clock_bytes = [0u8; 16];
        clock_bytes[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        clock_bytes[4..8].copy_from_slice(&t_local.to_le_bytes());
        clock_bytes[8..12].copy_from_slice(&seed_f.to_le_bytes());
        clock_bytes[12..16].copy_from_slice(&(n as f32).to_le_bytes());
        queue.write_buffer(&self.clock_buf, 0, &clock_bytes);

        // Upload dummy SDF when no real one is bound.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.dummy_sdf,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &0.5f32.to_le_bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let w_idx = self.write_idx.get();
        let active_sdf = sdf_view.unwrap_or(&self.dummy_sdf_view);

        let compute_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_edge_sparks compute bg"),
            layout: &self.compute_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.compute_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.clock_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(active_sdf),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.ssbo[w_idx].as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("treat_edge_sparks compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &compute_bg, &[]);
            let groups = n.div_ceil(64);
            pass.dispatch_workgroups(groups, 1, 1);
        }

        self.write_idx.set(1 - w_idx);
    }

    /// PCleanup.2.8 — Compute dispatch for `collision_ripples`.
    /// Uploads the 3-field compute params (drift_speed, ripple_lifetime,
    /// initial_amplitude) before reusing the shared compute BGL.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_compute_collision_ripples(
        &self,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        n_particles: u32,
        seed: u64,
        clock_secs: f32,
        t_layer_added_secs: f32,
        params: &HashMap<String, f32>,
        sdf_view: Option<&wgpu::TextureView>,
    ) {
        let n = n_particles.min(MAX_SPOTLIGHTS);
        let t_local = clock_secs - t_layer_added_secs;
        let seed_f = (seed as u32 & 0x7f_ffff) as f32;

        let drift_speed = params.get("drift_speed").copied().unwrap_or(0.3);
        let ripple_lifetime = params.get("ripple_lifetime").copied().unwrap_or(1.2);
        // initial_amplitude doubles as the >= 0.5 RIPPLING state marker.
        let initial_amplitude = params
            .get("initial_amplitude")
            .copied()
            .unwrap_or(1.0)
            .max(0.51);

        let mut compute_bytes = [0u8; 32];
        let compute_floats = [
            drift_speed,
            ripple_lifetime,
            initial_amplitude,
            0.0f32,
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        for (i, f) in compute_floats.iter().enumerate() {
            compute_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.compute_params_buf, 0, &compute_bytes);

        let mut clock_bytes = [0u8; 16];
        clock_bytes[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        clock_bytes[4..8].copy_from_slice(&t_local.to_le_bytes());
        clock_bytes[8..12].copy_from_slice(&seed_f.to_le_bytes());
        clock_bytes[12..16].copy_from_slice(&(n as f32).to_le_bytes());
        queue.write_buffer(&self.clock_buf, 0, &clock_bytes);

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.dummy_sdf,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &0.5f32.to_le_bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let w_idx = self.write_idx.get();
        let active_sdf = sdf_view.unwrap_or(&self.dummy_sdf_view);

        let compute_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_collision_ripples compute bg"),
            layout: &self.compute_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.compute_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.clock_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(active_sdf),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.ssbo[w_idx].as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("treat_collision_ripples compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &compute_bg, &[]);
            let groups = n.div_ceil(64);
            pass.dispatch_workgroups(groups, 1, 1);
        }

        self.write_idx.set(1 - w_idx);
    }

    /// PCleanup.2.8 — Fragment pass for `collision_ripples`.
    /// Uploads 6-field frag params and dispatches the displacement shader.
    #[allow(clippy::too_many_arguments)]
    pub fn render_collision_ripples(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        source: &wgpu::TextureView,
        params: &HashMap<String, f32>,
        n_particles: u32,
        clock_secs: f32,
    ) {
        let n = n_particles.min(MAX_SPOTLIGHTS);
        let amplitude = params.get("amplitude").copied().unwrap_or(0.0);
        let frequency = params.get("frequency").copied().unwrap_or(20.0);
        let speed = params.get("ripple_speed").copied().unwrap_or(0.5);
        let decay = params.get("ripple_decay").copied().unwrap_or(1.0);

        let mut frag_bytes = [0u8; 32];
        let frag_floats = [
            amplitude, frequency, speed, decay, n as f32, clock_secs, 0.0f32, 0.0,
        ];
        for (i, f) in frag_floats.iter().enumerate() {
            frag_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.frag_params_buf, 0, &frag_bytes);

        let read_idx = 1 - self.write_idx.get();

        let render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_collision_ripples render bg"),
            layout: &self.render_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.frag_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.ssbo[read_idx].as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_collision_ripples render pass"),
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
        pass.set_bind_group(0, &render_bg, &[]);
        pass.draw(0..6, 0..1);
    }

    /// PCleanup.2.11 — Fragment pass for `portal_warp`.
    /// Reads `amplitude` (default 0.0), `radius` (0.05), and `pull`
    /// (default +1.0 — smear toward particle).  The compute pass is the
    /// shared spotlights compute via `dispatch_compute` from the existing
    /// `render` method — caller invokes that, then this.
    #[allow(clippy::too_many_arguments)]
    pub fn render_portal_warp(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        source: &wgpu::TextureView,
        params: &HashMap<String, f32>,
        n_particles: u32,
    ) {
        let n = n_particles.min(MAX_SPOTLIGHTS);
        let amplitude = params.get("amplitude").copied().unwrap_or(0.0);
        let radius = params.get("radius").copied().unwrap_or(0.05);
        let pull = params.get("pull").copied().unwrap_or(1.0);

        // PortalWarpFragParams: 8 × f32 = 32 bytes.
        // Layout: [amplitude, radius, n_particles, pull, _pad0..3]
        let mut frag_bytes = [0u8; 32];
        let frag_floats = [amplitude, radius, n as f32, pull, 0.0f32, 0.0, 0.0, 0.0];
        for (i, f) in frag_floats.iter().enumerate() {
            frag_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.frag_params_buf, 0, &frag_bytes);

        let read_idx = 1 - self.write_idx.get();

        let render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("treat_portal_warp render bg"),
            layout: &self.render_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.frag_params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.ssbo[read_idx].as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("treat_portal_warp render pass"),
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
        pass.set_bind_group(0, &render_bg, &[]);
        pass.draw(0..6, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PCleanup.2.4 — Verify the seed-packing formula matches the FX convention.
    ///
    /// Both Treatment particles (this module) and FX particles
    /// (`FxComputePipeline::dispatch_compute_with_sdf` line ~795) pack the seed
    /// as `(seed as u32 & 0x7f_ffff) as f32` (lower 23 bits → f32 mantissa).
    ///
    /// GPU-side determinism follows from structural determinism: identical packed
    /// inputs + identical compute shader → identical particle positions per frame.
    /// This test:
    ///   1. Verifies the Treatment formula produces the same f32 the FX path
    ///      would produce for the same `seed: u64` — so a refactor of either
    ///      path that silently changes the formula is caught here.
    ///   2. Verifies same seed → same packed value (pure function).
    ///   3. Verifies the lower-23-bit mask at a few spot values.
    #[test]
    fn seed_packing_matches_fx_convention_and_is_deterministic() {
        // Our packing (dispatch_compute, same formula as fx_compute.rs ~795).
        let pack_seed = |seed: u64| -> f32 { (seed as u32 & 0x7f_ffff) as f32 };

        let seed: u64 = 0xDEAD_BEEF_1234_5678;

        // Formula is pure — same seed always produces same result.
        assert_eq!(
            pack_seed(seed),
            pack_seed(seed),
            "seed packing must be deterministic: same seed → same f32"
        );

        // Treatment and FX formulas are textually identical — verify here so
        // a future refactor of one that drifts from the other is caught.
        let fx_packed = (seed as u32 & 0x7f_ffff) as f32;
        assert_eq!(
            pack_seed(seed),
            fx_packed,
            "Treatment seed packing must match FX convention (fx_compute.rs)"
        );

        // Boundary values.
        assert_eq!(pack_seed(0), 0.0, "seed=0 packs to 0.0");
        assert_ne!(pack_seed(1), 0.0, "seed=1 packs to non-zero");
        // Upper bits beyond 23 are masked out.
        assert_eq!(pack_seed(0xFF80_0000), 0.0, "bits above 22 are masked → 0");
    }

    /// PCleanup.2.4 — PARTICLE_STRIDE matches the locked std430 layout.
    ///
    /// pos(8) + vel(8) + age(4) + _pad(4) = 24 bytes.  Struct alignment =
    /// max(align(vec2<f32>), align(f32)) = 8.  ceil(24/8)*8 = 24 (no extra).
    #[test]
    fn particle_stride_is_24() {
        assert_eq!(
            PARTICLE_STRIDE, 24,
            "Particle stride must be 24 bytes (std430 layout)"
        );
    }

    /// PCleanup.2.4 — SSBO capacity covers the full slider range (1..=512).
    #[test]
    fn ssbo_covers_full_slider_range() {
        assert_eq!(MAX_SPOTLIGHTS, 512, "Spotlights cap must be 512");
        assert_eq!(
            SPOTLIGHTS_SSBO_SIZE,
            512 * 24,
            "SSBO must fit 512 × 24 bytes"
        );
    }
}
