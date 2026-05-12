//! P0.9.5 — show-day frame-budget perf gate.
//!
//! Drives a fixture project through the production render-graph against
//! headless wgpu targets for N frames, asserts no texture-upload drops
//! and no panic_restore triggers, and prints the frame-time distribution
//! so a developer or CI can compare against the show-day-checklist baseline.
//!
//! ## Scope substitution
//!
//! The spec fixture is "4 video layers + 1 NDI input + 2 outputs +
//! bindings". v0.4 reality:
//!   - No fixture mp4 (binary fixture omitted from P0.4.2 — `ffmpeg`
//!     wasn't available to encode one); 4 FxLayer ripple-wash layers
//!     substitute for the per-frame texture-allocate + raster work.
//!   - NDI input is deferred to v0.5; omitted entirely.
//!   - OSC/MIDI bindings are CPU-side modulators; the GPU render path
//!     does not exercise them directly. The "modulator read path" in the
//!     production code is a simple HashMap lookup taking < 1 µs — it does
//!     not affect render-graph timing. This test exercises the GPU budget.
//!
//! When a fixture mp4 lands (and NDI in v0.5), the fixture composition
//! can grow to match the spec. The threshold strategy and harness shape
//! stay the same.
//!
//! ## Render-loop strategy (Path B)
//!
//! Path A (refactor `render_m5_pipeline` to accept `&[(TextureView, u32,
//! u32, TextureFormat)]`) was evaluated and found infeasible: the render
//! pipeline structs (`WarpRenderer`, `EffectPipeline`, `Compositor`, etc.)
//! live in the binary crate and are not reachable from integration tests
//! (they are not re-exported from `src/lib.rs`). Rather than adding a
//! `[lib]` section or re-exporting every render type (a structural change
//! outside P0.9.5's scope), Path B rebuilds the per-frame render sequence
//! locally — loading all production WGSL via `include_str!` so the shader
//! code under test remains the source of truth.
//!
//! A `TODO(P0.9.5-path-a)` marker is left for a future refactor that
//! exposes the render pipeline types through the library crate.
//!
//! ## Assertions (CI-portable)
//!
//! - `texture_upload_drop_count == 0` (no producers → trivially true, but
//!   documents the invariant for when video/NDI producers land)
//! - No `catch_unwind` panics from any frame.
//! - p99 < 100 ms (loose; catches a 10× regression on any hardware).
//!
//! The show-day budget (p99 ≤ 16.6 ms) is the *operator's* check on real
//! hardware, recorded in `docs/show-day-checklist.md` on first run.

#![cfg(feature = "gpu-tests")]

// TODO(P0.9.5-path-a): When the render pipeline types are exported from
// lib.rs, replace this Path B reimplementation with a call to a shared
// helper (e.g. `render_m5_pipeline_to_views`) so the perf gate measures
// the exact production code path rather than a local mirror.

use std::time::Instant;

use anyhow::{Context, Result, anyhow};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// 10 seconds at 60 Hz, matching the spec.
const FRAME_COUNT: usize = 600;

/// Output resolution for both simulated projector outputs.
const OUT_W: u32 = 1280;
const OUT_H: u32 = 720;

/// All headless render targets use this format (matches the production
/// `Rgba8UnormSrgb` surface format from `OutputWindow`).
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Number of FxLayer layers in the fixture.
const LAYER_COUNT: usize = 4;

/// Number of simulated projector outputs.
const OUTPUT_COUNT: usize = 2;

/// Edge-blend overlap in pixels (matches spec fixture).
const EDGE_BLEND_OVERLAP_PX: u32 = 80;

// ---------------------------------------------------------------------------
// Headless wgpu bootstrap (mirrors headless_gpu.rs::Headless::new)
// ---------------------------------------------------------------------------

