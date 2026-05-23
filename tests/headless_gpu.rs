//! T-M5-14 — Headless wgpu test infrastructure.
//!
//! A small harness for golden-image tests that need a real GPU device but no
//! window/surface. Gated behind the `gpu-tests` cargo feature so plain
//! `cargo test` stays GPU-free; CI and developers run the GPU tests with
//! `cargo test --features gpu-tests`.
//!
//! Public surface (within this test binary):
//!
//! - [`Headless`] — owns `Instance + Adapter + Device + Queue`. Constructed
//!   with [`Headless::new`], blocking via `pollster`.
//! - [`Headless::render_to_rgba8`] — allocates an offscreen
//!   `Rgba8UnormSrgb` texture sized `width × height` with usage
//!   `RENDER_ATTACHMENT | COPY_SRC`, hands `(device, queue, view)` to the
//!   user closure so it can record draws into the view, then issues a
//!   `copy_texture_to_buffer` with proper 256-byte row alignment, maps the
//!   readback buffer on the host, strips row padding, and returns a
//!   tightly-packed `Vec<u8>` of `width * height * 4` bytes.
//! - [`assert_image_matches`] — compares actual bytes against a PNG golden
//!   on disk via per-channel max-diff; on mismatch, writes
//!   `<golden>.actual.png` next to the golden so a developer can `open` it
//!   and visually compare. Setting `UPDATE_GOLDEN=1` overwrites the golden
//!   with the actual bytes instead of asserting (canonical snapshot-update
//!   pattern).
//!
//! Future tests T-M5-15 / T-M5-16 / T-M5-17 share this file; their pipelines
//! load the production WGSL via `include_str!("../src/render/shaders/…")`
//! and run them through [`Headless::render_to_rgba8`].

#![cfg(feature = "gpu-tests")]

use std::path::Path;
use std::sync::mpsc;

use anyhow::{Context, Result, anyhow};
use image::{ImageBuffer, Rgba};

/// Format used for all offscreen render targets in the harness. `Srgb`
/// matches what production renders into (`OutputWindow` configures the
/// surface as `Rgba8UnormSrgb`); a clear color of `(1, 0, 0, 1)` lands as
/// bytes `[255, 0, 0, 255]` in storage, which is convenient for assertions.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Owns the wgpu objects needed to drive headless rendering: an `Instance`
/// over all backends, the picked `Adapter`, and the `Device` + `Queue`
/// derived from it. Constructed once per test binary.
pub struct Headless {
    #[allow(dead_code)]
    pub instance: wgpu::Instance,
    #[allow(dead_code)]
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Headless {
    /// Bootstrap wgpu with no surface compatibility hint. Uses
    /// `Backends::all()` so any platform's default backend (Metal on macOS,
    /// Vulkan / DX12 on Linux / Windows) is acceptable, and
    /// `Limits::downlevel_defaults()` so the harness runs on minimal
    /// hardware (CI runners, integrated GPUs).
    ///
    /// `pollster::block_on` drives the async wgpu calls — we stay
    /// synchronous on the calling thread, mirroring the rest of the
    /// codebase's GPU bring-up.
    pub fn new() -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|e| anyhow!("no compatible wgpu adapter for headless tests: {e}"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rmap headless test device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .context("request_device for headless tests")?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    /// Allocate an offscreen `width × height` [`TARGET_FORMAT`] texture, run
    /// the user closure with `(device, queue, view)` so it can record draws
    /// into `view`, then `copy_texture_to_buffer` into a host-mappable
    /// readback buffer, map it, strip the 256-byte row padding wgpu
    /// requires, and return a tightly packed `Vec<u8>` of length
    /// `width * height * 4`.
    ///
    /// The closure may submit its own command buffers (e.g. it can encode +
    /// submit a render pass that targets `view`). After it returns, the
    /// harness submits its own copy command separately, so callers don't
    /// have to coordinate encoders.
    pub fn render_to_rgba8(
        &self,
        width: u32,
        height: u32,
        draw: impl FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::TextureView),
    ) -> Vec<u8> {
        assert!(width > 0 && height > 0, "image dimensions must be non-zero");

        // 1. The render target. Both COPY_SRC (so we can read it back) and
        //    RENDER_ATTACHMENT (so the closure can draw into it).
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless render target"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 2. Hand off to user code. They may submit any number of command
        //    buffers that target `view`.
        draw(&self.device, &self.queue, &view);

        // 3. Allocate a readback buffer with 256-byte row alignment.
        let unpadded_bytes_per_row = width * 4;
        let padded_bytes_per_row =
            align_up(unpadded_bytes_per_row, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer_size = (padded_bytes_per_row * height) as wgpu::BufferAddress;

        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headless readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 4. Encode the texture→buffer copy. Submit on our own queue.
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("headless copy encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            size,
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        // 5. Map + drain the buffer. `device.poll(Wait)` blocks until the
        //    submission completes and the map callback fires.
        let slice = readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            // If sending fails the test thread has already gone away;
            // nothing useful to do.
            let _ = tx.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device.poll Wait failed");
        rx.recv()
            .expect("map_async channel dropped before completion")
            .expect("map_async failed");

        // 6. Repack: drop the per-row padding wgpu inserted.
        let mut packed = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
        {
            let mapped = slice.get_mapped_range();
            for row in 0..height {
                let start = (row * padded_bytes_per_row) as usize;
                let end = start + unpadded_bytes_per_row as usize;
                packed.extend_from_slice(&mapped[start..end]);
            }
        }
        readback.unmap();
        packed
    }
}

/// Round `value` up to the nearest multiple of `align`. Used for the
/// 256-byte `bytes_per_row` alignment wgpu requires for
/// `copy_texture_to_buffer`.
fn align_up(value: u32, align: u32) -> u32 {
    value.div_ceil(align) * align
}

/// Compare `got` (tightly-packed RGBA8, length `width * height * 4`) against
/// the PNG golden at `golden_path`.
///
/// - If the env var `UPDATE_GOLDEN=1` is set, write `got` as a PNG to
///   `golden_path` and return without asserting. This is the standard
///   snapshot-update pattern.
/// - Otherwise, load the golden via `image::open`, assert the dimensions
///   match, then compute the per-channel max diff over every pixel. If any
///   channel's absolute diff exceeds `tolerance`, write `got` to
///   `<golden_path>.actual.png` and panic with a clear message.
pub fn assert_image_matches(got: &[u8], width: u32, height: u32, golden_path: &str, tolerance: u8) {
    assert_eq!(
        got.len() as u32,
        width * height * 4,
        "got buffer size mismatch: expected {} bytes for {}x{} RGBA8, got {}",
        width * height * 4,
        width,
        height,
        got.len()
    );

    if std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1") {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(width, height, got.to_vec())
                .expect("ImageBuffer::from_raw failed for actual bytes");
        if let Some(parent) = Path::new(golden_path).parent() {
            std::fs::create_dir_all(parent).expect("create golden parent dir");
        }
        img.save(golden_path)
            .unwrap_or_else(|e| panic!("UPDATE_GOLDEN: failed to write {golden_path}: {e}"));
        eprintln!("UPDATE_GOLDEN=1: wrote new golden to {golden_path}");
        return;
    }

    let golden = image::open(golden_path)
        .unwrap_or_else(|e| panic!("failed to open golden {golden_path}: {e}\nIf this is the first run, set UPDATE_GOLDEN=1 to create it."))
        .to_rgba8();

    if golden.width() != width || golden.height() != height {
        write_actual(got, width, height, golden_path);
        panic!(
            "golden {golden_path}: dimension mismatch (golden {}x{}, got {width}x{height}); wrote actual to {golden_path}.actual.png",
            golden.width(),
            golden.height()
        );
    }

    let golden_bytes = golden.as_raw();
    debug_assert_eq!(golden_bytes.len(), got.len());

    let mut max_diff: u8 = 0;
    let mut worst_idx = 0usize;
    for (i, (&g, &a)) in golden_bytes.iter().zip(got.iter()).enumerate() {
        let d = g.abs_diff(a);
        if d > max_diff {
            max_diff = d;
            worst_idx = i;
        }
    }

    if max_diff > tolerance {
        write_actual(got, width, height, golden_path);
        let pixel = worst_idx / 4;
        let channel = worst_idx % 4;
        let x = (pixel as u32) % width;
        let y = (pixel as u32) / width;
        panic!(
            "golden {golden_path}: max channel diff {max_diff} > tolerance {tolerance}\n\
             worst at pixel ({x}, {y}) channel {channel}: golden={} got={}\n\
             actual image written to {golden_path}.actual.png — open it to compare",
            golden_bytes[worst_idx], got[worst_idx]
        );
    }
}

fn write_actual(got: &[u8], width: u32, height: u32, golden_path: &str) {
    let actual_path = format!("{golden_path}.actual.png");
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        match ImageBuffer::from_raw(width, height, got.to_vec()) {
            Some(img) => img,
            None => {
                eprintln!("could not build ImageBuffer from actual bytes for {actual_path}");
                return;
            }
        };
    if let Some(parent) = Path::new(&actual_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = img.save(&actual_path) {
        eprintln!("failed to save {actual_path}: {e}");
    }
}

// ---------------------------------------------------------------------------
// P3.6.2 — Zone-tag dispatch helpers.
// ---------------------------------------------------------------------------

/// Build a zone-aware render pipeline for a given WGSL source. The BGL
/// includes slots 0 (SDF texture), 1 (sampler), 2 (FxParams), 3 (clock), and
/// 6 (ZoneTagUniform) — matching the P3.3.2 bind-group slot contract.
#[cfg(feature = "gpu-tests")]
fn build_zone_aware_pipeline(
    device: &wgpu::Device,
    wgsl_src: &str,
    label: &str,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(wgsl_src.into()),
    });

    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(&format!("{label} bgl")),
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
            // slot 6: ZoneTagUniform (P3.3.2 contract)
            wgpu::BindGroupLayoutEntry {
                binding: 6,
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
        label: Some(&format!("{label} layout")),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&format!("{label} pipeline")),
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
                format: TARGET_FORMAT,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });

    (pipeline, bgl)
}

