//! Curated mask-polygon starter shapes (T-M12-01).
//!
//! Roadmap §"Phase 2 — Introduce spatial zones as first-class authored
//! objects" calls for a "small semantic palette rather than ... arbitrary
//! low-level shader graphs." Four templates is intentionally tiny — the
//! point is the operator picks a named shape (window, arch, spotlight,
//! void) and drag-edits it with the M11 vertex handles. Anything more
//! exotic stays in the project file's mask_polygon field.
//!
//! Coordinates are normalized output-space `[0, 1]^2`; the SDF baker
//! consumes the same units.

/// 8-vertex circle/sphere approximation for the spotlight zone. More
/// vertices would smooth the silhouette; eight is enough for the SDF
/// `smoothstep` to hide any edge polygon flatness.
const CIRCLE_SAMPLES: usize = 24;

/// Tall rectangle, centered, ~30% wide × ~70% tall. The "project a
/// portrait onto a window" preset — operator drag-aligns to the actual
/// window opening.
pub fn window_rectangle() -> Vec<[f32; 2]> {
    vec![[0.35, 0.1], [0.65, 0.1], [0.65, 0.85], [0.35, 0.85]]
}

/// Bottom-aligned arch: rectangle + half-circle on top. ~13-vertex
/// polygon that drag-edits cleanly. The "project onto a doorway / portal"
/// preset.
pub fn arch_portal() -> Vec<[f32; 2]> {
    let mut pts = Vec::with_capacity(20);
    pts.push([0.3, 0.95]);
    pts.push([0.3, 0.4]);
    // Half-circle from (0.3, 0.4) over (0.5, 0.2) to (0.7, 0.4).
    let arc_samples = 12;
    for i in 0..=arc_samples {
        let t = i as f32 / arc_samples as f32;
        let theta = std::f32::consts::PI * (1.0 - t);
        let x = 0.5 + 0.2 * theta.cos();
        let y = 0.4 - 0.2 * theta.sin();
        pts.push([x, y]);
    }
    pts.push([0.7, 0.4]);
    pts.push([0.7, 0.95]);
    pts
}

/// Centered circle, radius ~25%. The "spotlight on the dance floor"
/// preset.
pub fn circle_spotlight() -> Vec<[f32; 2]> {
    let mut pts = Vec::with_capacity(CIRCLE_SAMPLES);
    let r = 0.25;
    for i in 0..CIRCLE_SAMPLES {
        let theta = 2.0 * std::f32::consts::PI * (i as f32) / (CIRCLE_SAMPLES as f32);
        pts.push([0.5 + r * theta.cos(), 0.5 + r * theta.sin()]);
    }
    pts
}

/// Centered square, ~40% wide. The "block out a no-project area"
/// preset — operator drag-edits to overlap a curtain, fixture, etc.
pub fn void_block() -> Vec<[f32; 2]> {
    vec![[0.3, 0.3], [0.7, 0.3], [0.7, 0.7], [0.3, 0.7]]
}

/// `(name, builder)` pair as it appears in the all-templates listing.
pub type ZoneTemplate = (&'static str, fn() -> Vec<[f32; 2]>);

/// All four templates by `(name, builder)` so the UI can iterate without
/// hard-coding the list.
pub fn all_templates() -> Vec<ZoneTemplate> {
    vec![
        ("window-rectangle", window_rectangle),
        ("arch-portal", arch_portal),
        ("circle-spotlight", circle_spotlight),
        ("void-block", void_block),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_in_unit_square(poly: &[[f32; 2]], name: &str) {
        for (i, p) in poly.iter().enumerate() {
            assert!(
                p[0] >= 0.0 && p[0] <= 1.0 && p[1] >= 0.0 && p[1] <= 1.0,
                "{name} vertex {i} out of [0,1]^2: {p:?}"
            );
        }
    }

    #[test]
    fn templates_are_well_formed() {
        for (name, build) in all_templates() {
            let poly = build();
            assert!(poly.len() >= 3, "{name} has fewer than 3 vertices");
            assert_in_unit_square(&poly, name);
        }
    }

    #[test]
    fn circle_spotlight_is_centered() {
        let pts = circle_spotlight();
        let n = pts.len() as f32;
        let cx: f32 = pts.iter().map(|p| p[0]).sum::<f32>() / n;
        let cy: f32 = pts.iter().map(|p| p[1]).sum::<f32>() / n;
        assert!((cx - 0.5).abs() < 1e-3);
        assert!((cy - 0.5).abs() < 1e-3);
    }
}