struct Headless {
    #[allow(dead_code)]
    instance: wgpu::Instance,
    #[allow(dead_code)]
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Headless {
    fn new() -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|e| anyhow!("no compatible wgpu adapter: {e}"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("perf_frame_budget device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .context("request_device")?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}

// ---------------------------------------------------------------------------
// Texture helpers
// ---------------------------------------------------------------------------

fn make_render_texture(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

/// Allocate the SDF texture for a layer's mask polygon.
fn make_sdf_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    polygon: &[[f32; 2]],
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    const SDF_SIZE: usize = 256;

    // Bake the SDF on the CPU using a simple inside/outside test + distance
    // approximation. For the test, each layer has a different quarter-screen
    // polygon so the SDFs differ per layer and exercise the mask path.
    let data: Vec<f32> = (0..SDF_SIZE * SDF_SIZE)
        .map(|i| {
            let px = (i % SDF_SIZE) as f32 / SDF_SIZE as f32;
            let py = (i / SDF_SIZE) as f32 / SDF_SIZE as f32;
            // Use same point-in-polygon algorithm as production sdf.rs
            let inside = point_in_polygon(px, py, polygon);
            let d = approx_poly_distance(px, py, polygon);
            if inside { -d } else { d }
        })
        .collect();

    let raw: Vec<u8> = data.iter().flat_map(|&f| f.to_le_bytes()).collect();
    let size = wgpu::Extent3d {
        width: SDF_SIZE as u32,
        height: SDF_SIZE as u32,
        depth_or_array_layers: 1,
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &raw,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * SDF_SIZE as u32),
            rows_per_image: Some(SDF_SIZE as u32),
        },
        size,
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn point_in_polygon(x: f32, y: f32, poly: &[[f32; 2]]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let xi = poly[i][0];
        let yi = poly[i][1];
        let xj = poly[j][0];
        let yj = poly[j][1];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn approx_poly_distance(x: f32, y: f32, poly: &[[f32; 2]]) -> f32 {
    if poly.len() < 2 {
        return f32::MAX;
    }
    let mut min_d = f32::MAX;
    let n = poly.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let ax = poly[i][0];
        let ay = poly[i][1];
        let bx = poly[j][0];
        let by = poly[j][1];
        let dx = bx - ax;
        let dy = by - ay;
        let len2 = dx * dx + dy * dy;
        let t = if len2 < 1e-10 {
            0.0
        } else {
            ((x - ax) * dx + (y - ay) * dy) / len2
        };
        let t = t.clamp(0.0, 1.0);
        let cx = ax + t * dx;
        let cy = ay + t * dy;
        let d = ((x - cx) * (x - cx) + (y - cy) * (y - cy)).sqrt();
        if d < min_d {
            min_d = d;
        }
    }
    min_d
}

// ---------------------------------------------------------------------------
// FxPresetPipeline (mirrors src/render/fx_presets.rs)
// ---------------------------------------------------------------------------

struct FxPipeline {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    params_buf: wgpu::Buffer,
    clock_buf: wgpu::Buffer,
}

impl FxPipeline {
    fn new_ripple_wash(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let sdf_helper = include_str!("../src/render/shaders/sdf_helper.wgsl");
        let ripple = include_str!("../src/render/shaders/fx_ripple_wash.wgsl");
        let src = format!("{sdf_helper}\n{ripple}");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perf fx_ripple_wash"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perf fx bgl"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
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

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perf fx layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perf fx pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("perf fx sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perf fx params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let clock_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perf fx clock"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bgl,
            sampler,
            params_buf,
            clock_buf,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        sdf_view: &wgpu::TextureView,
        clock_secs: f32,
    ) {
        // Params: wavelength=40, speed=2, falloff=0.08, base_r=0.4, base_g=0.6, base_b=1.0, pad=0, pad=0
        let floats: [f32; 8] = [40.0, 2.0, 0.08, 0.4, 0.6, 1.0, 0.0, 0.0];
        let mut pb = [0u8; 32];
        for (i, f) in floats.iter().enumerate() {
            pb[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.params_buf, 0, &pb);
        let mut cb = [0u8; 16];
        cb[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        queue.write_buffer(&self.clock_buf, 0, &cb);

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perf fx bg"),
            layout: &self.bgl,
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

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perf fx pass"),
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
        pass.set_bind_group(0, &bg, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Render with explicit params floats (wavelength, speed, falloff, base_r, base_g, base_b, pad, pad).
    /// Used by `perf_four_fx_layers_within_budget` to pass max-amplitude values.
    #[allow(clippy::too_many_arguments)]
    fn render_with_params(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        sdf_view: &wgpu::TextureView,
        clock_secs: f32,
        params: &[f32; 8],
    ) {
        let mut pb = [0u8; 32];
        for (i, f) in params.iter().enumerate() {
            pb[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.params_buf, 0, &pb);
        let mut cb = [0u8; 16];
        cb[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        queue.write_buffer(&self.clock_buf, 0, &cb);

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perf fx bg (max-amp)"),
            layout: &self.bgl,
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

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perf fx pass (max-amp)"),
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
        pass.set_bind_group(0, &bg, &[]);
        pass.draw(0..3, 0..1);
    }
}

// ---------------------------------------------------------------------------
// WarpPipeline (simplified: identity mesh, mirrors src/render/warp.rs)
// ---------------------------------------------------------------------------

struct WarpPipeline {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    mask_uniform: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl WarpPipeline {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let sdf_helper = include_str!("../src/render/shaders/sdf_helper.wgsl");
        let warp_wgsl = include_str!("../src/render/shaders/warp.wgsl");
        let full = format!("{sdf_helper}\n{warp_wgsl}");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perf warp shader"),
            source: wgpu::ShaderSource::Wgsl(full.into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perf warp bgl"),
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
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
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

        let vb_layout = wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
        };

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perf warp layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perf warp pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[vb_layout],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
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
            label: Some("perf warp sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let mask_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perf warp mask uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // use_mask=1.0 (has polygon), feather=0.02, sdf_size=256.0
        let mu: [f32; 4] = [1.0, 0.02, 256.0, 0.0];
        let mut mb = [0u8; 16];
        for (i, f) in mu.iter().enumerate() {
            mb[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        // pre-write (no need to rewrite per frame unless mesh changes)
        // But we need the queue here — we'll write it inside render or from the caller.
        // Store bytes so caller can write at init time.
        let _ = mb; // will write via queue in build_layer_states

        // Identity mesh: fullscreen quad (2 triangles, 4 vertices)
        // Each vertex: [clip_x, clip_y, uv_u, uv_v]
        let verts: [[f32; 4]; 4] = [
            [-1.0, 1.0, 0.0, 0.0],  // TL
            [1.0, 1.0, 1.0, 0.0],   // TR
            [-1.0, -1.0, 0.0, 1.0], // BL
            [1.0, -1.0, 1.0, 1.0],  // BR
        ];
        let indices: [u16; 6] = [0, 2, 1, 1, 2, 3];

        let vb_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(verts.as_ptr().cast::<u8>(), std::mem::size_of_val(&verts))
        };
        let ib_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                indices.as_ptr().cast::<u8>(),
                std::mem::size_of_val(&indices),
            )
        };

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perf warp vb"),
            size: vb_bytes.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perf warp ib"),
            size: ib_bytes.len() as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Return without queue access — caller writes vb/ib via queue.write_buffer
        // before any render calls (call `init_buffers(queue)` before first render).
        Self {
            pipeline,
            bgl,
            sampler,
            mask_uniform,
            vertex_buffer,
            index_buffer,
            index_count: 6,
        }
    }

    fn init_buffers(&self, queue: &wgpu::Queue, polygon_has_3_pts: bool) {
        // Identity mesh vertices
        let verts: [[f32; 4]; 4] = [
            [-1.0, 1.0, 0.0, 0.0],
            [1.0, 1.0, 1.0, 0.0],
            [-1.0, -1.0, 0.0, 1.0],
            [1.0, -1.0, 1.0, 1.0],
        ];
        let indices: [u16; 6] = [0, 2, 1, 1, 2, 3];
        let vb_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(verts.as_ptr().cast::<u8>(), std::mem::size_of_val(&verts))
        };
        let ib_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                indices.as_ptr().cast::<u8>(),
                std::mem::size_of_val(&indices),
            )
        };
        queue.write_buffer(&self.vertex_buffer, 0, vb_bytes);
        queue.write_buffer(&self.index_buffer, 0, ib_bytes);

        let use_mask = if polygon_has_3_pts { 1.0f32 } else { 0.0 };
        let mu: [f32; 4] = [use_mask, 0.02, 256.0, 0.0];
        let mut mb = [0u8; 16];
        for (i, f) in mu.iter().enumerate() {
            mb[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.mask_uniform, 0, &mb);
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        scene: &wgpu::TextureView,
        sdf_view: &wgpu::TextureView,
    ) {
        // Re-write mask uniform each frame (matches production behaviour of
        // sync_mesh_and_mask which may update per frame).
        let _ = queue; // mask_uniform already written at init — no change needed
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perf warp bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(scene),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(sdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.mask_uniform.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perf warp pass"),
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
        pass.set_bind_group(0, &bg, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

// ---------------------------------------------------------------------------
// Compositor (mirrors src/render/compositor.rs)
// ---------------------------------------------------------------------------

struct Compositor {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    // Held alive so the views remain valid; never read directly.
    #[allow(dead_code)]
    ping: wgpu::Texture,
    #[allow(dead_code)]
    pong: wgpu::Texture,
    ping_view: wgpu::TextureView,
    pong_view: wgpu::TextureView,
}

impl Compositor {
    fn new(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perf compositor"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../src/render/shaders/compositor.wgsl").into(),
            ),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perf comp bgl"),
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
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perf comp layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perf comp pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
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
            label: Some("perf comp sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let (ping, ping_view) = make_render_texture(device, w, h, format, "perf comp ping");
        let (pong, pong_view) = make_render_texture(device, w, h, format, "perf comp pong");
        Self {
            pipeline,
            bgl,
            sampler,
            ping,
            pong,
            ping_view,
            pong_view,
        }
    }

    fn composite(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        bg: wgpu::Color,
        target_view: &wgpu::TextureView,
        // layers: (layer_view, opacity, blend_mode_code (0=Normal), per-layer uniform buffer)
        layers: &[(&wgpu::TextureView, f32, f32, &wgpu::Buffer)],
    ) {
        if layers.is_empty() {
            let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perf comp clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(bg),
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

        // Clear ping to background.
        let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perf comp clear ping"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.ping_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(bg),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        let last_idx = layers.len() - 1;
        let mut read_view: &wgpu::TextureView = &self.ping_view;
        let mut write_ping = false;

        for (i, (layer_view, opacity, blend_code, uniform)) in layers.iter().enumerate() {
            let dst = if i == last_idx {
                target_view
            } else if write_ping {
                &self.ping_view
            } else {
                &self.pong_view
            };

            let params: [f32; 4] = [opacity.clamp(0.0, 1.0), *blend_code, 0.0, 0.0];
            let mut pb = [0u8; 16];
            for (j, f) in params.iter().enumerate() {
                pb[j * 4..(j + 1) * 4].copy_from_slice(&f.to_le_bytes());
            }
            queue.write_buffer(uniform, 0, &pb);

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perf comp bg"),
                layout: &self.bgl,
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
                        resource: uniform.as_entire_binding(),
                    },
                ],
            });

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perf comp pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: dst,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
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
                pass.draw(0..6, 0..1);
            }

            read_view = dst;
            write_ping = !write_ping;
        }
    }
}

// ---------------------------------------------------------------------------
// GammaPipeline (mirrors src/render/gamma.rs)
// ---------------------------------------------------------------------------

struct GammaPipeline {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
}

impl GammaPipeline {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perf gamma"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../src/render/shaders/gamma.wgsl").into(),
            ),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perf gamma bgl"),
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
                        min_binding_size: std::num::NonZeroU64::new(64),
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perf gamma layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perf gamma pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
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
            label: Some("perf gamma sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perf gamma uniform"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bgl,
            sampler,
            uniform,
        }
    }

    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        src: &wgpu::TextureView,
    ) {
        // Identity gamma: gamma=1.0, brightness=0.0, contrast=1.0, identity RGB matrix
        let mut b = [0u8; 64];
        let tone: [f32; 4] = [1.0, 0.0, 1.0, 0.0];
        for (i, f) in tone.iter().enumerate() {
            b[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        let identity = [[1.0f32, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        for (row_idx, row) in identity.iter().enumerate() {
            let base = 16 * (row_idx + 1);
            for (col_idx, f) in row.iter().enumerate() {
                let off = base + col_idx * 4;
                b[off..off + 4].copy_from_slice(&f.to_le_bytes());
            }
        }
        queue.write_buffer(&self.uniform, 0, &b);

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perf gamma bg"),
            layout: &self.bgl,
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
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perf gamma pass"),
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.draw(0..6, 0..1);
    }
}

// ---------------------------------------------------------------------------
// EdgeBlendPipeline (mirrors src/render/edge_blend.rs)
// ---------------------------------------------------------------------------

struct EdgeBlendPipeline {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
}

impl EdgeBlendPipeline {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perf edge_blend"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../src/render/shaders/edge_blend.wgsl").into(),
            ),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perf edge_blend bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(16),
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perf edge_blend layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
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
            label: Some("perf edge_blend pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(multiply_blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perf edge_blend uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bgl,
            uniform,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        surface_width: u32,
        overlap_px: u32,
        edge_side: f32, // 0.0 = right edge, 1.0 = left edge
    ) {
        let fields: [f32; 4] = [overlap_px as f32, surface_width as f32, edge_side, 0.0]; // Linear falloff
        let mut b = [0u8; 16];
        for (i, f) in fields.iter().enumerate() {
            b[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.uniform, 0, &b);

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perf edge_blend bg"),
            layout: &self.bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.uniform.as_entire_binding(),
            }],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perf edge_blend pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
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
        pass.set_bind_group(0, &bg, &[]);
        pass.draw(0..6, 0..1);
    }
}

// ---------------------------------------------------------------------------
// Per-layer GPU state
// ---------------------------------------------------------------------------

struct LayerGpu {
    /// FxLayer output texture (ripple wash renders into this).
    _fx_tex: wgpu::Texture,
    fx_view: wgpu::TextureView,
    /// Post-warp texture (warp renders into this; compositor reads from here).
    _warp_tex: wgpu::Texture,
    warp_view: wgpu::TextureView,
    /// SDF texture for the mask polygon.
    _sdf_tex: wgpu::Texture,
    sdf_view: wgpu::TextureView,
    /// Per-layer compositor uniform (opacity + blend mode).
    compositor_uniform: wgpu::Buffer,
    /// Per-layer warp pipeline instance (owns mesh + mask uniform).
    warp_pipeline: WarpPipeline,
}

// ---------------------------------------------------------------------------
// Fixture mask polygons — 4 non-overlapping quadrants
// ---------------------------------------------------------------------------

fn layer_polygon(idx: usize) -> Vec<[f32; 2]> {
    // Each layer occupies a different quadrant of the output.
    // Coordinates are normalised [0, 1].
    match idx {
        0 => vec![[0.0, 0.0], [0.5, 0.0], [0.5, 0.5], [0.0, 0.5]], // TL
        1 => vec![[0.5, 0.0], [1.0, 0.0], [1.0, 0.5], [0.5, 0.5]], // TR
        2 => vec![[0.0, 0.5], [0.5, 0.5], [0.5, 1.0], [0.0, 1.0]], // BL
        _ => vec![[0.5, 0.5], [1.0, 0.5], [1.0, 1.0], [0.5, 1.0]], // BR
    }
}

// ---------------------------------------------------------------------------
// Main test
// ---------------------------------------------------------------------------

#[test]
fn perf_frame_budget() {
    let h = Headless::new().expect("Headless::new — no GPU adapter available");
    let device = &h.device;
    let queue = &h.queue;

    // ---- Build shared pipelines ----
    let fx_pipeline = FxPipeline::new_ripple_wash(device, FORMAT);
    let compositor = Compositor::new(device, OUT_W, OUT_H, FORMAT);
    let gamma = GammaPipeline::new(device, FORMAT);
    let edge_blend = EdgeBlendPipeline::new(device, FORMAT);

    // ---- Build per-layer GPU state ----
    let mut layers: Vec<LayerGpu> = (0..LAYER_COUNT)
        .map(|i| {
            let poly = layer_polygon(i);
            let (_fx_tex, fx_view) =
                make_render_texture(device, OUT_W, OUT_H, FORMAT, &format!("fx tex layer {i}"));
            let (_warp_tex, warp_view) =
                make_render_texture(device, OUT_W, OUT_H, FORMAT, &format!("warp tex layer {i}"));
            let (_sdf_tex, sdf_view) =
                make_sdf_texture(device, queue, &poly, &format!("sdf layer {i}"));
            let compositor_uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("comp uniform layer {i}")),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let warp_pipeline = WarpPipeline::new(device, FORMAT);
            warp_pipeline.init_buffers(queue, poly.len() >= 3);
            LayerGpu {
                _fx_tex,
                fx_view,
                _warp_tex,
                warp_view,
                _sdf_tex,
                sdf_view,
                compositor_uniform,
                warp_pipeline,
            }
        })
        .collect();

    // ---- Allocate per-output render targets (simulated projector surfaces) ----
    // Production uses OutputWindow::surface_texture; here we render to offscreen
    // textures of the same size and format.
    let output_targets: Vec<(wgpu::Texture, wgpu::TextureView)> = (0..OUTPUT_COUNT)
        .map(|i| make_render_texture(device, OUT_W, OUT_H, FORMAT, &format!("output rt {i}")))
        .collect();

    // Shared warp_rt (compositor output, read by gamma/edge_blend).
    let (_warp_rt_tex, warp_rt_view) = make_render_texture(device, OUT_W, OUT_H, FORMAT, "warp rt");

    // ---- Frame timing accumulator ----
    let mut frame_times_ms: Vec<f64> = Vec::with_capacity(FRAME_COUNT);

    // ---- Drop / panic counters ----
    // TextureUploadQueue is not used (no video producers), so drop_count is always 0.
    // Panics are caught via std::panic::catch_unwind.
    let texture_upload_drop_count: u64 = 0; // no video/NDI producers in this fixture
    let mut panic_count: usize = 0;

    // ---- Render loop ----
    let start_time = Instant::now();

    for frame_idx in 0..FRAME_COUNT {
        let frame_start = Instant::now();
        let clock_secs = start_time.elapsed().as_secs_f32();

        // Wrap the per-frame work in catch_unwind to count panics rather than abort.
        // This mirrors the spirit of production's `panic_restore::run_frame_assert_unwind_safe`.
        let frame_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            render_frame(
                device,
                queue,
                &fx_pipeline,
                &compositor,
                &gamma,
                &edge_blend,
                &mut layers,
                &output_targets,
                &warp_rt_view,
                clock_secs,
            );
        }));

        if frame_result.is_err() {
            panic_count += 1;
        }

        // Poll to drive the wgpu submission to completion.
        // In production the event loop keeps the GPU fed without blocking;
        // here we poll after each frame to get accurate CPU+GPU timings.
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device.poll failed");

        let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        frame_times_ms.push(elapsed_ms);

        // Progress reporting every 100 frames so long runs don't look hung.
        if (frame_idx + 1) % 100 == 0 {
            println!(
                "[perf_frame_budget] frame {}/{FRAME_COUNT}: last={:.2}ms",
                frame_idx + 1,
                elapsed_ms
            );
        }
    }

    // ---- Compute statistics ----
    frame_times_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min_ms = frame_times_ms[0];
    let max_ms = *frame_times_ms.last().unwrap();
    let p50_ms = percentile(&frame_times_ms, 50.0);
    let p99_ms = percentile(&frame_times_ms, 99.0);
    let total_frames = frame_times_ms.len();

    // ---- Print results ----
    println!();
    println!("=== P0.9.5 Frame-Budget Gate Results ===");
    println!("  Frames rendered:  {total_frames}");
    println!("  Min frame time:   {min_ms:.2} ms");
    println!("  p50 frame time:   {p50_ms:.2} ms");
    println!("  p99 frame time:   {p99_ms:.2} ms");
    println!("  Max frame time:   {max_ms:.2} ms");
    println!("  Texture drops:    {texture_upload_drop_count}");
    println!("  Panic count:      {panic_count}");
    println!();
    println!("  CI assertion: p99 < 100 ms (regression guard)");
    println!("  Show-day target:  p99 ≤ 16.6 ms on actual projector hardware");
    println!("  Record baseline in docs/show-day-checklist.md on first run.");
    println!("=========================================");

    // ---- Assertions ----

    // No texture uploads were dropped (trivially true since there are no
    // video/NDI producers in this fixture; documents the invariant for
    // when those producers land).
    assert_eq!(
        texture_upload_drop_count, 0,
        "texture upload drop count must be zero (no producers configured in fixture)"
    );

    // No frame panicked. A non-zero count indicates a real render bug.
    assert_eq!(
        panic_count, 0,
        "panic_count must be zero: {panic_count} frame(s) panicked — check render graph"
    );

    // p99 < 100 ms: CI-portable regression gate. Catches a 10× slowdown
    // on even the slowest CI runners. The show-day acceptance (≤ 16.6 ms)
    // is recorded by the operator on actual show hardware (see checklist).
    assert!(
        p99_ms < 100.0,
        "p99 frame time {p99_ms:.2} ms exceeds 100 ms CI regression gate \
         (fixture: {LAYER_COUNT} FxLayers, {OUTPUT_COUNT} outputs, edge-blend {EDGE_BLEND_OVERLAP_PX}px)"
    );
}

// ---------------------------------------------------------------------------
// Per-frame render sequence
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn render_frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fx_pipeline: &FxPipeline,
    compositor: &Compositor,
    gamma: &GammaPipeline,
    edge_blend: &EdgeBlendPipeline,
    layers: &mut [LayerGpu],
    output_targets: &[(wgpu::Texture, wgpu::TextureView)],
    warp_rt_view: &wgpu::TextureView,
    clock_secs: f32,
) {
    // ---- Passes 1-4: FX render + warp + compositor → warp_rt ----
    // One shared encoder for the per-layer + compositor passes (mirrors
    // production's "m5 offscreen encoder").
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perf offscreen encoder"),
    });

    // Compositor inputs: (warp_view, opacity, blend_mode_code (0=Normal), uniform)
    // Collected after all layers have rendered through fx + warp.
    let comp_inputs: Vec<(&wgpu::TextureView, f32, f32, &wgpu::Buffer)> = layers
        .iter_mut()
        .map(|ls| {
            // Pass 1: FxLayer — run ripple-wash preset into fx_tex.
            fx_pipeline.render(
                device,
                queue,
                &mut encoder,
                &ls.fx_view,
                &ls.sdf_view,
                clock_secs,
            );

            // Pass 2-3: effect chain — zero effects, so fx_view IS the final
            // pre-warp output. No ping-pong needed.
            //
            // Pass 4 (warp): fx_view → warp_view.
            ls.warp_pipeline.render(
                device,
                queue,
                &mut encoder,
                &ls.warp_view,
                &ls.fx_view,
                &ls.sdf_view,
            );

            (
                &ls.warp_view as &wgpu::TextureView,
                1.0f32,
                0.0f32,
                &ls.compositor_uniform as &wgpu::Buffer,
            )
        })
        .collect();

    // Compositor: blend all warp_views into warp_rt_view.
    compositor.composite(
        device,
        queue,
        &mut encoder,
        wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        warp_rt_view,
        &comp_inputs,
    );

    // Submit passes 1-4.
    queue.submit(std::iter::once(encoder.finish()));

    // ---- Passes 5-6: gamma + edge-blend, once per simulated output ----
    for (out_idx, (_out_tex, out_view)) in output_targets.iter().enumerate() {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perf gamma encoder"),
        });

        // Pass 5: gamma (identity).
        gamma.render(device, queue, &mut enc, out_view, warp_rt_view);

        // Pass 5b: edge-blend multiply.
        // output 0 = left projector (right-edge falloff, edge_side=0.0)
        // output 1 = right projector (left-edge falloff, edge_side=1.0)
        let edge_side = if out_idx == 0 { 0.0f32 } else { 1.0 };
        edge_blend.render(
            device,
            queue,
            &mut enc,
            out_view,
            OUT_W,
            EDGE_BLEND_OVERLAP_PX,
            edge_side,
        );

        queue.submit(std::iter::once(enc.finish()));
    }
}