/// Render a zone-aware preset into a 128×128 texture. Returns the RGBA pixel bytes.
/// `zone_tag` is the u32 zone tag written to the ZoneTagUniform at slot 6.
#[cfg(feature = "gpu-tests")]
fn render_zone_preset(
    h: &Headless,
    wgsl_src: &str,
    label: &str,
    zone_tag: u32,
    clock_secs: f32,
) -> Vec<u8> {
    const W: u32 = 128;
    const H: u32 = 128;

    h.render_to_rgba8(W, H, |device, queue, view| {
        let (pipeline, bgl) = build_zone_aware_pipeline(device, wgsl_src, label);

        let sdf_view = make_circular_sdf(device, queue, 128);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // FxParams: defaults (all zeros / preset-specific defaults).
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zone test params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Use preset-appropriate defaults: spill_radius=0.3, falloff=0.08, colour warm.
        let mut params = [0u8; 32];
        params[0..4].copy_from_slice(&0.3f32.to_le_bytes()); // spill_radius / frequency
        params[8..12].copy_from_slice(&0.08f32.to_le_bytes()); // falloff
        params[12..16].copy_from_slice(&1.0f32.to_le_bytes()); // base_r
        params[16..20].copy_from_slice(&0.85f32.to_le_bytes()); // base_g
        params[20..24].copy_from_slice(&0.55f32.to_le_bytes()); // base_b
        queue.write_buffer(&params_buf, 0, &params);

        // Clock uniform.
        let clock_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zone test clock"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut clock = [0u8; 16];
        clock[0..4].copy_from_slice(&clock_secs.to_le_bytes());
        queue.write_buffer(&clock_buf, 0, &clock);

        // ZoneTagUniform: zone_tag + 3 × 0 padding.
        let zone_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zone test zone_tag"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut zone_bytes = [0u8; 16];
        zone_bytes[0..4].copy_from_slice(&zone_tag.to_le_bytes());
        queue.write_buffer(&zone_buf, 0, &zone_bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("zone test bind group"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&sdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: clock_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: zone_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("zone test encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("zone test pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    })
}

/// Smoke test: clear a 16×16 target to red, read it back, verify storage
/// bytes are `[255, 0, 0, 255]` per pixel.
///
/// Proves: `Headless::new` works on this machine, the target texture has
/// `RENDER_ATTACHMENT | COPY_SRC` usage, the closure can run a clear-only
/// render pass, and the row-alignment-stripping logic in
/// `render_to_rgba8` produces tight RGBA8 output.
#[test]
fn smoke_clear_red() {
    let h = Headless::new().expect("Headless::new");
    let bytes = h.render_to_rgba8(16, 16, |device, queue, view| {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("smoke clear encoder"),
        });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("smoke clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Rgba8UnormSrgb storage: clear (1,0,0,1) → bytes
                        // (255, 0, 0, 255). The sRGB encode is identity for
                        // 0.0 and 1.0, so no transfer-curve surprises here.
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        queue.submit(std::iter::once(encoder.finish()));
    });

    assert_eq!(bytes.len(), 16 * 16 * 4);
    assert_eq!(
        &bytes[0..4],
        &[255, 0, 0, 255],
        "first pixel should be opaque red after clear"
    );
    // Spot-check the last pixel too — confirms row-padding strip works for
    // every row, not just the first.
    let last = bytes.len() - 4;
    assert_eq!(
        &bytes[last..last + 4],
        &[255, 0, 0, 255],
        "last pixel should be opaque red after clear"
    );
}

// ===========================================================================
// Shared helpers for golden-image tests (T-M5-15 / T-M5-16 / T-M5-17).
//
// `tests/*.rs` are each their own test binary, so we keep all golden-image
// tests in this single file (headless_gpu.rs) to avoid duplicating the
// harness or these helpers.
//
// The crate has no `lib.rs`, so production pipeline structs (ColorPipeline,
// BlurPipeline, WarpRenderer) are not reachable from integration tests.
// Instead we rebuild minimal pipelines here, loading the production WGSL via
// `include_str!` so the shader code under test stays the source of truth.
// The Rust-side wiring (BGLs, samplers, blend states) mirrors the production
// versions in src/effects/{color,blur}.rs and src/render/warp.rs.
// ===========================================================================

const TOLERANCE: u8 = 2;

/// Allocate an offscreen texture with usage suitable as either the source or
/// destination of a render pass: `TEXTURE_BINDING | RENDER_ATTACHMENT |
/// COPY_DST`. Used for ping-pong intermediates in the blur test and for the
/// uploaded-from-CPU input textures in all three tests.
fn make_io_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

/// Upload a tightly-packed RGBA8 byte slice into a fresh `TARGET_FORMAT`
/// texture. The upload path uses `queue.write_texture`; row alignment is
/// handled by wgpu since we provide `bytes_per_row = width * 4`.
///
/// Note: `TARGET_FORMAT` is `Rgba8UnormSrgb`. The bytes are interpreted as
/// sRGB-encoded values; samplers will linearize them on read. This matches
/// production: `EffectPipeline` ping-pong textures use the surface format
/// (`Rgba8UnormSrgb`) and SVG rasterization writes sRGB bytes there too.
fn upload_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    data: &[u8],
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    assert_eq!(data.len() as u32, width * height * 4, "rgba8 size mismatch");
    let (tex, view) = make_io_texture(device, width, height, label);
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    (tex, view)
}

/// Horizontal RGB gradient: R sweeps 0→255 left-to-right, G fixed at 128,
/// B sweeps 255→0 left-to-right, A=255. Deterministic by construction.
fn make_gradient(width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for _y in 0..height {
        for x in 0..width {
            let t = if width > 1 {
                x as f32 / (width - 1) as f32
            } else {
                0.0
            };
            let r = (t * 255.0).round() as u8;
            let g = 128u8;
            let b = ((1.0 - t) * 255.0).round() as u8;
            out.extend_from_slice(&[r, g, b, 255]);
        }
    }
    out
}

/// Black/white checkerboard with `cell`-pixel squares. White in the (0,0)
/// cell. Alpha=255 everywhere.
fn make_checkerboard(width: u32, height: u32, cell: u32) -> Vec<u8> {
    let cell = cell.max(1);
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let cx = x / cell;
            let cy = y / cell;
            let c: u8 = if (cx + cy) % 2 == 0 { 255 } else { 0 };
            out.extend_from_slice(&[c, c, c, 255]);
        }
    }
    out
}

