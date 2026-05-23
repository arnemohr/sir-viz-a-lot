//! SVG path geometry extraction and arc-length parameterization.
//!
//! This module provides infrastructure for the `Effect::LightTrail` effect and any future
//! motion-path effects. It is independent of SVG rasterization (`svg_layer`).
//!
//! # Coordinate space
//!
//! All output is in **SVG user-space** coordinates (the same space `usvg::Tree::root()
//! .abs_bounding_box()` reports and that `svg_layer::raster_uniform_fit_transform`
//! operates on). Callers that need to align a trail with the rasterized layer image must
//! apply the same `raster_uniform_fit_transform` to the polyline points — see Risk 2 in
//! `specs/005-light-trail/01-path-extraction-decision.md`.
//!
//! # Multi-subpath behaviour
//!
//! A single SVG `<path>` element can contain multiple subpaths separated by `M` (MoveTo)
//! commands in its `d` attribute. This module uses only the **first** continuous subpath
//! within the selected path element (segments from the first `MoveTo` up to but not
//! including any subsequent `MoveTo`). A warning is emitted when additional subpaths are
//! discarded, so operators can see the truncation in the log.
//!
//! Multi-`<path>` SVGs are handled via the `path_index` parameter; see [`extract_path`].

use usvg::tiny_skia_path::{PathSegment, Point};

/// Error variants for path geometry operations.
#[derive(Debug, thiserror::Error)]
pub enum PathGeomError {
    /// The SVG source could not be parsed by usvg.
    #[error("malformed SVG: {0}")]
    MalformedSvg(String),

    /// The SVG tree contains no `<path>` elements (could be raster-only or text-only).
    ///
    /// Note: `<text>` elements that are not pre-converted to `<path>` will not appear
    /// as `Node::Path` nodes because `usvg::Options::default()` does not load system
    /// fonts. Operators must outline/convert text before using LightTrail.
    #[error("SVG contains no <path> elements")]
    NoPaths,

    /// The extracted path has zero arc-length (degenerate geometry).
    #[error("extracted path has zero arc-length")]
    ZeroLength,
}

/// Path segments in absolute SVG user-space, produced by [`extract_path`].
///
/// `path_count` is reported as `u32` (matching the `path_index` parameter type) even
/// though usvg uses `usize` internally; the cast happens at the extraction boundary and is
/// safe for any realistic SVG (2^32 path elements would be pathological).
#[derive(Debug, Clone)]
pub struct ExtractedPath {
    /// Segments with all coordinates in SVG user-space (`abs_transform` applied).
    pub segments: Vec<PathSegment>,
    /// Total number of `Node::Path` elements found in the SVG tree.
    pub path_count: u32,
}

/// Arc-length-parameterized polyline built from an [`ExtractedPath`].
#[derive(Debug, Clone)]
pub struct Polyline {
    /// Uniformly resampled points along the path.
    pub points: Vec<[f32; 2]>,
    /// Cumulative arc-length at each sample point. `cumulative_arclen[i]` is the
    /// distance from `points[0]` to `points[i]`.
    pub cumulative_arclen: Vec<f32>,
    /// Total arc-length of the polyline (equals `cumulative_arclen.last()`).
    pub total_length: f32,
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

/// Walk the usvg node tree depth-first, collecting all `Node::Path` leaf nodes.
fn collect_paths(group: &usvg::Group, out: &mut Vec<(Vec<PathSegment>, usvg::Transform)>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => collect_paths(g, out),
            usvg::Node::Path(p) => {
                let transform = p.abs_transform();
                let segments: Vec<PathSegment> = p.data().segments().collect();
                out.push((segments, transform));
            }
            usvg::Node::Image(_) | usvg::Node::Text(_) => {}
        }
    }
}

/// Apply a usvg `Transform` to a single `Point` (in-place).
fn map_point(ts: usvg::Transform, p: Point) -> Point {
    let mut q = p;
    ts.map_point(&mut q);
    q
}

