//! CPU-signed-distance baker for polygon masks (D-03). Output is a square
//! `R32Float` image: negative inside, positive outside, ~zero on edges.

pub const SDF_SIZE: usize = 256;

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

    fn square_poly() -> Vec<[f32; 2]> {
        vec![
            [0.25, 0.25],
            [0.75, 0.25],
            [0.75, 0.75],
            [0.25, 0.75],
        ]
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
}