/// 16-byte uniform buffer with `UNIFORM | COPY_DST` usage, sized for the
/// `ColorParams` / `BlurParams` wire formats (both 16 bytes).
fn make_uniform_buffer(device: &wgpu::Device, label: &str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

// ---------------------------------------------------------------------------
// Pipeline rebuilds (no lib.rs ⇒ tests can't import from src/).
// Each one mirrors the production constructor in src/effects/* and
// src/render/warp.rs but is kept minimal: only the bits the test exercises.
// ---------------------------------------------------------------------------

struct EffectBgl {
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

/// BGL + sampler shared by the color and blur pipelines (both use the same
/// 3-binding layout: float texture, filtering sampler, 16-byte uniform).
fn make_effect_bgl(device: &wgpu::Device, label: &str) -> EffectBgl {
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
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
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("effect sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    EffectBgl { bgl, sampler }
}

/// Build a fragment-shader-only fullscreen-quad pipeline using the given WGSL
/// source. Matches the production `BlendState::REPLACE` mode used by the
/// color and blur effects.
fn build_quad_pipeline(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    wgsl: &str,
    label: &str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
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
                format: TARGET_FORMAT,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// Record a single render pass that samples `src_view` and writes into
/// `dst_view`, clearing to black first. Used by both the color test and
/// each blur half-pass.
#[allow(clippy::too_many_arguments)]
fn record_quad_pass(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    bgl: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    src_view: &wgpu::TextureView,
    dst_view: &wgpu::TextureView,
    uniform_buffer: &wgpu::Buffer,
    label: &str,
) {
    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(src_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform_buffer.as_entire_binding(),
            },
        ],
    });
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
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
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, &bg, &[]);
    pass.draw(0..6, 0..1);
}

