// `needless_range_loop` triggers on Gauss-Jordan inner loops that
// index two parallel rows of the augmented matrix (`m[col][j]` +
// `m[row][j]`), which can't cleanly become an iterator. The render
// API also takes the standard wgpu-multi-borrow shape.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

//! Warp mesh geometry + mask sampling (T-M5-03 … T-M5-06).

use glam::{Mat3, Vec2, Vec3};
use wgpu::util::DeviceExt;

use crate::project::schema::WarpMesh;
use crate::render::sdf::{self, SDF_SIZE};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct WarpVertex {
    pos_clip: [f32; 2],
    src_uv: [f32; 2],
}

fn verts_as_bytes(v: &[WarpVertex]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), std::mem::size_of_val(v)) }
}

/// Solve 8×8 linear system (augmented column last).
fn gaussian_solve(mut m: [[f32; 9]; 8]) -> Option<[f32; 8]> {
    const N: usize = 8;
    for col in 0..N {
        let mut piv = col;
        while piv < N && m[piv][col].abs() < 1e-10 {
            piv += 1;
        }
        if piv >= N {
            return None;
        }
        if piv != col {
            m.swap(piv, col);
        }
        let div = m[col][col];
        for j in col..=N {
            m[col][j] /= div;
        }
        for row in 0..N {
            if row == col {
                continue;
            }
            let f = m[row][col];
            if f.abs() < 1e-12 {
                continue;
            }
            for j in col..=N {
                m[row][j] -= f * m[col][j];
            }
        }
    }
    let mut x = [0f32; 8];
    for i in 0..N {
        x[i] = m[i][N];
    }
    Some(x)
}

/// Homography H with h₃₃ = 1 mapping homogeneous src (u,v,1) to homogeneous dst so that
/// x = (h₁₁u + h₁₂v + h₁₃) / (h₃₁u + h₃₂v + 1), y analogous.
pub fn solve_homography(src: [[f32; 2]; 4], dst: [[f32; 2]; 4]) -> Option<Mat3> {
    let mut a = [[0f32; 9]; 8];
    for i in 0..4 {
        let u = src[i][0];
        let v = src[i][1];
        let x = dst[i][0];
        let y = dst[i][1];
        a[i * 2] = [u, v, 1.0, 0.0, 0.0, 0.0, -u * x, -v * x, x];
        a[i * 2 + 1] = [0.0, 0.0, 0.0, u, v, 1.0, -u * y, -v * y, y];
    }
    let h = gaussian_solve(a)?;
    Some(Mat3::from_cols(
        Vec3::new(h[0], h[3], h[6]),
        Vec3::new(h[1], h[4], h[7]),
        Vec3::new(h[2], h[5], 1.0),
    ))
}

fn clip_from_normalized_output(xy: Vec2) -> [f32; 2] {
    // Normalized: x right, y down → NDC y up
    [xy.x * 2.0 - 1.0, 1.0 - xy.y * 2.0]
}

