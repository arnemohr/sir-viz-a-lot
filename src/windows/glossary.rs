//! 003-T3.19 / T3.20 / T3.22 — Typed-enum glossary with `?` popovers.
//!
//! # Architecture
//!
//! [`GlossaryTerm`] is the only valid input to [`glossary_label`].  Adding a
//! new term requires:
//!   1. A new variant in [`GlossaryTerm`].
//!   2. A matching arm in [`entry`] — the exhaustive `match` (no `_` wildcard)
//!      makes omitting an arm a **compile error**.
//!
//! The [`lint_terms_have_entries`] unit test (T3.22) additionally guards
//! against empty-body arms (content debt).  It must be kept in sync with the
//! variant list — that's intentional: updating it is the "pay the tax" moment
//! that ensures copy lands alongside code.
//!
//! This module is `#[cfg(feature = "v3")]`-only; see `src/windows/mod.rs`.

/// Every domain term that can appear in the Advanced panel.
///
/// One variant per term — a typo is a compile error, not a runtime surprise.
///
/// Some variants are wired in by T3.21 (currently applied); others are
/// reserved for subsequent tasks that introduce their UI (T3.23 show-day
/// strip, T3.13 modulator, etc.).  The `allow(dead_code)` suppresses the
/// "never constructed" lint for forward-reserved variants.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlossaryTerm {
    Warp,
    MaskPolygon,
    Modulator,
    Gamma,
    Brightness,
    Contrast,
    BlendMode,
    Crossfade,
    Scene,
    ZoneTemplate,
    Blackout,
    Freeze,
    TestPattern,
    EditorOverlay,
    Effect,
    FitMode,
    MaskFeather,
    GridDetail,
    Opacity,
    /// 003-T3.28 — per-display tone overrides.
    DisplayOverride,
}

/// A single glossary entry: a short headline and a 1–2 sentence body.
pub struct GlossaryEntry {
    pub headline: &'static str,
    pub body: &'static str,
}

/// Look up the glossary entry for a term.
///
/// The `match` is **exhaustive** — no `_` arm — so a missing entry is a
/// compile error, not a silent blank popover.
pub fn entry(t: GlossaryTerm) -> GlossaryEntry {
    match t {
        GlossaryTerm::Warp => GlossaryEntry {
            headline: "Warp",
            body: "Per-layer corner-pin quad that places the layer on the projector. \
                   Each corner is a point in projector space; drag a corner to \
                   move/resize/distort the layer on the wall directly. Increase \
                   rows × cols (Advanced → Mapping) for finer mesh control.",
        },
        GlossaryTerm::MaskPolygon => GlossaryEntry {
            headline: "Mask Polygon",
            body: "A polygon drawn over the layer that blocks the image outside its \
                   boundary.  Use it to hide parts of the projection that fall on \
                   surfaces you don't want lit (doors, windows, pillars).",
        },
        GlossaryTerm::Modulator => GlossaryEntry {
            headline: "Modulator",
            body: "How a parameter changes over time.  \
                   `Static` holds a fixed value; `Sine`, `Sawtooth`, `Square`, \
                   `Random`, and `Bound` animate it on a repeating cycle.",
        },
        GlossaryTerm::Gamma => GlossaryEntry {
            headline: "Gamma",
            body: "Overall brightness curve of the output.  \
                   Values above 1.0 lift the midtones (brighter); \
                   below 1.0 they darken.  Start at 1.0 and nudge if \
                   the projector looks washed-out or too dark.",
        },
        GlossaryTerm::Brightness => GlossaryEntry {
            headline: "Brightness",
            body: "Additive offset applied to every pixel after gamma.  \
                   Positive values push towards white; negative values \
                   push towards black.  0.0 leaves the image unchanged.",
        },
        GlossaryTerm::Contrast => GlossaryEntry {
            headline: "Contrast",
            body: "Stretches the range between dark and bright.  \
                   1.0 is unchanged; higher values increase separation \
                   between shadows and highlights.",
        },
        GlossaryTerm::BlendMode => GlossaryEntry {
            headline: "Blend Mode",
            body: "How a layer's pixels combine with the layers below it.  \
                   Normal overwrites; Add makes bright areas brighter (good for \
                   fire / glow); Multiply darkens; Screen is similar to Add but \
                   gentler with very bright content.",
        },
        GlossaryTerm::Crossfade => GlossaryEntry {
            headline: "Crossfade",
            body: "Smooth transition between two scenes.  The duration controls \
                   how many seconds the blend takes to complete.",
        },
        GlossaryTerm::Scene => GlossaryEntry {
            headline: "Scene",
            body: "A saved snapshot of all layer content, positions, and settings.  \
                   Switch scenes mid-show to jump between prepared looks.",
        },
        GlossaryTerm::ZoneTemplate => GlossaryEntry {
            headline: "Zone Template",
            body: "A preset mask polygon shape (full, left half, right half, \
                   top/bottom split, etc.) that you can apply in one click instead \
                   of drawing vertices manually.",
        },
        GlossaryTerm::Blackout => GlossaryEntry {
            headline: "Blackout",
            body: "Cuts the projector output to black instantly.  \
                   The show continues running in the background; \
                   press again to restore.",
        },
        GlossaryTerm::Freeze => GlossaryEntry {
            headline: "Freeze",
            body: "Holds the last rendered frame on the projector while the \
                   control window keeps updating.  Useful for swapping content \
                   without the audience seeing the edit.",
        },
        GlossaryTerm::TestPattern => GlossaryEntry {
            headline: "Test Pattern",
            body: "Replaces the projector output with a calibration grid.  \
                   Cycle through patterns to check focus, geometry, \
                   and corner alignment.",
        },
        GlossaryTerm::EditorOverlay => GlossaryEntry {
            headline: "Editor Overlay",
            body: "Draws warp-grid handles and mask-polygon vertices on the \
                   projector output.  Toggle off during a live show so the \
                   audience never sees editing chrome.",
        },
        GlossaryTerm::Effect => GlossaryEntry {
            headline: "Effect",
            body: "A real-time filter applied to a single layer \
                   (e.g. Transform moves/scales, others tint or distort).  \
                   Effects stack in order; drag to reorder.",
        },
        GlossaryTerm::FitMode => GlossaryEntry {
            headline: "Fit Mode",
            body: "How the layer's source image fills its bounding rectangle.  \
                   `Stretch` fills exactly; `Contain` letterboxes; \
                   `Cover` crops to fill with no bars.",
        },
        GlossaryTerm::MaskFeather => GlossaryEntry {
            headline: "Mask Feather",
            body: "Soft fade at the edge of the mask polygon.  \
                   0 = hard edge; values up to about 0.5 produce a \
                   useful graduated blend between lit and dark.",
        },
        GlossaryTerm::GridDetail => GlossaryEntry {
            headline: "Grid Detail",
            body: "How many cells make up the warp mesh (rows × columns).  \
                   More cells allow finer local warps but add more control \
                   points to align (see Warp).",
        },
        GlossaryTerm::Opacity => GlossaryEntry {
            headline: "Opacity",
            body: "Overall transparency of the layer: 1.0 is fully opaque, \
                   0.0 is invisible.  Useful for subtle overlays or \
                   for fading content in and out.",
        },
        GlossaryTerm::DisplayOverride => GlossaryEntry {
            headline: "Display Override",
            body: "Optional per-projector gamma / brightness / contrast that \
                   replaces the master values for the projector output only.  \
                   Tune the master for what looks right on your laptop, then \
                   set the override for what looks right on the wall.",
        },
    }
}