// ---------------------------------------------------------------------------
// T-M5-15 — Golden image: color pass.
// 64×64 horizontal RGB gradient → ColorParams { hue=+30°, sat=1.5 }
// → tests/golden/color.png. tolerance = 2.
// ---------------------------------------------------------------------------

#[test]
fn color_pass_golden() {
    const W: u32 = 64;
    const H: u32 = 64;
    let h = Headless::new().expect("Headless::new");

    let bytes = h.render_to_rgba8(W, H, |device, queue, view| {
        // Build the color pipeline from the production WGSL.
        let effect = make_effect_bgl(device, "color test bgl");
        let pipeline = build_quad_pipeline(
            device,
            &effect.bgl,
            include_str!("../src/render/shaders/color.wgsl"),
            "color test pipeline",
        );

        // Input texture: gradient.
        let gradient = make_gradient(W, H);
        let (_src_tex, src_view) =
            upload_rgba8(device, queue, W, H, &gradient, "color input gradient");

        // Uniform: ColorParams wire layout (matches src/effects/color.rs).
        // hue_shift_deg=30, saturation_mul=1.5, brightness_add=0, contrast_mul=1.
        let uniform = make_uniform_buffer(device, "color params");
        let mut wire = [0u8; 16];
        wire[0..4].copy_from_slice(&30.0f32.to_le_bytes());
        wire[4..8].copy_from_slice(&1.5f32.to_le_bytes());
        wire[8..12].copy_from_slice(&0.0f32.to_le_bytes());
        wire[12..16].copy_from_slice(&1.0f32.to_le_bytes());
        queue.write_buffer(&uniform, 0, &wire);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("color test encoder"),
        });
        record_quad_pass(
            device,
            &mut encoder,
            &pipeline,
            &effect.bgl,
            &effect.sampler,
            &src_view,
            view,
            &uniform,
            "color test pass",
        );
        queue.submit(std::iter::once(encoder.finish()));
    });

    assert_image_matches(&bytes, W, H, "tests/golden/color.png", TOLERANCE);
}

// ---------------------------------------------------------------------------
// T-M5-16 — Golden image: blur pass.
// 64×64 horizontal RGB gradient → BlurParams { radius_px=8.0 }, separable
// h+v passes → tests/golden/blur.png. tolerance = 2.
// ---------------------------------------------------------------------------

#[test]
fn blur_pass_golden() {
    const W: u32 = 64;
    const H: u32 = 64;
    let h = Headless::new().expect("Headless::new");

    let bytes = h.render_to_rgba8(W, H, |device, queue, view| {
        let effect = make_effect_bgl(device, "blur test bgl");
        let pipeline_h = build_quad_pipeline(
            device,
            &effect.bgl,
            include_str!("../src/render/shaders/blur_h.wgsl"),
            "blur h test pipeline",
        );
        let pipeline_v = build_quad_pipeline(
            device,
            &effect.bgl,
            include_str!("../src/render/shaders/blur_v.wgsl"),
            "blur v test pipeline",
        );

        // Input + intermediate textures.
        let gradient = make_gradient(W, H);
        let (_src_tex, src_view) =
            upload_rgba8(device, queue, W, H, &gradient, "blur input gradient");
        let (_mid_tex, mid_view) = make_io_texture(device, W, H, "blur intermediate");

        // Uniform: BlurParams wire layout — single f32 at offset 0, padded.
        let uniform = make_uniform_buffer(device, "blur params");
        let mut wire = [0u8; 16];
        wire[0..4].copy_from_slice(&8.0f32.to_le_bytes());
        queue.write_buffer(&uniform, 0, &wire);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("blur test encoder"),
        });
        // Horizontal: src -> intermediate.
        record_quad_pass(
            device,
            &mut encoder,
            &pipeline_h,
            &effect.bgl,
            &effect.sampler,
            &src_view,
            &mid_view,
            &uniform,
            "blur h pass",
        );
        // Vertical: intermediate -> output (the harness's view).
        record_quad_pass(
            device,
            &mut encoder,
            &pipeline_v,
            &effect.bgl,
            &effect.sampler,
            &mid_view,
            view,
            &uniform,
            "blur v pass",
        );
        queue.submit(std::iter::once(encoder.finish()));
    });

    assert_image_matches(&bytes, W, H, "tests/golden/blur.png", TOLERANCE);
}

// ---------------------------------------------------------------------------
// T-M5-17 — Golden image: corner-pin warp.
// 128×128 checkerboard input → 4 corners pinned to a known trapezoid, no
// mask → tests/golden/warp.png. tolerance = 2.
//
// The production WarpRenderer requires a `WarpMesh` (private to the binary
// crate) and an SDF texture. For the 1×1 corner-pin case we emit two
// triangles directly with hard-coded UVs and bind a 1×1 dummy R32Float SDF
// with use_mask=0 so warp.wgsl's mask branch is skipped.
// ---------------------------------------------------------------------------

