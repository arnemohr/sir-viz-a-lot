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