/// Apply `abs_transform` to all points in every segment.
fn transform_segments(segments: &[PathSegment], ts: usvg::Transform) -> Vec<PathSegment> {
    if ts.is_identity() {
        return segments.to_vec();
    }
    segments
        .iter()
        .map(|seg| match *seg {
            PathSegment::MoveTo(p) => PathSegment::MoveTo(map_point(ts, p)),
            PathSegment::LineTo(p) => PathSegment::LineTo(map_point(ts, p)),
            PathSegment::QuadTo(p0, p1) => {
                PathSegment::QuadTo(map_point(ts, p0), map_point(ts, p1))
            }
            PathSegment::CubicTo(p0, p1, p2) => {
                PathSegment::CubicTo(map_point(ts, p0), map_point(ts, p1), map_point(ts, p2))
            }
            PathSegment::Close => PathSegment::Close,
        })
        .collect()
}

/// Extract path geometry from an SVG string.
///
/// # Parameters
/// - `svg_text` — the raw SVG source (UTF-8 text).
/// - `path_index` — selects the Nth `<path>` in DFS document order. Default is 0.
///   If out of range, clamps to the last valid index and emits a `tracing::warn!`.
///
/// # Errors
/// - [`PathGeomError::MalformedSvg`] — usvg could not parse the input.
/// - [`PathGeomError::NoPaths`] — the SVG contains no `<path>` elements.
pub fn extract_path(svg_text: &str, path_index: u32) -> Result<ExtractedPath, PathGeomError> {
    let tree = usvg::Tree::from_str(svg_text, &usvg::Options::default())
        .map_err(|e| PathGeomError::MalformedSvg(e.to_string()))?;

    let mut raw_paths: Vec<(Vec<PathSegment>, usvg::Transform)> = Vec::new();
    collect_paths(tree.root(), &mut raw_paths);

    if raw_paths.is_empty() {
        return Err(PathGeomError::NoPaths);
    }

    let path_count = raw_paths.len();
    let clamped_index = if path_index as usize >= path_count {
        let last = path_count - 1;
        tracing::warn!(
            path_index,
            path_count,
            clamped_to = last,
            "path_index out of range, clamped to last path"
        );
        last
    } else {
        path_index as usize
    };

    let (segments, transform) = &raw_paths[clamped_index];
    let world_segments = transform_segments(segments, *transform);

    Ok(ExtractedPath {
        segments: world_segments,
        // Safe cast: realistic SVGs have far fewer than 2^32 paths.
        path_count: path_count as u32,
    })
}

// ---------------------------------------------------------------------------
// Bézier flattening helpers
// ---------------------------------------------------------------------------

const FLATTEN_TOLERANCE: f32 = 0.25;

fn dist_sq(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    dx * dx + dy * dy
}

/// Recursive de Casteljau cubic Bézier flattening.
///
/// Appends intermediate points to `out`; the end-point `p3` is NOT appended
/// (the caller appends it after the final recursive call returns).
fn flatten_cubic(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], out: &mut Vec<[f32; 2]>) {
    // Flatness test: maximum squared distance of control points from the chord.
    let ax = 3.0 * p1[0] - 2.0 * p0[0] - p3[0];
    let ay = 3.0 * p1[1] - 2.0 * p0[1] - p3[1];
    let bx = 3.0 * p2[0] - 2.0 * p3[0] - p0[0];
    let by = 3.0 * p2[1] - 2.0 * p3[1] - p0[1];
    let flatness_sq = ax.max(bx) * ax.max(bx) + ay.max(by) * ay.max(by);
    if flatness_sq <= FLATTEN_TOLERANCE * FLATTEN_TOLERANCE {
        return;
    }
    // Split at t = 0.5 (de Casteljau midpoints).
    let m01 = midpoint(p0, p1);
    let m12 = midpoint(p1, p2);
    let m23 = midpoint(p2, p3);
    let m012 = midpoint(m01, m12);
    let m123 = midpoint(m12, m23);
    let m0123 = midpoint(m012, m123);
    flatten_cubic(p0, m01, m012, m0123, out);
    out.push(m0123);
    flatten_cubic(m0123, m123, m23, p3, out);
}