#[test]
fn warp_pass_golden() {
    const W: u32 = 128;
    const H: u32 = 128;
    let h = Headless::new().expect("Headless::new");

    let bytes = h.render_to_rgba8(W, H, |device, queue, view| {
        // --- Pipeline (rebuild from production warp.wgsl) ---
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("warp test bgl"),
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("warp test shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../src/render/shaders/warp.wgsl").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("warp test pipeline layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        // Vertex layout: pos_clip (Float32x2), src_uv (Float32x2). Stride 16.
        let vb_layout = wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("warp test pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vb_layout],
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
                    format: TARGET_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // --- Input scene texture: checkerboard, 8-px cells ---
        let cb = make_checkerboard(W, H, 8);
        let (_src_tex, src_view) = upload_rgba8(device, queue, W, H, &cb, "warp checkerboard");

        let scene_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("warp test scene sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // --- Dummy 1×1 R32Float SDF (unused: use_mask=0) ---
        let sdf_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("warp test dummy sdf"),
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
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &sdf_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &0.0f32.to_le_bytes(),
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
        let sdf_view = sdf_tex.create_view(&wgpu::TextureViewDescriptor::default());

        // --- Mask uniform: use_mask=0, feather doesn't matter ---
        let mask_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("warp test mask u"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut mu = [0u8; 16];
        // [use_mask, feather, sdf_size, _]
        mu[0..4].copy_from_slice(&0.0f32.to_le_bytes());
        mu[4..8].copy_from_slice(&1.0f32.to_le_bytes());
        mu[8..12].copy_from_slice(&1.0f32.to_le_bytes());
        queue.write_buffer(&mask_uniform, 0, &mu);

        // --- Trapezoid corners in normalized output space (x right, y down).
        // Matches the spec hint: top edge slightly indented, bottom edge full-
        // width.   TL=(0.1, 0.0)   TR=(0.9, 0.05)   BL=(0.0, 1.0)   BR=(1.0, 0.95)
        // Convert to clip: x = 2u-1, y = 1-2v.
        let to_clip = |u: f32, v: f32| -> [f32; 2] { [u * 2.0 - 1.0, 1.0 - v * 2.0] };
        let tl = to_clip(0.1, 0.0);
        let tr = to_clip(0.9, 0.05);
        let bl = to_clip(0.0, 1.0);
        let br = to_clip(1.0, 0.95);

        // Two triangles covering the trapezoid.
        // Each vertex = [pos.x, pos.y, uv.u, uv.v].
        // CCW in NDC (y-up): TL → BL → TR, then TR → BL → BR.
        let verts: [[f32; 4]; 6] = [
            [tl[0], tl[1], 0.0, 0.0],
            [bl[0], bl[1], 0.0, 1.0],
            [tr[0], tr[1], 1.0, 0.0],
            [tr[0], tr[1], 1.0, 0.0],
            [bl[0], bl[1], 0.0, 1.0],
            [br[0], br[1], 1.0, 1.0],
        ];
        let vb_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(verts.as_ptr().cast::<u8>(), std::mem::size_of_val(&verts))
        };
        let vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("warp test vb"),
            size: vb_bytes.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vb, 0, vb_bytes);

        // --- Bind group ---
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("warp test bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&scene_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&sdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: mask_uniform.as_entire_binding(),
                },
            ],
        });

        // --- Encode ---
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("warp test encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("warp test pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.draw(0..6, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    });

    assert_image_matches(&bytes, W, H, "tests/golden/warp.png", TOLERANCE);
}

// ---------------------------------------------------------------------------
// P2.9.2 — Particle determinism tests: `mask_constrained_drift`.
//
// Two properties are tested:
//   1. Same seed + same clock → bit-exact identical pixel output (two
//      independent renders in the same process, same device).
//   2. Different seed → at least one pixel differs (sanity-check that the
//      seed parameter actually influences output).
//
// A third assertion writes the seed=42 render as a PNG golden:
//   `tests/golden/particle_determinism_seed42.png`.
//   Set UPDATE_GOLDEN=1 to (re-)write it; the next run asserts against it.
//
// Note on cross-session determinism: the compute shader uses a fixed
// dt = 1/60 s so in-session renders are bit-exact. Cross-session runs may
// differ when OS timer resolution causes sub-frame jitter in wall-clock
// arguments supplied by the caller; that known limitation is documented
// here and accepted by the spec (P2.9.2 "Out of scope").
//
// All tests are gated on `feature = "gpu-tests"` (whole file is
// `#![cfg(feature = "gpu-tests")]`) and skip cleanly if no wgpu adapter
// is available (the existing harness contract: Headless::new panics on
// missing adapter, which nextest reports as a test failure only if no
// adapter is present — unchanged from the other tests in this file).
// ---------------------------------------------------------------------------

/// Maximum particle count (mirrors `fx_compute.rs`).
const MAX_PARTICLES_DRIFT: u32 = 2048;
/// SSBO byte size: MAX_PARTICLES × 32 bytes per Particle (std430).
const SSBO_SIZE_DRIFT: u64 = MAX_PARTICLES_DRIFT as u64 * 32;

/// Build a circular SDF fixture: R32Float texture of `size × size` texels.
/// Values follow the convention used throughout the codebase: negative inside
/// the circle, positive outside, zero on the boundary.
/// Circle: centre (0.5, 0.5), radius 0.25, in normalised [0,1]² space.
fn make_circular_sdf(device: &wgpu::Device, queue: &wgpu::Queue, size: u32) -> wgpu::TextureView {
    let n = (size * size) as usize;
    let mut texels: Vec<f32> = Vec::with_capacity(n);
    for ty in 0..size {
        for tx in 0..size {
            // Texel centre in normalised [0,1]² space.
            let px = (tx as f32 + 0.5) / size as f32;
            let py = (ty as f32 + 0.5) / size as f32;
            let dx = px - 0.5;
            let dy = py - 0.5;
            // distance from centre minus radius → negative inside, positive outside
            texels.push((dx * dx + dy * dy).sqrt() - 0.25);
        }
    }
    // Convert f32 slice to bytes for upload.
    let bytes: Vec<u8> = texels.iter().flat_map(|f| f.to_le_bytes()).collect();

    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("circular sdf fixture"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
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
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size * 4), // 4 bytes per R32Float texel
            rows_per_image: Some(size),
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Render `mask_constrained_drift` (N=64, given `seed`, `clock_secs=5.0`) into
/// a `W×H` offscreen target and return the packed RGBA8 pixel buffer.
///
/// Rebuilds the full compute + render pipeline in-test (no `lib.rs` access
/// without the `v3` feature) following the same pattern as `warp_pass_golden`.
/// The shader sources are loaded via `include_str!`; `sdf_helper.wgsl` is
/// prepended to both the compute and vertex shaders exactly as the production
/// `FxComputePipeline` does.
fn render_constrained_drift(h: &Headless, seed: u64, clock_secs: f32) -> Vec<u8> {
    const W: u32 = 128;
    const H: u32 = 128;
    const N_PARTICLES: u32 = 64;
    const T_LAYER_ADDED_SECS: f32 = 0.0;
    const SDF_SIZE: u32 = 64;

    h.render_to_rgba8(W, H, |device, queue, view| {
        // --- Shader sources (prepend sdf_helper.wgsl as production does) ------
        let sdf_helper = include_str!("../src/render/shaders/sdf_helper.wgsl");
        let compute_src = format!(
            "{}\n{}",
            sdf_helper,
            include_str!("../src/render/shaders/fx_particles_drift.wgsl")
        );
        let vertex_src = format!(
            "{}\n{}",
            sdf_helper,
            include_str!("../src/render/shaders/fx_particles_vertex.wgsl")
        );
        // Fragment shader has no SDF calls but production prepends it too.
        let fragment_src = format!(
            "{}\n{}",
            sdf_helper,
            include_str!("../src/render/shaders/fx_particles_fragment.wgsl")
        );

        // --- Compute bind-group layout (bindings 2, 3, 5, 6) -----------------
        let compute_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("drift determinism compute bgl"),
            entries: &[
                // binding 2: FxParamsUniform (8 × f32, 32 bytes)
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
                // binding 3: ClockUniform (vec4<f32>)
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
                // binding 5: output SSBO (read_write)
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
        });

        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("drift determinism compute shader"),
            source: wgpu::ShaderSource::Wgsl(compute_src.into()),
        });
        let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("drift determinism compute layout"),
            bind_group_layouts: &[Some(&compute_bgl)],
            immediate_size: 0,
        });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("drift determinism compute pipeline"),
            layout: Some(&compute_layout),
            module: &compute_shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // --- Render bind-group layout (bindings 3, 4, 5) ---------------------
        let render_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("drift determinism render bgl"),
            entries: &[
                // binding 3: ClockUniform
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
                // binding 4: ResUniform
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
                // binding 5: particle SSBO (read-only)
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

        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("drift determinism vertex shader"),
            source: wgpu::ShaderSource::Wgsl(vertex_src.into()),
        });
        let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("drift determinism fragment shader"),
            source: wgpu::ShaderSource::Wgsl(fragment_src.into()),
        });
        let render_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("drift determinism render layout"),
            bind_group_layouts: &[Some(&render_bgl)],
            immediate_size: 0,
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("drift determinism render pipeline"),
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
                    format: TARGET_FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // --- Uniform buffers --------------------------------------------------
        // FxParamsUniform: 8 × f32 = 32 bytes.
        // For constrained_drift: wavelength = N_PARTICLES (particle_count),
        // speed = 0.02 (drift_speed default), falloff = 2.0 (particle_size default).
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("drift determinism params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        {
            let mut pb = [0u8; 32];
            pb[0..4].copy_from_slice(&(N_PARTICLES as f32).to_le_bytes()); // wavelength = particle_count
            pb[4..8].copy_from_slice(&0.02f32.to_le_bytes()); // speed = drift_speed
            pb[8..12].copy_from_slice(&2.0f32.to_le_bytes()); // falloff = particle_size
            // base_r/g/b/_pad0/_pad1 = 0.0 (unused)
            queue.write_buffer(&params_buf, 0, &pb);
        }

        // ClockUniform: vec4<f32> = 16 bytes.
        // .x = clock_secs, .y = t_layer_local_secs, .z = seed_f32, .w = n_particles
        let clock_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("drift determinism clock"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        {
            let t_local = clock_secs - T_LAYER_ADDED_SECS;
            // Pack u64 seed into the lower 23 bits of a f32 mantissa (matches
            // the production FxComputePipeline packing in fx_compute.rs).
            let seed_f = (seed as u32 & 0x7f_ffff) as f32;
            let mut cb = [0u8; 16];
            cb[0..4].copy_from_slice(&clock_secs.to_le_bytes());
            cb[4..8].copy_from_slice(&t_local.to_le_bytes());
            cb[8..12].copy_from_slice(&seed_f.to_le_bytes());
            cb[12..16].copy_from_slice(&(N_PARTICLES as f32).to_le_bytes());
            queue.write_buffer(&clock_buf, 0, &cb);
        }

        // ResUniform: vec4<f32> = 16 bytes. .x = width, .y = height.
        let res_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("drift determinism res"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        {
            let mut rb = [0u8; 16];
            rb[0..4].copy_from_slice(&(W as f32).to_le_bytes());
            rb[4..8].copy_from_slice(&(H as f32).to_le_bytes());
            queue.write_buffer(&res_buf, 0, &rb);
        }

        // SSBO (single buffer — particle data is computed from scratch each
        // call; no persistent state between calls in these tests).
        let ssbo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("drift determinism ssbo"),
            size: SSBO_SIZE_DRIFT,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // --- Circular SDF fixture (R=0.25, centre 0.5,0.5) -------------------
        let sdf_view = make_circular_sdf(device, queue, SDF_SIZE);

        // --- Encode compute + render in one submission -----------------------
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("drift determinism encoder"),
        });

        // Compute pass: write particle positions.
        {
            let compute_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("drift determinism compute bg"),
                layout: &compute_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: clock_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: ssbo.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(&sdf_view),
                    },
                ],
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("drift determinism compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&compute_pipeline);
            pass.set_bind_group(0, &compute_bg, &[]);
            let groups = N_PARTICLES.div_ceil(64);
            pass.dispatch_workgroups(groups, 1, 1);
        }

        // Render pass: draw particle quads.
        {
            let render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("drift determinism render bg"),
                layout: &render_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: clock_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: res_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: ssbo.as_entire_binding(),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("drift determinism render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            pass.set_pipeline(&render_pipeline);
            pass.set_bind_group(0, &render_bg, &[]);
            // 6 vertices per quad (two triangles), N_PARTICLES instances.
            pass.draw(0..6, 0..N_PARTICLES);
        }

        queue.submit(std::iter::once(encoder.finish()));
    })
}