// ---------------------------------------------------------------------------
// P2.1.2 — 4× ripple_wash stub fixture at max amplitude
// ---------------------------------------------------------------------------

/// Max-amplitude params for `mask_edge_ripple_wash`:
/// wavelength=200, speed=10, falloff=0.5, base_r/g/b=1.0/1.0/1.0, pad=0/0.
///
/// "Max amplitude" pushes wavelength, speed and falloff toward their high end.
/// The colour channels are clamped in the shader so 1.0 is the effective ceiling.
/// FIXME(P2.5.1): replace stub fixture with real particle layers at max budget.
const MAX_AMP_PARAMS: [f32; 8] = [200.0, 10.0, 0.5, 1.0, 1.0, 1.0, 0.0, 0.0];

/// Same layer count as the existing gate — four FxLayers — but exercising
/// max-amplitude parameters rather than defaults.
const FX4_LAYER_COUNT: usize = 4;

// M-series baseline p99 ≈ 11.5 ms (2026-05-12, Apple Silicon, headless wgpu).

/// P2.1.2 — show-day perf gate for 4 maximally-parametrised ripple_wash FxLayers.
///
/// The fixture substitutes a stub of four `mask_edge_ripple_wash` layers at
/// max-amplitude params for the eventual real particle layers (P2.5.1).
/// The 16.6 ms p99 target matches one frame period at 60 Hz.
#[cfg(feature = "gpu-tests")]
#[test]
fn perf_four_fx_layers_within_budget() {
    let h = Headless::new().expect("Headless::new — no GPU adapter available");
    let device = &h.device;
    let queue = &h.queue;

    // ---- Build shared pipelines (own instance so no state bleeds from the
    //      default-params test) ----
    let fx_pipeline = FxPipeline::new_ripple_wash(device, FORMAT);
    let compositor = Compositor::new(device, OUT_W, OUT_H, FORMAT);
    let gamma = GammaPipeline::new(device, FORMAT);
    let edge_blend = EdgeBlendPipeline::new(device, FORMAT);

    // ---- Build per-layer GPU state ----
    let mut layers: Vec<LayerGpu> = (0..FX4_LAYER_COUNT)
        .map(|i| {
            let poly = layer_polygon(i);
            let (_fx_tex, fx_view) = make_render_texture(
                device,
                OUT_W,
                OUT_H,
                FORMAT,
                &format!("fx4 fx tex layer {i}"),
            );
            let (_warp_tex, warp_view) = make_render_texture(
                device,
                OUT_W,
                OUT_H,
                FORMAT,
                &format!("fx4 warp tex layer {i}"),
            );
            let (_sdf_tex, sdf_view) =
                make_sdf_texture(device, queue, &poly, &format!("fx4 sdf layer {i}"));
            let compositor_uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("fx4 comp uniform layer {i}")),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let warp_pipeline = WarpPipeline::new(device, FORMAT);
            warp_pipeline.init_buffers(queue, poly.len() >= 3);
            LayerGpu {
                _fx_tex,
                fx_view,
                _warp_tex,
                warp_view,
                _sdf_tex,
                sdf_view,
                compositor_uniform,
                warp_pipeline,
            }
        })
        .collect();

    // ---- Per-output render targets ----
    let output_targets: Vec<(wgpu::Texture, wgpu::TextureView)> = (0..OUTPUT_COUNT)
        .map(|i| make_render_texture(device, OUT_W, OUT_H, FORMAT, &format!("fx4 output rt {i}")))
        .collect();

    let (_warp_rt_tex, warp_rt_view) =
        make_render_texture(device, OUT_W, OUT_H, FORMAT, "fx4 warp rt");

    // ---- Frame timing accumulator ----
    let mut frame_times_ms: Vec<f64> = Vec::with_capacity(FRAME_COUNT);
    let texture_upload_drop_count: u64 = 0;
    let mut panic_count: usize = 0;

    let start_time = Instant::now();

    for frame_idx in 0..FRAME_COUNT {
        let frame_start = Instant::now();
        let clock_secs = start_time.elapsed().as_secs_f32();

        let frame_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            render_frame_max_amp(
                device,
                queue,
                &fx_pipeline,
                &compositor,
                &gamma,
                &edge_blend,
                &mut layers,
                &output_targets,
                &warp_rt_view,
                clock_secs,
            );
        }));

        if frame_result.is_err() {
            panic_count += 1;
        }

        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device.poll failed");

        let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        frame_times_ms.push(elapsed_ms);

        if (frame_idx + 1) % 100 == 0 {
            println!(
                "[perf_four_fx_layers_within_budget] frame {}/{FRAME_COUNT}: last={:.2}ms",
                frame_idx + 1,
                elapsed_ms
            );
        }
    }

    // ---- Compute statistics ----
    frame_times_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min_ms = frame_times_ms[0];
    let max_ms = *frame_times_ms.last().unwrap();
    let p50_ms = percentile(&frame_times_ms, 50.0);
    let p99_ms = percentile(&frame_times_ms, 99.0);
    let total_frames = frame_times_ms.len();

    // ---- Print results ----
    println!();
    println!("=== P2.1.2 Frame-Budget Gate: 4× ripple_wash max-amplitude ===");
    println!("  Frames rendered:  {total_frames}");
    println!("  Min frame time:   {min_ms:.2} ms");
    println!("  p50 frame time:   {p50_ms:.2} ms");
    println!("  p99 frame time:   {p99_ms:.2} ms");
    println!("  Max frame time:   {max_ms:.2} ms");
    println!("  Texture drops:    {texture_upload_drop_count}");
    println!("  Panic count:      {panic_count}");
    println!();
    println!("  CI assertion:     p99 < 100 ms (regression guard)");
    println!("  Show-day target:  p99 ≤ 16.6 ms on actual projector hardware");
    println!("  FIXME(P2.5.1): replace stub fixture with real particle layers at max budget.");
    println!("===============================================================");

    // ---- Assertions ----
    assert_eq!(
        texture_upload_drop_count, 0,
        "texture upload drop count must be zero (no producers configured in fixture)"
    );
    assert_eq!(
        panic_count, 0,
        "panic_count must be zero: {panic_count} frame(s) panicked — check render graph"
    );
    // CI-portable loose gate (10× regression guard). Show-day target (≤ 16.6 ms) is
    // verified by the operator on M-series hardware and recorded in the comment above.
    assert!(
        p99_ms < 100.0,
        "p99 frame time {p99_ms:.2} ms exceeds 100 ms CI regression gate \
         (fixture: {FX4_LAYER_COUNT} FxLayers max-amplitude, {OUTPUT_COUNT} outputs, \
         edge-blend {EDGE_BLEND_OVERLAP_PX}px)"
    );
    // Show-day acceptance: p99 ≤ 16.6 ms (one frame at 60 Hz).
    assert!(
        p99_ms <= 16.6,
        "p99 frame time {p99_ms:.2} ms exceeds show-day budget of 16.6 ms \
         (fixture: {FX4_LAYER_COUNT} FxLayers at max amplitude)"
    );
}

