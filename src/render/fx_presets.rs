//! P0.5.3 — FX preset registry + per-preset render pipeline.
//!
//! A preset is `(preset_id, shader source, default params, pipeline)`.
//! v0.4 ships one preset (`"mask_edge_ripple_wash"`) as the proof point.
//! Phase 2 will grow the registry into the full FX library.
//!
//! # Rendering
//!
//! Each FxLayer owns an `fx_texture` (output-sized, same format as the
//! surface swapchain — matches every other intermediate texture in the
//! pipeline). At per-frame render time, the preset's pipeline draws a
//! fullscreen triangle into `fx_texture`, reading the layer's SDF (via
//! `WarpRenderer::sdf_view`) and the current clock time.
//!
//! # Parameter uniform
//!
//! A fixed-shape struct, `FxParamsUniform`. Each preset documents which
//! fields it reads; unmapped fields stay zero. The fixed layout keeps the
//! shader bind-group layout stable across presets so adding a preset
//! doesn't churn the pipeline plumbing.
//!
//! # Future presets
//!
//! When Phase 2 adds more presets, each preset will get its own pipeline
//! (separate `new_<preset>` constructor); the `FxPresetPipeline` struct
//! stays the same shape — only the shader source and default params
//! differ. An FxLayer that carries an unknown `preset_id` is left
//! invisible (the per-frame loop skips it); the audit emits a warning
//! (wired in P0.5.1).
//!
//! # GPU tests (TODO)
//!
//! A golden-image test (`--features gpu-tests`) for the ripple-wash
//! preset rendered against a fixture polygon mask is deferred — it needs
//! a real wgpu adapter, which CI does not currently provide for the
//! `gpu-tests` feature path. Add it in Phase 2 alongside the golden
//! baseline image. See `tests/headless_gpu.rs` for the test harness.

use std::collections::HashMap;

use crate::render::sdf::SDF_HELPER_WGSL;

/// Preset id for the mask-edge ripple wash effect.
pub const RIPPLE_WASH_PRESET_ID: &str = "mask_edge_ripple_wash";

/// Rendered by `FxPresetPipeline::render` into an output-sized texture.
/// The texture then flows through the layer's normal effect chain + warp
/// pipeline, so Color / Blur / Transform effects and masking work
/// unchanged against FxLayer output.
pub struct FxPresetPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
    clock_buf: wgpu::Buffer,
}

impl FxPresetPipeline {
    /// Build the ripple-wash pipeline.
    ///
    /// `target_format` must match the output surface format so the fx
    /// texture blends correctly into the effect chain. v0.4 ships only
    /// this preset; Phase 2 generalises into a registry indexed by
    /// `preset_id`.
    pub fn new_ripple_wash(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // Concatenate the SDF helper at the front, as warp.rs does for
        // warp.wgsl. build.rs replicates this same concatenation for
        // compile-time naga validation (SDF_CONSUMERS includes "fx_").
        let shader_src = format!(
            "{}\n{}",
            SDF_HELPER_WGSL,
            include_str!("shaders/fx_ripple_wash.wgsl")
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fx_ripple_wash.wgsl"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fx ripple wash bgl"),
            entries: &[
                // binding 0: SDF texture (R32Float, unfilterable)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 1: sampler (NonFiltering — R32Float requires it).
                // Kept for bind-group layout symmetry with future presets;
                // the helper functions use textureLoad, so this is not
                // consumed at runtime by this shader.
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // binding 2: FxParams uniform
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
                // binding 3: clock uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
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
            label: Some("fx ripple wash pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fx ripple wash pipeline"),
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
                    format: target_format,
                    // Premultiplied alpha blend so the transparent outer
                    // region doesn't clobber what's behind the FxLayer.
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fx ripple wash sampler"),
            // NonFiltering to match R32Float's constraint.
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Params buffer: 32 bytes (8 × f32). Written each frame via
        // queue.write_buffer before the render pass.
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fx ripple wash params"),
            size: std::mem::size_of::<FxParamsUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Clock buffer: 16 bytes (vec4<f32>; only .x used).
        let clock_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fx ripple wash clock"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buf,
            clock_buf,
        }
    }