/// Recursive de Casteljau quadratic Bézier flattening.
fn flatten_quad(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], out: &mut Vec<[f32; 2]>) {
    // Flatness: deviation of control point from chord.
    let mx = 0.5 * (p0[0] + p2[0]);
    let my = 0.5 * (p0[1] + p2[1]);
    let dx = p1[0] - mx;
    let dy = p1[1] - my;
    if dx * dx + dy * dy <= FLATTEN_TOLERANCE * FLATTEN_TOLERANCE {
        return;
    }
    let m01 = midpoint(p0, p1);
    let m12 = midpoint(p1, p2);
    let mid = midpoint(m01, m12);
    flatten_quad(p0, m01, mid, out);
    out.push(mid);
    flatten_quad(mid, m12, p2, out);
}

#[inline]
fn midpoint(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}

fn point_to_arr(p: Point) -> [f32; 2] {
    [p.x, p.y]
}

// ---------------------------------------------------------------------------
// Polyline building
// ---------------------------------------------------------------------------

/// Flatten the segments of an [`ExtractedPath`] into a dense polyline.
///
/// Only the first continuous subpath is used. A "subpath" starts at a `MoveTo`
/// and ends just before the next `MoveTo` (or at the end of the segment list).
/// If additional subpaths are present they are silently discarded after emitting
/// a `tracing::warn!`.
fn flatten_to_dense(segments: &[PathSegment]) -> Vec<[f32; 2]> {
    let mut points: Vec<[f32; 2]> = Vec::new();
    let mut cursor: [f32; 2] = [0.0, 0.0];
    let mut started = false;
    let mut extra_subpaths = 0u32;

    for seg in segments {
        match *seg {
            PathSegment::MoveTo(p) => {
                if started {
                    // Second (or later) MoveTo — discard remaining subpaths.
                    extra_subpaths += 1;
                    break;
                }
                cursor = point_to_arr(p);
                points.push(cursor);
                started = true;
            }
            PathSegment::LineTo(p) => {
                cursor = point_to_arr(p);
                points.push(cursor);
            }
            PathSegment::QuadTo(p1, p2) => {
                let c1 = point_to_arr(p1);
                let c2 = point_to_arr(p2);
                flatten_quad(cursor, c1, c2, &mut points);
                cursor = c2;
                points.push(cursor);
            }
            PathSegment::CubicTo(p1, p2, p3) => {
                let c1 = point_to_arr(p1);
                let c2 = point_to_arr(p2);
                let c3 = point_to_arr(p3);
                flatten_cubic(cursor, c1, c2, c3, &mut points);
                cursor = c3;
                points.push(cursor);
            }
            PathSegment::Close => {
                // Close back to first point if we have one.
                if let Some(&first) = points.first() {
                    points.push(first);
                    cursor = first;
                }
            }
        }
    }

    if extra_subpaths > 0 {
        tracing::warn!(
            extra_subpaths,
            "path element contains multiple subpaths; only the first subpath is used"
        );
    }

    points
}

/// Build a cumulative arc-length array for a dense flat polyline.
fn build_arclen(points: &[[f32; 2]]) -> (Vec<f32>, f32) {
    let mut cumulative = vec![0.0f32; points.len()];
    let mut total = 0.0f32;
    for i in 1..points.len() {
        let d = dist_sq(points[i - 1], points[i]).sqrt();
        total += d;
        cumulative[i] = total;
    }
    (cumulative, total)
}

/// Resample `dense_points` (with cumulative arc-lengths `dense_arclen`) at
/// `n_samples` uniformly spaced distances along the path.
fn resample_uniform(
    dense_points: &[[f32; 2]],
    dense_arclen: &[f32],
    total_length: f32,
    n_samples: u32,
) -> (Vec<[f32; 2]>, Vec<f32>) {
    assert!(!dense_points.is_empty());
    assert_eq!(dense_points.len(), dense_arclen.len());

    let mut out_points: Vec<[f32; 2]> = Vec::with_capacity(n_samples as usize);
    let mut out_arclen: Vec<f32> = Vec::with_capacity(n_samples as usize);

    for i in 0..n_samples {
        let t = if n_samples > 1 {
            i as f32 / (n_samples - 1) as f32
        } else {
            0.0
        };
        let d = t * total_length;
        let p = sample_dense_at(dense_points, dense_arclen, total_length, d);
        out_points.push(p);
        out_arclen.push(d);
    }

    (out_points, out_arclen)
}