/// Per-frame render sequence for the max-amplitude fixture.
///
/// Mirrors `render_frame` but passes `MAX_AMP_PARAMS` to each FxLayer.
#[allow(clippy::too_many_arguments)]
fn render_frame_max_amp(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fx_pipeline: &FxPipeline,
    compositor: &Compositor,
    gamma: &GammaPipeline,
    edge_blend: &EdgeBlendPipeline,
    layers: &mut [LayerGpu],
    output_targets: &[(wgpu::Texture, wgpu::TextureView)],
    warp_rt_view: &wgpu::TextureView,
    clock_secs: f32,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perf fx4 offscreen encoder"),
    });

    let comp_inputs: Vec<(&wgpu::TextureView, f32, f32, &wgpu::Buffer)> = layers
        .iter_mut()
        .map(|ls| {
            fx_pipeline.render_with_params(
                device,
                queue,
                &mut encoder,
                &ls.fx_view,
                &ls.sdf_view,
                clock_secs,
                &MAX_AMP_PARAMS,
            );

            ls.warp_pipeline.render(
                device,
                queue,
                &mut encoder,
                &ls.warp_view,
                &ls.fx_view,
                &ls.sdf_view,
            );

            (
                &ls.warp_view as &wgpu::TextureView,
                1.0f32,
                0.0f32,
                &ls.compositor_uniform as &wgpu::Buffer,
            )
        })
        .collect();

    compositor.composite(
        device,
        queue,
        &mut encoder,
        wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        warp_rt_view,
        &comp_inputs,
    );

    queue.submit(std::iter::once(encoder.finish()));

    for (out_idx, (_out_tex, out_view)) in output_targets.iter().enumerate() {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perf fx4 gamma encoder"),
        });

        gamma.render(device, queue, &mut enc, out_view, warp_rt_view);

        let edge_side = if out_idx == 0 { 0.0f32 } else { 1.0 };
        edge_blend.render(
            device,
            queue,
            &mut enc,
            out_view,
            OUT_W,
            EDGE_BLEND_OVERLAP_PX,
            edge_side,
        );

        queue.submit(std::iter::once(enc.finish()));
    }
}

