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

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("rmap headless test device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
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
        let padded_bytes_per_row = align_up(unpadded_bytes_per_row, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
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
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, got.to_vec())
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
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = match ImageBuffer::from_raw(width, height, got.to_vec()) {
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
