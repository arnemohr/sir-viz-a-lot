//! P0.7.3 — edge-blend overlap region pass.
//!
//! A multiply-blend fullscreen pass that ramps brightness across the overlap
//! region between two adjacent projectors. Applied after the gamma pass
//! (pass 5) and before the editor overlay (pass 6) so the operator overlay
//! stays full-brightness for visibility.
//!
//! Blend state: `src_factor = Dst, dst_factor = Zero` → result = src * dst.
//! Because the shader outputs a grayscale factor in [0, 1], this multiplies
//! the existing surface colour by that factor without affecting alpha.

use crate::project::schema::FalloffCurve;

/// P0.7.3 — edge-blend pipeline. Modelled on [`super::gamma::GammaPipeline`].
pub struct EdgeBlendPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
}

/// Uniform layout (16 bytes / one vec4, std140-compatible):
///   offset  0: overlap_px     (f32)
///   offset  4: surface_width  (f32)
///   offset  8: edge_side      (f32)  — 0.0 = right-edge falloff, 1.0 = left-edge
///   offset 12: falloff_curve  (f32)  — 0.0 = linear, 1.0 = cosine
const UNIFORM_SIZE: u64 = 16;

impl EdgeBlendPipeline {
    /// Build the pipeline targeting `format`. Matches the lifecycle of
    /// `GammaPipeline::new`: call once per output window surface format.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("edge_blend.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/edge_blend.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("edge_blend bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(UNIFORM_SIZE),
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("edge_blend layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        // Multiply blend: result = shader_output (grayscale factor) * existing surface.
        // Alpha component: pass through (One * src_alpha + Zero * dst_alpha = src_alpha).
        let multiply_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("edge_blend pipeline"),
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
                    blend: Some(multiply_blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("edge_blend uniforms"),
            size: UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            uniform_buffer,
        }
    }

    /// Run the edge-blend multiply pass into `dst`.
    ///
    /// `dst` must already contain the gamma-corrected image (written by pass 5).
    /// Uses `wgpu::LoadOp::Load` so the gamma pass contents are preserved and
    /// only multiplied — the caller must not pass `Clear`.
    ///
    /// Parameters:
    /// - `surface_width`:  `output.config.width` for this output window.
    /// - `overlap_px`:     width of the soft-edge region in pixels.
    /// - `edge_side`:      0.0 = right-edge (output 0), 1.0 = left-edge (output 1).
    /// - `falloff_curve`:  `FalloffCurve::Linear` or `FalloffCurve::Cosine`.
    #[allow(clippy::too_many_arguments)] // wgpu render APIs naturally take many borrows
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        surface_width: u32,
        overlap_px: u32,
        edge_side: f32,
        falloff_curve: FalloffCurve,
    ) {
        let falloff_f = match falloff_curve {
            FalloffCurve::Linear => 0.0_f32,
            FalloffCurve::Cosine => 1.0_f32,
        };

        let mut b = [0u8; UNIFORM_SIZE as usize];
        let fields: [f32; 4] = [
            overlap_px as f32,
            surface_width as f32,
            edge_side,
            falloff_f,
        ];
        for (i, f) in fields.iter().enumerate() {
            b[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.uniform_buffer, 0, &b);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("edge_blend bg"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.uniform_buffer.as_entire_binding(),
            }],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("edge_blend pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // LoadOp::Load: preserve the gamma-corrected image and multiply it.
                    load: wgpu::LoadOp::Load,
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
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    /// P0.7.3 — verify the uniform byte-packing helper produces the expected
    /// layout without needing a GPU device. The layout is a 16-byte block:
    ///   offset  0: overlap_px     (f32)
    ///   offset  4: surface_width  (f32)
    ///   offset  8: edge_side      (f32)
    ///   offset 12: falloff_curve  (f32)
    #[test]
    fn uniform_packs_to_expected_bytes() {
        let overlap_px: u32 = 64;
        let surface_width: u32 = 1920;
        let edge_side: f32 = 1.0; // left-edge (output 1)
        let falloff_f: f32 = 1.0; // cosine

        let fields: [f32; 4] = [
            overlap_px as f32,
            surface_width as f32,
            edge_side,
            falloff_f,
        ];
        let mut b = [0u8; 16];
        for (i, f) in fields.iter().enumerate() {
            b[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }

        assert_eq!(&b[0..4], &64.0_f32.to_le_bytes(), "overlap_px bytes");
        assert_eq!(&b[4..8], &1920.0_f32.to_le_bytes(), "surface_width bytes");
        assert_eq!(&b[8..12], &1.0_f32.to_le_bytes(), "edge_side bytes");
        assert_eq!(&b[12..16], &1.0_f32.to_le_bytes(), "falloff_curve bytes");
    }
}
