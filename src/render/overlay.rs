//! Editor-overlay pipeline: paints per-layer bounding rectangles and
//! per-warp mask polygon outlines onto the projector swapchain. Runs
//! after gamma with `LoadOp::Load`, so it composites on top of the
//! finished frame.
//!
//! Why this exists: projection mapping is a "look at the wall" workflow
//! — the operator drags vertices in the control window while watching
//! the actual surface to see where each layer is mapped. Without an
//! overlay, the projector shows only the gamma-corrected scene with no
//! indication of which rectangle belongs to which layer or where the
//! mask polygon's edges live, so they have to guess.
//!
//! The overlay is toggled by `OutputState::show_editor_overlay`; flip it
//! off with the `O` key before the show. Defaults to ON because new
//! operators benefit from the feedback far more than they're hurt by it.
//!
//! Lines are expanded CPU-side into screen-aligned thin triangle strips
//! (configurable thickness in pixels). `wgpu`'s native `LineList`
//! topology renders one device pixel wide regardless of resolution,
//! which is invisible at projector resolution and across a venue.

use wgpu::util::DeviceExt;

use crate::effects::Effect;
use crate::modulators::Modulator;
use crate::project::schema::{LayerConfig, Project, WarpMesh};
use crate::render::warp::solve_homography;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OverlayVertex {
    pub pos_clip: [f32; 2],
    pub color: [f32; 4],
}

fn verts_as_bytes(v: &[OverlayVertex]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), std::mem::size_of_val(v)) }
}

/// One line segment in clip space (NDC, +X right, +Y up, range [-1, 1]).
#[derive(Clone, Copy, Debug)]
pub struct OverlayLine {
    pub a_clip: [f32; 2],
    pub b_clip: [f32; 2],
    pub color: [f32; 4],
    /// Half-width in pixels. The segment becomes a `2 * thickness_px`
    /// wide screen-aligned strip.
    pub thickness_px: f32,
}

pub struct OverlayPipeline {
    pipeline: wgpu::RenderPipeline,
    /// Reused across frames; reallocated only when the line count grows
    /// past the existing capacity. Bytes are overwritten via
    /// [`wgpu::Queue::write_buffer`] each frame.
    vertex_buffer: wgpu::Buffer,
    /// Capacity in vertices; not bytes.
    vertex_capacity: usize,
}

impl OverlayPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("overlay.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/overlay.wgsl").into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overlay layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let vb_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<OverlayVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay pipeline"),
            layout: Some(&layout),
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
                    format,
                    // Premultiplied alpha blend: src already has rgb*alpha
                    // baked in (see overlay.wgsl).
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // Start with room for a few hundred lines so the first non-empty
        // frame doesn't reallocate. 4 layers (4 lines each) + a 24-vertex
        // mask polygon = ~40 lines × 6 verts/line = 240 vertices; round up.
        let initial_capacity = 1024;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("overlay vb"),
            size: (initial_capacity * std::mem::size_of::<OverlayVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            vertex_capacity: initial_capacity,
        }
    }

    /// Expand `lines` into screen-aligned thin strips and draw on top of
    /// `dst`. `surface_size` is the destination's width/height in
    /// pixels — needed to convert pixel-space line thickness into
    /// clip-space deltas, otherwise the strips warp with the surface
    /// aspect.
    ///
    /// `LoadOp::Load`: the gamma pass already wrote the frame; we paint
    /// over it.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        surface_size: (u32, u32),
        lines: &[OverlayLine],
    ) {
        if lines.is_empty() {
            return;
        }
        let (sw, sh) = surface_size;
        if sw == 0 || sh == 0 {
            return;
        }
        let inv_w = 1.0 / sw as f32;
        let inv_h = 1.0 / sh as f32;
        let mut verts: Vec<OverlayVertex> = Vec::with_capacity(lines.len() * 6);
        for line in lines {
            // pixel-space endpoints
            let a_px = (
                (line.a_clip[0] * 0.5 + 0.5) * sw as f32,
                (line.a_clip[1] * -0.5 + 0.5) * sh as f32,
            );
            let b_px = (
                (line.b_clip[0] * 0.5 + 0.5) * sw as f32,
                (line.b_clip[1] * -0.5 + 0.5) * sh as f32,
            );
            let dx = b_px.0 - a_px.0;
            let dy = b_px.1 - a_px.1;
            let len = (dx * dx + dy * dy).sqrt().max(1e-4);
            let t = line.thickness_px.max(0.5);
            // perpendicular in pixel space, scaled to half-thickness
            let nx_px = -dy / len * t;
            let ny_px = dx / len * t;
            let to_clip = |p: (f32, f32)| -> [f32; 2] {
                [(p.0 * inv_w) * 2.0 - 1.0, 1.0 - (p.1 * inv_h) * 2.0]
            };
            let p0 = to_clip((a_px.0 + nx_px, a_px.1 + ny_px));
            let p1 = to_clip((a_px.0 - nx_px, a_px.1 - ny_px));
            let p2 = to_clip((b_px.0 + nx_px, b_px.1 + ny_px));
            let p3 = to_clip((b_px.0 - nx_px, b_px.1 - ny_px));
            // Two triangles: (p0, p1, p2) and (p2, p1, p3)
            verts.push(OverlayVertex {
                pos_clip: p0,
                color: line.color,
            });
            verts.push(OverlayVertex {
                pos_clip: p1,
                color: line.color,
            });
            verts.push(OverlayVertex {
                pos_clip: p2,
                color: line.color,
            });
            verts.push(OverlayVertex {
                pos_clip: p2,
                color: line.color,
            });
            verts.push(OverlayVertex {
                pos_clip: p1,
                color: line.color,
            });
            verts.push(OverlayVertex {
                pos_clip: p3,
                color: line.color,
            });
        }

        if verts.len() > self.vertex_capacity {
            let new_cap = verts.len().next_power_of_two().max(1024);
            self.vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("overlay vb (grown)"),
                contents: verts_as_bytes(&verts),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
            self.vertex_capacity = new_cap;
        } else {
            queue.write_buffer(&self.vertex_buffer, 0, verts_as_bytes(&verts));
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("overlay pass"),
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
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..(verts.len() as u32), 0..1);
    }
}