/// Tessellate each cell into `sub×sub` micro-quads with per-cell
/// homography (unit square → quad). Under v4 each layer's warp samples
/// the entire layer output; the v3 `source_rect` field is gone, so
/// `src_uv` runs over the full `[0,1]²` domain.
fn build_warp_vertices(mesh: &WarpMesh, sub: u32) -> (Vec<WarpVertex>, Vec<u32>) {
    let sub = sub.max(1);
    let rows = mesh.rows as usize;
    let cols = mesh.cols as usize;
    if rows == 0 || cols == 0 || mesh.grid.len() != rows + 1 {
        return (Vec::new(), Vec::new());
    }

    let nv = (rows * sub as usize + 1) * (cols * sub as usize + 1);
    let mut vertices = Vec::with_capacity(nv);
    let mut indices = Vec::new();

    let cs = cols * sub as usize;
    let rs = rows * sub as usize;

    for gi in 0..(rs + 1) {
        for gj in 0..(cs + 1) {
            let fu = gj as f32 / cs as f32;
            let fv = gi as f32 / rs as f32;
            let gx = fu * cols as f32;
            let gy = fv * rows as f32;
            let ix = (gx.floor() as usize).min(cols.saturating_sub(1));
            let iy = (gy.floor() as usize).min(rows.saturating_sub(1));
            let tx = gx - ix as f32;
            let ty = gy - iy as f32;

            let dst_c = [
                Vec2::from(mesh.grid[iy][ix]),
                Vec2::from(mesh.grid[iy][ix + 1]),
                Vec2::from(mesh.grid[iy + 1][ix + 1]),
                Vec2::from(mesh.grid[iy + 1][ix]),
            ];
            let src_unit = [[0f32, 0.], [1., 0.], [1., 1.], [0., 1.]];
            let dst_sq: [[f32; 2]; 4] = [
                dst_c[0].to_array(),
                dst_c[1].to_array(),
                dst_c[2].to_array(),
                dst_c[3].to_array(),
            ];
            let h = solve_homography(src_unit, dst_sq).unwrap_or(Mat3::IDENTITY);
            let dh = h * Vec3::new(tx, ty, 1.0);
            let w = dh.z.abs().max(1e-8);
            let dst_xy = Vec2::new(dh.x / w, dh.y / w);

            let src_uv = Vec2::new(fu, fv);

            let clip = clip_from_normalized_output(dst_xy);
            vertices.push(WarpVertex {
                pos_clip: clip,
                src_uv: src_uv.to_array(),
            });
        }
    }

    let stride = cs + 1;
    for ci in 0..rs {
        for cj in 0..cs {
            let i0 = ci * stride + cj;
            let i1 = i0 + 1;
            let i2 = i0 + stride + 1;
            let i3 = i0 + stride;
            indices.extend_from_slice(&[
                i0 as u32, i1 as u32, i3 as u32, i1 as u32, i2 as u32, i3 as u32,
            ]);
        }
    }

    (vertices, indices)
}

pub struct WarpRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    sampler_scene: wgpu::Sampler,
    mask_uniform: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    sdf_texture: wgpu::Texture,
    sdf_view: wgpu::TextureView,
    last_mesh_hash: u64,
    last_mask_hash: Option<u64>,
}

