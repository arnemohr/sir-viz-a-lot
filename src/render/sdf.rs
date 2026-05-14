//! CPU-signed-distance baker for polygon masks (D-03). Output is a square
//! `R32Float` image: negative inside, positive outside, ~zero on edges.

pub const SDF_SIZE: usize = 256;

/// P0.5.2 — WGSL helper source string. Consumers (e.g. warp,
/// FxLayer presets) prepend this to their own shader source at
/// pipeline build time via Rust string concatenation. The helper
/// exposes `sample_sdf_bilinear`, `sample_sdf_gradient`, and
/// `sample_sdf` — all taking the SDF texture as a function parameter
/// so consumers control their own bind slots.
pub const SDF_HELPER_WGSL: &str = include_str!("shaders/sdf_helper.wgsl");

/// P3.3.1 — WGSL snippet providing zone-role constants and the `ZoneTagUniform`
/// struct for zone-aware FX preset shaders.
///
/// Prepend this to zone-aware preset shaders at pipeline build time, after
/// `SDF_HELPER_WGSL`, using Rust string concatenation — exactly as
/// `SDF_HELPER_WGSL` is used for SDF-consuming shaders.
///
/// Zone-aware shaders also declare the binding in their own source:
/// ```wgsl
/// @group(0) @binding(6) var<uniform> u_zone: ZoneTagUniform;
/// ```
/// This binding is NOT included here so `zone_tag_helper.wgsl` validates
/// standalone (no entry point, no orphaned binding). See P3.3.2 for the
/// canonical bind-group slot table.
#[allow(dead_code)] // P3.3.2 wires the zone-aware pipeline call sites.
pub const ZONE_TAG_WGSL: &str = include_str!("shaders/zone_tag_helper.wgsl");