    /// Render into `dst` using `sdf_view` and the preset's params.
    ///
    /// `clock_secs` is the per-frame elapsed-time scalar (from
    /// `Clock::elapsed().as_secs_f32()`).
    ///
    /// The caller is responsible for calling `sync_mesh_and_mask` on the
    /// layer's `WarpRenderer` *before* this call so the SDF is up to date.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        sdf_view: &wgpu::TextureView,
        clock_secs: f32,
        params: &FxParamsUniform,
    ) {
        // Upload params uniform: 8 × f32 = 32 bytes, little-endian.
        let mut params_bytes = [0u8; 32];
        let floats = [
            params.wavelength,
            params.speed,
            params.falloff,
            params.base_r,
            params.base_g,
            params.base_b,
            params._pad0,
            params._pad1,
        ];
        for (i, f) in floats.iter().enumerate() {
            params_bytes[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.params_buf, 0, &params_bytes);

        // Upload clock uniform: vec4<f32> with .x = clock_secs, rest zero.
        let mut clock_bytes = [0u8; 16];
        clock_bytes[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        queue.write_buffer(&self.clock_buf, 0, &clock_bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fx ripple wash bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(sdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.clock_buf.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fx ripple wash pass"),
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
            // Fullscreen triangle: 3 vertices, 1 instance.
            pass.draw(0..3, 0..1);
        }
    }
}

/// Fixed-shape parameter struct for FX presets. Each preset documents which
/// fields it reads. Layout pinned at 32 bytes (8 floats). Add fields as the
/// registry grows; existing layouts stay stable by appending.
///
/// # `mask_edge_ripple_wash` field mapping
///
/// | field      | description                          | default |
/// |------------|--------------------------------------|---------|
/// | wavelength | ripple wavelength in normalised units | 40.0    |
/// | speed      | animation speed (cycles/sec)          | 2.0     |
/// | falloff    | exp-falloff distance (normalised)     | 0.08    |
/// | base_r     | base colour red channel               | 0.4     |
/// | base_g     | base colour green channel             | 0.6     |
/// | base_b     | base colour blue channel              | 1.0     |
/// | _pad0      | reserved                              | 0.0     |
/// | _pad1      | reserved                              | 0.0     |
#[derive(Debug, Clone, Copy, Default)]
pub struct FxParamsUniform {
    pub wavelength: f32,
    pub speed: f32,
    pub falloff: f32,
    pub base_r: f32,
    pub base_g: f32,
    pub base_b: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

impl FxParamsUniform {
    /// Build from the `LayerKind::FxLayer.params` HashMap, defaulting
    /// any missing key to a sensible value for the ripple-wash preset.
    ///
    /// All defaults are documented in the struct-level doc table above.
    pub fn for_ripple_wash(params: &HashMap<String, f32>) -> Self {
        Self {
            wavelength: params.get("wavelength").copied().unwrap_or(40.0),
            speed: params.get("speed").copied().unwrap_or(2.0),
            falloff: params.get("falloff").copied().unwrap_or(0.08),
            base_r: params.get("base_r").copied().unwrap_or(0.4),
            base_g: params.get("base_g").copied().unwrap_or(0.6),
            base_b: params.get("base_b").copied().unwrap_or(1.0),
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P0.5.3 acceptance: FxParamsUniform::for_ripple_wash returns documented
    /// defaults when the params map is empty.
    #[test]
    fn ripple_wash_defaults_from_empty_map() {
        let params = FxParamsUniform::for_ripple_wash(&HashMap::new());
        assert_eq!(params.wavelength, 40.0, "wavelength default");
        assert_eq!(params.speed, 2.0, "speed default");
        assert_eq!(params.falloff, 0.08, "falloff default");
        assert_eq!(params.base_r, 0.4, "base_r default");
        assert_eq!(params.base_g, 0.6, "base_g default");
        assert_eq!(params.base_b, 1.0, "base_b default");
        assert_eq!(params._pad0, 0.0, "_pad0 must be zero");
        assert_eq!(params._pad1, 0.0, "_pad1 must be zero");
    }

    /// P0.5.3 acceptance: all keys populated → values round-trip correctly.
    #[test]
    fn ripple_wash_all_keys_populated() {
        let mut map = HashMap::new();
        map.insert("wavelength".into(), 20.0_f32);
        map.insert("speed".into(), 5.0_f32);
        map.insert("falloff".into(), 0.15_f32);
        map.insert("base_r".into(), 1.0_f32);
        map.insert("base_g".into(), 0.5_f32);
        map.insert("base_b".into(), 0.2_f32);

        let params = FxParamsUniform::for_ripple_wash(&map);
        assert_eq!(params.wavelength, 20.0);
        assert_eq!(params.speed, 5.0);
        assert_eq!(params.falloff, 0.15);
        assert_eq!(params.base_r, 1.0);
        assert_eq!(params.base_g, 0.5);
        assert_eq!(params.base_b, 0.2);
        assert_eq!(params._pad0, 0.0, "_pad0 must still be zero");
        assert_eq!(params._pad1, 0.0, "_pad1 must still be zero");
    }
}
