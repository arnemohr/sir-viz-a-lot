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

use crate::windows::theme;

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
    /// P0.1.4 (W5) — procedural mask-driven layer type.
    FxLayer,
    /// P0.1.4 (W6) — live network video stream as a layer source.
    NdiSource,
    /// P0.1.4 (W7) — overlap zone between two projectors.
    EdgeBlendRegion,
    /// P0.1.4 (W8) — 3×3 per-projector colour matrix.
    RgbMatrix,
    /// P0.1.4 (W2) — right-click → bind next MIDI CC workflow.
    MidiLearn,
    /// P0.3.2 (W3) — frames dropped due to bounded-queue overflow.
    DroppedFrames,
    /// P1.1.3 (W2) — named image-grammar preset applied to an Image
    /// or Video layer before the per-pixel effect chain.
    Treatment,
    /// P1.1.3 (W3 / P1.3.1) — tone-map preset.
    ToneMap,
    /// P1.1.3 (W3 / P1.3.2) — blur-mask preset.
    BlurMask,
    /// P1.1.3 (W3 / P1.3.3) — luminance-reveal preset.
    LuminanceReveal,
    /// P1.1.3 (W3 / P1.3.4) — texture-overlay preset.
    TextureOverlay,
    /// P1.1.3 (W3 / P1.3.5) — palette-extract preset.
    PaletteExtract,
    /// P1.1.3 (W3 / P1.3.6) — collage preset.
    Collage,
    /// P1.1.3 (W2 / P1.2.4) — Cover-fit crop anchor.
    FocalPoint,
    /// P1.1.3 (W4 / P1.4.1) — clip-range trim points.
    InOutPoints,
    /// P1.1.3 (W4 / P1.4.2) — Once / Loop / PingPong end-of-clip policy.
    LoopMode,
    /// P1.1.3 (W4 / P1.4.4) — clip rate locked to current BPM.
    BpmLockedPlayback,
    /// P1.1.3 (W4 / P1.4.3) — negative-rate playback via keyframe cache.
    ReversePlayback,
    /// P1.1.3 (W4 / P1.4.5) — pre-decoded thumbnail strip with seek.
    ThumbnailScrub,
    // -----------------------------------------------------------------------
    // P2.1.1 — Phase 2 domain terms and built-in preset display labels.
    // -----------------------------------------------------------------------
    /// P2.1.1 — GPU point / sprite simulation layer.
    Particle,
    /// P2.1.1 — directional influence volume that steers particles.
    ForceField,
    /// P2.1.1 — Navier–Stokes-style velocity-field simulation.
    FluidSim,
    /// P2.1.1 — browsable collection of built-in and user FX presets.
    PresetLibrary,
    /// P2.1.1 — effect or emitter whose output is clipped to the mask boundary.
    MaskConstrained,
    /// P2.1.1 — mask shape used as the spawn surface for an emitter.
    EmitterMasking,
    /// P2.1.1 — outward-pointing normal derived from the mask's distance field.
    SdfNormal,
    /// P2.1.1 — preset family that pushes geometry outward in a wave.
    DisplacementPreset,
    /// P2.1.1 — preset family that bends light at the mask boundary.
    RefractionPreset,
    /// P2.1.1 — preset family that propagates oscillating wave patterns.
    WavePreset,
    /// P2.1.1 — ceiling on active particles for perf control.
    ParticleBudget,
    /// P2.1.1 — deterministic noise seed so a preset looks the same every take.
    SeedDeterminism,
    /// P2.1.1 — drag-to-reorder gesture for the per-layer effect chain.
    EffectChainReorder,
    /// P2.1.1 — preset saved by the operator for their own library.
    UserPreset,
    /// P2.1.1 — factory preset shipped with the application.
    BuiltInPreset,
    /// P2.1.1 — built-in preset: particle ripple spawned along the mask edge.
    MaskEdgeRippleWash,
    /// P2.1.1 — built-in preset: wave wash emitted from the mask edge.
    MaskEdgeWaveWash,
    /// P2.1.1 — built-in preset: particles drift inside the mask boundary.
    MaskConstrainedDrift,
    /// P2.1.1 — built-in preset: sparks emitted outward from the mask edge.
    MaskEdgeEmission,
    /// P2.1.1 — built-in preset: force-field flow driven by the mask shape.
    MaskFieldFlow,
    /// P2.1.1 — built-in preset: particles bounce off the mask boundary.
    MaskCollisionReflection,
    /// P2.1.1 — built-in preset: fluid simulation bounded by the mask.
    MaskBoundedFluid,
    /// P2.1.1 — built-in preset: ripple displacement emanating from the mask.
    DisplacementRipple,
    /// P2.1.1 — built-in preset: lens-refraction distortion at the mask edge.
    Refraction,
    // -----------------------------------------------------------------------
    // P3.1.1 — Phase 3 zone domain terms and role labels.
    // -----------------------------------------------------------------------
    /// P3.1.1 — semantic zone role: a transparent opening (window pane, glass
    /// facade). Distinct from `ZoneTemplate` (a geometry shortcut).
    ZoneRoleWindow,
    /// P3.1.1 — semantic zone role: a visual passageway or threshold that
    /// warrants through-the-surface effects.
    ZoneRolePortal,
    /// P3.1.1 — semantic zone role: a non-emitting blank region that should
    /// remain dark (recessed area, shadow pocket).
    ZoneRoleVoid,
    /// P3.1.1 — semantic zone role: a surface that catches stray light from
    /// a nearby bright zone (wall beside a lit window).
    ZoneRoleSpill,
    /// P3.1.1 — semantic zone role: the perimeter or boundary of a surface
    /// feature (sill, reveal, trim).
    ZoneRoleEdge,
    /// P3.1.1 — semantic zone role: a surface intended to catch a key light
    /// or colour accent (ceiling cove, accentuated panel).
    ZoneRoleHighlight,
    /// P3.1.1 — semantic zone role: a practical luminaire or architectural
    /// element that emits light in the scene (sconce, lantern, ceiling
    /// fixture).
    ZoneRoleLightSource,
    /// P3.1.1 — an FX preset whose behaviour changes based on the zone tag of
    /// the layer it is applied to.
    ZoneAwareShader,
    /// P3.1.1 — the semantic role tag attached to a mask polygon, drawn from
    /// the closed seven-role palette (Window, Portal, Void, Spill, Edge,
    /// Highlight, LightSource). Different from `ZoneTemplate` (a geometry
    /// shortcut for common polygon shapes).
    ZoneTag,
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
            body: "Replaces the projector output with a calibration source.  \
                   Cycle through patterns to check focus, geometry, corner \
                   alignment, and (P0.7.4) two-projector edge-blend overlap \
                   via the alignment cross + edge-blend gradient patterns.",
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
        GlossaryTerm::FxLayer => GlossaryEntry {
            headline: "FX Layer",
            body: "A layer whose visual content is generated from its mask \
                   rather than from media.  v0.4 ships a single proof-point \
                   preset (mask-edge ripple wash); the full library of \
                   particle / wave / fluid presets lands in Phase 2.",
        },
        GlossaryTerm::NdiSource => GlossaryEntry {
            headline: "NDI Source",
            body: "A live video stream received over the network from another \
                   machine (e.g. an OBS instance).  v0.4 supports NDI as input \
                   only; output (Phase 7) lets other apps consume rmap's render.",
        },
        GlossaryTerm::EdgeBlendRegion => GlossaryEntry {
            headline: "Edge-Blend Region",
            body: "The overlap zone between two projectors where image \
                   brightness is feathered so the seam between the two beams \
                   becomes invisible.  Configure overlap width and falloff \
                   curve per edge.",
        },
        GlossaryTerm::RgbMatrix => GlossaryEntry {
            headline: "RGB Matrix",
            body: "A 3×3 colour-correction matrix applied per-projector at \
                   present time, after gamma / brightness / contrast.  Use it \
                   to compensate for differences in projector colour response \
                   when two projectors share the same canvas.",
        },
        GlossaryTerm::MidiLearn => GlossaryEntry {
            headline: "MIDI Learn",
            body: "Right-click any parameter and pick \"Learn next MIDI CC\".  \
                   The next incoming control-change message binds to that \
                   parameter; press ESC to cancel before a CC arrives.",
        },
        GlossaryTerm::DroppedFrames => GlossaryEntry {
            headline: "Dropped Frames",
            body: "How many audio / video / NDI frames the renderer had to \
                   discard because a bounded queue was full.  A non-zero \
                   value during a show means a producer is outpacing the \
                   render thread; investigate before it becomes visible \
                   on the projector.",
        },
        GlossaryTerm::Treatment => GlossaryEntry {
            headline: "Treatment",
            body: "A named image-grammar preset applied to an Image or Video \
                   layer before the per-pixel effect chain.  Picks one preset \
                   per layer (tone map, blur mask, luminance reveal, etc.); \
                   parameters tune the look without exposing dozens of \
                   sliders.",
        },
        GlossaryTerm::ToneMap => GlossaryEntry {
            headline: "Tone Map",
            body: "S-curve exposure + contrast + highlight-rolloff applied \
                   per layer.  Lifts shadows and softens blown highlights so \
                   mixed-light footage matches the rest of the scene without \
                   touching the master gamma slider.",
        },
        GlossaryTerm::BlurMask => GlossaryEntry {
            headline: "Blur Mask",
            body: "Gaussian blur gated by the layer's mask SDF: pixels near \
                   the mask edge get heavy blur, pixels deep inside stay \
                   sharp.  Feathers a photo's silhouette into the background \
                   without losing detail in the centre.",
        },
        GlossaryTerm::LuminanceReveal => GlossaryEntry {
            headline: "Luminance Reveal",
            body: "Pixels brighter than the threshold show; everything else \
                   becomes transparent.  A soft band around the threshold \
                   smooths the cut.  Useful for keying out dark backgrounds \
                   without a proper chroma key (Phase 7).",
        },
        GlossaryTerm::TextureOverlay => GlossaryEntry {
            headline: "Texture Overlay",
            body: "A second image multiplies into the source — paper grain, \
                   noise, film texture, sky gradient.  Pick the overlay file \
                   via the preset; opacity + tint tune how much of the \
                   overlay's colour reaches the result.",
        },
        GlossaryTerm::PaletteExtract => GlossaryEntry {
            headline: "Palette Extract",
            body: "Posterises the source down to a few colours derived from \
                   the image itself (median-cut at layer load; video uses \
                   the first decoded frame).  Dither smooths transitions; \
                   vibrance exaggerates the extracted palette's saturation.",
        },
        GlossaryTerm::Collage => GlossaryEntry {
            headline: "Collage",
            body: "Composites up to four images on one layer in a grid \
                   (1×2, 2×1, or 2×2).  Spacing creates visible gaps between \
                   cells.  Richer authored compositions land alongside scene \
                   grammars in Phase 4.",
        },
        GlossaryTerm::FocalPoint => GlossaryEntry {
            headline: "Focal Point",
            body: "When the layer's fit mode is Cover, the focal point is \
                   the normalised position the crop centres on.  Click the \
                   preview thumbnail to anchor it on the subject (e.g. a \
                   face) so resizing the layer keeps the subject in frame.",
        },
        GlossaryTerm::InOutPoints => GlossaryEntry {
            headline: "In / Out Points",
            body: "Trim a video clip to play only between two timestamps. \
                   Seamless-loop wraps from the out-point back to the in-\
                   point instead of clip 0 → end.  Drag the markers on the \
                   scrub bar to set them.",
        },
        GlossaryTerm::LoopMode => GlossaryEntry {
            headline: "Loop Mode",
            body: "What happens when a video clip reaches its out-point.  \
                   Once stops on the last frame; Loop wraps to the in-point; \
                   PingPong alternates forward and reverse on each end.",
        },
        GlossaryTerm::BpmLockedPlayback => GlossaryEntry {
            headline: "BPM-Locked Playback",
            body: "Locks the clip's playback rate to the current BPM so the \
                   clip plays exactly N beats per loop.  Tap-tempo changes \
                   propagate to the rate on the next frame without re-\
                   encoding.",
        },
        GlossaryTerm::ReversePlayback => GlossaryEntry {
            headline: "Reverse Playback",
            body: "Negative speed plays the clip backwards.  v0.5 uses a \
                   pre-decoded keyframe cache (capped at 30-second clips); \
                   longer clips fall back to forward playback at the \
                   absolute rate with a hint.",
        },
        GlossaryTerm::ThumbnailScrub => GlossaryEntry {
            headline: "Thumbnail Scrub",
            body: "A timeline strip showing pre-decoded mini-frames from the \
                   clip.  Hover to preview a position; click to seek the \
                   playhead there.  Drag the edges to set in/out points.",
        },
        // -------------------------------------------------------------------
        // P2.1.1 — Phase 2 domain terms.
        // -------------------------------------------------------------------
        GlossaryTerm::Particle => GlossaryEntry {
            headline: "Particle",
            body: "A single GPU-simulated sprite that moves, fades, and \
                   disappears over its lifetime.  Hundreds of thousands run \
                   in parallel on the GPU; the Particle Budget slider caps \
                   the count to match your hardware.",
        },
        GlossaryTerm::ForceField => GlossaryEntry {
            headline: "Force Field",
            body: "A directional volume that pushes or pulls particles passing \
                   through it.  Combine several overlapping fields to build \
                   wind, vortex, or gravity effects without touching individual \
                   particle settings.",
        },
        GlossaryTerm::FluidSim => GlossaryEntry {
            headline: "Fluid Sim",
            body: "A velocity-field simulation that makes particles swirl and \
                   flow like smoke or ink.  Viscosity and diffusion sliders \
                   control whether the motion looks like water, fog, or thick \
                   paint.",
        },
        GlossaryTerm::PresetLibrary => GlossaryEntry {
            headline: "Preset Library",
            body: "The browsable panel showing all built-in and saved FX \
                   presets.  Click a preset to apply it to the selected FX \
                   Layer; use the star to pin favourites to the top.",
        },
        GlossaryTerm::MaskConstrained => GlossaryEntry {
            headline: "Mask-Constrained",
            body: "An effect or emitter whose output is clipped to the layer's \
                   mask polygon.  Particles or distortions that cross the \
                   boundary are removed, keeping the look tidy on irregular \
                   surfaces.",
        },
        GlossaryTerm::EmitterMasking => GlossaryEntry {
            headline: "Emitter Masking",
            body: "Uses the layer's mask polygon as the spawn surface for \
                   particles.  New particles appear along the mask edge or \
                   inside its filled area depending on the preset's emitter \
                   mode.",
        },
        GlossaryTerm::SdfNormal => GlossaryEntry {
            headline: "SDF Normal",
            body: "The outward-pointing direction at any point on the mask \
                   boundary, computed from its signed-distance field.  Presets \
                   use this to shoot particles or waves perpendicular to the \
                   mask edge without manual direction tuning.",
        },
        GlossaryTerm::DisplacementPreset => GlossaryEntry {
            headline: "Displacement Preset",
            body: "A preset family that pushes pixels outward from the mask \
                   boundary using a wave or ripple offset.  Strength and \
                   frequency sliders control how far pixels shift and how \
                   quickly the wave cycles.",
        },
        GlossaryTerm::RefractionPreset => GlossaryEntry {
            headline: "Refraction Preset",
            body: "A preset family that bends the layer image at the mask \
                   boundary as if viewed through glass or water.  Index and \
                   blur sliders control the strength of the lens effect.",
        },
        GlossaryTerm::WavePreset => GlossaryEntry {
            headline: "Wave Preset",
            body: "A preset family that propagates oscillating patterns outward \
                   from the mask edge.  Speed, amplitude, and wavelength \
                   sliders let you tune it from a gentle shimmer to a heavy \
                   ripple.",
        },
        GlossaryTerm::ParticleBudget => GlossaryEntry {
            headline: "Particle Budget",
            body: "The maximum number of live particles allowed on this layer \
                   at once.  Lower the budget if you see dropped frames; raise \
                   it for denser effects when headroom is available.  Changes \
                   take effect on the next spawn cycle.",
        },
        GlossaryTerm::SeedDeterminism => GlossaryEntry {
            headline: "Seed (Determinism)",
            body: "A numeric value that locks a preset's random noise to a \
                   fixed sequence.  Two shows with the same seed look \
                   identical.  Change the seed to get a different-but-\
                   repeatable variation of the same preset.",
        },
        GlossaryTerm::EffectChainReorder => GlossaryEntry {
            headline: "Effect-Chain Reorder",
            body: "Drag an effect card up or down in the effect chain to change \
                   the order effects are applied.  Order matters: a blur before \
                   a colour shift produces a different result than the reverse.",
        },
        GlossaryTerm::UserPreset => GlossaryEntry {
            headline: "User Preset",
            body: "An FX preset you saved from your own parameter tweaks.  \
                   User presets appear in the Preset Library alongside built-in \
                   ones; export them to share with other operators.",
        },
        GlossaryTerm::BuiltInPreset => GlossaryEntry {
            headline: "Built-In Preset",
            body: "A factory preset shipped with the application that cannot be \
                   deleted.  Built-in presets are a starting point; duplicate \
                   one to create an editable user preset.",
        },
        // -------------------------------------------------------------------
        // P2.1.1 — Built-in preset display labels.
        // -------------------------------------------------------------------
        GlossaryTerm::MaskEdgeRippleWash => GlossaryEntry {
            headline: "Mask-Edge Ripple Wash",
            body: "Spawns a continuous wash of particles along the mask \
                   boundary, flowing outward as a ripple.  Speed and density \
                   sliders tune how fast and thick the wash appears.",
        },
        GlossaryTerm::MaskEdgeWaveWash => GlossaryEntry {
            headline: "Mask-Edge Wave Wash",
            body: "Emits an oscillating wave wash from the mask edge that \
                   crests and fades across the layer surface.  Amplitude and \
                   frequency control the height and spacing of each wave crest.",
        },
        GlossaryTerm::MaskConstrainedDrift => GlossaryEntry {
            headline: "Mask-Constrained Drift",
            body: "Fills the mask interior with slowly drifting particles that \
                   stay within the boundary.  A gentle effect for giving a \
                   still image a sense of quiet movement without visible \
                   directional flow.",
        },
        GlossaryTerm::MaskEdgeEmission => GlossaryEntry {
            headline: "Mask-Edge Emission",
            body: "Shoots sparks outward from the mask perimeter, following \
                   the SDF normal direction.  Burst rate and spread angle let \
                   you scale from occasional glints to a constant halo.",
        },
        GlossaryTerm::MaskFieldFlow => GlossaryEntry {
            headline: "Mask Field Flow",
            body: "A force-field preset whose flow lines follow the mask \
                   boundary shape, carrying particles along the contour.  \
                   Works well on text or logo masks to trace their outline \
                   with motion.",
        },
        GlossaryTerm::MaskCollisionReflection => GlossaryEntry {
            headline: "Mask Collision Reflection",
            body: "Particles fill the layer and bounce elastically off the \
                   mask boundary as if the edge were a solid wall.  \
                   Restitution and particle size sliders control how \
                   energetic the collisions look.",
        },
        GlossaryTerm::MaskBoundedFluid => GlossaryEntry {
            headline: "Mask-Bounded Fluid",
            body: "A fluid simulation that treats the mask polygon as a \
                   container: the velocity field swirls inside but cannot \
                   escape the boundary.  Viscosity sets the feel from \
                   thin water to heavy oil.",
        },
        GlossaryTerm::DisplacementRipple => GlossaryEntry {
            headline: "Displacement Ripple",
            body: "Pushes pixels in a radial ripple pattern emanating from the \
                   mask edge, distorting the layer image as if a stone were \
                   dropped into the surface.  Speed sets the ring expansion \
                   rate; strength sets maximum pixel offset.",
        },
        GlossaryTerm::Refraction => GlossaryEntry {
            headline: "Refraction",
            body: "Bends the layer image at the mask boundary using a \
                   lens-distortion pass, creating a glass-edge or water-\
                   surface appearance.  Index controls how strongly light \
                   bends; blur softens the distorted region.",
        },
        // -------------------------------------------------------------------
        // P3.1.1 — Phase 3 zone domain terms.
        // -------------------------------------------------------------------
        GlossaryTerm::ZoneRoleWindow => GlossaryEntry {
            headline: "Zone Role: Window",
            body: "Tags a mask as a transparent opening (window pane, glass \
                   facade).  Zone-aware presets such as \"light spill from \
                   window zones\" read this tag and activate their effect only \
                   on layers with this role.  Unlike Zone Template, this is a \
                   semantic label — not a geometry shortcut.",
        },
        GlossaryTerm::ZoneRolePortal => GlossaryEntry {
            headline: "Zone Role: Portal",
            body: "Tags a mask as a visual passageway or threshold (archway, \
                   doorway, gap in a surface).  Presets such as \"particle drift \
                   through portal zones\" spawn their effect inside this \
                   boundary.  Apply to any region where you want through-the-\
                   surface-style visual depth.",
        },
        GlossaryTerm::ZoneRoleVoid => GlossaryEntry {
            headline: "Zone Role: Void",
            body: "Tags a mask as a non-emitting blank region intended to stay \
                   dark — a recessed area, shadow pocket, or unlit surface.  \
                   Zone-aware presets skip or invert their effect for Void-tagged \
                   layers so the dark region stays visually quiet.",
        },
        GlossaryTerm::ZoneRoleSpill => GlossaryEntry {
            headline: "Zone Role: Spill",
            body: "Tags a mask as a surface that catches stray light from a \
                   nearby bright zone — the wall beside a lit window, or a \
                   floor below a lantern.  Presets respond with a softer, \
                   indirect-light treatment rather than the full brightness \
                   reserved for the source zone.",
        },
        GlossaryTerm::ZoneRoleEdge => GlossaryEntry {
            headline: "Zone Role: Edge",
            body: "Tags a mask as the perimeter or boundary of a surface feature \
                   (window sill, door reveal, architectural trim).  The \
                   \"ripple at edge zones\" preset amplifies its wave effect \
                   specifically at Edge-tagged layers.  Distinct from the mask \
                   feather, which is a render setting rather than a semantic role.",
        },
        GlossaryTerm::ZoneRoleHighlight => GlossaryEntry {
            headline: "Zone Role: Highlight",
            body: "Tags a mask as a surface intended to catch a key light or \
                   colour accent — a ceiling cove, decorative panel, or any \
                   area you want to pop.  Zone-aware presets can use this tag \
                   to brighten or saturate the region relative to the rest of \
                   the scene.",
        },
        GlossaryTerm::ZoneRoleLightSource => GlossaryEntry {
            headline: "Zone Role: Light Source",
            body: "Tags a mask as a practical luminaire or architectural \
                   element that emits light in the scene — a sconce, lantern, \
                   or ceiling fixture.  Phase 5 will let fixtures bind to \
                   Light Source zone activity; in Phase 3 this tag acts as a \
                   semantic label FX presets can branch on.",
        },
        GlossaryTerm::ZoneAwareShader => GlossaryEntry {
            headline: "Zone-Aware Shader",
            body: "An FX preset whose behaviour adapts based on the zone tag \
                   of the layer it is applied to.  A zone-aware preset reads \
                   the tag at runtime and activates its effect only when the \
                   tag matches its target role; applying it to an untagged layer \
                   produces a neutral (transparent) output.",
        },
        GlossaryTerm::ZoneTag => GlossaryEntry {
            headline: "Zone Tag",
            body: "The semantic role attached to a mask polygon, chosen from \
                   the closed seven-role palette: Window, Portal, Void, Spill, \
                   Edge, Highlight, or Light Source.  Set the tag in the zone \
                   palette inside Mask mode.  Unlike Zone Template (a geometry \
                   shortcut for common polygon shapes), a Zone Tag is purely \
                   semantic — it does not change the polygon's shape.",
        },
    }
}