impl WarpRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("warp.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/warp.wgsl").into()),
        });

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("warp bgl"),
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
            array_stride: std::mem::size_of::<WarpVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("warp layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("warp pipeline"),
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
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let sampler_scene = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("warp scene sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let mask_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("warp mask u"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (vb, ib, ic, sdf_tex, sdf_v) = empty_mesh_and_dummy_sdf(device);

        Self {
            pipeline,
            bind_layout,
            sampler_scene,
            mask_uniform,
            vertex_buffer: vb,
            index_buffer: ib,
            index_count: ic,
            sdf_texture: sdf_tex,
            sdf_view: sdf_v,
            last_mesh_hash: 0,
            last_mask_hash: None,
        }
    }

    #[allow(dead_code)] // SDF debug-view overlay (T-M5+) will expose this
    pub fn sdf_view(&self) -> &wgpu::TextureView {
        &self.sdf_view
    }

    /// Hash mesh geometry for rebuild detection.
    pub fn mesh_geometry_hash(mesh: &WarpMesh) -> u64 {
        use std::hash::Hash;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        mesh.rows.hash(&mut h);
        mesh.cols.hash(&mut h);
        serde_json::to_string(&mesh.grid)
            .unwrap_or_default()
            .hash(&mut h);
        std::hash::Hasher::finish(&h)
    }

    /// Hash baker inputs (polygon only) for SDF rebuild detection.
    ///
    /// `mesh.mask_feather` is intentionally NOT in the key: it is sample-time
    /// only (a uniform read in `warp.wgsl`'s smoothstep), never an input to
    /// `bake_polygon_sdf`. Including it would invalidate the cache on every
    /// feather-slider drag and re-bake a 256×256 SDF for no reason.
    /// `SDF_SIZE` is a `pub const` and would belong here only if it ever
    /// becomes runtime-configurable — revisit then.
    pub fn mask_hash(mesh: &WarpMesh) -> u64 {
        use std::hash::Hash;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for pt in &mesh.mask_polygon {
            pt[0].to_bits().hash(&mut h);
            pt[1].to_bits().hash(&mut h);
        }
        std::hash::Hasher::finish(&h)
    }

    pub fn sync_mesh_and_mask(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &WarpMesh,
    ) {
        let geo_h = Self::mesh_geometry_hash(mesh);
        if geo_h != self.last_mesh_hash {
            self.last_mesh_hash = geo_h;
            let (v, idx) = build_warp_vertices(mesh, 12);
            let (vb, ib, ic) = upload_mesh(device, &v, &idx);
            self.vertex_buffer = vb;
            self.index_buffer = ib;
            self.index_count = ic;
        }

        let mask_h = Self::mask_hash(mesh);
        if self.last_mask_hash == Some(mask_h) {
            return;
        }
        self.last_mask_hash = Some(mask_h);

        let data = sdf::bake_polygon_sdf(&mesh.mask_polygon, SDF_SIZE);
        let size = wgpu::Extent3d {
            width: SDF_SIZE as u32,
            height: SDF_SIZE as u32,
            depth_or_array_layers: 1,
        };
        self.sdf_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mask sdf"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.sdf_view = self
            .sdf_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let raw: Vec<u8> = data.iter().flat_map(|&f| f.to_le_bytes()).collect();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.sdf_texture,
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
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::TextureView,
        scene: &wgpu::TextureView,
        mesh: &WarpMesh,
        load: wgpu::LoadOp<wgpu::Color>,
    ) {
        let use_mask = if mesh.mask_polygon.len() >= 3 {
            1.0f32
        } else {
            0.0
        };
        let feather = mesh.mask_feather.max(1e-5);
        let u = [use_mask, feather, SDF_SIZE as f32, 0.0];
        let mut b = [0u8; 16];
        for (i, f) in u.iter().enumerate() {
            b[i * 4..(i + 1) * 4].copy_from_slice(&f.to_le_bytes());
        }
        queue.write_buffer(&self.mask_uniform, 0, &b);

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("warp bg"),
            layout: &self.bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(scene),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_scene),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.sdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.mask_uniform.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("warp pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
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
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

fn upload_mesh(
    device: &wgpu::Device,
    v: &[WarpVertex],
    idx: &[u32],
) -> (wgpu::Buffer, wgpu::Buffer, u32) {
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("warp vb"),
        contents: verts_as_bytes(v),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib_bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(idx.as_ptr().cast::<u8>(), std::mem::size_of_val(idx))
    };
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("warp ib"),
        contents: ib_bytes,
        usage: wgpu::BufferUsages::INDEX,
    });
    (vb, ib, idx.len() as u32)
}

fn empty_mesh_and_dummy_sdf(
    device: &wgpu::Device,
) -> (
    wgpu::Buffer,
    wgpu::Buffer,
    u32,
    wgpu::Texture,
    wgpu::TextureView,
) {
    let mesh = crate::project::schema::WarpMesh::identity();
    let (v, idx) = build_warp_vertices(&mesh, 1);
    let (vb, ib, ic) = upload_mesh(device, &v, &idx);
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("dummy sdf"),
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
    let tv = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (vb, ib, ic, tex, tv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homography_round_trip() {
        let src = [[0f32, 0.], [1., 0.], [1., 1.], [0., 1.]];
        let dst = [[0.1, 0.05], [0.95, 0.0], [0.9, 0.95], [0.05, 0.85]];
        let h = solve_homography(src, dst).expect("solve");
        let inv = h.inverse();
        for i in 0..4 {
            let p = inv * Vec3::new(dst[i][0], dst[i][1], 1.0);
            let u = p.x / p.z;
            let v = p.y / p.z;
            assert!(
                (u - src[i][0]).abs() < 1e-4 && (v - src[i][1]).abs() < 1e-4,
                "corner {i}: got ({u},{v}) want {:?}",
                src[i]
            );
        }
    }
}