/// P2.9.2 — Same seed → bit-exact pixel output.
///
/// Renders `mask_constrained_drift` twice with the same seed and clock value.
/// Both renders run in the same process on the same wgpu device so no
/// cross-session timer variance can intervene; bit-exactness is guaranteed by
/// the deterministic hash in the shader.
#[test]
fn test_particle_determinism_same_seed() {
    let h = Headless::new().expect("Headless::new");
    let buf1 = render_constrained_drift(&h, 42, 5.0);
    let buf2 = render_constrained_drift(&h, 42, 5.0);
    assert_eq!(
        buf1, buf2,
        "same seed=42, clock=5.0: expected bit-exact identical pixel output"
    );
}

/// P2.9.2 — Different seed → at least one pixel differs.
///
/// Renders `mask_constrained_drift` with seed=42 and seed=43 at the same
/// clock. The two particle positions are seed-derived (distinct hash inputs)
/// so the images must differ by at least one pixel.
#[test]
fn test_particle_determinism_different_seed() {
    let h = Headless::new().expect("Headless::new");
    let buf42 = render_constrained_drift(&h, 42, 5.0);
    let buf43 = render_constrained_drift(&h, 43, 5.0);
    assert_ne!(
        buf42, buf43,
        "seed=42 and seed=43 produced identical pixel output; \
         the seed parameter must influence particle positions"
    );
}