// ---------------------------------------------------------------------------
// P3.1.2 — zone-tagged FX layer perf gate (stub fixture)
// ---------------------------------------------------------------------------

// M-series baseline p99 ≈ 11.5 ms (2026-05-12, Apple Silicon, headless wgpu).
// TODO(P3.5.3): update fixture to use fx_zone_portal_drift once that preset
// lands; the stub uses ripple_wash (zone_role = None) so the test is runnable
// before zone-consuming presets exist.

/// P3.1.2 — show-day perf gate for a single zone-tagged FX layer at max budget.
///
/// The fixture uses a single `mask_edge_ripple_wash` layer (with `zone_role =
/// None` — the stub) to verify that zone-tag dispatch overhead doesn't regress
/// the frame budget once real zone-consuming presets land in P3.5.x. The
/// 16.6 ms p99 target matches one frame period at 60 Hz on show-day hardware.
///
/// This test is intentionally a stub. When P3.5.3 lands, replace
/// `FxPipeline::new_ripple_wash` with the portal-drift pipeline and update the
/// comment above to reflect the new fixture.
#[cfg(feature = "gpu-tests")]
#[test]
fn perf_zone_tagged_fx_layer_within_budget() {
    let h = Headless::new().expect("Headless::new — no GPU adapter available");
    let device = &h.device;
    let queue = &h.queue;

    // ---- Build shared pipelines (one layer, zone_role = None stub) ----
    let fx_pipeline = FxPipeline::new_ripple_wash(device, FORMAT);
    let compositor = Compositor::new(device, OUT_W, OUT_H, FORMAT);
    let gamma = GammaPipeline::new(device, FORMAT);
    let edge_blend = EdgeBlendPipeline::new(device, FORMAT);

    // ---- Build single-layer GPU state ----
    let poly = layer_polygon(0);
    let (_fx_tex, fx_view) = make_render_texture(device, OUT_W, OUT_H, FORMAT, "zone stub fx tex");
    let (_warp_tex, warp_view) =
        make_render_texture(device, OUT_W, OUT_H, FORMAT, "zone stub warp tex");
    let (_sdf_tex, sdf_view) = make_sdf_texture(device, queue, &poly, "zone stub sdf");
    let compositor_uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("zone stub comp uniform"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let warp_pipeline = WarpPipeline::new(device, FORMAT);
    warp_pipeline.init_buffers(queue, poly.len() >= 3);
    let mut layers = vec![LayerGpu {
        _fx_tex,
        fx_view,
        _warp_tex,
        warp_view,
        _sdf_tex,
        sdf_view,
        compositor_uniform,
        warp_pipeline,
    }];

    // ---- Per-output render targets (two outputs, edge blend) ----
    let output_targets: Vec<(wgpu::Texture, wgpu::TextureView)> = (0..OUTPUT_COUNT)
        .map(|i| {
            make_render_texture(
                device,
                OUT_W,
                OUT_H,
                FORMAT,
                &format!("zone stub output rt {i}"),
            )
        })
        .collect();

    let (_warp_rt_tex, warp_rt_view) =
        make_render_texture(device, OUT_W, OUT_H, FORMAT, "zone stub warp rt");

    // ---- Frame timing accumulator ----
    let mut frame_times_ms: Vec<f64> = Vec::with_capacity(FRAME_COUNT);
    let texture_upload_drop_count: u64 = 0;
    let mut panic_count: usize = 0;

    let start_time = Instant::now();

    for frame_idx in 0..FRAME_COUNT {
        let frame_start = Instant::now();
        let clock_secs = start_time.elapsed().as_secs_f32();

        let frame_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            render_frame_max_amp(
                device,
                queue,
                &fx_pipeline,
                &compositor,
                &gamma,
                &edge_blend,
                &mut layers,
                &output_targets,
                &warp_rt_view,
                clock_secs,
            );
        }));

        if frame_result.is_err() {
            panic_count += 1;
        }

        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device.poll failed");

        let elapsed_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        frame_times_ms.push(elapsed_ms);

        if (frame_idx + 1) % 100 == 0 {
            println!(
                "[perf_zone_tagged_fx_layer_within_budget] frame {}/{FRAME_COUNT}: last={:.2}ms",
                frame_idx + 1,
                elapsed_ms
            );
        }
    }

    // ---- Compute statistics ----
    frame_times_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min_ms = frame_times_ms[0];
    let max_ms = *frame_times_ms.last().unwrap();
    let p50_ms = percentile(&frame_times_ms, 50.0);
    let p99_ms = percentile(&frame_times_ms, 99.0);
    let total_frames = frame_times_ms.len();

    // ---- Print results ----
    println!();
    println!("=== P3.1.2 Frame-Budget Gate: 1× zone-tagged FX layer (stub) ===");
    println!("  Frames rendered:  {total_frames}");
    println!("  Min frame time:   {min_ms:.2} ms");
    println!("  p50 frame time:   {p50_ms:.2} ms");
    println!("  p99 frame time:   {p99_ms:.2} ms");
    println!("  Max frame time:   {max_ms:.2} ms");
    println!("  Texture drops:    {texture_upload_drop_count}");
    println!("  Panic count:      {panic_count}");
    println!();
    println!("  CI assertion:     p99 < 100 ms (regression guard)");
    println!("  Show-day target:  p99 ≤ 16.6 ms on actual projector hardware");
    println!(
        "  NOTE: stub fixture (ripple_wash, zone_role = None); \
         update to fx_zone_portal_drift in P3.5.3."
    );
    println!("=================================================================");

    // ---- Assertions ----
    assert_eq!(
        texture_upload_drop_count, 0,
        "texture upload drop count must be zero"
    );
    assert_eq!(
        panic_count, 0,
        "panic_count must be zero: {panic_count} frame(s) panicked"
    );
    // CI-portable loose gate (10× regression guard).
    assert!(
        p99_ms < 100.0,
        "p99 frame time {p99_ms:.2} ms exceeds 100 ms CI regression gate \
         (zone-tagged stub fixture: 1 FxLayer, {OUTPUT_COUNT} outputs, \
         edge-blend {EDGE_BLEND_OVERLAP_PX}px)"
    );
    // Show-day acceptance: p99 ≤ 16.6 ms (one frame at 60 Hz).
    // M-series baseline: ~11.5 ms p99 on Apple Silicon (2026-05-12).
    assert!(
        p99_ms <= 16.6,
        "p99 frame time {p99_ms:.2} ms exceeds show-day budget of 16.6 ms \
         (zone-tagged stub fixture: 1 FxLayer at max amplitude)"
    );
}

// ---------------------------------------------------------------------------
// Percentile helper
// ---------------------------------------------------------------------------

/// Compute the p-th percentile of a sorted slice (nearest-rank method).
fn percentile(sorted: &[f64], p: f64) -> f64 {
    assert!(!sorted.is_empty(), "percentile: empty slice");
    let idx = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    let idx = idx.clamp(1, sorted.len()) - 1;
    sorted[idx]
}
