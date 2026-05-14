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
    // -----------------------------------------------------------------------
    // P4.1.1 — Phase 4 scene-grammar domain terms + template display labels.
    // -----------------------------------------------------------------------
    /// P4.1.1 — a named, parameterised recipe that assembles media, zones, and
    /// FX presets into a ready-to-run scene.
    SceneTemplate,
    /// P4.1.1 — the structural vocabulary of a scene: which zone roles it
    /// binds, which media it accepts, which FX presets it activates.
    SceneGrammar,
    /// P4.1.1 — a guided multi-step UI (template → media → zones → palette →
    /// tempo) for creating a scene from a scene template.
    Wizard,
    /// P4.1.1 — a curated set of warm, cool, or neutral accent colours used
    /// to tint a scene template's output.
    PaletteMood,
    /// P4.1.1 — the emotional character of a scene (Calm, Energetic, or
    /// Ethereal) that templates use to pre-select parameter ranges.
    Mood,
    /// P4.1.1 — locking a template's animation speed to the project BPM so
    /// the scene moves in time with the music.
    TempoSync,
    /// P4.1.1 — Bezier handles for mask polygon editing, letting operators
    /// draw smooth curves instead of angular vertices.
    BezierHandles,
    /// P4.1.1 — built-in scene template: soft light flows through tagged
    /// window zones revealing the scene beneath.
    WindowReveal,
    /// P4.1.1 — built-in scene template: fine particles drift slowly across
    /// the source media surface.
    PixelDrift,
    /// P4.1.1 — built-in scene template: four-image collage with particle
    /// blooms at each image edge.
    CollagBloom,
    /// P4.1.1 — built-in scene template: fluid light pools behind portal
    /// zones, evoking glow from architectural openings.
    GlowBehindOpenings,
    /// P4.1.1 — built-in scene template: portrait image broken into
    /// fragments by colliding particles at the mask boundary.
    FragmentedPortrait,
    /// P4.1.1 — built-in scene template: a gentle wave wash that traces the
    /// edge zones of architectural surfaces.
    ArchitecturalWash,
    /// P4.1.1 — built-in scene template: the classic mask-edge ripple wash
    /// as a standalone scene (no source media required).
    MaskEdgeRippleWashScene,
    /// P4.1.1 — built-in scene template: light appears to spill outward from
    /// tagged window zones as if leaking from an interior source.
    LightSpillFromWindows,
    // -----------------------------------------------------------------------
    // P5.1.1 — Phase 5 lighting output domain terms.
    // -----------------------------------------------------------------------
    /// P5.1.1 — a 512-channel DMX universe, the fundamental unit of lighting
    /// network traffic. Art-Net and sACN both address fixtures by universe
    /// number + channel offset.
    DmxUniverse,
    /// P5.1.1 — a named collection of fixtures sharing the same personality,
    /// DMX universe, and canvas sampling region. The operator's primary
    /// lighting object in rmap.
    FixtureGroup,
    /// P5.1.1 — a grid of sample points that maps a canvas region to a
    /// fixture group's DMX channels row-by-row, column-by-column.
    PixelMap,
    /// P5.1.1 — abstraction over the DMX wire protocol; Phase 5 ships
    /// Art-Net; Phase 7 can add sACN by implementing a second instance of
    /// this trait without changing the fixture or sampling code.
    DmxTransport,
    /// P5.1.1 — the Art-Net UDP transport that implements `DmxTransport`;
    /// sends Art-Net `ArtDmx` PDUs to a configurable unicast or broadcast
    /// address on port 6454.
    ArtNetTransport,
    /// P5.1.1 — the role of a single DMX channel within a fixture's footprint
    /// (e.g. Red, Green, Blue). Phase 5 ships three roles; Phase 7 adds White
    /// and colour-temperature channels additively.
    ChannelRole,
    /// P5.1.1 — the colour-space conversion applied when mapping a sampled
    /// canvas pixel to DMX byte values. Phase 5 ships RGB Direct and HSV
    /// Intensity Gate; Phase 7 adds RGBW Fill.
    ColorStrategy,
    /// P5.1.1 — the low-resolution (64×36) texture blit that downsamples the
    /// composited canvas once per frame so the lighting thread can sample any
    /// fixture region without stalling the render thread.
    LightingTap,
    /// P5.1.1 — the per-fixture-group choice of how canvas colour maps to DMX
    /// values: RGB Direct copies pixel bytes; HSV Intensity Gate dims the
    /// fixture by the pixel's brightness; more strategies land in Phase 7.
    OutputStrategy,
    // -----------------------------------------------------------------------
    // P6.1.1 — Phase 6 show-control and timecode domain terms.
    // -----------------------------------------------------------------------
    /// P6.1.1 — a single entry in the cuelist: a saved scene snapshot plus
    /// per-cue timing, fire mode, and optional timecode trigger fields.
    Cue,
    /// P6.1.1 — the ordered list of cues the operator steps through during
    /// a live show.
    Cuelist,
    /// P6.1.1 — the next cue in the list, highlighted in the strip and ready
    /// to fire on the next Space / MIDI Note 60 / OSC go command.
    ArmedNext,
    /// P6.1.1 — the cue currently playing or crossfading on the projector.
    LiveCue,
    /// P6.1.1 — a sequence of Follow-mode cues that fire automatically one
    /// after another without operator input.
    FollowChain,
    /// P6.1.1 — a cue fire mode that waits for a Space / MIDI / OSC trigger
    /// before advancing to the next cue.
    GoOnTrigger,
    /// P6.1.1 — a cue fire mode that fires the next cue automatically after
    /// the current cue's hold time expires.
    FollowMode,
    /// P6.1.1 — the crossfade duration from the previous cue state into this
    /// cue's scene snapshot (seconds).
    InTime,
    /// P6.1.1 — how long the cue stays fully live before the follow chain or
    /// operator trigger can advance to the next cue (seconds or indefinite).
    HoldTime,
    /// P6.1.1 — the crossfade duration from this cue's scene out to the next
    /// cue's in-time (seconds; usually 0 because `in_time` of the next cue
    /// handles the blend).
    OutTime,
    /// P6.1.1 — fires a cue on the next 1 / 2 / 4 / 8-bar beat boundary
    /// at the current BPM instead of immediately on the trigger event.
    BpmQuantize,
    /// P6.1.1 — fires a cue automatically when the incoming timecode signal
    /// reaches a specific HH:MM:SS:FF position.
    TimecodePosition,
    /// P6.1.1 — persistent on-screen readout showing live BPM, tap source,
    /// armed-cue name, and quantize selector.
    TransportHud,
    /// P6.1.1 — SMPTE Linear Timecode carried as an audio-rate signal; decoded
    /// via the `ltc` cargo feature to drive automatic cue firing.
    Ltc,
    /// P6.1.1 — MIDI Timecode: quarter-frame messages (status 0xF1) that
    /// assemble into HH:MM:SS:FF positions for cue triggering.
    Mtc,
    /// P6.1.1 — MIDI timing clock: 24 pulses per quarter note (status 0xF8)
    /// used to derive a live BPM and optionally drive cue quantize.
    MidiClock,
    // -----------------------------------------------------------------------
    // P7.1.1 — Phase 7 domain terms.
    // -----------------------------------------------------------------------
    /// P7.1.1 — macOS inter-application video sharing protocol that lets rmap
    /// feed its output to OBS, VDMX, Resolume Arena, and other Syphon-aware
    /// applications on the same machine without a capture card.
    Syphon,
    /// P7.1.1 — the rmap feature that publishes the composited projector output
    /// as a named Syphon source visible to other macOS applications.
    SyphonOutput,
    /// P7.1.1 — a separate `.rmap-calibration.json` file that stores the warp,
    /// mask, gamma, and monitor identity for a physical venue, independent of
    /// any show file, so the same geometry can be reused across shows.
    CalibrationFile,
    /// P7.1.1 — a logical output slot in the calibration file identified by a
    /// stable UUID, bound to a physical display by the show file's OutputTarget.
    SurfaceSlot,
    /// P7.1.1 — the per-venue calibration data (warp + mask + gamma + display
    /// identity) that travels with a venue rather than with a show file, enabling
    /// one geometry setup to serve many different shows.
    VenueCalibration,
    /// P7.1.1 — a warp mesh whose edges are defined by cubic Bezier curves,
    /// allowing smooth curved surfaces (columns, arches, organic walls) that
    /// bilinear quads cannot describe.
    BezierWarp,
    /// P7.1.1 — a corner point of a Bezier warp mesh that lies exactly on the
    /// surface; dragging an anchor moves the point and its attached handles.
    Anchor,
    /// P7.1.1 — a control point attached to an anchor that adjusts the curvature
    /// of the Bezier edge; dragging a handle bows the edge without moving the
    /// anchor itself.
    TangentHandle,
    /// P7.1.1 — a mask mode that swaps the opaque and transparent regions of the
    /// mask polygon: areas previously blocked become revealed, and vice versa.
    InverseMask,
    /// P7.1.1 — a mask mode that derives alpha from the brightness of the
    /// rendered output: bright pixels become opaque, dark pixels become
    /// transparent (or the inverse), driven by threshold and softness sliders.
    LumaKey,
    /// P7.1.1 — a mask mode that derives alpha from a hue range in the rendered
    /// output: pixels whose hue, saturation, and value fall within the configured
    /// range become transparent, enabling green-screen or colour-spill removal.
    ChromaKey,
    /// P7.1.1 — a four-channel DMX colour model (Red, Green, Blue, White) used
    /// by warm-white architectural LED fixtures; the White channel carries the
    /// dominant neutral component, reducing colour noise at high intensities.
    Rgbw,
    /// P7.1.1 — the perceived warmth or coolness of a light source, expressed in
    /// Kelvin; lower values (2700 K) appear warm amber, higher values (6500 K)
    /// appear cool blue-white.  rmap uses CCT to compute which fraction of the
    /// sampled canvas colour should flow into an RGBW fixture's White channel.
    ColourTemperature,
    /// P7.1.1 — short for Correlated Colour Temperature; the Kelvin value that
    /// characterises the white point of a light source or fixture group.
    Cct,
    /// P7.1.1 — a portable `.rmap-scene-pack.zip` archive that bundles one or
    /// more scene templates with their referenced assets, enabling template
    /// sharing across projects and between operators.
    ScenePack,
    /// P7.1.1 — a calibration verify pattern that fades from opaque to
    /// transparent across one or more screen edges, used to set up and verify
    /// edge-blend overlap zones between adjacent projectors.
    EdgeBlendGradient,
    /// P7.1.1 — a full-screen test pattern rendered on the projector output to
    /// verify warp accuracy, colour balance, focus, or edge alignment; activated
    /// from the Output panel without affecting the show file.
    CalibrationVerify,
    // -----------------------------------------------------------------------
    // PCleanup.0.1 — Cleanup phase architectural variants + W2 SourceModifier
    // preset names. See specs/004-phase-cleanup.md and the W2 task list in
    // specs/004-phase-cleanup-tasks.md.
    // -----------------------------------------------------------------------
    /// PCleanup.0.1 — FX preset family that reads the underlying layer image
    /// and writes a modified version, as opposed to a generative overlay that
    /// paints its own pixels on top.
    FxFamilySourceModifier,
    /// PCleanup.0.1 — per-layer Treatment effect chain variant: applies any
    /// treatment (tone map, displacement ripple, refraction, etc.) to a single
    /// layer rather than the whole composited frame.
    EffectTreatment,
    /// PCleanup.0.1 — effect that blends the previous frame's layer output back
    /// into the current frame, producing trails, echoes, and motion smear with
    /// modulator-driven decay.
    EffectFeedback,
    /// PCleanup.0.1 — three-mode colour-mixing effect: multiply (proper tint),
    /// additive (wash), or screen, with a modulator-driven amount.
    EffectTint,
    /// PCleanup.0.1 — mask-graph node that combines two mask polygons so the
    /// result covers either region: a Boolean union of overlapping shapes.
    MaskNodeUnion,
    /// PCleanup.0.1 — mask-graph node that cuts one polygon out of another:
    /// the first mask's coverage minus the second mask's coverage.
    MaskNodeSubtract,
    /// PCleanup.0.1 — built-in SourceModifier preset: 2D fluid simulation
    /// inside the mask, displacing the underlying photo by the velocity field.
    FluidWarp,
    /// PCleanup.0.1 — built-in SourceModifier preset: concentric rings from the
    /// mask edge act as refraction lenses bulging the underlying image.
    RippleLens,
    /// PCleanup.0.1 — built-in SourceModifier preset: four traveling refraction
    /// bumps orbit the mask edge, distorting the image at each crest.
    EdgeLens,
    /// PCleanup.0.1 — built-in SourceModifier preset: like FluidWarp but
    /// unbounded — the underlying photo flows across the entire layer.
    FluidWarpFull,
    /// PCleanup.0.1 — built-in SourceModifier preset: each particle is a soft
    /// Gaussian brightener that lifts source-pixel luminance in its radius.
    Spotlights,
    /// PCleanup.0.1 — built-in SourceModifier preset: particles drift inside
    /// the mask as pinholes through which the underlying photo is visible.
    DriftPinholes,
    /// PCleanup.0.1 — built-in SourceModifier preset: particles fly outward
    /// from the mask edge, additively lifting source luminance in a soft radius.
    EdgeSparks,
    /// PCleanup.0.1 — built-in SourceModifier preset: uses the mask SDF
    /// gradient field to advect the underlying photo along the mask normals.
    FieldAdvectSource,
    /// PCleanup.0.1 — built-in SourceModifier preset: particles bounce inside
    /// the mask; each collision injects a ripple that warps the underlying photo.
    CollisionRipples,
    /// PCleanup.0.1 — built-in SourceModifier preset: multiplicatively lifts
    /// the luminance of source pixels in the zone-spill region.
    ZoneBrighten,
    /// PCleanup.0.1 — built-in SourceModifier preset: displaces the underlying
    /// photo's UV in a thin band at the zone edge.
    ZoneLens,
    /// PCleanup.0.1 — built-in SourceModifier preset: particles drift through
    /// portal zones, each one displacing source pixels in its radius.
    PortalWarp,
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
        // -------------------------------------------------------------------
        // P4.1.1 — Phase 4 scene-grammar domain terms.
        // -------------------------------------------------------------------
        GlossaryTerm::SceneTemplate => GlossaryEntry {
            headline: "Scene Template",
            body: "A named, parameterised recipe that assembles media, zones, \
                   and FX presets into a ready-to-run scene.  Pick a template \
                   in the wizard, assign a few assets, and confirm — the scene \
                   is live in under a minute.",
        },
        GlossaryTerm::SceneGrammar => GlossaryEntry {
            headline: "Scene Grammar",
            body: "The structural vocabulary of a scene: which zone roles it \
                   binds, which media slots it accepts, and which FX presets it \
                   activates.  Every built-in template documents its grammar so \
                   the zone-mapping step is unambiguous.",
        },
        GlossaryTerm::Wizard => GlossaryEntry {
            headline: "Scene Wizard",
            body: "A guided multi-step flow for creating a scene from a \
                   template: template select → media → zone binding → palette \
                   → tempo → confirm.  Cancel at any step to return to the \
                   previous state; one Cmd-Z undoes the entire commit.",
        },
        GlossaryTerm::PaletteMood => GlossaryEntry {
            headline: "Palette",
            body: "A curated colour accent set (Warm, Cool, or Neutral) that \
                   templates apply to tint their output.  Warm leans amber and \
                   gold; Cool leans blue and cyan; Neutral leaves the source \
                   colours largely unchanged.",
        },
        GlossaryTerm::Mood => GlossaryEntry {
            headline: "Mood",
            body: "The emotional character of a scene template: Calm (gentle, \
                   slow motion), Energetic (fast, punchy), or Ethereal (soft, \
                   dreamy).  Templates use this to pre-select animation speed \
                   and particle density defaults.",
        },
        GlossaryTerm::TempoSync => GlossaryEntry {
            headline: "Tempo Sync",
            body: "Locks a template's animation speed to the project BPM so \
                   particle bursts, wave cycles, and wipe timings all fall on \
                   the beat.  Requires the BPM clock to be running; toggle \
                   off for freeform, non-musical scenes.",
        },
        GlossaryTerm::BezierHandles => GlossaryEntry {
            headline: "Bezier Handles",
            body: "Smooth-curve control handles for mask polygon edges that \
                   let operators draw arched window reveals and organic shapes \
                   without lots of vertices.  Bezier handles are planned for \
                   Phase 7; current masks use straight-edge polygons only.",
        },
        // -------------------------------------------------------------------
        // P4.1.1 — Built-in scene template display labels.
        // -------------------------------------------------------------------
        GlossaryTerm::WindowReveal => GlossaryEntry {
            headline: "Window Reveal",
            body: "A soft wash of light that flows through masks tagged as \
                   Window zones, evoking daylight filtering through a glass \
                   facade.  Assign a background image and tag your window \
                   masks to see the reveal animate across the surface.",
        },
        GlossaryTerm::PixelDrift => GlossaryEntry {
            headline: "Pixel Drift",
            body: "Fine particles drift slowly and quietly across the source \
                   media, giving a still photograph the feeling of faint \
                   movement without any visible direction or narrative.",
        },
        GlossaryTerm::CollagBloom => GlossaryEntry {
            headline: "Collage Bloom",
            body: "Composites four images in a 2×2 grid with particles \
                   blooming outward from each image edge.  Assign a different \
                   photo to each slot for maximum contrast between the cells.",
        },
        GlossaryTerm::GlowBehindOpenings => GlossaryEntry {
            headline: "Glow Behind Openings",
            body: "Fluid light pools inside masks tagged as Portal zones, \
                   suggesting that warm interior light is spilling through \
                   archways or doorways.  Intensity follows the fluid \
                   simulation viscosity slider.",
        },
        GlossaryTerm::FragmentedPortrait => GlossaryEntry {
            headline: "Fragmented Portrait",
            body: "A portrait image that shatters into fragment-like particles \
                   bouncing off the mask boundary.  Works best with a \
                   high-contrast portrait on a dark background.",
        },
        GlossaryTerm::ArchitecturalWash => GlossaryEntry {
            headline: "Architectural Wash",
            body: "A gentle ripple-wave wash that traces the edges of surfaces \
                   tagged as Edge zones — sills, reveals, and trims.  \
                   Upgrade of the v3 Architectural Wash FX preset; the \
                   underlying effect is unchanged, but this scene template \
                   adds media input and zone composition.",
        },
        GlossaryTerm::MaskEdgeRippleWashScene => GlossaryEntry {
            headline: "Mask-Edge Ripple Wash (Scene)",
            body: "The classic ripple-wash FX preset promoted to a full scene \
                   template for one-click setup.  No source media required — \
                   the effect generates its own visual content from the mask \
                   boundary alone.",
        },
        GlossaryTerm::LightSpillFromWindows => GlossaryEntry {
            headline: "Light Spill from Windows",
            body: "Light appears to leak outward from masks tagged as Window \
                   zones, as if an interior lamp is casting through the \
                   aperture onto the surrounding wall.  Assign an interior-\
                   light image to the media slot for the strongest effect.",
        },
        // -------------------------------------------------------------------
        // P5.1.1 — Phase 5 lighting output domain terms.
        // -------------------------------------------------------------------
        GlossaryTerm::DmxUniverse => GlossaryEntry {
            headline: "DMX Universe",
            body: "A 512-channel DMX data packet, the fundamental unit of \
                   lighting network traffic.  Each Art-Net or sACN packet \
                   carries one universe; fixtures are addressed by universe \
                   number + channel offset within that universe.",
        },
        GlossaryTerm::FixtureGroup => GlossaryEntry {
            headline: "Fixture Group",
            body: "A named collection of fixtures that share a personality \
                   (channel layout), DMX universe, and canvas sampling region.  \
                   rmap sends one DMX value per fixture in the group each \
                   frame, derived from the assigned canvas area.",
        },
        GlossaryTerm::PixelMap => GlossaryEntry {
            headline: "Pixel Map",
            body: "A grid of UV sample points spread across a canvas region.  \
                   rmap averages the pixel colours at each grid point and maps \
                   the result to the fixture group's DMX channels, letting \
                   physical lights chase the projected image in real time.",
        },
        GlossaryTerm::DmxTransport => GlossaryEntry {
            headline: "DMX Transport",
            body: "The network protocol used to carry DMX universe packets to \
                   fixtures.  Phase 5 ships Art-Net (UDP, port 6454); the \
                   transport abstraction lets Phase 7 add sACN (E1.31) without \
                   changing the fixture or canvas-sampling code.",
        },
        GlossaryTerm::ArtNetTransport => GlossaryEntry {
            headline: "Art-Net Transport",
            body: "The Phase 5 implementation of the DMX Transport that sends \
                   Art-Net ArtDmx PDUs over UDP to a configurable unicast or \
                   subnet-broadcast address on port 6454.  Compatible with \
                   Enttec, Artistic Licence, and most budget Art-Net nodes.",
        },
        GlossaryTerm::ChannelRole => GlossaryEntry {
            headline: "Channel Role",
            body: "The function of a single DMX channel within a fixture's \
                   footprint — Red, Green, or Blue in Phase 5.  rmap uses the \
                   channel map to write the correct colour byte to the correct \
                   DMX address; unknown roles are left at zero and will be \
                   extended in Phase 7.",
        },
        GlossaryTerm::ColorStrategy => GlossaryEntry {
            headline: "Colour Strategy",
            body: "How the sampled canvas pixel colour is converted to DMX byte \
                   values.  RGB Direct copies the pixel bytes unchanged.  HSV \
                   Intensity Gate dims all channels by the pixel's brightness \
                   so dark canvas areas fade the fixture toward black.  \
                   RGBW Fill is planned for Phase 7.",
        },
        GlossaryTerm::LightingTap => GlossaryEntry {
            headline: "Lighting Tap",
            body: "A 64×36 downsampled copy of the composited canvas rendered \
                   once per frame on the GPU and read back by the lighting \
                   thread.  The low resolution keeps readback bandwidth to \
                   under 10 KB per frame, leaving the render thread unaffected.",
        },
        GlossaryTerm::OutputStrategy => GlossaryEntry {
            headline: "Output Strategy",
            body: "The per-fixture-group setting that controls how canvas colour \
                   is translated to DMX values: RGB Direct for accurate colour \
                   reproduction, HSV Intensity Gate for brightness-following \
                   wash behaviour.  Additional strategies are planned for Phase 7.",
        },
        // -------------------------------------------------------------------
        // P6.1.1 — Phase 6 show-control and timecode domain terms.
        // -------------------------------------------------------------------
        GlossaryTerm::Cue => GlossaryEntry {
            headline: "Cue",
            body: "A single entry in the cuelist: a saved scene snapshot plus \
                   per-cue timing (in-time, hold, out-time), fire mode (Follow \
                   or Go-on-trigger), BPM-bar quantize, and an optional timecode \
                   trigger position.",
        },
        GlossaryTerm::Cuelist => GlossaryEntry {
            headline: "Cuelist",
            body: "The ordered list of cues the operator steps through during a \
                   live show.  Navigate with ←/→ to arm the next cue and Space \
                   (or MIDI 60) to fire it.  Cues in Follow mode advance \
                   automatically without operator input.",
        },
        GlossaryTerm::ArmedNext => GlossaryEntry {
            headline: "Armed Next",
            body: "The next cue in the cuelist, highlighted with an amber ring in \
                   the cue strip and ready to fire on the next go command (Space / \
                   MIDI Note 60 / OSC /rmap/cue/go).  Move the arm with ←/→ \
                   without firing.",
        },
        GlossaryTerm::LiveCue => GlossaryEntry {
            headline: "Live Cue",
            body: "The cue currently playing or crossfading on the projector.  \
                   Shown with a \"LIVE\" badge on its tile and a progress ring \
                   during the in-time crossfade.  The live cue advances to the \
                   next cue on a go command or automatically in Follow mode.",
        },
        GlossaryTerm::FollowChain => GlossaryEntry {
            headline: "Follow Chain",
            body: "A sequence of cues whose fire mode is set to Follow — they \
                   advance automatically from one to the next after each cue's \
                   hold time expires, without operator input.  The chain halts \
                   at the first Go-on-trigger cue or when the list is exhausted.",
        },
        GlossaryTerm::GoOnTrigger => GlossaryEntry {
            headline: "Go-on-trigger",
            body: "A cue fire mode that pauses the follow chain and waits for an \
                   explicit go command (Space / MIDI Note 60 / OSC /rmap/cue/go) \
                   before advancing to the next cue.  Use this for cues that \
                   require the operator to confirm timing live.",
        },
        GlossaryTerm::FollowMode => GlossaryEntry {
            headline: "Follow (cue mode)",
            body: "A cue fire mode that fires the next cue automatically after the \
                   current cue's hold time expires, with no operator input required.  \
                   Chain multiple Follow-mode cues to build an auto-advancing \
                   sequence; the chain halts at the first Go-on-trigger cue.",
        },
        GlossaryTerm::InTime => GlossaryEntry {
            headline: "In-time",
            body: "The crossfade duration (seconds) from the previous cue's state \
                   into this cue's scene snapshot.  0.0 snaps instantly; values up \
                   to 60 s create a slow dissolve.  The progress ring on the live \
                   cue tile shows in-time completion.",
        },
        GlossaryTerm::HoldTime => GlossaryEntry {
            headline: "Hold time",
            body: "How long the cue stays fully live after its in-time completes \
                   before the transport can advance (seconds).  \"∞\" (no value) \
                   means hold indefinitely until a go command or timecode trigger \
                   fires.  Used with Follow mode to create timed auto-advance.",
        },
        GlossaryTerm::OutTime => GlossaryEntry {
            headline: "Out-time",
            body: "The crossfade duration (seconds) from this cue's scene as the \
                   next cue's in-time begins.  In most workflows this is 0 because \
                   the next cue's in-time handles the blend; set it non-zero when \
                   you want the outgoing cue to fade before the next one fades in.",
        },
        GlossaryTerm::BpmQuantize => GlossaryEntry {
            headline: "BPM Quantize",
            body: "Defers a cue's fire to the next 1 / 2 / 4 / 8-bar boundary at \
                   the current BPM instead of firing immediately on the go command.  \
                   The armed cue ring stays visible during the wait so the operator \
                   can see the pending fire.  Set to Off to fire on the exact \
                   trigger moment.",
        },
        GlossaryTerm::TimecodePosition => GlossaryEntry {
            headline: "Timecode Position",
            body: "An HH:MM:SS:FF timestamp (hours, minutes, seconds, frames) used \
                   as a cue trigger: when the incoming LTC or MTC timecode signal \
                   reaches this position the transport fires the cue automatically.  \
                   Requires a timecode source (LTC or MTC) to be active.",
        },
        GlossaryTerm::TransportHud => GlossaryEntry {
            headline: "Transport HUD",
            body: "The persistent on-screen panel showing live BPM value, the \
                   current tap source (Space / MIDI / OSC / MIDI Clock), the \
                   armed-next cue name and index, and a global BPM quantize \
                   override selector.  Always visible during Editing and Go-live \
                   states.",
        },
        GlossaryTerm::Ltc => GlossaryEntry {
            headline: "LTC (Linear Timecode)",
            body: "SMPTE 12M timecode carried as an audio-rate biphase-mark \
                   signal on a standard audio cable.  rmap decodes it via the \
                   `ltc` cargo feature (requires libltc) and uses the decoded \
                   HH:MM:SS:FF position to fire timecode-triggered cues within \
                   ±1 frame of the specified position.",
        },
        GlossaryTerm::Mtc => GlossaryEntry {
            headline: "MTC (MIDI Timecode)",
            body: "MIDI Timecode: eight quarter-frame MIDI messages (status 0xF1) \
                   sent by a DAW or hardware sequencer that assemble into a full \
                   HH:MM:SS:FF timecode position.  Decoded inside the MIDI bus \
                   (no extra feature gate) and used to fire timecode-triggered \
                   cues.",
        },
        GlossaryTerm::MidiClock => GlossaryEntry {
            headline: "MIDI Clock",
            body: "MIDI timing clock: 24 pulses per quarter note sent as status \
                   0xF8 messages.  rmap derives a rolling BPM average from the \
                   inter-pulse timing and uses it as an alternative tap source \
                   alongside Space, MIDI Note 60, and OSC /rmap/tap.",
        },
        // -------------------------------------------------------------------
        // P7.1.1 — Phase 7 domain terms.
        // -------------------------------------------------------------------
        GlossaryTerm::Syphon => GlossaryEntry {
            headline: "Syphon",
            body: "macOS inter-application video sharing protocol.  A Syphon \
                   server publishes a texture by name; any Syphon client on the \
                   same machine — OBS, VDMX, Resolume Arena, Millumin — can \
                   subscribe to it without a capture card or network hop.",
        },
        GlossaryTerm::SyphonOutput => GlossaryEntry {
            headline: "Syphon Output",
            body: "When enabled, rmap publishes its composited projector output \
                   as a Syphon source named \"rmap – <project name>\".  Toggle \
                   it in the Output panel; OBS or another Syphon client will see \
                   it in its source picker immediately.",
        },
        GlossaryTerm::CalibrationFile => GlossaryEntry {
            headline: "Calibration File",
            body: "A `.rmap-calibration.json` file that stores the warp mesh, \
                   mask polygon, gamma curve, and display identity for a physical \
                   venue.  Saved once per venue; loaded alongside any show file so \
                   the geometry is always correct without editing the show itself.",
        },
        GlossaryTerm::SurfaceSlot => GlossaryEntry {
            headline: "Surface Slot",
            body: "A logical projector output identified by a stable UUID inside \
                   the calibration file.  The show file's OutputTarget binds to \
                   a surface slot at load time; if the IDs don't match rmap falls \
                   back to an identity warp and shows an audit warning.",
        },
        GlossaryTerm::VenueCalibration => GlossaryEntry {
            headline: "Venue Calibration",
            body: "The complete set of per-surface geometric and colour corrections \
                   (warp, mask, gamma) for a physical installation.  Venue \
                   calibration travels with the room, not the show: one calibration \
                   file works with every show file designed for that venue.",
        },
        GlossaryTerm::BezierWarp => GlossaryEntry {
            headline: "Bezier Warp",
            body: "A warp mesh whose row and column edges are cubic Bezier curves \
                   rather than straight lines.  Pull the tangent handles on any \
                   anchor point to bow a single edge, enabling smooth wrapping on \
                   columns, arches, and organic architectural shapes.",
        },
        GlossaryTerm::Anchor => GlossaryEntry {
            headline: "Anchor",
            body: "A corner point of the Bezier warp mesh that lies exactly on the \
                   projection surface.  Drag an anchor in Anchor mode to reposition \
                   it and its attached tangent handles together; the corner point \
                   itself is always on the surface.",
        },
        GlossaryTerm::TangentHandle => GlossaryEntry {
            headline: "Tangent Handle",
            body: "A control point connected to an anchor by a thin line.  Dragging \
                   the handle bows the adjacent Bezier edge without moving the \
                   anchor.  In Tangent mode the two handles of a smooth pair mirror \
                   each other; hold Shift to break symmetry for a cusp corner.",
        },
        GlossaryTerm::InverseMask => GlossaryEntry {
            headline: "Inverse Mask",
            body: "Flips the mask so the region that was blocked becomes revealed \
                   and the region that was visible becomes blocked.  Toggle \
                   \"Inverse\" in the Mask sub-row; undo restores the previous \
                   state.",
        },
        GlossaryTerm::LumaKey => GlossaryEntry {
            headline: "Luma Key",
            body: "Derives the mask alpha from the brightness of the rendered \
                   output: pixels above the threshold become opaque; pixels below \
                   become transparent (or the reverse when Inverse is also active).  \
                   Adjust the Threshold and Softness sliders in the Mask panel.",
        },
        GlossaryTerm::ChromaKey => GlossaryEntry {
            headline: "Chroma Key",
            body: "Removes a specific hue range from the rendered output by setting \
                   those pixels to transparent — the classic green-screen technique.  \
                   Set the Hue Centre, Hue Range, Saturation Threshold, and Softness \
                   in the Mask panel; works on any colour, not just green.",
        },
        GlossaryTerm::Rgbw => GlossaryEntry {
            headline: "RGBW",
            body: "A four-channel DMX colour model (Red, Green, Blue, White) \
                   used by warm-white architectural LED fixtures.  rmap extracts \
                   the white component from the sampled canvas colour using the \
                   fixture group's CCT, reducing colour noise at high intensities \
                   compared with driving the white channel from raw pixel data.",
        },
        GlossaryTerm::ColourTemperature => GlossaryEntry {
            headline: "Colour Temperature",
            body: "The Kelvin value describing the warmth or coolness of a light \
                   source.  2700 K is warm amber (like an incandescent lamp); \
                   6500 K is cool blue-white (like daylight).  Set the colour \
                   temperature of an RGBW fixture group to match the physical \
                   fixture spec so the White channel renders correctly.",
        },
        GlossaryTerm::Cct => GlossaryEntry {
            headline: "CCT (Correlated Colour Temperature)",
            body: "Short for Correlated Colour Temperature: the Kelvin value that \
                   best describes the white point of a light source.  rmap uses the \
                   CCT setting on an RGBW fixture group to compute how much of the \
                   sampled canvas colour flows into the White DMX channel versus \
                   the Red, Green, and Blue channels.",
        },
        GlossaryTerm::ScenePack => GlossaryEntry {
            headline: "Scene Pack",
            body: "A portable `.rmap-scene-pack.zip` archive containing one or more \
                   scene templates and their referenced assets (images, SVGs, FX \
                   presets).  Export a scene pack from the layer context menu; \
                   import it via File > Import Scene Pack to make its templates \
                   available in the Preset Browser.",
        },
        GlossaryTerm::EdgeBlendGradient => GlossaryEntry {
            headline: "Edge-Blend Gradient",
            body: "A calibration verify pattern that fades from fully opaque to \
                   transparent across a configurable screen edge and blend width.  \
                   Used to set up and verify the overlap zone between two adjacent \
                   projectors; deactivate when the show is running.",
        },
        GlossaryTerm::CalibrationVerify => GlossaryEntry {
            headline: "Calibration Verify",
            body: "A full-screen test pattern (alignment cross, dot grid, colour \
                   bars, edge-blend gradient, focus chart, or geometry grid) \
                   rendered over the projector output to verify warp accuracy, \
                   colour, and focus.  Select a pattern in the Output panel's \
                   Verify section; deactivate to return to the show.",
        },
        // PCleanup.0.1 — Cleanup phase entries.
        GlossaryTerm::FxFamilySourceModifier => GlossaryEntry {
            headline: "Source-Modifying FX",
            body: "A class of FX preset that reads the underlying layer image \
                   and writes a modified version — warping, lensing, brightening, \
                   or smearing the photo — rather than painting its own pixels on \
                   top.  Distinct from generative overlays like ripple wash or \
                   edge emission.",
        },
        GlossaryTerm::EffectTreatment => GlossaryEntry {
            headline: "Treatment (per-layer)",
            body: "An effect-chain variant that runs a single treatment preset \
                   (tone map, displacement ripple, refraction, palette extract, \
                   collage, etc.) on one layer only, rather than as a global \
                   pass over the composited frame.  Lets you grade or warp one \
                   layer while others remain untouched.",
        },
        GlossaryTerm::EffectFeedback => GlossaryEntry {
            headline: "Feedback / Trails",
            body: "An effect that blends the previous frame's layer output back \
                   into the current frame, producing trails, echoes, and motion \
                   smear.  Decay controls trail length; offset adds directional \
                   motion-trail.  Decay is modulator-driven so the trail can \
                   pulse with audio or MIDI.",
        },
        GlossaryTerm::EffectTint => GlossaryEntry {
            headline: "Tint",
            body: "A colour-mixing effect with three modes: multiply (proper tint \
                   that darkens toward the chosen colour), additive (wash that \
                   lightens), and screen.  The amount slider is modulator-driven \
                   so the tint can pulse with audio or MIDI input.",
        },
        GlossaryTerm::MaskNodeUnion => GlossaryEntry {
            headline: "Mask Union",
            body: "A mask-graph operation that combines two mask polygons so the \
                   result covers either region — the Boolean union of overlapping \
                   shapes.  Use it when you want one logical mask out of two \
                   separately drawn polygons.",
        },
        GlossaryTerm::MaskNodeSubtract => GlossaryEntry {
            headline: "Mask Subtract",
            body: "A mask-graph operation that cuts the second polygon out of the \
                   first, leaving the area the first mask covers minus the area \
                   the second mask covers.  Useful for window-with-mullion shapes \
                   and other 'hole through' geometry.",
        },
        GlossaryTerm::FluidWarp => GlossaryEntry {
            headline: "Fluid Warp",
            body: "Source-modifying preset: runs a 2D fluid simulation inside \
                   the mask, displacing the underlying photo by the velocity \
                   field.  The image flows like water; amplitude scales the \
                   distortion.  Mask-bounded — fluid never leaks outside the \
                   shape.",
        },
        GlossaryTerm::RippleLens => GlossaryEntry {
            headline: "Ripple Lens",
            body: "Source-modifying preset: concentric rings from the mask edge \
                   act as refraction lenses, bulging and contracting the \
                   underlying image in bands.  Optional chromatic split per ring \
                   gives a chromatic-aberration look.",
        },
        GlossaryTerm::EdgeLens => GlossaryEntry {
            headline: "Edge Lens",
            body: "Source-modifying preset: four traveling refraction bumps orbit \
                   the mask edge, distorting the image at each crest and letting \
                   it recover between them — a 'force field' look without any \
                   overlay geometry.",
        },
        GlossaryTerm::FluidWarpFull => GlossaryEntry {
            headline: "Fluid Warp (Full)",
            body: "Source-modifying preset: like Fluid Warp but unbounded — the \
                   underlying photo flows across the entire layer, not just \
                   inside the mask.  Pair with Fluid Warp on a sibling layer for \
                   'fluid inside, calm outside' compositions.",
        },
        GlossaryTerm::Spotlights => GlossaryEntry {
            headline: "Spotlights",
            body: "Source-modifying preset: each particle becomes a soft Gaussian \
                   brightener that lifts source-pixel luminance in its radius.  \
                   The underlying photo stays visible everywhere; particles light \
                   it up rather than painting on top.",
        },
        GlossaryTerm::DriftPinholes => GlossaryEntry {
            headline: "Drift Pinholes",
            body: "Source-modifying preset: white-dot particles drift inside the \
                   mask, but instead of being opaque dots each one is a pinhole \
                   through which the underlying photo is visible.  The layer \
                   becomes a moving stencil of the image.",
        },
        GlossaryTerm::EdgeSparks => GlossaryEntry {
            headline: "Edge Sparks",
            body: "Source-modifying preset: particles fly outward from the mask \
                   edge and each one additively lifts the underlying source's \
                   luminance in a soft radius — sparks light up the photo rather \
                   than overlaying opaque dots.",
        },
        GlossaryTerm::FieldAdvectSource => GlossaryEntry {
            headline: "Field Advect Source",
            body: "Source-modifying preset: uses the mask SDF gradient field to \
                   advect the underlying photo along the mask normals over time.  \
                   No visible particles — the gradient acts on the image \
                   directly.",
        },
        GlossaryTerm::CollisionRipples => GlossaryEntry {
            headline: "Collision Ripples",
            body: "Source-modifying preset: particles bounce inside the mask, and \
                   each collision event injects a small ripple into a \
                   displacement field that warps the underlying photo — physical \
                   interaction between simulation and image.",
        },
        GlossaryTerm::ZoneBrighten => GlossaryEntry {
            headline: "Zone Brighten",
            body: "Source-modifying preset: like Zone Light Spill but instead of \
                   adding a warm colour overlay, multiplicatively lifts the \
                   luminance of source pixels in the spill region.  Same falloff \
                   curve, more grounded look.",
        },
        GlossaryTerm::ZoneLens => GlossaryEntry {
            headline: "Zone Lens",
            body: "Source-modifying preset: displaces the underlying photo's UV \
                   in a thin band at the zone edge, so the image warps when you \
                   cross through the zone perimeter.  Rest of the layer remains \
                   untouched.",
        },
        GlossaryTerm::PortalWarp => GlossaryEntry {
            headline: "Portal Warp",
            body: "Source-modifying preset: particles drift through portal zones \
                   (doorways, openings) and each one displaces source pixels in \
                   its radius — produces a 'ghost moving through the room' \
                   effect on a photo of the room.",
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
        // P4.1.1 — Phase 4 scene-grammar domain terms.
        GlossaryTerm::SceneTemplate,
        GlossaryTerm::SceneGrammar,
        GlossaryTerm::Wizard,
        GlossaryTerm::PaletteMood,
        GlossaryTerm::Mood,
        GlossaryTerm::TempoSync,
        GlossaryTerm::BezierHandles,
        // P4.1.1 — Built-in scene template display labels.
        GlossaryTerm::WindowReveal,
        GlossaryTerm::PixelDrift,
        GlossaryTerm::CollagBloom,
        GlossaryTerm::GlowBehindOpenings,
        GlossaryTerm::FragmentedPortrait,
        GlossaryTerm::ArchitecturalWash,
        GlossaryTerm::MaskEdgeRippleWashScene,
        GlossaryTerm::LightSpillFromWindows,
        // P5.1.1 — Phase 5 lighting output domain terms.
        GlossaryTerm::DmxUniverse,
        GlossaryTerm::FixtureGroup,
        GlossaryTerm::PixelMap,
        GlossaryTerm::DmxTransport,
        GlossaryTerm::ArtNetTransport,
        GlossaryTerm::ChannelRole,
        GlossaryTerm::ColorStrategy,
        GlossaryTerm::LightingTap,
        GlossaryTerm::OutputStrategy,
        // P6.1.1 — Phase 6 show-control and timecode domain terms.
        GlossaryTerm::Cue,
        GlossaryTerm::Cuelist,
        GlossaryTerm::ArmedNext,
        GlossaryTerm::LiveCue,
        GlossaryTerm::FollowChain,
        GlossaryTerm::GoOnTrigger,
        GlossaryTerm::FollowMode,
        GlossaryTerm::InTime,
        GlossaryTerm::HoldTime,
        GlossaryTerm::OutTime,
        GlossaryTerm::BpmQuantize,
        GlossaryTerm::TimecodePosition,
        GlossaryTerm::TransportHud,
        GlossaryTerm::Ltc,
        GlossaryTerm::Mtc,
        GlossaryTerm::MidiClock,
        // P7.1.1 — Phase 7 domain terms.
        GlossaryTerm::Syphon,
        GlossaryTerm::SyphonOutput,
        GlossaryTerm::CalibrationFile,
        GlossaryTerm::SurfaceSlot,
        GlossaryTerm::VenueCalibration,
        GlossaryTerm::BezierWarp,
        GlossaryTerm::Anchor,
        GlossaryTerm::TangentHandle,
        GlossaryTerm::InverseMask,
        GlossaryTerm::LumaKey,
        GlossaryTerm::ChromaKey,
        GlossaryTerm::Rgbw,
        GlossaryTerm::ColourTemperature,
        GlossaryTerm::Cct,
        GlossaryTerm::ScenePack,
        GlossaryTerm::EdgeBlendGradient,
        GlossaryTerm::CalibrationVerify,
        // PCleanup.0.1 — Cleanup phase architectural variants.
        GlossaryTerm::FxFamilySourceModifier,
        GlossaryTerm::EffectTreatment,
        GlossaryTerm::EffectFeedback,
        GlossaryTerm::EffectTint,
        GlossaryTerm::MaskNodeUnion,
        GlossaryTerm::MaskNodeSubtract,
        // PCleanup.0.1 — Cleanup phase W2 SourceModifier preset names.
        GlossaryTerm::FluidWarp,
        GlossaryTerm::RippleLens,
        GlossaryTerm::EdgeLens,
        GlossaryTerm::FluidWarpFull,
        GlossaryTerm::Spotlights,
        GlossaryTerm::DriftPinholes,
        GlossaryTerm::EdgeSparks,
        GlossaryTerm::FieldAdvectSource,
        GlossaryTerm::CollisionRipples,
        GlossaryTerm::ZoneBrighten,
        GlossaryTerm::ZoneLens,
        GlossaryTerm::PortalWarp,
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
        const EXPECTED_VARIANT_COUNT: usize = 147;
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