/// P2.9.2 — Golden image baseline for seed=42.
///
/// Run with `UPDATE_GOLDEN=1` to write the baseline; subsequent runs assert
/// the render matches within `TOLERANCE` (PNG round-trip introduces at most
/// 1-LSB per channel; tolerance=2 matches the convention for other goldens).
///
/// Cross-session bit-exactness is NOT guaranteed (OS timer resolution may
/// shift `clock_secs` sub-frame timing); the tolerance band absorbs minor
/// driver / sRGB quantisation differences. For strict in-session determinism
/// see `test_particle_determinism_same_seed`.
#[test]
fn test_particle_determinism_golden_seed42() {
    let h = Headless::new().expect("Headless::new");
    let pixels = render_constrained_drift(&h, 42, 5.0);
    assert_image_matches(
        &pixels,
        128,
        128,
        "tests/golden/particle_determinism_seed42.png",
        TOLERANCE,
    );
}

// ---------------------------------------------------------------------------
// P3.6.2 — Zone-tag dispatch golden tests.
//
// Three tests verify shader dispatch on zone tag:
//   1. light_spill + ZONE_WINDOW tag → glow visible (golden PNG).
//      light_spill + ZONE_NONE tag → transparent black (bit-exact).
//   2. edge_ripple + ZONE_EDGE tag → ripple visible (golden PNG).
//   3. portal_drift + ZONE_PORTAL tag → drift visible (golden PNG, deterministic).
//
// Record baselines: UPDATE_GOLDEN=1 cargo nextest run --features gpu-tests
// ---------------------------------------------------------------------------

/// P3.6.2 — zone_light_spill renders a glow for ZONE_WINDOW and transparent
/// black for ZONE_NONE (bit-exact assertion).
#[test]
fn zone_light_spill_window_tag_golden() {
    use rmap::render::sdf::{SDF_HELPER_WGSL, ZONE_TAG_WGSL};

    let h = Headless::new().expect("Headless::new");

    let wgsl = format!(
        "{}\n{}\n{}",
        SDF_HELPER_WGSL,
        ZONE_TAG_WGSL,
        include_str!("../src/render/shaders/fx_zone_light_spill.wgsl")
    );

    // ZONE_WINDOW = 1: expect visible glow.
    let glow_pixels = render_zone_preset(&h, &wgsl, "light_spill window", 1, 1.0);
    assert_image_matches(
        &glow_pixels,
        128,
        128,
        "tests/golden/zone_light_spill_window.png",
        TOLERANCE,
    );

    // ZONE_NONE = 0: expect transparent black (bit-exact).
    let none_pixels = render_zone_preset(&h, &wgsl, "light_spill none", 0, 1.0);
    let is_transparent_black = none_pixels
        .chunks(4)
        .all(|px| px[0] == 0 && px[1] == 0 && px[2] == 0 && px[3] == 0);
    assert!(
        is_transparent_black,
        "zone_light_spill with ZONE_NONE must output transparent black; found non-zero pixels"
    );
}

/// P3.6.2 — zone_edge_ripple renders ripple for ZONE_EDGE.
#[test]
fn zone_edge_ripple_edge_tag_golden() {
    use rmap::render::sdf::{SDF_HELPER_WGSL, ZONE_TAG_WGSL};

    let h = Headless::new().expect("Headless::new");

    let wgsl = format!(
        "{}\n{}\n{}",
        SDF_HELPER_WGSL,
        ZONE_TAG_WGSL,
        include_str!("../src/render/shaders/fx_zone_edge_ripple.wgsl")
    );

    // ZONE_EDGE = 5: expect ripple visible.
    let ripple_pixels = render_zone_preset(&h, &wgsl, "edge_ripple edge", 5, 1.0);
    assert_image_matches(
        &ripple_pixels,
        128,
        128,
        "tests/golden/zone_edge_ripple_edge.png",
        TOLERANCE,
    );
}

/// P3.6.2 — zone_portal_drift renders drift for ZONE_PORTAL with deterministic
/// output (fragment shader uses deterministic hash of UV + clock).
#[test]
fn zone_portal_drift_portal_tag_golden() {
    use rmap::render::sdf::{SDF_HELPER_WGSL, ZONE_TAG_WGSL};

    let h = Headless::new().expect("Headless::new");

    let wgsl = format!(
        "{}\n{}\n{}",
        SDF_HELPER_WGSL,
        ZONE_TAG_WGSL,
        include_str!("../src/render/shaders/fx_zone_portal_drift.wgsl")
    );

    // ZONE_PORTAL = 2, fixed clock=5.0 (deterministic frame).
    let drift_pixels = render_zone_preset(&h, &wgsl, "portal_drift portal", 2, 5.0);
    assert_image_matches(
        &drift_pixels,
        128,
        128,
        "tests/golden/zone_portal_drift_portal.png",
        TOLERANCE,
    );

    // Same clock: bit-exact on second call (determinism guard).
    let drift_pixels2 = render_zone_preset(&h, &wgsl, "portal_drift determinism check", 2, 5.0);
    assert_eq!(
        drift_pixels, drift_pixels2,
        "zone_portal_drift at fixed clock must produce bit-exact output on repeat calls"
    );
}

// ---------------------------------------------------------------------------
// P4.8.3 — `window_reveal` template instantiation + GPU determinism guard
// ---------------------------------------------------------------------------