// ---------------------------------------------------------------------------
// T5.12 — canonical term list for the in-app Glossary window.
// ---------------------------------------------------------------------------

/// All [`GlossaryTerm`] variants in display order.
///
/// This is the single source of truth for both the `lint_terms_have_entries`
/// unit test and the in-app Glossary window — keeping them in sync is
/// automatic.  When you add a new variant to [`GlossaryTerm`] you must also
/// add it here (the compiler will remind you if you forget the `entry()` arm;
/// this list is the "add copy alongside code" tax).
pub fn all_terms() -> &'static [GlossaryTerm] {
    &[
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
        GlossaryTerm::FxLayer,
        GlossaryTerm::NdiSource,
        GlossaryTerm::EdgeBlendRegion,
        GlossaryTerm::RgbMatrix,
        GlossaryTerm::MidiLearn,
        GlossaryTerm::DroppedFrames,
        // P1.1.3 — Phase 1 domain terms.
        GlossaryTerm::Treatment,
        GlossaryTerm::ToneMap,
        GlossaryTerm::BlurMask,
        GlossaryTerm::LuminanceReveal,
        GlossaryTerm::TextureOverlay,
        GlossaryTerm::PaletteExtract,
        GlossaryTerm::Collage,
        GlossaryTerm::FocalPoint,
        GlossaryTerm::InOutPoints,
        GlossaryTerm::LoopMode,
        GlossaryTerm::BpmLockedPlayback,
        GlossaryTerm::ReversePlayback,
        GlossaryTerm::ThumbnailScrub,
        // P2.1.1 — Phase 2 domain terms.
        GlossaryTerm::Particle,
        GlossaryTerm::ForceField,
        GlossaryTerm::FluidSim,
        GlossaryTerm::PresetLibrary,
        GlossaryTerm::MaskConstrained,
        GlossaryTerm::EmitterMasking,
        GlossaryTerm::SdfNormal,
        GlossaryTerm::DisplacementPreset,
        GlossaryTerm::RefractionPreset,
        GlossaryTerm::WavePreset,
        GlossaryTerm::ParticleBudget,
        GlossaryTerm::SeedDeterminism,
        GlossaryTerm::EffectChainReorder,
        GlossaryTerm::UserPreset,
        GlossaryTerm::BuiltInPreset,
        // P2.1.1 — Built-in preset display labels.
        GlossaryTerm::MaskEdgeRippleWash,
        GlossaryTerm::MaskEdgeWaveWash,
        GlossaryTerm::MaskConstrainedDrift,
        GlossaryTerm::MaskEdgeEmission,
        GlossaryTerm::MaskFieldFlow,
        GlossaryTerm::MaskCollisionReflection,
        GlossaryTerm::MaskBoundedFluid,
        GlossaryTerm::DisplacementRipple,
        GlossaryTerm::Refraction,
        // P3.1.1 — Phase 3 zone domain terms.
        GlossaryTerm::ZoneRoleWindow,
        GlossaryTerm::ZoneRolePortal,
        GlossaryTerm::ZoneRoleVoid,
        GlossaryTerm::ZoneRoleSpill,
        GlossaryTerm::ZoneRoleEdge,
        GlossaryTerm::ZoneRoleHighlight,
        GlossaryTerm::ZoneRoleLightSource,
        GlossaryTerm::ZoneAwareShader,
        GlossaryTerm::ZoneTag,
    ]
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
                .color(theme::TEXT_SECONDARY),
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
    /// Uses `all_terms()` as the single source of truth for the term list:
    /// adding a new variant requires updating `all_terms()` and the `entry()`
    /// match; this test then automatically covers the new variant.
    #[test]
    fn lint_terms_have_entries() {
        for &t in super::all_terms() {
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

    /// T5.12 — `all_terms()` covers every variant (no missing terms).
    ///
    /// The exhaustive `entry()` match is the compile-time guard against
    /// *missing* variants; this test is the runtime guard against *forgetting
    /// to add to `all_terms()`*. The expected count must be bumped whenever
    /// a new `GlossaryTerm` variant is added.
    #[test]
    fn all_terms_covers_every_variant() {
        // Bump this when you add a new GlossaryTerm variant.
        const EXPECTED_VARIANT_COUNT: usize = 72;
        assert_eq!(
            super::all_terms().len(),
            EXPECTED_VARIANT_COUNT,
            "all_terms() has {} entries but expected {}. \
             Add the new variant to all_terms() and bump EXPECTED_VARIANT_COUNT.",
            super::all_terms().len(),
            EXPECTED_VARIANT_COUNT,
        );
    }
}