/// Per-layer outline color. Cycles through an 8-entry palette by index;
/// kept in sync with `windows::scene_editor::layer_color` so the colour
/// shown in the control-panel preview matches the colour painted on the
/// projector.
fn layer_color(idx: usize) -> [f32; 4] {
    const PALETTE: [[f32; 4]; 8] = [
        [1.00, 0.43, 0.51, 1.0],
        [0.43, 0.78, 1.00, 1.0],
        [0.71, 0.94, 0.51, 1.0],
        [1.00, 0.78, 0.35, 1.0],
        [0.75, 0.51, 0.96, 1.0],
        [0.43, 0.90, 0.78, 1.0],
        [0.96, 0.59, 0.31, 1.0],
        [0.71, 0.71, 0.86, 1.0],
    ];
    PALETTE[idx % PALETTE.len()]
}

/// Read the layer's effective static `(translate, scale, rotate_deg)`.
/// Mirrors `windows::scene_editor::effective_static_transform` — kept in
/// this module to avoid the windows → render dependency direction.
fn effective_static_transform(layer: &LayerConfig) -> ([f32; 2], [f32; 2], f32) {
    for e in layer.effects.iter() {
        if let Effect::Transform {
            translate,
            scale_x,
            scale_y,
            rotate_deg,
        } = e
        {
            let s_x = match scale_x {
                Modulator::Static(v) => *v,
                _ => 1.0,
            };
            let s_y = match scale_y {
                Modulator::Static(v) => *v,
                _ => 1.0,
            };
            let rot = match rotate_deg {
                Modulator::Static(v) => *v,
                _ => 0.0,
            };
            return (*translate, [s_x, s_y], rot);
        }
    }
    ([0.0, 0.0], [1.0, 1.0], 0.0)
}

/// Convert normalized output-space (x right, y down, [0,1]²) to clip
/// space (x right, y up, [-1,1]²). Same convention as
/// `render::warp::clip_from_normalized_output`.
fn norm_to_clip(n: [f32; 2]) -> [f32; 2] {
    [n[0] * 2.0 - 1.0, 1.0 - n[1] * 2.0]
}

/// Forward-map a point from a layer's pre-warp space (y-down [0,1]²,
/// the same space the warp shader samples via `t_scene`) into the
/// projector's surface space (also y-down [0,1]²) using the **layer's
/// own warp mesh**.
///
/// v4: each layer owns its warp; the mapping is per-layer so the
/// outline of layer N follows layer N's deformation. `t`/`tx`/`ty` are
/// unclamped so the homography extrapolates continuously past the
/// `[0,1]²` cell domain — gives a smooth outline when a layer is
/// scaled or translated past the unit square.
fn warp_source_to_surface(p_src: [f32; 2], warp: &WarpMesh) -> [f32; 2] {
    if !p_src[0].is_finite() || !p_src[1].is_finite() {
        return p_src;
    }
    let rows = warp.rows as usize;
    let cols = warp.cols as usize;
    if rows == 0 || cols == 0 || warp.grid.len() != rows + 1 {
        return p_src;
    }
    let gx = p_src[0] * cols as f32;
    let gy = p_src[1] * rows as f32;
    let ix = (gx.floor() as i32).clamp(0, cols as i32 - 1) as usize;
    let iy = (gy.floor() as i32).clamp(0, rows as i32 - 1) as usize;
    let tx = gx - ix as f32;
    let ty = gy - iy as f32;
    if warp.grid[iy].len() <= ix + 1 || warp.grid[iy + 1].len() <= ix + 1 {
        return p_src;
    }
    let dst = [
        warp.grid[iy][ix],
        warp.grid[iy][ix + 1],
        warp.grid[iy + 1][ix + 1],
        warp.grid[iy + 1][ix],
    ];
    let src_unit = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    if let Some(h) = solve_homography(src_unit, dst) {
        let v = h * glam::Vec3::new(tx, ty, 1.0);
        let w = v.z.abs().max(1e-8);
        return [v.x / w, v.y / w];
    }
    p_src
}