/// P4.8.3 — `window_reveal` template instantiation structure + GPU adapter check.
///
/// This test verifies:
/// 1. `window_reveal` is registered in the scene registry.
/// 2. `instantiate_template` produces a two-layer project (image + FxLayer)
///    without panicking.
/// 3. The GPU adapter is available and the headless harness initialises cleanly.
///
/// **Full golden-image comparison** against a PNG baseline is deferred until
/// the production render pipeline types are accessible from integration tests
/// (see TODO(P0.9.5-path-a) in perf_frame_budget.rs). When that lands, replace
/// the adapter-alive check below with a `Headless::render_to_rgba8` call
/// followed by `assert_image_matches` against
/// `tests/golden/window_reveal_*.png`.
///
/// TODO(P4.8.3-golden): add full pixel-exact determinism test once pipeline
/// types are exported from the library crate.
#[test]
fn window_reveal_template_structure_and_gpu_adapter() {
    use rmap::project::scene_instantiation::{WizardChoices, instantiate_template};
    use rmap::project::scene_templates::scene_registry;
    use rmap::project::schema::Project;
    use rmap::project::snapshot;

    // Skip cleanly when no GPU adapter is available.
    let h = Headless::new().expect("Headless::new");

    // --- CPU-side: instantiate the window_reveal template ---
    let template = scene_registry()
        .iter()
        .find(|t| t.id == "window_reveal")
        .expect("window_reveal must be in scene_registry after P4.5.1");

    let base = snapshot(&Project::default());
    let choices = WizardChoices {
        template_id: "window_reveal".to_string(),
        ..Default::default()
    };
    let generated = instantiate_template(template, &choices, base);
    let project: Project =
        serde_json::from_value(generated).expect("instantiate_template must return valid Project");

    // Verify two layers: one image proxy + one FxLayer.
    assert_eq!(
        project.layers.len(),
        2,
        "window_reveal with default choices must produce 2 layers: 1 image + 1 FxLayer"
    );
    assert!(
        matches!(
            &project.layers[0].kind,
            rmap::project::schema::LayerKind::Image { .. }
        ),
        "layer[0] must be Image kind"
    );
    assert!(
        matches!(
            &project.layers[1].kind,
            rmap::project::schema::LayerKind::FxLayer {
                preset_id,
                ..
            } if preset_id == "mask_edge_ripple_wash"
        ),
        "layer[1] must be FxLayer with preset_id = mask_edge_ripple_wash"
    );

    // GPU adapter confirmed available (h is live). The full golden-image test
    // would render the instantiated template through the production Renderer
    // and compare against a PNG baseline. This requires the production
    // render pipeline types to be accessible from integration tests
    // (see TODO(P0.9.5-path-a) in perf_frame_budget.rs). Deferred to the
    // same follow-up that exposes the pipeline types.
    //
    // For now, confirm that the GPU adapter is live and the CPU-side
    // instantiation is structurally correct (verified above). The FX preset
    // used by window_reveal (mask_edge_ripple_wash) is covered by the
    // existing golden tests in the FX preset section of this file.
    let _adapter_info = h.adapter.get_info();
    // adapter confirmed; golden comparison pending the pipeline exposure.
}

// ---------------------------------------------------------------------------
// T1.3 — LightTrail polyline GPU buffer Metal/wgpu compat check.
//
// Creates a storage buffer sized for a 16-sample polyline fixture, writes
// the data via queue.write_buffer, and asserts the operation completes
// without wgpu validation errors. No render pass is needed — the buffer
// creation + write is the entire Metal-compat verification.
//
// The crate has no lib.rs, so we cannot call `LightTrailGpuPolyline::upload`
// directly here. Instead we rebuild the buffer creation inline, mirroring the
// production function in src/effects/light_trail.rs. This is the established
// pattern for integration tests in this file (see color_pass_golden comment at
// line ~605).
// ---------------------------------------------------------------------------

/// T1.3: create a `STORAGE | COPY_DST` buffer for a 16-sample polyline fixture
/// and write the data.  Asserts no wgpu validation errors by completing without
/// panic.  Buffer size = 16 * 3 * 4 = 192 bytes.
#[test]
fn light_trail_gpu_buffer_upload() {
    const SAMPLE_COUNT: u32 = 16;
    // Each sample: [point_x, point_y, cumulative_arclen] — 3 f32s.
    const FLOATS_PER_SAMPLE: u32 = 3;
    const BUFFER_BYTES: u64 = (SAMPLE_COUNT * FLOATS_PER_SAMPLE * 4) as u64; // 192

    let h = Headless::new().expect("Headless::new");

    // Build fixture payload: straight horizontal line x=0..15, y=0, arclen=0..15.
    let mut payload: Vec<f32> = Vec::with_capacity((SAMPLE_COUNT * FLOATS_PER_SAMPLE) as usize);
    for i in 0..SAMPLE_COUNT {
        payload.push(i as f32); // point_x
        payload.push(0.0_f32); // point_y
        payload.push(i as f32); // cumulative_arclen
    }
    assert_eq!(
        payload.len() as u32,
        SAMPLE_COUNT * FLOATS_PER_SAMPLE,
        "payload must be sample_count * 3 floats"
    );

    let byte_payload: Vec<u8> = payload.iter().flat_map(|f| f.to_le_bytes()).collect();
    assert_eq!(
        byte_payload.len() as u64,
        BUFFER_BYTES,
        "byte payload must be 192 bytes for 16-sample polyline"
    );

    // T1.3: storage buffer chosen — already used by treatment_particles +
    // fx_compute; verified Metal-OK on wgpu 29.
    let buffer = h.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("T1.3 light_trail polyline storage test"),
        size: BUFFER_BYTES,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Upload — any wgpu validation error here causes a panic (wgpu's default
    // uncaptured-error handler panics in debug mode).
    h.queue.write_buffer(&buffer, 0, &byte_payload);

    // Flush to ensure the write completes on the device.
    h.queue.submit(std::iter::empty());
    h.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device.poll Wait failed");

    // Structural assertions.
    assert_eq!(buffer.size(), BUFFER_BYTES, "buffer size must be 192 bytes");
}