/// Linear interpolation between two 2-d points.
fn lerp2(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// Sample the dense polyline at distance `d`.
fn sample_dense_at(
    dense_points: &[[f32; 2]],
    dense_arclen: &[f32],
    total_length: f32,
    d: f32,
) -> [f32; 2] {
    let d = d.clamp(0.0, total_length);
    if d <= 0.0 {
        return dense_points[0];
    }
    if d >= total_length {
        return *dense_points.last().unwrap();
    }
    // Binary search for the segment containing `d`.
    let idx = dense_arclen.partition_point(|&s| s <= d);
    let idx = idx.min(dense_points.len() - 1);
    if idx == 0 {
        return dense_points[0];
    }
    let lo = dense_arclen[idx - 1];
    let hi = dense_arclen[idx];
    let span = hi - lo;
    let t = if span > 0.0 { (d - lo) / span } else { 0.0 };
    lerp2(dense_points[idx - 1], dense_points[idx], t)
}

// ---------------------------------------------------------------------------
// Public Polyline constructors
// ---------------------------------------------------------------------------

/// Clamp `sample_resolution` to the legal range `64..=4096`.
const SAMPLE_MIN: u32 = 64;
const SAMPLE_MAX: u32 = 4096;
/// Suggested default for `sample_resolution` (pass this if you have no specific preference).
#[allow(dead_code)]
pub const SAMPLE_DEFAULT: u32 = 512;

impl Polyline {
    /// Build a `Polyline` from an [`ExtractedPath`].
    ///
    /// `sample_resolution` is the number of uniformly-spaced arc-length samples.
    /// Clamped to `64..=4096`; pass [`SAMPLE_DEFAULT`] (512) for the default.
    ///
    /// # Errors
    /// - [`PathGeomError::NoPaths`] — segments list is empty (should not happen
    ///   when called after a successful [`extract_path`], but guarded defensively).
    /// - [`PathGeomError::ZeroLength`] — the flattened polyline has zero total
    ///   arc-length (degenerate path).
    pub fn build(
        extracted: &ExtractedPath,
        sample_resolution: u32,
    ) -> Result<Polyline, PathGeomError> {
        let n_samples = sample_resolution.clamp(SAMPLE_MIN, SAMPLE_MAX);

        let dense = flatten_to_dense(&extracted.segments);
        if dense.is_empty() {
            return Err(PathGeomError::NoPaths);
        }

        let (dense_arclen, total_length) = build_arclen(&dense);

        if total_length == 0.0 {
            return Err(PathGeomError::ZeroLength);
        }

        let (points, cumulative_arclen) =
            resample_uniform(&dense, &dense_arclen, total_length, n_samples);

        Ok(Polyline {
            points,
            cumulative_arclen,
            total_length,
        })
    }

    /// Sample the polyline at arc-length distance `d`.
    ///
    /// Returns `(position, tangent_unit)` where `tangent_unit` is the unit direction
    /// vector at that point. `d` is clamped to `[0, total_length]`; boundary queries
    /// never panic.
    pub fn sample_at_distance(&self, d: f32) -> ([f32; 2], [f32; 2]) {
        let d = d.clamp(0.0, self.total_length);

        // Find the segment bracket.
        let n = self.points.len();
        if n == 1 {
            return (self.points[0], [1.0, 0.0]);
        }

        let idx = self.cumulative_arclen.partition_point(|&s| s <= d);
        let idx = idx.min(n - 1);

        // Determine which segment to interpolate within.
        let (i_lo, i_hi) = if idx == 0 {
            (0, 1usize.min(n - 1))
        } else if idx >= n - 1 {
            (n - 2, n - 1)
        } else {
            (idx - 1, idx)
        };

        let lo_arclen = self.cumulative_arclen[i_lo];
        let hi_arclen = self.cumulative_arclen[i_hi];
        let span = hi_arclen - lo_arclen;
        let t = if span > 0.0 {
            ((d - lo_arclen) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let p = lerp2(self.points[i_lo], self.points[i_hi], t);

        // Tangent: direction from i_lo → i_hi.
        let dx = self.points[i_hi][0] - self.points[i_lo][0];
        let dy = self.points[i_hi][1] - self.points[i_lo][1];
        let len = (dx * dx + dy * dy).sqrt();
        let tangent = if len > 1e-9 {
            [dx / len, dy / len]
        } else {
            [1.0, 0.0]
        };

        (p, tangent)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helpers -------------------------------------------------------

    /// Minimal SVG wrapper for a path `d` string.
    fn svg_with_paths(paths: &[&str]) -> String {
        let path_els: String = paths
            .iter()
            .enumerate()
            .map(|(i, d)| format!(r#"<path id="p{i}" d="{d}"/>"#))
            .collect::<Vec<_>>()
            .join("\n");
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">{path_els}</svg>"#)
    }

    fn approx_eq(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    fn approx_eq2(a: [f32; 2], b: [f32; 2], tol: f32) -> bool {
        approx_eq(a[0], b[0], tol) && approx_eq(a[1], b[1], tol)
    }

    // ---- Tests ---------------------------------------------------------

    /// A horizontal line from (0,0) to (100,0) should have total_length ≈ 100.
    /// Samples should interpolate along x; tangent should be (1.0, 0.0).
    #[test]
    fn straight_horizontal_line() {
        let svg = svg_with_paths(&["M 0 0 L 100 0"]);
        let ep = extract_path(&svg, 0).expect("extract");
        let poly = Polyline::build(&ep, 512).expect("build");

        assert!(
            approx_eq(poly.total_length, 100.0, 0.5),
            "total_length={} expected ~100",
            poly.total_length
        );

        // sample at distance 0 → first point near (0,0)
        let (p0, t0) = poly.sample_at_distance(0.0);
        assert!(approx_eq2(p0, [0.0, 0.0], 0.5), "start point {p0:?}");
        assert!(approx_eq2(t0, [1.0, 0.0], 0.01), "start tangent {t0:?}");

        // sample at distance 100 → last point near (100,0)
        let (p1, t1) = poly.sample_at_distance(100.0);
        assert!(approx_eq2(p1, [100.0, 0.0], 0.5), "end point {p1:?}");
        assert!(approx_eq2(t1, [1.0, 0.0], 0.01), "end tangent {t1:?}");

        // sample at midpoint
        let (pm, _) = poly.sample_at_distance(50.0);
        assert!(approx_eq(pm[0], 50.0, 1.0), "mid x={}", pm[0]);
        assert!(approx_eq(pm[1], 0.0, 1.0), "mid y={}", pm[1]);
    }

    /// Single cubic Bézier: the arc-length midpoint of a well-known curve should
    /// be close to the geometric midpoint computed by heavy oversampling.
    #[test]
    fn cubic_bezier_midpoint_arclen() {
        // Symmetric cubic from (0,0) to (100,0) with control points pulled up.
        // By symmetry the arc-length midpoint is at the tip: approximately (50, 75).
        let svg = svg_with_paths(&["M 0 0 C 0 100 100 100 100 0"]);
        let ep = extract_path(&svg, 0).expect("extract");

        // Reference: very high sample count to approximate ground truth.
        let hi_res = Polyline::build(&ep, 4096).expect("hi_res");
        let lo_res = Polyline::build(&ep, 512).expect("lo_res");

        let (hi_mid, _) = hi_res.sample_at_distance(hi_res.total_length * 0.5);
        let (lo_mid, _) = lo_res.sample_at_distance(lo_res.total_length * 0.5);

        assert!(
            approx_eq2(lo_mid, hi_mid, 2.0),
            "midpoint mismatch: lo={lo_mid:?} hi={hi_mid:?}"
        );

        // Both should be near x=50 (symmetric curve).
        assert!(
            approx_eq(lo_mid[0], 50.0, 5.0),
            "midpoint x should be near 50, got {}",
            lo_mid[0]
        );
    }

    /// Multi-path SVG: path_index selects the correct element.
    #[test]
    fn multi_path_index_selection() {
        // Two horizontal lines at y=10 and y=20.
        let svg = svg_with_paths(&["M 0 10 L 100 10", "M 0 20 L 100 20"]);

        let ep0 = extract_path(&svg, 0).expect("path 0");
        let ep1 = extract_path(&svg, 1).expect("path 1");

        let poly0 = Polyline::build(&ep0, 512).expect("poly 0");
        let poly1 = Polyline::build(&ep1, 512).expect("poly 1");

        let (start0, _) = poly0.sample_at_distance(0.0);
        let (start1, _) = poly1.sample_at_distance(0.0);

        assert!(approx_eq(start0[1], 10.0, 0.5), "path 0 y={}", start0[1]);
        assert!(approx_eq(start1[1], 20.0, 0.5), "path 1 y={}", start1[1]);
        assert_eq!(ep0.path_count, 2);
    }

    /// path_index out of range should clamp to last path (and warn).
    #[test]
    fn path_index_out_of_range_clamps() {
        let svg = svg_with_paths(&["M 0 10 L 100 10", "M 0 20 L 100 20"]);

        // index 1 is the last valid one; 999 should clamp to 1.
        let ep_last = extract_path(&svg, 1).expect("path 1 direct");
        let ep_clamped = extract_path(&svg, 999).expect("path 999 clamped");

        // Verify clamped result matches last path by comparing start point.
        let poly_last = Polyline::build(&ep_last, 512).expect("poly last");
        let poly_clamped = Polyline::build(&ep_clamped, 512).expect("poly clamped");

        let (p_last, _) = poly_last.sample_at_distance(0.0);
        let (p_clamped, _) = poly_clamped.sample_at_distance(0.0);

        assert!(
            approx_eq2(p_last, p_clamped, 0.01),
            "clamped start {p_clamped:?} should equal last path start {p_last:?}"
        );
    }

    /// Malformed SVG must return the MalformedSvg error variant.
    #[test]
    fn malformed_svg_returns_error() {
        let result = extract_path("this is not xml", 0);
        assert!(
            matches!(result, Err(PathGeomError::MalformedSvg(_))),
            "expected MalformedSvg, got {result:?}"
        );
    }

    /// SVG with no drawable path-like elements returns NoPaths error.
    ///
    /// Note: usvg converts primitive shapes (rect, circle, etc.) into `Node::Path` nodes
    /// during parsing. To test the NoPaths case we use an SVG that contains only a `<defs>`
    /// block (no visible elements), which produces an empty node tree.
    #[test]
    fn no_paths_returns_error() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs/></svg>"#;
        let result = extract_path(svg, 0);
        assert!(
            matches!(result, Err(PathGeomError::NoPaths)),
            "expected NoPaths, got {result:?}"
        );
    }

    /// sample_at_distance(0.0) returns first point; sample_at_distance(total_length) returns last.
    #[test]
    fn sample_at_distance_boundaries() {
        let svg = svg_with_paths(&["M 10 20 L 90 20"]);
        let ep = extract_path(&svg, 0).expect("extract");
        let poly = Polyline::build(&ep, 512).expect("build");

        let (first, _) = poly.sample_at_distance(0.0);
        let (last, _) = poly.sample_at_distance(poly.total_length);

        assert!(approx_eq2(first, [10.0, 20.0], 0.5), "first={first:?}");
        assert!(approx_eq2(last, [90.0, 20.0], 0.5), "last={last:?}");

        // Clamping: values outside range should not panic.
        let (before, _) = poly.sample_at_distance(-10.0);
        let (after, _) = poly.sample_at_distance(poly.total_length + 100.0);
        assert!(approx_eq2(before, first, 0.5), "before={before:?}");
        assert!(approx_eq2(after, last, 0.5), "after={after:?}");
    }
}