/// Point-in-polygon (ray cast along +X).
pub fn point_in_polygon(x: f32, y: f32, poly: &[[f32; 2]]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let pi = poly[i];
        let pj = poly[j];
        let intersect = (pi[1] > y) != (pj[1] > y)
            && x < (pj[0] - pi[0]) * (y - pi[1]) / (pj[1] - pi[1] + 1e-12) + pi[0];
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dist_point_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;
    let ab_len2 = abx * abx + aby * aby;
    if ab_len2 < 1e-12 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = ((apx * abx + apy * aby) / ab_len2).clamp(0.0, 1.0);
    let cx = ax + t * abx;
    let cy = ay + t * aby;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// Brute-force SDF in normalized [0,1]² space. Texel centers sample the field.
/// Distance is in **the same normalized units** as polygon coordinates (edge length scale ~1).
pub fn bake_polygon_sdf(poly: &[[f32; 2]], size: usize) -> Vec<f32> {
    if poly.is_empty() || size == 0 {
        return vec![1.0; size * size];
    }

    let mut out = Vec::with_capacity(size * size);
    let scale = 1.0 / size as f32;

    for ty in 0..size {
        for tx in 0..size {
            let x = (tx as f32 + 0.5) * scale;
            let y = (ty as f32 + 0.5) * scale;

            let mut d_min = f32::MAX;
            let n = poly.len();
            for i in 0..n {
                let a = poly[i];
                let b = poly[(i + 1) % n];
                let d = dist_point_segment(x, y, a[0], a[1], b[0], b[1]);
                d_min = d_min.min(d);
            }

            let inside = point_in_polygon(x, y, poly);
            let signed = if inside { -d_min } else { d_min };
            out.push(signed);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// P7.4.2 — MaskGraph CPU SDF evaluator
// ---------------------------------------------------------------------------

/// P7.4.2 — Evaluate a `MaskGraph` to a signed-distance field.
///
/// - A single `Polygon` node produces pixel-identical output to
///   `bake_polygon_sdf` (backward-compatibility invariant).
/// - An `Inverse` node wraps another node and negates its SDF values
///   (inside ↔ outside).
/// - `Union`, `Subtract` — schema-only in Phase 7; fall through to an
///   all-positive SDF (full canvas, effectively no mask).
/// - `LumaKey`, `ChromaKey` — require a rendered frame as input; return
///   an all-negative SDF (fully opaque) as a safe fallback.
///
/// ## TODO(P7.4.2-gpu)
/// Wire this evaluator into the render pipeline:
/// - Replace the `bake_polygon_sdf(layer.warp.mask_polygon, …)` call site
///   in `warp.rs` with `bake_mask_graph_sdf(layer.mask_graph, …)`.
/// - LumaKey / ChromaKey evaluation requires the layer's rendered texture
///   as input; defer until the render pipeline has that texture available.
#[allow(dead_code)] // TODO(P7.4.2-gpu): wire to render pipeline
pub fn bake_mask_graph_sdf(graph: &crate::project::schema::MaskGraph, size: usize) -> Vec<f32> {
    if graph.nodes.is_empty() || size == 0 {
        // Empty graph = full-canvas identity (no mask; all negative = inside).
        return vec![-1.0; size * size];
    }

    // Evaluate node 0 (the root).
    eval_node(&graph.nodes, 0, size)
}

/// Recursively evaluate the SDF of a single `MaskNode` by index.
#[allow(dead_code)] // called from bake_mask_graph_sdf; both deferred until GPU wiring lands
fn eval_node(nodes: &[crate::project::schema::MaskNode], idx: usize, size: usize) -> Vec<f32> {
    use crate::project::schema::MaskNode;

    let Some(node) = nodes.get(idx) else {
        // Stale NodeId — fall back to full-canvas negative SDF.
        return vec![-1.0; size * size];
    };

    match node {
        MaskNode::Polygon { points, feather: _ } => {
            // Single-node polygon path — pixel-identical to the legacy
            // `bake_polygon_sdf` (backward-compat invariant, P7.4.2).
            bake_polygon_sdf(points, size)
        }

        MaskNode::Inverse { of } => {
            // Negate the SDF of the referenced node.
            let inner_idx = *of;
            let inner = eval_node(nodes, inner_idx, size);
            inner.into_iter().map(|v| -v).collect()
        }

        MaskNode::Union { a, b } => {
            // PCleanup.5.1 — union of two SDFs is the per-pixel min:
            // a pixel is inside the union iff it's inside either operand
            // (i.e., the SDF is negative iff at least one operand's SDF
            // is negative). `min(a, b)` is the standard signed-distance
            // semantics; if both are positive the closest one wins (the
            // negative-of-min-positive is preserved correctly).
            let a_buf = eval_node(nodes, *a, size);
            let b_buf = eval_node(nodes, *b, size);
            a_buf
                .into_iter()
                .zip(b_buf)
                .map(|(av, bv)| av.min(bv))
                .collect()
        }

        MaskNode::Subtract { base, sub } => {
            // PCleanup.5.1 — subtract = base ∧ ¬sub. A pixel is inside
            // the result iff it's inside `base` AND outside `sub`. SDF
            // arithmetic: max(base, -sub). When base is positive (outside
            // base) the result is positive (base wins); when base is
            // negative and sub is negative (inside both), -sub is
            // positive and wins, kicking the pixel outside the result.
            let base_buf = eval_node(nodes, *base, size);
            let sub_buf = eval_node(nodes, *sub, size);
            base_buf
                .into_iter()
                .zip(sub_buf)
                .map(|(b, s)| b.max(-s))
                .collect()
        }

        MaskNode::LumaKey { .. } | MaskNode::ChromaKey { .. } => {
            // TODO(P7.4.2-gpu): LumaKey and ChromaKey require the rendered
            // frame as input (the luminance / hue of each pixel).  These
            // cannot be evaluated purely from geometry; the CPU SDF baker
            // does not have access to the rendered texture at this call site.
            //
            // For now, return a fully-opaque SDF (all negative = inside,
            // no masking applied) as a safe-show-day fallback.
            vec![-1.0; size * size]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // P2.3.1 — CPU mirror of `sample_sdf_bilinear` (WGSL). Matches the shader
    // implementation exactly: texel-centre layout, bilinear mix, clamped indices.
    fn sample_sdf_bilinear_cpu(buf: &[f32], w: usize, h: usize, uv: [f32; 2]) -> f32 {
        let (wf, hf) = (w as f32, h as f32);
        let px = uv[0] * wf - 0.5;
        let py = uv[1] * hf - 0.5;
        let i0x = px.floor() as i32;
        let i0y = py.floor() as i32;
        let fx = px - i0x as f32;
        let fy = py - i0y as f32;
        let ix0 = i0x.clamp(0, w as i32 - 1) as usize;
        let iy0 = i0y.clamp(0, h as i32 - 1) as usize;
        let ix1 = (i0x + 1).clamp(0, w as i32 - 1) as usize;
        let iy1 = (i0y + 1).clamp(0, h as i32 - 1) as usize;
        let v00 = buf[iy0 * w + ix0];
        let v10 = buf[iy0 * w + ix1];
        let v01 = buf[iy1 * w + ix0];
        let v11 = buf[iy1 * w + ix1];
        let row0 = v00 + (v10 - v00) * fx;
        let row1 = v01 + (v11 - v01) * fx;
        row0 + (row1 - row0) * fy
    }

    // P2.3.1 — CPU mirror of `sample_sdf_gradient` (WGSL). Central-difference
    // at one-texel epsilon.
    fn sample_sdf_gradient_cpu(buf: &[f32], w: usize, h: usize, uv: [f32; 2]) -> [f32; 2] {
        let eps_x = 1.0 / w as f32;
        let eps_y = 1.0 / h as f32;
        let dx_p = sample_sdf_bilinear_cpu(buf, w, h, [uv[0] + eps_x, uv[1]]);
        let dx_n = sample_sdf_bilinear_cpu(buf, w, h, [uv[0] - eps_x, uv[1]]);
        let dy_p = sample_sdf_bilinear_cpu(buf, w, h, [uv[0], uv[1] + eps_y]);
        let dy_n = sample_sdf_bilinear_cpu(buf, w, h, [uv[0], uv[1] - eps_y]);
        [(dx_p - dx_n) / (2.0 * eps_x), (dy_p - dy_n) / (2.0 * eps_y)]
    }

    // P2.3.1 — CPU mirror of `sample_sdf_normal` (WGSL). Normalises the
    // gradient; returns [0.0, 0.0] when magnitude is below the 1e-6 floor.
    fn sample_sdf_normal_cpu(buf: &[f32], w: usize, h: usize, uv: [f32; 2]) -> [f32; 2] {
        let g = sample_sdf_gradient_cpu(buf, w, h, uv);
        let len = (g[0] * g[0] + g[1] * g[1]).sqrt();
        if len < 1e-6 {
            return [0.0, 0.0];
        }
        [g[0] / len, g[1] / len]
    }

    /// P0.5.2 smoke test: confirms the helper constant contains all three
    /// exported function signatures. Not a functional test — just guards
    /// against accidental truncation or wrong file path.
    #[test]
    fn sdf_helper_wgsl_contains_all_functions() {
        assert!(
            SDF_HELPER_WGSL.contains("fn sample_sdf_bilinear("),
            "SDF_HELPER_WGSL missing sample_sdf_bilinear"
        );
        assert!(
            SDF_HELPER_WGSL.contains("fn sample_sdf_gradient("),
            "SDF_HELPER_WGSL missing sample_sdf_gradient"
        );
        assert!(
            SDF_HELPER_WGSL.contains("fn sample_sdf("),
            "SDF_HELPER_WGSL missing sample_sdf"
        );
        assert!(
            SDF_HELPER_WGSL.contains("fn sample_sdf_normal("),
            "SDF_HELPER_WGSL missing sample_sdf_normal"
        );
    }

    fn square_poly() -> Vec<[f32; 2]> {
        vec![[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75]]
    }

    #[test]
    fn sdf_inside_is_negative() {
        let poly = square_poly();
        let tex = bake_polygon_sdf(&poly, 64);
        let ix = 32;
        let iy = 32;
        let d = tex[iy * 64 + ix];
        assert!(d < 0.0, "center should be inside, got {d}");
    }

    #[test]
    fn sdf_outside_is_positive() {
        let poly = square_poly();
        let tex = bake_polygon_sdf(&poly, 64);
        let d = tex[5 * 64 + 5];
        assert!(d > 0.0, "corner should be outside, got {d}");
    }

    #[test]
    fn sdf_on_edge_is_zero() {
        let poly = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let size = 128;
        let tex = bake_polygon_sdf(&poly, size);
        // Texel on bottom edge center
        let tx = 64;
        let ty = 0;
        let d = tex[ty * size + tx].abs();
        let texel = 1.0 / size as f32;
        assert!(
            d < texel * 2.0,
            "edge distance should be ~0, got {d} (texel ~{texel})"
        );
    }

    /// P2.3.1 — Construct a synthetic 64×64 circle SDF (centre 0.5, radius 0.25)
    /// and verify `sample_sdf_normal` returns a vector within 0.05 per component
    /// of the analytic radial unit-vector at four cardinal UV positions outside
    /// the circle.
    #[test]
    fn sdf_normal_matches_radial_for_circle() {
        let size: usize = 64;
        let center = (0.5_f32, 0.5_f32);
        let radius = 0.25_f32;

        // Build circle SDF: value = dist_from_centre - radius.
        let buf: Vec<f32> = (0..size * size)
            .map(|idx| {
                let ix = idx % size;
                let iy = idx / size;
                let x = (ix as f32 + 0.5) / size as f32;
                let y = (iy as f32 + 0.5) / size as f32;
                let dist = ((x - center.0).powi(2) + (y - center.1).powi(2)).sqrt();
                dist - radius
            })
            .collect();

        // Four cardinal UV positions outside the circle (centre + radius + margin).
        let test_uvs: &[[f32; 2]] = &[
            [0.5, 0.1], // above centre (small y)
            [0.5, 0.9], // below centre (large y)
            [0.1, 0.5], // left of centre
            [0.9, 0.5], // right of centre
        ];

        let tol = 0.05_f32;

        for &uv in test_uvs {
            let normal = sample_sdf_normal_cpu(&buf, size, size, uv);

            // Analytic: unit vector from centre to uv.
            let dx = uv[0] - center.0;
            let dy = uv[1] - center.1;
            let len = (dx * dx + dy * dy).sqrt();
            let expected = [dx / len, dy / len];

            assert!(
                (normal[0] - expected[0]).abs() < tol,
                "uv={uv:?}: normal.x={} expected {} (tol {tol})",
                normal[0],
                expected[0],
            );
            assert!(
                (normal[1] - expected[1]).abs() < tol,
                "uv={uv:?}: normal.y={} expected {} (tol {tol})",
                normal[1],
                expected[1],
            );
        }
    }

    /// P2.3.1 — At the exact circle centre the SDF gradient is degenerate
    /// (equidistant in all directions); `sample_sdf_normal` must return a
    /// near-zero vector rather than a NaN or an arbitrary direction.
    #[test]
    fn sdf_normal_zero_when_degenerate() {
        let size: usize = 64;
        let center = (0.5_f32, 0.5_f32);
        let radius = 0.25_f32;

        let buf: Vec<f32> = (0..size * size)
            .map(|idx| {
                let ix = idx % size;
                let iy = idx / size;
                let x = (ix as f32 + 0.5) / size as f32;
                let y = (iy as f32 + 0.5) / size as f32;
                let dist = ((x - center.0).powi(2) + (y - center.1).powi(2)).sqrt();
                dist - radius
            })
            .collect();

        // At the exact centre the gradient should be near-zero (by symmetry,
        // the central-difference stencil cancels). If it doesn't fully cancel
        // due to floating-point, the magnitude must still be below 1e-6 so the
        // function returns [0, 0].
        let normal = sample_sdf_normal_cpu(&buf, size, size, [center.0, center.1]);
        let mag = (normal[0] * normal[0] + normal[1] * normal[1]).sqrt();
        assert!(
            mag < 1e-4,
            "expected near-zero normal at circle centre, got {normal:?} (mag={mag})"
        );
    }

    // --- P3.3.1 ZONE_TAG_WGSL tests ---

    /// P3.3.1 — ZONE_TAG_WGSL contains all zone constant definitions and the
    /// ZoneTagUniform struct.
    #[test]
    fn zone_tag_wgsl_contains_required_declarations() {
        assert!(
            ZONE_TAG_WGSL.contains("ZONE_NONE"),
            "ZONE_TAG_WGSL missing ZONE_NONE constant"
        );
        assert!(
            ZONE_TAG_WGSL.contains("ZONE_WINDOW"),
            "ZONE_TAG_WGSL missing ZONE_WINDOW constant"
        );
        assert!(
            ZONE_TAG_WGSL.contains("ZONE_PORTAL"),
            "ZONE_TAG_WGSL missing ZONE_PORTAL constant"
        );
        assert!(
            ZONE_TAG_WGSL.contains("ZONE_VOID"),
            "ZONE_TAG_WGSL missing ZONE_VOID constant"
        );
        assert!(
            ZONE_TAG_WGSL.contains("ZONE_SPILL"),
            "ZONE_TAG_WGSL missing ZONE_SPILL constant"
        );
        assert!(
            ZONE_TAG_WGSL.contains("ZONE_EDGE"),
            "ZONE_TAG_WGSL missing ZONE_EDGE constant"
        );
        assert!(
            ZONE_TAG_WGSL.contains("ZONE_HIGHLIGHT"),
            "ZONE_TAG_WGSL missing ZONE_HIGHLIGHT constant"
        );
        assert!(
            ZONE_TAG_WGSL.contains("ZONE_LIGHT_SOURCE"),
            "ZONE_TAG_WGSL missing ZONE_LIGHT_SOURCE constant"
        );
        assert!(
            ZONE_TAG_WGSL.contains("struct ZoneTagUniform"),
            "ZONE_TAG_WGSL missing ZoneTagUniform struct"
        );
        assert!(
            ZONE_TAG_WGSL.contains("zone_tag: u32"),
            "ZONE_TAG_WGSL missing zone_tag field in ZoneTagUniform"
        );
    }

    // --- P7.4.2 MaskGraph SDF evaluator tests ---

    /// P7.4.2 — Single-node polygon `MaskGraph` produces pixel-identical output
    /// to `bake_polygon_sdf` (backward-compat invariant).
    #[test]
    fn mask_graph_single_polygon_pixel_identical_to_bake_polygon_sdf() {
        use crate::project::schema::{MaskGraph, MaskNode};

        let poly = vec![[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75]];
        let size = 64;

        let direct = bake_polygon_sdf(&poly, size);
        let graph = MaskGraph {
            nodes: vec![MaskNode::Polygon {
                points: poly.clone(),
                feather: 0.0,
            }],
        };
        let via_graph = bake_mask_graph_sdf(&graph, size);

        assert_eq!(direct.len(), via_graph.len(), "SDF lengths must match");
        for (i, (&a, &b)) in direct.iter().zip(via_graph.iter()).enumerate() {
            assert!(
                (a - b).abs() < 1e-6,
                "Pixel {i}: direct={a}, graph={b} — must be pixel-identical"
            );
        }
    }

    /// P7.4.2 — `Inverse` node produces negated SDF (inside ↔ outside).
    #[test]
    fn mask_graph_inverse_node_negates_sdf() {
        use crate::project::schema::{MaskGraph, MaskNode};

        let poly = vec![[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75]];
        let size = 64;

        // Forward polygon SDF.
        let forward_graph = MaskGraph {
            nodes: vec![MaskNode::Polygon {
                points: poly.clone(),
                feather: 0.0,
            }],
        };
        let forward = bake_mask_graph_sdf(&forward_graph, size);

        // Inverse: node 0 = Polygon, node 1 = Inverse { of: 0 }; root is node 0
        // but the Inverse node wraps it — we make the Inverse the root (index 0).
        let inverse_graph = MaskGraph {
            nodes: vec![
                MaskNode::Inverse { of: 1 },
                MaskNode::Polygon {
                    points: poly,
                    feather: 0.0,
                },
            ],
        };
        let inverse = bake_mask_graph_sdf(&inverse_graph, size);

        assert_eq!(forward.len(), inverse.len());
        for (i, (&f, &inv)) in forward.iter().zip(inverse.iter()).enumerate() {
            assert!(
                (f + inv).abs() < 1e-6,
                "Pixel {i}: forward={f}, inverse={inv} — must be negated"
            );
        }

        // Inside pixels (negative forward SDF) must be outside in inverse (positive).
        let center_fwd = forward[32 * size + 32];
        let center_inv = inverse[32 * size + 32];
        assert!(center_fwd < 0.0, "Center must be inside polygon (fwd<0)");
        assert!(
            center_inv > 0.0,
            "Center must be outside inverse polygon (inv>0)"
        );
    }

    /// P7.4.2 — Empty `MaskGraph` returns full-canvas identity (all negative).
    #[test]
    fn mask_graph_empty_returns_all_negative() {
        use crate::project::schema::MaskGraph;

        let graph = MaskGraph { nodes: vec![] };
        let sdf = bake_mask_graph_sdf(&graph, 16);
        for &v in &sdf {
            assert!(v < 0.0, "Empty graph must return all-inside SDF, got {v}");
        }
    }

    // ----- PCleanup.5.1 — MaskNode::Union / Subtract -----------------------

    /// PCleanup.5.1 — Union of two non-overlapping polygons covers both
    /// (per-pixel min of SDFs ≤ 0 wherever either operand's SDF ≤ 0).
    /// Construction: two unit squares offset on the X axis, no overlap;
    /// pixels inside either square must read negative in the union; pixels
    /// outside both must read positive.
    #[test]
    fn mask_graph_union_covers_both_operands() {
        use crate::project::schema::{MaskGraph, MaskNode};
        let size = 64;
        // Two disjoint squares: left half-canvas + right half-canvas
        // (skinny strips so a centre column lies outside both).
        let left = vec![[0.05, 0.40], [0.30, 0.40], [0.30, 0.60], [0.05, 0.60]];
        let right = vec![[0.70, 0.40], [0.95, 0.40], [0.95, 0.60], [0.70, 0.60]];

        let graph = MaskGraph {
            nodes: vec![
                MaskNode::Polygon {
                    points: left.clone(),
                    feather: 0.0,
                },
                MaskNode::Polygon {
                    points: right.clone(),
                    feather: 0.0,
                },
                MaskNode::Union { a: 0, b: 1 },
            ],
        };
        // Reorder so the Union is the root: the eval starts at node 0.
        let graph_with_root = MaskGraph {
            nodes: vec![
                MaskNode::Union { a: 1, b: 2 },
                MaskNode::Polygon {
                    points: left,
                    feather: 0.0,
                },
                MaskNode::Polygon {
                    points: right,
                    feather: 0.0,
                },
            ],
        };
        let union_sdf = bake_mask_graph_sdf(&graph_with_root, size);

        // Pick three sample points:
        //  (a) inside the left square          → must be negative
        //  (b) inside the right square         → must be negative
        //  (c) in the centre gap (no operand)  → must be positive
        let pix = |uv: [f32; 2]| {
            let x = (uv[0] * size as f32) as usize;
            let y = (uv[1] * size as f32) as usize;
            union_sdf[y * size + x]
        };
        assert!(
            pix([0.17, 0.50]) < 0.0,
            "inside left square must be negative"
        );
        assert!(
            pix([0.82, 0.50]) < 0.0,
            "inside right square must be negative"
        );
        assert!(pix([0.50, 0.50]) > 0.0, "centre gap must be positive");

        // Sanity: silence the unused-variable warning for the original
        // graph constructor above (we keep it for documentation).
        let _ = graph;
    }

    /// PCleanup.5.1 — Subtract removes the `sub` region from the `base`.
    /// Construction: base = a centred square; sub = a smaller centred
    /// square fully inside it. The result is a square donut — pixels in
    /// the donut's ring read negative; pixels in the hole (inside both)
    /// read positive (kicked outside by the subtract).
    #[test]
    fn mask_graph_subtract_carves_hole() {
        use crate::project::schema::{MaskGraph, MaskNode};
        let size = 64;
        let big = vec![[0.20, 0.20], [0.80, 0.20], [0.80, 0.80], [0.20, 0.80]];
        let small = vec![[0.40, 0.40], [0.60, 0.40], [0.60, 0.60], [0.40, 0.60]];

        let graph = MaskGraph {
            nodes: vec![
                MaskNode::Subtract { base: 1, sub: 2 },
                MaskNode::Polygon {
                    points: big,
                    feather: 0.0,
                },
                MaskNode::Polygon {
                    points: small,
                    feather: 0.0,
                },
            ],
        };
        let donut = bake_mask_graph_sdf(&graph, size);

        let pix = |uv: [f32; 2]| {
            let x = (uv[0] * size as f32) as usize;
            let y = (uv[1] * size as f32) as usize;
            donut[y * size + x]
        };
        // Inside donut's ring (in big, NOT in small) → negative.
        assert!(pix([0.30, 0.30]) < 0.0, "ring must be negative");
        // Inside the hole (in both big AND small) → positive (carved out).
        assert!(pix([0.50, 0.50]) > 0.0, "hole must be positive");
        // Outside the big square → positive.
        assert!(pix([0.10, 0.10]) > 0.0, "outside big must be positive");
    }

    /// P3.3.1 — `From<ZoneRole> for u32` mapping matches WGSL constant values.
    #[test]
    fn zone_role_u32_values_match_wgsl_constants() {
        use crate::project::schema::ZoneRole;
        // Parse the ZONE_TAG_WGSL to verify the constants in the file.
        // We check that the Rust From impl and the WGSL constants agree
        // by verifying both use the same numeric values.
        assert_eq!(u32::from(ZoneRole::Window), 1);
        assert_eq!(u32::from(ZoneRole::Portal), 2);
        assert_eq!(u32::from(ZoneRole::Void), 3);
        assert_eq!(u32::from(ZoneRole::Spill), 4);
        assert_eq!(u32::from(ZoneRole::Edge), 5);
        assert_eq!(u32::from(ZoneRole::Highlight), 6);
        assert_eq!(u32::from(ZoneRole::LightSource), 7);
        // Verify the WGSL file contains the matching values.
        assert!(ZONE_TAG_WGSL.contains("ZONE_WINDOW: u32 = 1u"));
        assert!(ZONE_TAG_WGSL.contains("ZONE_PORTAL: u32 = 2u"));
        assert!(ZONE_TAG_WGSL.contains("ZONE_VOID: u32 = 3u"));
        assert!(ZONE_TAG_WGSL.contains("ZONE_SPILL: u32 = 4u"));
        assert!(ZONE_TAG_WGSL.contains("ZONE_EDGE: u32 = 5u"));
        assert!(ZONE_TAG_WGSL.contains("ZONE_HIGHLIGHT: u32 = 6u"));
        assert!(ZONE_TAG_WGSL.contains("ZONE_LIGHT_SOURCE: u32 = 7u"));
    }
}