// ---------------------------------------------------------------------------
// T3.19 — glossary_label primitive
// ---------------------------------------------------------------------------

/// Render a domain-term label with a small `?` icon to its right.
///
/// Hovering over either the label text or the `?` triggers an egui popover
/// after the default tooltip delay (≈ 250 ms), satisfying T3.19 §3
/// ("transient cursor passes don't trigger").
///
/// The `?` is rendered in a low-contrast colour so it doesn't compete
/// with the label text for visual weight.
pub fn glossary_label(ui: &mut egui::Ui, term: GlossaryTerm) -> egui::Response {
    let e = entry(term);
    let mut total_resp: Option<egui::Response> = None;
    ui.horizontal(|ui| {
        let r = ui.label(e.headline);
        let q = ui.label(
            egui::RichText::new(" ?")
                .small()
                .color(egui::Color32::from_gray(140)),
        );
        let combined = r.union(q);
        let with_tip = combined.on_hover_ui(|ui| {
            ui.set_max_width(280.0);
            ui.strong(e.headline);
            ui.add_space(4.0);
            ui.small(e.body);
        });
        total_resp = Some(with_tip);
    });
    total_resp.expect("horizontal closure always runs once")
}

// ---------------------------------------------------------------------------
// T3.22 — content-debt guard
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    /// T3.22 — every GlossaryTerm variant produces a non-empty entry.
    ///
    /// The exhaustive `match` in `entry()` ensures a *missing* arm is a
    /// **compile error**.  This test catches the complementary failure:
    /// an arm that exists but has empty or suspiciously short copy.
    ///
    /// IMPORTANT: when you add a new `GlossaryTerm` variant, also add it
    /// to `all` below — the compiler won't remind you here, only in `entry()`.
    #[test]
    fn lint_terms_have_entries() {
        let all = [
            GlossaryTerm::Warp,
            GlossaryTerm::MaskPolygon,
            GlossaryTerm::Modulator,
            GlossaryTerm::Gamma,
            GlossaryTerm::Brightness,
            GlossaryTerm::Contrast,
            GlossaryTerm::BlendMode,
            GlossaryTerm::Crossfade,
            GlossaryTerm::Scene,
            GlossaryTerm::ZoneTemplate,
            GlossaryTerm::Blackout,
            GlossaryTerm::Freeze,
            GlossaryTerm::TestPattern,
            GlossaryTerm::EditorOverlay,
            GlossaryTerm::Effect,
            GlossaryTerm::FitMode,
            GlossaryTerm::MaskFeather,
            GlossaryTerm::GridDetail,
            GlossaryTerm::Opacity,
            GlossaryTerm::DisplayOverride,
        ];
        for t in all {
            let e = entry(t);
            assert!(!e.headline.is_empty(), "headline empty for {t:?}");
            assert!(!e.body.is_empty(), "body empty for {t:?}");
            assert!(
                e.body.len() > 20,
                "body suspiciously short for {t:?}: {:?}",
                e.body
            );
        }
    }
}