/// Build the per-frame overlay line list: one rotation-aware bounding
/// rectangle per enabled layer plus one polygon outline per warp that
/// has a mask. `selected_layer` picks which layer (if any) to draw at
/// double thickness so the selection pops on the projector too.
///
/// Coordinates: layer corners come out of the same NDC-space math the
/// vertex shader runs (unit quad → scale → rotate → translate*2 with
/// y-flipped to match the schema's y-down convention). Mask polygons
/// are stored in normalized output-space already; just convert to clip.
pub fn build_overlay_lines(project: &Project, selected_layer: Option<usize>) -> Vec<OverlayLine> {
    let mut lines = Vec::new();

    // Per-layer outlines: project the four edges through the same warp
    // the shader uses, so the box on the wall tracks the projected
    // content under corner-pin and mesh deformation. Each edge is
    // sampled at `EDGE_SAMPLES + 1` points and emitted as that many
    // straight segments — enough to follow the bow of a 4-point
    // perspective without spamming the vertex buffer.
    const EDGE_SAMPLES: usize = 16;
    for (idx, layer) in project.layers.iter().enumerate() {
        if !layer.enabled {
            continue;
        }
        let (translate, scale, rotate_deg) = effective_static_transform(layer);
        // Layer center + half-extents in normalized output (= source)
        // space, y-down. The same place the shader samples the layer
        // FBO from before warping — no `* 2` here, that doubling only
        // exists for the NDC-space matrix in transform.rs.
        let cx = 0.5 + translate[0];
        let cy = 0.5 + translate[1];
        let half = [scale[0].abs() * 0.5, scale[1].abs() * 0.5];
        // Negate sin to match the y-down convention (a math-positive
        // rotation in the y-up shader appears CCW on the wall; do the
        // same on the y-down overlay coords).
        let rad = rotate_deg.to_radians();
        let cos_r = rad.cos();
        let sin_r = -rad.sin();
        let signs: [(f32, f32); 4] = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        let corners_src: [[f32; 2]; 4] = std::array::from_fn(|i| {
            let (sx, sy) = signs[i];
            let lx = sx * half[0];
            let ly = sy * half[1];
            let rx = lx * cos_r - ly * sin_r;
            let ry = lx * sin_r + ly * cos_r;
            [cx + rx, cy + ry]
        });
        let selected = selected_layer == Some(idx);
        let mut color = layer_color(idx);
        if !selected {
            color[3] = 0.75;
        }
        let thickness_px = if selected { 4.0 } else { 2.5 };
        for edge in 0..4 {
            let a = corners_src[edge];
            let b = corners_src[(edge + 1) % 4];
            let mut prev = warp_source_to_surface(a, &layer.warp);
            for i in 1..=EDGE_SAMPLES {
                let t = i as f32 / EDGE_SAMPLES as f32;
                let p_src = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                let p_surf = warp_source_to_surface(p_src, &layer.warp);
                lines.push(OverlayLine {
                    a_clip: norm_to_clip(prev),
                    b_clip: norm_to_clip(p_surf),
                    color,
                    thickness_px,
                });
                prev = p_surf;
            }
        }
    }

    // Mask polygons: white, slightly translucent so they don't fully
    // hide content underneath. One closed loop per layer with a mask.
    for layer in project.layers.iter() {
        if !layer.enabled {
            continue;
        }
        let n = layer.warp.mask_polygon.len();
        if n < 2 {
            continue;
        }
        let color = [1.0, 1.0, 1.0, 0.85];
        for i in 0..n {
            let a = norm_to_clip(layer.warp.mask_polygon[i]);
            let b = norm_to_clip(layer.warp.mask_polygon[(i + 1) % n]);
            lines.push(OverlayLine {
                a_clip: a,
                b_clip: b,
                color,
                thickness_px: 2.0,
            });
        }
    }

    lines
}
