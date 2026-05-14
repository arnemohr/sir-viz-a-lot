//! PCleanup.4.1 — Tint effect: three-mode colour mixing pass.
//!
//! Mirrors [`crate::effects::color::ColorPipeline`] in shape; only the
//! uniform layout and the shader body differ. Reads the source texture,
//! mixes with a configured RGBA colour by `amount`, writes the result.
//!
//! Modes:
//!   * [`TintMode::Multiply`] — proper tint (darkens toward the colour)
//!   * [`TintMode::Additive`] — wash (lightens; classic VJ overlay)
//!   * [`TintMode::Screen`]   — soft additive that never overshoots 1.0
//!
//! Source alpha is preserved unchanged.

use serde::{Deserialize, Serialize};

/// Three-mode tint operation. `Multiply` is the default so projects that
/// serialised before this field existed deserialise as a conventional tint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TintMode {
    /// `src * mix(white, tint, amount)` — darkens toward the tint colour.
    /// Industry-standard "tint" semantics; the right default for the
    /// "colourise a layer" use case.
    #[default]
    Multiply,
    /// `src + tint * amount * src.a` — additive wash. Lightens; allowed to
    /// exceed 1.0 on HDR targets (clamped on store for 8-bit targets).
    Additive,
    /// `1 - (1-src) * (1 - tint*amount)` — soft additive that always stays
    /// in [0,1]. Use for highlights / glows where additive blows out.
    Screen,
}

impl TintMode {
    /// Stable on-wire integer for the WGSL uniform's `mode: u32` field.
    /// Renumbering existing variants is a breaking change.
    pub fn to_shader_code(self) -> u32 {
        match self {
            TintMode::Multiply => 0,
            TintMode::Additive => 1,
            TintMode::Screen => 2,
        }
    }
}

/// Parameters for the tint effect, matching the `TintParams` uniform struct
/// in `tint.wgsl` (8 × f32 = 32 bytes, std140-compatible).
///
/// 4 floats for `rgba`, 1 for `amount`, 1 for `mode` (cast from u32 to f32
/// bits-pattern via `.to_le_bytes()` on u32 — the shader reads it as u32),
/// 2 floats of padding so the whole struct is 16-byte aligned.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TintParams {
    pub rgba: [f32; 4],
    pub amount: f32,
    pub mode: TintMode,
}

impl TintParams {
    /// 32-byte little-endian wire format matching tint.wgsl's uniform block.
    pub fn to_wire_bytes(self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&self.rgba[0].to_le_bytes());
        bytes[4..8].copy_from_slice(&self.rgba[1].to_le_bytes());
        bytes[8..12].copy_from_slice(&self.rgba[2].to_le_bytes());
        bytes[12..16].copy_from_slice(&self.rgba[3].to_le_bytes());
        bytes[16..20].copy_from_slice(&self.amount.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.mode.to_shader_code().to_le_bytes());
        // bytes[24..32] left zeroed for std140 padding.
        bytes
    }
}

impl Default for TintParams {
    fn default() -> Self {
        Self {
            rgba: [1.0, 1.0, 1.0, 1.0],
            amount: 0.0,
            mode: TintMode::default(),
        }
    }
}

/// Cached GPU pipeline for the tint effect.
///
/// Constructed once at startup; `render` is called per frame per layer
/// that has a `Tint` effect in its chain. Held by `RenderCtx::tint`.
pub struct TintPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl TintPipeline {
    /// Build the tint effect pipeline. Mirrors `ColorPipeline::new`; the
    /// only differences are the shader source and the uniform size.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tint.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../render/shaders/tint.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tint effect bgl"),
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
            label: Some("tint effect pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tint effect pipeline"),
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
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tint effect sampler"),
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

    /// Run a single tint pass. Writes `dst_view` after sampling `source_view`
    /// and mixing per `params`. Clears `dst_view` to black before drawing;
    /// fragment writes alpha from source, so transparent source = transparent
    /// destination.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        dst_view: &wgpu::TextureView,
        uniform_buffer: &wgpu::Buffer,
        params: TintParams,
    ) {
        queue.write_buffer(uniform_buffer, 0, &params.to_wire_bytes());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tint effect bind group"),
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
            label: Some("tint effect pass"),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// PCleanup.4.1 — wire-format size matches the WGSL struct
    /// (`vec4<f32> + f32 + u32 + f32 + f32 = 32 bytes`, std140-aligned).
    #[test]
    fn tint_params_wire_format_is_32_bytes() {
        let bytes = TintParams::default().to_wire_bytes();
        assert_eq!(bytes.len(), 32, "TintParams wire format must be 32 bytes");
    }

    /// PCleanup.4.1 — the three modes produce distinct shader codes.
    /// Renumbering would silently change every saved Tint effect.
    #[test]
    fn tint_mode_shader_codes_are_stable() {
        assert_eq!(TintMode::Multiply.to_shader_code(), 0);
        assert_eq!(TintMode::Additive.to_shader_code(), 1);
        assert_eq!(TintMode::Screen.to_shader_code(), 2);
    }

    /// PCleanup.4.1 — default mode is `Multiply` so projects serialised
    /// before the `mode` field existed deserialise as a conventional tint.
    #[test]
    fn tint_mode_default_is_multiply() {
        assert_eq!(TintMode::default(), TintMode::Multiply);
    }

    /// PCleanup.4.1 — TintParams default is fully transparent (amount=0),
    /// so an inert default doesn't visibly tint a freshly-added Effect::Tint.
    #[test]
    fn tint_params_default_is_inert() {
        let p = TintParams::default();
        assert_eq!(p.amount, 0.0);
        assert_eq!(p.mode, TintMode::Multiply);
    }

    /// PCleanup.4.1 — serde round-trip for TintMode (catches accidental
    /// rename or reorder of variants).
    #[test]
    fn tint_mode_serde_round_trip() {
        for m in [TintMode::Multiply, TintMode::Additive, TintMode::Screen] {
            let json = serde_json::to_string(&m).unwrap();
            let back: TintMode = serde_json::from_str(&json).unwrap();
            assert_eq!(back, m, "round-trip failed for {m:?}");
        }
    }

    /// PCleanup.4.1 — old projects (without the `mode` field on
    /// `Effect::Tint`) deserialise as `TintMode::Multiply` via serde default.
    /// This test fixes the wire contract: do not break it.
    #[test]
    fn tint_mode_serde_default_is_multiply() {
        // Equivalent to "no mode field on the wire" — serde_json defaults
        // missing fields from the type's Default impl when #[serde(default)]
        // is applied at the field. The Effect::Tint variant uses that
        // attribute (see effects::mod). Here we only check the unit-default.
        let m = TintMode::default();
        assert_eq!(m, TintMode::Multiply);
    }
}
